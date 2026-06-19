/** Thin Tauri invoke wrapper + event listeners used by both overlay and main windows. */
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

// ─── Types (mirrors Rust types.rs) ──────────────────────────────────────

export interface Position { x: number; y: number }
export interface CaptureResult { text: string; position: Position }
export interface OptimizeRequest { raw: string; framework: string; model: string; context_id?: string }
export interface OptimizeResult { optimized: string; score: number; diff: string; tokens: number; session_id: string }
export interface ReplaceResult { success: boolean; fallback: boolean }
export interface ModelInfo { id: string; name: string }
export interface Prompt { id: string; title: string; body: string; framework?: string; model_used?: string; score: number; usage_count: number; source_app?: string; created_at: string }
export interface ContextProfile { id: string; name: string; role?: string; audience?: string; tone?: string; style_snippet?: string }
export interface HistoryEntry { id: string; raw_prompt: string; optimized_prompt: string; model: string; score?: number; timestamp: string }
export interface Settings { hotkey: string; theme: string; default_framework: string; default_model: string; ollama_url: string; default_provider_id?: string; overlay_opacity?: number }
export interface FrameworkInfo { id: string; name: string }
export interface ProviderConfig { id: string; kind: string; label: string; base_url: string; api_key_slot?: string; default_model: string; enabled: boolean; sort_order: number }

// ─── Commands ─────────────────────────────────────────────────────────────

export const cmd = {
  captureText: () => invoke<CaptureResult>("capture_text", {}),
  optimizePrompt: (req: OptimizeRequest) => invoke<OptimizeResult>("optimize_prompt", { raw: req.raw, framework: req.framework, model: req.model, context_id: req.context_id }),
  acceptReplacement: (text: string) => invoke<ReplaceResult>("accept_replacement", { text }),
  getModels: (provider: string) => invoke<ModelInfo[]>("get_models", { provider }),
  testProvider: (id: string) => invoke<{ alive: boolean; models: ModelInfo[]; error: string | null }>("test_provider", { id }),
  savePrompt: (p: Prompt) => invoke<string>("save_prompt", { prompt: p }),
  listPrompts: () => invoke<Prompt[]>("list_prompts", {}),
  searchPrompts: (q: string) => invoke<Prompt[]>("search_prompts", { query: q }),
  deletePrompt: (id: string) => invoke<void>("delete_prompt", { id }),
  saveContext: (c: ContextProfile) => invoke<void>("save_context", { profile: c }),
  listContexts: () => invoke<ContextProfile[]>("list_contexts", {}),
  listHistory: (limit?: number) => invoke<HistoryEntry[]>("list_history", { limit }),
  getSettings: () => invoke<Settings>("get_settings", {}),
  setSetting: (key: string, value: string) => invoke<void>("set_setting", { key, value }),
  listFrameworks: () => invoke<FrameworkInfo[]>("list_frameworks", {}),
  importFramework: (pack: FrameworkInfo & { variables?: string[]; template: string }) => invoke<void>("import_framework", { pack }),
  deleteFramework: (id: string) => invoke<void>("delete_framework", { id }),
  showOverlay: (pos: Position) => invoke<void>("show_overlay", { pos }),
  hideOverlay: () => invoke<void>("hide_overlay", {}),
  dbStats: () => invoke<Record<string, number>>("db_stats", {}),
  listProviders: () => invoke<ProviderConfig[]>("list_providers", {}),
  getProvider: (id: string) => invoke<ProviderConfig | null>("get_provider", { id }),
  saveProvider: (p: ProviderConfig) => invoke<void>("save_provider", { provider: p }),
  deleteProvider: (id: string) => invoke<void>("delete_provider", { id }),
  setProviderEnabled: (id: string, enabled: boolean) => invoke<void>("set_provider_enabled", { id, enabled }),
  getMeta: (key: string) => invoke<string | null>("get_meta", { key }),
  setMeta: (key: string, value: string) => invoke<void>("set_meta", { key, value }),
  clearHistory: () => invoke<void>("clear_history", {}),
  setProviderKey: (id: string, key: string) => invoke<void>("set_provider_key", { id, key }),
  getOnboardingState: () => invoke<{ completed: boolean; has_enabled_provider: boolean }>("get_onboarding_state", {}),
  completeOnboarding: (provider_id: string | null, model: string | null, skipped: boolean) => invoke<void>("complete_onboarding", { providerId: provider_id, model, skipped }),
};

// ─── Event helpers ───────────────────────────────────────────────────────

export type OptChunkEvent = { text: string; session_id: string };
export type OptDoneEvent = OptimizeResult;
export type OptErrorEvent = { code: string; message: string; session_id: string };

export function onOptChunk(
  cb: (e: OptChunkEvent) => void,
): Promise<UnlistenFn> {
  return listen<OptChunkEvent>("opt_chunk", (evt) => cb(evt.payload));
}

export function onOptDone(
  cb: (e: OptDoneEvent) => void,
): Promise<UnlistenFn> {
  return listen<OptDoneEvent>("opt_done", (evt) => cb(evt.payload));
}

export function onOptError(
  cb: (e: OptErrorEvent) => void,
): Promise<UnlistenFn> {
  return listen<OptErrorEvent>("opt_error", (evt) => cb(evt.payload));
}

export type OverlayShowEvent = { text: string };

export function onOverlayShow(
  cb: (e: OverlayShowEvent) => void,
): Promise<UnlistenFn> {
  return listen<OverlayShowEvent>("overlay_show", (evt) => cb(evt.payload));
}

export type ProviderStatusEvent = { provider: string; alive: boolean };

export function onProviderStatus(
  cb: (e: ProviderStatusEvent) => void,
): Promise<UnlistenFn> {
  return listen<ProviderStatusEvent>("provider_status", (evt) => cb(evt.payload));
}

export type HotkeyErrorEvent = { shortcut: string; message: string };

export function onHotkeyError(
  cb: (e: HotkeyErrorEvent) => void,
): Promise<UnlistenFn> {
  return listen<HotkeyErrorEvent>("hotkey_error", (evt) => cb(evt.payload));
}
