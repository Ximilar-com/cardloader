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
        match e {
            // GitHub answered latest.json with an error status: releases before 0.5.0, or one built without the
            // signing key, don't have it. (Being offline is a different error.)
            tauri_plugin_updater::Error::ReleaseNotFound => Error::msg(
                "the latest release on GitHub can't be installed from the app (it has no latest.json); \
                 download newer versions from the releases page",
            ),
            e => Error::msg(format!("update: {e}")),
        }
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
    /// `busy` tells whether anything runs that the restart would cut off.
    pub async fn install(&self, app: &AppHandle, busy: impl Fn() -> bool) -> Result<()> {
        let update = self
            .found
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| Error::msg("no update to install; check for updates first"))?;
        let mut downloaded = 0u64;
        let mut last = Instant::now();
        let download = async {
            let bytes = update
                .download(
                    |chunk, total| {
                        downloaded += chunk as u64;
                        if last.elapsed() > Duration::from_millis(100) {
                            last = Instant::now();
                            let _ =
                                app.emit("update-progress", UpdateProgress { downloaded, total });
                        }
                    },
                    || {},
                )
                .await?;
            Ok(bytes)
        };
        let install = |bytes: Vec<u8>| Ok(update.install(bytes)?);
        download_then_install(busy, download, install).await?;
        app.request_restart();
        Ok(())
    }
}

/// Installs what `download` fetched unless something is running. `busy` is asked before the download and again
/// right before installing: a session the user or an agent starts while the update downloads would be cut off
/// by the restart.
async fn download_then_install(
    busy: impl Fn() -> bool,
    download: impl std::future::Future<Output = Result<Vec<u8>>>,
    install: impl FnOnce(Vec<u8>) -> Result<()>,
) -> Result<()> {
    if busy() {
        return Err(Error::msg(
            "a session, sync, eBay job or export is still running; update once it has finished",
        ));
    }
    let bytes = download.await?;
    if busy() {
        return Err(Error::msg(
            "a session, sync, eBay job or export started while the update downloaded; update once it has finished",
        ));
    }
    install(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[tokio::test]
    async fn nothing_installs_while_something_runs() {
        let running = AtomicBool::new(false);
        let installed = AtomicBool::new(false);
        let busy = || running.load(Ordering::SeqCst);
        let install = |bytes: Vec<u8>| {
            assert_eq!(bytes, b"bundle");
            installed.store(true, Ordering::SeqCst);
            Ok(())
        };

        // A session starts while the update downloads.
        let download = async {
            running.store(true, Ordering::SeqCst);
            Ok(b"bundle".to_vec())
        };
        let err = download_then_install(busy, download, install)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("started while"), "{err}");
        assert!(!installed.load(Ordering::SeqCst));

        // Running before: not even downloaded.
        let downloaded = AtomicBool::new(false);
        let download = async {
            downloaded.store(true, Ordering::SeqCst);
            Ok(b"bundle".to_vec())
        };
        assert!(download_then_install(busy, download, install)
            .await
            .is_err());
        assert!(!downloaded.load(Ordering::SeqCst) && !installed.load(Ordering::SeqCst));

        running.store(false, Ordering::SeqCst);
        download_then_install(busy, async { Ok(b"bundle".to_vec()) }, install)
            .await
            .unwrap();
        assert!(installed.load(Ordering::SeqCst));
    }
}
