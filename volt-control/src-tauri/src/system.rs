//! Windows system volume: the default playback device's master volume and mute
//! (the same values as the taskbar volume flyout), exposed as `win.volume` /
//! `win.mute`. A worker thread owns the COM objects, applies writes and polls,
//! so changes made elsewhere (volume keys, the flyout, a new default device)
//! show up too. Only outside changes are reported, never our own writes.

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use anyhow::Result;
use serde_json::json;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;

use crate::i18n;
use crate::model::{Kind, Param, Target};
use windows::Win32::Media::Audio::{IMMDeviceEnumerator, MMDeviceEnumerator, eConsole, eRender};
use windows::Win32::System::Com::{CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree};

pub const VOLUME: &str = "win.volume";
pub const MUTE: &str = "win.mute";

/// Outside changes show up within this long.
const POLL: Duration = Duration::from_millis(100);
/// How often the default playback device is checked for a switch.
const REBIND: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct State {
    /// Scalar volume 0..1, as shown by Windows (x100).
    pub volume: f32,
    pub mute: bool,
}

/// The two parameters for a volume state.
pub fn params(s: State) -> Vec<Param> {
    let pct = (s.volume as f64 * 100.0).round();
    vec![
        Param {
            id: VOLUME.into(),
            label: i18n::param_label(VOLUME),
            group: "system".into(),
            kind: Kind::Continuous,
            normalized: s.volume as f64,
            value: json!(pct),
            display: format!("{pct}"),
            target: Target::System,
            volume: None,
        },
        Param {
            id: MUTE.into(),
            label: i18n::param_label(MUTE),
            group: "system".into(),
            kind: Kind::Toggle,
            normalized: s.mute as u8 as f64,
            value: json!(s.mute),
            display: if s.mute { "ON" } else { "OFF" }.into(),
            target: Target::System,
            volume: None,
        },
    ]
}

pub fn kinds() -> [(String, Kind); 2] {
    [(VOLUME.into(), Kind::Continuous), (MUTE.into(), Kind::Toggle)]
}

pub enum Cmd {
    Volume(f32),
    Mute(bool),
}

/// Starts the worker; `on_change` runs on the worker thread.
pub fn spawn(on_change: impl Fn(State) + Send + 'static) -> Sender<Cmd> {
    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("system-volume".into())
        .spawn(move || {
            if let Err(e) = run(&rx, &on_change) {
                log::warn!("system volume unavailable: {e}");
            }
        })
        .expect("spawn system volume thread");
    tx
}

struct Endpoint {
    id: String,
    volume: IAudioEndpointVolume,
}

fn run(rx: &Receiver<Cmd>, on_change: &dyn Fn(State)) -> Result<()> {
    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok()? };
    let devices: IMMDeviceEnumerator = unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)? };
    let mut endpoint: Option<Endpoint> = None;
    let mut checked = Instant::now() - REBIND;
    let mut last: Option<State> = None;
    loop {
        if checked.elapsed() >= REBIND {
            checked = Instant::now();
            match default_endpoint(&devices, endpoint.as_ref().map(|e| e.id.as_str())) {
                Ok(Some(e)) => {
                    log::info!("system volume follows {}", e.id);
                    endpoint = Some(e);
                    last = None; // report the new device's state
                }
                Ok(None) => {}
                Err(e) => {
                    log::debug!("no default playback device: {e}");
                    endpoint = None;
                }
            }
        }
        // Wait for a write or the next poll; apply every queued write.
        let mut cmds = Vec::new();
        match rx.recv_timeout(POLL) {
            Ok(c) => cmds.push(c),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return Ok(()),
        }
        cmds.extend(rx.try_iter());
        let Some(ep) = &endpoint else { continue };
        let wrote = !cmds.is_empty();
        for c in cmds {
            let r = unsafe {
                match c {
                    Cmd::Volume(v) => ep.volume.SetMasterVolumeLevelScalar(v.clamp(0.0, 1.0), std::ptr::null()),
                    Cmd::Mute(m) => ep.volume.SetMute(m, std::ptr::null()),
                }
            };
            if let Err(e) = r {
                log::warn!("set system volume: {e}");
            }
        }
        let now = match read(&ep.volume) {
            Ok(s) => s,
            Err(e) => {
                log::debug!("read system volume: {e}");
                endpoint = None;
                checked = Instant::now() - REBIND;
                continue;
            }
        };
        // A write's own result is not news: the caller already shows it.
        if !wrote && last != Some(now) {
            on_change(now);
        }
        last = Some(now);
    }
}

/// The default playback device's volume control, if it differs from `current`.
fn default_endpoint(devices: &IMMDeviceEnumerator, current: Option<&str>) -> Result<Option<Endpoint>> {
    unsafe {
        let device = devices.GetDefaultAudioEndpoint(eRender, eConsole)?;
        let raw = device.GetId()?;
        let id = raw.to_string()?;
        CoTaskMemFree(Some(raw.0 as *const _));
        if current == Some(id.as_str()) {
            return Ok(None);
        }
        let volume: IAudioEndpointVolume = device.Activate(CLSCTX_ALL, None)?;
        Ok(Some(Endpoint { id, volume }))
    }
}

fn read(v: &IAudioEndpointVolume) -> Result<State> {
    unsafe { Ok(State { volume: v.GetMasterVolumeLevelScalar()?, mute: v.GetMute()?.as_bool() }) }
}
