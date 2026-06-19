//! Tauri IPC commands (Frontend ↔ Backend).
//!
//! See `design_docs/05_API_Design.md` §2 for the command catalogue.
//! Each command is `#[tauri::command]` and takes Tauri managed state via params.

use tauri::{AppHandle, Emitter, State};

use crate::accessibility::{AccessibilityService, IAccessibilityService};
use crate::db::DbService;
use crate::engine::OptimizationEngine;
use crate::overlay;
use crate::replacement::ReplacementService;
use crate::types::*;

// ─── Capture ─────────────────────────────────────────────────────────────

#[tauri::command]
pub fn capture_text(
    access: State<'_, AccessibilityService>,
) -> Result<CaptureResult, ApiError> {
    let text = access
        .get_active_element_text()
        .unwrap_or_default();
    let pos = access
        .get_caret_position()
        .unwrap_or(Position { x: 400.0, y: 300.0 });
    Ok(CaptureResult { text, position: pos })
}

// ─── Optimize ────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn optimize_prompt(
    app: AppHandle,
    engine: State<'_, OptimizationEngine>,
    db: State<'_, DbService>,
    request: OptimizeRequest,
) -> Result<OptimizeResult, ApiError> {
    let settings = db.get_settings().map_err(|e| ApiError::internal(e.to_string()))?;
    engine
        .optimize(app, &db, request, &settings.ollama_url)
        .await
        .map_err(|e| ApiError::provider_unreachable(e.to_string()))
}

// ─── Replace ────────────────────────────────────────────────────────────

#[tauri::command]
pub fn accept_replacement(
    app: AppHandle,
    access: State<'_, AccessibilityService>,
    text: String,
) -> Result<ReplaceResult, ApiError> {
    Ok(ReplacementService::replace(&app, &access, &text))
}

// ─── Models ───────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_models(
    db: State<'_, DbService>,
    provider: String,
) -> Result<Vec<ModelInfo>, ApiError> {
    let settings = db.get_settings().map_err(|e| ApiError::internal(e.to_string()))?;
    let selector = format!("{provider}:");
    let adapter = crate::providers::build(&selector, &settings.ollama_url)
        .ok_or_else(|| ApiError::provider_unreachable(format!("unknown provider: {provider}")))?;
    adapter
        .list_models()
        .await
        .map_err(|e| ApiError::provider_unreachable(e.to_string()))
}

// ─── Prompt library ──────────────────────────────────────────────────────

#[tauri::command]
pub fn save_prompt(db: State<'_, DbService>, prompt: Prompt) -> Result<String, ApiError> {
    db.save_prompt(&prompt)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(prompt.id)
}

#[tauri::command]
pub fn list_prompts(db: State<'_, DbService>) -> Result<Vec<Prompt>, ApiError> {
    db.list_prompts().map_err(|e| ApiError::internal(e.to_string()))
}

#[tauri::command]
pub fn search_prompts(db: State<'_, DbService>, query: String) -> Result<Vec<Prompt>, ApiError> {
    db.search_prompts(&query).map_err(|e| ApiError::internal(e.to_string()))
}

#[tauri::command]
pub fn delete_prompt(db: State<'_, DbService>, id: String) -> Result<(), ApiError> {
    db.delete_prompt(&id).map_err(|e| ApiError::internal(e.to_string()))
}

// ─── Context profiles ─────────────────────────────────────────────────────

#[tauri::command]
pub fn save_context(db: State<'_, DbService>, profile: ContextProfile) -> Result<(), ApiError> {
    db.save_context(&profile).map_err(|e| ApiError::internal(e.to_string()))
}

#[tauri::command]
pub fn list_contexts(db: State<'_, DbService>) -> Result<Vec<ContextProfile>, ApiError> {
    db.list_contexts().map_err(|e| ApiError::internal(e.to_string()))
}

// ─── History ──────────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_history(db: State<'_, DbService>, limit: Option<i64>) -> Result<Vec<HistoryEntry>, ApiError> {
    db.list_history(limit.unwrap_or(50))
        .map_err(|e| ApiError::internal(e.to_string()))
}

// ─── Settings ─────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_settings(db: State<'_, DbService>) -> Result<Settings, ApiError> {
    db.get_settings().map_err(|e| ApiError::internal(e.to_string()))
}

#[tauri::command]
pub fn set_setting(app: AppHandle, db: State<'_, DbService>, key: String, value: String) -> Result<(), ApiError> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;

    // If changing the hotkey, re-register the global shortcut (spec §11 GAP-5).
    if key == "hotkey" {
        let old = db.get_settings().map(|s| s.hotkey).unwrap_or_default();
        db.set_setting(&key, &value).map_err(|e| ApiError::internal(e.to_string()))?;
        // Unregister old shortcut.
        if let Ok(shortcut) = old.parse::<tauri_plugin_global_shortcut::Shortcut>() {
            let _ = app.global_shortcut().unregister(shortcut);
        }
        // Register new — emit error if conflict (spec §11 GAP-7).
        if let Err(e) = crate::hotkey::register(&app, &value) {
            let _ = app.emit("hotkey_error", serde_json::json!({
                "shortcut": value,
                "message": e.to_string(),
            }));
        }
        return Ok(());
    }

    db.set_setting(&key, &value).map_err(|e| ApiError::internal(e.to_string()))
}

// ─── Frameworks ───────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_frameworks(engine: State<'_, OptimizationEngine>) -> Result<Vec<serde_json::Value>, ApiError> {
    let packs = engine.list_frameworks();
    let out: Vec<serde_json::Value> = packs
        .iter()
        .map(|p| serde_json::json!({ "id": p.id, "name": p.name }))
        .collect();
    Ok(out)
}

/// Import a custom framework pack (writes JSON to app-data dir, reloads engine).
/// Refuses to overwrite a built-in by treating it as user data — actually
/// we ALLOW override (built-ins are fallbacks) but the UI hides delete on built-ins.
#[tauri::command]
pub fn import_framework(
    app: AppHandle,
    engine: State<'_, OptimizationEngine>,
    pack: crate::engine::FrameworkPack,
) -> Result<(), ApiError> {
    use tauri::Manager;
    let app_data_dir = app.path().app_data_dir()
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let packs_dir = app_data_dir.join("framework_packs");
    std::fs::create_dir_all(&packs_dir).map_err(|e| ApiError::internal(e.to_string()))?;
    let file = packs_dir.join(format!("{}.json", sanitize_id(&pack.id)));
    let json = serde_json::to_string_pretty(&pack)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    std::fs::write(&file, json).map_err(|e| ApiError::internal(e.to_string()))?;

    let resource_dir = app.path().resource_dir().ok();
    engine.reload(&app_data_dir, resource_dir.as_deref())
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(())
}

/// Delete a framework pack by id. Refuses built-in ids.
#[tauri::command]
pub fn delete_framework(
    app: AppHandle,
    engine: State<'_, OptimizationEngine>,
    id: String,
) -> Result<(), ApiError> {
    use tauri::Manager;
    if crate::engine::is_builtin(&id) {
        return Err(ApiError::internal("cannot delete built-in framework"));
    }
    let app_data_dir = app.path().app_data_dir()
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let file = app_data_dir.join("framework_packs").join(format!("{}.json", sanitize_id(&id)));
    if file.exists() {
        std::fs::remove_file(&file).map_err(|e| ApiError::internal(e.to_string()))?;
    }
    let resource_dir = app.path().resource_dir().ok();
    engine.reload(&app_data_dir, resource_dir.as_deref())
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(())
}

/// Strip path separators / unsafe chars from an id before using as filename.
fn sanitize_id(id: &str) -> String {
    id.chars()
        .map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect()
}

// ─── Overlay ──────────────────────────────────────────────────────────────

#[tauri::command]
pub fn show_overlay(app: AppHandle, pos: Position) -> Result<(), ApiError> {
    overlay::show_overlay(&app, pos).map_err(|e| ApiError::internal(e.to_string()))
}

#[tauri::command]
pub fn hide_overlay(app: AppHandle) -> Result<(), ApiError> {
    overlay::hide_overlay(&app);
    Ok(())
}

// ─── Diagnostics ──────────────────────────────────────────────────────────

#[tauri::command]
pub fn db_stats(db: State<'_, DbService>) -> serde_json::Value {
    db.stats()
}
