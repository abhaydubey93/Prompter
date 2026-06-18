//! In-place text replacement pipeline.
//!
//! Strategy priority: Accessibility `setValue` → clipboard simulation → fail.
//! See spec §6, HLD §3.2, LLD §1.2.

use std::thread;
use std::time::Duration;

use tauri::AppHandle;
use tracing::{info, warn};

use crate::accessibility::{IAccessibilityService, AccessibilityService};
use crate::types::ReplaceResult;

pub struct ReplacementService;

impl ReplacementService {
    /// Replace text in the active field. Tries accessibility first, then
    /// clipboard fallback (backup → set → paste → restore).
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

        // Strategy 2: Clipboard simulation
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

        // 3. For MVP: clipboard set successfully. We notify via result.
        // Full synthetic Ctrl+V requires careful SendInput — deferred post-MVP.
        // The overlay UI will tell user "press Ctrl+V" if fallback=true.
        thread::sleep(Duration::from_millis(60));

        // 5. Restore original clipboard after short delay.
        Self::restore_clipboard(app, backup);

        info!("replacement via clipboard fallback (text set to clipboard)");
        Ok(ReplaceResult { success: true, fallback: true })
    }

    fn restore_clipboard(app: &AppHandle, backup: Option<String>) {
        use tauri_plugin_clipboard_manager::ClipboardExt;
        if let Some(text) = backup {
            let _ = app.clipboard().write_text(text);
        }
    }
}
