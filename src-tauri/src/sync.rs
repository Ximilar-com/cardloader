//! Mirroring a local session to Cards Cloud: create (or reopen) the remote session, upload finished
//! items in batches (re-sent when grading arrives — the server keys rows by `event_id`), then each
//! row's image, and close the remote session when the local run completed.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use crate::imaging;
use crate::model::Session;
use crate::runner::Runner;
use crate::ximilar::cards_cloud::{
    outcome, pick_patch, session_item, CardsCloudApi, Outcome, MAX_BATCH,
};
use crate::ximilar::http::XimilarClient;

const FLUSH_EVERY: Duration = Duration::from_secs(2);
const MAX_BACKOFF: Duration = Duration::from_secs(60);
const FINAL_ATTEMPTS: u32 = 4;
const IMAGES_PER_FLUSH: usize = 25;

enum Flush {
    Done,
    Retry(String),
    Stopped(String),
}

/// Runs until `finished` holds `Some(close)`: then one last flush (a few attempts), and the remote
/// session is closed when `close` is true.
pub async fn run(
    runner: Arc<Runner>,
    session_id: String,
    client: XimilarClient,
    mut finished: watch::Receiver<Option<bool>>,
    cancel: CancellationToken,
) {
    let api = CardsCloudApi::new(client);
    let mut sync = Sync {
        runner: runner.clone(),
        api,
        session_id: session_id.clone(),
        include_grading: true,
    };

    let remote = match sync.ensure_remote().await {
        Ok(id) => id,
        Err(e) => {
            sync.error(Some(&e));
            return;
        }
    };
    sync.error(None);

    let mut delay = FLUSH_EVERY;
    let mut final_attempts = 0;
    let mut retrying = false;
    loop {
        let close = *finished.borrow();
        match sync.flush(&remote).await {
            Flush::Done => {
                delay = FLUSH_EVERY;
                if std::mem::take(&mut retrying) {
                    sync.error(None);
                }
                if let Some(close) = close {
                    if close {
                        if let Err(e) = sync.api.close(&remote).await {
                            sync.error(Some(&format!("could not close the remote session: {e}")));
                        }
                    }
                    break;
                }
            }
            Flush::Retry(why) => {
                retrying = true;
                sync.error(Some(&format!("{why} — retrying")));
                delay = (delay * 2).min(MAX_BACKOFF);
                if close.is_some() {
                    final_attempts += 1;
                    if final_attempts >= FINAL_ATTEMPTS {
                        sync.error(Some(&format!(
                            "{why} — not everything was uploaded; use Upload to retry"
                        )));
                        break;
                    }
                }
            }
            Flush::Stopped(why) => {
                sync.error(Some(&format!(
                    "Cards Cloud stopped accepting uploads: {why}"
                )));
                break;
            }
        }
        runner.emit_session(&session_id);
        tokio::select! {
            _ = tokio::time::sleep(delay) => {},
            _ = finished.changed() => {},
            _ = cancel.cancelled() => break,
        }
    }
    runner.emit_session(&session_id);
}

struct Sync {
    runner: Arc<Runner>,
    api: CardsCloudApi,
    session_id: String,
    include_grading: bool,
}

impl Sync {
    fn error(&self, error: Option<&str>) {
        let _ = self.runner.db.set_remote(&self.session_id, None, error);
        self.runner.emit_session(&self.session_id);
    }

    /// The remote session id: the one we have (reopened if closed), else a new one.
    async fn ensure_remote(&mut self) -> Result<String, String> {
        let session = self
            .runner
            .db
            .session(&self.session_id)
            .map_err(|e| e.to_string())?;
        if let Some(remote) = &session.remote_session_id {
            match self.api.get(remote).await {
                Ok(s) if s["status"] == "live" => return Ok(remote.clone()),
                Ok(_) => {
                    return match self.api.reopen(remote).await {
                        Ok(_) => Ok(remote.clone()),
                        Err(e) => Err(format!("could not reopen the remote session: {e}")),
                    }
                }
                Err(e) if e.status() == Some(404) => {
                    // Deleted on the platform: start over in a new remote session.
                    let _ = self.runner.db.reset_sync(&self.session_id);
                }
                Err(e) => return Err(format!("Cards Cloud unreachable: {e}")),
            }
        }
        let created = self
            .api
            .create(&create_payload(&session))
            .await
            .map_err(|e| match e.status() {
                Some(403) => format!("Cards Cloud is not enabled for this account ({e})"),
                Some(402) => {
                    format!("not enough credits to create a session in Cards Cloud ({e})")
                }
                _ => format!("could not create the remote session: {e}"),
            })?;
        let id = created["id"]
            .as_str()
            .ok_or("Cards Cloud returned no session id")?
            .to_string();
        self.runner
            .db
            .set_remote(&self.session_id, Some(&id), None)
            .map_err(|e| e.to_string())?;
        Ok(id)
    }

    async fn flush(&mut self, remote: &str) -> Flush {
        loop {
            let batch = match self.runner.db.unsynced(&self.session_id, MAX_BATCH) {
                Ok(b) => b,
                Err(e) => return Flush::Retry(e.to_string()),
            };
            if batch.is_empty() {
                break;
            }
            let rows: Vec<Value> = batch
                .iter()
                .filter_map(|(id, _)| self.runner.db.item(*id).ok())
                .map(|item| session_item(&item, self.include_grading))
                .collect();
            let result = self.api.upload(remote, &rows).await;
            match outcome(&result) {
                Outcome::Stored => {}
                Outcome::Rejected(why) if self.include_grading && why.contains("grading") => {
                    // A server without grading support: send the rows without it.
                    tracing::warn!("Cards Cloud rejected grading ({why}); uploading without it");
                    self.include_grading = false;
                    continue;
                }
                Outcome::Rejected(why) => {
                    tracing::warn!("Cards Cloud rejected {} row(s): {why}", rows.len());
                    self.error(Some(&format!("{} row(s) rejected: {why}", rows.len())));
                }
                Outcome::Retry(why) => return Flush::Retry(why),
                Outcome::Stopped(why) => return Flush::Stopped(why),
            }
            // Stored or rejected: either way this revision is settled.
            for (id, rev) in &batch {
                let _ = self.runner.db.mark_synced(*id, *rev);
                self.runner.emit_item(*id);
            }
            if batch.len() < MAX_BATCH {
                break;
            }
        }
        match self.upload_images(remote).await {
            Flush::Done => push_picks(&self.runner, &self.api, &self.session_id, remote).await,
            other => other,
        }
    }

    async fn upload_images(&self, remote: &str) -> Flush {
        let ids = self
            .runner
            .db
            .images_to_upload(&self.session_id, IMAGES_PER_FLUSH)
            .unwrap_or_default();
        for id in ids {
            let Ok(item) = self.runner.db.item(id) else {
                continue;
            };
            let Some(preview) = item.preview_path.clone() else {
                continue;
            };
            let path = PathBuf::from(preview);
            let encoded = tokio::task::spawn_blocking(move || {
                imaging::encode_for_upload(&path, imaging::CARDS_CLOUD_LONG_EDGE, 88)
            })
            .await;
            let Ok(Ok(b64)) = encoded else {
                let _ = self.runner.db.mark_image_synced(id);
                continue;
            };
            let result = self.api.upload_image(remote, &item.event_id, &b64).await;
            match outcome(&result) {
                Outcome::Stored => {
                    // The answer is the row: keep its id for later PATCHes.
                    if let Some(row) = result.as_ref().ok().and_then(|r| r["id"].as_str()) {
                        let _ = self.runner.db.set_remote_id(id, row);
                    }
                }
                // 404: the row was deleted on the platform — only this image is lost.
                Outcome::Rejected(_) => {}
                Outcome::Stopped(why)
                    if result.as_ref().err().and_then(|e| e.status()) == Some(404) =>
                {
                    tracing::warn!("image of item {id} not stored: {why}");
                }
                Outcome::Retry(why) => return Flush::Retry(why),
                Outcome::Stopped(why) => return Flush::Stopped(why),
            }
            let _ = self.runner.db.mark_image_synced(id);
        }
        Flush::Done
    }
}

/// Sends the user's picks as PATCHes of the rows. Works on closed remote sessions too, so it also runs on
/// its own right after a pick (see [`push_picks_now`]).
async fn push_picks(runner: &Runner, api: &CardsCloudApi, session_id: &str, remote: &str) -> Flush {
    let picks = runner.db.picks_to_sync(session_id).unwrap_or_default();
    for (id, picks) in picks {
        let Ok(item) = runner.db.item(id) else {
            continue;
        };
        let row = match &item.remote_id {
            Some(row) => Some(row.clone()),
            None => {
                let found = api.find_identification(remote, &item.event_id).await;
                match outcome(&found) {
                    Outcome::Retry(why) => return Flush::Retry(why),
                    Outcome::Stopped(why) => return Flush::Stopped(why),
                    _ => {}
                }
                found.ok().flatten()
            }
        };
        let Some(row) = row else {
            // Not in Cards Cloud (deleted there): nothing to correct.
            let _ = runner.db.mark_pick_synced(id, picks);
            continue;
        };
        let _ = runner.db.set_remote_id(id, &row);
        let result = api.patch_identification(&row, &pick_patch(&item)).await;
        match outcome(&result) {
            Outcome::Stored => {}
            Outcome::Rejected(why) => tracing::warn!("pick of item {id} not stored: {why}"),
            Outcome::Stopped(_) if result.as_ref().err().and_then(|e| e.status()) == Some(404) => {}
            Outcome::Retry(why) => return Flush::Retry(why),
            Outcome::Stopped(why) => return Flush::Stopped(why),
        }
        let _ = runner.db.mark_pick_synced(id, picks);
        runner.emit_item(id);
    }
    Flush::Done
}

/// One pass of [`push_picks`] for a session that has a remote copy, outside a sync run.
pub async fn push_picks_now(runner: Arc<Runner>, session_id: String, client: XimilarClient) {
    let Ok(session) = runner.db.session(&session_id) else {
        return;
    };
    let Some(remote) = session.remote_session_id else {
        return;
    };
    let api = CardsCloudApi::new(client);
    match push_picks(&runner, &api, &session_id, &remote).await {
        Flush::Done => {}
        Flush::Retry(why) | Flush::Stopped(why) => {
            tracing::warn!("picks of session {session_id} not sent: {why}");
            let _ = runner.db.set_remote(
                &session_id,
                None,
                Some(&format!(
                    "match corrections not sent: {why} — use Upload to retry"
                )),
            );
            runner.emit_session(&session_id);
        }
    }
}

fn create_payload(session: &Session) -> Value {
    let game = match session.id_mode {
        crate::model::IdMode::Auto => "",
        crate::model::IdMode::Tcg => "TCG",
        crate::model::IdMode::Sport => "Sports",
        crate::model::IdMode::Comics => "Comics",
        crate::model::IdMode::Slab => "Slabs",
    };
    json!({
        "name": session.name.chars().take(255).collect::<String>(),
        "description": format!("Imported with cardloader from {}", session.source_kind.as_str().replace('_', " ")),
        "game": game,
        "platform": "other",
        "client": {
            "name": "cardloader",
            "version": env!("CARGO_PKG_VERSION"),
            "os": std::env::consts::OS,
        },
        "meta_data": {
            "cardloader_session": session.id,
            "source": session.source_kind.as_str(),
            "id_mode": session.id_mode.as_str(),
            "grading": session.options.grading.as_str(),
            "pairing": session.options.pairing.as_str(),
            "both_sides": session.options.both_sides,
            "magic_ai": session.options.magic_ai,
            "skip_detection": session.options.skip_detection,
            "alphabet": session.options.alphabet,
            "subcategory": session.options.subcategory,
        },
    })
}
