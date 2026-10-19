//! JSON-lines protocol between the cardloader app and this sidecar.
//!
//! Commands arrive one per line on stdin; events are written one per line on
//! stdout (flushed after each line). Diagnostics go to stderr only.
#![cfg_attr(not(windows), allow(dead_code))]

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::io::Write;

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "cmd", rename_all = "lowercase")]
pub enum Command {
    List,
    Probe {
        #[serde(default)]
        device: Option<String>,
    },
    Scan(ScanRequest),
    Stop,
    Configure {
        #[serde(default)]
        device: Option<String>,
        settings_path: String,
    },
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ColorMode {
    #[default]
    Color,
    Gray,
    Bw,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Multifeed {
    /// Try to switch double-feed detection off (cards are thick and trip ultrasonic sensors).
    Off,
    /// Leave whatever the driver is configured with.
    #[default]
    Default,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ScanRequest {
    /// Product name or a case-insensitive substring of it.
    #[serde(default)]
    pub device: Option<String>,
    pub out_dir: String,
    #[serde(default = "default_dpi")]
    pub dpi: u32,
    #[serde(default)]
    pub color: ColorMode,
    #[serde(default = "default_true")]
    pub duplex: bool,
    /// When both are given, the scan area is set to this size (top-left aligned).
    #[serde(default)]
    pub page_width_mm: Option<f64>,
    #[serde(default)]
    pub page_height_mm: Option<f64>,
    #[serde(default)]
    pub multifeed: Multifeed,
    #[serde(default = "default_prefix")]
    pub prefix: String,
    /// First file number (`<prefix>_<NNNN>.png`); defaults to 1.
    #[serde(default = "default_start_index")]
    pub start_index: u32,
    /// Driver settings blob saved by `configure`, replayed before capability negotiation.
    #[serde(default)]
    pub settings_path: Option<String>,
}

fn default_dpi() -> u32 {
    300
}
fn default_true() -> bool {
    true
}
fn default_prefix() -> String {
    "scan".into()
}
fn default_start_index() -> u32 {
    1
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub backend: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Front,
    Back,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    PaperJam,
    DoubleFeed,
    NoMedia,
    NoDevice,
    DeviceOffline,
    TwainError,
    DsmNotFound,
    CustomDsDataUnsupported,
    Io,
    BadCommand,
    Busy,
    Cancelled,
    UnsupportedPlatform,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "lowercase")]
pub enum Event {
    Ready {
        version: String,
        arch: String,
    },
    Devices {
        devices: Vec<Device>,
    },
    Probe {
        source: Value,
        ds2: bool,
        caps: Map<String, Value>,
    },
    Status {
        message: String,
    },
    Cap {
        name: String,
        requested: Value,
        actual: Value,
        ok: bool,
    },
    Page {
        path: String,
        index: u32,
        side: Side,
    },
    Error {
        code: ErrorCode,
        message: String,
        /// `true` when the current command was aborted because of it.
        fatal: bool,
    },
    Done {
        pages: u32,
    },
    Configured {
        settings_path: String,
        saved: bool,
        cancelled: bool,
    },
}

impl Event {
    pub fn error(code: ErrorCode, message: impl Into<String>, fatal: bool) -> Self {
        Event::Error {
            code,
            message: message.into(),
            fatal,
        }
    }
    pub fn status(message: impl Into<String>) -> Self {
        Event::Status {
            message: message.into(),
        }
    }
}

pub fn parse_command(line: &str) -> Result<Command, String> {
    serde_json::from_str(line).map_err(|e| e.to_string())
}

pub fn arch() -> &'static str {
    if cfg!(target_arch = "x86") {
        "x86"
    } else if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "unknown"
    }
}

/// Write one event as a single JSON line on stdout and flush. Safe to call from any thread.
pub fn emit(event: &Event) {
    let line = serde_json::to_string(event).expect("events always serialize");
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{line}");
    let _ = out.flush();
}

/// Diagnostics; never on stdout.
#[cfg_attr(not(windows), allow(unused_macros))]
macro_rules! log {
    ($($arg:tt)*) => { eprintln!("[cardloader-twain] {}", format!($($arg)*)) };
}
#[cfg_attr(not(windows), allow(unused_imports))]
pub(crate) use log;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_simple_commands() {
        assert_eq!(parse_command(r#"{"cmd":"list"}"#).unwrap(), Command::List);
        assert_eq!(parse_command(r#"{"cmd":"stop"}"#).unwrap(), Command::Stop);
        assert_eq!(parse_command(r#"{"cmd":"quit"}"#).unwrap(), Command::Quit);
        assert_eq!(
            parse_command(r#"{"cmd":"probe","device":"fi-8170"}"#).unwrap(),
            Command::Probe {
                device: Some("fi-8170".into())
            }
        );
        assert_eq!(
            parse_command(r#"{"cmd":"probe"}"#).unwrap(),
            Command::Probe { device: None }
        );
        assert_eq!(
            parse_command(r#"{"cmd":"configure","device":"fi","settings_path":"C:\\s.bin"}"#)
                .unwrap(),
            Command::Configure {
                device: Some("fi".into()),
                settings_path: "C:\\s.bin".into()
            }
        );
    }

    #[test]
    fn parse_full_scan() {
        let cmd = parse_command(
            r#"{"cmd":"scan","device":"fi-8170","out_dir":"C:\\out","dpi":600,"color":"gray",
                "duplex":false,"page_width_mm":66.0,"page_height_mm":92.0,"multifeed":"off",
                "prefix":"card","settings_path":"s.bin","extra":1}"#,
        )
        .unwrap();
        let Command::Scan(r) = cmd else {
            panic!("not a scan")
        };
        assert_eq!(r.device.as_deref(), Some("fi-8170"));
        assert_eq!(r.out_dir, "C:\\out");
        assert_eq!(r.dpi, 600);
        assert_eq!(r.color, ColorMode::Gray);
        assert!(!r.duplex);
        assert_eq!(r.page_width_mm, Some(66.0));
        assert_eq!(r.page_height_mm, Some(92.0));
        assert_eq!(r.multifeed, Multifeed::Off);
        assert_eq!(r.prefix, "card");
        assert_eq!(r.settings_path.as_deref(), Some("s.bin"));
    }

    #[test]
    fn scan_defaults() {
        let Command::Scan(r) = parse_command(r#"{"cmd":"scan","out_dir":"o"}"#).unwrap() else {
            panic!()
        };
        assert_eq!(r.device, None);
        assert_eq!(r.dpi, 300);
        assert_eq!(r.color, ColorMode::Color);
        assert!(r.duplex);
        assert_eq!(r.page_width_mm, None);
        assert_eq!(r.multifeed, Multifeed::Default);
        assert_eq!(r.prefix, "scan");
        assert_eq!(r.start_index, 1);
    }

    #[test]
    fn rejects_bad_commands() {
        assert!(parse_command(r#"{"cmd":"explode"}"#).is_err());
        assert!(parse_command(r#"{"cmd":"scan"}"#).is_err()); // out_dir required
        assert!(parse_command(r#"{"cmd":"scan","out_dir":"o","color":"sepia"}"#).is_err());
        assert!(parse_command("not json").is_err());
    }

    fn to_json(e: &Event) -> Value {
        serde_json::to_value(e).unwrap()
    }

    #[test]
    fn serialize_events() {
        assert_eq!(
            to_json(&Event::Ready {
                version: "0.1.0".into(),
                arch: "x86".into()
            }),
            json!({"event":"ready","version":"0.1.0","arch":"x86"})
        );
        assert_eq!(
            to_json(&Event::Devices {
                devices: vec![Device {
                    id: "fi-8170dj".into(),
                    name: "PFU fi-8170dj".into(),
                    backend: "twain".into()
                }]
            }),
            json!({"event":"devices","devices":[{"id":"fi-8170dj","name":"PFU fi-8170dj","backend":"twain"}]})
        );
        assert_eq!(
            to_json(&Event::Page {
                path: "a.png".into(),
                index: 3,
                side: Side::Back
            }),
            json!({"event":"page","path":"a.png","index":3,"side":"back"})
        );
        assert_eq!(
            to_json(&Event::error(ErrorCode::DoubleFeed, "x", false)),
            json!({"event":"error","code":"double_feed","message":"x","fatal":false})
        );
        assert_eq!(
            to_json(&Event::Cap {
                name: "ICAP_XRESOLUTION".into(),
                requested: json!(300.0),
                actual: json!(300.0),
                ok: true
            }),
            json!({"event":"cap","name":"ICAP_XRESOLUTION","requested":300.0,"actual":300.0,"ok":true})
        );
        assert_eq!(
            to_json(&Event::Done { pages: 7 }),
            json!({"event":"done","pages":7})
        );
        assert_eq!(
            to_json(&Event::status("hi")),
            json!({"event":"status","message":"hi"})
        );
        assert_eq!(
            to_json(&Event::error(ErrorCode::UnsupportedPlatform, "m", true))["code"],
            "unsupported_platform"
        );
        assert_eq!(
            to_json(&Event::error(ErrorCode::CustomDsDataUnsupported, "m", true))["code"],
            "custom_ds_data_unsupported"
        );
    }

    #[test]
    fn events_roundtrip() {
        let e = Event::Configured {
            settings_path: "p".into(),
            saved: true,
            cancelled: false,
        };
        let s = serde_json::to_string(&e).unwrap();
        assert!(!s.contains('\n'));
        assert_eq!(serde_json::from_str::<Event>(&s).unwrap(), e);
    }
}
