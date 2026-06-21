# Prompter Documentation

## Table of Contents
1. [Introduction](#introduction)
2. [Architecture](#architecture)
3. [Features](#features)
4. [User Guide](#user-guide)
5. [Developer Guide](#developer-guide)
6. [Security & Privacy](#security--privacy)

---

## Introduction

Prompter is a local-first prompt optimization overlay designed for Windows. It provides a seamless way to enhance your text and prompts using various Large Language Models (LLMs) without breaking your workflow. Triggered by a global hotkey, Prompter captures focused text, runs it through an LLM based on specific prompt engineering frameworks, and replaces the text in-place.

## Architecture

Prompter is built on the Tauri 2.0 framework, blending a high-performance Rust backend with a modern React/TypeScript frontend.

### Frontend (React + TypeScript + Vite)
- **OverlayApp:** The main overlay window that handles text capture, streaming optimization results, and text replacement.
- **App/Settings:** The primary window containing the prompt library, optimization history, and deep configuration settings.
- **Onboarding:** A guided first-run experience for setting up LLM providers.
- **Styling:** Tailwind CSS with support for seamless dark/light theme switching.

### Backend (Rust)
- **Engine:** Evaluates templates (using `minijinja`) and orchestrates LLM interactions.
- **Providers:** Abstraction layers for LLM backends (Ollama, OpenAI, Anthropic, Gemini, etc.).
- **Accessibility:** Uses the Windows UIAutomation API (`uiautomation` crate) to capture and replace text in native applications.
- **Database:** SQLite (via `rusqlite`) stores settings, history, saved prompts, context profiles, and provider configurations.
- **Security:** API keys are encrypted via AES-256-GCM and stored using a combination of the OS Keychain and a secure fallback mechanism (`keys_fallback.enc`).

---

## Features

- **Global Hotkey Activation:** Press `Ctrl+Shift+E` anywhere in Windows to trigger the overlay.
- **In-Place Replacement:** Replaces text natively in text boxes. Falls back to clipboard if UIA is blocked by the target application.
- **Multi-Provider Support:** Supports local inference (Ollama, LM Studio) and cloud inference (OpenAI, Anthropic, Gemini, OpenRouter, NVIDIA NIM).
- **Prompt Frameworks:** 10 built-in frameworks (CREATE, APE, TAG, RACE, CARE, RISE, ERA, TRACE, ROSES, SPARK) for structured generation.
- **Security-First Credentials:** API keys are never stored in plain text. They are kept in the OS keychain and securely backed up with hardware-bound AES encryption.
- **System Tray Integration:** Quick access to settings or quitting the application securely.

---

## User Guide

### 1. Setup
On first launch, Prompter will guide you through setting up a provider. You can choose a local provider like Ollama or enter an API key for a cloud provider.

### 2. Using the Overlay
- Highlight or focus your cursor on text in any application.
- Press `Ctrl+Shift+E`.
- Select your desired Framework and Model from the dropdowns.
- Click **Optimize** or press `Enter` (if focus is not on text).
- Once the text is streamed and verified, click **Accept** to replace the original text.

### 3. Settings & History
Right-click the system tray icon and select **Open Settings**.
- **Library:** View and reuse saved prompts.
- **History:** Review past optimizations.
- **Settings:** Configure hotkeys, themes, default providers, context profiles, and custom frameworks.

---

## Developer Guide

### Prerequisites
- Node.js 18+
- Rust stable toolchain
- Windows SDK (for UIAutomation components)

### Running Locally
```bash
npm install
npm run tauri dev
```

### Building for Production
```bash
npm run build
npm run tauri build
```
This produces `.msi`, `-setup.exe`, and portable `.exe` artifacts in `src-tauri/target/release/bundle/`.

---

## Security & Privacy

Prompter takes security seriously:
- **No Telemetry by Default:** Anonymous telemetry is off unless explicitly enabled in settings.
- **Key Encryption:** Cloud API keys are managed via the OS native keychain. If the keychain drops the keys (a known Windows edge-case), Prompter securely falls back to an encrypted local file using AES-256-GCM. The encryption key is derived from a SHA-256 hash of your machine's unique hardware UID, meaning keys cannot be decrypted on a different machine.
- **Local-First:** If using Ollama or LM Studio, no data ever leaves your machine.
