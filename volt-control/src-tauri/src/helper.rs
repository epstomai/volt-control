//! Keeps `UAMixerHelper.exe` (installed with UAD Console) running.
//!
//! Nothing in the UA stack reliably starts it at login: UA Connect only asks the
//! helper service to launch it right after a driver install. So Volt Control
//! starts it itself when port 4720 refuses connections. The helper is
//! single-instance (a second copy exits on its own) and is deliberately left
//! running when Volt Control quits, because UAD Console shares it.

use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};

pub const EXE: &str = "UAMixerHelper.exe";
const INSTALL_DIR: &str = r"Universal Audio\UAD Console";

/// No console window, not tied to our console / Ctrl+C group.
const DETACHED_PROCESS: u32 = 0x0000_0008;
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;

/// Location of the helper, if UAD Console is installed.
pub fn find() -> Option<PathBuf> {
    ["ProgramFiles(x86)", "ProgramFiles"]
        .iter()
        .filter_map(std::env::var_os)
        .map(|root| PathBuf::from(root).join(INSTALL_DIR).join(EXE))
        .find(|p| p.is_file())
}

pub fn is_running() -> bool {
    process_running(EXE)
}

/// Starts the helper detached so it outlives Volt Control.
pub fn launch(exe: &PathBuf) -> Result<()> {
    let dir = exe.parent().context("helper path has no directory")?;
    Command::new(exe)
        .current_dir(dir)
        .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
        .spawn()
        .with_context(|| format!("start {}", exe.display()))?;
    log::info!("launched {}", exe.display());
    Ok(())
}

/// Whether any process has this image name (case-insensitive).
fn process_running(name: &str) -> bool {
    let Ok(snap) = (unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }) else {
        return false;
    };
    let mut entry = PROCESSENTRY32W { dwSize: size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
    let mut found = false;
    let mut ok = unsafe { Process32FirstW(snap, &mut entry) }.is_ok();
    while ok {
        let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
        if String::from_utf16_lossy(&entry.szExeFile[..len]).eq_ignore_ascii_case(name) {
            found = true;
            break;
        }
        ok = unsafe { Process32NextW(snap, &mut entry) }.is_ok();
    }
    let _ = unsafe { CloseHandle(snap) };
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sees_own_process() {
        let me = std::env::current_exe().unwrap();
        let name = me.file_name().unwrap().to_string_lossy().to_uppercase();
        assert!(process_running(&name));
        assert!(!process_running("definitely-not-running-4720.exe"));
    }
}
