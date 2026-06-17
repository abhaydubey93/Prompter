# Sequence Diagrams — PromptOpt Overlay

| Field | Value |
|-------|-------|
| **Document ID** | SEQ-001 |
| **Version** | 1.0 |
| **Date** | 2026-06-17 |
| **Status** | Draft for Review |

---

## 1. Introduction

This document details the core interaction flows for the PromptOpt Overlay application.

## 2. Main Success Scenario (MSS)

```mermaid
sequenceDiagram
    actor U as User
    participant App as Target App
    participant Core as Rust Core
    participant UI as Overlay UI
    participant LLM as LLM Provider
    
    U->>App: Types raw prompt
    U->>App: Presses Cmd/Ctrl+Shift+E
    
    Note over Core: HotkeyService intercepts
    Core->>App: AccessibilityService.getActiveFieldText()
    App-->>Core: Returns raw text + caret pos
    
    Core->>UI: Show Overlay (anchored to caret)
    UI->>UI: Pre-fill raw text & defaults
    
    U->>UI: Clicks Optimize
    UI->>Core: optimize_prompt(text, framework, model)
    Core->>Core: Render Jinja meta-prompt
    Core->>LLM: POST /chat (stream=true)
    
    loop Stream
        LLM-->>Core: Chunk
        Core-->>UI: emit("opt_chunk")
        UI->>U: Progressive render
    end
    
    LLM-->>Core: [DONE]
    Core->>Core: Calculate diff & score
    Core-->>UI: emit("opt_done")
    
    U->>UI: Clicks Accept
    UI->>Core: accept_replacement(enhanced_text)
    Core->>App: AccessibilityService.set_value(text)
    App-->>Core: Success
    
    Core->>UI: Close Overlay
    Core->>App: Restore focus
    App-->>U: Enhanced prompt visible
```

---

## 3. Alternate Flow: Clipboard Fallback

Triggered when the target application does not support native Accessibility `setValue`.

```mermaid
sequenceDiagram
    actor U as User
    participant App as Target App (e.g., Chrome)
    participant Core as Rust Core
    participant OS as OS Clipboard
    participant KB as Keyboard Simulator
    
    U->>Core: Clicks Accept
    Core->>App: Try AccessibilityService.set_value(text)
    App-->>Core: Error / Text unchanged
    
    Note over Core: Fallback triggered
    
    Core->>OS: Get current clipboard contents
    OS-->>Core: old_clipboard
    
    Core->>OS: Set clipboard to enhanced_text
    Core->>KB: Simulate Cmd/Ctrl+V
    
    Note over App: App processes paste command
    App->>App: Insert text from clipboard
    
    Core->>Core: Sleep(50ms)
    Core->>OS: Restore old_clipboard
    
    Core->>U: Toast: "Pasted via clipboard"
```

---

## 4. Alternate Flow: PII Privacy Guard

Triggered when a user attempts to optimize a prompt containing PII with a Cloud provider.

```mermaid
sequenceDiagram
    participant U as User
    participant UI as Overlay UI
    participant Engine as Optimization Engine
    participant Router as Provider Router
    
    U->>UI: Enters prompt with SSN
    U->>UI: Selects Cloud Model (GPT-4o)
    U->>UI: Clicks Optimize
    
    UI->>Engine: optimize_prompt(raw, model="gpt-4o")
    Engine->>Engine: Run PII Regex Check
    Note over Engine: Match found: \d{3}-\d{2}-\d{4}
    
    Engine->>Router: Check routing rules
    Router-->>Engine: Cloud blocked, fallback to Local
    
    Engine-->>UI: Error/Warning Event
    UI->>U: "PII Detected. Switching to Local model for privacy."
    
    Engine->>Router: Route to Ollama
    Note over Engine: Continue optimization locally
```

---

## 5. Alternate Flow: Hotkey Conflict Detection

```mermaid
sequenceDiagram
    participant User
    participant Settings as Settings UI
    participant Core as Rust Core
    participant OS
    
    User->>Settings: Attempts to bind Cmd+Shift+S
    Settings->>Core: check_hotkey_conflict("Cmd+Shift+S")
    Core->>OS: Query registered global shortcuts
    
    alt Conflict Found
        OS-->>Core: Conflict with App X
        Core-->>Settings: Return ConflictError
        Settings->>User: "Cmd+Shift+S is used by App X. Pick another."
    else No Conflict
        OS-->>Core: Clear
        Core->>OS: Register new hotkey
        Core-->>Settings: Success
        Settings->>User: "Hotkey saved"
    end
```

---

*End of Sequence Diagrams.*
