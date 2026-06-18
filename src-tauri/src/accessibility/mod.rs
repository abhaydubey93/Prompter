//! OS accessibility abstraction.
//!
//! `IAccessibilityService` trait — platform-specific impls handle text capture
//! and in-place replacement. Windows uses UIAutomation; Mac/Linux are stubbed for
//! the MVP.

use crate::types::Position;

pub mod platform; // `platform.rs` does cfg-gate

pub use platform::AccessibilityService;

/// Unified interface for reading/writing text in the active application.
pub trait IAccessibilityService: Send + Sync {
    /// Get the text currently in the focused element (or selected text).
    fn get_active_element_text(&self) -> Result<String, AccessError>;

    /// Get screen position of the caret or focused element.
    fn get_caret_position(&self) -> Result<Position, AccessError>;

    /// Replace the text in the focused element (must NOT steal focus).
    fn set_element_text(&self, text: &str) -> Result<(), AccessError>;
}

#[derive(Debug, thiserror::Error)]
pub enum AccessError {
    #[error("accessibility API error: {0}")]
    Api(String),
    #[error("no focused element found")]
    NoFocus,
    #[error("element is read-only / not replaceable")]
    ReadOnly,
    #[error("permissions not granted")]
    Permission,
}

/// Replacement strategy enum (see HLD §3.2 pipeline).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ReplaceStrategy {
    Accessibility,
    Clipboard,
    SyntheticKeys,
}
