# PromptOpt Overlay

> Local-first prompt optimization overlay. Press a hotkey anywhere → capture text → optimize via local LLM → replace in place.

**Status:** Core MVP (Windows) — v0.1.0
**Spec:** [`docs/superpowers/specs/2026-06-17-promptopt-mvp-design.md`](docs/superpowers/specs/2026-06-17-promptopt-mvp-design.md) (SPEC-001, v1.0, Approved)

---

## What it does

PromptOpt sits in the background. Press `Ctrl+Shift+E` while focused on any text field. A non-activating overlay appears anchored near your caret, showing your raw prompt and a streaming optimized version powered by a local LLM. Accept the result and it replaces the original text in place — no copy-paste, no context switch.

The core use case (UC-PO-001): **write a rough prompt → hotkey → get a structured, specific, optimized prompt in the same field.**

```
launch app ─► press Ctrl+Shift+E ─► overlay appears ─► Optimize streams from Ollama
          ─► Accept replaces text in place ─► logged to SQLite
```

---

## Features (MVP scope)

### End-to-end flow
- Global hotkey (`Ctrl+Shift+E`) via `tauri-plugin-global-shortcut`
- Windows UIAutomation text capture (focused element value) via the `uiautomation` crate
- Streaming optimization with live token chunks (`opt_chunk` → `opt_done`)
- In-place replacement: accessibility `setValue` first, clipboard + synthetic `Ctrl+V` fallback
- Every optimization logged to SQLite history regardless of accept

### Optimization engine
- Framework-as-data: Jinja templates loaded from `framework_packs/*.json`
- Ships with two frameworks: **CREATE** and **APE**
- Minijinja rendering with variables from the optional context profile
- Heuristic quality score (0–100): length band, structure markers, specificity signals, repetition penalty
- Unified diff between raw and optimized (`similar` crate)

### Providers
- **Ollama** adapter (default): `/api/chat` NDJSON streaming, `/api/tags` model listing
- **OpenAI** adapter: trait stub (compiles, not wired to UI — cloud key vault deferred)

### UI
- **Overlay window** (400×340, borderless, always-on-top, non-activating): raw/optimized split panes, framework selector, model selector, score badge, diff toggle, Save / Copy / Accept actions
- **Main window**: Prompt Library (search + delete), History (last 50), Settings (all 5 keys editable + keyboard reference)
- Keyboard shortcuts: `Esc` close, `Enter` accept

### Persistence (SQLite)
Five tables (4 from the design spec + a settings key/value table):
- `prompts` — saved library entries (soft-delete, indexed by framework + score)
- `context_profiles` — role/audience/tone/style snippets for template variables
- `app_profiles` — per-app replacement strategy (`Accessibility` / `Clipboard` / `SyntheticKeys`)
- `history` — every optimization, raw + optimized + model + score
- `settings` — key/value app config

Location: `%APPDATA%\PromptOpt\data.db`

---

## Tech stack

| Layer | Technology |
|---|---|
| Shell | **Tauri 2** (Rust backend + WebView frontend) |
| Backend | **Rust** (edition 2021) |
| Frontend | **React 19** + **TypeScript 5.8** + **Vite 7** (spec §2 says React 18; 19 is superset-compatible) |
| Styling | **Tailwind CSS 3.4** (dark theme, custom palette) |
| Database | **SQLite** via `rusqlite` (bundled) |
| Templates | **Minijinja 2** |
| Diffing | **similar 2** |
| LLM transport | **reqwest** (rustls-tls, streaming) |
| Windows capture | **uiautomation 0.16** (UIValuePattern get/set) |
| Synthetic input | **enigo 0.6** (Ctrl+V paste simulation) |
| Hotkeys | **tauri-plugin-global-shortcut 2** |
| Clipboard | **tauri-plugin-clipboard-manager 2** |

---

## Project structure

```
PromptForge/
├── docs/superpowers/specs/         # Source spec (SPEC-001)
├── framework_packs/                # Jinja template packs (dev copy)
│   ├── create.json
│   └── ape.json
├── src/                            # React frontend
│   ├── main.tsx                    # Hash router: #/overlay → OverlayApp, else App
│   ├── App.tsx                     # Main window (Library / History / Settings)
│   ├── overlay/
│   │   └── OverlayApp.tsx          # Overlay window (capture/optimize/accept)
│   ├── lib/
│   │   └── tauri.ts                # Typed invoke wrappers + event listeners
│   └── index.css                   # Tailwind + scrollbar/diff styles
├── src-tauri/                      # Rust backend
│   ├── Cargo.toml
│   ├── tauri.conf.json             # Two windows (main + overlay), capabilities
│   ├── capabilities/default.json   # Permissions for both windows
│   ├── framework_packs/            # Template packs shipped with the binary
│   └── src/
│       ├── main.rs / lib.rs        # App entry, plugin/state/setup
│       ├── types.rs                # All IPC types + ApiError envelope
│       ├── commands.rs             # 17 #[tauri::command] handlers
│       ├── db/mod.rs               # DbService: migrate + CRUD for 5 tables
│       ├── providers/
│       │   ├── mod.rs              # LLMAdapter trait + ProviderError + factory
│       │   ├── ollama.rs           # NDJSON /api/chat + /api/tags
│       │   └── openai.rs           # Stub (returns Unimplemented)
│       ├── engine/mod.rs           # OptimizationEngine: render→stream→score→diff
│       ├── accessibility/
│       │   ├── mod.rs              # IAccessibilityService trait + AccessError
│       │   └── platform.rs         # Windows UIAutomation impl + non-Windows stub
│       ├── replacement.rs          # UIA→verify→clipboard+enigo pipeline
│       ├── hotkey.rs               # Global shortcut → capture → overlay_show
│       └── overlay.rs              # Show/hide, edge-aware positioning
├── index.html
├── package.json
├── tailwind.config.js
├── tsconfig.json
└── vite.config.ts
```

---

## Architecture

```
┌──────────────────────────────────────────────────────────┐
│ React UI (WebView)                                        │
│  OverlayApp ───┐                                           │
│   raw pane │ optimized pane (streaming)                   │
│   framework select │ model select │ score │ diff │ actions│
│  App (Library / History / Settings)                       │
└───────────────▲───────────────────────────┬───────────────┘
                │ Tauri IPC (invoke/emit)   │
┌───────────────┴───────────────────────────▼───────────────┐
│ Rust Core                                                 │
│  commands.rs ──► OptimizationEngine ──► ProviderRouter     │
│  capture_text ──► AccessibilityService (Win UIA)          │
│  accept_repl  ──► ReplacementService (UIA→clipboard→enigo) │
│  hotkey.rs    ──► overlay.rs (caret-aware positioning)    │
│  DbService ◄──► SQLite (data.db)                          │
└──────────────────────────────────────────────────────────┘
```

### Tauri IPC

**Commands (frontend → backend):**

| Command | Purpose |
|---|---|
| `capture_text` | Read focused element text + caret position |
| `optimize_prompt` | Render template → stream from provider → emit events |
| `accept_replacement` | UIA `setValue` → verify → clipboard+`Ctrl+V` fallback |
| `get_models` | List models for a provider |
| `save_prompt` / `list_prompts` / `search_prompts` / `delete_prompt` | Library CRUD |
| `save_context` / `list_contexts` | Context profiles |
| `list_history` | Recent optimizations |
| `get_settings` / `set_setting` | Settings key/value |
| `list_frameworks` | Available framework packs |
| `show_overlay` / `hide_overlay` | Window control |
| `db_stats` | Row counts per table (diagnostics) |

**Events (backend → frontend):**

| Event | Payload | When |
|---|---|---|
| `overlay_show` | `{ text, position }` | Hotkey fired; overlay receives captured text |
| `opt_chunk` | `{ text, session_id }` | Each streamed token chunk |
| `opt_done` | `{ optimized, score, diff, tokens, session_id }` | Stream complete |
| `opt_error` | `{ code, message, session_id }` | Stream failure |
| `provider_status` | `{ provider, alive }` | Startup health check result |

### Error envelope

Normalized `ApiError` codes surfaced to the UI:

| Code | Meaning |
|---|---|
| `PROVIDER_UNREACHABLE` | LLM endpoint not reachable (e.g. Ollama not running) |
| `REPLACEMENT_FAILED` | Both accessibility and clipboard replacement failed |
| `PERMISSION_DENIED` | OS accessibility permission missing |
| `PII_BLOCKED` | Defined, not enforced (deferred) |
| `INTERNAL` | Catch-all backend error |

---

## Getting started

### Prerequisites

- **Windows 10/11** (x64) — MVP is Windows-only; macOS/Linux are stubbed behind traits
- [Node.js](https://nodejs.org/) 18+ and npm
- [Rust](https://www.rust-lang.org/tools/install) (stable) with `cargo`
- [Ollama](https://ollama.com/) installed and running (`ollama serve`)
- At least one Ollama model pulled, e.g. `ollama pull llama3`
- WebView2 runtime (preinstalled on Windows 11; bundled in the installers)

### Development

```bash
# 1. Install frontend deps
npm install

# 2. Run in dev mode (Vite + Tauri, hot reload)
npm run tauri dev
```

The dev server starts on `http://localhost:1420`. The first Rust build will take several minutes; subsequent builds are incremental.

### Production build

```bash
npm run tauri build
```

Artifacts land in `src-tauri/target/release/bundle/`:

| File | Type |
|---|---|
| `msi/PromptOpt_0.1.0_x64_en-US.msi` | Windows MSI installer |
| `nsis/PromptOpt_0.1.0_x64-setup.exe` | NSIS installer |
| `promptopt.exe` | Standalone executable |

### Type checking

```bash
npx tsc --noEmit          # frontend
cd src-tauri && cargo check   # backend
```

---

## Configuration

All settings live in the SQLite `settings` table and are editable from the in-app Settings tab. Defaults seeded on first run:

| Key | Default | Notes |
|---|---|---|
| `hotkey` | `Ctrl+Shift+E` | Global shortcut; restart to apply |
| `theme` | `dark` | `dark` \| `light` \| `system` |
| `default_framework` | `CREATE` | Preselected framework in overlay |
| `default_model` | `ollama:llama3` | `provider:model` selector |
| `ollama_url` | `http://localhost:11434` | Ollama base URL |

---

## Framework packs

Frameworks are data, not code. Each pack is a JSON file with a Jinja template:

```json
{
  "id": "CREATE",
  "name": "CREATE",
  "variables": ["context","role","task","explanation","constraints","tone","extras"],
  "template": "You are a prompt engineer. Rewrite the user's raw prompt using the CREATE framework.\n\nCREATE = Context, Request, Explanation, Action, Tone, Extras.\nReturn ONLY the rewritten prompt.\n\nRaw prompt:\n{{ raw_prompt }}\n{% if context %}Context: {{ context }}{% endif %}\n{% if tone %}Tone: {{ tone }}{% endif %}"
}
```

- `raw_prompt` is always provided
- Optional variables (`context`, `role`, `tone`, `audience`, …) come from the context profile
- Packs are loaded from `<exe_dir>/framework_packs/` and `<app_data_dir>/framework_packs/`
- Hardcoded CREATE/APE fallbacks exist if no files are found on disk
- Add your own pack by dropping a `*.json` file in `framework_packs/` and restarting

---

## Keyboard shortcuts

| Shortcut | Action |
|---|---|
| `Ctrl+Shift+E` | Open overlay / capture focused text |
| `Enter` | Accept optimized text (replace in place) |
| `Esc` | Close overlay |

---

## How replacement works

When you click **Accept**, the `ReplacementService` runs this pipeline (spec §6):

1. **Accessibility `setValue`** — get the focused element's `UIValuePattern` and set its value
2. **Verify** — re-read the element; if it matches the new text, done (`fallback: false`)
3. **Clipboard fallback** — if `setValue` fails (common on web SPAs):
   - Back up the current clipboard
   - Write the optimized text to the clipboard
   - Simulate `Ctrl+V` via `enigo`
   - Restore the original clipboard after a short delay
   - Returns `fallback: true` so the UI can prompt the user if the paste didn't land

---

## Out of scope (this pass)

Deferred to later passes per spec §2:

- macOS AXUIElement / Linux AT-SPI accessibility (trait + no-op stubs only)
- Cloud provider adapters (OpenAI/Anthropic/Gemini/…) — trait surface only
- API-key vault via `keyring`
- Arena mode, PII blocklist enforcement, telemetry toggle UI
- Auto-update, code signing, notarization
- Context Genie UI (table exists; UI deferred)
- FTS5 search (plain `LIKE` filter for now)
- Auto-save winners, few-shot injection, per-app overrides UI

---

## Troubleshooting

| Symptom | Fix |
|---|---|
| Overlay doesn't appear on hotkey | Another app owns `Ctrl+Shift+E`; change it in Settings → Hotkey and restart |
| Models dropdown empty | Ollama not running — start with `ollama serve` and `ollama pull llama3` |
| Replacement doesn't land in web apps | Expected on some SPAs; the clipboard fallback fires — press `Ctrl+V` manually if prompted |
| `PROVIDER_UNREACHABLE` error | Check `ollama_url` in Settings; verify `http://localhost:11434` responds |
| Build slow first time | Release LTO over Tauri + uiautomation is heavy (~1–10 min); incremental builds are fast |

---

## License

Proprietary — PromptForge project.

## Acknowledgements

Built against the approved MVP design spec (`SPEC-001`). Powered by Tauri, Rust, React, and Ollama.
