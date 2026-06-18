//! Windows UIAutomation impl.
//!
//! Uses `uiautomation` crate v0.16 (UIAutomation API).

use crate::types::Position;
use super::{AccessError, IAccessibilityService};

pub struct WindowsAccessibilityService;

impl WindowsAccessibilityService {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self)
    }
}

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
        // Spec §7: capture caret pos via UIA, or fallback to mouse pos.
        // UIA caret tracking needs ITextRangeProvider (deferred); use mouse
        // position as the caret fallback so the overlay appears where the user
        // is actually looking.
        if let Some(pos) = mouse_position() {
            return Ok(pos);
        }
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

/// Get current mouse cursor position via Win32 `GetCursorPos` (spec §7 fallback).
fn mouse_position() -> Option<Position> {
    #[repr(C)]
    struct Point { x: i32, y: i32 }

    extern "system" {
        fn GetCursorPos(lppoint: *mut Point) -> i32;
    }

    let mut pt = Point { x: 0, y: 0 };
    // SAFETY: GetCursorPos writes to our local Point; no aliasing.
    let ok = unsafe { GetCursorPos(&mut pt) };
    if ok != 0 {
        Some(Position { x: pt.x as f64, y: pt.y as f64 })
    } else {
        None
    }
}
