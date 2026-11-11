//! MIDI controller support: port I/O, message parsing, mapping resolution
//! (absolute / relative encoders / buttons) with soft takeover, and feedback.

use std::time::{Duration, Instant};

use midir::{MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;

use crate::config::MidiConfig;
use crate::i18n;
use crate::model::Kind;

const CLIENT_NAME: &str = "Volt Control";
/// While a control is being moved by hand, don't echo its own value back (motor-fader fights).
pub const FEEDBACK_HOLDOFF: Duration = Duration::from_millis(300);
/// Soft takeover: an absolute control grabs the parameter once it is this close (normalized).
const PICKUP_WINDOW: f64 = 0.03;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MsgKind {
    Cc,
    /// High-resolution CC: MSB on CC n (0..31) + LSB on CC n+32, value 0..16383.
    Cc14,
    Note,
    PitchBend,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Knob/fader position maps directly onto [min, max].
    Absolute,
    /// Relative encoder, two's complement (1..63 up, 127..65 down).
    RelativeTwos,
    /// Relative encoder, binary offset (65.. up, ..63 down).
    RelativeOffset,
    /// Relative encoder, sign bit (1..63 up, 65..127 down).
    RelativeSigned,
    /// Each press flips the value (cycles through choices).
    Toggle,
    /// Held = max, released = min.
    Momentary,
    /// Press fires an action.
    Trigger,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Mapping {
    pub id: u64,
    pub param: String,
    pub kind: MsgKind,
    pub channel: u8,
    pub number: u8,
    pub mode: Mode,
    /// Normalized range the controller covers.
    pub min: f64,
    pub max: f64,
    pub invert: bool,
    /// Soft takeover for absolute controls (prevents volume jumps).
    pub pickup: bool,
    pub feedback: bool,
    /// Normalized change per encoder tick (relative modes).
    pub step: f64,
}

impl Default for Mapping {
    fn default() -> Self {
        Mapping {
            id: 0,
            param: String::new(),
            kind: MsgKind::Cc,
            channel: 0,
            number: 0,
            mode: Mode::Absolute,
            min: 0.0,
            max: 1.0,
            invert: false,
            pickup: false,
            feedback: true,
            step: 0.01,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct MidiMsg {
    pub kind: MsgKind,
    pub channel: u8,
    pub number: u8,
    /// 0..127 (0..16383 for 14-bit CC and pitch bend). Note-off is a note with value 0.
    pub value: u16,
}

impl MidiMsg {
    fn high_res(&self) -> bool {
        matches!(self.kind, MsgKind::PitchBend | MsgKind::Cc14)
    }
    fn full_scale(&self) -> f64 {
        if self.high_res() { 16383.0 } else { 127.0 }
    }
    /// The coarse 7-bit value (MSB for high-resolution messages).
    fn value7(&self) -> u16 {
        if self.high_res() { self.value >> 7 } else { self.value }
    }
    fn pressed(&self) -> bool {
        match self.kind {
            MsgKind::Note => self.value > 0,
            _ => self.value7() >= 64,
        }
    }
    /// Whether a new mapping for this message must replace `m`: same control, or
    /// for a 14-bit CC either of its two 7-bit halves (CC n and CC n+32).
    pub fn conflicts_with(&self, m: &Mapping) -> bool {
        if self.matches(m) {
            return true;
        }
        let pair = |cc14: u8, cc: u8| cc == cc14 || cc == cc14 + 32;
        self.channel == m.channel
            && match (self.kind, m.kind) {
                (MsgKind::Cc14, MsgKind::Cc) => pair(self.number, m.number),
                (MsgKind::Cc, MsgKind::Cc14) => pair(m.number, self.number),
                _ => false,
            }
    }

    /// A CC that may be the MSB half of a 14-bit pair.
    pub fn is_cc_msb(&self) -> bool {
        self.kind == MsgKind::Cc && self.number < 32
    }
    pub fn matches(&self, m: &Mapping) -> bool {
        self.kind == m.kind && self.channel == m.channel && (self.kind == MsgKind::PitchBend || self.number == m.number)
    }
}

pub fn parse(b: &[u8]) -> Option<MidiMsg> {
    let (&status, rest) = b.split_first()?;
    let channel = status & 0x0F;
    let d1 = *rest.first()? as u16;
    let d2 = rest.get(1).copied().unwrap_or(0) as u16;
    let (kind, number, value) = match status & 0xF0 {
        0xB0 => (MsgKind::Cc, d1 as u8, d2),
        0x90 => (MsgKind::Note, d1 as u8, d2),
        0x80 => (MsgKind::Note, d1 as u8, 0),
        0xE0 => (MsgKind::PitchBend, 0, d1 | (d2 << 7)),
        _ => return None,
    };
    Some(MidiMsg { kind, channel, number, value })
}

/// Pairs CC n (MSB, n < 32) with CC n+32 (LSB) into [`MsgKind::Cc14`] messages,
/// per the MIDI spec's 14-bit controller convention (MSB first, then LSB).
#[derive(Default)]
pub struct Cc14Assembler {
    msb: std::collections::HashMap<(u8, u8), u16>,
    /// Controls seen sending an LSB: their MSB alone is half a message.
    paired: std::collections::HashSet<(u8, u8)>,
}

impl Cc14Assembler {
    /// Feeds a raw message; returns the 14-bit message it completes, if any.
    pub fn feed(&mut self, msg: &MidiMsg) -> Option<MidiMsg> {
        if msg.kind != MsgKind::Cc {
            return None;
        }
        match msg.number {
            0..=31 => {
                self.msb.insert((msg.channel, msg.number), msg.value);
                None
            }
            32..=63 => {
                let number = msg.number - 32;
                let hi = *self.msb.get(&(msg.channel, number))?;
                self.paired.insert((msg.channel, number));
                Some(MidiMsg { kind: MsgKind::Cc14, channel: msg.channel, number, value: (hi << 7) | (msg.value & 0x7F) })
            }
            _ => None,
        }
    }
}

impl Cc14Assembler {
    /// `msg` is the MSB of a known 14-bit control: the LSB that follows completes it.
    pub fn is_half(&self, msg: &MidiMsg) -> bool {
        msg.kind == MsgKind::Cc && msg.number < 32 && self.paired.contains(&(msg.channel, msg.number))
    }
}

/// Largest change of a high-resolution value (14-bit CC, pitch bend) that
/// counts as noise: ~0.1% of the travel. A resting Fader4 fader flips between
/// two values 4 apart every few seconds (measured 2026-11-03).
pub const JITTER: u16 = 16;

/// Dead band for high-resolution controls: a value passes once it is more than
/// [`JITTER`] from the last value that passed. The ends of the range always
/// pass, so a fader still reaches exactly 0 and full scale. 7-bit messages and
/// notes always pass (one step there is a real move).
#[derive(Default)]
pub struct JitterFilter {
    last: std::collections::HashMap<(MsgKind, u8, u8), u16>,
}

impl JitterFilter {
    pub fn pass(&mut self, msg: &MidiMsg) -> bool {
        if !msg.high_res() {
            return true;
        }
        let key = (msg.kind, msg.channel, msg.number);
        let end = msg.value == 0 || msg.value == 0x3FFF;
        match self.last.get(&key) {
            Some(&v) if !end && v.abs_diff(msg.value) <= JITTER => false,
            Some(&v) if end && v == msg.value => false,
            _ => {
                self.last.insert(key, msg.value);
                true
            }
        }
    }
}

/// Per-mapping soft-takeover state.
#[derive(Default, Clone, Debug)]
pub struct Pickup {
    pub engaged: bool,
    last: Option<f64>,
}

impl Pickup {
    pub fn release(&mut self) {
        self.engaged = false;
    }
}

#[derive(Debug, PartialEq)]
pub enum Outcome {
    Set(f64),
    Trigger,
    Ignore,
}

fn relative_delta(mode: Mode, v: u16) -> i32 {
    let v = v as i32;
    match mode {
        Mode::RelativeTwos => if v < 64 { v } else { v - 128 },
        Mode::RelativeOffset => v - 64,
        Mode::RelativeSigned => if v & 0x40 != 0 { -(v & 0x3F) } else { v & 0x3F },
        _ => 0,
    }
}

fn steps_of(kind: &Kind) -> Option<usize> {
    match kind {
        Kind::Choice { options } => Some(options.len()),
        Kind::Toggle => Some(2),
        _ => None,
    }
}

/// Snaps a normalized value to the parameter's discrete grid, if any.
fn snap(kind: &Kind, n: f64) -> f64 {
    match steps_of(kind) {
        Some(len) if len > 1 => (n * (len - 1) as f64).round() / (len - 1) as f64,
        _ => n,
    }
}

/// Decides what an incoming message does to a parameter currently at `current`.
pub fn resolve(m: &Mapping, msg: &MidiMsg, current: f64, kind: &Kind, pickup: &mut Pickup) -> Outcome {
    let (lo, hi) = (m.min.min(m.max), m.min.max(m.max));
    match m.mode {
        Mode::Absolute => {
            let mut x = msg.value as f64 / msg.full_scale();
            if m.invert {
                x = 1.0 - x;
            }
            let target = snap(kind, lo + x * (hi - lo));
            if m.pickup && !pickup.engaged {
                let close = (target - current).abs() <= PICKUP_WINDOW;
                let crossed = pickup.last.is_some_and(|prev| (prev - current) * (target - current) <= 0.0);
                pickup.last = Some(target);
                if !(close || crossed) {
                    return Outcome::Ignore;
                }
                pickup.engaged = true;
            }
            pickup.last = Some(target);
            Outcome::Set(target)
        }
        Mode::RelativeTwos | Mode::RelativeOffset | Mode::RelativeSigned => {
            let mut d = relative_delta(m.mode, msg.value7());
            if m.invert {
                d = -d;
            }
            if d == 0 {
                return Outcome::Ignore;
            }
            let step = match steps_of(kind) {
                Some(len) if len > 1 => 1.0 / (len - 1) as f64,
                _ => m.step,
            };
            Outcome::Set(snap(kind, (current + d as f64 * step).clamp(lo, hi)))
        }
        Mode::Toggle => {
            if !msg.pressed() {
                return Outcome::Ignore;
            }
            match steps_of(kind) {
                // Choices cycle: OFF -> A -> B -> C -> OFF
                Some(len) if len > 2 => {
                    let next = (current * (len - 1) as f64).round() as usize + 1;
                    Outcome::Set(if next >= len { 0.0 } else { next as f64 / (len - 1) as f64 })
                }
                _ => Outcome::Set(if current >= (lo + hi) / 2.0 { lo } else { hi }),
            }
        }
        Mode::Momentary => Outcome::Set(if msg.pressed() { hi } else { lo }),
        Mode::Trigger => if msg.pressed() { Outcome::Trigger } else { Outcome::Ignore },
    }
}

/// MIDI messages that show normalized `n` on the controller for mapping `m`.
pub fn feedback_messages(m: &Mapping, n: f64) -> Vec<Vec<u8>> {
    let (lo, hi) = (m.min.min(m.max), m.min.max(m.max));
    let mut x = if hi > lo { ((n - lo) / (hi - lo)).clamp(0.0, 1.0) } else { 0.0 };
    if m.invert {
        x = 1.0 - x;
    }
    let ch = m.channel & 0x0F;
    let v14 = (x * 16383.0).round() as u16;
    match m.kind {
        MsgKind::Cc => vec![vec![0xB0 | ch, m.number & 0x7F, (x * 127.0).round() as u8]],
        MsgKind::Cc14 => {
            let n = m.number & 0x1F;
            vec![vec![0xB0 | ch, n, (v14 >> 7) as u8], vec![0xB0 | ch, n + 32, (v14 & 0x7F) as u8]]
        }
        MsgKind::Note => vec![vec![0x90 | ch, m.number & 0x7F, if x >= 0.5 { 127 } else { 0 }]],
        MsgKind::PitchBend => vec![vec![0xE0 | ch, (v14 & 0x7F) as u8, (v14 >> 7) as u8]],
    }
}

/// Mode a freshly learned mapping starts with.
pub fn learned_mapping(id: u64, param: &str, kind: &Kind, msg: &MidiMsg) -> Mapping {
    let mode = match (msg.kind, kind) {
        (_, Kind::Action) => Mode::Trigger,
        (MsgKind::Note, _) => Mode::Toggle,
        _ => Mode::Absolute,
    };
    let is_level = matches!(kind, Kind::Continuous)
        && (param.ends_with(".volume") || param.ends_with(".gain"));
    Mapping {
        id,
        param: param.to_string(),
        kind: msg.kind,
        channel: msg.channel,
        number: msg.number,
        mode,
        pickup: mode == Mode::Absolute && is_level,
        ..Mapping::default()
    }
}

// ------------------------------------------------------------------ port I/O

#[derive(Serialize)]
pub struct Ports {
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
}

pub fn list_ports() -> Ports {
    let inputs = MidiInput::new(CLIENT_NAME)
        .map(|mi| mi.ports().iter().filter_map(|p| mi.port_name(p).ok()).collect())
        .unwrap_or_default();
    let outputs = MidiOutput::new(CLIENT_NAME)
        .map(|mo| mo.ports().iter().filter_map(|p| mo.port_name(p).ok()).collect())
        .unwrap_or_default();
    Ports { inputs, outputs }
}

#[derive(Default)]
pub struct MidiIo {
    inputs: Vec<MidiInputConnection<()>>,
    output: Option<MidiOutputConnection>,
    pub open_inputs: Vec<String>,
    pub open_output: Option<String>,
}

impl MidiIo {
    /// (Re)opens the configured ports; returns human-readable errors.
    pub fn open(&mut self, cfg: &MidiConfig, tx: UnboundedSender<MidiMsg>) -> Vec<String> {
        self.close();
        let mut errors = Vec::new();
        for name in &cfg.inputs {
            let result = (|| -> anyhow::Result<MidiInputConnection<()>> {
                let mut mi = MidiInput::new(CLIENT_NAME)?;
                mi.ignore(midir::Ignore::All);
                let port = mi
                    .ports()
                    .into_iter()
                    .find(|p| mi.port_name(p).ok().as_deref() == Some(name))
                    .ok_or_else(|| anyhow::anyhow!(i18n::t("err.no_port").to_string()))?;
                let tx = tx.clone();
                let conn = mi
                    .connect(&port, "volt-control-in", move |_, bytes, _| {
                        if let Some(msg) = parse(bytes) {
                            let _ = tx.send(msg);
                        }
                    }, ())
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
                Ok(conn)
            })();
            match result {
                Ok(c) => {
                    self.inputs.push(c);
                    self.open_inputs.push(name.clone());
                }
                Err(e) => errors.push(i18n::tf("midi.input_failed", &[name, &e])),
            }
        }
        if let Some(name) = &cfg.output {
            let result = (|| -> anyhow::Result<MidiOutputConnection> {
                let mo = MidiOutput::new(CLIENT_NAME)?;
                let port = mo
                    .ports()
                    .into_iter()
                    .find(|p| mo.port_name(p).ok().as_deref() == Some(name))
                    .ok_or_else(|| anyhow::anyhow!(i18n::t("err.no_port").to_string()))?;
                mo.connect(&port, "volt-control-out").map_err(|e| anyhow::anyhow!("{e}"))
            })();
            match result {
                Ok(c) => {
                    self.output = Some(c);
                    self.open_output = Some(name.clone());
                }
                Err(e) => errors.push(i18n::tf("midi.output_failed", &[name, &e])),
            }
        }
        errors
    }

    pub fn close(&mut self) {
        for c in self.inputs.drain(..) {
            c.close();
        }
        if let Some(c) = self.output.take() {
            c.close();
        }
        self.open_inputs.clear();
        self.open_output = None;
    }

    pub fn send(&mut self, bytes: &[u8]) {
        if let Some(out) = self.output.as_mut()
            && let Err(e) = out.send(bytes)
        {
            log::warn!("MIDI feedback send failed: {e}");
        }
    }
}

/// Runtime MIDI state shared by the handler and the feedback path.
#[derive(Default)]
pub struct MidiRuntime {
    pub io: MidiIo,
    pub learn: Option<String>,
    pub pickup: std::collections::HashMap<u64, Pickup>,
    pub last_input: std::collections::HashMap<u64, Instant>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jitter_filter_drops_resting_flicker() {
        let cc14 = |value| MidiMsg { kind: MsgKind::Cc14, channel: 0, number: 16, value };
        let mut f = JitterFilter::default();
        assert!(f.pass(&cc14(11337))); // first value of a control
        assert!(!f.pass(&cc14(11341)) && !f.pass(&cc14(11337))); // the measured flicker
        assert!(f.pass(&cc14(11360))); // a real move
        assert!(f.pass(&cc14(16380)) && f.pass(&cc14(0x3FFF))); // the end of travel always lands
        assert!(!f.pass(&cc14(0x3FFF)));
        let cc = |value| MidiMsg { kind: MsgKind::Cc, channel: 0, number: 7, value };
        assert!(f.pass(&cc(64)) && f.pass(&cc(65)) && f.pass(&cc(64))); // 7-bit: every step counts
    }

    #[test]
    fn cc14_msb_is_half_once_paired() {
        let cc = |number, value| MidiMsg { kind: MsgKind::Cc, channel: 0, number, value };
        let mut a = Cc14Assembler::default();
        assert!(!a.is_half(&cc(16, 88))); // could still be a plain 7-bit CC
        a.feed(&cc(16, 88));
        a.feed(&cc(48, 73));
        assert!(a.is_half(&cc(16, 88)));
    }

    fn cc(v: u16) -> MidiMsg {
        MidiMsg { kind: MsgKind::Cc, channel: 0, number: 7, value: v }
    }

    #[test]
    #[ignore]
    fn live_list_ports() {
        let p = list_ports();
        println!("inputs:  {:?}
outputs: {:?}", p.inputs, p.outputs);
    }

    #[test]
    fn parses_messages() {
        assert_eq!(parse(&[0xB1, 7, 100]).map(|m| (m.kind, m.channel, m.number, m.value)), Some((MsgKind::Cc, 1, 7, 100)));
        assert_eq!(parse(&[0x80, 60, 64]).map(|m| m.value), Some(0));
        assert_eq!(parse(&[0xE0, 0x7F, 0x7F]).map(|m| m.value), Some(16383));
        assert!(parse(&[0xF8]).is_none());
    }

    #[test]
    fn pickup_blocks_jumps_until_crossing() {
        let m = Mapping { pickup: true, ..Mapping::default() };
        let mut st = Pickup::default();
        // Parameter sits at 0.5; knob starts at 0 -> ignored.
        assert_eq!(resolve(&m, &cc(0), 0.5, &Kind::Continuous, &mut st), Outcome::Ignore);
        assert_eq!(resolve(&m, &cc(40), 0.5, &Kind::Continuous, &mut st), Outcome::Ignore);
        // Knob passes 0.5 -> engaged.
        assert!(matches!(resolve(&m, &cc(70), 0.5, &Kind::Continuous, &mut st), Outcome::Set(_)));
        assert!(st.engaged);
    }

    #[test]
    fn relative_encoders() {
        let m = Mapping { mode: Mode::RelativeTwos, step: 0.1, ..Mapping::default() };
        let mut st = Pickup::default();
        assert_eq!(resolve(&m, &cc(1), 0.5, &Kind::Continuous, &mut st), Outcome::Set(0.6));
        let Outcome::Set(v) = resolve(&m, &cc(127), 0.5, &Kind::Continuous, &mut st) else { panic!() };
        assert!((v - 0.4).abs() < 1e-9);
        assert_eq!(relative_delta(Mode::RelativeOffset, 63), -1);
        assert_eq!(relative_delta(Mode::RelativeSigned, 65), -1);
    }

    #[test]
    fn toggle_cycles_choices() {
        let m = Mapping { mode: Mode::Toggle, kind: MsgKind::Note, ..Mapping::default() };
        let kind = Kind::Choice { options: vec!["OFF".into(), "A".into(), "B".into(), "C".into()] };
        let press = MidiMsg { kind: MsgKind::Note, channel: 0, number: 0, value: 127 };
        let mut st = Pickup::default();
        let Outcome::Set(v) = resolve(&m, &press, 0.0, &kind, &mut st) else { panic!() };
        assert!((v - 1.0 / 3.0).abs() < 1e-9);
        assert_eq!(resolve(&m, &press, 1.0, &kind, &mut st), Outcome::Set(0.0));
    }

    #[test]
    fn feedback_scales_to_range() {
        let m = Mapping { min: 0.0, max: 0.5, ..Mapping::default() };
        assert_eq!(feedback_messages(&m, 0.25), vec![vec![0xB0, 0, 64]]);
    }

    #[test]
    fn cc14_learn_replaces_its_7bit_halves() {
        let cc14 = MidiMsg { kind: MsgKind::Cc14, channel: 0, number: 7, value: 0 };
        let map = |kind, number| Mapping { kind, number, ..Mapping::default() };
        assert!(cc14.conflicts_with(&map(MsgKind::Cc, 7)));
        assert!(cc14.conflicts_with(&map(MsgKind::Cc, 39)));
        assert!(!cc14.conflicts_with(&map(MsgKind::Cc, 8)));
        let lsb = MidiMsg { kind: MsgKind::Cc, channel: 0, number: 39, value: 0 };
        assert!(lsb.conflicts_with(&map(MsgKind::Cc14, 7)));
    }

    #[test]
    fn assembles_14bit_cc() {
        let mut a = Cc14Assembler::default();
        let raw = |n, v| MidiMsg { kind: MsgKind::Cc, channel: 2, number: n, value: v };
        assert!(a.feed(&raw(7, 0x40)).is_none()); // MSB alone: wait for LSB
        let m = a.feed(&raw(39, 0x05)).unwrap(); // LSB of CC 7
        assert_eq!((m.kind, m.channel, m.number, m.value), (MsgKind::Cc14, 2, 7, (0x40 << 7) | 0x05));
        assert!(a.feed(&raw(40, 1)).is_none()); // LSB without a known MSB
        assert!(a.feed(&raw(70, 1)).is_none());
    }

    #[test]
    fn cc14_absolute_uses_full_resolution_and_feedback() {
        let m = Mapping { kind: MsgKind::Cc14, number: 7, ..Mapping::default() };
        let msg = MidiMsg { kind: MsgKind::Cc14, channel: 0, number: 7, value: 8192 };
        let Outcome::Set(v) = resolve(&m, &msg, 0.0, &Kind::Continuous, &mut Pickup::default()) else { panic!() };
        assert!((v - 8192.0 / 16383.0).abs() < 1e-9);
        assert_eq!(feedback_messages(&m, 1.0), vec![vec![0xB0, 7, 127], vec![0xB0, 39, 127]]);
        assert_eq!(feedback_messages(&m, 0.0), vec![vec![0xB0, 7, 0], vec![0xB0, 39, 0]]);
    }
}
