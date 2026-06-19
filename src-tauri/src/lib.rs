//! PromptOpt — Local-first prompt optimization overlay.
//!
//! Entry point called from `main.rs`. Initializes all services, registers
//! Tauri plugins and IPC commands, and starts the event loop.

pub mod accessibility;
pub mod commands;
pub mod db;
pub mod engine;
pub mod hotkey;
pub mod overlay;
pub mod providers;
pub mod replacement;
pub mod types;

use tauri::{Emitter, Manager};
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

use accessibility::AccessibilityService;
use db::DbService;
use engine::OptimizationEngine;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Logging.
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("promptopt=debug".parse().unwrap()))
        .with_target(false)
        .init();

    info!("PromptOpt starting");

    // Pre-compute the app data dir (before Tauri Builder, which may need it).
    let app_data_dir = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("PromptOpt");

    // ─── Build ────────────────────────────────────────────────────────────
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(setup_db(&app_data_dir))
        .manage(setup_accessibility())
        .manage(overlay::PriorFocus(std::sync::Mutex::new(None)))
        .invoke_handler(tauri::generate_handler![
            commands::capture_text,
            commands::optimize_prompt,
            commands::accept_replacement,
            commands::get_models,
            commands::test_provider,
            commands::save_prompt,
            commands::list_prompts,
            commands::search_prompts,
            commands::delete_prompt,
            commands::save_context,
            commands::list_contexts,
            commands::list_history,
            commands::get_settings,
            commands::set_setting,
            commands::list_frameworks,
            commands::show_overlay,
            commands::hide_overlay,
            commands::db_stats,
            commands::import_framework,
            commands::delete_framework,
            commands::list_providers,
            commands::get_provider,
            commands::save_provider,
            commands::delete_provider,
            commands::set_provider_enabled,
            commands::get_meta,
            commands::set_meta,
            commands::clear_history,
            commands::set_provider_key,
            commands::get_onboarding_state,
            commands::complete_onboarding,
        ])
        .setup(move |app| {
            // Resolve bundled resource dir (framework_packs shipped in the binary).
            let resource_dir = app.path().resource_dir().ok();

            // Init engine with resource dir (post-builder, so paths are available).
            let engine = OptimizationEngine::new(&app_data_dir, resource_dir.as_deref())
                .expect("failed to init optimization engine");
            app.manage(engine);

            // Register global hotkey.
            let handle = app.handle().clone();
            let settings = handle.state::<DbService>().get_settings().ok();
            let shortcut = settings
                .as_ref()
                .map(|s| s.hotkey.clone())
                .unwrap_or_else(|| "Ctrl+Shift+E".to_string());

            if let Err(e) = hotkey::register(&handle, &shortcut) {
                tracing::warn!("failed to register hotkey '{shortcut}': {e}");
            }

            // Startup provider health sweep (spec §11 mitigation).
            // Probe every enabled provider; emit one provider_status event each.
            // Used by the overlay's dead-provider banner and the onboarding gate.
            {
                let providers = handle.state::<DbService>().list_providers().unwrap_or_default();
                let handle2 = handle.clone();
                tauri::async_runtime::spawn(async move {
                    for cfg in providers.iter().filter(|p| p.enabled) {
                        let alive = match providers::build_adapter(cfg) {
                            Ok(a) => a.health_check().await,
                            Err(_) => false, // missing key etc.
                        };
                        if alive {
                            info!(provider = %cfg.id, "provider reachable at startup");
                        } else {
                            warn!(provider = %cfg.id, "provider NOT reachable at startup");
                        }
                        let _ = handle2.emit("provider_status", serde_json::json!({
                            "provider": cfg.id,
                            "alive": alive,
                        }));
                    }
                });
            }

            // Ensure overlay window starts hidden.
            if let Some(overlay) = app.get_webview_window("overlay") {
                let _ = overlay.hide();
            }

            info!("PromptOpt ready (hotkey: {shortcut})");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running PromptOpt");
}

fn setup_db(app_data_dir: &std::path::Path) -> DbService {
    let db_path = app_data_dir.join("data.db");
    info!(?db_path, "opening database");
    DbService::open(&db_path).expect("failed to open database")
}

fn setup_accessibility() -> AccessibilityService {
    AccessibilityService::new().expect("failed to init accessibility service")
}
