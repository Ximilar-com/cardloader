//! The TWAIN state machine. Everything in here runs on the main (TWAIN) thread;
//! the stdin reader only queues commands and flips the stop flag.
//!
//! TWAIN states used below (spec ch. 2): 1 nothing loaded, 3 DSM open,
//! 4 source open (capability negotiation), 5 source enabled (waiting for
//! MSG_XFERREADY), 6 transfer ready, 7 transferring.

use crate::caps::{self, current_value, values_match};
use crate::dib;
use crate::dsm::{Dsm, Mem};
use crate::protocol::{
    emit, log, parse_command, ColorMode, Command, Device, ErrorCode, Event, Multifeed, ScanRequest,
    Side,
};
use crate::twain_ffi::*;
use crate::window::{self, Pump};
use serde_json::{json, Map, Value};
use std::ffi::c_void;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use windows::Win32::Foundation::{GlobalFree, HGLOBAL, HWND};
use windows::Win32::Graphics::Gdi::BITMAPINFOHEADER;
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
use windows::Win32::UI::WindowsAndMessaging::MSG;

/// Entry point of the Windows build; returns the process exit code.
pub fn run() -> i32 {
    // Some drivers use COM/OLE internally and expect an STA thread.
    // SAFETY: called once on the main thread before any window or DSM use.
    if let Err(e) = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.ok() {
        log!("CoInitializeEx failed: {e}");
    }
    let hwnd = match window::create_hidden_window() {
        Ok(h) => h,
        Err(e) => {
            emit(&Event::error(
                ErrorCode::TwainError,
                format!("cannot create window: {e}"),
                true,
            ));
            return 1;
        }
    };
    let stop = Arc::new(AtomicBool::new(false));
    let (tx, rx) = channel();
    spawn_stdin_reader(tx, stop.clone());
    emit(&Event::Ready {
        version: env!("CARGO_PKG_VERSION").into(),
        arch: crate::protocol::arch().into(),
    });
    let mut ctl = Controller {
        hwnd,
        rx,
        stop,
        dsm: None,
        conn: None,
        job: Job::Idle,
        quitting: false,
    };
    window::run_loop(&mut ctl);
    drop(ctl); // closes any open source/DSM
               // SAFETY: balances CoInitializeEx above.
    unsafe { CoUninitialize() };
    0
}

fn spawn_stdin_reader(tx: Sender<Command>, stop: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if line.trim().is_empty() {
                continue;
            }
            match parse_command(&line) {
                Ok(cmd) => {
                    // The main thread may be busy inside a transfer loop and not
                    // reading the channel, so stop/quit also raise the flag directly.
                    if matches!(cmd, Command::Stop | Command::Quit) {
                        stop.store(true, Ordering::SeqCst);
                    }
                    if tx.send(cmd).is_err() {
                        return;
                    }
                    window::post_wake();
                }
                Err(e) => emit(&Event::error(ErrorCode::BadCommand, e, false)),
            }
        }
        // EOF: the parent went away; finish (stop any scan) and exit.
        stop.store(true, Ordering::SeqCst);
        let _ = tx.send(Command::Quit);
        window::post_wake();
    });
}

// ------------------------------------------------------------------ errors

#[derive(Debug)]
struct TwErr {
    code: ErrorCode,
    message: String,
}

type TwResult<T> = Result<T, TwErr>;

fn err(code: ErrorCode, message: impl Into<String>) -> TwErr {
    TwErr {
        code,
        message: message.into(),
    }
}

fn map_condition(cc: u16) -> ErrorCode {
    match cc {
        TWCC_PAPERJAM => ErrorCode::PaperJam,
        TWCC_PAPERDOUBLEFEED => ErrorCode::DoubleFeed,
        TWCC_NOMEDIA => ErrorCode::NoMedia,
        TWCC_NODS => ErrorCode::NoDevice,
        TWCC_CHECKDEVICEONLINE => ErrorCode::DeviceOffline,
        _ => ErrorCode::TwainError,
    }
}

fn condition_name(cc: u16) -> String {
    let name = match cc {
        TWCC_SUCCESS => "TWCC_SUCCESS",
        TWCC_BUMMER => "TWCC_BUMMER",
        TWCC_LOWMEMORY => "TWCC_LOWMEMORY",
        TWCC_NODS => "TWCC_NODS",
        TWCC_MAXCONNECTIONS => "TWCC_MAXCONNECTIONS",
        TWCC_OPERATIONERROR => "TWCC_OPERATIONERROR",
        TWCC_BADCAP => "TWCC_BADCAP",
        TWCC_BADPROTOCOL => "TWCC_BADPROTOCOL",
        TWCC_BADVALUE => "TWCC_BADVALUE",
        TWCC_SEQERROR => "TWCC_SEQERROR",
        TWCC_BADDEST => "TWCC_BADDEST",
        TWCC_CAPUNSUPPORTED => "TWCC_CAPUNSUPPORTED",
        TWCC_CAPBADOPERATION => "TWCC_CAPBADOPERATION",
        TWCC_CAPSEQERROR => "TWCC_CAPSEQERROR",
        TWCC_DENIED => "TWCC_DENIED",
        TWCC_PAPERJAM => "TWCC_PAPERJAM",
        TWCC_PAPERDOUBLEFEED => "TWCC_PAPERDOUBLEFEED",
        TWCC_CHECKDEVICEONLINE => "TWCC_CHECKDEVICEONLINE",
        TWCC_INTERLOCK => "TWCC_INTERLOCK",
        TWCC_NOMEDIA => "TWCC_NOMEDIA",
        _ => return format!("TWCC {cc}"),
    };
    name.into()
}

// -------------------------------------------------------------- connection

/// An open DSM (state >= 3) and possibly an open source (state >= 4).
/// Dropping it walks the source back down to state 1.
struct Conn {
    dsm: Rc<Dsm>,
    mem: Mem,
    app: Box<TW_IDENTITY>,
    src: Box<TW_IDENTITY>,
    hwnd: HWND,
    state: u8,
    callbacks: bool,
}

impl Conn {
    fn open(dsm: Rc<Dsm>, hwnd: HWND) -> TwResult<Conn> {
        let ver = env!("CARGO_PKG_VERSION").split('.').collect::<Vec<_>>();
        let mut app = Box::new(TW_IDENTITY {
            Id: 0, // assigned by the DSM
            Version: TW_VERSION {
                MajorNum: ver[0].parse().unwrap_or(0),
                MinorNum: ver.get(1).and_then(|v| v.parse().ok()).unwrap_or(0),
                Language: TWLG_ENGLISH_USA,
                Country: TWCY_USA,
                Info: str32(env!("CARGO_PKG_VERSION")),
            },
            ProtocolMajor: TWON_PROTOCOLMAJOR,
            ProtocolMinor: TWON_PROTOCOLMINOR,
            SupportedGroups: DF_APP2 | DG_CONTROL | DG_IMAGE,
            Manufacturer: str32("Ximilar"),
            ProductFamily: str32("cardloader"),
            ProductName: str32("cardloader-twain"),
        });
        let mut parent = hwnd.0;
        // SAFETY: MSG_OPENDSM takes a pointer to the parent HWND on Windows.
        let rc = unsafe {
            dsm.entry(
                &mut *app,
                std::ptr::null_mut(),
                DG_CONTROL,
                DAT_PARENT,
                MSG_OPENDSM,
                &mut parent as *mut _ as *mut c_void,
            )
        };
        if rc != TWRC_SUCCESS {
            return Err(err(
                ErrorCode::TwainError,
                format!("MSG_OPENDSM failed (rc={rc})"),
            ));
        }
        let mut conn = Conn {
            dsm,
            mem: Mem::default(),
            app,
            src: Box::default(),
            hwnd,
            state: 3,
            callbacks: false,
        };
        // A 2.x DSM sets DF_DSM2 in our identity and must then provide its allocator.
        if conn.app.SupportedGroups & DF_DSM2 != 0 {
            let mut ep = TW_ENTRYPOINT {
                Size: std::mem::size_of::<TW_ENTRYPOINT>() as u32,
                ..Default::default()
            };
            if conn.call(false, DG_CONTROL, DAT_ENTRYPOINT, MSG_GET, &mut ep) == TWRC_SUCCESS {
                conn.mem = Mem::with_entrypoint(ep);
            }
        }
        Ok(conn)
    }

    /// One DSM_Entry call; `to_source` selects the open source as destination.
    fn call<T>(&mut self, to_source: bool, dg: u32, dat: u16, msg: u16, data: &mut T) -> u16 {
        let dest: *mut TW_IDENTITY = if to_source {
            &mut *self.src
        } else {
            std::ptr::null_mut()
        };
        // SAFETY: callers pass the structure matching (dg, dat, msg).
        unsafe {
            self.dsm.entry(
                &mut *self.app,
                dest,
                dg,
                dat,
                msg,
                data as *mut T as *mut c_void,
            )
        }
    }

    fn ds<T>(&mut self, dg: u32, dat: u16, msg: u16, data: &mut T) -> u16 {
        self.call(true, dg, dat, msg, data)
    }

    /// DAT_STATUS: must be queried right after a failure (it is reset by the next call).
    fn condition(&mut self) -> u16 {
        let mut st = TW_STATUS::default();
        let to_source = self.state >= 4;
        self.call(to_source, DG_CONTROL, DAT_STATUS, MSG_GET, &mut st);
        st.ConditionCode
    }

    fn fail(&mut self, what: &str, rc: u16) -> TwErr {
        let cc = self.condition();
        err(
            map_condition(cc),
            format!("{what} failed (rc={rc}, {})", condition_name(cc)),
        )
    }

    fn sources(&mut self) -> Vec<TW_IDENTITY> {
        let mut out = Vec::new();
        let mut msg = MSG_GETFIRST;
        loop {
            let mut id = TW_IDENTITY::default();
            if self.call(false, DG_CONTROL, DAT_IDENTITY, msg, &mut id) != TWRC_SUCCESS {
                break; // TWRC_ENDOFLIST (or failure)
            }
            out.push(id);
            msg = MSG_GETNEXT;
        }
        out
    }

    fn select(&mut self, query: Option<&str>) -> TwResult<TW_IDENTITY> {
        let all = self.sources();
        let product = |id: &TW_IDENTITY| from_str32(&id.ProductName).to_lowercase();
        if let Some(q) = query.map(str::trim).filter(|q| !q.is_empty()) {
            let q = q.to_lowercase();
            return all
                .iter()
                .find(|id| product(id).contains(&q))
                .copied()
                .ok_or_else(|| {
                    let names: Vec<_> = all.iter().map(|i| from_str32(&i.ProductName)).collect();
                    err(
                        ErrorCode::NoDevice,
                        format!("no TWAIN source matches {q:?} (available: {names:?})"),
                    )
                });
        }
        if let Some(id) = all.iter().find(|id| product(id).contains("fi-8")) {
            return Ok(*id);
        }
        let mut id = TW_IDENTITY::default();
        if self.call(false, DG_CONTROL, DAT_IDENTITY, MSG_GETDEFAULT, &mut id) == TWRC_SUCCESS {
            return Ok(id);
        }
        all.first()
            .copied()
            .ok_or_else(|| err(ErrorCode::NoDevice, "no TWAIN sources installed"))
    }

    fn open_source(&mut self, id: TW_IDENTITY) -> TwResult<()> {
        *self.src = id;
        let src: *mut TW_IDENTITY = &mut *self.src;
        // SAFETY: MSG_OPENDS takes the source identity as pData (pDest is NULL).
        let rc = unsafe {
            self.dsm.entry(
                &mut *self.app,
                std::ptr::null_mut(),
                DG_CONTROL,
                DAT_IDENTITY,
                MSG_OPENDS,
                src as *mut c_void,
            )
        };
        if rc != TWRC_SUCCESS {
            let e = self.fail("MSG_OPENDS", rc);
            return Err(err(
                if e.code == ErrorCode::TwainError {
                    ErrorCode::NoDevice
                } else {
                    e.code
                },
                e.message,
            ));
        }
        self.state = 4;
        // TWAIN 2: if both DSM and source are 2.x, prefer the callback over
        // filtering every window message through MSG_PROCESSEVENT.
        if self.app.SupportedGroups & DF_DSM2 != 0 && self.src.SupportedGroups & DF_DS2 != 0 {
            let mut cb = TW_CALLBACK2 {
                CallBackProc: window::twain_callback as TWAINCALLBACKPROC as *mut c_void,
                RefCon: 0,
                Message: 0,
            };
            let rc = self.ds(DG_CONTROL, DAT_CALLBACK2, MSG_REGISTER_CALLBACK, &mut cb);
            self.callbacks = rc == TWRC_SUCCESS;
            if !self.callbacks {
                let cc = self.condition();
                log!(
                    "DAT_CALLBACK2 registration failed ({}); using MSG_PROCESSEVENT",
                    condition_name(cc)
                );
            }
        }
        log!(
            "opened source {:?} (callbacks: {})",
            from_str32(&self.src.ProductName),
            self.callbacks
        );
        Ok(())
    }

    fn get_cap(&mut self, cap: u16, msg: u16) -> TwResult<Value> {
        let mut c = TW_CAPABILITY {
            Cap: cap,
            ConType: TWON_DONTCARE16,
            hContainer: std::ptr::null_mut(),
        };
        let rc = self.ds(DG_CONTROL, DAT_CAPABILITY, msg, &mut c);
        if rc != TWRC_SUCCESS {
            return Err(self.fail(&format!("{} get", caps::cap_name(cap)), rc));
        }
        let (con_type, h) = (c.ConType, c.hContainer);
        if h.is_null() {
            return Err(err(ErrorCode::TwainError, "source returned no container"));
        }
        // SAFETY: the source allocated the container; we own it now and must free it.
        let result = unsafe {
            let p = self.mem.lock(h);
            let r = if p.is_null() {
                Err("cannot lock container".to_string())
            } else {
                // Every container is at least 6 bytes; that header gives the full length.
                let header = std::slice::from_raw_parts(p, 6);
                match caps::container_len(con_type, header) {
                    Some(len) => {
                        caps::decode_container(con_type, std::slice::from_raw_parts(p, len))
                    }
                    None => Err(format!("unsupported container {con_type}")),
                }
            };
            self.mem.unlock(h);
            self.mem.free(h);
            r
        };
        result.map_err(|m| err(ErrorCode::TwainError, m))
    }

    fn set_cap(&mut self, cap: u16, con_type: u16, bytes: &[u8]) -> u16 {
        let Some(h) = self.mem.alloc_copy(bytes) else {
            return TWRC_FAILURE;
        };
        let mut c = TW_CAPABILITY {
            Cap: cap,
            ConType: con_type,
            hContainer: h,
        };
        let rc = self.ds(DG_CONTROL, DAT_CAPABILITY, MSG_SET, &mut c);
        // SAFETY: the app owns containers it allocates for MSG_SET.
        unsafe { self.mem.free(h) };
        rc
    }

    fn reset_cap(&mut self, cap: u16) -> u16 {
        let mut c = TW_CAPABILITY {
            Cap: cap,
            ConType: TWON_DONTCARE16,
            hContainer: std::ptr::null_mut(),
        };
        let rc = self.ds(DG_CONTROL, DAT_CAPABILITY, MSG_RESET, &mut c);
        // MSG_RESET returns the new current value in a container we must free.
        // SAFETY: as in get_cap.
        let h = c.hContainer;
        unsafe { self.mem.free(h) };
        rc
    }

    fn query_support(&mut self, cap: u16) -> u32 {
        self.get_cap(cap, MSG_QUERYSUPPORT)
            .ok()
            .and_then(|v| current_value(&v).as_u64())
            .unwrap_or(0) as u32
    }

    /// MSG_SET a ONEVALUE, read back MSG_GETCURRENT and report a `cap` event.
    fn negotiate(&mut self, cap: u16, ty: u16, requested: Value) -> Value {
        let rc = match caps::encode_onevalue(ty, &requested) {
            Ok(bytes) => self.set_cap(cap, TWON_ONEVALUE, &bytes),
            Err(e) => {
                log!("{e}");
                TWRC_FAILURE
            }
        };
        // TWRC_CHECKSTATUS = "accepted, but changed to the closest supported value".
        let set_ok = rc == TWRC_SUCCESS || rc == TWRC_CHECKSTATUS;
        if !set_ok {
            let e = self.fail(&format!("{} set", caps::cap_name(cap)), rc);
            log!("{}", e.message);
        }
        let actual = self
            .get_cap(cap, MSG_GETCURRENT)
            .map(|v| current_value(&v))
            .unwrap_or(Value::Null);
        let ok = set_ok && (actual.is_null() || values_match(&requested, &actual));
        emit(&Event::Cap {
            name: caps::cap_name(cap),
            requested,
            actual: actual.clone(),
            ok,
        });
        actual
    }

    fn set_layout(&mut self, width_in: f64, height_in: f64) {
        let mut lay = TW_IMAGELAYOUT::default();
        self.ds(DG_IMAGE, DAT_IMAGELAYOUT, MSG_GET, &mut lay); // keep Document/Page/FrameNumber
        lay.Frame = TW_FRAME {
            Left: TW_FIX32::from_f64(0.0),
            Top: TW_FIX32::from_f64(0.0),
            Right: TW_FIX32::from_f64(width_in),
            Bottom: TW_FIX32::from_f64(height_in),
        };
        let rc = self.ds(DG_IMAGE, DAT_IMAGELAYOUT, MSG_SET, &mut lay);
        let set_ok = rc == TWRC_SUCCESS || rc == TWRC_CHECKSTATUS;
        if !set_ok {
            self.condition();
        }
        let mut got = TW_IMAGELAYOUT::default();
        let actual = if self.ds(DG_IMAGE, DAT_IMAGELAYOUT, MSG_GET, &mut got) == TWRC_SUCCESS {
            let f = got.Frame;
            json!({"left": f.Left.to_f64(), "top": f.Top.to_f64(), "right": f.Right.to_f64(), "bottom": f.Bottom.to_f64()})
        } else {
            Value::Null
        };
        let requested = json!({"left": 0.0, "top": 0.0, "right": width_in, "bottom": height_in});
        let ok = set_ok && (actual.is_null() || values_match(&requested, &actual));
        emit(&Event::Cap {
            name: "DAT_IMAGELAYOUT".into(),
            requested,
            actual,
            ok,
        });
    }

    fn get_custom_data(&mut self) -> TwResult<Vec<u8>> {
        let mut cd = TW_CUSTOMDSDATA {
            InfoLength: 0,
            hData: std::ptr::null_mut(),
        };
        let rc = self.ds(DG_CONTROL, DAT_CUSTOMDSDATA, MSG_GET, &mut cd);
        if rc != TWRC_SUCCESS {
            return Err(self.fail("DAT_CUSTOMDSDATA get", rc));
        }
        let (len, h) = (cd.InfoLength as usize, cd.hData);
        // SAFETY: the source hands us a handle of InfoLength bytes that we must free.
        unsafe {
            let p = self.mem.lock(h);
            let bytes = if p.is_null() {
                Vec::new()
            } else {
                std::slice::from_raw_parts(p, len).to_vec()
            };
            self.mem.unlock(h);
            self.mem.free(h);
            if bytes.is_empty() {
                return Err(err(
                    ErrorCode::TwainError,
                    "source returned empty custom data",
                ));
            }
            Ok(bytes)
        }
    }

    fn set_custom_data(&mut self, bytes: &[u8]) -> TwResult<()> {
        let h = self
            .mem
            .alloc_copy(bytes)
            .ok_or_else(|| err(ErrorCode::TwainError, "out of memory"))?;
        let mut cd = TW_CUSTOMDSDATA {
            InfoLength: bytes.len() as u32,
            hData: h,
        };
        let rc = self.ds(DG_CONTROL, DAT_CUSTOMDSDATA, MSG_SET, &mut cd);
        // SAFETY: we allocated it.
        unsafe { self.mem.free(h) };
        if rc != TWRC_SUCCESS {
            return Err(self.fail("DAT_CUSTOMDSDATA set", rc));
        }
        Ok(())
    }

    fn user_interface(&mut self, msg: u16, show: bool) -> u16 {
        let mut ui = TW_USERINTERFACE {
            ShowUI: show as u16,
            ModalUI: 0,
            hParent: self.hwnd.0,
        };
        self.ds(DG_CONTROL, DAT_USERINTERFACE, msg, &mut ui)
    }

    /// DAT_PENDINGXFERS/MSG_RESET: drop all pending images, back to state 5.
    fn reset_pending(&mut self) {
        let mut px = TW_PENDINGXFERS::default();
        self.ds(DG_CONTROL, DAT_PENDINGXFERS, MSG_RESET, &mut px);
        self.state = 5;
    }

    /// TWEI_PAGESIDE (valid in state 7, i.e. after the transfer, before ENDXFER).
    fn page_side(&mut self) -> Option<Side> {
        let info = TW_INFO {
            InfoID: TWEI_PAGESIDE,
            ItemType: TWTY_UINT16,
            ..Default::default()
        };
        let mut ext = TW_EXTIMAGEINFO {
            NumInfos: 1,
            Info: [info],
        };
        let rc = self.ds(DG_IMAGE, DAT_EXTIMAGEINFO, MSG_GET, &mut ext);
        if rc != TWRC_SUCCESS {
            self.condition();
            return None;
        }
        let info = ext.Info[0];
        let (code, n, item) = (info.ReturnCode, info.NumItems, info.Item);
        if code != TWRC_SUCCESS || n == 0 {
            return None;
        }
        if n > 1 {
            // Multiple values live in a handle the application must free.
            // SAFETY: per spec the app owns that handle.
            unsafe { self.mem.free(item as TW_HANDLE) };
            return None;
        }
        match item as u16 {
            TWCS_TOP => Some(Side::Front),
            TWCS_BOTTOM => Some(Side::Back),
            _ => None,
        }
    }

    fn identity_json(&self) -> Value {
        let s = &*self.src;
        let v = s.Version;
        json!({
            "id": from_str32(&s.ProductName),
            "manufacturer": from_str32(&s.Manufacturer),
            "product_family": from_str32(&s.ProductFamily),
            "product_name": from_str32(&s.ProductName),
            "version": format!("{}.{} {}", { v.MajorNum }, { v.MinorNum }, from_str32(&v.Info)),
            "protocol": format!("{}.{}", { s.ProtocolMajor }, { s.ProtocolMinor }),
            "supported_groups": format!("0x{:08x}", { s.SupportedGroups }),
            "callbacks": self.callbacks,
            "dsm": self.dsm.path,
        })
    }

    /// Walk back down to state 1 from wherever we are.
    fn close(&mut self) {
        if self.state == 7 {
            let mut px = TW_PENDINGXFERS::default();
            self.ds(DG_CONTROL, DAT_PENDINGXFERS, MSG_ENDXFER, &mut px);
            self.state = 6;
        }
        if self.state == 6 {
            self.reset_pending();
        }
        if self.state == 5 {
            self.user_interface(MSG_DISABLEDS, false);
            self.state = 4;
        }
        if self.state == 4 {
            let src: *mut TW_IDENTITY = &mut *self.src;
            // SAFETY: MSG_CLOSEDS takes the source identity as pData.
            unsafe {
                self.dsm.entry(
                    &mut *self.app,
                    std::ptr::null_mut(),
                    DG_CONTROL,
                    DAT_IDENTITY,
                    MSG_CLOSEDS,
                    src as *mut c_void,
                )
            };
            self.state = 3;
            self.callbacks = false;
        }
        if self.state == 3 {
            let mut parent = self.hwnd.0;
            self.call(false, DG_CONTROL, DAT_PARENT, MSG_CLOSEDSM, &mut parent);
            self.state = 2;
        }
    }
}

impl Drop for Conn {
    fn drop(&mut self) {
        self.close();
    }
}

/// Lock a native-transfer DIB, decode it, and free it. On Windows the spec
/// defines this handle as a global-memory DIB (GlobalAlloc) owned by the
/// application after TWRC_XFERDONE.
fn take_native_image(h: TW_HANDLE) -> Result<dib::Image, String> {
    if h.is_null() {
        return Err("source returned a null image handle".into());
    }
    let hg = HGLOBAL(h);
    // SAFETY: see above; we lock, copy out and free exactly once.
    unsafe {
        let p = GlobalLock(hg) as *const u8;
        let result = if p.is_null() {
            Err("cannot lock image handle".to_string())
        } else {
            let mut len = GlobalSize(hg);
            if len == 0 {
                // Should not happen for a global handle; derive the size from the header.
                let hdr: BITMAPINFOHEADER = std::ptr::read_unaligned(p as *const BITMAPINFOHEADER);
                let bpp = hdr.biBitCount as usize;
                let stride = (hdr.biWidth.unsigned_abs() as usize * bpp).div_ceil(32) * 4;
                let colors = match (bpp, hdr.biClrUsed) {
                    (0..=8, 0) => 1 << bpp,
                    (0..=8, n) => n as usize,
                    _ => 0,
                };
                let masks = if hdr.biCompression == 3 { 12 } else { 0 }; // BI_BITFIELDS
                len = hdr.biSize as usize
                    + colors * 4
                    + masks
                    + stride * hdr.biHeight.unsigned_abs() as usize;
            }
            dib::decode_dib(std::slice::from_raw_parts(p, len))
        };
        let _ = GlobalUnlock(hg);
        let _ = GlobalFree(Some(hg));
        result
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

// -------------------------------------------------------------- controller

enum Job {
    Idle,
    Scan(ScanJob),
    Configure {
        settings_path: String,
        supported: bool,
    },
}

struct ScanJob {
    req: ScanRequest,
    out_dir: PathBuf,
    duplex: Option<bool>,
    pages: u32,
    transfers: u32,
    stop_sent: bool,
}

impl ScanJob {
    /// Used when the source cannot report TWEI_PAGESIDE. Breaks if the driver
    /// drops blank sides, hence only a fallback.
    fn guessed_side(&self) -> Side {
        match self.duplex {
            Some(true) if self.transfers.is_multiple_of(2) => Side::Front,
            Some(true) => Side::Back,
            Some(false) => Side::Front,
            None => Side::Unknown,
        }
    }
}

enum Flow {
    Continue,
    Finished,
}

struct Controller {
    hwnd: HWND,
    rx: Receiver<Command>,
    stop: Arc<AtomicBool>,
    dsm: Option<Rc<Dsm>>,
    conn: Option<Conn>,
    job: Job,
    quitting: bool,
}

impl Pump for Controller {
    fn pre_translate(&mut self, msg: &MSG) -> bool {
        let Some(conn) = self.conn.as_mut() else {
            return false;
        };
        if conn.state < 5 || conn.callbacks {
            return false;
        }
        // Pre-TWAIN-2 path: every message goes through the source first.
        let mut ev = TW_EVENT {
            pEvent: msg as *const MSG as *mut c_void,
            TWMessage: MSG_NULL,
        };
        let rc = conn.ds(DG_CONTROL, DAT_EVENT, MSG_PROCESSEVENT, &mut ev);
        let twain_msg = ev.TWMessage;
        if twain_msg != MSG_NULL {
            self.on_twain_message(twain_msg);
        }
        rc == TWRC_DSEVENT
    }

    fn after_message(&mut self) -> bool {
        for m in window::take_twain_messages() {
            self.on_twain_message(m);
        }
        while let Ok(cmd) = self.rx.try_recv() {
            self.on_command(cmd);
        }
        !(self.quitting && self.conn.is_none())
    }
}

impl Controller {
    fn busy(&self) -> bool {
        self.conn.is_some()
    }

    fn dsm(&mut self) -> TwResult<Rc<Dsm>> {
        if self.dsm.is_none() {
            let dsm = Dsm::load().map_err(|m| err(ErrorCode::DsmNotFound, m))?;
            self.dsm = Some(Rc::new(dsm));
        }
        Ok(self.dsm.clone().expect("loaded above"))
    }

    fn open_device(&mut self, device: Option<&str>) -> TwResult<Conn> {
        let mut conn = Conn::open(self.dsm()?, self.hwnd)?;
        let id = conn.select(device)?;
        conn.open_source(id)?;
        emit(&Event::status(format!(
            "opened {}",
            from_str32(&conn.src.ProductName)
        )));
        Ok(conn)
    }

    fn on_command(&mut self, cmd: Command) {
        let rejected = !matches!(cmd, Command::Stop | Command::Quit) && self.busy();
        if rejected {
            emit(&Event::error(
                ErrorCode::Busy,
                "another scan/configure session is active",
                false,
            ));
            return;
        }
        match cmd {
            Command::List => self.list(),
            Command::Probe { device } => {
                if let Err(e) = self.probe(device.as_deref()) {
                    emit(&Event::error(e.code, e.message, true));
                }
            }
            Command::Scan(req) => self.start_scan(req),
            Command::Configure {
                device,
                settings_path,
            } => {
                if let Err(e) = self.start_configure(device.as_deref(), settings_path) {
                    emit(&Event::error(e.code, e.message, true));
                }
            }
            Command::Stop => {
                self.stop.store(true, Ordering::SeqCst);
                self.cancel_waiting();
            }
            Command::Quit => {
                self.quitting = true;
                self.stop.store(true, Ordering::SeqCst);
                self.cancel_waiting();
            }
        }
    }

    /// Stop a session that is idle in state 5 (no transfer loop running).
    fn cancel_waiting(&mut self) {
        match self.job {
            Job::Scan(_) => self.finish_scan(),
            Job::Configure { .. } => self.finish_configure(false),
            Job::Idle => {}
        }
    }

    fn list(&mut self) {
        let result = self.dsm().and_then(|dsm| Conn::open(dsm, self.hwnd));
        let devices = match result {
            Ok(mut conn) => conn
                .sources()
                .iter()
                .map(|id| {
                    let product = from_str32(&id.ProductName);
                    let name = format!("{} {}", from_str32(&id.Manufacturer), product)
                        .trim()
                        .to_string();
                    Device {
                        id: product,
                        name,
                        backend: "twain".into(),
                    }
                })
                .collect(),
            Err(e) => {
                emit(&Event::error(e.code, e.message, true));
                Vec::new()
            }
        };
        emit(&Event::Devices { devices });
    }

    fn probe(&mut self, device: Option<&str>) -> TwResult<()> {
        let mut conn = self.open_device(device)?;
        let mut caps_out = Map::new();
        let supported = conn.get_cap(CAP_SUPPORTEDCAPS, MSG_GET);
        if let Ok(v) = &supported {
            let names: Vec<String> = current_value(v)
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|c| c.as_u64())
                        .map(|c| caps::cap_name(c as u16))
                        .collect()
                })
                .unwrap_or_default();
            caps_out.insert(
                "CAP_SUPPORTEDCAPS".into(),
                json!({"container": v, "names": names}),
            );
        } else if let Err(e) = &supported {
            caps_out.insert("CAP_SUPPORTEDCAPS".into(), json!({"error": e.message}));
        }
        for &cap in caps::PROBE_CAPS {
            let qs = conn.query_support(cap);
            let mut entry = match conn.get_cap(cap, MSG_GET) {
                Ok(v) => v,
                Err(e) => json!({"error": e.message}),
            };
            entry["query_support"] = json!(caps::query_support_names(qs));
            caps_out.insert(caps::cap_name(cap), entry);
        }
        let ds2 = conn.src.SupportedGroups & DF_DS2 != 0;
        emit(&Event::Probe {
            source: conn.identity_json(),
            ds2,
            caps: caps_out,
        });
        Ok(())
    }

    // ---- scan

    fn start_scan(&mut self, req: ScanRequest) {
        self.stop.store(false, Ordering::SeqCst);
        match self.setup_scan(&req) {
            Ok((conn, duplex)) => {
                self.conn = Some(conn);
                self.job = Job::Scan(ScanJob {
                    out_dir: PathBuf::from(&req.out_dir),
                    req,
                    duplex,
                    pages: 0,
                    transfers: 0,
                    stop_sent: false,
                });
                emit(&Event::status("scanning"));
            }
            Err(e) => {
                // An empty feeder is an expected outcome, not a failure of the session.
                let fatal = e.code != ErrorCode::NoMedia;
                emit(&Event::error(e.code, e.message, fatal));
                emit(&Event::Done { pages: 0 });
            }
        }
    }

    /// States 3 -> 5: open, negotiate, enable. Returns the duplex state actually in effect.
    fn setup_scan(&mut self, req: &ScanRequest) -> TwResult<(Conn, Option<bool>)> {
        std::fs::create_dir_all(&req.out_dir)
            .map_err(|e| err(ErrorCode::Io, format!("cannot create {}: {e}", req.out_dir)))?;
        let mut conn = self.open_device(req.device.as_deref())?;

        // Replay saved driver settings first so explicit parameters below win.
        if let Some(path) = &req.settings_path {
            match std::fs::read(path) {
                Ok(bytes) => match conn.set_custom_data(&bytes) {
                    Ok(()) => emit(&Event::status(format!(
                        "applied driver settings from {path}"
                    ))),
                    Err(e) => emit(&Event::error(e.code, e.message, false)),
                },
                Err(e) => emit(&Event::error(
                    ErrorCode::Io,
                    format!("cannot read {path}: {e}"),
                    false,
                )),
            }
        }

        conn.negotiate(ICAP_XFERMECH, TWTY_UINT16, json!(TWSX_NATIVE));
        conn.negotiate(CAP_FEEDERENABLED, TWTY_BOOL, json!(true));
        conn.negotiate(CAP_AUTOFEED, TWTY_BOOL, json!(true));
        let duplex = conn
            .negotiate(CAP_DUPLEXENABLED, TWTY_BOOL, json!(req.duplex))
            .as_bool();
        conn.negotiate(CAP_XFERCOUNT, TWTY_INT16, json!(-1));
        conn.negotiate(CAP_INDICATORS, TWTY_BOOL, json!(false));
        // Units before anything measured in them (resolution, layout).
        conn.negotiate(ICAP_UNITS, TWTY_UINT16, json!(TWUN_INCHES));
        let (pixel_type, depth) = match req.color {
            ColorMode::Color => (TWPT_RGB, 24),
            ColorMode::Gray => (TWPT_GRAY, 8),
            ColorMode::Bw => (TWPT_BW, 1),
        };
        conn.negotiate(ICAP_PIXELTYPE, TWTY_UINT16, json!(pixel_type));
        conn.negotiate(ICAP_BITDEPTH, TWTY_UINT16, json!(depth));
        conn.negotiate(ICAP_XRESOLUTION, TWTY_FIX32, json!(req.dpi));
        conn.negotiate(ICAP_YRESOLUTION, TWTY_FIX32, json!(req.dpi));
        if let (Some(w), Some(h)) = (req.page_width_mm, req.page_height_mm) {
            // ICAP_SUPPORTEDSIZES must be TWSS_NONE or the frame is ignored.
            conn.negotiate(ICAP_SUPPORTEDSIZES, TWTY_UINT16, json!(TWSS_NONE));
            conn.set_layout(w / 25.4, h / 25.4);
        }
        if req.multifeed == Multifeed::Off {
            self.disable_double_feed(&mut conn);
        }

        let loaded = conn
            .get_cap(CAP_FEEDERLOADED, MSG_GETCURRENT)
            .map(|v| current_value(&v));
        if let Ok(Value::Bool(false)) = loaded {
            return Err(err(ErrorCode::NoMedia, "the document feeder is empty"));
        }

        let rc = conn.user_interface(MSG_ENABLEDS, false);
        // TWRC_CHECKSTATUS: the source could not hide its UI and shows it anyway.
        if rc != TWRC_SUCCESS && rc != TWRC_CHECKSTATUS {
            return Err(conn.fail("MSG_ENABLEDS", rc));
        }
        conn.state = 5;
        Ok((conn, duplex))
    }

    /// CAP_DOUBLEFEEDDETECTION is an array of active sensors (TWDF_*); an empty
    /// array means "off". Not every driver honours it; vendor caps or the
    /// driver profile may still override it.
    fn disable_double_feed(&mut self, conn: &mut Conn) {
        let qs = conn.query_support(CAP_DOUBLEFEEDDETECTION);
        let mut ok = false;
        if qs & TWQC_SET != 0 {
            let bytes = caps::encode_array(TWTY_UINT16, &[]).expect("empty array encodes");
            let rc = conn.set_cap(CAP_DOUBLEFEEDDETECTION, TWON_ARRAY, &bytes);
            ok = rc == TWRC_SUCCESS || rc == TWRC_CHECKSTATUS;
            if !ok {
                conn.condition();
            }
        }
        if !ok && qs & TWQC_RESET != 0 {
            ok = conn.reset_cap(CAP_DOUBLEFEEDDETECTION) == TWRC_SUCCESS;
        }
        let actual = if qs & (TWQC_GET | TWQC_GETCURRENT) != 0 {
            conn.get_cap(CAP_DOUBLEFEEDDETECTION, MSG_GETCURRENT)
                .map(|v| current_value(&v))
                .unwrap_or(Value::Null)
        } else {
            Value::Null
        };
        let off = matches!(&actual, Value::Array(a) if a.is_empty());
        emit(&Event::Cap {
            name: "CAP_DOUBLEFEEDDETECTION".into(),
            requested: json!([]),
            actual,
            ok: ok && off,
        });
    }

    fn on_twain_message(&mut self, msg: u16) {
        if self.conn.is_none() {
            return;
        }
        match msg {
            MSG_XFERREADY => {
                if matches!(self.job, Job::Scan(_))
                    && self.conn.as_ref().is_some_and(|c| c.state == 5)
                {
                    self.transfer_all();
                    self.finish_scan();
                }
            }
            // CLOSEDSREQ: user cancelled / driver wants to close (e.g. after its own
            // error dialog). CLOSEDSOK: "OK" pressed in ENABLEDSUIONLY mode.
            MSG_CLOSEDSREQ | MSG_CLOSEDSOK => match self.job {
                Job::Scan(_) => self.finish_scan(),
                Job::Configure { .. } => self.finish_configure(msg == MSG_CLOSEDSOK),
                Job::Idle => {}
            },
            MSG_DEVICEEVENT => log!("MSG_DEVICEEVENT ignored"),
            other => log!("unhandled TWAIN message 0x{other:04x}"),
        }
    }

    fn transfer_all(&mut self) {
        let (Some(conn), Job::Scan(job)) = (self.conn.as_mut(), &mut self.job) else {
            return;
        };
        conn.state = 6;
        while let Flow::Continue = transfer_one(conn, job, &self.stop) {}
    }

    fn finish_scan(&mut self) {
        drop(self.conn.take()); // DISABLEDS -> CLOSEDS -> CLOSEDSM
        if let Job::Scan(job) = std::mem::replace(&mut self.job, Job::Idle) {
            emit(&Event::Done { pages: job.pages });
        }
    }

    // ---- configure

    fn start_configure(&mut self, device: Option<&str>, settings_path: String) -> TwResult<()> {
        let mut conn = self.open_device(device)?;
        let supported = conn
            .get_cap(CAP_CUSTOMDSDATA, MSG_GETCURRENT)
            .map(|v| current_value(&v));
        let supported = matches!(supported, Ok(Value::Bool(true)));
        if !supported {
            emit(&Event::error(
                ErrorCode::CustomDsDataUnsupported,
                "source does not support CAP_CUSTOMDSDATA; settings cannot be saved",
                false,
            ));
        } else if let Ok(bytes) = std::fs::read(&settings_path) {
            // Start the dialog from the previously saved settings.
            if let Err(e) = conn.set_custom_data(&bytes) {
                log!("replaying {settings_path} failed: {}", e.message);
            }
        }
        let rc = conn.user_interface(MSG_ENABLEDSUIONLY, true);
        if rc != TWRC_SUCCESS && rc != TWRC_CHECKSTATUS {
            return Err(conn.fail("MSG_ENABLEDSUIONLY", rc));
        }
        conn.state = 5;
        self.conn = Some(conn);
        self.job = Job::Configure {
            settings_path,
            supported,
        };
        emit(&Event::status("driver settings dialog open"));
        Ok(())
    }

    fn finish_configure(&mut self, accepted: bool) {
        let Job::Configure {
            settings_path,
            supported,
        } = std::mem::replace(&mut self.job, Job::Idle)
        else {
            return;
        };
        let Some(mut conn) = self.conn.take() else {
            return;
        };
        if conn.state == 5 {
            conn.user_interface(MSG_DISABLEDS, false);
            conn.state = 4;
        }
        let mut saved = false;
        if accepted && supported {
            // DAT_CUSTOMDSDATA is only valid in state 4, hence after DISABLEDS.
            match conn.get_custom_data() {
                Ok(bytes) => match write_atomic(Path::new(&settings_path), &bytes) {
                    Ok(()) => saved = true,
                    Err(e) => emit(&Event::error(
                        ErrorCode::Io,
                        format!("cannot write {settings_path}: {e}"),
                        false,
                    )),
                },
                Err(e) => emit(&Event::error(e.code, e.message, false)),
            }
        }
        drop(conn);
        emit(&Event::Configured {
            settings_path,
            saved,
            cancelled: !accepted,
        });
    }
}

/// One image: state 6 -> 7 -> (6 | 5). Spec ch. 4 "Native mode transfer".
fn transfer_one(conn: &mut Conn, job: &mut ScanJob, stop: &AtomicBool) -> Flow {
    let mut info = TW_IMAGEINFO::default();
    let dpi = (conn.ds(DG_IMAGE, DAT_IMAGEINFO, MSG_GET, &mut info) == TWRC_SUCCESS)
        .then(|| info.XResolution.to_f64())
        .or(Some(job.req.dpi as f64));

    let mut handle: TW_HANDLE = std::ptr::null_mut();
    let rc = conn.ds(DG_IMAGE, DAT_IMAGENATIVEXFER, MSG_GET, &mut handle);
    let mut abort = false;
    match rc {
        TWRC_XFERDONE => {
            conn.state = 7;
            let side = conn.page_side().unwrap_or_else(|| job.guessed_side());
            job.transfers += 1;
            let index = job.req.start_index + job.pages;
            let path = job
                .out_dir
                .join(format!("{}_{index:04}.png", job.req.prefix));
            let saved = take_native_image(handle)
                .and_then(|img| dib::write_png_atomic(&path, &img, dpi).map_err(|e| e.to_string()));
            match saved {
                Ok(()) => {
                    job.pages += 1;
                    emit(&Event::Page {
                        path: path.display().to_string(),
                        index,
                        side,
                    });
                }
                Err(e) => {
                    emit(&Event::error(
                        ErrorCode::Io,
                        format!("{}: {e}", path.display()),
                        true,
                    ));
                    abort = true;
                }
            }
        }
        TWRC_CANCEL => {
            // The user cancelled in the source's UI; the transfer still has to be ended.
            conn.state = 7;
            // SAFETY: free whatever the source may have handed over.
            unsafe { conn.mem.free(handle) };
            emit(&Event::error(
                ErrorCode::Cancelled,
                "transfer cancelled by the source",
                false,
            ));
            abort = true;
        }
        _ => {
            // TWRC_FAILURE leaves us in state 6: paper jam, double feed, ...
            let e = conn.fail("DAT_IMAGENATIVEXFER", rc);
            emit(&Event::error(e.code, e.message, true));
            conn.reset_pending();
            return Flow::Finished;
        }
    }

    let mut px = TW_PENDINGXFERS::default();
    let rc = conn.ds(DG_CONTROL, DAT_PENDINGXFERS, MSG_ENDXFER, &mut px);
    if rc != TWRC_SUCCESS {
        let e = conn.fail("MSG_ENDXFER", rc);
        emit(&Event::error(e.code, e.message, true));
        conn.state = 6;
        conn.reset_pending();
        return Flow::Finished;
    }
    if px.Count == 0 {
        conn.state = 5;
        return Flow::Finished;
    }
    // Count may be 0xFFFF ("unknown, more to come") with an ADF.
    conn.state = 6;
    if abort {
        conn.reset_pending();
        return Flow::Finished;
    }
    if stop.load(Ordering::SeqCst) && !job.stop_sent {
        // MSG_STOPFEEDER halts the feeder but lets pages already in the
        // transport arrive, so keep looping until Count reaches 0.
        job.stop_sent = true;
        emit(&Event::status("stopping feeder"));
        let mut px = TW_PENDINGXFERS::default();
        if conn.ds(DG_CONTROL, DAT_PENDINGXFERS, MSG_STOPFEEDER, &mut px) != TWRC_SUCCESS {
            conn.condition();
            conn.reset_pending();
            return Flow::Finished;
        }
    }
    Flow::Continue
}
