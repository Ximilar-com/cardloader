//! Hand-written subset of `twain.h` (TWAIN 2.5).
//!
//! Values were checked against the official header shipped in
//! <https://github.com/twain/twain-dsm> (`TWAIN_DSM/src/twain.h`).
//!
//! Windows `twain.h` wraps everything in `#pragma pack(2)`, hence
//! `#[repr(C, packed(2))]` on every struct. On Windows `TW_UINT32` is a
//! C `unsigned long` (32 bit even on Win64) and `TW_HANDLE`/`TW_MEMREF` are
//! pointer sized. The module is pure data, so it is compiled on every platform
//! to let the layout tests run on CI hosts too (a 64-bit host has the same
//! layout as Win64 for these definitions).
#![allow(
    non_camel_case_types,
    non_snake_case,
    dead_code,
    clippy::upper_case_acronyms
)]

use std::ffi::c_void;

pub type TW_HANDLE = *mut c_void;
pub type TW_MEMREF = *mut c_void;
pub type TW_UINTPTR = usize;
pub type TW_STR32 = [u8; 34];

// ---------------------------------------------------------------- structs

#[repr(C, packed(2))]
#[derive(Clone, Copy, Default, Debug)]
pub struct TW_FIX32 {
    pub Whole: i16,
    pub Frac: u16,
}

impl TW_FIX32 {
    /// Same rounding as `FloatToFIX32` in the TWAIN sample code.
    pub fn from_f64(v: f64) -> Self {
        let raw = (v * 65536.0).round() as i32;
        TW_FIX32 {
            Whole: (raw >> 16) as i16,
            Frac: (raw & 0xffff) as u16,
        }
    }
    pub fn to_f64(self) -> f64 {
        self.Whole as f64 + self.Frac as f64 / 65536.0
    }
    pub fn to_u32(self) -> u32 {
        (self.Whole as u16 as u32) | ((self.Frac as u32) << 16)
    }
}

#[repr(C, packed(2))]
#[derive(Clone, Copy, Default, Debug)]
pub struct TW_FRAME {
    pub Left: TW_FIX32,
    pub Top: TW_FIX32,
    pub Right: TW_FIX32,
    pub Bottom: TW_FIX32,
}

#[repr(C, packed(2))]
#[derive(Clone, Copy, Debug)]
pub struct TW_VERSION {
    pub MajorNum: u16,
    pub MinorNum: u16,
    pub Language: u16,
    pub Country: u16,
    pub Info: TW_STR32,
}

#[repr(C, packed(2))]
#[derive(Clone, Copy, Debug)]
pub struct TW_IDENTITY {
    /// `TW_UINT32` on Windows (it is a `TW_MEMREF` only on macOS).
    pub Id: u32,
    pub Version: TW_VERSION,
    pub ProtocolMajor: u16,
    pub ProtocolMinor: u16,
    pub SupportedGroups: u32,
    pub Manufacturer: TW_STR32,
    pub ProductFamily: TW_STR32,
    pub ProductName: TW_STR32,
}

impl Default for TW_IDENTITY {
    fn default() -> Self {
        // SAFETY: all-zero is a valid TW_IDENTITY (plain integers and byte arrays).
        unsafe { std::mem::zeroed() }
    }
}

#[repr(C, packed(2))]
#[derive(Clone, Copy, Debug)]
pub struct TW_USERINTERFACE {
    pub ShowUI: u16,
    pub ModalUI: u16,
    pub hParent: TW_HANDLE,
}

#[repr(C, packed(2))]
#[derive(Clone, Copy, Debug)]
pub struct TW_CAPABILITY {
    pub Cap: u16,
    pub ConType: u16,
    pub hContainer: TW_HANDLE,
}

#[repr(C, packed(2))]
#[derive(Clone, Copy, Default, Debug)]
pub struct TW_ONEVALUE {
    pub ItemType: u16,
    pub Item: u32,
}

/// Header only; `ItemList` follows at offset 6.
#[repr(C, packed(2))]
#[derive(Clone, Copy, Default, Debug)]
pub struct TW_ARRAY {
    pub ItemType: u16,
    pub NumItems: u32,
}

/// Header only; `ItemList` follows at offset 14.
#[repr(C, packed(2))]
#[derive(Clone, Copy, Default, Debug)]
pub struct TW_ENUMERATION {
    pub ItemType: u16,
    pub NumItems: u32,
    pub CurrentIndex: u32,
    pub DefaultIndex: u32,
}

#[repr(C, packed(2))]
#[derive(Clone, Copy, Default, Debug)]
pub struct TW_RANGE {
    pub ItemType: u16,
    pub MinValue: u32,
    pub MaxValue: u32,
    pub StepSize: u32,
    pub DefaultValue: u32,
    pub CurrentValue: u32,
}

#[repr(C, packed(2))]
#[derive(Clone, Copy, Default, Debug)]
pub struct TW_IMAGELAYOUT {
    pub Frame: TW_FRAME,
    pub DocumentNumber: u32,
    pub PageNumber: u32,
    pub FrameNumber: u32,
}

#[repr(C, packed(2))]
#[derive(Clone, Copy, Default, Debug)]
pub struct TW_IMAGEINFO {
    pub XResolution: TW_FIX32,
    pub YResolution: TW_FIX32,
    pub ImageWidth: i32,
    pub ImageLength: i32,
    pub SamplesPerPixel: i16,
    pub BitsPerSample: [i16; 8],
    pub BitsPerPixel: i16,
    pub Planar: u16,
    pub PixelType: i16,
    pub Compression: u16,
}

#[repr(C, packed(2))]
#[derive(Clone, Copy, Default, Debug)]
pub struct TW_PENDINGXFERS {
    pub Count: u16,
    /// Union of `EOJ` / `Reserved`.
    pub EOJ: u32,
}

#[repr(C, packed(2))]
#[derive(Clone, Copy, Debug)]
pub struct TW_EVENT {
    pub pEvent: TW_MEMREF,
    pub TWMessage: u16,
}

#[repr(C, packed(2))]
#[derive(Clone, Copy, Default, Debug)]
pub struct TW_STATUS {
    pub ConditionCode: u16,
    /// Union of `Data` / `Reserved`.
    pub Data: u16,
}

#[repr(C, packed(2))]
#[derive(Clone, Copy, Debug)]
pub struct TW_CALLBACK2 {
    pub CallBackProc: TW_MEMREF,
    pub RefCon: TW_UINTPTR,
    pub Message: i16,
}

pub type DSMENTRYPROC = unsafe extern "system" fn(
    origin: *mut TW_IDENTITY,
    dest: *mut TW_IDENTITY,
    dg: u32,
    dat: u16,
    msg: u16,
    data: TW_MEMREF,
) -> u16;
/// The application callback has the same shape as `DSM_Entry`.
pub type TWAINCALLBACKPROC = DSMENTRYPROC;
pub type DSM_MEMALLOCATE = unsafe extern "system" fn(size: u32) -> TW_HANDLE;
pub type DSM_MEMFREE = unsafe extern "system" fn(h: TW_HANDLE);
pub type DSM_MEMLOCK = unsafe extern "system" fn(h: TW_HANDLE) -> TW_MEMREF;
pub type DSM_MEMUNLOCK = unsafe extern "system" fn(h: TW_HANDLE);

#[repr(C, packed(2))]
#[derive(Clone, Copy, Default, Debug)]
pub struct TW_ENTRYPOINT {
    pub Size: u32,
    pub DSM_Entry: Option<DSMENTRYPROC>,
    pub DSM_MemAllocate: Option<DSM_MEMALLOCATE>,
    pub DSM_MemFree: Option<DSM_MEMFREE>,
    pub DSM_MemLock: Option<DSM_MEMLOCK>,
    pub DSM_MemUnlock: Option<DSM_MEMUNLOCK>,
}

#[repr(C, packed(2))]
#[derive(Clone, Copy, Debug)]
pub struct TW_CUSTOMDSDATA {
    pub InfoLength: u32,
    pub hData: TW_HANDLE,
}

#[repr(C, packed(2))]
#[derive(Clone, Copy, Default, Debug)]
pub struct TW_INFO {
    pub InfoID: u16,
    pub ItemType: u16,
    pub NumItems: u16,
    /// Union of `ReturnCode` / deprecated `CondCode`.
    pub ReturnCode: u16,
    /// Holds the value itself when it fits, otherwise a handle the app must free.
    pub Item: TW_UINTPTR,
}

/// `TW_EXTIMAGEINFO` with room for exactly one `TW_INFO`.
#[repr(C, packed(2))]
#[derive(Clone, Copy, Default, Debug)]
pub struct TW_EXTIMAGEINFO {
    pub NumInfos: u32,
    pub Info: [TW_INFO; 1],
}

// -------------------------------------------------------------- constants

pub const TWON_PROTOCOLMAJOR: u16 = 2;
pub const TWON_PROTOCOLMINOR: u16 = 5;

// Data groups and feature flags (SupportedGroups).
pub const DG_CONTROL: u32 = 0x0001;
pub const DG_IMAGE: u32 = 0x0002;
pub const DF_DSM2: u32 = 0x1000_0000;
pub const DF_APP2: u32 = 0x2000_0000;
pub const DF_DS2: u32 = 0x4000_0000;

// Data argument types.
pub const DAT_CAPABILITY: u16 = 0x0001;
pub const DAT_EVENT: u16 = 0x0002;
pub const DAT_IDENTITY: u16 = 0x0003;
pub const DAT_PARENT: u16 = 0x0004;
pub const DAT_PENDINGXFERS: u16 = 0x0005;
pub const DAT_STATUS: u16 = 0x0008;
pub const DAT_USERINTERFACE: u16 = 0x0009;
pub const DAT_CUSTOMDSDATA: u16 = 0x000c;
pub const DAT_CALLBACK: u16 = 0x0010;
pub const DAT_CALLBACK2: u16 = 0x0012;
pub const DAT_IMAGEINFO: u16 = 0x0101;
pub const DAT_IMAGELAYOUT: u16 = 0x0102;
pub const DAT_IMAGENATIVEXFER: u16 = 0x0104;
pub const DAT_EXTIMAGEINFO: u16 = 0x010b;
pub const DAT_ENTRYPOINT: u16 = 0x0403;

// Messages.
pub const MSG_NULL: u16 = 0x0000;
pub const MSG_GET: u16 = 0x0001;
pub const MSG_GETCURRENT: u16 = 0x0002;
pub const MSG_GETDEFAULT: u16 = 0x0003;
pub const MSG_GETFIRST: u16 = 0x0004;
pub const MSG_GETNEXT: u16 = 0x0005;
pub const MSG_SET: u16 = 0x0006;
pub const MSG_RESET: u16 = 0x0007;
pub const MSG_QUERYSUPPORT: u16 = 0x0008;
pub const MSG_XFERREADY: u16 = 0x0101;
pub const MSG_CLOSEDSREQ: u16 = 0x0102;
pub const MSG_CLOSEDSOK: u16 = 0x0103;
pub const MSG_DEVICEEVENT: u16 = 0x0104;
pub const MSG_OPENDSM: u16 = 0x0301;
pub const MSG_CLOSEDSM: u16 = 0x0302;
pub const MSG_OPENDS: u16 = 0x0401;
pub const MSG_CLOSEDS: u16 = 0x0402;
pub const MSG_DISABLEDS: u16 = 0x0501;
pub const MSG_ENABLEDS: u16 = 0x0502;
pub const MSG_ENABLEDSUIONLY: u16 = 0x0503;
pub const MSG_PROCESSEVENT: u16 = 0x0601;
pub const MSG_ENDXFER: u16 = 0x0701;
pub const MSG_STOPFEEDER: u16 = 0x0702;
pub const MSG_REGISTER_CALLBACK: u16 = 0x0902;

// Return codes.
pub const TWRC_SUCCESS: u16 = 0;
pub const TWRC_FAILURE: u16 = 1;
pub const TWRC_CHECKSTATUS: u16 = 2;
pub const TWRC_CANCEL: u16 = 3;
pub const TWRC_DSEVENT: u16 = 4;
pub const TWRC_NOTDSEVENT: u16 = 5;
pub const TWRC_XFERDONE: u16 = 6;
pub const TWRC_ENDOFLIST: u16 = 7;
pub const TWRC_INFONOTSUPPORTED: u16 = 8;
pub const TWRC_DATANOTAVAILABLE: u16 = 9;
pub const TWRC_BUSY: u16 = 10;
pub const TWRC_SCANNERLOCKED: u16 = 11;

// Condition codes.
pub const TWCC_SUCCESS: u16 = 0;
pub const TWCC_BUMMER: u16 = 1;
pub const TWCC_LOWMEMORY: u16 = 2;
pub const TWCC_NODS: u16 = 3;
pub const TWCC_MAXCONNECTIONS: u16 = 4;
pub const TWCC_OPERATIONERROR: u16 = 5;
pub const TWCC_BADCAP: u16 = 6;
pub const TWCC_BADPROTOCOL: u16 = 9;
pub const TWCC_BADVALUE: u16 = 10;
pub const TWCC_SEQERROR: u16 = 11;
pub const TWCC_BADDEST: u16 = 12;
pub const TWCC_CAPUNSUPPORTED: u16 = 13;
pub const TWCC_CAPBADOPERATION: u16 = 14;
pub const TWCC_CAPSEQERROR: u16 = 15;
pub const TWCC_DENIED: u16 = 16;
pub const TWCC_PAPERJAM: u16 = 20;
pub const TWCC_PAPERDOUBLEFEED: u16 = 21;
pub const TWCC_CHECKDEVICEONLINE: u16 = 23;
pub const TWCC_INTERLOCK: u16 = 24;
pub const TWCC_NOMEDIA: u16 = 29;

// Container types.
pub const TWON_ARRAY: u16 = 3;
pub const TWON_ENUMERATION: u16 = 4;
pub const TWON_ONEVALUE: u16 = 5;
pub const TWON_RANGE: u16 = 6;
pub const TWON_DONTCARE16: u16 = 0xffff;
pub const TWON_DONTCARE32: u32 = 0xffff_ffff;

// Item types.
pub const TWTY_INT8: u16 = 0x0000;
pub const TWTY_INT16: u16 = 0x0001;
pub const TWTY_INT32: u16 = 0x0002;
pub const TWTY_UINT8: u16 = 0x0003;
pub const TWTY_UINT16: u16 = 0x0004;
pub const TWTY_UINT32: u16 = 0x0005;
pub const TWTY_BOOL: u16 = 0x0006;
pub const TWTY_FIX32: u16 = 0x0007;
pub const TWTY_FRAME: u16 = 0x0008;
pub const TWTY_STR32: u16 = 0x0009;
pub const TWTY_STR64: u16 = 0x000a;
pub const TWTY_STR128: u16 = 0x000b;
pub const TWTY_STR255: u16 = 0x000c;
pub const TWTY_STR1024: u16 = 0x000d;
pub const TWTY_UNI512: u16 = 0x000e;
pub const TWTY_HANDLE: u16 = 0x000f;

// Capabilities.
pub const CAP_XFERCOUNT: u16 = 0x0001;
pub const ICAP_COMPRESSION: u16 = 0x0100;
pub const ICAP_PIXELTYPE: u16 = 0x0101;
pub const ICAP_UNITS: u16 = 0x0102;
pub const ICAP_XFERMECH: u16 = 0x0103;
pub const CAP_FEEDERENABLED: u16 = 0x1002;
pub const CAP_FEEDERLOADED: u16 = 0x1003;
pub const CAP_SUPPORTEDCAPS: u16 = 0x1005;
pub const CAP_AUTOFEED: u16 = 0x1007;
pub const CAP_INDICATORS: u16 = 0x100b;
pub const CAP_PAPERDETECTABLE: u16 = 0x100d;
pub const CAP_UICONTROLLABLE: u16 = 0x100e;
pub const CAP_DEVICEONLINE: u16 = 0x100f;
pub const CAP_DUPLEX: u16 = 0x1012;
pub const CAP_DUPLEXENABLED: u16 = 0x1013;
pub const CAP_CUSTOMDSDATA: u16 = 0x1015;
pub const CAP_DOUBLEFEEDDETECTION: u16 = 0x103f;
pub const ICAP_FRAMES: u16 = 0x1114;
pub const ICAP_XRESOLUTION: u16 = 0x1118;
pub const ICAP_YRESOLUTION: u16 = 0x1119;
pub const ICAP_SUPPORTEDSIZES: u16 = 0x1122;
pub const ICAP_BITDEPTH: u16 = 0x112b;
pub const ICAP_AUTOMATICBORDERDETECTION: u16 = 0x1150;
pub const ICAP_AUTOMATICDESKEW: u16 = 0x1151;

// Capability values.
pub const TWPT_BW: u16 = 0;
pub const TWPT_GRAY: u16 = 1;
pub const TWPT_RGB: u16 = 2;
pub const TWSS_NONE: u16 = 0;
pub const TWSX_NATIVE: u16 = 0;
pub const TWUN_INCHES: u16 = 0;
pub const TWCS_BOTH: u16 = 0;
pub const TWCS_TOP: u16 = 1;
pub const TWCS_BOTTOM: u16 = 2;
pub const TWDF_ULTRASONIC: u16 = 0;
pub const TWDF_BYLENGTH: u16 = 1;
pub const TWDF_INFRARED: u16 = 2;
pub const TWEI_PAGESIDE: u16 = 0x1245;

// MSG_QUERYSUPPORT result bits.
pub const TWQC_GET: u32 = 0x0001;
pub const TWQC_SET: u32 = 0x0002;
pub const TWQC_GETDEFAULT: u32 = 0x0004;
pub const TWQC_GETCURRENT: u32 = 0x0008;
pub const TWQC_RESET: u32 = 0x0010;

pub const TWLG_ENGLISH_USA: u16 = 13; // TWLG_USA
pub const TWCY_USA: u16 = 1;

/// Copy a Rust string into a NUL-terminated `TW_STR32`.
pub fn str32(s: &str) -> TW_STR32 {
    let mut out = [0u8; 34];
    let bytes = s.as_bytes();
    let n = bytes.len().min(32);
    out[..n].copy_from_slice(&bytes[..n]);
    out
}

/// Read a NUL-terminated `TW_STR32` (sources use the ANSI code page; we decode lossily).
pub fn from_str32(s: &TW_STR32) -> String {
    let end = s.iter().position(|&b| b == 0).unwrap_or(s.len());
    String::from_utf8_lossy(&s[..end]).trim().to_string()
}

// Compile-time layout checks, so `cargo check --target i686-pc-windows-msvc`
// verifies the 32-bit layout even where tests cannot run.
const _: () = {
    use std::mem::size_of;
    const P: usize = size_of::<usize>();
    assert!(size_of::<TW_IDENTITY>() == 156);
    assert!(size_of::<TW_ONEVALUE>() == 6);
    assert!(size_of::<TW_ENUMERATION>() == 14);
    assert!(size_of::<TW_RANGE>() == 22);
    assert!(size_of::<TW_IMAGEINFO>() == 42);
    assert!(size_of::<TW_IMAGELAYOUT>() == 28);
    assert!(size_of::<TW_PENDINGXFERS>() == 6);
    assert!(size_of::<TW_CAPABILITY>() == 4 + P);
    assert!(size_of::<TW_USERINTERFACE>() == 4 + P);
    assert!(size_of::<TW_EVENT>() == P + 2);
    assert!(size_of::<TW_CALLBACK2>() == 2 * P + 2);
    assert!(size_of::<TW_ENTRYPOINT>() == 4 + 5 * P);
    assert!(size_of::<TW_CUSTOMDSDATA>() == 4 + P);
    assert!(size_of::<TW_INFO>() == 8 + P);
    assert!(size_of::<TW_EXTIMAGEINFO>() == 12 + P);
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;

    #[test]
    fn pointer_independent_sizes() {
        assert_eq!(size_of::<TW_FIX32>(), 4);
        assert_eq!(size_of::<TW_FRAME>(), 16);
        assert_eq!(size_of::<TW_VERSION>(), 42);
        assert_eq!(size_of::<TW_IDENTITY>(), 156);
        assert_eq!(size_of::<TW_ONEVALUE>(), 6);
        assert_eq!(size_of::<TW_ARRAY>(), 6);
        assert_eq!(size_of::<TW_ENUMERATION>(), 14);
        assert_eq!(size_of::<TW_RANGE>(), 22);
        assert_eq!(size_of::<TW_IMAGELAYOUT>(), 28);
        assert_eq!(size_of::<TW_IMAGEINFO>(), 42);
        assert_eq!(size_of::<TW_PENDINGXFERS>(), 6);
        assert_eq!(size_of::<TW_STATUS>(), 4);
    }

    #[test]
    #[cfg(target_pointer_width = "32")]
    fn sizes_32bit() {
        assert_eq!(size_of::<TW_USERINTERFACE>(), 8);
        assert_eq!(size_of::<TW_CAPABILITY>(), 8);
        assert_eq!(size_of::<TW_EVENT>(), 6);
        assert_eq!(size_of::<TW_CALLBACK2>(), 10);
        assert_eq!(size_of::<TW_ENTRYPOINT>(), 24);
        assert_eq!(size_of::<TW_CUSTOMDSDATA>(), 8);
        assert_eq!(size_of::<TW_INFO>(), 12);
        assert_eq!(size_of::<TW_EXTIMAGEINFO>(), 16);
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn sizes_64bit() {
        assert_eq!(size_of::<TW_USERINTERFACE>(), 12);
        assert_eq!(size_of::<TW_CAPABILITY>(), 12);
        assert_eq!(size_of::<TW_EVENT>(), 10);
        assert_eq!(size_of::<TW_CALLBACK2>(), 18);
        assert_eq!(size_of::<TW_ENTRYPOINT>(), 44);
        assert_eq!(size_of::<TW_CUSTOMDSDATA>(), 12);
        assert_eq!(size_of::<TW_INFO>(), 16);
        assert_eq!(size_of::<TW_EXTIMAGEINFO>(), 20);
    }

    #[test]
    fn identity_field_offsets() {
        let id = TW_IDENTITY::default();
        let base = &id as *const _ as usize;
        assert_eq!(std::ptr::addr_of!(id.Version) as usize - base, 4);
        assert_eq!(std::ptr::addr_of!(id.SupportedGroups) as usize - base, 50);
        assert_eq!(std::ptr::addr_of!(id.Manufacturer) as usize - base, 54);
        assert_eq!(std::ptr::addr_of!(id.ProductName) as usize - base, 122);
    }

    #[test]
    fn fix32_roundtrip() {
        let f = TW_FIX32::from_f64(300.0);
        assert_eq!((f.Whole, f.Frac), (300, 0));
        let f = TW_FIX32::from_f64(2.5);
        assert_eq!((f.Whole, f.Frac), (2, 32768));
        assert_eq!(f.to_f64(), 2.5);
        assert_eq!(f.to_u32(), 2 | (32768 << 16));
    }

    #[test]
    fn str32_roundtrip() {
        let s = str32("fi-8170dj");
        assert_eq!(from_str32(&s), "fi-8170dj");
        let long = str32(&"x".repeat(40));
        assert_eq!(long[32], 0);
        assert_eq!(from_str32(&long).len(), 32);
    }
}
