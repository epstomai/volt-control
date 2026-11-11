//! Each tab remembers its own window size (inner, logical px). Switching tabs
//! resizes the window instantly, keeping its top-left corner unless that would
//! push it off the screen. A maximized window is left alone.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewWindow, Window};

use crate::engine::Shared;

/// Resize events this soon after our own resize are ours, not the user's.
const OWN_RESIZE: Duration = Duration::from_millis(400);
/// Window sizes are saved once dragging has paused this long.
const SAVE_DELAY: Duration = Duration::from_millis(600);

pub struct TabWindow {
    tab: Mutex<String>,
    applied_at: Mutex<Option<Instant>>,
    maximized: AtomicBool,
    save_gen: AtomicU64,
}

impl Default for TabWindow {
    fn default() -> Self {
        TabWindow { tab: Mutex::new("main".into()), applied_at: Mutex::default(), maximized: AtomicBool::default(), save_gen: AtomicU64::default() }
    }
}

/// Sizes that fit each tab's content.
pub fn default_size(tab: &str) -> [f64; 2] {
    match tab {
        "mixer" => [520.0, 500.0],
        "midi" => [1170.0, 480.0],
        "settings" => [810.0, 615.0],
        _ => [858.0, 400.0],
    }
}

fn size_for(app: &AppHandle, tab: &str) -> [f64; 2] {
    let shared = app.state::<Arc<Shared>>();
    let saved = shared.config.lock().unwrap().window_sizes.get(tab).copied();
    saved.unwrap_or_else(|| default_size(tab))
}

/// Makes `tab` current and gives the window that tab's size.
pub fn show_tab(app: &AppHandle, tab: &str) {
    *app.state::<TabWindow>().tab.lock().unwrap() = tab.to_string();
    let Some(w) = app.get_webview_window("main") else { return };
    if w.is_maximized().unwrap_or(false) || w.is_minimized().unwrap_or(false) {
        return;
    }
    if let Err(e) = apply(app, &w, size_for(app, tab)) {
        log::warn!("resize for tab {tab}: {e}");
    }
}

fn apply(app: &AppHandle, w: &WebviewWindow, [lw, lh]: [f64; 2]) -> tauri::Result<()> {
    let scale = w.scale_factor()?;
    let (inner, outer) = (w.inner_size()?, w.outer_size()?);
    let frame_w = outer.width.saturating_sub(inner.width) as i64;
    let frame_h = outer.height.saturating_sub(inner.height) as i64;
    let mut width = (lw * scale).round() as i64;
    let mut height = (lh * scale).round() as i64;
    let mut pos = w.outer_position()?;
    if let Some(m) = w.current_monitor()? {
        let area = m.work_area();
        let (ax, ay) = (area.position.x as i64, area.position.y as i64);
        let (aw, ah) = (area.size.width as i64, area.size.height as i64);
        width = width.min(aw - frame_w);
        height = height.min(ah - frame_h);
        let x = (pos.x as i64).min(ax + aw - width - frame_w).max(ax);
        let y = (pos.y as i64).min(ay + ah - height - frame_h).max(ay);
        pos = PhysicalPosition::new(x as i32, y as i32);
    }
    log::debug!("apply {lw}x{lh} -> {width}x{height} at {pos:?} (frame {frame_w}x{frame_h})");
    *app.state::<TabWindow>().applied_at.lock().unwrap() = Some(Instant::now());
    w.set_size(PhysicalSize::new(width.max(1) as u32, height.max(1) as u32))?;
    w.set_position(pos)
}

/// Remembers a size the user dragged to, for the current tab.
pub fn on_resized(window: &Window, size: PhysicalSize<u32>) {
    let app = window.app_handle();
    let tw = app.state::<TabWindow>();
    log::debug!(
        "resized {}x{} maximized={:?} flag={} since_apply={:?} tab={}",
        size.width, size.height, window.is_maximized(), tw.maximized.load(Ordering::SeqCst),
        tw.applied_at.lock().unwrap().map(|t| t.elapsed()), tw.tab.lock().unwrap()
    );
    if size.width == 0 || size.height == 0 || window.is_minimized().unwrap_or(false) {
        return;
    }
    if window.is_maximized().unwrap_or(false) {
        tw.maximized.store(true, Ordering::SeqCst);
        return;
    }
    let tab = tw.tab.lock().unwrap().clone();
    if tw.maximized.swap(false, Ordering::SeqCst) {
        // Restored from maximized, possibly on another tab than it was maximized on.
        show_tab(app, &tab);
        return;
    }
    if tw.applied_at.lock().unwrap().is_some_and(|t| t.elapsed() < OWN_RESIZE) {
        return;
    }
    let scale = window.scale_factor().unwrap_or(1.0);
    let logical = size.to_logical::<f64>(scale);
    let shared = app.state::<Arc<Shared>>();
    shared.config.lock().unwrap().window_sizes.insert(tab, [logical.width.round(), logical.height.round()]);
    let generation = tw.save_gen.fetch_add(1, Ordering::SeqCst) + 1;
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(SAVE_DELAY).await;
        if app.state::<TabWindow>().save_gen.load(Ordering::SeqCst) == generation {
            app.state::<Arc<Shared>>().save_config();
        }
    });
}
