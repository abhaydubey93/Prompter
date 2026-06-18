//! No-op accessibility impl for non-Windows builds (macOS / Linux).
//!
//! The MVP is Windows-only; this stub satisfies the trait surface so the
//! crate still compiles cross-platform.

use crate::types::Position;
use super::{AccessError, IAccessibilityService};

pub struct StubAccessibilityService;

impl StubAccessibilityService {
    pub fn new() -> anyhow::Result<Self> {
        tracing::warn!("accessibility service: stub (non-Windows platform)");
        Ok(Self)
    }
}

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
