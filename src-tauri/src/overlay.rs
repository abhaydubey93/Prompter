//! Overlay window management.
//!
//! Controls show/hide, caret/mouse-anchored positioning, and edge-aware
//! placement. See spec §7, LLD §2.1–2.2.

use std::sync::Mutex;

use tauri::{AppHandle, Manager};
use tracing::info;

use crate::types::Position;

/// Tracks the window that had focus before the overlay was shown,
/// so we can restore focus to it on hide (spec §7: "restore focus to prior target").
pub struct PriorFocus(pub Mutex<Option<usize>>);

/// Buffer for the text captured at hotkey time. The overlay window reads
/// this on mount via `take_pending_text` — avoids the emit/show race where
/// the global `overlay_show` event fires before the overlay's listener is
/// registered (React hasn't mounted yet).
pub struct PendingText(pub Mutex<Option<String>>);

pub fn set_pending_text(app: &AppHandle, text: String) {
    if let Some(p) = app.try_state::<PendingText>() {
        *p.0.lock().unwrap() = Some(text);
    }
}

/// Consume and return the buffered text (None if already taken or never set).
pub fn take_pending_text(app: &AppHandle) -> Option<String> {
    app.try_state::<PendingText>()
        .and_then(|p| p.0.lock().unwrap().take())
}

/// Show the overlay anchored near the given position, with edge-aware
/// adjustments so it doesn't clip off-screen.
pub fn show_overlay(app: &AppHandle, pos: Position) -> anyhow::Result<()> {
    let Some(window) = app.get_webview_window("overlay") else {
        anyhow::bail!("overlay window not found");
    };

    // Real monitor size if available (WP-F fix: was hardcoded 1920x1080).
    let (mon_w, mon_h, mon_x, mon_y) = window
        .current_monitor()
        .ok()
        .flatten()
        .map(|m| {
            let s = m.size();
            let p = m.position();
            (s.width as f64, s.height as f64, p.x as f64, p.y as f64)
        })
        .unwrap_or((1920.0, 1080.0, 0.0, 0.0));

    let (x, y) = compute_position(pos, 560.0, 420.0, mon_w, mon_h, mon_x, mon_y);
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
        if let Some(hwnd) = prior.0.lock().unwrap().take() {
            set_foreground_window(hwnd);
        }
    }
}

#[cfg(windows)]
pub fn get_foreground_window() -> Option<usize> {
    extern "system" {
        fn GetForegroundWindow() -> isize;
    }
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd != 0 {
        Some(hwnd as usize)
    } else {
        None
    }
}

#[cfg(not(windows))]
pub fn get_foreground_window() -> Option<usize> {
    None
}

#[cfg(windows)]
pub fn set_foreground_window(hwnd_val: usize) {
    extern "system" {
        fn SetForegroundWindow(hwnd: isize) -> i32;
    }
    unsafe {
        SetForegroundWindow(hwnd_val as isize);
    }
}

#[cfg(not(windows))]
pub fn set_foreground_window(_hwnd_val: usize) {}

/// Edge-aware position calculation (LLD §2.2). Clamps inside the given
/// monitor's rect so the overlay never clips off-screen.
fn compute_position(
    caret: Position,
    width: f64,
    height: f64,
    mon_w: f64,
    mon_h: f64,
    mon_x: f64,
    mon_y: f64,
) -> (f64, f64) {
    let mut x = caret.x;
    let mut y = caret.y + 20.0; // below caret by default

    let right = mon_x + mon_w;
    let bottom = mon_y + mon_h;

    if x + width > right {
        x = (right - width - 10.0).max(mon_x);
    }
    if x < mon_x {
        x = mon_x + 10.0;
    }
    if y + height > bottom {
        y = (caret.y - height - 10.0).max(mon_y); // flip above
    }
    if y < mon_y {
        y = mon_y + 10.0;
    }

    (x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clamps_to_monitor_right_edge() {
        // Caret near right edge → overlay pushed left to stay inside.
        let (x, _) = compute_position(
            Position { x: 1900.0, y: 100.0 },
            400.0, 340.0,
            1920.0, 1080.0, 0.0, 0.0,
        );
        assert!(x + 400.0 <= 1920.0, "x={x} should keep overlay inside");

        // New 560x420 size also clamps.
        let (x2, _) = compute_position(
            Position { x: 1900.0, y: 100.0 },
            560.0, 420.0,
            1920.0, 1080.0, 0.0, 0.0,
        );
        assert!(x2 + 560.0 <= 1920.0, "x2={x2} should keep overlay inside");
    }

    #[test]
    fn test_flips_above_near_bottom() {
        // Caret near bottom → overlay flips above.
        let (_, y) = compute_position(
            Position { x: 100.0, y: 1000.0 },
            400.0, 340.0,
            1920.0, 1080.0, 0.0, 0.0,
        );
        assert!(y + 340.0 <= 1080.0, "y={y} should keep overlay inside");
    }

    #[test]
    fn test_handles_negative_monitor_origin() {
        // Second monitor to the left of origin (mon_x = -1920).
        let (x, _) = compute_position(
            Position { x: -1900.0, y: 100.0 },
            400.0, 340.0,
            1920.0, 1080.0, -1920.0, 0.0,
        );
        assert!(x >= -1920.0, "x={x} should stay within monitor bounds");
    }
}
