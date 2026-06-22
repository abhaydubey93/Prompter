<p align="center">
  <h1 align="center">Prompter</h1>
  <p align="center">
    <strong>Local-first prompt optimization overlay for Windows</strong><br/>
    Press a hotkey anywhere → capture text → optimize via LLM → replace in place.
  </p>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/version-0.1.0-blue?style=flat-square" alt="Version" />
  <img src="https://img.shields.io/badge/platform-Windows%20x64-lightgrey?style=flat-square" alt="Platform" />
  <img src="https://img.shields.io/badge/Tauri-2.0-24C8D8?style=flat-square&logo=tauri" alt="Tauri" />
  <img src="https://img.shields.io/badge/React-19-61DAFB?style=flat-square&logo=react" alt="React" />
  <img src="https://img.shields.io/badge/Rust-edition%202021-CE422B?style=flat-square&logo=rust" alt="Rust" />
  <img src="https://img.shields.io/badge/tests-31%20passed-brightgreen?style=flat-square" alt="Tests" />
</p>

---

## Overview

Prompter is a desktop overlay application that streamlines prompt engineering. It runs in the background and activates via a global hotkey (`Ctrl+Shift+E`). When triggered, it captures the text from the focused element, applies a selected prompt framework to optimize it, streams the results from an LLM, scores the output, and replaces the original text in place.

```
First launch → Onboarding wizard (pick provider + model)
Workflow     → Ctrl+Shift+E → capture → stream optimize → Accept → replaced in place
```

### Key Features

- **Multi-provider LLM support** — Ollama, LM Studio, OpenAI, Anthropic, OpenRouter, NVIDIA NIM, Gemini, and any OpenAI-compatible endpoint
- **10 built-in prompt frameworks** — CREATE, APE, TAG, RACE, CARE, RISE, ERA, TRACE, ROSES, SPARK — with custom import support
- **In-place text replacement** — Accessibility API first, clipboard fallback second
- **Streaming optimization** — Real-time token-by-token display of LLM output
- **Heuristic quality scoring** — 0–100 score based on structure, specificity, and repetition
- **Unified diff view** — Side-by-side comparison between original and optimized text
- **OS-native credential storage** — API keys stored in the system keychain via `keyring`
- **Dark & light themes** — Persistent theme preference with instant switching
- **First-run onboarding** — Guided provider setup wizard on initial launch (skippable)

---

## ⚠️ Disclaimer

**Developed with AI Assistance** — This application was developed using artificial intelligence tools to accelerate development and iteration.

**Platform Support** — Prompter is currently **Windows-only** (Windows 10/11, x64). Support for macOS and Linux is planned for upcoming releases. The codebase includes architecture traits for platform abstraction, enabling future cross-platform expansion.

---

## Table of Contents

- [Overview](#overview)
  - [Key Features](#key-features)
- [Disclaimer](#-disclaimer)
- [Table of Contents](#table-of-contents)
- [Supported Providers](#supported-providers)
- [Prompt Frameworks](#prompt-frameworks)
- [Prerequisites](#prerequisites)
- [Installation](#installation)
  - [From Source](#from-source)
  - [Production Installers](#production-installers)
- [Development](#development)
  - [Linting \& Type Checking](#linting--type-checking)
  - [Running Tests](#running-tests)
- [Production Build](#production-build)
- [Project Structure](#project-structure)
- [Architecture](#architecture)
  - [Database Schema (SQLite)](#database-schema-sqlite)
  - [Tauri IPC Surface](#tauri-ipc-surface)
- [Configuration](#configuration)
- [Keyboard Shortcuts](#keyboard-shortcuts)
- [Troubleshooting](#troubleshooting)
- [Contributing](#contributing)
- [Tech Stack](#tech-stack)
- [License](#license)

---

## Supported Providers

| Provider | Kind | Default URL | API Key | Status |
|:---------|:-----|:------------|:--------|:-------|
| **Ollama** | `ollama` | `http://localhost:11434` | Not required | Enabled by default |
| **LM Studio** | `openai_compat` | `http://localhost:1234/v1` | Not required | Enabled by default |
| **OpenAI** | `openai_compat` | `https://api.openai.com` | Required | Disabled by default |
| **Anthropic** | `anthropic` | `https://api.anthropic.com` | Required | Disabled by default |
| **OpenRouter** | `openai_compat` | `https://openrouter.ai/api/v1` | Required | Disabled by default |
| **NVIDIA NIM** | `openai_compat` | `https://integrate.api.nvidia.com/v1` | Required | Disabled by default |
| **Gemini** | `gemini` | `https://generativelanguage.googleapis.com` | Required | Disabled by default |

> **Custom providers:** Any OpenAI-compatible endpoint can be added via Settings → Providers. Use kind `openai_compat` with your endpoint URL.
>
> **API key storage:** Keys are stored in the OS-native credential store (Windows Credential Manager, macOS Keychain, etc.) using the `keyring` crate. Keys are never written to the database.

Provider selection format: `provider_id:model_name` (e.g., `ollama:llama3`, `openai:gpt-4o`).

---

## Prompt Frameworks

Frameworks are data, not code — each is a JSON file containing a Jinja template. Prompter ships with 10 built-in frameworks and supports custom imports.

| ID | Name | Focus |
|:---|:----|:------|
| `CREATE` | CREATE | Context, Request, Explanation, Action, Tone, Extras |
| `APE` | APE | Audience, Purpose, Execution |
| `TAG` | TAG | Topic, Audience, Goal |
| `RACE` | RACE | Role, Action, Context, Expectation |
| `CARE` | CARE | Context, Action, Result, Example |
| `RISE` | RISE | Role, Input, Steps, Expectation |
| `ERA` | ERA | Explain, Request, Action |
| `TRACE` | TRACE | Task, Request, Action, Context, Expectation |
| `ROSES` | ROSES | Role, Objective, Scenario, Expected, Style |
| `SPARK` | SPARK | Specific, Purpose, Action, Result, Key Detail |

Custom frameworks can be imported via Settings → Frameworks or by placing `.json` files in the `framework_packs/` directory. User packs override built-ins by matching ID.

---

## Prerequisites

- **Windows 10/11** (x64) — MVP is Windows-only; macOS/Linux are stubbed behind traits
- **[Node.js](https://nodejs.org/)** 18+ and npm
- **[Rust](https://www.rust-lang.org/tools/install)** stable toolchain with `cargo`
- **At least one LLM provider:**
  - [Ollama](https://ollama.com/) installed and running (for local inference), **or**
  - An API key for a cloud provider (OpenAI, Anthropic, etc.)
- **WebView2 Runtime** — Preinstalled on Windows 11; bundled with Tauri installers on Windows 10

---

## Installation

### From Source

```bash
# Clone the repository
git clone https://github.com/<org>/PromptForge.git
cd PromptForge

# Install frontend dependencies
npm install

# Run in development mode (hot reload)
npm run tauri dev
```

On first launch, the onboarding wizard will guide you through provider setup.

### Production Installers

Download the latest release from [Releases](../../releases):

| Artifact | Format | Description |
|:---------|:-------|:------------|
| `Prompter_0.1.0_x64_en-US.msi` | MSI | Windows Installer (MSI) |
| `Prompter_0.1.0_x64-setup.exe` | NSIS | Windows Setup (NSIS) |
| `prompter.exe` | Portable | Standalone executable |

---

## Development

```bash
# Install dependencies
npm install

# Start dev server (Vite + Tauri, hot reload)
npm run tauri dev
```

The dev server runs on `http://localhost:1420`. The first Rust compilation takes several minutes; subsequent builds are incremental.

### Linting & Type Checking

```bash
# TypeScript type check
npx tsc --noEmit

# Rust type check
cd src-tauri && cargo check
```

### Running Tests

```bash
# Run all 31 Rust tests
cd src-tauri && cargo test
```

---

## Production Build

```bash
npm run tauri build
```

Output artifacts are written to `src-tauri/target/release/bundle/`.

---

## Project Structure

```
PromptForge/
├── framework_packs/                           # Jinja template packs (10 built-ins)
│   ├── ape.json / care.json / create.json
│   ├── era.json / race.json / rise.json
│   ├── roses.json / spark.json / tag.json / trace.json
├── src/                                       # React frontend
│   ├── main.tsx                              # Hash router (#/overlay → OverlayApp)
│   ├── App.tsx                               # Main window (Library / History / Settings)
│   ├── Onboarding.tsx                        # 3-step first-run provider wizard
│   ├── overlay/
│   │   └── OverlayApp.tsx                    # Overlay window (capture/optimize/accept)
│   ├── lib/
│   │   └── tauri.ts                          # Typed Tauri IPC wrappers + event listeners
│   └── index.css                             # Tailwind + dark/light theme styles
├── src-tauri/                                 # Rust backend
│   ├── Cargo.toml                            # Rust dependencies
│   ├── tauri.conf.json                       # Tauri config (windows, bundle, resources)
│   ├── capabilities/
│   │   └── default.json                      # Window permissions
│   ├── framework_packs/                      # Packs bundled with the binary at build time
│   └── src/
│       ├── main.rs / lib.rs                  # App entry, plugin init, state management
│       ├── types.rs                          # IPC types, ProviderConfig, ApiError
│       ├── commands.rs                       # 25+ #[tauri::command] handlers
│       ├── db/
│       │   └── mod.rs                        # DbService — migrations, CRUD for 7 tables
│       ├── providers/
│       │   ├── mod.rs                        # LLMAdapter trait + build_adapter factory
│       │   ├── ollama.rs                     # NDJSON streaming (/api/chat, /api/tags)
│       │   ├── openai_compat.rs              # OpenAI-compatible (SSE /v1/chat/completions)
│       │   ├── anthropic.rs                  # Native Anthropic (/v1/messages)
│       │   ├── gemini.rs                     # Native Gemini (Generate Content)
│       │   └── keys.rs                       # OS keychain get/set/delete via keyring
│       ├── engine/
│       │   ├── mod.rs                        # OptimizationEngine (RwLock<FrameworkMap>)
│       │   └── frameworks.rs                 # 10 built-ins + file loader + override merge
│       ├── accessibility/
│       │   ├── mod.rs                        # IAccessibilityService trait + AccessError
│       │   ├── win.rs                        # Windows UIAutomation implementation
│       │   └── stub.rs                       # Non-Windows no-op stub
│       ├── replacement.rs                    # UIA setValue → verify → clipboard fallback
│       ├── hotkey.rs                         # Global shortcut → text capture → overlay
│       └── overlay.rs                        # Window show/hide, monitor-aware positioning
├── index.html
├── package.json
├── postcss.config.js
├── tailwind.config.js
├── tsconfig.json
├── tsconfig.node.json
├── vite.config.ts
└── README.md
```

---

## Architecture

```
┌────────────────────────────────────────────────────────────────[...]
│                         React UI (WebView)                       │
│                                                                  │
│  ┌──────────────┐  ┌──────────────────────────────────────────┐  │
│  │  Onboarding  │  │  OverlayApp                              │  │
│  │  (first run) │  │  ┌──────────┐  ┌───────────────────┐     │  │
│  └──────────────┘  │  │ Raw pane │  │ Optimized (stream)│     │  │
│                    │  └──────────┘  └───────────────────┘     │  │
│  ┌──────────────┐  │  Framework ▼  Model ▼  Score  Diff       │  │
│  │  App         │  │  Save  Copy  Accept                      │  │
│  │  Library     │  └──────────────────────────────────────────┘  │
│  │  History     │                                                │
│  │  Settings(5) │                                                │
│  └──────────────┘                                                │
└─────────────┬──────────────────────────────────┬───────────────[...]
              │  Tauri IPC (invoke / events)     │
┌─────────────▼──────────────────────────────────▼───────────────[...]
│                         Rust Core                                │
│                                                                  │
│  commands.rs ──► OptimizationEngine ──► LLMAdapter               │
│                                     (build_adapter per config)   │
│                                                                  │
│  capture_text   ──► AccessibilityService (Win32 UIAutomation)    │
│  accept_replace ──► ReplacementService (UIA → clipboard → enigo) │
│  hotkey.rs      ──► overlay.rs (monitor-aware positioning)       │
│  DbService      ◄──► SQLite (data.db, 7 tables)                  │
│  keys.rs        ◄──► OS Keychain (keyring)                       │
└────────────────────────────────────────────────────────────────[...]
```

### Database Schema (SQLite)

Seven tables in `%APPDATA%\Prompter\data.db`:

| Table | Purpose |
|:------|:--------|
| `prompts` | Saved prompt library entries (soft-delete, indexed by framework + score) |
| `context_profiles` | Role/audience/tone/style variables for template rendering |
| `app_profiles` | Per-application replacement strategy configuration |
| `history` | Every optimization logged (raw, optimized, model, score, timestamp) |
| `settings` | Key-value application configuration |
| `providers` | LLM provider configurations (7 seeded defaults, user-editable) |
| `meta` | Onboarding state and other application flags |

### Tauri IPC Surface

**Commands** (frontend → backend):

| Command | Purpose |
|:--------|:--------|
| `capture_text` | Read focused element text + caret position |
| `optimize_prompt` | Render template → stream from provider → emit events |
| `accept_replacement` | UIA setValue → verify → clipboard + Ctrl+V fallback |
| `get_models` | List available models for a provider |
| `test_provider` | Test provider connection and list models |
| `list_providers` / `get_provider` / `save_provider` / `delete_provider` | Provider CRUD |
| `set_provider_enabled` / `set_provider_key` | Provider state management |
| `list_frameworks` / `import_framework` / `delete_framework` | Framework management |
| `get_onboarding_state` / `complete_onboarding` | Onboarding flow |
| `save_prompt` / `list_prompts` / `search_prompts` / `delete_prompt` / `bump_usage` | Prompt library |
| `save_context` / `list_contexts` | Context profile CRUD |
| `list_history` / `clear_history` | History management |
| `get_settings` / `set_setting` | Settings key-value |
| `get_meta` / `set_meta` | Metadata key-value |
| `show_overlay` / `hide_overlay` / `db_stats` | Utility |

**Events** (backend → frontend):

| Event | Payload | Trigger |
|:------|:--------|:--------|
| `overlay_show` | `{ text, position }` | Hotkey fired |
| `opt_chunk` | `{ text, session_id }` | Each streamed token |
| `opt_done` | `{ optimized, score, diff, tokens, session_id }` | Stream complete |
| `opt_error` | `{ code, message, session_id }` | Stream failure |
| `provider_status` | `{ provider, alive }` | Startup health check |

---

## Configuration

All settings are editable in-app via **Settings** (5 tabs: General, Providers, Frameworks, Context Profiles, Privacy).

| Key | Default | Description |
|:----|:--------|:------------|
| `hotkey` | `Ctrl+Shift+E` | Global shortcut (applied dynamically) |
| `theme` | `dark` | `dark` · `light` · `system` |
| `default_framework` | `CREATE` | Pre-selected framework in overlay |
| `default_model` | `ollama:llama3` | Provider:model selector |
| `default_provider_id` | `ollama` | Default provider for overlay |
| `ollama_url` | `http://localhost:11434` | Ollama base URL |
| `overlay_opacity` | `90` | Overlay window opacity (0–100) |

---

## Keyboard Shortcuts

| Shortcut | Context | Action |
|:---------|:--------|:-------|
| `Ctrl+Shift+E` | Global | Open overlay / capture focused text |
| `Enter` | Overlay | Accept optimized text (replace in place) |
| `Shift+Enter` | Overlay | Insert newline (does not accept) |
| `Esc` | Overlay | Close overlay |

---

## Troubleshooting

| Symptom | Resolution |
|:--------|:-----------|
| Onboarding reappears on launch | No enabled provider found — complete setup or enable one in **Settings → Providers** |
| Overlay does not appear on hotkey | Another application may own `Ctrl+Shift+E` — change it in **Settings → General** and restart |
| Models dropdown is empty | Provider is unreachable — verify it is running and test the connection in **Settings → Providers** |
| `PROVIDER_UNREACHABLE` error | Check the provider URL in **Settings → Providers** and ensure the endpoint is responding |
| Replacement fails in web apps | Some SPAs block accessibility APIs — the clipboard fallback activates automatically; press `Ctrl+V` if prompted |
| API key does not persist | Verify the OS keychain service is available (Windows Credential Manager, macOS Keychain) |
| Slow initial build | Release LTO + `uiautomation` crate is heavy (~1–10 min first time); incremental builds are fast |

---

## Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/my-feature`)
3. Commit your changes (`git commit -m 'Add my feature'`)
4. Push to the branch (`git push origin feature/my-feature`)
5. Open a Pull Request

Please ensure all tests pass (`cargo test`) and there are no TypeScript errors (`tsc --noEmit`) before submitting.

---

## Tech Stack

| Layer | Technology |
|:------|:-----------|
| Shell | [Tauri 2](https://tauri.app/) — Rust + WebView |
| Backend | Rust (edition 2021) |
| Frontend | [React 19](https://react.dev/) + [TypeScript 5.8](https://www.typescriptlang.org/) + [Vite 7](https://vite.dev/) |
| Styling | [Tailwind CSS 3.4](https://tailwindcss.com/) (dark + light theme) |
| Database | SQLite via [rusqlite](https://github.com/rusqlite/rusqlite) (bundled) |
| Templates | [Minijinja 2](https://docs.rs/minijinja/) |
| Diffing | [similar 2](https://github.com/mitsuhiko/similar) |
| HTTP | [reqwest](https://crates.io/crates/reqwest) (rustls-tls, streaming) |
| Keychain | [keyring 3](https://github.com/hwchen/keyring-rs) (OS-native credentials) |
| Accessibility | [uiautomation 0.16](https://crates.io/crates/uiautomation) (Windows UIAutomation) |
| Input Simulation | [enigo 0.6](https://crates.io/crates/enigo) |
| Hotkeys | [tauri-plugin-global-shortcut 2](https://v2.tauri.app/plugin/global-shortcut/) |
| Clipboard | [tauri-plugin-clipboard-manager 2](https://v2.tauri.app/plugin/clipboard-manager/) |

---

## License

This project is licensed under the MIT License - see the [LICENSE](file:///c:/Users/abhay/VibeCode/PromptForge/LICENSE) file for details.
