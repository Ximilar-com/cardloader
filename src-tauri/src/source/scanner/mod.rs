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

use crate::error::{Error, Result};
use crate::imaging;
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

/// Opens the scanner driver's own settings dialog; TWAIN (Windows) only. Returns whether they were saved.
pub async fn configure(app: &AppHandle, device: &str) -> Result<bool> {
    if cfg!(windows) {
        twain::configure(app, device).await
    } else {
        Err(Error::msg(
            "the driver settings dialog is only available for TWAIN scanners on Windows",
        ))
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
    let rotation = settings.rotation;
    let mut cards = Cards::new(pairing, settings.duplex && settings.back_first);
    let pairer_task = tokio::spawn(async move {
        while let Some((path, side)) = page_rx.recv().await {
            let page = path.clone();
            let turned = tokio::task::spawn_blocking(move || imaging::rotate_file(&page, rotation))
                .await
                .unwrap_or_else(|e| Err(Error::msg(e.to_string())));
            if let Err(e) = turned {
                tracing::warn!("could not turn {}: {e}", path.display());
            }
            if let Some(unit) = cards.push(path, side) {
                if tx.send(unit).await.is_err() {
                    return;
                }
            }
        }
        if let Some(unit) = cards.flush() {
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

/// Scanned pages as cards, in the order the scanner delivers them.
struct Cards {
    pairing: Pairing,
    /// The page the scanner calls the front is the card's back.
    back_first: bool,
    pairer: StreamPairer,
}

impl Cards {
    fn new(pairing: Pairing, back_first: bool) -> Self {
        Cards {
            pairing,
            back_first,
            pairer: StreamPairer::default(),
        }
    }

    fn push(&mut self, path: PathBuf, side: Side) -> Option<Unit> {
        match self.pairing {
            Pairing::Single => Some(Unit {
                front: path,
                back: None,
            }),
            Pairing::FrontBack => self.pairer.push(path, side).map(|u| self.order(u)),
        }
    }

    fn flush(&mut self) -> Option<Unit> {
        self.pairer.flush()
    }

    fn order(&self, unit: Unit) -> Unit {
        match unit.back {
            Some(back) if self.back_first => Unit {
                front: back,
                back: Some(unit.front),
            },
            _ => unit,
        }
    }
}

/// Duplex scanners deliver front, back, front, back…
fn side_for(index: usize, duplex: bool) -> Side {
    match (duplex, index % 2) {
        (false, _) => Side::Front,
        (true, 1) => Side::Front,
        (true, _) => Side::Back,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pages(cards: &mut Cards, n: usize) -> Vec<Unit> {
        let mut units: Vec<Unit> = (1..=n)
            .filter_map(|i| cards.push(PathBuf::from(format!("scan_{i}.png")), side_for(i, true)))
            .collect();
        units.extend(cards.flush());
        units
    }

    fn unit(front: &str, back: Option<&str>) -> Unit {
        Unit {
            front: front.into(),
            back: back.map(PathBuf::from),
        }
    }

    #[test]
    fn duplex_pages_pair_front_first() {
        let units = pages(&mut Cards::new(Pairing::FrontBack, false), 4);
        assert_eq!(
            units,
            vec![
                unit("scan_1.png", Some("scan_2.png")),
                unit("scan_3.png", Some("scan_4.png"))
            ]
        );
    }

    #[test]
    fn duplex_pages_pair_back_first() {
        let units = pages(&mut Cards::new(Pairing::FrontBack, true), 5);
        assert_eq!(
            units,
            vec![
                unit("scan_2.png", Some("scan_1.png")),
                unit("scan_4.png", Some("scan_3.png")),
                unit("scan_5.png", None)
            ]
        );
    }
}
