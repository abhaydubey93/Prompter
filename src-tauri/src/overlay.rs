//! Overlay window management.
//!
//! Controls show/hide, caret/mouse-anchored positioning, and edge-aware
//! placement. See spec §7, LLD §2.1–2.2.

use std::sync::Mutex;

use tauri::{AppHandle, Manager};
use tracing::info;

use crate::types::Position;

/// Tracks the window that had focus before the overlay was shown,
/// so we can restore it on hide (spec §7: "restore focus to prior target").
pub struct PriorFocus(pub Mutex<Option<String>>);

/// Show the overlay anchored near the given position, with edge-aware
/// adjustments so it doesn't clip off-screen.
pub fn show_overlay(app: &AppHandle, pos: Position) -> anyhow::Result<()> {
    let Some(window) = app.get_webview_window("overlay") else {
        anyhow::bail!("overlay window not found");
    };

    let (x, y) = compute_position(pos, 400.0, 340.0);
    let _ = window.set_position(tauri::Position::Physical(
        tauri::PhysicalPosition::new(x as i32, y as i32),
    ));

    window.show()?;
    info!(x, y, "overlay shown");
    Ok(())
}

/// Hide the overlay and restore focus to the prior target window (spec §7).
pub fn hide_overlay(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("overlay") {
        let _ = window.hide();
    }
    // Restore focus to the window that was active before the overlay.
    if let Some(prior) = app.try_state::<PriorFocus>() {
        if let Some(label) = prior.0.lock().unwrap().take() {
            if let Some(target) = app.get_webview_window(&label) {
                let _ = target.set_focus();
                return;
            }
        }
    }
    // Fallback: main window.
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.set_focus();
    }
}

/// Edge-aware position calculation (LLD §2.2).
fn compute_position(caret: Position, width: f64, height: f64) -> (f64, f64) {
    let mut x = caret.x;
    let mut y = caret.y + 20.0; // Below caret by default

    // Clamp to primary monitor (use screen size from Tauri or safe defaults).
    let screen_w = 1920.0; // conservative default
    let screen_h = 1080.0;

    if x + width > screen_w {
        x = (screen_w - width - 10.0).max(0.0);
    }
    if x < 0.0 {
        x = 10.0;
    }
    if y + height > screen_h {
        y = (caret.y - height - 10.0).max(0.0); // Flip above
    }
    if y < 0.0 {
        y = 10.0;
    }

    (x, y)
}
