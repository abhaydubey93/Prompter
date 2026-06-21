//! Global hotkey registration.
//!
//! Default: `Ctrl+Shift+E`. Uses `tauri-plugin-global-shortcut`.
//! On conflict, the plugin returns an error which we surface as a toast.

use tauri::{AppHandle, Emitter, Manager};
use tracing::info;

use crate::accessibility::IAccessibilityService;
use crate::overlay;

pub fn register(app: &AppHandle, shortcut: &str) -> anyhow::Result<()> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;

    app.global_shortcut().on_shortcuts([shortcut], move |_app, _shortcut, event| {
        if event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed {
            let app = _app.clone();

            // Remember the currently-focused window so we can restore it on Esc.
            if let Some(prior) = app.try_state::<overlay::PriorFocus>() {
                *prior.0.lock().unwrap() = overlay::get_foreground_window();
            }

            // Capture text + caret position from accessibility layer.
            let access = app.state::<crate::accessibility::AccessibilityService>();
            let mut text = access.get_active_element_text().unwrap_or_default();
            let pos = access.get_caret_position().unwrap_or(crate::types::Position {
                x: 400.0,
                y: 300.0,
            });

            // If text is empty (e.g. Scintilla/Notepad++), try clipboard fallback.
            if text.trim().is_empty() {
                if let Ok(clip) = capture_via_clipboard(&app) {
                    text = clip;
                }
            }

            // Show overlay at caret-aware position.
            let _ = overlay::show_overlay(&app, pos);

            // Buffer text for the overlay to fetch on mount (avoids emit/show
            // race where the React listener isn't registered yet).
            overlay::set_pending_text(&app, text.clone());

            // Also emit overlay_show for late listeners (best-effort backup).
            let _ = app.emit("overlay_show", serde_json::json!({
                "text": text,
                "position": pos,
            }));
        }
    })?;

    info!(%shortcut, "global hotkey registered");
    Ok(())
}

fn capture_via_clipboard(app: &AppHandle) -> anyhow::Result<String> {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    use std::thread;
    use std::time::Duration;

    // Backup current clipboard.
    let backup: Option<String> = app.clipboard().read_text().ok();

    // Clear clipboard to detect if Ctrl+C copied anything.
    let _ = app.clipboard().write_text("".to_string());

    // Send synthetic Ctrl+C
    #[cfg(windows)]
    {
        use enigo::{Direction, Enigo, Key, Keyboard, Settings};
        if let Ok(mut enigo) = Enigo::new(&Settings::default()) {
            // Release Shift/Alt/Meta in case user is holding them for the hotkey (e.g. Ctrl+Shift+E)
            let _ = enigo.key(Key::Shift, Direction::Release);
            let _ = enigo.key(Key::Alt, Direction::Release);
            let _ = enigo.key(Key::Meta, Direction::Release);
            let _ = enigo.key(Key::Control, Direction::Release);

            let _ = enigo.key(Key::Control, Direction::Press);
            let _ = enigo.key(Key::Unicode('c'), Direction::Click);
            let _ = enigo.key(Key::Control, Direction::Release);
        }
    }

    // Wait for clipboard to populate (apps might take a moment to write to clipboard).
    // Loop for up to 400ms to catch slower apps like Notepad++
    let mut captured = String::new();
    for _ in 0..8 {
        thread::sleep(Duration::from_millis(50));
        if let Ok(text) = app.clipboard().read_text() {
            if !text.is_empty() {
                captured = text;
                break;
            }
        }
    }

    // Restore original clipboard.
    if let Some(b) = backup {
        let _ = app.clipboard().write_text(b);
    } else {
        let _ = app.clipboard().write_text("".to_string());
    }

    Ok(captured)
}
