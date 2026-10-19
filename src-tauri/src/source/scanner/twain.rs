//! TWAIN on Windows through the `cardloader-twain` sidecar (see `crates/cardloader-twain`).
//!
//! The helper owns the TWAIN state machine on its own thread and talks JSON lines over stdio, so a
//! driver that hangs or crashes takes down the helper, never the app.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{json, Value};
use tauri::AppHandle;
use tauri_plugin_shell::process::{CommandChild, CommandEvent};
use tauri_plugin_shell::ShellExt;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::{side_for, DeviceInfo, ScannerSupport};
use crate::error::{Error, Result};
use crate::model::{ColorMode, MultifeedMode, ScanSettings};
use crate::pairing::Side;

const SIDECAR: &str = "cardloader-twain";
/// No event for this long while scanning means the driver is stuck.
const WATCHDOG: Duration = Duration::from_secs(180);

pub fn support(_app: &AppHandle) -> ScannerSupport {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf));
    let found = exe_dir
        .map(|d| d.join(format!("{SIDECAR}.exe")).is_file())
        .unwrap_or(false);
    ScannerSupport {
        backend: "twain".into(),
        available: found,
        hint: if found {
            None
        } else {
            Some("TWAIN helper missing — reinstall cardloader (or run `just sidecar` in development)".into())
        },
    }
}

struct Helper {
    events: mpsc::Receiver<CommandEvent>,
    child: Option<CommandChild>,
}

impl Helper {
    fn spawn(app: &AppHandle) -> Result<Self> {
        let (events, child) = app
            .shell()
            .sidecar(SIDECAR)
            .map_err(|e| Error::msg(format!("TWAIN helper: {e}")))?
            .spawn()
            .map_err(|e| Error::msg(format!("TWAIN helper failed to start: {e}")))?;
        Ok(Helper {
            events,
            child: Some(child),
        })
    }

    fn send(&mut self, cmd: &Value) -> Result<()> {
        let child = self
            .child
            .as_mut()
            .ok_or_else(|| Error::msg("TWAIN helper exited"))?;
        let mut line = cmd.to_string();
        line.push('\n');
        child
            .write(line.as_bytes())
            .map_err(|e| Error::msg(format!("TWAIN helper: {e}")))
    }

    /// The next JSON event; None when the helper exited.
    async fn next(&mut self, timeout: Duration) -> Result<Option<Value>> {
        loop {
            let event = tokio::time::timeout(timeout, self.events.recv())
                .await
                .map_err(|_| Error::msg("the scanner driver stopped responding"))?;
            match event {
                Some(CommandEvent::Stdout(bytes)) => {
                    let text = String::from_utf8_lossy(&bytes);
                    match serde_json::from_str::<Value>(text.trim()) {
                        Ok(v) => return Ok(Some(v)),
                        Err(_) => tracing::debug!("twain: {}", text.trim()),
                    }
                }
                Some(CommandEvent::Stderr(bytes)) => {
                    tracing::debug!("twain: {}", String::from_utf8_lossy(&bytes).trim())
                }
                Some(CommandEvent::Terminated(_)) | None => return Ok(None),
                Some(_) => {}
            }
        }
    }

    fn kill(&mut self) {
        if let Some(child) = self.child.take() {
            let _ = child.kill();
        }
    }
}

impl Drop for Helper {
    fn drop(&mut self) {
        let _ = self.send(&json!({"cmd": "quit"}));
        self.kill();
    }
}

fn error_text(event: &Value) -> String {
    let msg = event["message"].as_str().unwrap_or("scanner error");
    match event["code"].as_str() {
        Some("paper_jam") => format!("Paper jam — clear the feeder and run again ({msg})"),
        Some("double_feed") => {
            format!("Double feed detected — re-stack the cards and run again ({msg})")
        }
        Some("no_media") => "The feeder is empty".into(),
        Some("no_device") => format!("Scanner not found — is it on and connected? ({msg})"),
        _ => msg.to_string(),
    }
}

pub async fn list_devices(app: &AppHandle) -> Result<Vec<DeviceInfo>> {
    let mut helper = Helper::spawn(app)?;
    helper.send(&json!({"cmd": "list"}))?;
    while let Some(event) = helper.next(Duration::from_secs(60)).await? {
        match event["event"].as_str() {
            Some("devices") => {
                return Ok(serde_json::from_value(event["devices"].clone()).unwrap_or_default());
            }
            Some("error") if event["fatal"].as_bool().unwrap_or(true) => {
                return Err(Error::msg(error_text(&event)))
            }
            _ => {}
        }
    }
    Err(Error::msg("TWAIN helper exited without listing scanners"))
}

pub async fn scan(
    app: &AppHandle,
    device: &str,
    s: &ScanSettings,
    out_dir: &Path,
    prefix: &str,
    pages: mpsc::Sender<(PathBuf, Side)>,
    cancel: CancellationToken,
) -> Result<()> {
    let mut helper = Helper::spawn(app)?;
    let color = match s.color {
        ColorMode::Color => "color",
        ColorMode::Gray => "gray",
        ColorMode::Bw => "bw",
    };
    helper.send(&json!({
        "cmd": "scan",
        "device": device,
        "out_dir": out_dir,
        "prefix": prefix,
        "dpi": s.dpi,
        "color": color,
        "duplex": s.duplex,
        "page_width_mm": s.page_width_mm,
        "page_height_mm": s.page_height_mm,
        "multifeed": if s.multifeed == MultifeedMode::Off { "off" } else { "default" },
    }))?;

    let mut stop_sent = false;
    let mut index = 0;
    loop {
        let event = tokio::select! {
            e = helper.next(WATCHDOG) => e,
            _ = cancel.cancelled(), if !stop_sent => {
                stop_sent = true;
                let _ = helper.send(&json!({"cmd": "stop"}));
                continue;
            }
        };
        let event = match event {
            Ok(Some(e)) => e,
            Ok(None) => return Err(Error::msg("the TWAIN helper exited unexpectedly")),
            Err(e) => {
                helper.kill();
                return Err(e);
            }
        };
        match event["event"].as_str() {
            Some("page") => {
                index += 1;
                let side = match event["side"].as_str() {
                    Some("front") => Side::Front,
                    Some("back") => Side::Back,
                    _ => side_for(index, s.duplex),
                };
                if let Some(path) = event["path"].as_str() {
                    if pages.send((PathBuf::from(path), side)).await.is_err() {
                        return Ok(());
                    }
                }
            }
            Some("cap") if event["ok"] == false => {
                tracing::warn!("scanner ignored {}: {}", event["name"], event);
            }
            Some("status") => tracing::info!("scanner: {}", event["message"]),
            Some("error") => {
                if event["code"] == "no_media" {
                    if index > 0 {
                        continue;
                    }
                    return Err(Error::msg(error_text(&event)));
                }
                if event["fatal"].as_bool().unwrap_or(true) {
                    return Err(Error::msg(error_text(&event)));
                }
                tracing::warn!("scanner: {}", error_text(&event));
            }
            Some("done") => return Ok(()),
            _ => {}
        }
    }
}
