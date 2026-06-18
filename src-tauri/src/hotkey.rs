//! Global hotkey registration.
//!
//! Default: `Ctrl+Shift+E`. Uses `tauri-plugin-global-shortcut`.
//! On conflict, the plugin returns an error which we surface as a toast.

use tauri::{AppHandle, Manager};
use tracing::info;

pub fn register(app: &AppHandle, shortcut: &str) -> anyhow::Result<()> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;

    app.global_shortcut().on_shortcuts([shortcut], |_app, _shortcut, event| {
        if event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed {
            if let Some(overlay) = _app.get_webview_window("overlay") {
                let _ = overlay.show();
                let _ = overlay.set_focus();
            }
        }
    })?;

    info!(%shortcut, "global hotkey registered");
    Ok(())
}
