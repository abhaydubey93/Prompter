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

            // Capture text + caret position from accessibility layer.
            let access = app.state::<crate::accessibility::AccessibilityService>();
            let text = access.get_active_element_text().unwrap_or_default();
            let pos = access.get_caret_position().unwrap_or(crate::types::Position {
                x: 400.0,
                y: 300.0,
            });

            // Show overlay at caret-aware position.
            let _ = overlay::show_overlay(&app, pos);

            // Emit overlay_show event so the overlay React component receives raw text.
            let _ = app.emit("overlay_show", serde_json::json!({
                "text": text,
                "position": { "x": pos.x, "y": pos.y },
            }));
        }
    })?;

    info!(%shortcut, "global hotkey registered");
    Ok(())
}
