//! HKCU Run entry so Volt Control starts with Windows (hidden in the tray).

use std::path::{Path, PathBuf};

use anyhow::Result;
use winreg::RegKey;
use winreg::enums::*;

const VALUE: &str = "Volt Control";
const KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const HIDDEN: &str = "--hidden";

pub fn launched_hidden() -> bool {
    std::env::args().any(|a| a == HIDDEN)
}

/// Writes or removes the login Run entry. Debug builds skip this so `tauri dev`
/// cannot register a binary that will not work after a reboot.
pub fn apply(enabled: bool) -> Result<()> {
    if cfg!(debug_assertions) {
        log::debug!("debug build: not changing Windows autostart");
        return Ok(());
    }
    let exe = strip_verbatim(std::env::current_exe()?);
    apply_named(VALUE, enabled, &exe)
}

fn apply_named(name: &str, enabled: bool, exe: &Path) -> Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu.create_subkey(KEY)?;
    if enabled {
        key.set_value(name, &command_line(exe))?;
        log::info!("autostart on ({})", command_line(exe));
    } else if let Err(e) = key.delete_value(name) {
        if e.kind() != std::io::ErrorKind::NotFound {
            return Err(e.into());
        }
        log::info!("autostart off");
    } else {
        log::info!("autostart off");
    }
    Ok(())
}

fn command_line(exe: &Path) -> String {
    format!("\"{}\" {HIDDEN}", exe.display())
}

/// `canonicalize` / some `current_exe` paths get a `\\?\` prefix that the Run
/// key will not launch.
fn strip_verbatim(p: PathBuf) -> PathBuf {
    let s = p.to_string_lossy();
    s.strip_prefix(r"\\?\").map(PathBuf::from).unwrap_or(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_path_with_spaces() {
        let p = Path::new(r"C:\Users\me\AppData\Local\Volt Control\volt-control.exe");
        assert_eq!(command_line(p), r#""C:\Users\me\AppData\Local\Volt Control\volt-control.exe" --hidden"#);
    }

    #[test]
    fn strip_verbatim_prefix() {
        let p = PathBuf::from(r"\\?\H:\Codex Projs\UAD-RE\volt-control.exe");
        assert_eq!(strip_verbatim(p), PathBuf::from(r"H:\Codex Projs\UAD-RE\volt-control.exe"));
    }

    #[test]
    fn registry_roundtrip() {
        let name = "Volt Control (test, safe to delete)";
        let exe = Path::new(r"C:\Windows\System32\notepad.exe");
        apply_named(name, true, exe).unwrap();
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let key = hkcu.open_subkey(KEY).unwrap();
        let val: String = key.get_value(name).unwrap();
        assert_eq!(val, command_line(exe));
        apply_named(name, false, exe).unwrap();
        let key = hkcu.open_subkey(KEY).unwrap();
        let missing: Result<String, _> = key.get_value(name);
        assert!(missing.is_err());
    }
}
