//! Document scanners (RICOH/Fujitsu fi-8170 and friends).
//!
//! Linux and macOS drive SANE through the `scanimage` CLI; Windows drives TWAIN through the
//! `cardloader-twain` sidecar (32-bit, to match the PaperStream IP driver). Both deliver PNG pages into the
//! session's `scans/` folder, which are paired into cards as they arrive.

mod sane;
mod twain;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::error::Result;
use crate::model::{Pairing, ScanSettings};
use crate::pairing::{Side, StreamPairer, Unit};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub backend: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScannerSupport {
    pub backend: String,
    pub available: bool,
    pub hint: Option<String>,
}

pub fn support(app: &AppHandle) -> ScannerSupport {
    if cfg!(windows) {
        twain::support(app)
    } else {
        sane::support()
    }
}

pub async fn list_devices(app: &AppHandle) -> Result<Vec<DeviceInfo>> {
    if cfg!(windows) {
        twain::list_devices(app).await
    } else {
        sane::list_devices().await
    }
}

/// Scans one batch (until the feeder is empty or the session stops) and sends the cards.
pub async fn scan(
    app: &AppHandle,
    device: &str,
    settings: &ScanSettings,
    out_dir: PathBuf,
    pairing: Pairing,
    tx: mpsc::Sender<Unit>,
    cancel: CancellationToken,
) -> Result<()> {
    std::fs::create_dir_all(&out_dir)?;
    let prefix = chrono::Local::now()
        .format("scan_%Y%m%d_%H%M%S")
        .to_string();
    let (page_tx, mut page_rx) = mpsc::channel::<(PathBuf, Side)>(64);
    let pairer_task = tokio::spawn(async move {
        let mut pairer = StreamPairer::default();
        while let Some((path, side)) = page_rx.recv().await {
            let unit = match pairing {
                Pairing::Single => Some(Unit {
                    front: path,
                    back: None,
                }),
                Pairing::FrontBack => pairer.push(path, side),
            };
            if let Some(unit) = unit {
                if tx.send(unit).await.is_err() {
                    return;
                }
            }
        }
        if let Some(unit) = pairer.flush() {
            let _ = tx.send(unit).await;
        }
    });
    let result = if cfg!(windows) {
        twain::scan(app, device, settings, &out_dir, &prefix, page_tx, cancel).await
    } else {
        sane::scan(device, settings, &out_dir, &prefix, page_tx, cancel).await
    };
    let _ = pairer_task.await;
    result
}

/// Duplex scanners deliver front, back, front, back…
fn side_for(index: usize, duplex: bool) -> Side {
    match (duplex, index % 2) {
        (false, _) => Side::Front,
        (true, 1) => Side::Front,
        (true, _) => Side::Back,
    }
}
