"""One-off patch: flatten learn(), replace conflicting mappings when learning 14-bit CC."""
import sys


def patch(path, pairs):
    s = open(path, encoding="utf-8").read()
    for old, new in pairs:
        if old not in s:
            sys.exit(f"{path}: anchor not found:\n{old[:120]}")
        s = s.replace(old, new, 1)
    open(path, "w", encoding="utf-8").write(s)


patch("engine.rs", [
("""    {
        let kind = shared.model.lock().unwrap().as_ref().and_then(|m| m.get(&param)).map(|p| p.kind.clone());
        let kind = kind.ok_or_else(|| anyhow!("unknown param {param}"))?;
        {
            let mut cfg = shared.config.lock().unwrap();
            let id = cfg.midi.mappings.iter().map(|m| m.id).max().unwrap_or(0) + 1;
            cfg.midi.mappings.retain(|m| !msg.matches(m));
            cfg.midi.mappings.push(midi::learned_mapping(id, &param, &kind, &msg));
        }
        shared.midi.lock().unwrap().learn = None;
        shared.save_config();
        let mappings = shared.config.lock().unwrap().midi.mappings.clone();
        let _ = shared.app.emit(EV_MIDI_MAPPINGS, mappings);
    }
    Ok(())
}""", """    let kind = shared.model.lock().unwrap().as_ref().and_then(|m| m.get(&param)).map(|p| p.kind.clone());
    let kind = kind.ok_or_else(|| anyhow!("unknown param {param}"))?;
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
}"""),
])

patch("midi.rs", [
("""    /// A CC that may be the MSB half of a 14-bit pair.""", """    /// Whether a new mapping for this message must replace `m`: same control, or
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

    /// A CC that may be the MSB half of a 14-bit pair."""),
("""    #[test]
    fn assembles_14bit_cc() {""", """    #[test]
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
    fn assembles_14bit_cc() {"""),
])
print("patched")
