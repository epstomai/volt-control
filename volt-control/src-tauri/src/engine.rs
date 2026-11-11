//! Connection lifecycle and the glue between UASrv, the model, the UI and MIDI.

use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow, bail};
use serde::Serialize;
use serde_json::{Map, Value, json};
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;

use crate::config::{Config, Store};
use crate::helper;
use crate::i18n::{self, Lang, Msg};
use crate::midi::{self, MidiMsg, MidiRuntime, Outcome};
use crate::model::{self, Kind, Model, Op, Param};
use crate::screen;
use crate::system;
use crate::uasrv::{self, Client, Push};

const RECONNECT_DELAY: Duration = Duration::from_secs(2);
/// Time a freshly launched UAMixerHelper gets to open its port before it is
/// launched again (it normally listens within ~5 s).
const HELPER_STARTUP: Duration = Duration::from_secs(20);
/// Topology changes (stereo link) just need a quick re-discovery.
const REDISCOVER_DELAY: Duration = Duration::from_millis(300);
/// Flush period for coalesced writes, UI updates and meters.
const TICK: Duration = Duration::from_millis(15);
/// UAMixerHelper resets clients that send nothing for ~75 s (measured), so
/// ping well inside that window; a missing reply also reveals a hung server.
const KEEPALIVE: Duration = Duration::from_secs(20);
/// How often configured MIDI ports are checked for unplug / replug.
const MIDI_WATCH: Duration = Duration::from_secs(3);

/// No meter push for this long: treat meters as gone (they normally arrive at ~50 Hz).
const METER_SILENCE: Duration = Duration::from_millis(250);

pub const EV_STATUS: &str = "volt://status";
pub const EV_SNAPSHOT: &str = "volt://snapshot";
pub const EV_PARAMS: &str = "volt://params";
pub const EV_METERS: &str = "volt://meters";
pub const EV_NOTICE: &str = "volt://notice";
pub const EV_MIDI_ACTIVITY: &str = "midi://activity";
pub const EV_MIDI_MAPPINGS: &str = "midi://mappings";

#[derive(Clone, Debug, Serialize)]
pub struct Status {
    pub connected: bool,
    pub message: String,
    /// Source of `message`, rendered again when the language changes.
    #[serde(skip)]
    msg: Msg,
}

impl Status {
    fn new(connected: bool, msg: Msg) -> Status {
        Status { connected, message: msg.render(), msg }
    }
}

/// Where a write came from (MIDI mappings are not echoed back to themselves).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Origin {
    Ui,
    Midi(u64),
}

pub struct Shared {
    pub app: AppHandle,
    pub model: Mutex<Option<Model>>,
    /// Parameter kinds from the last discovery. Kept while offline so MIDI
    /// learn still works during a reconnect.
    kinds: Mutex<HashMap<String, Kind>>,
    /// Windows system volume; None until the worker has read it.
    system: Mutex<Option<system::State>>,
    system_tx: OnceLock<std::sync::mpsc::Sender<system::Cmd>>,
    pub status: Mutex<Status>,
    pub config: Mutex<Config>,
    pub store: Store,
    pub midi: Mutex<MidiRuntime>,
    pub midi_tx: mpsc::UnboundedSender<MidiMsg>,
    ops: mpsc::UnboundedSender<Op>,
    /// Last user activity, for screen sleep (see `screen`).
    pub idle: Mutex<Instant>,
}

/// Why a session ended.
enum End {
    /// Connection or device lost: retry after [`RECONNECT_DELAY`].
    Lost,
    /// Bus layout changed (e.g. stereo link): rebuild the model right away.
    Rediscover,
}

pub fn start(app: AppHandle, store: Store) -> Arc<Shared> {
    let config = store.load();
    let (ops_tx, ops_rx) = mpsc::unbounded_channel();
    let (midi_tx, midi_rx) = mpsc::unbounded_channel();
    let shared = Arc::new(Shared {
        app,
        model: Mutex::new(None),
        kinds: Mutex::new(system::kinds().into_iter().collect()),
        system: Mutex::default(),
        system_tx: OnceLock::new(),
        status: Mutex::new(Status::new(false, Msg::new("status.starting"))),
        config: Mutex::new(config),
        store,
        midi: Mutex::new(MidiRuntime::default()),
        midi_tx,
        ops: ops_tx,
        idle: Mutex::new(Instant::now()),
    });
    let errors = shared.reopen_midi();
    for e in errors {
        shared.notice(&e);
    }
    tauri::async_runtime::spawn(connection_loop(shared.clone(), ops_rx));
    tauri::async_runtime::spawn(midi_loop(shared.clone(), midi_rx));
    tauri::async_runtime::spawn(midi_watch(shared.clone()));
    let sh = shared.clone();
    let tx = system::spawn(move |s| sh.system_changed(s));
    let _ = shared.system_tx.set(tx);
    shared
}

/// Reopens configured MIDI ports that appear later (still held by a previous
/// process at startup, or replugged) and drops ports that were unplugged.
async fn midi_watch(shared: Arc<Shared>) {
    let mut every = tokio::time::interval(MIDI_WATCH);
    loop {
        every.tick().await;
        let want = shared.config.lock().unwrap().midi.clone();
        let (open_in, open_out) = {
            let g = shared.midi.lock().unwrap();
            (g.io.open_inputs.clone(), g.io.open_output.clone())
        };
        let ports = midi::list_ports();
        let appeared = want.inputs.iter().any(|n| !open_in.contains(n) && ports.inputs.contains(n))
            || want.output.as_ref().is_some_and(|n| open_out.as_ref() != Some(n) && ports.outputs.contains(n));
        let vanished = open_in.iter().any(|n| !ports.inputs.contains(n))
            || open_out.as_ref().is_some_and(|n| !ports.outputs.contains(n));
        if !appeared && !vanished {
            continue;
        }
        let errors = shared.reopen_midi();
        if vanished {
            shared.notice(i18n::t("notice.midi_gone"));
        } else if errors.is_empty() {
            shared.notice(i18n::t("notice.midi_back"));
            shared.resync_feedback();
        } else {
            // Present but not openable (e.g. held exclusively): retry quietly.
            log::debug!("MIDI reopen: {errors:?}");
        }
    }
}

impl Shared {
    fn set_status(&self, connected: bool, msg: Msg) {
        let st = Status::new(connected, msg);
        *self.status.lock().unwrap() = st.clone();
        let _ = self.app.emit(EV_STATUS, st);
    }

    /// Switches the UI language (None = follow Windows) and re-translates
    /// what the backend already sent. Returns the language now in effect.
    pub fn set_language(&self, setting: Option<Lang>) -> Lang {
        self.config.lock().unwrap().language = setting;
        self.save_config();
        let lang = i18n::resolve(setting);
        i18n::set(lang);
        if let Some(m) = self.model.lock().unwrap().as_mut() {
            m.relabel();
        }
        let (connected, msg) = {
            let st = self.status.lock().unwrap();
            (st.connected, st.msg.clone())
        };
        self.set_status(connected, msg);
        self.update_tray();
        lang
    }

    pub fn notice(&self, msg: &str) {
        log::warn!("{msg}");
        let _ = self.app.emit(EV_NOTICE, msg);
    }

    pub fn save_config(&self) {
        let cfg = self.config.lock().unwrap().clone();
        if let Err(e) = self.store.save(&cfg) {
            self.notice(&i18n::tf("notice.save_failed", &[&e]));
        }
    }

    pub fn reopen_midi(&self) -> Vec<String> {
        let cfg = self.config.lock().unwrap().midi.clone();
        self.midi.lock().unwrap().io.open(&cfg, self.midi_tx.clone())
    }

    /// Current value of a parameter, device or system.
    pub fn param(&self, id: &str) -> Option<Param> {
        if id.starts_with("win.") {
            return self.system_params().into_iter().find(|p| p.id == id);
        }
        self.model.lock().unwrap().as_ref().and_then(|m| m.get(id).cloned())
    }

    pub fn system_params(&self) -> Vec<Param> {
        self.system.lock().unwrap().map(system::params).unwrap_or_default()
    }

    /// The system volume changed outside this app (volume keys, the flyout, another device).
    fn system_changed(&self, s: system::State) {
        let old = self.system.lock().unwrap().replace(s);
        let params: Vec<Param> = system::params(s)
            .into_iter()
            .filter(|p| match old {
                None => true,
                Some(o) if p.id == system::VOLUME => o.volume != s.volume,
                Some(o) => o.mute != s.mute,
            })
            .collect();
        let _ = self.app.emit(EV_PARAMS, &params);
        self.feedback(&params, Origin::Ui);
        if old.is_some() {
            self.activity(); // volume keys: someone is at the desk
        }
    }

    fn set_system(&self, id: &str, n: f64, origin: Origin) -> Result<()> {
        let tx = self.system_tx.get().ok_or_else(|| anyhow!(i18n::t("err.system_volume").to_string()))?;
        let s = {
            let mut g = self.system.lock().unwrap();
            let mut s = g.ok_or_else(|| anyhow!(i18n::t("err.system_volume").to_string()))?;
            let cmd = match id {
                system::VOLUME => {
                    s.volume = n as f32;
                    system::Cmd::Volume(s.volume)
                }
                system::MUTE => {
                    s.mute = n >= 0.5;
                    system::Cmd::Mute(s.mute)
                }
                _ => bail!("unknown parameter {id}"),
            };
            tx.send(cmd).map_err(|_| anyhow!(i18n::t("err.system_volume").to_string()))?;
            *g = Some(s);
            s
        };
        if let Some(p) = system::params(s).into_iter().find(|p| p.id == id) {
            let _ = self.app.emit(EV_PARAMS, [&p]);
            self.feedback(std::slice::from_ref(&p), origin);
        }
        Ok(())
    }

    /// Moves a parameter (normalized 0..1). Actions fire regardless of `n`.
    pub fn set_param(&self, id: &str, n: f64, origin: Origin) -> Result<()> {
        let limit = self.config.lock().unwrap().limits.get(id).copied();
        let n = limit.map_or(n, |max| n.min(max));
        if id.starts_with("win.") {
            return self.set_system(id, n, origin);
        }
        let (op, param) = {
            let mut guard = self.model.lock().unwrap();
            let m = guard.as_mut().ok_or_else(|| anyhow!(i18n::t("err.not_connected").to_string()))?;
            let op = m.plan_write(id, n)?;
            (op, m.get(id).cloned())
        };
        self.ops.send(op).map_err(|_| anyhow!(i18n::t("err.engine_stopped").to_string()))?;
        if let Some(p) = param.filter(|p| p.kind != Kind::Action) {
            let _ = self.app.emit(EV_PARAMS, [&p]);
            self.feedback(std::slice::from_ref(&p), origin);
        }
        Ok(())
    }

    /// Sends controller feedback for changed params and re-arms soft takeover
    /// on mappings that did not cause the change.
    fn feedback(&self, params: &[Param], origin: Origin) {
        let mappings = self.config.lock().unwrap().midi.mappings.clone();
        let mut rt = self.midi.lock().unwrap();
        let now = Instant::now();
        for p in params {
            for m in mappings.iter().filter(|m| m.param == p.id) {
                if origin == Origin::Midi(m.id) {
                    continue;
                }
                rt.pickup.entry(m.id).or_default().release();
                let busy = rt.last_input.get(&m.id).is_some_and(|t| now.duration_since(*t) < midi::FEEDBACK_HOLDOFF);
                if m.feedback && !busy {
                    for msg in midi::feedback_messages(m, p.normalized) {
                        rt.io.send(&msg);
                    }
                }
            }
        }
    }

    /// Switches the monitor / headphone volume curve and re-syncs UI and controller.
    pub fn set_volume_curve(&self, on: bool) {
        self.config.lock().unwrap().volume_curve = on;
        self.save_config();
        let changed = self.model.lock().unwrap().as_mut().map(|m| m.set_volume_curve(on)).unwrap_or_default();
        let _ = self.app.emit(EV_PARAMS, &changed);
        self.feedback(&changed, Origin::Ui);
    }

    /// Pushes the current value of every mapped parameter to the controller.
    pub fn resync_feedback(&self) {
        let mut params: Vec<Param> = self.model.lock().unwrap().as_ref().map(|m| m.params.clone()).unwrap_or_default();
        params.extend(self.system_params());
        self.feedback(&params, Origin::Ui);
    }

    fn update_tray(&self) {
        let (mon, hp) = {
            let g = self.model.lock().unwrap();
            let d = |id| g.as_ref().and_then(|m| m.get(id)).map(|p| p.display.clone()).unwrap_or("—".into());
            (d("mon.volume"), d("hp.volume"))
        };
        if let Some(tray) = self.app.tray_by_id("main") {
            let _ = tray.set_tooltip(Some(i18n::tf("tray.tooltip", &[&mon, &hp])));
        }
    }
}

/// Makes sure UAMixerHelper is running after a refused connection and returns
/// the status line to show. `launched_at` rate-limits relaunches.
fn ensure_helper(launched_at: &mut Option<Instant>) -> Msg {
    let recent = launched_at.is_some_and(|t| t.elapsed() < HELPER_STARTUP);
    if helper::is_running() {
        return Msg::new(if recent { "status.helper_starting" } else { "status.helper_unresponsive" });
    }
    if recent {
        return Msg::new("status.helper_exited");
    }
    let Some(exe) = helper::find() else {
        return Msg::with("status.helper_missing", &[&helper::EXE]);
    };
    *launched_at = Some(Instant::now());
    match helper::launch(&exe) {
        Ok(()) => Msg::new("status.helper_starting"),
        Err(e) => {
            log::warn!("{e:#}");
            Msg::with("status.helper_failed", &[&format!("{e:#}")])
        }
    }
}

async fn connection_loop(shared: Arc<Shared>, mut ops_rx: mpsc::UnboundedReceiver<Op>) {
    let mut launched_at: Option<Instant> = None;
    loop {
        shared.set_status(false, Msg::new("status.connecting"));
        match Client::connect(uasrv::DEFAULT_ADDR).await {
            Err(e) => {
                log::warn!("connect {}: {e}", uasrv::DEFAULT_ADDR);
                shared.set_status(false, ensure_helper(&mut launched_at));
            }
            Ok((client, mut rx)) => match model::discover(&client).await {
                Err(e) => shared.set_status(false, Msg::raw(e.to_string())),
                Ok(mut model) => {
                    model.set_volume_curve(shared.config.lock().unwrap().volume_curve);
                    let subs = model.subscriptions();
                    let snapshot = model.snapshot();
                    *shared.kinds.lock().unwrap() =
                        snapshot.params.iter().map(|p| (p.id.clone(), p.kind.clone())).chain(system::kinds()).collect();
                    *shared.model.lock().unwrap() = Some(model);
                    let _ = shared.app.emit(EV_SNAPSHOT, &snapshot);
                    let mut failed = 0;
                    for path in &subs {
                        if let Err(e) = client.subscribe(path).await {
                            // Meters of disabled buses (unlinked 1-2, loopback L/R) don't exist yet.
                            if path.ends_with("/meters/level") {
                                log::debug!("meter not available: {path}");
                            } else {
                                failed += 1;
                                log::warn!("subscribe {path}: {e}");
                            }
                        }
                    }
                    while ops_rx.try_recv().is_ok() {} // drop writes queued while offline
                    let msg = if failed > 0 {
                        Msg::with("status.connected_partial", &[&snapshot.device.name, &failed])
                    } else {
                        Msg::with("status.connected", &[&snapshot.device.name])
                    };
                    shared.set_status(true, msg);
                    shared.resync_feedback();
                    shared.update_tray();
                    shared.activity(); // also lights a screen left dark by a previous run
                    let end = run_session(&shared, &client, &mut rx, &mut ops_rx).await;
                    if let End::Rediscover = end {
                        // Keep the UI populated; the new snapshot replaces it in a moment.
                        tokio::time::sleep(REDISCOVER_DELAY).await;
                        continue;
                    }
                    *shared.model.lock().unwrap() = None;
                    let _ = shared.app.emit(EV_SNAPSHOT, Value::Null);
                }
            },
        }
        tokio::time::sleep(RECONNECT_DELAY).await;
    }
}

async fn run_session(
    shared: &Arc<Shared>,
    client: &Client,
    rx: &mut mpsc::UnboundedReceiver<Push>,
    ops_rx: &mut mpsc::UnboundedReceiver<Op>,
) -> End {
    let mut tick = tokio::time::interval(TICK);
    let mut keepalive = tokio::time::interval_at(tokio::time::Instant::now() + KEEPALIVE, KEEPALIVE);
    let mut idle = tokio::time::interval(screen::CHECK);
    let mut sets: HashMap<String, String> = HashMap::new();
    let mut posts: Vec<(String, Value)> = Vec::new();
    let mut changed: BTreeSet<String> = BTreeSet::new();
    let mut meters: HashMap<String, Map<String, Value>> = HashMap::new();
    let mut last_meters: Option<Instant> = None;
    loop {
        tokio::select! {
            push = rx.recv() => match push {
                None => {
                    log::warn!("session ended: UASrv closed the connection");
                    shared.set_status(false, Msg::new("status.lost"));
                    return End::Lost;
                }
                Some(Push::Value { path, value }) => {
                    let ids = shared.model.lock().unwrap().as_mut().map(|m| m.apply(&path, &value)).unwrap_or_default();
                    if ids.iter().any(|id| id == "dev.online") && value == Value::Bool(false) {
                        log::warn!("session ended: device reported offline");
                        shared.set_status(false, Msg::new("status.device_offline"));
                        return End::Lost;
                    }
                    if ids.iter().any(|id| id.starts_with("bus.") && id.ends_with(".enabled")) {
                        log::info!("bus layout changed ({path}); rediscovering");
                        return End::Rediscover;
                    }
                    // A change on the device (knob, buttons, cables) is activity; our
                    // own screen dimming is not.
                    if ids.iter().any(|id| id != screen::BRIGHTNESS) {
                        shared.activity();
                    }
                    changed.extend(ids);
                }
                Some(Push::Meters(list)) => {
                    let g = shared.model.lock().unwrap();
                    let Some(m) = g.as_ref() else { continue };
                    for (path, v) in list {
                        if let Some((obj, prop)) = path.rsplit_once('/')
                            && let Some(ids) = m.meters.get(obj)
                        {
                            let v = if m.meter_percent.contains(obj) && prop != "clip" { percent_to_amplitude(v) } else { v };
                            for id in ids {
                                meters.entry(id.clone()).or_default().insert(prop.to_string(), v.clone());
                            }
                        }
                    }
                }
            },
            op = ops_rx.recv() => match op {
                Some(Op::Set { path, arg }) => { sets.insert(path, arg); }
                Some(Op::Post { path, args }) => posts.push((path, args)),
                None => return End::Lost,
            },
            _ = idle.tick() => shared.check_idle(),
            _ = keepalive.tick() => {
                if let Err(e) = client.get("/ping").await {
                    log::warn!("session ended: keepalive failed: {e}");
                    shared.set_status(false, Msg::new("status.no_response"));
                    return End::Lost;
                }
            },
            _ = tick.tick() => {
                for (path, arg) in sets.drain() {
                    if client.send_line("set", &path, Some(&arg)).is_err() {
                        log::warn!("session ended: write failed");
                        return End::Lost;
                    }
                }
                for (path, args) in posts.drain(..) {
                    let (c, sh) = (client.clone(), shared.clone());
                    tauri::async_runtime::spawn(async move {
                        match c.post(&path, &args).await {
                            Ok(v) if v.get("error").is_some() => sh.notice(&i18n::tf("notice.post_failed", &[&path, &v["error"]])),
                            Ok(_) => {}
                            Err(e) => sh.notice(&i18n::tf("notice.post_failed", &[&path, &e])),
                        }
                    });
                }
                if !changed.is_empty() {
                    let params: Vec<Param> = {
                        let g = shared.model.lock().unwrap();
                        g.as_ref().map(|m| changed.iter().filter_map(|id| m.get(id).cloned()).collect()).unwrap_or_default()
                    };
                    let touches_tray = changed.iter().any(|id| id == "mon.volume" || id == "hp.volume");
                    changed.clear();
                    let _ = shared.app.emit(EV_PARAMS, &params);
                    shared.feedback(&params, Origin::Ui);
                    if touches_tray {
                        shared.update_tray();
                    }
                }
                if !meters.is_empty() {
                    let _ = shared.app.emit(EV_METERS, &meters);
                    meters.clear();
                    last_meters = Some(Instant::now());
                } else if last_meters.is_some_and(|t| t.elapsed() > METER_SILENCE) {
                    // Meter pushes stopped (stalled session): don't leave the last frame lit.
                    let _ = shared.app.emit(EV_METERS, &meters);
                    last_meters = None;
                }
            }
        }
    }
}

/// UA meter-scale positions -> linear amplitude (the UI meters expect amplitude).
fn percent_to_amplitude(v: Value) -> Value {
    match v {
        Value::Array(a) => Value::Array(a.into_iter().map(percent_to_amplitude).collect()),
        Value::Number(n) => n.as_f64().map_or(Value::Number(n), |x| json!(model::meter_amplitude(x))),
        other => other,
    }
}

/// While learning, a CC 0..31 waits this long for its LSB (CC n+32) before it is
/// bound as a plain 7-bit CC.
const CC14_LEARN_WINDOW: Duration = Duration::from_millis(60);

async fn midi_loop(shared: Arc<Shared>, mut rx: mpsc::UnboundedReceiver<MidiMsg>) {
    let mut cc14 = midi::Cc14Assembler::default();
    let mut jitter = midi::JitterFilter::default();
    // A learn candidate that might turn out to be the MSB of a 14-bit pair.
    let mut pending: Option<(MidiMsg, tokio::time::Instant)> = None;
    loop {
        let next = match pending {
            Some((msb, deadline)) => tokio::select! {
                m = rx.recv() => m,
                _ = tokio::time::sleep_until(deadline) => {
                    pending = None;
                    learn_or_report(&shared, msb); // no LSB followed: plain 7-bit CC
                    continue;
                }
            },
            None => rx.recv().await,
        };
        let Some(msg) = next else { break };
        let half = cc14.is_half(&msg);
        let combined = cc14.feed(&msg);
        // A resting fader's flicker is not a move: no LED, no write, no learn.
        let moved = jitter.pass(&combined.unwrap_or(msg));
        log::trace!("midi in {msg:?} combined={combined:?} moved={moved} learn={:?}", shared.midi.lock().unwrap().learn);
        if !moved {
            // Its MSB may be waiting as a learn candidate: drop that too.
            if let (Some(c), Some((p, _))) = (combined, pending)
                && p.channel == c.channel
                && p.number == c.number
            {
                pending = None;
            }
            continue;
        }
        if !half {
            let _ = shared.app.emit(EV_MIDI_ACTIVITY, combined.unwrap_or(msg));
        }
        shared.activity();

        if shared.midi.lock().unwrap().learn.is_some() {
            match (combined, pending) {
                (Some(c), Some((p, _))) if p.channel == c.channel && p.number == c.number => {
                    pending = None;
                    learn_or_report(&shared, c);
                }
                _ if msg.is_cc_msb() => pending = Some((msg, tokio::time::Instant::now() + CC14_LEARN_WINDOW)),
                _ if pending.is_some() => {} // stray message inside the 14-bit detection window
                _ => learn_or_report(&shared, msg),
            }
            continue;
        }
        pending = None;
        log_err(dispatch(&shared, msg));
        if let Some(c) = combined {
            log_err(dispatch(&shared, c));
        }
    }
}

fn log_err(r: Result<()>) {
    if let Err(e) = r {
        log::debug!("midi: {e}");
    }
}

/// Runs [`learn`]; a failure is shown to the user and ends learn mode instead
/// of leaving the control armed with nothing happening.
fn learn_or_report(shared: &Arc<Shared>, msg: MidiMsg) {
    if let Err(e) = learn(shared, msg) {
        shared.midi.lock().unwrap().learn = None;
        shared.notice(&i18n::tf("notice.learn_failed", &[&e]));
        let mappings = shared.config.lock().unwrap().midi.mappings.clone();
        let _ = shared.app.emit(EV_MIDI_MAPPINGS, mappings); // also clears the armed state in the UI
    }
}

/// MIDI learn: binds the waiting parameter to `msg`.
fn learn(shared: &Arc<Shared>, msg: MidiMsg) -> Result<()> {
    let Some(param) = shared.midi.lock().unwrap().learn.clone() else {
        return Ok(()); // learn was cancelled meanwhile
    };
    if msg.kind == midi::MsgKind::Note && msg.value == 0 {
        return Ok(()); // ignore the release of the learning press
    }
    let kind = shared.kinds.lock().unwrap().get(&param).cloned();
    let kind = kind.ok_or_else(|| anyhow!(i18n::tf("err.unknown_param", &[&param])))?;
    {
        let mut cfg = shared.config.lock().unwrap();
        let id = cfg.midi.mappings.iter().map(|m| m.id).max().unwrap_or(0) + 1;
        cfg.midi.mappings.retain(|m| !msg.conflicts_with(m));
        cfg.midi.mappings.push(midi::learned_mapping(id, &param, &kind, &msg));
    }
    shared.midi.lock().unwrap().learn = None;
    shared.save_config();
    let mappings = shared.config.lock().unwrap().midi.mappings.clone();
    let _ = shared.app.emit(EV_MIDI_MAPPINGS, mappings);
    Ok(())
}

/// Applies `msg` to every mapping it matches.
fn dispatch(shared: &Arc<Shared>, msg: MidiMsg) -> Result<()> {
    let mappings: Vec<_> = shared.config.lock().unwrap().midi.mappings.iter().filter(|m| msg.matches(m)).cloned().collect();
    for m in mappings {
        let current = shared.param(&m.param).map(|p| (p.normalized, p.kind));
        let Some((cur, kind)) = current else { continue };
        let outcome = {
            let mut rt = shared.midi.lock().unwrap();
            rt.last_input.insert(m.id, Instant::now());
            let pickup = rt.pickup.entry(m.id).or_default();
            let before = pickup.clone();
            let outcome = midi::resolve(&m, &msg, cur, &kind, pickup);
            log::trace!("midi map#{} {} cur={cur:.4} pickup={before:?} -> {outcome:?}", m.id, m.param);
            outcome
        };
        match outcome {
            Outcome::Set(n) => shared.set_param(&m.param, n, Origin::Midi(m.id))?,
            Outcome::Trigger => shared.set_param(&m.param, 1.0, Origin::Midi(m.id))?,
            Outcome::Ignore => {}
        }
    }
    Ok(())
}
