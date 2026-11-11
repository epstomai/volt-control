mod autostart;
mod config;
mod engine;
mod export;
mod helper;
mod i18n;
mod midi;
mod model;
mod screen;
mod system;
mod uasrv;
mod window;

use std::sync::Arc;

use serde::Serialize;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, RunEvent, State, WindowEvent};

use config::Config;
use engine::{Origin, Shared, Status};
use midi::{Mapping, Ports};
use model::Snapshot;

type AppState<'a> = State<'a, Arc<Shared>>;
type CmdResult<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

// ------------------------------------------------------------- device commands

#[tauri::command]
fn get_status(s: AppState) -> Status {
    s.status.lock().unwrap().clone()
}

#[tauri::command]
fn get_snapshot(s: AppState) -> Option<Snapshot> {
    s.model.lock().unwrap().as_ref().map(|m| m.snapshot())
}

#[tauri::command]
fn set_param(s: AppState, id: String, normalized: f64) -> CmdResult<()> {
    s.activity();
    s.set_param(&id, normalized, Origin::Ui).map_err(err)
}

#[tauri::command]
fn trigger(s: AppState, id: String) -> CmdResult<()> {
    s.activity();
    s.set_param(&id, 1.0, Origin::Ui).map_err(err)
}

// ------------------------------------------------------------- settings

#[tauri::command]
fn get_config(s: AppState) -> Config {
    s.config.lock().unwrap().clone()
}

/// `max` = None removes the ceiling.
#[tauri::command]
fn set_limit(s: AppState, id: String, max: Option<f64>) {
    {
        let mut cfg = s.config.lock().unwrap();
        match max {
            Some(v) => cfg.limits.insert(id, v.clamp(0.0, 1.0)),
            None => cfg.limits.remove(&id),
        };
    }
    s.save_config();
}

#[tauri::command]
fn set_close_to_tray(s: AppState, enabled: bool) {
    s.config.lock().unwrap().close_to_tray = enabled;
    s.save_config();
}

#[tauri::command]
fn set_autostart(s: AppState, enabled: bool) -> CmdResult<()> {
    autostart::apply(enabled).map_err(err)?;
    s.config.lock().unwrap().autostart = enabled;
    s.save_config();
    Ok(())
}

/// Idle minutes before the Volt's screen goes dark (0 = never).
#[tauri::command]
fn set_screen_sleep(s: AppState, minutes: u32) {
    s.config.lock().unwrap().screen_sleep_min = minutes;
    s.save_config();
    s.activity();
}

/// Language in effect (resolved from the setting).
#[tauri::command]
fn get_language() -> i18n::Lang {
    i18n::current()
}

/// `lang` = None follows the Windows display language. Returns the language in effect.
#[tauri::command]
fn set_language(app: AppHandle, s: AppState, lang: Option<i18n::Lang>) -> i18n::Lang {
    let lang = s.set_language(lang);
    if let (Some(tray), Ok(menu)) = (app.tray_by_id("main"), tray_menu(&app)) {
        let _ = tray.set_menu(Some(menu));
    }
    lang
}

#[tauri::command]
fn set_volume_curve(s: AppState, enabled: bool) {
    s.set_volume_curve(enabled);
}

#[tauri::command]
fn get_system(s: AppState) -> Vec<model::Param> {
    s.system_params()
}

/// Saves the device's UASrv tree to the desktop and shows it in Explorer.
#[tauri::command]
async fn export_device_info(app: AppHandle) -> CmdResult<String> {
    let tree = export::collect().await.map_err(err)?;
    let dir = app.path().desktop_dir().map_err(err)?;
    let path = export::save(&tree, &dir).map_err(err)?;
    log::info!("device info exported to {}", path.display());
    reveal(&path);
    Ok(path.display().to_string())
}

fn reveal(path: &std::path::Path) {
    use std::os::windows::process::CommandExt;
    let _ = std::process::Command::new("explorer").raw_arg(format!("/select,\"{}\"", path.display())).spawn();
}

#[tauri::command]
fn show_tab(app: AppHandle, tab: String) {
    window::show_tab(&app, &tab);
}

// ------------------------------------------------------------- MIDI

#[derive(Serialize)]
struct MidiState {
    ports: Ports,
    open_inputs: Vec<String>,
    open_output: Option<String>,
    learn: Option<String>,
    mappings: Vec<Mapping>,
}

#[tauri::command]
fn midi_state(s: AppState) -> MidiState {
    let rt = s.midi.lock().unwrap();
    MidiState {
        ports: midi::list_ports(),
        open_inputs: rt.io.open_inputs.clone(),
        open_output: rt.io.open_output.clone(),
        learn: rt.learn.clone(),
        mappings: s.config.lock().unwrap().midi.mappings.clone(),
    }
}

/// Selects ports, reopens them and returns errors (empty = all opened).
#[tauri::command]
fn midi_select(s: AppState, inputs: Vec<String>, output: Option<String>) -> Vec<String> {
    {
        let mut cfg = s.config.lock().unwrap();
        cfg.midi.inputs = inputs;
        cfg.midi.output = output;
    }
    s.save_config();
    let errors = s.reopen_midi();
    s.resync_feedback();
    errors
}

/// Arms MIDI learn for a parameter (None cancels).
#[tauri::command]
fn midi_learn(s: AppState, param: Option<String>) {
    s.midi.lock().unwrap().learn = param;
}

fn publish_mappings(s: &Shared) {
    let mappings = s.config.lock().unwrap().midi.mappings.clone();
    let _ = s.app.emit(engine::EV_MIDI_MAPPINGS, mappings);
}

#[tauri::command]
fn midi_update_mapping(s: AppState, mapping: Mapping) -> CmdResult<()> {
    {
        let mut cfg = s.config.lock().unwrap();
        let slot = cfg.midi.mappings.iter_mut().find(|m| m.id == mapping.id).ok_or(i18n::t("err.no_mapping"))?;
        *slot = mapping;
    }
    s.save_config();
    publish_mappings(&s);
    Ok(())
}

#[tauri::command]
fn midi_delete_mapping(s: AppState, id: u64) {
    s.config.lock().unwrap().midi.mappings.retain(|m| m.id != id);
    s.save_config();
    publish_mappings(&s);
}

// ------------------------------------------------------------- window & tray

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

fn toggle_param(app: &AppHandle, id: &str) {
    let s = app.state::<Arc<Shared>>();
    s.activity();
    let current = s.model.lock().unwrap().as_ref().and_then(|m| m.get(id)).map(|p| p.normalized);
    if let Some(n) = current
        && let Err(e) = s.set_param(id, if n >= 0.5 { 0.0 } else { 1.0 }, Origin::Ui)
    {
        s.notice(&e.to_string());
    }
}

/// Tray menu in the current language (rebuilt when it changes).
fn tray_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let item = |id: &str, key: &str| MenuItem::with_id(app, id, i18n::t(key), true, None::<&str>);
    let show = item("show", "tray.show")?;
    let mon_mute = item("mon_mute", "tray.mon_mute")?;
    let hp_mute = item("hp_mute", "tray.hp_mute")?;
    let quit = item("quit", "tray.quit")?;
    let sep = PredefinedMenuItem::separator(app)?;
    Menu::with_items(app, &[&show, &mon_mute, &hp_mute, &sep, &quit])
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let menu = tray_menu(app)?;
    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip("Volt Control")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, ev| match ev.id().as_ref() {
            "show" => show_main(app),
            "mon_mute" => toggle_param(app, "mon.mute"),
            "hp_mute" => toggle_param(app, "hp.mute"),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, ev| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = ev {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
/// `VOLT_LOG=debug|trace` raises logging for diagnosis; otherwise Debug in dev, Info in release.
fn log_level() -> log::LevelFilter {
    std::env::var("VOLT_LOG")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(if cfg!(debug_assertions) { log::LevelFilter::Debug } else { log::LevelFilter::Info })
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_main(app)))
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(log_level())
                // The default (one 40 KB file) loses evidence within minutes at trace level.
                .max_file_size(2_000_000)
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepSome(5))
                .timezone_strategy(tauri_plugin_log::TimezoneStrategy::UseLocal)
                // Third-party crates stay at Info even when VOLT_LOG raises ours.
                .level_for("tao", log::LevelFilter::Info)
                .level_for("wry", log::LevelFilter::Info)
                .build(),
        )
        .setup(|app| {
            let dir = app.path().app_config_dir()?;
            let store = config::Store::new(&dir);
            // Before anything renders text (status, tray, parameter names).
            i18n::set(i18n::resolve(store.load().language));
            let shared = engine::start(app.handle().clone(), store);
            if let Err(e) = autostart::apply(shared.config.lock().unwrap().autostart) {
                log::warn!("autostart: {e}");
            }
            app.manage(shared);
            app.manage(window::TabWindow::default());
            window::show_tab(app.handle(), "main");
            if !autostart::launched_hidden() {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }
            build_tray(app.handle())?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::Resized(size) = event {
                window::on_resized(window, *size);
            }
            if let WindowEvent::CloseRequested { api, .. } = event {
                let to_tray = window.state::<Arc<Shared>>().config.lock().unwrap().close_to_tray;
                if to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_status,
            get_snapshot,
            set_param,
            trigger,
            get_config,
            set_limit,
            set_close_to_tray,
            set_autostart,
            set_volume_curve,
            set_screen_sleep,
            get_language,
            set_language,
            show_tab,
            export_device_info,
            get_system,
            midi_state,
            midi_select,
            midi_learn,
            midi_update_mapping,
            midi_delete_mapping,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Volt Control")
        .run(|app, event| match event {
            // Last window closed/hidden: stay in the tray. Tray "退出" calls
            // app.exit(0) and arrives here with Some(0), which we allow.
            RunEvent::ExitRequested { api, code, .. } => {
                log::info!("exit requested code={code:?}");
                if code.is_none() {
                    api.prevent_exit();
                }
            }
            RunEvent::Exit => {
                log::info!("exit");
                app.state::<Arc<Shared>>().wake_screen_now();
            }
            _ => {}
        });
}
