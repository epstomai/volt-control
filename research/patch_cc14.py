"""One-off patch: 14-bit CC support in midi.rs / engine.rs. Run from src-tauri/src."""
import sys


def patch(path, pairs):
    s = open(path, encoding="utf-8").read()
    for old, new in pairs:
        if old not in s:
            sys.exit(f"{path}: anchor not found:\n{old[:120]}")
        s = s.replace(old, new, 1)
    open(path, "w", encoding="utf-8").write(s)


patch("midi.rs", [
("""pub enum MsgKind {
    Cc,
    Note,
    PitchBend,
}""", """pub enum MsgKind {
    Cc,
    /// High-resolution CC: MSB on CC n (0..31) + LSB on CC n+32, value 0..16383.
    Cc14,
    Note,
    PitchBend,
}"""),
("""    /// 0..127 (0..16383 for pitch bend). Note-off is a note with value 0.""",
 """    /// 0..127 (0..16383 for 14-bit CC and pitch bend). Note-off is a note with value 0."""),
("""    fn full_scale(&self) -> f64 {
        if self.kind == MsgKind::PitchBend { 16383.0 } else { 127.0 }
    }
    fn pressed(&self) -> bool {
        match self.kind {
            MsgKind::Note => self.value > 0,
            _ => self.value >= 64,
        }
    }""", """    fn high_res(&self) -> bool {
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
    /// A CC that may be the MSB half of a 14-bit pair.
    pub fn is_cc_msb(&self) -> bool {
        self.kind == MsgKind::Cc && self.number < 32
    }"""),
("""/// Per-mapping soft-takeover state.""", """/// Pairs CC n (MSB, n < 32) with CC n+32 (LSB) into [`MsgKind::Cc14`] messages,
/// per the MIDI spec's 14-bit controller convention (MSB first, then LSB).
#[derive(Default)]
pub struct Cc14Assembler {
    msb: std::collections::HashMap<(u8, u8), u16>,
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
                Some(MidiMsg { kind: MsgKind::Cc14, channel: msg.channel, number, value: (hi << 7) | (msg.value & 0x7F) })
            }
            _ => None,
        }
    }
}

/// Per-mapping soft-takeover state."""),
("""            let mut d = relative_delta(m.mode, msg.value);""",
 """            let mut d = relative_delta(m.mode, msg.value7());"""),
("""/// Bytes that show normalized `n` on the controller for mapping `m`.
pub fn feedback_bytes(m: &Mapping, n: f64) -> Vec<u8> {""",
 """/// MIDI messages that show normalized `n` on the controller for mapping `m`.
pub fn feedback_messages(m: &Mapping, n: f64) -> Vec<Vec<u8>> {"""),
("""    match m.kind {
        MsgKind::Cc => vec![0xB0 | ch, m.number & 0x7F, (x * 127.0).round() as u8],
        MsgKind::Note => vec![0x90 | ch, m.number & 0x7F, if x >= 0.5 { 127 } else { 0 }],
        MsgKind::PitchBend => {
            let v = (x * 16383.0).round() as u16;
            vec![0xE0 | ch, (v & 0x7F) as u8, (v >> 7) as u8]
        }
    }""", """    let v14 = (x * 16383.0).round() as u16;
    match m.kind {
        MsgKind::Cc => vec![vec![0xB0 | ch, m.number & 0x7F, (x * 127.0).round() as u8]],
        MsgKind::Cc14 => {
            let n = m.number & 0x1F;
            vec![vec![0xB0 | ch, n, (v14 >> 7) as u8], vec![0xB0 | ch, n + 32, (v14 & 0x7F) as u8]]
        }
        MsgKind::Note => vec![vec![0x90 | ch, m.number & 0x7F, if x >= 0.5 { 127 } else { 0 }]],
        MsgKind::PitchBend => vec![vec![0xE0 | ch, (v14 & 0x7F) as u8, (v14 >> 7) as u8]],
    }"""),
("""    #[test]
    fn feedback_scales_to_range() {
        let m = Mapping { min: 0.0, max: 0.5, ..Mapping::default() };
        assert_eq!(feedback_bytes(&m, 0.25), vec![0xB0, 0, 64]);
    }""", """    #[test]
    fn feedback_scales_to_range() {
        let m = Mapping { min: 0.0, max: 0.5, ..Mapping::default() };
        assert_eq!(feedback_messages(&m, 0.25), vec![vec![0xB0, 0, 64]]);
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
    }"""),
])

patch("engine.rs", [
("""                if m.feedback && !busy {
                    rt.io.send(&midi::feedback_bytes(m, p.normalized));
                }""", """                if m.feedback && !busy {
                    for msg in midi::feedback_messages(m, p.normalized) {
                        rt.io.send(&msg);
                    }
                }"""),
("""async fn midi_loop(shared: Arc<Shared>, mut rx: mpsc::UnboundedReceiver<MidiMsg>) {
    while let Some(msg) = rx.recv().await {
        let _ = shared.app.emit(EV_MIDI_ACTIVITY, msg);
        if let Err(e) = handle_midi(&shared, msg) {
            log::debug!("midi: {e}");
        }
    }
}

fn handle_midi(shared: &Arc<Shared>, msg: MidiMsg) -> Result<()> {
    // MIDI learn: the first press/move binds the waiting parameter.
    let learning = shared.midi.lock().unwrap().learn.clone();
    if let Some(param) = learning {
        if msg.kind == midi::MsgKind::Note && msg.value == 0 {
            return Ok(()); // ignore the release of the learning press
        }
""", """/// While learning, a CC 0..31 waits this long for its LSB (CC n+32) before it is
/// bound as a plain 7-bit CC.
const CC14_LEARN_WINDOW: Duration = Duration::from_millis(60);

async fn midi_loop(shared: Arc<Shared>, mut rx: mpsc::UnboundedReceiver<MidiMsg>) {
    let mut cc14 = midi::Cc14Assembler::default();
    // A learn candidate that might turn out to be the MSB of a 14-bit pair.
    let mut pending: Option<(MidiMsg, tokio::time::Instant)> = None;
    loop {
        let next = match pending {
            Some((msb, deadline)) => tokio::select! {
                m = rx.recv() => m,
                _ = tokio::time::sleep_until(deadline) => {
                    pending = None;
                    log_err(learn(&shared, msb)); // no LSB followed: plain 7-bit CC
                    continue;
                }
            },
            None => rx.recv().await,
        };
        let Some(msg) = next else { break };
        let combined = cc14.feed(&msg);
        let _ = shared.app.emit(EV_MIDI_ACTIVITY, combined.unwrap_or(msg));

        if shared.midi.lock().unwrap().learn.is_some() {
            match (combined, pending) {
                (Some(c), Some((p, _))) if p.channel == c.channel && p.number == c.number => {
                    pending = None;
                    log_err(learn(&shared, c));
                }
                _ if msg.is_cc_msb() => pending = Some((msg, tokio::time::Instant::now() + CC14_LEARN_WINDOW)),
                _ if pending.is_some() => {} // stray message inside the 14-bit detection window
                _ => log_err(learn(&shared, msg)),
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

/// MIDI learn: binds the waiting parameter to `msg`.
fn learn(shared: &Arc<Shared>, msg: MidiMsg) -> Result<()> {
    let Some(param) = shared.midi.lock().unwrap().learn.clone() else {
        return Ok(()); // learn was cancelled meanwhile
    };
    if msg.kind == midi::MsgKind::Note && msg.value == 0 {
        return Ok(()); // ignore the release of the learning press
    }
    {
"""),
("""        let _ = shared.app.emit(EV_MIDI_MAPPINGS, mappings);
        return Ok(());
    }

    let mappings: Vec<_>""", """        let _ = shared.app.emit(EV_MIDI_MAPPINGS, mappings);
    }
    Ok(())
}

/// Applies `msg` to every mapping it matches.
fn dispatch(shared: &Arc<Shared>, msg: MidiMsg) -> Result<()> {
    let mappings: Vec<_>"""),
])
print("patched")
