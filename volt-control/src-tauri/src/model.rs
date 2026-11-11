//! Volt device model: discovers the device in the UASrv tree, builds a flat list
//! of user-facing parameters, applies pushes to them and plans writes.

use std::collections::{BTreeSet, HashMap, HashSet};

use anyhow::{Result, anyhow, bail};
use serde::Serialize;
use serde_json::{Value, json};

use crate::i18n;
use crate::uasrv::Client;

/// Fader value that the server reports for -inf dB.
const MINUS_INF_DB: f64 = -144.0;

/// Output volumes (value 0..100) attenuate by exactly 1 dB per step: 100 = 0 dB,
/// 0 = mute (measured with a physical loopback, see docs/protocol.md). That is a
/// 100 dB range, so the bottom 40 steps are practically silent.
pub mod volume {
    /// Parameters that get the volume curve.
    pub const IDS: [&str; 2] = ["mon.volume", "hp.volume"];
    /// With the curve on, the travel is two straight segments: 0..KNEE covers the
    /// near-silent device values 0..FLOOR quickly, KNEE..1 covers FLOOR..100
    /// evenly (0.67 step = 0.67 dB per 1%). Continuous, so no jump to mute.
    pub const FLOOR: f64 = 40.0;
    pub const KNEE: f64 = 0.1;

    /// Device value (0..100) -> control position (0..1).
    pub fn position(value: f64, curve: bool) -> f64 {
        if !curve {
            return (value / 100.0).clamp(0.0, 1.0);
        }
        let p = if value < FLOOR {
            value / FLOOR * KNEE
        } else {
            KNEE + (value - FLOOR) / (100.0 - FLOOR) * (1.0 - KNEE)
        };
        p.clamp(0.0, 1.0)
    }

    /// Control position (0..1) -> device value (0..100).
    pub fn value(position: f64, curve: bool) -> f64 {
        let p = position.clamp(0.0, 1.0);
        if !curve {
            return p * 100.0;
        }
        if p < KNEE { p / KNEE * FLOOR } else { FLOOR + (p - KNEE) / (1.0 - KNEE) * (100.0 - FLOOR) }
    }

    /// The number the device itself shows.
    pub fn display(value: f64) -> String {
        format!("{}", value.max(0.0).round() as i64)
    }
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Kind {
    /// Set through `normalized_value` (0..1); the server applies the taper.
    Continuous,
    Toggle,
    Choice { options: Vec<String> },
    ReadOnly,
    Action,
}

#[derive(Clone, Debug)]
pub enum Target {
    /// A UASrv control object (`.../controls/<name>`).
    Control { path: String, real_values: Vec<Value> },
    /// A plain property. `range` makes an integer property continuous.
    Prop { obj: String, prop: String, values: Vec<Value>, range: Option<(f64, f64)> },
    /// Stereo link of the input pair: read from the stereo bus's `enabled`,
    /// written through `post /link_channels`.
    Link { obj: String, device_uid: String, channel_uid: String },
    /// A command with fixed arguments.
    Post { path: String, args: Value },
    /// An on/off control whose meaning is flipped: DIRECT monitoring is ON
    /// when the input's monitor-mix `mute` is 0.
    Inverted { path: String },
    /// Windows system volume (see `system`), not a UASrv object.
    System,
}

#[derive(Clone, Debug, Serialize)]
pub struct Param {
    pub id: String,
    pub label: String,
    pub group: String,
    #[serde(flatten)]
    pub kind: Kind,
    pub normalized: f64,
    pub value: Value,
    pub display: String,
    #[serde(skip)]
    pub target: Target,
    /// An output volume (see [`volume`]); `Some(curve)` once known.
    #[serde(skip)]
    pub volume: Option<bool>,
}

/// A wire operation.
#[derive(Clone, Debug)]
pub enum Op {
    Set { path: String, arg: String },
    Post { path: String, args: Value },
}

#[derive(Clone, Debug, Serialize)]
pub struct Bus {
    pub uid: String,
    pub slug: String,
    pub name: String,
    pub bus_type: String,
    pub stereo: bool,
    /// Stereo link parameter of the input pair this bus belongs to (set on the
    /// stereo bus and both mono halves), see [`link_pairs`].
    pub link: Option<String>,
    #[serde(skip)]
    pub path: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct DeviceInfo {
    pub uid: String,
    pub name: String,
    pub device_type: String,
    pub serial: String,
    pub firmware: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct Snapshot {
    pub device: DeviceInfo,
    pub buses: Vec<Bus>,
    pub params: Vec<Param>,
    pub meters: Vec<String>,
}

pub struct Model {
    pub device: DeviceInfo,
    pub buses: Vec<Bus>,
    pub params: Vec<Param>,
    by_id: HashMap<String, usize>,
    /// object path -> params whose state lives on that object
    by_obj: HashMap<String, Vec<usize>>,
    /// meter object path (`.../meters/level`) -> meter ids fed by it
    pub meters: HashMap<String, Vec<String>>,
    /// meters whose values are UA meter-scale positions (`units: "percent"`), see [`meter_amplitude`]
    pub meter_percent: HashSet<String>,
}

impl Model {
    pub fn snapshot(&self) -> Snapshot {
        let mut meters: Vec<String> = self.meters.values().flatten().cloned().collect();
        meters.sort();
        Snapshot { device: self.device.clone(), buses: self.buses.clone(), params: self.params.clone(), meters }
    }

    pub fn get(&self, id: &str) -> Option<&Param> {
        self.by_id.get(id).map(|&i| &self.params[i])
    }

    /// Re-translates parameter names after the UI language changed.
    pub fn relabel(&mut self) {
        for p in &mut self.params {
            p.label = i18n::param_label(&p.id);
        }
    }

    /// Paths to subscribe to: every watched object plus every meter.
    pub fn subscriptions(&self) -> Vec<String> {
        let mut set: BTreeSet<String> = self.by_obj.keys().cloned().collect();
        set.extend(self.meters.keys().cloned());
        set.into_iter().collect()
    }

    /// Applies a property update (`<obj>/<prop>`); returns the ids of params that changed.
    pub fn apply(&mut self, path: &str, v: &Value) -> Vec<String> {
        let Some((obj, prop)) = path.rsplit_once('/') else { return vec![] };
        let Some(idxs) = self.by_obj.get(obj) else { return vec![] };
        let mut changed = Vec::new();
        for &i in idxs {
            let p = &mut self.params[i];
            if apply_to_param(p, prop, v) {
                changed.push(p.id.clone());
            }
        }
        changed
    }

    /// Switches the output volume curve; returns the params whose position changed.
    pub fn set_volume_curve(&mut self, curve: bool) -> Vec<Param> {
        let mut changed = Vec::new();
        for p in self.params.iter_mut().filter(|p| volume::IDS.contains(&p.id.as_str())) {
            p.volume = Some(curve);
            set_volume(p, p.value.clone());
            changed.push(p.clone());
        }
        changed
    }

    /// Plans the write that moves `id` to normalized `n` and updates the local
    /// copy optimistically. Actions ignore `n`.
    pub fn plan_write(&mut self, id: &str, n: f64) -> Result<Op> {
        let i = *self.by_id.get(id).ok_or_else(|| anyhow!("unknown parameter {id}"))?;
        let p = &mut self.params[i];
        let n = n.clamp(0.0, 1.0);
        let (kind, target) = (p.kind.clone(), p.target.clone());
        let op = match (&kind, &target) {
            (Kind::ReadOnly, _) => bail!("{id} is read-only"),
            (_, Target::Post { path, args }) => Op::Post { path: path.clone(), args: args.clone() },
            (_, Target::Link { device_uid, channel_uid, .. }) => {
                let on = n >= 0.5;
                Op::Post {
                    path: "/link_channels".into(),
                    args: json!({"device_uid": device_uid, "channel_uid": channel_uid, "stem_format": if on {"stereo"} else {"mono"}}),
                }
            }
            (Kind::Continuous, Target::Control { path, .. }) if p.volume.is_some() => {
                let v = (volume::value(n, p.volume == Some(true)) * 100.0).round() / 100.0;
                set_volume(p, json!(v));
                p.normalized = n; // keep the exact position (no rounding jitter on MIDI feedback)
                Op::Set { path: format!("{path}/value/value"), arg: fmt_num(v) }
            }
            (Kind::Continuous, Target::Control { path, .. }) => {
                p.normalized = n;
                Op::Set { path: format!("{path}/normalized_value/value"), arg: fmt_num(n) }
            }
            (Kind::Toggle, Target::Control { path, .. }) => {
                let v = json!(if n >= 0.5 { 1 } else { 0 });
                set_local(p, v.clone());
                Op::Set { path: format!("{path}/value/value"), arg: v.to_string() }
            }
            (Kind::Toggle, Target::Inverted { path }) => {
                let raw = json!(if n >= 0.5 { 0 } else { 1 });
                set_inverted(p, &raw);
                Op::Set { path: format!("{path}/value/value"), arg: raw.to_string() }
            }
            (Kind::Choice { options }, Target::Control { path, real_values }) => {
                let idx = choice_index(n, options.len());
                let v = real_values.get(idx).cloned().unwrap_or(json!(idx));
                set_local(p, v.clone());
                Op::Set { path: format!("{path}/value/value"), arg: v.to_string() }
            }
            (kind, Target::Prop { obj, prop, values, range }) => {
                let v = match kind {
                    Kind::Toggle if p.value.is_boolean() => json!(n >= 0.5),
                    Kind::Toggle => json!(if n >= 0.5 { 1 } else { 0 }),
                    Kind::Choice { options } => {
                        let idx = choice_index(n, options.len());
                        values.get(idx).map(choice_value).ok_or_else(|| anyhow!("{id}: no choice {idx}"))?
                    }
                    _ => {
                        let (lo, hi) = range.ok_or_else(|| anyhow!("{id}: no range"))?;
                        json!((lo + n * (hi - lo)).round() as i64)
                    }
                };
                let path = format!("{obj}/{prop}/value");
                set_local(p, v.clone());
                Op::Set { path, arg: v.to_string() }
            }
            (k, t) => bail!("{id}: unsupported {k:?} / {t:?}"),
        };
        Ok(op)
    }
}

fn fmt_num(n: f64) -> String {
    // A JSON number with enough precision for smooth faders (n is already clamped to 0..1).
    let s = format!("{n:.6}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn choice_index(n: f64, len: usize) -> usize {
    if len <= 1 { 0 } else { (n * (len - 1) as f64).round() as usize }
}

fn choice_value(v: &Value) -> Value {
    v.get("value").cloned().unwrap_or_else(|| v.clone())
}

fn as_f64(v: &Value) -> Option<f64> {
    v.as_f64().or_else(|| v.as_bool().map(|b| b as u8 as f64))
}

/// Recomputes normalized/display from a raw `value` for kinds where we can.
fn set_local(p: &mut Param, v: Value) {
    let real_values = match &p.target {
        Target::Control { real_values, .. } => real_values.clone(),
        Target::Prop { values, .. } => values.iter().map(choice_value).collect(),
        _ => vec![],
    };
    p.value = v;
    match &p.kind {
        Kind::Toggle | Kind::ReadOnly if p.value.is_boolean() || p.value.is_number() => {
            let on = as_f64(&p.value).unwrap_or(0.0) >= 0.5;
            p.normalized = on as u8 as f64;
            if p.kind == Kind::Toggle {
                p.display = if on { "ON" } else { "OFF" }.into();
            } else {
                p.display = display_of(&p.value);
            }
        }
        Kind::Choice { options } => {
            if let Some(idx) = real_values.iter().position(|rv| values_equal(rv, &p.value)) {
                p.normalized = if options.len() > 1 { idx as f64 / (options.len() - 1) as f64 } else { 0.0 };
                p.display = options.get(idx).cloned().unwrap_or_default();
            }
        }
        Kind::Continuous => {
            if let Target::Prop { range: Some((lo, hi)), .. } = &p.target {
                let x = as_f64(&p.value).unwrap_or(*lo);
                p.normalized = ((x - lo) / (hi - lo)).clamp(0.0, 1.0);
                p.display = format!("{}", x.round() as i64);
            }
        }
        _ => p.display = display_of(&p.value),
    }
}

/// Updates an inverted toggle from the underlying control's raw value.
fn set_inverted(p: &mut Param, raw: &Value) {
    let on = as_f64(raw).unwrap_or(1.0) < 0.5;
    p.value = raw.clone();
    p.normalized = on as u8 as f64;
    p.display = if on { "ON" } else { "OFF" }.into();
}

/// Fallback text for a continuous value the server didn't format.
fn fmt_real(v: &Value) -> String {
    match as_f64(v) {
        Some(x) if x <= MINUS_INF_DB => "-∞".into(),
        Some(x) => format!("{x:.1}"),
        None => display_of(v),
    }
}

fn values_equal(a: &Value, b: &Value) -> bool {
    match (as_f64(a), as_f64(b)) {
        (Some(x), Some(y)) => (x - y).abs() < 1e-6,
        _ => a == b,
    }
}

fn display_of(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => "—".into(),
        other => other.to_string(),
    }
}

/// Server display strings: `-∞` arrives mangled (not UTF-8 on the wire).
fn clean_display(s: &str, value: &Value) -> String {
    if s.contains('\u{FFFD}') || as_f64(value).is_some_and(|x| x <= MINUS_INF_DB) {
        return "-∞".into();
    }
    s.to_string()
}

/// Updates an output volume from its device value (0..100).
fn set_volume(p: &mut Param, v: Value) {
    let x = as_f64(&v).unwrap_or(0.0);
    p.value = v;
    p.normalized = volume::position(x, p.volume == Some(true));
    p.display = volume::display(x);
}

fn apply_to_param(p: &mut Param, prop: &str, v: &Value) -> bool {
    if p.volume.is_some() {
        // Position and text come from the device value; the server's own
        // normalized/string values are linear 0..100.
        return match prop {
            "value" => {
                set_volume(p, v.clone());
                true
            }
            _ => false,
        };
    }
    match &p.target {
        Target::Control { .. } => match prop {
            "normalized_value" => {
                let Some(n) = v.as_f64() else { return false };
                p.normalized = n;
                true
            }
            "string_value" => {
                let s = clean_display(v.as_str().unwrap_or_default(), &p.value);
                p.display = match p.kind {
                    Kind::Toggle => p.display.clone(),
                    _ => s,
                };
                true
            }
            "value" => {
                set_local(p, v.clone());
                if p.kind == Kind::Continuous && as_f64(v).is_some_and(|x| x <= MINUS_INF_DB) {
                    p.display = "-∞".into();
                }
                true
            }
            _ => false,
        },
        Target::Prop { prop: want, .. } if want == prop => {
            set_local(p, v.clone());
            true
        }
        Target::Inverted { .. } if prop == "value" => {
            set_inverted(p, v);
            true
        }
        Target::Link { .. } if prop == "enabled" => {
            set_local(p, v.clone());
            true
        }
        _ => false,
    }
}

/// UA meter scale (`units: "percent"`, measured with sine tones): position -> dBFS peak.
/// From 0 to -21 dB it is exactly `1 + dB/33`; below that the scale is compressed.
const METER_SCALE: [(f64, f64); 14] = [
    (0.0, -100.0),
    (0.0009, -80.0),
    (0.0037, -70.0),
    (0.0149, -60.0),
    (0.0332, -55.0),
    (0.0651, -50.0),
    (0.1003, -45.0),
    (0.1455, -40.0),
    (0.1920, -35.0),
    (0.2425, -30.0),
    (0.2728, -27.0),
    (0.3182, -24.0),
    (0.3637, -21.0),
    (1.0, 0.0),
];

/// Converts a UA meter-scale position (0..1) into linear amplitude.
pub fn meter_amplitude(position: f64) -> f64 {
    if position <= 0.0 {
        return 0.0;
    }
    let p = position.min(1.0);
    let i = METER_SCALE.iter().position(|&(x, _)| x >= p).unwrap_or(METER_SCALE.len() - 1).max(1);
    let ((x0, d0), (x1, d1)) = (METER_SCALE[i - 1], METER_SCALE[i]);
    let db = d0 + (p - x0) / (x1 - x0) * (d1 - d0);
    10f64.powf(db / 20.0)
}

// ---------------------------------------------------------------- discovery

/// UASrv exposes `preamp_compressor` on every Volt, but only the Max and 76
/// models have one: on a Volt 2 Gen 2 it measurably does nothing (docs/protocol.md).
/// Unknown types keep the control.
fn has_compressor(device_type: &str) -> bool {
    !matches!(device_type, "volt_1_gen2" | "volt_2_gen2" | "volt_4_gen2")
}

/// Bus type of the mix the headphone output plays (`master` or `cue_0` / `cue_1`),
/// following the physical outputs' `mix_in_source` chain.
async fn headphone_source(b: &mut Builder<'_>, dev: &str) -> Option<String> {
    let folder = b.obj(&format!("{dev}/outputs")).await?;
    let mut sources = HashMap::new(); // output name -> mix_in_source
    for uid in children(&folder) {
        if let Some(o) = b.obj(&format!("{dev}/outputs/{uid}")).await {
            sources.insert(pstr(&o, "name"), pstr(&o, "mix_in_source"));
        }
    }
    let mut token = sources.iter().find(|(name, _)| name.starts_with("HP"))?.1.clone();
    for _ in 0..3 {
        let (output, bus_type) = match token.as_str() {
            "mon" => return Some("master".into()),
            "cue1" => ("CUE 1", "cue_0"),
            "cue2" => ("CUE 2", "cue_1"),
            _ => return None,
        };
        match sources.get(output).map(String::as_str) {
            Some("mon") => token = "mon".into(),
            _ => return Some(bus_type.into()),
        }
    }
    None
}

fn props(obj: &Value) -> &Value {
    &obj["properties"]
}

fn pv<'a>(obj: &'a Value, key: &str) -> &'a Value {
    &props(obj)[key]["value"]
}

fn pstr(obj: &Value, key: &str) -> String {
    pv(obj, key).as_str().unwrap_or_default().to_string()
}

pub(crate) fn children(obj: &Value) -> Vec<String> {
    obj["children"]
        .as_array()
        .map(|a| a.iter().filter_map(|c| c["path"].as_str().map(String::from)).collect())
        .unwrap_or_default()
}

/// The mono halves of a stereo bus, by name: "ANALOG 1-2" -> ("ANALOG 1", "ANALOG 2").
fn pair_members(name: &str) -> Option<(String, String)> {
    let (first, second) = name.rsplit_once('-')?;
    let prefix = first.trim_end_matches(|c: char| c.is_ascii_digit());
    let digits = |x: &str| !x.is_empty() && x.chars().all(|c| c.is_ascii_digit());
    if !digits(&first[prefix.len()..]) || !digits(second) {
        return None;
    }
    Some((first.to_string(), format!("{prefix}{second}")))
}

/// Marks each stereo input bus and its two mono halves with the pair's link
/// parameter id `in.<stereo slug>.link`. Pairs whose halves are missing stay unlinked.
fn link_pairs(buses: &mut [Bus]) {
    let input = |b: &Bus, stereo: bool| b.bus_type == "input" && b.stereo == stereo;
    for i in 0..buses.len() {
        if !input(&buses[i], true) {
            continue;
        }
        let Some((a, b)) = pair_members(&buses[i].name) else { continue };
        let half = |name: &str| buses.iter().position(|x| input(x, false) && x.name == name);
        let (Some(ia), Some(ib)) = (half(&a), half(&b)) else { continue };
        let id = Some(format!("in.{}.link", buses[i].slug));
        for j in [i, ia, ib] {
            buses[j].link = id.clone();
        }
    }
}

fn slug_for(name: &str, bus_type: &str) -> String {
    match bus_type {
        "master" => "mon".into(),
        "headphone" => "hp".into(),
        "cue_0" => "cue1".into(),
        "cue_1" => "cue2".into(),
        _ => name
            .to_lowercase()
            .replace("analog ", "a")
            .replace("loopback", "lb")
            .replace("talkback", "tb")
            .replace([' ', '-'], ""),
    }
}

struct Builder<'a> {
    c: &'a Client,
    params: Vec<Param>,
    objs: HashMap<String, Value>,
}

impl Builder<'_> {
    async fn obj(&mut self, path: &str) -> Option<Value> {
        if let Some(v) = self.objs.get(path) {
            return Some(v.clone());
        }
        match self.c.get(path).await {
            Ok(v) => {
                self.objs.insert(path.to_string(), v.clone());
                Some(v)
            }
            Err(e) => {
                log::debug!("skip {path}: {e}");
                None
            }
        }
    }

    /// Adds a parameter backed by a UASrv control object; silently skipped if absent.
    async fn control(&mut self, id: &str, group: &str, path: &str) {
        let Some(o) = self.obj(path).await else { return };
        if props(&o).get("control_type").is_none() {
            return;
        }
        let discrete = pv(&o, "discrete").as_bool().unwrap_or(false);
        let steps = pv(&o, "num_steps").as_u64().unwrap_or(0);
        let strings: Vec<String> = pv(&o, "string_values")
            .as_array()
            .map(|a| a.iter().map(display_of).collect())
            .unwrap_or_default();
        let real_values = pv(&o, "real_values").as_array().cloned().unwrap_or_default();
        let boolish = strings.iter().all(|x| matches!(x.to_lowercase().as_str(), "false" | "true" | "off" | "on"));
        let kind = if !discrete {
            Kind::Continuous
        } else if steps == 2 && boolish {
            Kind::Toggle
        } else if !strings.is_empty() {
            Kind::Choice { options: strings }
        } else {
            let opts: Vec<String> = (0..steps).map(|i| i.to_string()).collect();
            Kind::Choice { options: opts }
        };
        let real_values = if real_values.is_empty() {
            match &kind {
                Kind::Choice { options } => (0..options.len()).map(|i| json!(i)).collect(),
                _ => vec![],
            }
        } else {
            real_values
        };
        let value = pv(&o, "value").clone();
        let mut p = Param {
            id: id.into(),
            label: i18n::param_label(id),
            group: group.into(),
            kind,
            normalized: pv(&o, "normalized_value").as_f64().unwrap_or(0.0),
            value: Value::Null,
            display: String::new(),
            target: Target::Control { path: path.into(), real_values },
            volume: None,
        };
        set_local(&mut p, value);
        if p.kind == Kind::Continuous {
            let shown = pv(&o, "string_value").as_str().unwrap_or_default();
            p.display = if shown.is_empty() { fmt_real(&p.value) } else { clean_display(shown, &p.value) };
        }
        self.params.push(p);
    }

    /// Adds an on/off parameter that is ON when the control at `path` is 0.
    async fn inverted(&mut self, id: &str, group: &str, path: &str) {
        let Some(o) = self.obj(path).await else { return };
        if props(&o).get("control_type").is_none() {
            return;
        }
        let mut p = Param {
            id: id.into(),
            label: i18n::param_label(id),
            group: group.into(),
            kind: Kind::Toggle,
            normalized: 0.0,
            value: Value::Null,
            display: String::new(),
            target: Target::Inverted { path: path.into() },
            volume: None,
        };
        set_inverted(&mut p, pv(&o, "value"));
        self.params.push(p);
    }

    /// Adds a parameter backed by a plain property of `obj`.
    async fn prop(&mut self, id: &str, group: &str, obj: &str, prop: &str, kind: Kind, range: Option<(f64, f64)>) {
        let Some(o) = self.obj(obj).await else { return };
        let Some(entry) = props(&o).get(prop) else { return };
        let values = entry.get("values").and_then(Value::as_array).cloned().unwrap_or_default();
        let kind = match kind {
            Kind::Choice { options } if options.is_empty() => Kind::Choice {
                options: values.iter().map(choice_label).collect(),
            },
            k => k,
        };
        let mut p = Param {
            id: id.into(),
            label: i18n::param_label(id),
            group: group.into(),
            kind,
            normalized: 0.0,
            value: Value::Null,
            display: String::new(),
            target: Target::Prop { obj: obj.into(), prop: prop.into(), values, range },
            volume: None,
        };
        set_local(&mut p, entry["value"].clone());
        self.params.push(p);
    }

    fn action(&mut self, id: &str, group: &str, path: &str, args: Value) {
        self.params.push(Param {
            id: id.into(),
            label: i18n::param_label(id),
            group: group.into(),
            kind: Kind::Action,
            normalized: 0.0,
            value: Value::Null,
            display: String::new(),
            target: Target::Post { path: path.into(), args },
            volume: None,
        });
    }
}

fn choice_label(v: &Value) -> String {
    if let Some(s) = v.get("string").and_then(Value::as_str) {
        return s.to_string();
    }
    let raw = choice_value(v);
    match raw.as_f64() {
        Some(hz) if hz >= 1000.0 => format!("{} kHz", hz / 1000.0),
        _ => display_of(&raw),
    }
}

/// Finds the first Volt device and builds the model.
pub async fn discover(c: &Client) -> Result<Model> {
    let devices = c.get("/devices").await?;
    let mut dev_path = None;
    let mut dev_obj = Value::Null;
    for uid in children(&devices) {
        let path = format!("/devices/{uid}");
        let Ok(o) = c.get(&path).await else { continue };
        if pstr(&o, "device_type").starts_with("volt") && pv(&o, "online").as_bool().unwrap_or(true) {
            dev_path = Some(path);
            dev_obj = o;
            break;
        }
    }
    let dev = dev_path.ok_or_else(|| anyhow!(i18n::t("err.no_volt").to_string()))?;
    let device = DeviceInfo {
        uid: pstr(&dev_obj, "uid"),
        name: pstr(&dev_obj, "name"),
        device_type: pstr(&dev_obj, "device_type"),
        serial: pstr(&dev_obj, "serial_number"),
        firmware: pstr(&dev_obj, "firmware_version"),
    };

    let mut b = Builder { c, params: Vec::new(), objs: HashMap::new() };
    b.objs.insert(dev.clone(), dev_obj);

    let mut buses = Vec::new();
    let bus_folder = b.obj(&format!("{dev}/buses")).await.ok_or_else(|| anyhow!("no buses"))?;
    for uid in children(&bus_folder) {
        let path = format!("{dev}/buses/{uid}");
        let Some(o) = b.obj(&path).await else { continue };
        let name = pstr(&o, "name");
        let bus_type = pstr(&o, "bus_type");
        buses.push(Bus {
            slug: slug_for(&name, &bus_type),
            stereo: pstr(&o, "stem_format") == "stereo",
            link: None,
            uid,
            name,
            bus_type,
            path,
        });
    }
    link_pairs(&mut buses);
    let find = |t: &str| buses.iter().find(|x| x.bus_type == t).cloned();

    // --- monitor & headphone
    for (bus, slug) in [(find("master"), "mon"), (find("headphone"), "hp")] {
        let Some(bus) = bus else { continue };
        let ctl = format!("{}/outputs/main/controls", bus.path);
        b.control(&format!("{slug}.volume"), "monitor", &format!("{ctl}/gain")).await;
        b.control(&format!("{slug}.mute"), "monitor", &format!("{ctl}/mute")).await;
        b.control(&format!("{slug}.mono"), "monitor", &format!("{ctl}/mix_to_mono")).await;
    }
    b.prop("dev.dim", "monitor", &dev, "dim_on", Kind::Toggle, None).await;
    for (bus, slug) in [(find("cue_0"), "cue1"), (find("cue_1"), "cue2")] {
        let Some(bus) = bus else { continue };
        let ctl = format!("{}/outputs/main/controls", bus.path);
        b.control(&format!("{slug}.volume"), "cue", &format!("{ctl}/gain")).await;
        b.control(&format!("{slug}.mute"), "cue", &format!("{ctl}/mute")).await;
        b.control(&format!("{slug}.mono"), "cue", &format!("{ctl}/mix_to_mono")).await;
    }

    // --- preamps (mono inputs and the linked stereo bus)
    let inputs: Vec<Bus> = buses.iter().filter(|x| x.bus_type == "input").cloned().collect();
    for bus in &inputs {
        let s = &bus.slug;
        let ctl = format!("{}/controls", bus.path);
        let g = format!("input.{s}");
        b.control(&format!("in.{s}.gain"), &g, &format!("{ctl}/preamp_gain")).await;
        b.control(&format!("in.{s}.48v"), &g, &format!("{ctl}/preamp_48v")).await;
        b.control(&format!("in.{s}.tone"), &g, &format!("{ctl}/preamp_tone")).await;
        if has_compressor(&device.device_type) {
            b.control(&format!("in.{s}.comp"), &g, &format!("{ctl}/preamp_compressor")).await;
        }
        b.control(&format!("in.{s}.invert"), &g, &format!("{ctl}/preamp_invert")).await;
        // DIRECT = this input is heard in the monitor mix (front-panel DIRECT key).
        b.inverted(&format!("in.{s}.direct"), &g, &format!("{}/outputs/main/controls/mute", bus.path)).await;
        let jd = format!("{}/jack_detect", bus.path);
        for k in ["mic", "line", "hiz"] {
            b.prop(&format!("in.{s}.jack_{k}"), &g, &jd, &format!("jack_detect_{k}"), Kind::ReadOnly, None).await;
        }
        b.action(
            &format!("in.{s}.autogain"),
            &g,
            "/auto_gain/trigger_auto_gain",
            json!({"device_uid": device.uid, "channels": [bus.uid], "duration": 0}),
        );
    }
    // --- stereo link per input pair, written through the pair's first mono channel
    for st in inputs.iter().filter(|x| x.stereo) {
        let Some(id) = &st.link else { continue };
        let Some(mono) = inputs.iter().find(|x| !x.stereo && x.link.as_ref() == Some(id)) else { continue };
        let o = b.obj(&st.path).await.unwrap_or(Value::Null);
        let mut p = Param {
            id: id.clone(),
            label: i18n::param_label(id),
            group: format!("input.{}", st.slug),
            kind: Kind::Toggle,
            normalized: 0.0,
            value: Value::Null,
            display: String::new(),
            target: Target::Link { obj: st.path.clone(), device_uid: device.uid.clone(), channel_uid: mono.uid.clone() },
            volume: None,
        };
        set_local(&mut p, pv(&o, "enabled").clone());
        b.params.push(p);
    }

    // --- monitor mixer: every input / loopback bus
    let sources: Vec<Bus> = buses.iter().filter(|x| matches!(x.bus_type.as_str(), "input" | "loopback")).cloned().collect();
    for bus in &sources {
        let s = &bus.slug;
        let g = format!("mix.{s}");
        b.prop(&format!("bus.{s}.enabled"), &g, &bus.path, "enabled", Kind::ReadOnly, None).await;
        b.control(&format!("mix.{s}.solo"), &g, &format!("{}/controls/solo", bus.path)).await;
        for (out, tag) in [("main", "main"), ("cue_0", "cue1"), ("cue_1", "cue2")] {
            let ctl = format!("{}/outputs/{out}/controls", bus.path);
            let pre = format!("mix.{s}.{tag}");
            b.control(&format!("{pre}.gain"), &g, &format!("{ctl}/gain")).await;
            b.control(&format!("{pre}.mute"), &g, &format!("{ctl}/mute")).await;
            if bus.stereo {
                b.control(&format!("{pre}.pan_l"), &g, &format!("{ctl}/pan_left")).await;
                b.control(&format!("{pre}.pan_r"), &g, &format!("{ctl}/pan_right")).await;
            } else {
                b.control(&format!("{pre}.pan"), &g, &format!("{ctl}/pan")).await;
            }
            if out != "main" {
                b.control(&format!("{pre}.pre"), &g, &format!("{ctl}/pre_fader")).await;
            }
        }
    }

    // --- device & global settings
    b.prop("dev.online", "device", &dev, "online", Kind::ReadOnly, None).await;
    b.prop("dev.sample_rate", "device", &dev, "sample_rate", Kind::Choice { options: vec![] }, None).await;
    b.prop("dev.brightness", "device", &dev, "display_brightness", Kind::Continuous, Some((0.0, 255.0))).await;
    b.prop("dev.no_clip", "device", &dev, "no_clip", Kind::Toggle, None).await;
    b.prop("sys.peak_hold", "device", "/", "peak_hold", Kind::Choice { options: vec![] }, None).await;
    b.prop("sys.clip_hold", "device", "/", "clip_hold", Kind::Choice { options: vec![] }, None).await;
    b.prop("sys.post_fader_meters", "device", "/", "post_fader_meters", Kind::Toggle, None).await;
    b.action("act.identify", "device", "/devices/identify", json!({"device_uid": device.uid}));
    b.action("act.clear_solos", "mix", &format!("{dev}/clear_solos"), json!({}));

    // --- meters. Every bus meters on `outputs/main/meters/level` ("Input Metering" on
    // inputs). The bus-level `meters/level` ("ConsoleInput") never moves on the Volt.
    let mut meters: HashMap<String, Vec<String>> = HashMap::new();
    let mut meter_percent = HashSet::new();
    for bus in &buses {
        let suffix = match bus.bus_type.as_str() {
            "input" | "loopback" => "in",
            "master" | "headphone" | "cue_0" | "cue_1" => "out",
            _ => continue,
        };
        let path = format!("{}/outputs/main/meters/level", bus.path);
        if b.obj(&path).await.is_some_and(|o| pv(&o, "units").as_str() == Some("percent")) {
            meter_percent.insert(path.clone());
        }
        meters.entry(path).or_default().push(format!("{}.{suffix}", bus.slug));
    }
    // The headphone bus meter never pushes. The headphone output plays a mix
    // (by default `HP 1` -> `cue1` -> `CUE 1` output -> `mon`): show that mix's meter.
    if let Some(src) = headphone_source(&mut b, &dev).await
        && let Some(bus) = buses.iter().find(|x| x.bus_type == src)
    {
        log::debug!("headphone meter follows {}", bus.name);
        for ids in meters.values_mut() {
            ids.retain(|id| id != "hp.out");
        }
        let path = format!("{}/outputs/main/meters/level", bus.path);
        meters.entry(path).or_default().push("hp.out".into());
    }

    let params = b.params;
    let mut by_id = HashMap::new();
    let mut by_obj: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, p) in params.iter().enumerate() {
        by_id.insert(p.id.clone(), i);
        let obj = match &p.target {
            Target::Control { path, .. } | Target::Inverted { path } => Some(path.clone()),
            Target::Prop { obj, .. } | Target::Link { obj, .. } => Some(obj.clone()),
            Target::Post { .. } | Target::System => None,
        };
        if let Some(o) = obj {
            by_obj.entry(o).or_default().push(i);
        }
    }
    log::info!("discovered {} ({}) with {} params, {} meters", device.name, device.device_type, params.len(), meters.len());
    Ok(Model { device, buses, params, by_id, by_obj, meters, meter_percent })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toggle_ctl() -> Param {
        Param {
            id: "mon.mute".into(),
            label: "".into(),
            group: "".into(),
            kind: Kind::Toggle,
            normalized: 0.0,
            value: json!(0),
            display: "OFF".into(),
            target: Target::Control { path: "/d/m".into(), real_values: vec![] },
            volume: None,
        }
    }

    #[test]
    fn inverted_toggle_flips_mute() {
        let mut p = Param { kind: Kind::Toggle, target: Target::Inverted { path: "/b/outputs/main/controls/mute".into() }, ..toggle_ctl() };
        assert!(apply_to_param(&mut p, "value", &json!(1)));
        assert_eq!((p.normalized, p.display.as_str()), (0.0, "OFF")); // muted -> DIRECT off
        let mut m = Model { device: DeviceInfo { uid: "".into(), name: "".into(), device_type: "".into(), serial: "".into(), firmware: "".into() },
            buses: vec![], params: vec![p], by_id: [("mon.mute".to_string(), 0)].into(), by_obj: HashMap::new(), meters: HashMap::new(), meter_percent: HashSet::new() };
        let Op::Set { path, arg } = m.plan_write("mon.mute", 1.0).unwrap() else { panic!() };
        assert_eq!((path.as_str(), arg.as_str()), ("/b/outputs/main/controls/mute/value/value", "0")); // DIRECT on -> unmute
        assert_eq!(m.params[0].normalized, 1.0);
    }

    #[test]
    fn volume_curve_maps_position_to_db() {
        use super::volume::{position, value};
        assert_eq!(value(1.0, true), 100.0);
        assert_eq!(value(0.0, true), 0.0); // mute
        assert!((value(0.05, true) - 20.0).abs() < 1e-9);
        assert!((value(0.1, true) - 40.0).abs() < 1e-9); // knee
        assert!((value(0.55, true) - 70.0).abs() < 1e-9); // -30 dB
        assert!((value(0.0999, true) - value(0.1001, true)).abs() < 0.1); // continuous at the knee
        assert!((value(0.15, true) - value(0.14, true) - (value(0.95, true) - value(0.94, true))).abs() < 1e-9); // even above it
        for v in [0.0, 10.0, 20.0, 40.0, 49.0, 70.0, 100.0] {
            assert!((value(position(v, true), true) - v).abs() < 1e-9, "{v}");
            assert!((value(position(v, false), false) - v).abs() < 1e-9, "{v}");
        }
    }

    #[test]
    fn volume_param_writes_value_and_ignores_linear_pushes() {
        let p = Param { id: "hp.volume".into(), kind: Kind::Continuous, value: json!(100.0), volume: Some(true), ..toggle_ctl() };
        let mut m = Model { device: DeviceInfo { uid: String::new(), name: String::new(), device_type: String::new(), serial: String::new(), firmware: String::new() },
            buses: vec![], params: vec![p], by_id: [("hp.volume".to_string(), 0)].into(), by_obj: [("/d/m".to_string(), vec![0])].into(), meters: HashMap::new(), meter_percent: HashSet::new() };
        m.set_volume_curve(true);
        assert_eq!((m.params[0].normalized, m.params[0].display.as_str()), (1.0, "100"));
        let Op::Set { path, arg } = m.plan_write("hp.volume", 0.4).unwrap() else { panic!() };
        assert_eq!((path.as_str(), arg.as_str()), ("/d/m/value/value", "60"));
        assert_eq!((m.params[0].normalized, m.params[0].display.as_str()), (0.4, "60"));
        assert!(m.apply("/d/m/normalized_value", &json!(0.49)).is_empty());
        assert!(m.apply("/d/m/string_value", &json!("49.0")).is_empty());
        assert_eq!(m.apply("/d/m/value", &json!(46.0)), vec!["hp.volume".to_string()]);
        assert!((m.params[0].normalized - 0.19).abs() < 1e-9);
        assert_eq!(m.params[0].display, "46");
        let changed = m.set_volume_curve(false);
        assert_eq!(changed.len(), 1);
        assert!((m.params[0].normalized - 0.46).abs() < 1e-9);
    }

    #[test]
    fn meter_scale_converts_to_amplitude() {
        let db = |p: f64| 20.0 * meter_amplitude(p).log10();
        assert!((db(1.0) - 0.0).abs() < 1e-9);
        assert!((db(0.4545) - -18.0).abs() < 0.01);
        assert!((db(0.2425) - -30.0).abs() < 1e-9);
        assert!((db(0.0326 * 2.0) - -50.0).abs() < 0.1);
        assert_eq!(meter_amplitude(0.0), 0.0);
        assert!(meter_amplitude(1.3) <= 1.0);
    }

    #[test]
    fn fmt_num_is_json() {
        assert_eq!(fmt_num(0.5), "0.5");
        assert_eq!(fmt_num(1.0), "1");
        assert_eq!(fmt_num(0.0), "0");
    }

    #[test]
    fn toggle_value_push_updates_display() {
        let mut p = toggle_ctl();
        assert!(apply_to_param(&mut p, "value", &json!(1)));
        assert_eq!((p.normalized, p.display.as_str()), (1.0, "ON"));
    }

    #[test]
    fn choice_snaps_to_index() {
        assert_eq!(choice_index(0.0, 4), 0);
        assert_eq!(choice_index(0.4, 4), 1);
        assert_eq!(choice_index(1.0, 4), 3);
    }

    /// Read-only discovery against the real UAMixerHelper: `cargo test live_ -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn live_discover() {
        let (c, _rx) = Client::connect(crate::uasrv::DEFAULT_ADDR).await.expect("UAMixerHelper running");
        let m = discover(&c).await.expect("volt found");
        for p in &m.params {
            println!("{:<22} {:<14} {:<10} n={:.3} {:<12} {}", p.id, p.label, format!("{:?}", p.kind).chars().take(10).collect::<String>(), p.normalized, p.display, p.value);
        }
        println!("meters: {:?}", m.snapshot().meters);
        println!("subscriptions: {}", m.subscriptions().len());
        assert!(m.get("mon.volume").is_some() && m.get("hp.volume").is_some());
    }

    /// End-to-end write through the same path the UI uses, on harmless params, then restore.
    /// `cargo test live_ -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn live_write_roundtrip() {
        use crate::uasrv::Push;
        let (c, mut rx) = Client::connect(crate::uasrv::DEFAULT_ADDR).await.unwrap();
        let mut m = discover(&c).await.unwrap();
        for path in m.subscriptions() {
            let _ = c.subscribe(&path).await;
        }
        let send = |op: Op| match op {
            Op::Set { path, arg } => c.send_line("set", &path, Some(&arg)).unwrap(),
            Op::Post { .. } => panic!("unexpected post"),
        };
        // Drain pushes for a while, applying them to the model.
        async fn pump(m: &mut Model, rx: &mut tokio::sync::mpsc::UnboundedReceiver<Push>, ms: u64) {
            let end = tokio::time::Instant::now() + std::time::Duration::from_millis(ms);
            while let Ok(Some(p)) = tokio::time::timeout_at(end, rx.recv()).await {
                if let Push::Value { path, value } = p {
                    m.apply(&path, &value);
                }
            }
        }
        pump(&mut m, &mut rx, 300).await;

        for (id, target) in [("in.a2.tone", 1.0 / 3.0), ("dev.brightness", 40.0 / 255.0)] {
            let before = m.get(id).unwrap().normalized;
            send(m.plan_write(id, target).unwrap());
            pump(&mut m, &mut rx, 400).await;
            let fresh = c.get(&match &m.get(id).unwrap().target {
                Target::Control { path, .. } => path.clone(),
                Target::Prop { obj, .. } => obj.clone(),
                _ => unreachable!(),
            }).await.unwrap();
            let p = m.get(id).unwrap().clone();
            println!("{id}: {before:.3} -> model n={:.3} display={} | server {}", p.normalized, p.display,
                match id { "dev.brightness" => fresh["properties"]["display_brightness"]["value"].to_string(), _ => fresh["properties"]["string_value"]["value"].to_string() });
            assert!((p.normalized - target).abs() < 0.01, "{id} did not reach target");
            send(m.plan_write(id, before).unwrap());
            pump(&mut m, &mut rx, 400).await;
            assert!((m.get(id).unwrap().normalized - before).abs() < 0.01, "{id} not restored");
            println!("{id}: restored to {}", m.get(id).unwrap().display);
        }
    }

    #[test]
    fn minus_inf_display() {
        assert_eq!(clean_display("-\u{FFFD}\u{FFFD}", &json!(-144.0)), "-∞");
        assert_eq!(clean_display("+3.0", &json!(3.0)), "+3.0");
    }

    // ---- offline discovery from exported trees (`export`), one file per model

    /// Serves `get` from an exported tree, like UAMixerHelper would; everything else fails.
    async fn fake_uasrv(nodes: serde_json::Map<String, Value>) -> String {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        tokio::spawn(async move {
            let (mut s, _) = listener.accept().await.unwrap();
            let (mut buf, mut chunk) = (Vec::new(), [0u8; 4096]);
            loop {
                let n = match s.read(&mut chunk).await {
                    Ok(0) | Err(_) => return,
                    Ok(n) => n,
                };
                buf.extend_from_slice(&chunk[..n]);
                while let Some(end) = buf.iter().position(|&b| b == 0) {
                    let frame: Vec<u8> = buf.drain(..=end).collect();
                    let line = String::from_utf8_lossy(&frame[..end]).to_string();
                    let mut words = line.splitn(3, ' ');
                    let (verb, path) = (words.next().unwrap_or_default(), words.next().unwrap_or_default());
                    let reply = match nodes.get(path) {
                        Some(d) if verb == "get" && d.get("error").is_none() => json!({"path": path, "data": d}),
                        _ => json!({"path": path, "error": "not in fixture"}),
                    };
                    let mut out = serde_json::to_vec(&reply).unwrap();
                    out.push(0);
                    if s.write_all(&out).await.is_err() {
                        return;
                    }
                }
            }
        });
        addr
    }

    /// `tests/fixtures/<name>.json`, exported with 设置 → 导出设备信息.
    fn fixture(name: &str) -> serde_json::Map<String, Value> {
        let path = format!("{}/tests/fixtures/{name}.json", env!("CARGO_MANIFEST_DIR"));
        let tree: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(tree["format"], crate::export::FORMAT, "{path}");
        tree["nodes"].as_object().unwrap().clone()
    }

    async fn discover_fixture(name: &str) -> Model {
        let (c, _rx) = Client::connect(&fake_uasrv(fixture(name)).await).await.unwrap();
        discover(&c).await.unwrap_or_else(|e| panic!("{name}: {e}"))
    }

    /// Every exported model: the main view's building blocks exist.
    #[tokio::test]
    async fn fixtures_discover() {
        let dir = format!("{}/tests/fixtures", env!("CARGO_MANIFEST_DIR"));
        for entry in std::fs::read_dir(dir).unwrap() {
            let name = entry.unwrap().path().file_stem().unwrap().to_string_lossy().to_string();
            let m = discover_fixture(&name).await;
            let s = m.snapshot();
            assert!(m.get("mon.volume").is_some(), "{name}: no monitor volume");
            for b in s.buses.iter().filter(|b| b.bus_type == "input") {
                assert!(m.get(&format!("in.{}.gain", b.slug)).is_some(), "{name}: {} has no gain", b.name);
                assert!(s.meters.contains(&format!("{}.in", b.slug)), "{name}: {} has no meter", b.name);
                if let Some(link) = &b.link {
                    assert!(m.get(link).is_some(), "{name}: {link} missing");
                }
            }
            println!("{name}: {} buses, {} params", s.buses.len(), s.params.len());
        }
    }

    #[tokio::test]
    async fn fixture_volt_2_gen2() {
        let m = discover_fixture("volt_2_gen2").await;
        let s = m.snapshot();
        assert_eq!(s.device.device_type, "volt_2_gen2");
        let link = |slug: &str| s.buses.iter().find(|b| b.slug == slug).and_then(|b| b.link.clone());
        for slug in ["a1", "a2", "a12"] {
            assert_eq!(link(slug).as_deref(), Some("in.a12.link"), "{slug}");
        }
        assert_eq!(link("lb"), None); // loopback is not an input pair
        assert!(m.get("in.a12.link").is_some());
        assert!(m.get("in.a1.tone").is_some() && m.get("in.a1.comp").is_none()); // no onboard compressor
        assert!(m.get("hp.volume").is_some() && m.get("cue1.volume").is_some());
    }

    #[test]
    fn stereo_pair_members_by_name() {
        assert_eq!(pair_members("ANALOG 1-2"), Some(("ANALOG 1".into(), "ANALOG 2".into())));
        assert_eq!(pair_members("ANALOG 3-4"), Some(("ANALOG 3".into(), "ANALOG 4".into())));
        assert_eq!(pair_members("HI-Z"), None);
        assert_eq!(pair_members("LOOPBACK"), None);
    }
}
