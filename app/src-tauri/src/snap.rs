//! Windows 11 snap layouts for the custom maximize button (spec section
//! 9.1; Plan 2b carry-over 3.1 item 1).
//!
//! Windows shows the snap layout flyout when the pointer rests on an area
//! whose WM_NCHITTEST answer is HTMAXBUTTON. Jojo's title bar is HTML inside
//! WebView2, and WebView2 takes mouse input in a child window owned by the
//! WebView2 browser process, which this process cannot subclass. This
//! module creates a small child window of the main window (an input sink,
//! the technique Windows Terminal uses for its title bar), keeps it above
//! the WebView2 window exactly over the HTML maximize button, and answers
//! HTMAXBUTTON for it. Because the sink receives the button's mouse input,
//! it also maximizes or restores the window on click, and it reports pointer
//! enter and leave to the frontend (`HOVER_EVENT`) so the HTML button can
//! still draw its hover state.
//!
//! The sink is a layered window (WS_EX_LAYERED) without a redirection bitmap
//! (WS_EX_NOREDIRECTIONBITMAP): it never paints, so nothing of it is
//! visible, and because its layered alpha is 255 Windows still hit-tests it.
//! Layered child windows require an application manifest that declares
//! Windows 8 or later; app/src-tauri/windows-app-manifest.xml declares
//! Windows 10 and 11.

use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::OnceLock;
use tauri::{AppHandle, Emitter};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{TrackMouseEvent, TME_LEAVE, TME_NONCLIENT, TRACKMOUSEEVENT};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, GetParent, IsZoomed, LoadCursorW, RegisterClassExW, SetLayeredWindowAttributes,
    SetWindowPos, ShowWindow, HTMAXBUTTON, HWND_TOP, IDC_ARROW, LWA_ALPHA, SWP_NOACTIVATE, SWP_SHOWWINDOW,
    SW_MAXIMIZE, SW_RESTORE, WM_NCHITTEST, WM_NCLBUTTONDBLCLK, WM_NCLBUTTONDOWN, WM_NCLBUTTONUP, WM_NCMOUSELEAVE,
    WM_NCMOUSEMOVE, WNDCLASSEXW, WS_CHILD, WS_CLIPSIBLINGS, WS_EX_LAYERED, WS_EX_NOREDIRECTIONBITMAP, WS_VISIBLE,
};

/// Event sent to the frontend: `true` when the pointer enters the sink,
/// `false` when it leaves.
pub const HOVER_EVENT: &str = "maximize-hover";

/// The sink's HWND, stored as an integer because HWND is a raw pointer
/// (not Send); 0 until `install` succeeds.
static SINK: AtomicIsize = AtomicIsize::new(0);
/// True between the first WM_NCMOUSEMOVE and the WM_NCMOUSELEAVE that
/// TrackMouseEvent requested.
static TRACKING: AtomicBool = AtomicBool::new(false);
static APP: OnceLock<AppHandle> = OnceLock::new();

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Creates the sink as a child of the main window, whose HWND is passed as
/// an integer (`parent`): HWND is a raw pointer, and a safe public function
/// must not take a raw pointer that it hands to the OS (clippy's
/// `not_unsafe_ptr_arg_deref`). Call once, on the main thread, after the
/// webview exists, so the sink is created above the WebView2 child window.
pub fn install(app: &AppHandle, parent: isize) -> Result<(), String> {
    let parent = parent as HWND;
    let _ = APP.set(app.clone());
    let class = wide("JojoMaximizeButtonSink");
    // SAFETY: every pointer passed below is either null where the API allows
    // null, or points to a NUL-terminated UTF-16 buffer (`class`) or a fully
    // initialized struct (`wc`) that outlives the call. `parent` is the live
    // main window handle from Tauri.
    unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: 0,
            lpfnWndProc: Some(sink_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance,
            hIcon: std::ptr::null_mut(),
            hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
            hbrBackground: std::ptr::null_mut(),
            lpszMenuName: std::ptr::null(),
            lpszClassName: class.as_ptr(),
            hIconSm: std::ptr::null_mut(),
        };
        if RegisterClassExW(&wc) == 0 {
            return Err(format!("RegisterClassExW failed: {}", std::io::Error::last_os_error()));
        }
        let sink = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_NOREDIRECTIONBITMAP,
            class.as_ptr(),
            std::ptr::null(),
            WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
            0,
            0,
            0,
            0,
            parent,
            std::ptr::null_mut(),
            instance,
            std::ptr::null(),
        );
        if sink.is_null() {
            return Err(format!("CreateWindowExW failed: {}", std::io::Error::last_os_error()));
        }
        SetLayeredWindowAttributes(sink, 0, 255, LWA_ALPHA);
        SINK.store(sink as isize, Ordering::Release);
    }
    Ok(())
}

/// Moves the sink over the HTML maximize button. Coordinates are physical
/// pixels in the main window's client area. Does nothing before `install`
/// has succeeded.
pub fn set_button_rect(x: i32, y: i32, width: i32, height: i32) {
    let sink = SINK.load(Ordering::Acquire) as HWND;
    if sink.is_null() {
        return;
    }
    // SAFETY: `sink` is the window `install` created; it lives as long as
    // its parent, the main window. HWND_TOP puts it above its siblings,
    // including the WebView2 child window.
    unsafe {
        SetWindowPos(sink, HWND_TOP, x, y, width.max(0), height.max(0), SWP_NOACTIVATE | SWP_SHOWWINDOW);
    }
}

fn emit_hover(hovered: bool) {
    if let Some(app) = APP.get() {
        let _ = app.emit(HOVER_EVENT, hovered);
    }
}

/// Window procedure of the sink.
unsafe extern "system" fn sink_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        // The whole sink is the maximize button. This answer is what makes
        // Windows 11 show the snap layout flyout on hover.
        WM_NCHITTEST => HTMAXBUTTON as LRESULT,
        WM_NCMOUSEMOVE => {
            if !TRACKING.swap(true, Ordering::AcqRel) {
                let mut track = TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE | TME_NONCLIENT,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                TrackMouseEvent(&mut track);
                emit_hover(true);
            }
            0
        }
        WM_NCMOUSELEAVE => {
            TRACKING.store(false, Ordering::Release);
            emit_hover(false);
            0
        }
        // Swallowed: DefWindowProcW would run its own caption-button
        // tracking for a window that has no caption.
        WM_NCLBUTTONDOWN | WM_NCLBUTTONDBLCLK => 0,
        WM_NCLBUTTONUP => {
            if wparam as u32 == HTMAXBUTTON {
                let parent = GetParent(hwnd);
                ShowWindow(parent, if IsZoomed(parent) != 0 { SW_RESTORE } else { SW_MAXIMIZE });
            }
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sink_answers_hit_tests_as_the_maximize_button() {
        // SAFETY: WM_NCHITTEST is answered without using the window handle.
        let result = unsafe { sink_proc(std::ptr::null_mut(), WM_NCHITTEST, 0, 0) };
        assert_eq!(result, HTMAXBUTTON as LRESULT);
    }

    #[test]
    fn moving_the_sink_before_it_exists_does_nothing() {
        set_button_rect(0, 0, 46, 38);
    }
}
