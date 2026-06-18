//! Platform-specific accessibility service.
//!
//! Windows: `uiautomation` crate v0.16 (UIAutomation API).
//! Other platforms: no-op stub (MVP is Windows-only).

use crate::types::Position;
use super::{AccessError, IAccessibilityService};

#[cfg(windows)]
pub type AccessibilityService = WindowsAccessibilityService;

#[cfg(not(windows))]
pub type AccessibilityService = StubAccessibilityService;

// ─── Windows impl ────────────────────────────────────────────────────────

#[cfg(windows)]
pub struct WindowsAccessibilityService;

#[cfg(windows)]
impl WindowsAccessibilityService {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self)
    }
}

#[cfg(windows)]
impl IAccessibilityService for WindowsAccessibilityService {
    fn get_active_element_text(&self) -> Result<String, AccessError> {
        use uiautomation::UIAutomation;
        use uiautomation::patterns::UIValuePattern;
        let uia = UIAutomation::new()
            .map_err(|e| AccessError::Api(format!("cannot init UIAutomation: {e}")))?;

        let focused = uia.get_focused_element()
            .map_err(|e| AccessError::Api(format!("get_focused_element failed: {e}")))?;

        // Try ValuePattern.get_value() first (real text fields).
        if let Ok(pattern) = focused.get_pattern::<UIValuePattern>() {
            if let Ok(value) = pattern.get_value() {
                if !value.is_empty() {
                    return Ok(value);
                }
            }
        }

        // Fallback: element name (some controls expose text as Name).
        let name = focused.get_name().unwrap_or_default();
        if !name.is_empty() {
            return Ok(name);
        }

        Ok(String::new())
    }

    fn get_caret_position(&self) -> Result<Position, AccessError> {
        // MVP: return a safe default. Full caret tracking needs
        // ITextRangeProvider — deferred post-MVP. Overlay centers if unknown.
        Ok(Position { x: 400.0, y: 300.0 })
    }

    fn set_element_text(&self, text: &str) -> Result<(), AccessError> {
        use uiautomation::UIAutomation;
        use uiautomation::patterns::UIValuePattern;

        let uia = UIAutomation::new()
            .map_err(|e| AccessError::Api(format!("cannot init UIAutomation: {e}")))?;

        let focused = uia.get_focused_element()
            .map_err(|_| AccessError::NoFocus)?;

        match focused.get_pattern::<UIValuePattern>() {
            Ok(pattern) => {
                pattern.set_value(text)
                    .map_err(|e| AccessError::Api(format!("SetValue failed: {e}")))?;
                Ok(())
            }
            Err(_) => Err(AccessError::ReadOnly),
        }
    }
}

// ─── Stub (macOS / Linux) ────────────────────────────────────────────────

#[cfg(not(windows))]
pub struct StubAccessibilityService;

#[cfg(not(windows))]
impl StubAccessibilityService {
    pub fn new() -> anyhow::Result<Self> {
        tracing::warn!("accessibility service: stub (non-Windows platform)");
        Ok(Self)
    }
}

#[cfg(not(windows))]
impl IAccessibilityService for StubAccessibilityService {
    fn get_active_element_text(&self) -> Result<String, AccessError> {
        Err(AccessError::Api("accessibility not implemented on this platform".into()))
    }
    fn get_caret_position(&self) -> Result<Position, AccessError> {
        Err(AccessError::Api("accessibility not implemented on this platform".into()))
    }
    fn set_element_text(&self, _text: &str) -> Result<(), AccessError> {
        Err(AccessError::Api("accessibility not implemented on this platform".into()))
    }
}
