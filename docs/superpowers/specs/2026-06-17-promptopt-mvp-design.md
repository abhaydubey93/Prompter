# PromptOpt Overlay — Core MVP (Windows) Design

| Field | Value |
|-------|-------|
| **Doc ID** | SPEC-001 |
| **Version** | 1.0 |
| **Date** | 2026-06-17 |
| **Status** | Approved |
| **Source docs** | `prd_product_requirements.md`, `technical_specification.md`, `use_case_definition.md`, `design_docs/01–10` |

---

## 1. Goal

Produce a **runnable, end-to-end PromptOpt Overlay on Windows** that fulfills the
core use case (UC-PO-001 §9 Main Success Scenario) for a single local provider
(Ollama) and two frameworks (CREATE, APE). Mac/Linux and cloud providers are
out of scope for this pass and are stubbed behind traits.

**Definition of done** (smoke test): launch app → press `Ctrl+Shift+E` in any
text field → overlay appears → Optimize streams a result from Ollama → Accept
replaces the original text in place → optimization is logged to SQLite.

## 2. Scope

### In scope
- Tauri 2 + React 18/TS + Tailwind scaffold.
- SQLite via `rusqlite` with schema from `04_Database_Design.md`
  (`prompts`, `context_profiles`, `app_profiles`, `history`).
- `LLMAdapter` trait + Ollama adapter (`/api/chat`, SSE streaming).
- OpenAI adapter stubbed behind trait (compiles, not wired to UI).
- Optimization engine: `minijinja` template rendering, score heuristic,
  diff (`similar` crate).
- Framework packs: CREATE, APE (JSON + Jinja template).
- Windows UIAutomation capture + in-place replace via `uiautomation` crate,
  behind `IAccessibilityService` trait.
- Clipboard fallback (`tauri-plugin-clipboard-manager` + synthetic paste).
- Global hotkey `Ctrl+Shift+E` via `tauri-plugin-global-shortcut`.
- Overlay window: non-activating, always-on-top, caret/mouse-anchored,
  edge-aware positioning.
- Tauri IPC commands: `capture_text`, `optimize_prompt`, `accept_replacement`,
  `get_models`, `save_prompt`, `get_settings`, `list_prompts`, `list_history`.
- React UI: OverlayContainer (raw/optimized split panes, framework selector,
  model selector, score badge, diff toggle, actions), Settings window,
  Prompt Library list.
- Streaming via Tauri events `opt_chunk` / `opt_done` / `opt_error`.
- Error envelope codes: `PROVIDER_UNREACHABLE`, `REPLACEMENT_FAILED`,
  `PERMISSION_DENIED`, `PII_BLOCKED` (defined, not enforced).

### Out of scope (this pass)
- macOS AXUIElement, Linux AT-SPI (trait + no-op stubs only).
- Cloud provider adapters (OpenAI/Anthropic/OpenRouter/Gemini/…) — trait only.
- API-key vault / `keyring` (no cloud keys needed yet).
- Arena mode, PII blocklist enforcement, telemetry toggle UI, auto-update,
  code signing/notarization, app-profile registry beyond defaults.
- Context Genie UI (table exists; UI deferred).
- FTS5 search (plain `LIKE` filter for library).
- Auto-save winners, few-shot injection, per-app overrides UI.

## 3. Architecture (this pass)

```
┌──────────────────────────────────────────────────────────┐
│ React UI (WebView)                                        │
│  OverlayContainer ──┐                                      │
│   raw pane │ optimized pane (streaming)                   │
│   framework select │ model select │ score │ diff │ actions│
│  SettingsWindow │ PromptLibrary                            │
└───────────────▲───────────────────────────┬───────────────┘
                │ Tauri IPC (invoke/emit)   │
┌───────────────┴───────────────────────────▼───────────────┐
│ Rust Core                                                 │
│  commands.rs  ──►  OptimizationEngine ──► ProviderRouter   │
│  capture_text ──►  AccessibilityService (Win UIA)         │
│  accept_repl  ──►  ReplacementService (UIA→clipboard)      │
│  hotkey.rs    ──►  show overlay (overlay.rs)              │
│  DbService ◄──► SQLite (data.db)                          │
└──────────────────────────────────────────────────────────┘
```

### 3.1 Module map

| Module | Responsibility |
|---|---|
| `main.rs` | `tauri::Builder`, plugin registration, command handlers, state. |
| `commands.rs` | `#[tauri::command]` functions callable from frontend. |
| `db/mod.rs` | `DbService`: open, migrate, CRUD for all 4 tables. |
| `providers/mod.rs` | `LLMAdapter` trait, `ChatStream`, `Message`, `ChatParams`. |
| `providers/ollama.rs` | Ollama `/api/chat` + `/api/tags` via `reqwest`/`tokio`. |
| `providers/openai.rs` | Trait stub (returns `Unimplemented`). |
| `engine/mod.rs` | `OptimizationEngine`: render → route → stream → score → diff. |
| `engine/frameworks.rs` | Load JSON packs from `framework_packs/`, register with minijinja. |
| `accessibility/mod.rs` | `IAccessibilityService` trait. |
| `accessibility/win.rs` | UIAutomation impl: `get_active_element_text`, `set_element_text`. |
| `accessibility/stub.rs` | No-op impl for non-Windows build. |
| `hotkey.rs` | Register `Ctrl+Shift+E`, dispatch to overlay show. |
| `overlay.rs` | Window show/hide, caret/mouse position, edge-aware placement. |
| `replacement.rs` | `ReplacementService`: UIA→verify→clipboard fallback pipeline. |

### 3.2 Traits

```rust
#[async_trait]
pub trait LLMAdapter: Send + Sync {
    fn id(&self) -> &str;
    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError>;
    async fn stream_chat(&self, messages: Vec<Message>, params: ChatParams)
        -> Result<Pin<Box<dyn Stream<Item = Result<String, ProviderError>> + Send>>, ProviderError>;
    fn health_check(&self) -> bool;
}

pub trait IAccessibilityService: Send + Sync {
    fn get_active_element_text(&self) -> Result<String, AccessError>;
    fn get_caret_position(&self) -> Result<Position, AccessError>;
    fn set_element_text(&self, text: &str) -> Result<(), AccessError>;
}
```

## 4. Data model

DDL taken verbatim from `design_docs/04_Database_Design.md` (§3), with one
addition: a `settings` key/value table for app config (hotkey, theme, default
framework, default model) since the class diagram references a `Settings` model.

```sql
CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
```

Seed defaults on first run:
`hotkey='Ctrl+Shift+E'`, `theme='dark'`, `default_framework='CREATE'`,
`default_model='ollama:llama3'`, `ollama_url='http://localhost:11434'`.

## 5. Optimization flow

1. UI sends `optimize_prompt { raw, framework, model, context_id? }`.
2. Engine loads framework template by id from `framework_packs/<id>.json`.
3. Renders with minijinja: `{ raw_prompt, context_profile, role, audience, tone }`.
4. Builds `messages = [system(rendered_template), user(raw)]`.
5. Calls `provider.stream_chat(messages, params)`; forwards each chunk via
   `app.emit("opt_chunk", { text })`.
6. Accumulates full output. On stream end:
   - Heuristic score 0–100 (length band, presence of structure markers like
     `#` headings, action verbs, quantified constraints).
   - Diff via `similar` crate, rendered as unified diff string.
   - Emits `opt_done { optimized, score, diff, tokens }`.
7. Inserts a row into `history` regardless of accept.

## 6. Replacement pipeline (Windows)

Per `02_High_Level_Design.md` §3.2 and `03_Low_Level_Design.md` §1.2:

```
accept_replacement(text):
  1. target = accessibility.focused_element()
  2. try accessibility.set_element_text(text):
       verify by re-reading; if matches → return {success:true, fallback:false}
  3. catch → fallback_clipboard(text):
       backup = clipboard.get()
       clipboard.set(text)
       keyboard.simulate_paste()      // enigo Ctrl+V
       sleep(60ms)
       clipboard.set(backup)
       return {success:true, fallback:true}
```

`enigo` crate used for synthetic paste. Web SPAs that reject `setValue` fall
through to clipboard path automatically.

## 7. Overlay window

- Created once at app start (label `overlay`), hidden.
- `always_on_top: true`, `decorations: false`, `focus: false` (non-activating),
  `resizable: true`, default `400x340`.
- On hotkey: capture caret pos (UIA) or fallback to mouse pos → compute
  edge-aware rect (flip above caret if near bottom edge, shift left if near
  right) → `set_position` + `show` → emit `overlay_show` with raw text.
- `Esc` → hide, restore focus to prior target.
- Streaming text updates optimized pane; `Enter` = Accept.

## 8. Framework packs (data, not code)

`framework_packs/create.json`:
```json
{
  "id": "CREATE",
  "name": "CREATE",
  "variables": ["context","role","task","explanation","constraints","tone","extras"],
  "template": "You are a prompt engineer. Rewrite the user's raw prompt in  detail using the CREATE framework.\n\nCREATE = Context, Request, Explanation, Action, Tone, Extras.\nReturn ONLY the rewritten prompt.\n\nRaw prompt:\n{{ raw_prompt }}\n{% if context %}Context: {{ context }}{% endif %}\n{% if tone %}Tone: {{ tone }}{% endif %}"
}
```

`framework_packs/ape.json`:
```json
{
  "id": "APE",
  "name": "APE (Action, Purpose, Expectation)",
  "variables": ["action","purpose","expectation"],
  "template": "You are a prompt engineer. Rewrite the user's raw prompt in  detail using the APE framework (Action, Purpose, Expectation).\nReturn ONLY the rewritten prompt.\n\nRaw prompt:\n{{ raw_prompt }}"
}
```

`raw_prompt` always provided; optional vars come from `context_profile` if present.

## 9. Settings model

```rust
pub struct Settings {
    pub hotkey: String,
    pub theme: String,            // "dark" | "light" | "system"
    pub default_framework: String,
    pub default_model: String,    // "provider:model" form
    pub ollama_url: String,
}
```

## 10. Testing strategy (this pass)

- **Unit (Rust):** template render, score heuristic, diff, DbService CRUD
  against temp-file SQLite, framework pack loading.
- **Integration:** Ollama adapter against a fake SSE stream (mock HTTP via
  `wiremock` or recorded fixture) — optional if time-boxed.
- **Manual smoke (this pass's "done" gate):** the flow in §1.
- Full matrix/E2E/CI deferred (see `design_docs/08_Test_Strategy.md`).

## 11. Risks & mitigations

| Risk | Mitigation |
|---|---|
| UIA `setValue` fails on web SPAs | Clipboard fallback (§6). |
| Ollama not running | `health_check` on app start + provider dropdown marks dead; `opt_error` with `PROVIDER_UNREACHABLE`. |
| Overlay steals focus → replacement fails | Non-activating window (`focus:false`); restore focus before replace. |
| Hotkey conflict | `tauri-plugin-global-shortcut` returns error on conflict → toast. |
| Streaming chunks race with UI teardown | Per-session id; UI ignores `opt_chunk` after dismiss. |

## 12. Open questions deferred to later passes

1. Bundled fallback local model for zero-config offline? (Open Q in use_case §19)
2. Cloud key vault UX (out of scope; needed before cloud providers land).
3. Arena mode data model (no table yet).
4. App-profile registry population strategy.

## 13. Next step

After this spec: invoke `writing-plans` to produce an ordered implementation
plan, then `subagent-driven-development` / direct execution.
