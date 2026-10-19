//! Loading the TWAIN Data Source Manager and DSM-aware memory management.

use crate::protocol::log;
use crate::twain_ffi::*;
use libloading::Library;
use std::ffi::c_void;
use std::path::PathBuf;
use windows::Win32::Foundation::{GlobalFree, HGLOBAL};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GHND};

pub struct Dsm {
    entry: DSMENTRYPROC,
    pub path: String,
    _lib: Library,
}

impl Dsm {
    /// Search order: `TWAINDSM.dll` next to our exe (the bundled copy, which must
    /// match our bitness), then the system search path (System32 / SysWOW64),
    /// then — only for 32-bit builds — the legacy TWAIN 1.x `twain_32.dll`.
    pub fn load() -> Result<Dsm, String> {
        let mut candidates: Vec<PathBuf> = Vec::new();
        if let Some(bundled) = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("TWAINDSM.dll")))
        {
            candidates.push(bundled);
        }
        candidates.push("TWAINDSM.dll".into());
        if cfg!(target_pointer_width = "32") {
            candidates.push("twain_32.dll".into());
        }
        let mut errors = Vec::new();
        for path in candidates {
            if path.is_absolute() && !path.exists() {
                continue;
            }
            // SAFETY: loading the DSM runs its DllMain; that is the documented way to use TWAIN.
            match unsafe { Library::new(&path) } {
                Ok(lib) => {
                    // SAFETY: DSM_Entry has the DSMENTRYPROC signature in every DSM.
                    let entry = match unsafe { lib.get::<DSMENTRYPROC>(b"DSM_Entry\0") } {
                        Ok(sym) => *sym,
                        Err(e) => {
                            errors.push(format!("{}: {e}", path.display()));
                            continue;
                        }
                    };
                    log!("loaded DSM from {}", path.display());
                    return Ok(Dsm {
                        entry,
                        path: path.display().to_string(),
                        _lib: lib,
                    });
                }
                Err(e) => errors.push(format!("{}: {e}", path.display())),
            }
        }
        Err(format!("TWAIN DSM not found ({})", errors.join("; ")))
    }

    /// Raw `DSM_Entry` call.
    ///
    /// # Safety
    /// `data` must point to the structure the (DG, DAT, MSG) triplet expects.
    pub unsafe fn entry(
        &self,
        origin: *mut TW_IDENTITY,
        dest: *mut TW_IDENTITY,
        dg: u32,
        dat: u16,
        msg: u16,
        data: *mut c_void,
    ) -> u16 {
        unsafe { (self.entry)(origin, dest, dg, dat, msg, data) }
    }
}

/// Memory for containers exchanged with the source. TWAIN 2 requires using the
/// DSM's allocator (DAT_ENTRYPOINT) when the DSM is 2.x; otherwise Global* is
/// the Windows convention.
#[derive(Clone, Copy, Default)]
pub struct Mem {
    ep: Option<TW_ENTRYPOINT>,
}

impl Mem {
    pub fn with_entrypoint(ep: TW_ENTRYPOINT) -> Self {
        // Braces copy the fields out of the packed struct (no unaligned references).
        let complete = { ep.DSM_MemAllocate }.is_some()
            && { ep.DSM_MemFree }.is_some()
            && { ep.DSM_MemLock }.is_some()
            && { ep.DSM_MemUnlock }.is_some();
        Mem {
            ep: complete.then_some(ep),
        }
    }

    pub fn alloc(&self, size: usize) -> TW_HANDLE {
        match self.ep.and_then(|e| e.DSM_MemAllocate) {
            // SAFETY: valid DSM function pointer returned by DAT_ENTRYPOINT.
            Some(f) => unsafe { f(size as u32) },
            // SAFETY: plain Win32 allocation.
            None => unsafe { GlobalAlloc(GHND, size) }
                .map(|h| h.0)
                .unwrap_or(std::ptr::null_mut()),
        }
    }

    /// # Safety
    /// `h` must be a live handle from `alloc` or from the source.
    pub unsafe fn lock(&self, h: TW_HANDLE) -> *mut u8 {
        unsafe {
            match self.ep.and_then(|e| e.DSM_MemLock) {
                Some(f) => f(h) as *mut u8,
                None => GlobalLock(HGLOBAL(h)) as *mut u8,
            }
        }
    }

    /// # Safety
    /// `h` must be locked.
    pub unsafe fn unlock(&self, h: TW_HANDLE) {
        unsafe {
            match self.ep.and_then(|e| e.DSM_MemUnlock) {
                Some(f) => f(h),
                None => {
                    let _ = GlobalUnlock(HGLOBAL(h));
                }
            }
        }
    }

    /// # Safety
    /// `h` must be a live, unlocked handle; it is invalid afterwards.
    pub unsafe fn free(&self, h: TW_HANDLE) {
        if h.is_null() {
            return;
        }
        unsafe {
            match self.ep.and_then(|e| e.DSM_MemFree) {
                Some(f) => f(h),
                None => {
                    let _ = GlobalFree(Some(HGLOBAL(h)));
                }
            }
        }
    }

    /// Allocate a handle holding a copy of `bytes`.
    pub fn alloc_copy(&self, bytes: &[u8]) -> Option<TW_HANDLE> {
        let h = self.alloc(bytes.len().max(1));
        if h.is_null() {
            return None;
        }
        // SAFETY: freshly allocated handle of at least bytes.len() bytes.
        unsafe {
            let p = self.lock(h);
            if p.is_null() {
                self.free(h);
                return None;
            }
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len());
            self.unlock(h);
        }
        Some(h)
    }
}
