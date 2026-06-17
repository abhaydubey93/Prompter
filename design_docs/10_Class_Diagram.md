# Class Diagram — PromptOpt Overlay

| Field | Value |
|-------|-------|
| **Document ID** | CLS-001 |
| **Version** | 1.0 |
| **Date** | 2026-06-17 |
| **Status** | Draft for Review |

---

## 1. Introduction

This document outlines the core class structures for both the Rust backend (Traits and Structs) and the React frontend (Components).

---

## 2. Rust Backend Core

```mermaid
classDiagram
    class IAccessibilityService {
        <<interface>>
        +get_active_element_text() Result~String~
        +get_caret_position() Result~Position~
        +set_element_text(text: String) Result~()~
        +simulate_paste() Result~()~
    }
    
    class WindowsAccessibilityService {
        -uiautomation: UIAutomation
        +get_active_element_text() Result~String~
        +set_element_text(text: String) Result~()~
    }
    
    class MacAccessibilityService {
        -axui: AXUIElement
        +get_active_element_text() Result~String~
        +set_element_text(text: String) Result~()~
    }
    
    class ILLMAdapter {
        <<interface>>
        +stream_chat(messages: Vec~Message~) Result~ChatStream~
        +list_models() Result~Vec~ModelInfo~~
    }
    
    class OllamaAdapter {
        -endpoint: String
        +stream_chat(messages: Vec~Message~) Result~ChatStream~
    }
    
    class OpenAIAdapter {
        -api_key: String
        +stream_chat(messages: Vec~Message~) Result~ChatStream~
    }
    
    class OptimizationEngine {
        -db: DatabaseService
        -render_template(template, context) String
        +optimize(raw_text, framework, model) Result~OptimizedPrompt~
        +calculate_diff(raw, optimized) Diff
        +evaluate_score(optimized) u8
    }
    
    class ReplacementService {
        -access_service: IAccessibilityService
        +replace_in_place(text: String) Result~()~
        -fallback_clipboard(text: String) Result~()~
    }
    
    IAccessibilityService <|.. WindowsAccessibilityService
    IAccessibilityService <|.. MacAccessibilityService
    ILLMAdapter <|.. OllamaAdapter
    ILLMAdapter <|.. OpenAIAdapter
    OptimizationEngine --> ILLMAdapter : uses
    ReplacementService --> IAccessibilityService : uses
```

---

## 3. React Frontend Components

```mermaid
classDiagram
    class App {
        +invoke(command, payload)
    }
    
    class OverlayContainer {
        +rawText: String
        +optimizedText: String
        +isStreaming: Boolean
    }
    
    class ResultPanel {
        +diff: Diff
        +score: Number
        +onAccept()
        +onRefine()
    }
    
    class ProviderSelector {
        +providers: List
        +onSelect(model)
    }
    
    class PromptLibrary {
        +prompts: List
        +onSave()
    }
    
    App --> OverlayContainer : renders
    OverlayContainer --> ResultPanel : contains
    OverlayContainer --> ProviderSelector : contains
    App --> PromptLibrary : renders
```

---

## 4. Data Models

```mermaid
classDiagram
    class Prompt {
        +UUID id
        +String title
        +String body
        +String framework
        +String model_used
        +Integer score
        +Integer usage_count
        +String source_app
    }
    
    class ContextProfile {
        +UUID id
        +String name
        +String role
        +String audience
        +String tone
    }
    
    class AppProfile {
        +String app_name
        +String default_framework
        +String replacement_strategy
    }
    
    class Settings {
        +String hotkey
        +String theme
        +Boolean telemetry
        +String pii_regex
    }
    
    Prompt --> ContextProfile : optional
    AppProfile --> ContextProfile : default
```

---

*End of Class Diagram.*
