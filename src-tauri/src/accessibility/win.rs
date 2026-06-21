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
        // Try OS caret first, then fallback to mouse pos
        if let Some(pos) = caret_position() {
            return Ok(pos);
        }
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
    let ok = unsafe { GetCursorPos(&mut pt) };
    if ok != 0 {
        Some(Position { x: pt.x as f64, y: pt.y as f64 })
    } else {
        None
    }
}

/// Try to get caret position using Win32 GetGUIThreadInfo
fn caret_position() -> Option<Position> {
    #[repr(C)]
    struct RECT { left: i32, top: i32, right: i32, bottom: i32 }
    #[repr(C)]
    struct GUITHREADINFO {
        cbSize: u32,
        flags: u32,
        hwndActive: *mut std::ffi::c_void,
        hwndFocus: *mut std::ffi::c_void,
        hwndCapture: *mut std::ffi::c_void,
        hwndMenuOwner: *mut std::ffi::c_void,
        hwndMoveSize: *mut std::ffi::c_void,
        hwndCaret: *mut std::ffi::c_void,
        rcCaret: RECT,
    }
    #[repr(C)]
    struct POINT { x: i32, y: i32 }

    extern "system" {
        fn GetGUIThreadInfo(idThread: u32, lpgui: *mut GUITHREADINFO) -> i32;
        fn ClientToScreen(hWnd: *mut std::ffi::c_void, lpPoint: *mut POINT) -> i32;
        fn GetForegroundWindow() -> isize;
        fn GetWindowThreadProcessId(hWnd: *mut std::ffi::c_void, lpdwProcessId: *mut u32) -> u32;
    }

    unsafe {
        let hwnd = GetForegroundWindow() as *mut std::ffi::c_void;
        if hwnd.is_null() { return None; }
        let thread_id = GetWindowThreadProcessId(hwnd, std::ptr::null_mut());
        
        let mut gti = GUITHREADINFO {
            cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
            flags: 0,
            hwndActive: std::ptr::null_mut(),
            hwndFocus: std::ptr::null_mut(),
            hwndCapture: std::ptr::null_mut(),
            hwndMenuOwner: std::ptr::null_mut(),
            hwndMoveSize: std::ptr::null_mut(),
            hwndCaret: std::ptr::null_mut(),
            rcCaret: RECT { left: 0, top: 0, right: 0, bottom: 0 },
        };
        
        if GetGUIThreadInfo(thread_id, &mut gti) != 0 && !gti.hwndCaret.is_null() {
            let mut pt = POINT { x: gti.rcCaret.left, y: gti.rcCaret.bottom };
            if ClientToScreen(gti.hwndCaret, &mut pt) != 0 {
                return Some(Position { x: pt.x as f64, y: pt.y as f64 });
            }
        }
    }
    None
}
