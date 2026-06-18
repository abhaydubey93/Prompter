# Gaps To Be Fixed — Spec vs Implementation Audit

**Source of truth:** `docs/superpowers/specs/2026-06-17-promptopt-mvp-design.md` (SPEC-001 v1.0)
**Audit date:** 2026-06-18
**Auditor:** caveman-mode deep scan

Each gap cites the spec section, describes what code does today, and gives the fix. Sorted: **HIGH** first.

---

## HIGH severity

### GAP-1 — Module file split missing (spec §3.1)
**Spec says:**
| Module | Responsibility |
|---|---|
| `engine/frameworks.rs` | Load JSON packs from `framework_packs/`, register with minijinja |
| `accessibility/win.rs` | UIAutomation impl |
| `accessibility/stub.rs` | No-op impl for non-Windows build |

**Code today:** All three concerns collapsed into `engine/mod.rs` and `accessibility/platform.rs`. Functionally fine, structurally wrong vs spec.

**Fix:**
- Split `engine/mod.rs` → `engine/mod.rs` (engine logic) + `engine/frameworks.rs` (pack loading + hardcoded fallbacks).
- Split `accessibility/platform.rs` → `accessibility/win.rs` (`#[cfg(windows)]`) + `accessibility/stub.rs` (`#[cfg(not(windows))]`), both re-exported from `accessibility/mod.rs`.

---

### GAP-2 — `Esc` restores focus to MAIN window, not prior target (spec §7)
**Spec says:** "`Esc` → hide, restore focus to **prior target**."

**Code today:** `overlay::hide_overlay()` hides overlay then calls `main.set_focus()`. The text field that had focus before the overlay opened loses focus — breaks the next `accept_replacement` on web SPAs.

**Fix:** Track the previously-focused window handle in `hotkey.rs` before showing overlay. On hide, restore focus to that handle, not main.

---

### GAP-3 — Caret position hardcoded (spec §7)
**Spec says:** "On hotkey: capture caret pos (UIA) **or fallback to mouse pos** → compute edge-aware rect."

**Code today:** `WindowsAccessibilityService::get_caret_position()` returns constant `Position { x: 400.0, y: 300.0 }`. Comment admits "deferred post-MVP". Overlay always opens at (400, 300).

**Fix:** On Windows, fall back to `GetCursorPos` (Win32) when UIA caret is unavailable. `enigo`/`windows` crate or a tiny `winapi` call returns mouse coords. At minimum, use mouse position so overlay appears where the user is looking.

---

### GAP-4 — `provider_status` event emitted but never consumed (spec §11)
**Spec says:** "`health_check` on app start + provider dropdown marks dead; `opt_error` with `PROVIDER_UNREACHABLE`."

**Code today:** `lib.rs` setup hook spawns async `ollama_health()` and emits `provider_status { provider, alive }`. Frontend (`src/lib/tauri.ts`, `OverlayApp.tsx`) has **no listener** for `provider_status`. Dropdown never marks dead. Dead event.

**Fix:** Add `onProviderStatus` helper in `tauri.ts`. In `OverlayApp`, on `provider_status` with `alive=false`, show a warning banner ("Ollama not running") and visually mark the model selector red/disabled.

---

### GAP-5 — Hotkey change in Settings does not re-register (spec §11)
**Spec says:** Hotkey conflict / change should reflect. Settings has an editable `hotkey` field.

**Code today:** `hotkey::register()` runs once at startup reading the DB. When user edits `hotkey` in Settings UI and saves (`set_setting`), nothing re-registers the global shortcut. Old hotkey still active, new one ignored. Also no toast on conflict.

**Fix:** In `commands::set_setting`, when `key == "hotkey"`, after saving: unregister old shortcut, register new one, return error to UI on conflict so a toast shows.

---

## MEDIUM severity

### GAP-6 — Zero unit tests (spec §10)
**Spec says:**
> **Unit (Rust):** template render, score heuristic, diff, DbService CRUD against temp-file SQLite, framework pack loading.

**Code today:** No `#[cfg(test)]` blocks anywhere in `src-tauri/src/`. No `tests/` directory. Definition-of-done smoke test is manual only.

**Fix:** Add `#[cfg(test)] mod tests` in:
- `engine/mod.rs` — `test_score_prompt_*`, `test_compute_diff`, `test_render_template_*`
- `db/mod.rs` — `test_crud_prompts`, `test_history_insert`, `test_settings_roundtrip` (open temp-file DB)
- `engine/frameworks.rs` (after GAP-1 split) — `test_load_hardcoded_packs`

---

### GAP-7 — Toast on hotkey conflict missing (spec §11)
**Spec says:** "`tauri-plugin-global-shortcut` returns error on conflict → toast."

**Code today:** `hotkey::register()` returns `Err`, setup hook logs `warn!`. No UI surface — user never sees conflict. No toast system at all.

**Fix:** Either (a) emit a `hotkey_error` event that the main window listens to and shows a toast, or (b) have the `set_setting` command for hotkey return the conflict error to the invoke caller (covers GAP-5 too).

---

### GAP-8 — `capture_text` IPC command is dead (spec §2)
**Spec lists** `capture_text` as a required IPC command callable from frontend.

**Code today:** `commands::capture_text` exists and is registered. Frontend `cmd.captureText()` wrapper exists in `tauri.ts`. **Nobody calls it** — the hotkey handler in Rust captures directly and emits `overlay_show`. The IPC command is orphaned.

**Fix:** Either (a) delete `capture_text` from spec's required list (spec drift) and remove the dead command + TS wrapper, or (b) call `cmd.captureText()` from `OverlayApp` on mount as a fallback when `overlay_show.text` is empty (defensive capture for when the user opens the overlay without the hotkey, e.g. via tray).

Recommendation: keep the command, call it as fallback in `OverlayApp` init when `overlay_show` delivers no text.

---

### GAP-9 — `save_context` / `list_contexts` IPC commands unused by UI (spec §2)
**Spec lists** context profiles as part of the data model and `optimize_prompt` accepts a `context_id?`.

**Code today:** DB table `context_profiles` exists, CRUD methods exist, IPC commands exist. **Frontend has no UI to create/select a context profile.** `OverlayApp` never passes `context_id` in `optimizePrompt()`. Template variables `context/role/tone/audience` are always empty strings.

**Fix:** Spec §2 explicitly says "Context Genie UI (table exists; UI deferred)" — so this is **declared deferred, not a true gap**. Lower priority. Document as intentional in README. No code change required for MVP.

---

### GAP-10 — Overlay `position` from `overlay_show` event unused in frontend
**Code today:** `hotkey.rs` emits `{ text, position }` in `overlay_show`. `OverlayApp.onOverlayShow` reads `ev.text` but ignores `ev.position`. Window position is already set by `overlay::show_overlay` in Rust, so this is harmless redundancy, but the payload is asymmetric vs consumer.

**Fix:** Either drop `position` from the event payload (Rust already positions the window) or log it for debugging. Trivial.

---

## LOW severity / cosmetic

### GAP-11 — App identifier vs productName casing
**Code today:** `tauri.conf.json` has `identifier: "com.promptopt.overlay"`, `productName: "PromptOpt"`. Spec references `PromptOpt`. Fine, but `data.db` lands in `%APPDATA%\PromptOpt\data.db` while identifier uses lowercase. No conflict, just noting.

**Fix:** None needed. Document only.

### GAP-12 — `README.md` claims React 19, spec §2 says React 18
**Spec §2:** "Tauri 2 + **React 18**/TS + Tailwind scaffold."
**package.json:** `"react": "^19.1.0"`.

**Fix:** Either downgrade to React 18, or note in README that the scaffold shipped React 19 (superset-compatible). Cosmetic.

### GAP-13 — `increment_usage` exists in DbService but never called
**Code today:** `db::increment_usage(id)` defined, never invoked anywhere. Library prompts always show `usage_count: 0` (set to 1 on save in OverlayApp, but never incremented on reuse).

**Fix:** Call `db.increment_usage(id)` when a prompt is reused from the library (no library-reuse UI exists yet — deferred). Or remove the dead method. Low priority.

---

## Summary table

| ID | Severity | Spec § | Effort | Status |
|---|---|---|---|---|
| GAP-1 | HIGH | §3.1 | S (refactor) | pending |
| GAP-2 | HIGH | §7 | S | pending |
| GAP-3 | HIGH | §7 | M | pending |
| GAP-4 | HIGH | §11 | S | pending |
| GAP-5 | HIGH | §11 | S | pending |
| GAP-6 | MEDIUM | §10 | M | pending |
| GAP-7 | MEDIUM | §11 | S | pending |
| GAP-8 | MEDIUM | §2 | XS | pending |
| GAP-9 | MEDIUM (deferred) | §2 | — | intentional |
| GAP-10 | MEDIUM | §7 | XS | pending |
| GAP-11 | LOW | — | — | no-op |
| GAP-12 | LOW | §2 | XS | pending |
| GAP-13 | LOW | — | XS | pending |

---

## Recommended execution order

1. **GAP-4** (wire `provider_status` listener) — smallest, removes a dead event, completes §11.
2. **GAP-2** (Esc restores prior target focus) — small, fixes a real UX bug.
3. **GAP-5 + GAP-7** (hotkey re-register + conflict toast) — together, completes §11 hotkey story.
4. **GAP-3** (mouse-position caret fallback) — medium, big UX win.
5. **GAP-1** (module split) — pure refactor, no behavior change, do last to avoid merge conflicts.
6. **GAP-6** (unit tests) — do after all behavior fixes land so tests cover final shape.
7. **GAP-8, GAP-10, GAP-12, GAP-13** — cosmetic cleanup pass.
8. **GAP-9** — leave deferred, document in README.

## Definition of done for this plan

All HIGH gaps closed, all MEDIUM gaps closed or explicitly waived in README, unit tests added for engine + db, `cargo test` + `tsc --noEmit` + `npx tauri build` all green.
