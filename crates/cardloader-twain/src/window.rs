//! Hidden top-level window and the Win32 message pump of the TWAIN thread.
//!
//! TWAIN on Windows is message driven: the source either posts its
//! notifications through the application's message loop (filtered with
//! DAT_EVENT/MSG_PROCESSEVENT) or calls a registered callback. Driver dialogs
//! (configuration UI, PaperStream error pop-ups) also need a running pump, and
//! sources want a real (if invisible) top-level window as parent — a
//! message-only window (HWND_MESSAGE) breaks several drivers.

use crate::twain_ffi::{TWRC_SUCCESS, TW_IDENTITY, TW_MEMREF};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::ffi::c_void;
use std::sync::atomic::{AtomicUsize, Ordering};
use windows::core::{w, Result};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;

/// Posted by the stdin thread: "a command is waiting in the channel".
pub const WM_COMMAND_WAKE: u32 = WM_APP + 1;
/// Posted by the TWAIN callback; WPARAM carries the TWAIN message (MSG_XFERREADY, ...).
pub const WM_TWAIN_MSG: u32 = WM_APP + 2;

/// Our window handle as an integer so other threads (stdin reader, the source's
/// callback thread) can post to it. HWNDs are valid process-wide.
static HWND_RAW: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    /// TWAIN messages received through the callback path, drained by the main loop.
    /// Queued from the window procedure so that nothing is lost while a driver
    /// runs a nested modal loop inside one of our DSM calls.
    static PENDING: RefCell<VecDeque<u16>> = const { RefCell::new(VecDeque::new()) };
}

pub fn create_hidden_window() -> Result<HWND> {
    unsafe {
        let instance: HINSTANCE = GetModuleHandleW(None)?.into();
        let class = w!("CardloaderTwainHidden");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            lpszClassName: class,
            ..Default::default()
        };
        if RegisterClassW(&wc) == 0 {
            return Err(windows::core::Error::from_thread());
        }
        // Never shown: WS_OVERLAPPEDWINDOW without WS_VISIBLE.
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class,
            w!("cardloader-twain"),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            None,
            None,
            Some(instance),
            None,
        )?;
        HWND_RAW.store(hwnd.0 as usize, Ordering::SeqCst);
        Ok(hwnd)
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_TWAIN_MSG => {
            PENDING.with(|q| q.borrow_mut().push_back(wp.0 as u16));
            LRESULT(0)
        }
        // The main loop drains the command channel after every message anyway.
        WM_COMMAND_WAKE => LRESULT(0),
        _ => unsafe { DefWindowProcW(hwnd, msg, wp, lp) },
    }
}

fn post(msg: u32, wparam: usize) {
    let raw = HWND_RAW.load(Ordering::SeqCst);
    if raw != 0 {
        // SAFETY: PostMessageW is thread-safe; a stale HWND just fails.
        let _ = unsafe {
            PostMessageW(
                Some(HWND(raw as *mut c_void)),
                msg,
                WPARAM(wparam),
                LPARAM(0),
            )
        };
    }
}

/// Wake the main thread after queueing a command (callable from any thread).
pub fn post_wake() {
    post(WM_COMMAND_WAKE, 0);
}

pub fn take_twain_messages() -> Vec<u16> {
    PENDING.with(|q| q.borrow_mut().drain(..).collect())
}

/// DAT_CALLBACK2 target. The source may call this from any thread and from
/// inside our own DSM calls, so per the TWAIN 2 spec it must not call back
/// into the DSM: it only forwards the message to our queue.
pub unsafe extern "system" fn twain_callback(
    _origin: *mut TW_IDENTITY,
    _dest: *mut TW_IDENTITY,
    _dg: u32,
    _dat: u16,
    msg: u16,
    _data: TW_MEMREF,
) -> u16 {
    post(WM_TWAIN_MSG, msg as usize);
    TWRC_SUCCESS
}

pub trait Pump {
    /// Offered every message before dispatch; return `true` if it was consumed
    /// (a TWAIN event that must not be dispatched).
    fn pre_translate(&mut self, msg: &MSG) -> bool;
    /// Called after every message; return `false` to leave the loop.
    fn after_message(&mut self) -> bool;
}

pub fn run_loop(pump: &mut dyn Pump) {
    let mut msg = MSG::default();
    loop {
        // SAFETY: standard GetMessage loop on the thread owning the window.
        let ret = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        if ret.0 == 0 || ret.0 == -1 {
            break;
        }
        if !pump.pre_translate(&msg) {
            unsafe {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        if !pump.after_message() {
            break;
        }
    }
}
