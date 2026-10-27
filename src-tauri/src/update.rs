//! Updating the app from its GitHub releases. The release workflow signs the bundles and uploads `latest.json`
//! (tauri-action); the updater reads it from the latest published release, so a draft release offers nothing yet.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::error::{Error, Result};

/// The update the last check found, kept for installing it.
#[derive(Default)]
pub struct Updates {
    found: Mutex<Option<Update>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub current_version: String,
}

/// Download progress, emitted as `update-progress`.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProgress {
    pub downloaded: u64,
    pub total: Option<u64>,
}

impl From<tauri_plugin_updater::Error> for Error {
    fn from(e: tauri_plugin_updater::Error) -> Self {
        Error::msg(format!("update: {e}"))
    }
}

impl Updates {
    /// A newer release than this app, or None.
    pub async fn check(&self, app: &AppHandle) -> Result<Option<UpdateInfo>> {
        let handle = app.clone();
        // Windows runs the installer and exits the process without the usual exit events.
        let update = app
            .updater_builder()
            .on_before_exit(move || crate::before_exit(&handle))
            .build()?
            .check()
            .await?;
        let info = update.as_ref().map(|u| UpdateInfo {
            version: u.version.clone(),
            current_version: u.current_version.clone(),
        });
        *self.found.lock().unwrap() = update;
        Ok(info)
    }

    /// Downloads and installs the update found by `check`, then restarts the app (Windows: its installer does).
    pub async fn install(&self, app: &AppHandle, busy: bool) -> Result<()> {
        if busy {
            return Err(Error::msg(
                "a session, sync, eBay job or export is still running; update once it has finished",
            ));
        }
        let update = self
            .found
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| Error::msg("no update to install; check for updates first"))?;
        let mut downloaded = 0u64;
        let mut last = Instant::now();
        update
            .download_and_install(
                |chunk, total| {
                    downloaded += chunk as u64;
                    if last.elapsed() > Duration::from_millis(100) {
                        last = Instant::now();
                        let _ = app.emit("update-progress", UpdateProgress { downloaded, total });
                    }
                },
                || {},
            )
            .await?;
        app.request_restart();
        Ok(())
    }
}
