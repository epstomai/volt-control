//! Persistent user settings (`<app config dir>/config.json`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::i18n::Lang;
use crate::midi::Mapping;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MidiConfig {
    /// Input port names to listen on.
    pub inputs: Vec<String>,
    /// Output port name for feedback (LED rings / motor faders).
    pub output: Option<String>,
    pub mappings: Vec<Mapping>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub midi: MidiConfig,
    /// Safety ceiling per parameter id, in normalized units (applies to UI and MIDI writes).
    pub limits: HashMap<String, f64>,
    /// Closing the window hides it to the tray instead of quitting.
    pub close_to_tray: bool,
    /// Start with Windows, hidden in the tray.
    pub autostart: bool,
    /// Monitor / headphone volume follow a perceptual curve instead of the
    /// device's 1 dB-per-step scale (see `model::volume`).
    pub volume_curve: bool,
    /// Window size per tab (inner, logical px) once the user resized it.
    pub window_sizes: HashMap<String, [f64; 2]>,
    /// Idle minutes before the Volt's screen goes dark; 0 = never (see `screen`).
    pub screen_sleep_min: u32,
    /// Normalized brightness to restore while the screen is dark from idling.
    pub screen_dimmed_from: Option<f64>,
    /// UI language; None follows the Windows display language.
    pub language: Option<Lang>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            midi: MidiConfig::default(),
            limits: HashMap::new(),
            close_to_tray: true,
            autostart: true,
            volume_curve: true,
            window_sizes: HashMap::new(),
            screen_sleep_min: 15,
            screen_dimmed_from: None,
            language: None,
        }
    }
}

/// Parameter ids renamed since earlier versions: (old, new).
const RENAMED: [(&str, &str); 1] = [
    // One link per stereo input pair; the Volt 2's only pair was "in.link".
    ("in.link", "in.a12.link"),
];

impl Config {
    /// Moves mappings and limits of renamed parameters to their new ids.
    fn migrate(&mut self) {
        for (old, new) in RENAMED {
            for m in self.midi.mappings.iter_mut().filter(|m| m.param == old) {
                m.param = new.to_string();
            }
            if let Some(v) = self.limits.remove(old) {
                self.limits.insert(new.to_string(), v);
            }
        }
    }
}

pub struct Store {
    path: PathBuf,
}

impl Store {
    pub fn new(dir: &Path) -> Store {
        Store { path: dir.join("config.json") }
    }

    /// Missing file -> defaults. A corrupt file is kept aside and reported, never silently lost.
    pub fn load(&self) -> Config {
        match std::fs::read_to_string(&self.path) {
            Ok(text) => match serde_json::from_str::<Config>(&text) {
                Ok(mut cfg) => {
                    cfg.migrate();
                    cfg
                }
                Err(e) => {
                    let backup = self.path.with_extension("json.bad");
                    log::error!("config {} is invalid ({e}); moved to {}", self.path.display(), backup.display());
                    let _ = std::fs::rename(&self.path, backup);
                    Config::default()
                }
            },
            Err(_) => Config::default(),
        }
    }

    pub fn save(&self, cfg: &Config) -> Result<()> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(cfg)?)?;
        std::fs::rename(tmp, &self.path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrate_moves_renamed_ids() {
        let mut cfg = Config::default();
        cfg.midi.mappings.push(Mapping { param: "in.link".into(), ..Mapping::default() });
        cfg.midi.mappings.push(Mapping { param: "mon.volume".into(), ..Mapping::default() });
        cfg.limits.insert("in.link".into(), 0.5);
        cfg.migrate();
        let params: Vec<&str> = cfg.midi.mappings.iter().map(|m| m.param.as_str()).collect();
        assert_eq!(params, ["in.a12.link", "mon.volume"]);
        assert_eq!(cfg.limits.get("in.a12.link"), Some(&0.5));
        assert!(!cfg.limits.contains_key("in.link"));
    }

    #[test]
    fn missing_autostart_defaults_on() {
        let cfg: Config = serde_json::from_str(r#"{"close_to_tray":true}"#).unwrap();
        assert!(cfg.autostart);
        assert!(Config::default().autostart);
    }
}
