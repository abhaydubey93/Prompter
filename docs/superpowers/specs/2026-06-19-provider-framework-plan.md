# Round 3 Plan — Multi-Provider, Framework Library, Onboarding, Settings Overhaul

**Plan ID:** PLAN-R3
**Date:** 2026-06-19
**Author:** caveman-mode audit
**Source docs:** `use_case_definition.md` (§13, §15, FR-L1..L8, FR-E1..E2, FR-S1), `prd_product_requirements.md`, `technical_specification.md`
**Predecessor:** `gaps_to_be_fixed.md` (round 1+2 closed; this round is the multi-provider story that the MVP spec deliberately stubbed)

---

## 0. Decisions (locked with user)

| Question | Decision |
|---|---|
| API-key storage | OS keychain via `keyring` crate (FR-S1) |
| First-run onboarding | Skippable wizard, persistent re-prompt until a provider health-checks |
| Provider coverage | Full §13 matrix: Ollama (native) + OpenAI-compatible adapter (covers OpenAI, LM Studio, llama.cpp, OpenRouter, NVIDIA NIM, Mistral, Groq, Together, Custom) + Anthropic (native) + Gemini (native) |

---

## 1. Problem statement

Round 1+2 closed every gap in the Core MVP spec (single local provider, 2 frameworks). The **use-case + PRD + technical spec** explicitly call for the broader product, and the user now wants the missing surface:

1. **No provider choice** — UI shows only `ollama:llama3`; everything funnels through one `ollama_url`. `providers/build()` is a 2-arm match.
2. **No framework library** — only CREATE + APE exist; "Default Framework" is free-text, not a dropdown; `framework_packs/` isn't bundled so on-disk packs are invisible at runtime; no import/custom-pack UI.
3. **No first-run setup** — app boots into hardcoded Ollama defaults with no onboarding.
4. **No real customization** — Settings is one page of text inputs; per-provider endpoint/key/timeout/max-tokens/temperature (FR-L6) absent; Settings tabs (§15) absent.
5. **Latent bugs** — Enter-in-raw-textarea misfires Accept; theme stored but never applied; `increment_usage` dead; model dropdown hardcoded options.

---

## 2. Work packages

Grouped so each WP compiles + tests independently. Execute top-to-bottom. Every WP ends with `cargo check` (or `tsc --noEmit`) green before the next.

### WP-A — Provider abstraction rewrite (HIGH, M)
**Closes:** FR-L1, FR-L2, FR-L3, FR-L5, FR-L6, FR-L7, use-case §13.

**Why first:** every other WP consumes the new provider registry. Get the trait + registry right, the rest falls into place.

**Changes:**

- **`src-tauri/Cargo.toml`** — add `keyring = "3"`.
- **`src-tauri/src/providers/mod.rs`** — replace the hardcoded `build()` with a **`ProviderRegistry`** (managed state) holding `Vec<ProviderConfig>` loaded from DB. Add:
  ```rust
  pub struct ProviderConfig {
      pub id: String,            // stable key, e.g. "openai", "my-local"
      pub kind: ProviderKind,    // Ollama | OpenAiCompat | Anthropic | Gemini
      pub label: String,         // user-facing
      pub base_url: String,
      pub api_key_slot: Option<String>,  // keyring service name; None = no auth
      pub default_model: String,
      pub enabled: bool,
  }
  pub enum ProviderKind { Ollama, OpenAiCompat, Anthropic, Gemini }
  ```
  Registry methods: `list()`, `get(id)`, `add(cfg)`, `update(id, cfg)`, `remove(id)`, `build_adapter(cfg) -> Box<dyn LLMAdapter>`, `health_check(id) -> bool` (async, per-provider).
- **`providers/ollama.rs`** — keep as-is, take `base_url` from config (already does).
- **`providers/openai_compat.rs`** — **NEW**, full implementation of `LLMAdapter`:
  - `list_models()` → `GET {base}/v1/models`, Bearer key if slot set.
  - `stream_chat()` → `POST {base}/v1/chat/completions` with `stream:true`, parse `data: {...}` SSE lines, yield `choices[0].delta.content`.
  - `health_check()` → `GET {base}/v1/models` 200.
  - Replace the stub `openai.rs`; keep file but have it re-export / delegate to `openai_compat` so the module map in spec §3.1 stays valid.
- **`providers/anthropic.rs`** — **NEW**, native `/v1/messages`:
  - Headers `x-api-key`, `anthropic-version: 2023-06-01`.
  - `list_models()` → hardcoded family list (Anthropic has no list endpoint) — Claude 3.5/4 Sonnet/Haiku/Opus.
  - `stream_chat()` → `POST {base}/v1/messages` `stream:true`, SSE `content_block_delta` events.
- **`providers/gemini.rs`** — **NEW**, native Generate Content:
  - `list_models()` → `GET {base}/v1beta/models?key=`.
  - `stream_chat()` → `POST {base}/v1beta/models/{model}:streamGenerateContent?alt=sse&key=`, parse `candidates[0].content.parts[0].text`.
- **`providers/keys.rs`** — **NEW**, thin wrapper over `keyring`:
  - `get(slot) -> Option<String>`, `set(slot, val)`, `delete(slot)`. Service name = `promptopt.<provider_id>`, account = `api_key`.
- **Delete** the old `build(selector, ollama_url)` function; update callers (`commands::get_models`, `engine::optimize`) to use the registry.

**Tests:** unit-test `ProviderKind` parsing from JSON, keyring set/get roundtrip against a temp entry (keyring Entry API is sync, no network), OpenAI-compat SSE line parser with a fixture string.

---

### WP-B — DB schema for providers + per-provider settings (HIGH, S)
**Closes:** FR-L6, enables WP-A/WP-C.

**Changes (`src-tauri/src/db/mod.rs`):**

- Add migration (idempotent `CREATE TABLE IF NOT EXISTS`):
  ```sql
  CREATE TABLE IF NOT EXISTS providers (
      id            TEXT PRIMARY KEY,
      kind          TEXT NOT NULL,           -- ollama | openai_compat | anthropic | gemini
      label         TEXT NOT NULL,
      base_url      TEXT NOT NULL,
      api_key_slot  TEXT,                    -- NULL = no auth
      default_model TEXT NOT NULL DEFAULT '',
      enabled       INTEGER NOT NULL DEFAULT 1,
      sort_order    INTEGER NOT NULL DEFAULT 0,
      created_at    TEXT NOT NULL DEFAULT (datetime('now'))
  );
  CREATE TABLE IF NOT EXISTS meta (
      key   TEXT PRIMARY KEY,
      value TEXT NOT NULL
  );   -- for onboarding_completed, telemetry toggle, etc.
  ```
- **Seed default providers** on first run (only if table empty) so the app boots with a usable registry:
  - `ollama` (Ollama, `http://localhost:11434`, no key)
  - `lmstudio` (OpenAiCompat, `http://localhost:1234`, no key)
  - `openai` (OpenAiCompat, `https://api.openai.com`, key slot, disabled until key set)
  - `anthropic` (Anthropic, `https://api.anthropic.com`, key slot, disabled)
  - `openrouter` (OpenAiCompat, `https://openrouter.ai/api`, key slot, disabled)
  - `nvidia_nim` (OpenAiCompat, `https://integrate.api.nvidia.com`, key slot, disabled)
  - `gemini` (Gemini, `https://generativelanguage.googleapis.com`, key slot, disabled)
  - `custom` placeholder disabled (user fills URL)
- CRUD: `list_providers()`, `get_provider(id)`, `save_provider(cfg)`, `delete_provider(id)`, `set_provider_enabled(id, bool)`, `reorder_providers(ids)`.
- `meta` get/set (`onboarding_completed`, `default_provider_id`, `default_model`, etc.).
- **Keep** the legacy `settings` table for hotkey/theme; do **not** delete `ollama_url` (back-compat for the old `Settings` struct — repoint `optimize_prompt` to use the registry's selected provider's `base_url` instead).

**Tests:** provider CRUD roundtrip, seed-on-empty, meta set/get.

---

### WP-C — Framework library expansion + bundling + import (HIGH, M)
**Closes:** FR-E1, FR-E2, framework dropdown bug.

**Changes:**

- **Add 8 framework packs** under `framework_packs/` (one JSON each): `tag.json`, `race.json`, `care.json`, `rise.json`, `era.json`, `trace.json`, `roses.json`, `spark.json`. Each follows the existing `{id,name,variables,template}` schema, references `{{ raw_prompt }}`, optionally `{{ role }} {{ tone }} {{ audience }} {{ context }}`.
- **Update `engine/frameworks.rs`** hardcoded fallbacks to include all 10 (so a missing-bundle build still has them) and document each framework's acronym in `name` (e.g. `"RACE (Role, Action, Context, Expectation)"`).
- **Bundle on-disk packs into the binary** — `src-tauri/tauri.conf.json`:
  ```json
  "bundle": { "resources": ["../framework_packs/*.json"], ... }
  ```
  And resolve the resource dir at runtime via `app.path().resource_dir()`; pass that into `OptimizationEngine::new()` so `load_all()` reads from the bundled location first, then app-data dir (user imports), then hardcoded fallbacks.
- **New IPC commands** (`commands.rs`):
  - `list_frameworks()` — already exists, now returns all 10.
  - `import_framework(pack: FrameworkPack)` — writes to `<app_data>/framework_packs/<id>.json`, reloads engine.
  - `delete_framework(id)` — removes user pack (refuse built-in ids).
- **Frontend**: change overlay framework `<select>` to render `frameworks` state (already does — bug was just that only 2 existed). Add Settings → Frameworks tab: list with edit/delete (built-ins read-only), "Import JSON" button.
- **Overlay Settings bug**: replace `SettingRow` for "Default Framework" with a `<select>` bound to `list_frameworks()` output. Same for "Default Model" → populate from `list_providers()`.

**Tests:** all 10 packs load, import→reload→delete roundtrip, built-in ids refused on delete.

---

### WP-D — Onboarding wizard (HIGH, M)
**Closes:** first-run setup requirement.

**Changes:**

- **`src/Onboarding.tsx`** — NEW full-screen modal rendered by `App.tsx` when `meta.onboarding_completed != "1"`. Steps:
  1. Welcome → "Choose your LLM provider" (list of seeded providers as cards, click to configure).
  2. For the chosen provider: show endpoint (editable), API key field (if `api_key_slot` set), "Test connection" button calling a new `test_provider(id)` IPC that runs `health_check` + `list_models` and shows pass/fail + model count.
  3. Pick default model from the discovered list.
  4. Done → sets `meta.onboarding_completed=1`, `default_provider_id`, `default_model`. "Skip" button sets `onboarding_completed=1` with defaults (so it won't nag the same session, but re-prompts next launch if no provider health-checks).
- **`lib.rs` startup** — after DB init, run a quick health sweep of **all enabled providers** (not just Ollama), emit a `provider_status` event per provider. Reuse existing `onProviderStatus` listener (extend payload to `{provider, alive, latency_ms?}`).
- **Re-prompt logic** — if no provider is alive and onboarding wasn't explicitly completed this session, the wizard re-opens on next launch (use a `meta.onboarding_completed=1` AND at-least-one-alive gate).
- **New IPC**: `get_onboarding_state() -> {completed, has_alive_provider}`, `complete_onboarding(provider_id, model, skipped)`, `test_provider(id) -> {alive, models, error?}`.

**Tests:** (frontend logic) unit-test the gate condition; (backend) `test_provider` against a mocked Ollama URL returns alive=false gracefully.

---

### WP-E — Settings overhaul: tabbed UI + provider manager (HIGH, M)
**Closes:** use-case §15, FR-L6, FR-C1..C3.

**Changes:**

- **`src/App.tsx` Settings tab** → replace single-page form with sub-tabs: **General | Providers | Frameworks | Context | Privacy**.
  - **General**: hotkey (existing), theme (existing — wire it up, see WP-F), overlay opacity slider, default framework `<select>`, default provider+model `<select>`.
  - **Providers**: list (label, kind, endpoint, enabled toggle, "Test", "Edit", "Delete", "Add custom"). Edit modal: endpoint, API key (masked, write-only — show "Set" not the value), default model, timeout, max_tokens, temperature. Backed by `list_providers`/`save_provider`.
  - **Frameworks**: WP-C UI.
  - **Context**: CRUD for context profiles (un-defer GAP-9) — name/role/audience/tone/style. Overlay gets a context `<select>` in the header.
  - **Privacy**: telemetry toggle (default off), "clear history" button, history-retention selector. Cloud-denylist toggle is stretch (note in README).
- **New IPC** needed by Settings: `update_provider`, `add_provider`, `test_provider` (shared with onboarding), `save_context`/`list_contexts` (exist), `clear_history`, list/set telemetry.
- **`Settings` Rust struct** — extend with `default_provider_id`, `overlay_opacity`, `telemetry_enabled`, `history_retention_days`. Update `get_settings`/`set_setting` accordingly.

**Tests:** frontend snapshot-free; backend `clear_history` empties the table.

---

### WP-F — Bug-fix sweep (MEDIUM, S each)

1. **Enter-in-raw-textarea misfire** (`OverlayApp.tsx:177-190`): scope the Enter→Accept handler to fire only when `document.activeElement` is NOT the raw textarea; use Shift+Enter for newline. Per spec §14.
2. **Theme never applied**: add a `data-theme` attribute on `#root` driven by `settings.theme`; extend `index.css` with light-theme tokens (mirror the existing `bg-900` etc. via CSS variables). Toggle persists via existing `set_setting("theme", …)`.
3. **`increment_usage` dead code**: call it from `save_prompt` when an id collides (re-save = reuse) OR add a `bump_usage(id)` IPC the library list calls on "copy". Pick: call on re-save (simplest).
4. **Model dropdown hardcoded `<option value="ollama:llama3">`** (`OverlayApp.tsx:221`): remove the hardcoded option, drive entirely from `models` state; fall back to `settings.default_model` when empty.
5. **`compute_position` hardcodes 1920×1080** (`overlay.rs:60`): use `window.current_monitor()` from Tauri to get real primary monitor size; falls back to defaults if None.
6. **`capture_text` returns caret pos that overlay ignores** — already noted (GAP-10); now that positioning is dynamic, pass real caret pos into `show_overlay` (already does in Rust; confirm no regression).
7. **Health sweep only-Ollama** — fixed in WP-D startup hook.

**Tests:** add a focused test for the Enter-key gate logic (pure function extracted), monitor-size clamp with a fake monitor.

---

### WP-G — Docs + verification (LOW, S)

- Update `README.md`: new provider matrix table, framework list, onboarding screenshot placeholder, keychain note, new Settings tabs.
- Update `gaps_to_be_fixed.md`: append "Round 3 — closed" section listing all WP-A..F items.
- Add a `docs/providers.md` quick-reference: how to add a custom OpenAI-compatible endpoint.
- Final gate: `cargo test` (all green, expect ~30+ tests now), `npx tsc --noEmit`, `npx tauri build` produces MSI/NSIS/EXE. Manual smoke: onboarding → pick Ollama → optimize → accept.

---

## 3. Dependency additions

| Crate | Version | Why |
|---|---|---|
| `keyring` | 3 | OS keychain for API keys (FR-S1) |

No frontend deps added (lucide-react already covers icons). No changes to existing crate versions.

---

## 4. New IPC surface (delta over current)

| Command | Purpose | WP |
|---|---|---|
| `list_providers` | registry listing | A/B/E |
| `save_provider` | add/update | B/E |
| `delete_provider` | remove | B/E |
| `set_provider_enabled` | toggle | E |
| `test_provider` | health + model list | D/E |
| `import_framework` | add user pack | C |
| `delete_framework` | remove user pack | C |
| `get_onboarding_state` | wizard gate | D |
| `complete_onboarding` | finish wizard | D |
| `clear_history` | privacy | E |
| `get_meta` / `set_meta` | generic kv | B/D/E |
| `bump_usage` | (or fold into save_prompt) | F |

---

## 5. Effort + order

| WP | Severity | Effort | Depends on |
|---|---|---|---|
| A — Provider rewrite | HIGH | M | — |
| B — DB schema | HIGH | S | A (types) |
| C — Framework library | HIGH | M | — (independent) |
| D — Onboarding | HIGH | M | A, B |
| E — Settings overhaul | HIGH | M | A, B, C |
| F — Bug sweep | MEDIUM | S×7 | mostly independent; F5 after A |
| G — Docs + verify | LOW | S | all |

**Recommended sequence:** C (parallel, no deps) → B → A → D → E → F → G. C and B can run concurrently since they touch different files.

---

## 6. Definition of done

1. User can launch fresh → onboarding wizard appears → pick any of Ollama / LM Studio / OpenAI / Anthropic / OpenRouter / NVIDIA NIM / Gemini / Custom → enter key → "Test connection" passes → set default model → done.
2. Overlay framework dropdown shows all 10 frameworks (APE/TAG/RACE/CARE/RISE/ERA/CREATE/TRACE/ROSES/SPARK); user can import a custom JSON pack from Settings.
3. Settings has 5 sub-tabs (General/Providers/Frameworks/Context/Privacy) with full CRUD.
4. API keys live in OS keychain, never plaintext in DB or logs.
5. No provider is hardcoded in `build()`; adding a provider = a DB row.
6. `framework_packs/*.json` bundled into the installed app (works post-build, not just `cargo run`).
7. All 7 bug-sweep items closed.
8. `cargo test` green (30+ tests), `tsc --noEmit` clean, `npx tauri build` produces installers.

---

## 7. Out of scope (explicitly deferred, document in README)

- Arena mode (multi-model parallel) — needs its own UI; §10 A5.
- Few-shot injection from library by tag (FR-E6).
- Routing rules "local for short, cloud for long" (FR-L8).
- Auto-update channel, code signing.
- Cloud denylist global toggle (FR-S4) — Privacy tab gets a placeholder.
- PII regex blocklist enforcement (FR-S3) — defined not enforced since MVP.
- Browser extension, mobile.
