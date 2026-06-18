//! In-place text replacement pipeline.
//!
//! Strategy priority: Accessibility `setValue` → clipboard simulation → fail.
//! See spec §6, HLD §3.2, LLD §1.2.

use std::thread;
use std::time::Duration;

use tauri::AppHandle;
use tracing::{info, warn};

use crate::accessibility::{AccessibilityService, IAccessibilityService};
use crate::types::ReplaceResult;

pub struct ReplacementService;

impl ReplacementService {
    /// Replace text in the active field. Tries accessibility first, then
    /// clipboard fallback (backup → set → paste → restore) per spec §6.
    pub fn replace(
        app: &AppHandle,
        access: &AccessibilityService,
        text: &str,
    ) -> ReplaceResult {
        // Strategy 1: Accessibility API
        match access.set_element_text(text) {
            Ok(()) => {
                // Verify by re-reading.
                match access.get_active_element_text() {
                    Ok(read_back) if read_back == text => {
                        info!("replacement via accessibility succeeded");
                        return ReplaceResult { success: true, fallback: false };
                    }
                    Ok(_) => {
                        warn!("accessibility set succeeded but verification failed");
                    }
                    Err(e) => {
                        warn!("verification read failed: {e}, falling back");
                    }
                }
            }
            Err(e) => {
                warn!("accessibility set_element_text failed: {e}, trying clipboard fallback");
            }
        }

        // Strategy 2: Clipboard simulation (spec §6)
        if let Ok(res) = Self::clipboard_fallback(app, text) {
            return res;
        }

        ReplaceResult { success: false, fallback: false }
    }

    fn clipboard_fallback(app: &AppHandle, text: &str) -> Result<ReplaceResult, ()> {
        use tauri_plugin_clipboard_manager::ClipboardExt;

        // 1. Backup current clipboard.
        let backup: Option<String> = app.clipboard().read_text().ok();

        // 2. Set enhanced text to clipboard.
        if app.clipboard().write_text(text.to_string()).is_err() {
            warn!("clipboard write failed");
            return Err(());
        }

        // 3. Synthetic Ctrl+V paste via enigo (spec §6).
        //    Small delay so clipboard is committed before paste.
        thread::sleep(Duration::from_millis(60));
        let pasted = simulate_paste();
        if pasted {
            info!("synthetic Ctrl+V delivered via enigo");
        } else {
            warn!("enigo paste simulation unavailable — user must press Ctrl+V");
        }

        // 4. Restore original clipboard after paste settles.
        thread::sleep(Duration::from_millis(60));
        Self::restore_clipboard(app, backup);

        info!("replacement via clipboard fallback (text set to clipboard)");
        Ok(ReplaceResult {
            success: pasted,
            // fallback=true means "used clipboard path"; UI may also prompt Ctrl+V if paste failed.
            fallback: true,
        })
    }

    fn restore_clipboard(app: &AppHandle, backup: Option<String>) {
        use tauri_plugin_clipboard_manager::ClipboardExt;
        if let Some(text) = backup {
            let _ = app.clipboard().write_text(text);
        }
    }
}

/// Send a synthetic Ctrl+V keypress on Windows via enigo.
/// Returns `false` on non-Windows or on enigo error.
#[cfg(windows)]
fn simulate_paste() -> bool {
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};
    let mut enigo = match Enigo::new(&Settings::default()) {
        Ok(e) => e,
        Err(e) => {
            warn!("enigo init failed: {e}");
            return false;
        }
    };
    // Press Ctrl, click V, release Ctrl.
    if enigo.key(Key::Control, Direction::Press).is_err() {
        return false;
    }
    let v_ok = enigo.key(Key::Unicode('v'), Direction::Click).is_ok();
    let _ = enigo.key(Key::Control, Direction::Release);
    v_ok
}

#[cfg(not(windows))]
fn simulate_paste() -> bool {
    false
}
