//! Running a session: images come in from the source, workers recognise (and grade) them, results
//! go to the database and to the UI as events, and — when asked — to Cards Cloud.
//!
//! One task per running session, controlled by a pause flag and a cancellation token:
//! * the **ingest** task turns source units into queued items (thumbnail + preview made first);
//! * the **dispatcher** claims queued items in order, up to `concurrency` at a time, while not paused;
//! * the **grading poller** waits for async grading jobs (they survive pause, stop and restarts);
//! * the **sync** task mirrors finished items to Cards Cloud (see [`crate::sync`]).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::json;
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, watch, Notify, Semaphore};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use crate::db::Db;
use crate::ebay::sync::{EbayProgress, Job};
use crate::ebay::EbayAuth;
use crate::error::{Error, Result};
use crate::imaging;
use crate::local::LocalModels;
use crate::model::*;
use crate::pairing::Unit;
use crate::settings::SettingsStore;
use crate::source;
use crate::source::csv_file::CsvRow;
use crate::ximilar::cards_cloud::CardsCloudApi;
use crate::ximilar::grading::{self, JobStatus};
use crate::ximilar::http::{ApiError, Failure, XimilarClient};
use crate::ximilar::recognition::{self, RecognitionOptions};

pub const EVENT_SESSION: &str = "session-updated";
pub const EVENT_ITEM: &str = "item-updated";
pub const EVENT_EBAY: &str = "ebay-progress";
const GRADING_POLL: Duration = Duration::from_secs(3);

/// Where progress goes: the webview in the app, a recorder in tests.
pub trait EventSink: Send + Sync {
    fn session(&self, session: Session);
    fn item(&self, item: ItemSummary);
    fn ebay(&self, _progress: &EbayProgress) {}
    /// A run ended on its own (not stopped): tell the user if they are looking elsewhere.
    fn finished(&self, _session: &Session) {}
}

impl EventSink for AppHandle {
    fn session(&self, session: Session) {
        let _ = self.emit(EVENT_SESSION, session);
    }

    fn item(&self, item: ItemSummary) {
        let _ = self.emit(EVENT_ITEM, item);
    }

    fn ebay(&self, progress: &EbayProgress) {
        let _ = self.emit(EVENT_EBAY, progress);
    }

    fn finished(&self, session: &Session) {
        use tauri::Manager;
        use tauri_plugin_notification::NotificationExt;
        let focused = self
            .get_webview_window("main")
            .and_then(|w| w.is_focused().ok())
            .unwrap_or(false);
        if !focused {
            let _ = self
                .notification()
                .builder()
                .title(&session.name)
                .body(finished_text(session))
                .show();
        }
    }
}

/// "Done: 412 cards, $1,284.50 · 23 to review · 2 failed", or why the run failed.
fn finished_text(session: &Session) -> String {
    let c = &session.counts;
    if session.status == SessionStatus::Failed {
        return format!(
            "Stopped: {}",
            session.error.as_deref().unwrap_or("an error")
        );
    }
    let mut text = format!(
        "Done: {} card{}",
        c.done,
        if c.done == 1 { "" } else { "s" }
    );
    if c.total_price > 0.0 {
        text += &format!(", ${:.2}", c.total_price);
    }
    if c.to_review > 0 {
        text += &format!(" · {} to review", c.to_review);
    }
    if c.errors > 0 {
        text += &format!(" · {} failed", c.errors);
    }
    text
}

struct EbayJob {
    cancel: CancellationToken,
    progress: EbayProgress,
}

struct RunControl {
    paused: watch::Sender<bool>,
    cancel: CancellationToken,
}

pub struct Runner {
    /// Only needed to start the Windows scanner sidecar.
    app: Option<AppHandle>,
    events: Arc<dyn EventSink>,
    pub db: Db,
    pub settings: Arc<SettingsStore>,
    pub data_dir: PathBuf,
    runs: Mutex<HashMap<String, RunControl>>,
    syncs: Mutex<HashMap<String, CancellationToken>>,
    ebay_jobs: Mutex<HashMap<String, EbayJob>>,
    /// Running marketplace exports (Whatnot uploads pictures), by session.
    pub exports: Mutex<HashMap<String, CancellationToken>>,
    /// The eBay access token.
    pub ebay: EbayAuth,
    /// On-device models (downloads, the loaded model).
    pub local: Arc<LocalModels>,
}

impl Runner {
    pub fn new(
        app: Option<AppHandle>,
        events: Arc<dyn EventSink>,
        db: Db,
        settings: Arc<SettingsStore>,
        data_dir: PathBuf,
    ) -> Arc<Self> {
        Arc::new(Runner {
            app,
            events,
            db,
            settings,
            runs: Mutex::new(HashMap::new()),
            syncs: Mutex::new(HashMap::new()),
            ebay_jobs: Mutex::new(HashMap::new()),
            exports: Mutex::new(HashMap::new()),
            ebay: EbayAuth::default(),
            local: Arc::new(LocalModels::new(data_dir.join("models"))),
            data_dir,
        })
    }

    pub fn session_dir(&self, id: &str) -> PathBuf {
        self.data_dir.join("sessions").join(id)
    }

    pub fn client(&self) -> Result<XimilarClient> {
        let settings = self.settings.get();
        let token = self.settings.token().ok_or(ApiError::NoToken)?;
        Ok(XimilarClient::new(
            &settings.api_base_url,
            &token,
            settings.workspace,
        )?)
    }

    pub fn emit_session(&self, id: &str) {
        if let Ok(session) = self.db.session(id) {
            self.events.session(session);
        }
    }

    pub fn emit_item(&self, id: i64) {
        if let Ok(item) = self.db.item_summary(id) {
            self.events.item(item);
        }
    }

    /// Makes candidate `index` (into the stored response's alternatives) the item's match, and sends the
    /// correction to Cards Cloud when the session has a remote copy.
    pub fn pick_match(self: &Arc<Self>, item_id: i64, index: usize) -> Result<ItemDetail> {
        let item = self.db.item(item_id)?;
        if !matches!(item.summary.status, ItemStatus::Done | ItemStatus::Grading) {
            return Err(Error::Msg("this card hasn't been identified yet".into()));
        }
        let mut raw = item
            .recognition
            .clone()
            .ok_or_else(|| Error::Msg("this card has no identification".into()))?;
        recognition::select_alternative(&mut raw["response"], index).map_err(Error::Msg)?;
        let endpoint = recognition::Endpoint::of_raw(&raw);
        let parsed = recognition::parse(endpoint, &raw["response"]);
        self.db.save_pick(
            item_id,
            &parsed.card,
            parsed.confidence.as_deref(),
            parsed.distance,
            parsed.price,
            &raw,
        )?;
        self.emit_item(item_id);
        self.emit_session(&item.summary.session_id);
        self.push_corrections(&item.summary.session_id)?;
        self.db.item(item_id)
    }

    /// Saves the fields, price, notes and location the user typed for a card. The card fields and price
    /// are kept only once it is identified; a correction reaches Cards Cloud like a pick.
    pub fn edit_item(self: &Arc<Self>, item_id: i64, mut edit: ItemEdit) -> Result<ItemDetail> {
        let item = self.db.item(item_id)?;
        let before = &item.summary;
        if let Some(p) = edit.price {
            if !p.is_finite() || p < 0.0 {
                return Err(Error::Msg("the price must be a positive number".into()));
            }
        }
        if matches!(before.status, ItemStatus::Done | ItemStatus::Grading) {
            edit.card.trim();
            let renamed = edit.card.name != before.card.name
                || edit.card.set_name != before.card.set_name
                || edit.card.card_number != before.card.card_number;
            // The full name repeats the others; one left as it was would still name the old card.
            if renamed && edit.card.full_name == before.card.full_name {
                edit.card.full_name = edit.card.composed_full_name();
            }
        } else {
            edit.card = before.card.clone();
            edit.price = before.price;
        }
        edit.notes = edit.notes.trim().to_string();
        edit.location = edit.location.trim().to_string();
        self.db.save_edit(item_id, &edit)?;
        self.emit_item(item_id);
        self.emit_session(&before.session_id);
        self.push_corrections(&before.session_id)?;
        self.db.item(item_id)
    }

    /// Accepts the cards' matches (or takes that back); returns how many changed.
    pub fn review_items(&self, session_id: &str, ids: &[i64], reviewed: bool) -> Result<usize> {
        let n = self.db.set_reviewed(session_id, ids, reviewed)?;
        self.emit_items(session_id, ids);
        Ok(n)
    }

    /// Sets where the cards are kept; returns how many changed.
    pub fn locate_items(&self, session_id: &str, ids: &[i64], location: &str) -> Result<usize> {
        let n = self.db.set_location(session_id, ids, location.trim())?;
        self.emit_items(session_id, ids);
        Ok(n)
    }

    fn emit_items(&self, session_id: &str, ids: &[i64]) {
        for &id in ids {
            self.emit_item(id);
        }
        self.emit_session(session_id);
    }

    /// Deletes cards from a stopped session with the files cardloader made for them (never the scans or the
    /// user's images). Their Cards Cloud rows stay.
    pub fn delete_items(&self, session_id: &str, ids: &[i64]) -> Result<usize> {
        if self.is_running(session_id) {
            return Err(Error::msg("stop the session before deleting cards"));
        }
        if self.ebay_job(session_id).is_some() {
            return Err(Error::msg("wait until the eBay job has finished"));
        }
        for &id in ids {
            let Ok(item) = self.db.item_summary(id) else {
                continue;
            };
            let on_ebay = item.ebay.as_ref().and_then(|e| e.status);
            if matches!(on_ebay, Some(EbayStatus::Draft | EbayStatus::Listed)) {
                return Err(Error::msg(format!(
                    "card {} is on eBay — remove it from eBay first",
                    item.seq
                )));
            }
        }
        let deleted = self.db.delete_items(session_id, ids)?;
        let dir = self.session_dir(session_id);
        for item in &deleted {
            let s = &item.summary;
            let own = [&s.thumb_path, &item.preview_path, &item.back_preview_path]
                .into_iter()
                .flatten()
                .chain(&item.grading_images);
            for path in own.map(PathBuf::from).filter(|p| p.starts_with(&dir)) {
                let _ = std::fs::remove_file(path);
            }
        }
        self.emit_session(session_id);
        Ok(deleted.len())
    }

    /// Sends picks and edits to the session's Cards Cloud copy, if it has one, in the background.
    fn push_corrections(self: &Arc<Self>, session_id: &str) -> Result<()> {
        if self.db.session(session_id)?.remote_session_id.is_some() {
            if let Ok(client) = self.client() {
                tauri::async_runtime::spawn(crate::sync::push_picks_now(
                    self.clone(),
                    session_id.to_string(),
                    client,
                ));
            }
        }
        Ok(())
    }

    /// Waits (a few seconds at most) until the session's run and sync have ended, e.g. after `stop`.
    pub async fn wait_idle(&self, id: &str) {
        let started = std::time::Instant::now();
        while (self.is_running(id)
            || self.syncs.lock().unwrap().contains_key(id)
            || self.ebay_jobs.lock().unwrap().contains_key(id))
            && started.elapsed() < Duration::from_secs(10)
        {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    /// Gives the Cards Cloud copy the session's current name, in the background.
    pub fn push_remote_name(self: &Arc<Self>, id: &str) {
        let Ok(session) = self.db.session(id) else {
            return;
        };
        let (Some(remote), Ok(client)) = (session.remote_session_id, self.client()) else {
            return;
        };
        let runner = self.clone();
        let id = id.to_string();
        tauri::async_runtime::spawn(async move {
            let api = crate::ximilar::cards_cloud::CardsCloudApi::new(client);
            match api.rename(&remote, &session.name).await {
                Err(e) if e.status() != Some(404) => {
                    let why = format!("could not rename the Cards Cloud copy: {e}");
                    let _ = runner.db.set_remote(&id, None, Some(&why));
                    runner.emit_session(&id);
                }
                _ => {}
            }
        });
    }

    pub fn is_running(&self, id: &str) -> bool {
        self.runs.lock().unwrap().contains_key(id)
    }

    /// Starts the session, or resumes it when paused.
    pub fn start(self: &Arc<Self>, id: &str) -> Result<()> {
        self.start_run(id, true)
    }

    /// Analyses the session's queued cards (e.g. ones to analyse again) without reading its source: no new
    /// scan, no new files from the folder.
    pub fn analyse_queued(self: &Arc<Self>, id: &str) -> Result<()> {
        self.start_run(id, false)
    }

    fn start_run(self: &Arc<Self>, id: &str, read_source: bool) -> Result<()> {
        let mut runs = self.runs.lock().unwrap();
        if let Some(run) = runs.get(id) {
            run.paused.send_replace(false);
            self.db
                .set_session_status(id, SessionStatus::Running, None)?;
            self.emit_session(id);
            return Ok(());
        }
        let mut session = self.db.session(id)?;
        if session.source_kind == SourceKind::Scanner {
            let options = with_scanner(&session.options, &self.settings.get().scan);
            if options != session.options {
                self.db
                    .update_session_meta(id, &session.name, session.id_mode, &options)?;
                session.options = options;
            }
        }
        let local = session.options.engine == Engine::Local;
        if local && !self.local.is_ready(&session.options.local_model) {
            return Err(Error::msg(
                "the local model isn't downloaded — download it in Settings, or identify with Ximilar",
            ));
        }
        // A local session needs the Ximilar API only to sync (or for grading jobs started earlier).
        let needs_api =
            !local || session.options.sync_remote || !self.db.grading_jobs(id)?.is_empty();
        let client = match self.client() {
            Ok(client) => Some(client),
            Err(_) if !needs_api => None,
            Err(e) => return Err(e),
        };
        let (paused, paused_rx) = watch::channel(false);
        let cancel = CancellationToken::new();
        runs.insert(
            id.to_string(),
            RunControl {
                paused,
                cancel: cancel.clone(),
            },
        );
        drop(runs);

        self.db
            .set_session_status(id, SessionStatus::Running, None)?;
        self.emit_session(id);
        let runner = self.clone();
        tauri::async_runtime::spawn(async move {
            let id = session.id.clone();
            let outcome = runner
                .clone()
                .run(session, client, read_source, paused_rx, cancel.clone())
                .await;
            runner.runs.lock().unwrap().remove(&id);
            let (status, error) = match outcome {
                Ok(error) if cancel.is_cancelled() => (SessionStatus::Stopped, error),
                Ok(error) => (SessionStatus::Completed, error),
                Err(e) => (SessionStatus::Failed, Some(e.to_string())),
            };
            let _ = runner.db.set_session_status(&id, status, error.as_deref());
            runner.emit_session(&id);
            if status != SessionStatus::Stopped {
                if let Ok(session) = runner.db.session(&id) {
                    runner.events.finished(&session);
                }
            }
        });
        Ok(())
    }

    pub fn pause(&self, id: &str) -> Result<()> {
        self.pause_with_error(id, None)
    }

    /// Pauses the session and says why on it (Resume clears it).
    fn pause_with(&self, id: &str, why: &str) -> Result<()> {
        self.pause_with_error(id, Some(why))
    }

    fn pause_with_error(&self, id: &str, why: Option<&str>) -> Result<()> {
        if let Some(run) = self.runs.lock().unwrap().get(id) {
            run.paused.send_replace(true);
            self.db.set_session_status(id, SessionStatus::Paused, why)?;
            self.emit_session(id);
        }
        Ok(())
    }

    pub fn stop(&self, id: &str) -> Result<()> {
        if let Some(run) = self.runs.lock().unwrap().get(id) {
            run.cancel.cancel();
        } else {
            // Paused after a restart: nothing runs, just mark it.
            let session = self.db.session(id)?;
            if matches!(
                session.status,
                SessionStatus::Paused | SessionStatus::Running
            ) {
                self.db
                    .set_session_status(id, SessionStatus::Stopped, None)?;
                self.emit_session(id);
            }
        }
        if let Some(sync) = self.syncs.lock().unwrap().get(id) {
            sync.cancel();
        }
        self.ebay_cancel(id);
        Ok(())
    }

    /// Starts an eBay job for a session that isn't running. A refresh while another job runs is skipped
    /// (`false`); any other job waits for its turn by failing.
    pub fn ebay_start(self: &Arc<Self>, id: &str, job: Job) -> Result<bool> {
        if self.is_running(id) {
            return Err(Error::msg("wait until the session has finished"));
        }
        let mut jobs = self.ebay_jobs.lock().unwrap();
        if jobs.contains_key(id) {
            return match job {
                Job::Refresh { .. } => Ok(false),
                _ => Err(Error::msg("eBay is still busy with this session")),
            };
        }
        let cancel = CancellationToken::new();
        jobs.insert(
            id.to_string(),
            EbayJob {
                cancel: cancel.clone(),
                progress: EbayProgress {
                    session_id: id.to_string(),
                    running: true,
                    ..Default::default()
                },
            },
        );
        drop(jobs);
        let runner = self.clone();
        let id = id.to_string();
        tauri::async_runtime::spawn(async move {
            crate::ebay::sync::run(runner.clone(), id.clone(), job, cancel).await;
            let done = runner.ebay_jobs.lock().unwrap().remove(&id);
            if let Some(job) = done {
                runner.events.ebay(&EbayProgress {
                    running: false,
                    ..job.progress
                });
            }
        });
        Ok(true)
    }

    pub fn ebay_cancel(&self, id: &str) {
        if let Some(job) = self.ebay_jobs.lock().unwrap().get(id) {
            job.cancel.cancel();
        }
    }

    /// The session's running eBay job.
    pub fn ebay_job(&self, id: &str) -> Option<EbayProgress> {
        self.ebay_jobs
            .lock()
            .unwrap()
            .get(id)
            .map(|j| j.progress.clone())
    }

    pub fn ebay_progress(&self, progress: &EbayProgress) {
        if let Some(job) = self.ebay_jobs.lock().unwrap().get_mut(&progress.session_id) {
            job.progress = progress.clone();
        }
        self.events.ebay(progress);
    }

    pub fn stop_all(&self) {
        for run in self.runs.lock().unwrap().values() {
            run.cancel.cancel();
        }
    }

    /// Mirrors the session to Cards Cloud once (for sessions not currently running), unless already syncing.
    pub fn sync_now(self: &Arc<Self>, id: &str) -> Result<()> {
        if self.is_running(id) || self.syncs.lock().unwrap().contains_key(id) {
            return Ok(());
        }
        let client = self.client()?;
        let cancel = CancellationToken::new();
        self.syncs
            .lock()
            .unwrap()
            .insert(id.to_string(), cancel.clone());
        let close = self.db.session(id)?.status == SessionStatus::Completed;
        let (_tx, finished) = watch::channel(Some(close));
        let runner = self.clone();
        let id = id.to_string();
        tauri::async_runtime::spawn(async move {
            crate::sync::run(runner.clone(), id.clone(), client, finished, cancel).await;
            runner.syncs.lock().unwrap().remove(&id);
            runner.emit_session(&id);
        });
        Ok(())
    }

    /// The session's main loop. Returns a non-fatal error message to keep on the session (e.g. a
    /// scanner error after some cards were scanned).
    async fn run(
        self: Arc<Self>,
        session: Session,
        client: Option<XimilarClient>,
        read_source: bool,
        mut paused: watch::Receiver<bool>,
        cancel: CancellationToken,
    ) -> Result<Option<String>> {
        let id = session.id.clone();
        // The local model reads one card at a time.
        let concurrency = match session.options.engine {
            Engine::Local => 1,
            Engine::Ximilar => {
                let max = crate::settings::MAX_CONCURRENCY;
                self.settings.get().concurrency.clamp(1, max) as usize
            }
        };

        let (finished_tx, finished_rx) = watch::channel(None);
        let sync_task = client
            .clone()
            .filter(|_| session.options.sync_remote)
            .map(|client| {
                tokio::spawn(crate::sync::run(
                    self.clone(),
                    id.clone(),
                    client,
                    finished_rx.clone(),
                    CancellationToken::new(),
                ))
            });

        // Ingest.
        let new_items = Arc::new(Notify::new());
        let (unit_tx, unit_rx) = mpsc::channel::<Unit>(8);
        let source_task = {
            let runner = self.clone();
            let session = session.clone();
            let cancel = cancel.clone();
            let client = client.clone();
            tokio::spawn(async move {
                if !read_source {
                    return Ok(());
                }
                runner
                    .source(&session, client.as_ref(), unit_tx, cancel)
                    .await
            })
        };
        let ingest_task = {
            let runner = self.clone();
            let notify = new_items.clone();
            let id = id.clone();
            // Scans are named scan_0001… in every session, so only other files are keyed by name.
            let keyed = session.source_kind != SourceKind::Scanner;
            tokio::spawn(async move { runner.ingest(&id, keyed, unit_rx, notify).await })
        };

        // Grading results arrive independently of the dispatcher.
        let (dispatch_done_tx, dispatch_done_rx) = watch::channel(false);
        let grades = session.options.grading != GradingMode::Off
            && session.options.engine == Engine::Ximilar;
        let poller = client
            .clone()
            .filter(|_| grades || !self.db.grading_jobs(&id).unwrap_or_default().is_empty())
            .map(|client| {
                tokio::spawn(self.clone().poll_grading(
                    session.clone(),
                    client,
                    dispatch_done_rx,
                    cancel.clone(),
                ))
            });

        // Dispatch.
        let semaphore = Arc::new(Semaphore::new(concurrency));
        let mut workers: JoinSet<Option<String>> = JoinSet::new();
        let mut fatal: Option<String> = None;
        let mut ingest_task = Some(ingest_task);
        let mut ingest_result: Option<Result<()>> = None;
        loop {
            while let Some(done) = workers.try_join_next() {
                if let Ok(Some(err)) = done {
                    fatal.get_or_insert(err);
                }
            }
            if fatal.is_some() {
                cancel.cancel();
            }
            if cancel.is_cancelled() {
                break;
            }
            if *paused.borrow() {
                tokio::select! {
                    _ = paused.changed() => {},
                    _ = cancel.cancelled() => {},
                }
                continue;
            }
            let permit = tokio::select! {
                p = semaphore.clone().acquire_owned() => p.expect("semaphore open"),
                _ = cancel.cancelled() => break,
            };
            match self.db.claim_next(&id)? {
                Some(item_id) => {
                    let runner = self.clone();
                    let session = session.clone();
                    let client = client.clone();
                    let cancel = cancel.clone();
                    workers.spawn(async move {
                        let r = runner
                            .process(&session, client.as_ref(), item_id, &cancel)
                            .await;
                        drop(permit);
                        r
                    });
                }
                None => {
                    drop(permit);
                    if let Some(task) = ingest_task.as_mut() {
                        if task.is_finished() {
                            ingest_result =
                                Some(ingest_task.take().unwrap().await.unwrap_or(Ok(())));
                            continue; // one more look: the last units may have just been queued
                        }
                    } else if workers.is_empty() {
                        break;
                    } else {
                        // A card in progress may come back to the queue (no connection): wait for it.
                        if let Some(Ok(Some(err))) = workers.join_next().await {
                            fatal.get_or_insert(err);
                        }
                        continue;
                    }
                    tokio::select! {
                        _ = new_items.notified() => {},
                        _ = tokio::time::sleep(Duration::from_millis(500)) => {},
                        _ = cancel.cancelled() => {},
                    }
                }
            }
        }
        while let Some(done) = workers.join_next().await {
            if let Ok(Some(err)) = done {
                fatal.get_or_insert(err);
            }
        }
        dispatch_done_tx.send_replace(true);
        if let Some(poller) = poller {
            let _ = poller.await;
        }
        let source_result = source_task.await.unwrap_or(Ok(()));
        if let Some(task) = ingest_task {
            task.abort();
        }

        // The remote session is closed only when the run completed.
        let completed = fatal.is_none() && !cancel.is_cancelled();
        finished_tx.send_replace(Some(completed));
        if let Some(task) = sync_task {
            let _ = task.await;
        }
        match (fatal, source_result, ingest_result) {
            (Some(e), _, _) => Err(Error::msg(e)),
            (None, Err(e), _) | (None, _, Some(Err(e))) => Ok(Some(e.to_string())),
            _ => Ok(None),
        }
    }

    async fn source(
        &self,
        session: &Session,
        client: Option<&XimilarClient>,
        tx: mpsc::Sender<Unit>,
        cancel: CancellationToken,
    ) -> Result<()> {
        let mut known = self.db.known_paths(&session.id)?;
        let pairing = session.options.pairing;
        // A session linked to Cards Cloud (e.g. continued from another computer) skips the files whose
        // cards are there already.
        if let (Some(remote), Some(client), SourceKind::Folder | SourceKind::WatchFolder) =
            (&session.remote_session_id, client, session.source_kind)
        {
            match CardsCloudApi::new(client.clone()).event_ids(remote).await {
                Ok(keys) => {
                    let dir = Path::new(&session.source_ref);
                    let skip = source::files_with_keys(dir, pairing, &keys)?;
                    tracing::info!(
                        "{} file(s) already in Cards Cloud session {remote}",
                        skip.len()
                    );
                    known.extend(skip);
                }
                // Without the list, a new session would analyse (and pay for) every card again.
                Err(e) if known.is_empty() => {
                    return Err(Error::msg(format!(
                        "could not read which cards the Cards Cloud session has: {e}"
                    )))
                }
                Err(e) => tracing::warn!("Cards Cloud rows of {remote} not read: {e}"),
            }
        }
        match session.source_kind {
            SourceKind::Folder => {
                source::folder(PathBuf::from(&session.source_ref), pairing, known, tx).await
            }
            SourceKind::WatchFolder => {
                source::watch_folder(
                    PathBuf::from(&session.source_ref),
                    pairing,
                    known,
                    tx,
                    cancel,
                )
                .await
            }
            SourceKind::Csv => source::csv(PathBuf::from(&session.source_ref), known, tx).await,
            SourceKind::Scanner => {
                let out = self.session_dir(&session.id).join("scans");
                let app = self
                    .app
                    .as_ref()
                    .ok_or_else(|| Error::msg("no scanner access"))?;
                source::scanner::scan(
                    app,
                    &session.source_ref,
                    &scan_settings(&session.options),
                    out,
                    pairing,
                    tx,
                    cancel,
                )
                .await
            }
        }
    }

    /// Stores the identified rows of a CSV as done cards, with thumbnails of their images when the files
    /// exist. Rows whose images are to be analysed (`reanalyze`) and image-only rows are left to the run.
    pub async fn import_rows(
        &self,
        session_id: &str,
        rows: Vec<CsvRow>,
        reanalyze: bool,
    ) -> Result<usize> {
        let thumbs = self.session_dir(session_id).join("thumbs");
        let mut imported = 0;
        for row in rows {
            if !row.has_card() || (reanalyze && row.front.is_some()) {
                continue;
            }
            let images = match &row.front {
                Some(front) if front.is_file() => {
                    let unit = Unit {
                        front: front.clone(),
                        back: row.back.clone().filter(|b| b.is_file()),
                    };
                    let dir = thumbs.clone();
                    tokio::task::spawn_blocking(move || derive_images(&unit, &dir))
                        .await
                        .map_err(|e| Error::msg(e.to_string()))??
                }
                _ => ItemImages::default(),
            };
            // An image that can't be read only costs the thumbnail; the card is still imported.
            let images = ItemImages {
                error: None,
                ..images
            };
            let grading = (row.grade.is_some() || row.condition.is_some()).then(|| {
                json!({"endpoint": "imported", "response": {"records": [
                    {"grades": {"final": row.grade, "condition": row.condition}}]}})
            });
            let path = |p: &Option<PathBuf>| p.as_ref().map(|p| p.to_string_lossy().into_owned());
            let id = self.db.insert_imported(
                session_id,
                &path(&row.front).unwrap_or_default(),
                path(&row.back).as_deref(),
                &images,
                row.id_type.as_deref().unwrap_or("tcg"),
                &row.card,
                row.distance,
                row.distance.map(recognition::tier),
                row.price,
                row.grade,
                row.condition.as_deref(),
                grading.as_ref(),
            )?;
            if !row.notes.is_empty() || !row.location.is_empty() {
                self.db.set_notes(id, &row.notes, &row.location)?;
            }
            imported += 1;
        }
        self.emit_session(session_id);
        Ok(imported)
    }

    /// Makes each unit's thumbnail and preview, then queues it.
    async fn ingest(
        &self,
        session_id: &str,
        keyed: bool,
        mut rx: mpsc::Receiver<Unit>,
        notify: Arc<Notify>,
    ) -> Result<()> {
        let thumbs = self.session_dir(session_id).join("thumbs");
        while let Some(unit) = rx.recv().await {
            let dir = thumbs.clone();
            let images =
                tokio::task::spawn_blocking(move || derive_images(&unit, &dir).map(|i| (unit, i)))
                    .await
                    .map_err(|e| Error::msg(e.to_string()))?;
            let (unit, images) = images?;
            let item_id = self.db.insert_item(
                session_id,
                &unit.front.to_string_lossy(),
                unit.back
                    .as_ref()
                    .map(|b| b.to_string_lossy().into_owned())
                    .as_deref(),
                &images,
                keyed.then(|| source::file_key(&unit.front)).as_deref(),
            )?;
            self.emit_item(item_id);
            self.emit_session(session_id);
            notify.notify_one();
        }
        Ok(())
    }

    /// Recognises one claimed item and, if the session grades, submits its grading job.
    /// Returns a message when the whole session must stop (bad token, no credits).
    async fn process(
        &self,
        session: &Session,
        client: Option<&XimilarClient>,
        item_id: i64,
        cancel: &CancellationToken,
    ) -> Option<String> {
        self.emit_item(item_id);
        let result = self.recognize_item(session, client, item_id, cancel).await;
        let fatal = match result {
            Ok(()) => None,
            Err(Error::Api(ApiError::Cancelled)) => {
                let _ = self.db.set_item_status(item_id, ItemStatus::Queued, None);
                None
            }
            // Still failing after the retries: the connection (or Ximilar) is down. Every other card would
            // fail the same way, so the session waits for the user instead.
            Err(Error::Api(e)) if e.failure() == Failure::Retry => {
                let _ = self.db.set_item_status(item_id, ItemStatus::Queued, None);
                let _ = self.pause_with(
                    &session.id,
                    &format!(
                        "Paused: Ximilar can’t be reached ({e}). Resume when you are back online."
                    ),
                );
                None
            }
            Err(Error::Api(e)) if e.failure() == Failure::Auth => {
                let _ = self.db.set_item_status(item_id, ItemStatus::Queued, None);
                Some(match e.status() {
                    Some(402) => format!("Out of Ximilar credits ({e})"),
                    _ => format!("The Ximilar API refused the token ({e}) — check it in Settings"),
                })
            }
            Err(e) => {
                let _ = self
                    .db
                    .set_item_status(item_id, ItemStatus::Error, Some(&e.to_string()));
                None
            }
        };
        self.emit_item(item_id);
        self.emit_session(&session.id);
        fatal
    }

    async fn recognize_item(
        &self,
        session: &Session,
        client: Option<&XimilarClient>,
        item_id: i64,
        cancel: &CancellationToken,
    ) -> Result<()> {
        let item = self.db.item(item_id)?;
        if session.options.engine == Engine::Local {
            return self.identify_locally(session, &item).await;
        }
        let client = client.ok_or(ApiError::NoToken)?;
        let preview = item
            .preview_path
            .clone()
            .ok_or_else(|| Error::msg("no preview image"))?;
        let b64 = read_b64(PathBuf::from(&preview)).await?;
        let o = &session.options;
        let back = item.back_preview_path.as_ref().filter(|_| o.both_sides);
        let back_b64 = match back {
            Some(back) => Some(read_b64(PathBuf::from(back)).await?),
            None => None,
        };
        let skip_detection = if o.skip_detection {
            let size = |p: &str| image::image_dimensions(p).map_err(|e| Error::msg(e.to_string()));
            Some(recognition::ImageSizes {
                front: size(&preview)?,
                back: back.map(|b| size(b)).transpose()?,
            })
        } else {
            None
        };
        let opts = RecognitionOptions {
            price_stats: o.price_stats,
            slab: o.slab,
            magic_ai: o.magic_ai,
            both_sides: o.both_sides,
            custom_endpoint: recognition::custom_path(&o.custom_endpoint),
            skip_detection,
            known_category: o.known_category,
            known_side: o.known_side,
            known_rotation: o.known_rotation,
            alphabet: Some(o.alphabet.clone()).filter(|a| !a.is_empty()),
            subcategory: Some(o.subcategory.clone()).filter(|s| !s.is_empty()),
            detects_type: false,
        };
        let (rec, raw, calls) = recognition::recognize(
            client,
            session.id_mode,
            &b64,
            back_b64.as_deref(),
            opts,
            cancel,
        )
        .await?;
        let grading = session.options.grading;
        let next = if grading == GradingMode::Off {
            ItemStatus::Done
        } else {
            ItemStatus::Grading
        };
        self.db.save_recognition(
            item_id,
            &rec.id_type,
            &rec.card,
            rec.confidence.as_deref(),
            rec.distance,
            rec.price,
            rec.slab.as_ref(),
            &raw,
            calls,
            next,
        )?;
        if grading == GradingMode::Off {
            return Ok(());
        }
        self.emit_item(item_id);

        // Grading wants the full scan, not the preview.
        let front = encode(
            PathBuf::from(&item.summary.front_path),
            imaging::GRADING_LONG_EDGE,
        )
        .await?;
        let back = match &item.summary.back_path {
            Some(b) => Some(encode(PathBuf::from(b), imaging::GRADING_LONG_EDGE).await?),
            None => None,
        };
        let body = grading::build_request(
            grading,
            &session.options.condition_mode,
            &front,
            back.as_deref(),
        );
        match grading::submit(client, &body, cancel).await {
            Ok(job) => self.db.set_grading_job(item_id, &job)?,
            Err(e) if matches!(e, ApiError::Cancelled) || e.failure() == Failure::Auth => {
                return Err(e.into())
            }
            Err(e) => self.db.set_item_status(
                item_id,
                ItemStatus::Error,
                Some(&format!("grading: {e}")),
            )?,
        }
        Ok(())
    }

    /// Identifies the card with the on-device model: no API calls, no prices, alternatives or grading.
    async fn identify_locally(&self, session: &Session, item: &ItemDetail) -> Result<()> {
        let image = item
            .preview_path
            .clone()
            .unwrap_or_else(|| item.summary.front_path.clone());
        let model = &session.options.local_model;
        let (rec, raw) = self.local.identify(model, Path::new(&image)).await?;
        let grading = session.options.grading;
        self.db.save_recognition(
            item.summary.id,
            &rec.id_type,
            &rec.card,
            rec.confidence.as_deref(),
            None,
            None,
            None,
            &raw,
            0,
            if grading == GradingMode::Off {
                ItemStatus::Done
            } else {
                ItemStatus::Grading
            },
        )?;
        if grading == GradingMode::Off {
            return Ok(());
        }
        self.emit_item(item.summary.id);
        let sides: Vec<PathBuf> = [Some(image), item.back_preview_path.clone()]
            .into_iter()
            .flatten()
            .map(PathBuf::from)
            .collect();
        let scale = crate::local::grade::scale(&session.options.condition_mode);
        match self.local.grade(model, &sides, grading, scale).await {
            Ok(g) => self.db.save_grading(
                item.summary.id,
                g.grade,
                g.condition.as_deref(),
                &g.raw,
                &[],
            )?,
            Err(e) => self.db.set_item_status(
                item.summary.id,
                ItemStatus::Error,
                Some(&format!("grading: {e}")),
            )?,
        }
        Ok(())
    }

    /// Polls the session's grading jobs until none is left and nothing new can come.
    async fn poll_grading(
        self: Arc<Self>,
        session: Session,
        client: XimilarClient,
        dispatch_done: watch::Receiver<bool>,
        cancel: CancellationToken,
    ) {
        let mode = session.options.grading;
        let dir = self.session_dir(&session.id).join("grading");
        loop {
            let jobs = self.db.grading_jobs(&session.id).unwrap_or_default();
            if jobs.is_empty() && *dispatch_done.borrow() {
                return;
            }
            for (item_id, job) in jobs {
                if cancel.is_cancelled() {
                    return;
                }
                match grading::status(&client, &job, &cancel).await {
                    Ok(JobStatus::Pending) => {}
                    Ok(JobStatus::Done) => {
                        if let Err(e) = self
                            .store_grading(mode, &client, item_id, &job, &dir, &cancel)
                            .await
                        {
                            tracing::warn!("grading result of item {item_id}: {e}");
                            if !matches!(e, Error::Api(ApiError::Cancelled)) {
                                let _ = self.db.set_item_status(
                                    item_id,
                                    ItemStatus::Error,
                                    Some(&format!("grading: {e}")),
                                );
                            }
                        }
                        self.emit_item(item_id);
                        self.emit_session(&session.id);
                    }
                    Ok(JobStatus::Failed(why)) => {
                        let _ = self.db.set_item_status(
                            item_id,
                            ItemStatus::Error,
                            Some(&format!("grading failed: {why}")),
                        );
                        self.emit_item(item_id);
                        self.emit_session(&session.id);
                    }
                    Err(ApiError::Cancelled) => return,
                    Err(e) => tracing::warn!("grading status of item {item_id}: {e}"),
                }
            }
            tokio::select! {
                _ = tokio::time::sleep(GRADING_POLL) => {},
                _ = cancel.cancelled() => return,
            }
        }
    }

    async fn store_grading(
        &self,
        mode: GradingMode,
        client: &XimilarClient,
        item_id: i64,
        job: &str,
        dir: &Path,
        cancel: &CancellationToken,
    ) -> Result<()> {
        let response = grading::result(client, job, cancel).await?;
        // A job submitted in another mode (settings changed since) is read in its own mode.
        let mode = match response["endpoint"].as_str() {
            Some(e) => GradingMode::parse(e),
            None => mode,
        };
        let summary = grading::summarize(mode, &response);
        std::fs::create_dir_all(dir)?;
        let mut images = Vec::new();
        for (record, key, url) in &summary.images {
            let ext = url
                .rsplit('.')
                .next()
                .filter(|e| e.len() <= 4 && !e.contains('/'))
                .unwrap_or("webp");
            let path = dir.join(format!("{item_id}_{record}_{key}.{ext}"));
            match client.download(url).await {
                Ok(bytes) => {
                    std::fs::write(&path, bytes)?;
                    images.push(path.to_string_lossy().into_owned());
                }
                Err(e) => tracing::warn!("grading image {url}: {e}"),
            }
        }
        self.db.save_grading(
            item_id,
            summary.grade,
            summary.condition.as_deref(),
            &json!({"endpoint": mode.as_str(), "response": response}),
            &images,
        )?;
        if let Some(err) = summary.error {
            self.db.set_item_status(
                item_id,
                ItemStatus::Error,
                Some(&format!("grading: {err}")),
            )?;
        }
        Ok(())
    }
}

fn derive_images(unit: &Unit, dir: &Path) -> Result<ItemImages> {
    let stem = uuid::Uuid::new_v4().simple().to_string();
    let mut images = ItemImages::default();
    match imaging::make_derivatives(&unit.front, dir, &stem) {
        Ok((thumb, preview)) => {
            images.thumb = Some(thumb);
            images.preview = Some(preview);
        }
        Err(e) => {
            images.error = Some(format!("cannot read {}: {e}", unit.front.display()));
            return Ok(images);
        }
    }
    if let Some(back) = &unit.back {
        match imaging::make_derivatives(back, dir, &format!("{stem}b")) {
            Ok((_, preview)) => images.back_preview = Some(preview),
            Err(e) => images.error = Some(format!("cannot read {}: {e}", back.display())),
        }
    }
    Ok(images)
}

async fn read_b64(path: PathBuf) -> Result<String> {
    let bytes = tokio::fs::read(&path).await?;
    Ok(imaging::b64(&bytes))
}

async fn encode(path: PathBuf, long_edge: u32) -> Result<String> {
    tokio::task::spawn_blocking(move || imaging::encode_for_upload(&path, long_edge, 92))
        .await
        .map_err(|e| Error::msg(e.to_string()))?
}

/// Recognition gets a preview of at most `PREVIEW_LONG_EDGE` px, which 300 dpi nearly fills; only grading
/// uses the full scan, so a session that doesn't grade scans at 300 dpi at most.
const IDENTIFICATION_DPI: u32 = 300;

fn scan_settings(options: &SessionOptions) -> ScanSettings {
    let grades = options.grading != GradingMode::Off && options.engine == Engine::Ximilar;
    ScanSettings {
        dpi: if grades {
            options.scan.dpi
        } else {
            options.scan.dpi.min(IDENTIFICATION_DPI)
        },
        ..options.scan.clone()
    }
}

/// Scanner sessions run with the scanner settings as they are now, so a change in Settings applies to
/// sessions that are continued too. A duplex scan gives front + back of every card.
fn with_scanner(options: &SessionOptions, scan: &ScanSettings) -> SessionOptions {
    SessionOptions {
        scan: scan.clone(),
        pairing: if scan.duplex {
            Pairing::FrontBack
        } else {
            Pairing::Single
        },
        ..options.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;
    use image::{Rgb, RgbImage};
    use serde_json::Value;
    use std::time::Instant;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, Request, ResponseTemplate};

    #[derive(Default)]
    struct Recorder {
        sessions: Mutex<Vec<Session>>,
        items: Mutex<Vec<ItemSummary>>,
    }

    impl EventSink for Recorder {
        fn session(&self, session: Session) {
            self.sessions.lock().unwrap().push(session);
        }
        fn item(&self, item: ItemSummary) {
            self.items.lock().unwrap().push(item);
        }
    }

    async fn mock_ximilar() -> MockServer {
        let server = MockServer::start().await;
        let ok = |body: Value| ResponseTemplate::new(200).set_body_json(body);
        // Auto mode's /analyze tags and identifies the card in one call.
        for p in ["/collectibles/v2/analyze", "/collectibles/v2/tcg_id"] {
            Mock::given(method("POST"))
            .and(path(p))
            .respond_with(ok(json!({"records": [{"_objects": [{"name": "Card",
                "_tags": {"Category": [{"name": "Card/Trading Card Game"}]}, "_identification": {
                "best_match": {"name": "Charizard ex", "full_name": "Charizard ex OBF 125", "set": "Obsidian Flames",
                    "set_code": "OBF", "card_number": "125", "year": 2023, "subcategory": "Pokemon",
                    "links": {"tcgplayer.com": "https://example.com/1"},
                    "price_stats": [{"stats_type": "ungraded", "value": {"median": 12.5}}]},
                "alternatives": [{"name": "Charizard ex", "full_name": "Charizard ex OBF 223", "set": "Obsidian Flames",
                    "set_code": "OBF", "card_number": "223", "year": 2023,
                    "price_stats": [{"stats_type": "ungraded", "value": {"median": 80.0}}]}],
                "distances": [0.2, 0.34]}},
                {"name": "Slab Label", "_tags": {"Company": [{"name": "PSA"}]},
                 "_identification": {"best_match": {"grade": "10", "certificate_number": "87654321"}}}]}]})))
            .mount(&server)
            .await;
        }
        Mock::given(method("POST"))
            .and(path("/account/v2/request/"))
            .respond_with(ok(json!({"id": "job-1", "status": "CREATED"})))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/account/v2/request/job-1/status"))
            .respond_with(ok(json!({"status": "DONE"})))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/account/v2/request/job-1"))
            .respond_with(ok(json!({"id": "job-1", "status": "DONE", "response": {"endpoint": "grade", "records": [
                {"grades": {"corners": 8.0, "edges": 8.0, "surface": 6.0, "centering": 9.0, "final": 7.5, "condition": "Near Mint"},
                 "_full_url_card": format!("{}/files/full.webp", server.uri())},
                {"grades": {"final": 7.0}}]}})))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/files/full.webp"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"RIFF....WEBP".to_vec()))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/cards-cloud/v2/session/"))
            .respond_with(ok(json!({"id": "remote-1", "status": "live"})))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/cards-cloud/v2/session/remote-1/identifications/"))
            .respond_with(ok(
                json!({"received": 1, "created": 1, "updated": 0, "duplicates": 0}),
            ))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/cards-cloud/v2/session/remote-1/images/"))
            .respond_with(ok(json!({"id": "row-1"})))
            .mount(&server)
            .await;
        Mock::given(method("PATCH"))
            .and(path("/cards-cloud/v2/identification/row-1/"))
            .respond_with(ok(json!({"id": "row-1"})))
            .mount(&server)
            .await;
        for p in ["/cards-cloud/v2/session/remote-1/close/"] {
            Mock::given(method("POST"))
                .and(path(p))
                .respond_with(ok(json!({})))
                .mount(&server)
                .await;
        }
        server
    }

    fn bodies(requests: &[Request], p: &str) -> Vec<Value> {
        requests
            .iter()
            .filter(|r| r.url.path() == p)
            .map(|r| serde_json::from_slice(&r.body).unwrap_or(Value::Null))
            .collect()
    }

    /// A scanner session that is run again scans with the scanner settings as they are in Settings now.
    #[tokio::test(flavor = "multi_thread")]
    async fn scanner_session_uses_current_scan_settings() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::memory().unwrap();
        let scan = ScanSettings {
            dpi: 300,
            duplex: true,
            rotation: ScanRotation::UpsideDown,
            back_first: true,
            ..ScanSettings::default()
        };
        let store = Arc::new(SettingsStore::for_test(
            dir.path().join("settings.json"),
            Settings {
                scan: scan.clone(),
                ..Settings::default()
            },
            "token",
        ));
        let runner = Runner::new(
            None,
            Arc::new(Recorder::default()),
            db.clone(),
            store,
            dir.path().join("data"),
        );
        let session = db
            .create_session(&NewSession {
                name: "Scans".into(),
                source_kind: SourceKind::Scanner,
                source_ref: "fujitsu:fi-8170:1234".into(),
                id_mode: IdMode::Tcg,
                options: SessionOptions {
                    pairing: Pairing::Single,
                    scan: ScanSettings {
                        duplex: false,
                        ..ScanSettings::default()
                    },
                    ..SessionOptions::default()
                },
            })
            .unwrap();

        runner.start(&session.id).unwrap();
        let options = db.session(&session.id).unwrap().options;
        assert_eq!(options.scan, scan);
        assert_eq!(options.pairing, Pairing::FrontBack);
        runner.stop(&session.id).ok();
    }

    #[test]
    fn scans_at_300_dpi_without_grading() {
        let mut options = SessionOptions {
            scan: ScanSettings {
                dpi: 600,
                ..ScanSettings::default()
            },
            ..SessionOptions::default()
        };
        assert_eq!(scan_settings(&options).dpi, 300);
        options.grading = GradingMode::Grade;
        assert_eq!(scan_settings(&options).dpi, 600);
        options.engine = Engine::Local;
        assert_eq!(scan_settings(&options).dpi, 300);
    }

    /// A folder of two front/back cards, auto-detected, graded and synced — end to end against a mock API.
    #[tokio::test(flavor = "multi_thread")]
    async fn folder_session_end_to_end() {
        let server = mock_ximilar().await;
        let dir = tempfile::tempdir().unwrap();
        let cards = dir.path().join("cards");
        std::fs::create_dir_all(&cards).unwrap();
        for name in ["a_front", "a_back", "b_front", "b_back"] {
            RgbImage::from_pixel(630, 880, Rgb([200, 30, 30]))
                .save(cards.join(format!("{name}.png")))
                .unwrap();
        }

        let db = Db::memory().unwrap();
        let settings = Settings {
            api_base_url: server.uri(),
            concurrency: 2,
            ..Settings::default()
        };
        let store = Arc::new(SettingsStore::for_test(
            dir.path().join("settings.json"),
            settings,
            "token",
        ));
        let events = Arc::new(Recorder::default());
        let runner = Runner::new(
            None,
            events.clone(),
            db.clone(),
            store,
            dir.path().join("data"),
        );

        let session = db
            .create_session(&NewSession {
                name: "E2E".into(),
                source_kind: SourceKind::Folder,
                source_ref: cards.to_string_lossy().into_owned(),
                id_mode: IdMode::Auto,
                options: SessionOptions {
                    grading: GradingMode::Grade,
                    pairing: Pairing::FrontBack,
                    sync_remote: true,
                    ..SessionOptions::default()
                },
            })
            .unwrap();
        runner.start(&session.id).unwrap();

        let started = Instant::now();
        let session = loop {
            let s = db.session(&session.id).unwrap();
            if !runner.is_running(&s.id) && s.status != SessionStatus::Running {
                break s;
            }
            assert!(
                started.elapsed() < Duration::from_secs(30),
                "session did not finish: {s:?}"
            );
            tokio::time::sleep(Duration::from_millis(100)).await;
        };

        assert_eq!(session.status, SessionStatus::Completed, "{session:?}");
        assert_eq!(session.counts.total, 2);
        assert_eq!(session.counts.done, 2);
        assert_eq!(session.counts.synced, 2);
        assert_eq!(session.remote_session_id.as_deref(), Some("remote-1"));
        assert_eq!(session.counts.total_price, 25.0);

        let items = db.items(&session.id).unwrap();
        for item in &items {
            assert_eq!(item.card.name, "Charizard ex");
            assert_eq!(item.id_type.as_deref(), Some("tcg"));
            assert_eq!(item.confidence.as_deref(), Some("high"));
            assert_eq!(item.grade, Some(7.5));
            assert_eq!(item.condition.as_deref(), Some("Near Mint"));
            let slab = item.slab.as_ref().unwrap();
            assert_eq!(
                (&*slab.company, &*slab.grade, &*slab.certificate),
                ("PSA", "10", "87654321")
            );
            assert!(item.back_path.as_deref().unwrap().ends_with("_back.png"));
            let detail = db.item(item.id).unwrap();
            assert_eq!(detail.calls, 2, "analyze + grade");
            let recognition = detail.recognition.unwrap();
            assert_eq!(recognition["endpoint"], "collectibles/v2/analyze");
            assert_eq!(recognition["id_type"], "tcg");
            assert_eq!(detail.grading_images.len(), 1);
            assert!(Path::new(&detail.grading_images[0]).is_file());
            assert!(Path::new(detail.back_preview_path.as_deref().unwrap()).is_file());
        }

        let requests = server.received_requests().await.unwrap();
        assert!(
            bodies(&requests, "/collectibles/v2/tcg_id").is_empty(),
            "one call identifies"
        );
        let analyzed = bodies(&requests, "/collectibles/v2/analyze");
        assert_eq!(analyzed.len(), 2);
        assert_eq!(analyzed[0]["price_stats"], true);
        let grading = bodies(&requests, "/account/v2/request/");
        assert_eq!(grading.len(), 2);
        assert_eq!(
            grading[0]["records"].as_array().unwrap().len(),
            2,
            "front and back are graded together"
        );
        let uploads = bodies(
            &requests,
            "/cards-cloud/v2/session/remote-1/identifications/",
        );
        let rows: Vec<&Value> = uploads
            .iter()
            .flat_map(|b| b["identifications"].as_array().unwrap())
            .collect();
        assert!(
            rows.iter().any(|r| r["grading"]["grades"]["final"] == 7.5),
            "graded rows are re-sent"
        );
        assert!(rows
            .iter()
            .all(|r| r["name"] == "Charizard ex" && r["price_stats"].is_array()));
        assert_eq!(
            bodies(&requests, "/cards-cloud/v2/session/remote-1/images/").len(),
            2
        );
        assert_eq!(
            bodies(&requests, "/cards-cloud/v2/session/remote-1/close/").len(),
            1
        );
        let created = bodies(&requests, "/cards-cloud/v2/session/");
        assert_eq!(created[0]["client"]["name"], "cardloader");

        assert!(
            events.items.lock().unwrap().len() >= 6,
            "items are announced as they change"
        );
        assert!(events
            .sessions
            .lock()
            .unwrap()
            .iter()
            .any(|s| s.status == SessionStatus::Completed));

        // The user picks the other candidate: the card follows it and Cards Cloud gets a PATCH.
        let first = items[0].id;
        let picked = runner.pick_match(first, 0).unwrap();
        assert_eq!(picked.summary.card.card_number, "223");
        assert_eq!(picked.summary.price, Some(80.0));
        assert_eq!(picked.summary.confidence.as_deref(), Some("medium"));
        assert!(picked.summary.picked && !picked.summary.synced);
        assert_eq!(picked.summary.grade, Some(7.5), "grading stays");
        assert_eq!(picked.summary.slab, items[0].slab, "so does the slab label");
        let started = Instant::now();
        while !db.item_summary(first).unwrap().synced {
            assert!(started.elapsed() < Duration::from_secs(10), "pick not sent");
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        let requests = server.received_requests().await.unwrap();
        let patches = bodies(&requests, "/cards-cloud/v2/identification/row-1/");
        assert_eq!(patches.len(), 1);
        assert_eq!(patches[0]["card_number"], "223");
        assert_eq!(patches[0]["price"], 80.0);
        assert_eq!(db.session(&session.id).unwrap().counts.total_price, 92.5);
        // Picking the original back restores it.
        let back = runner.pick_match(first, 0).unwrap();
        assert_eq!(back.summary.card.card_number, "125");
        assert!(back.summary.reviewed, "a pick is a review");
        assert!(runner.pick_match(first, 7).is_err());
        let wait_synced = || async {
            let started = Instant::now();
            while !db.item_summary(first).unwrap().synced {
                assert!(started.elapsed() < Duration::from_secs(10), "edit not sent");
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        };
        wait_synced().await;

        // A typed correction travels as a PATCH too; notes and the location stay local.
        let card = back.summary.card.clone();
        let edit = ItemEdit {
            card: CardFields {
                name: " Charizard ".into(),
                card_number: "4".into(),
                ..card.clone()
            },
            price: Some(300.0),
            notes: " crease ".into(),
            location: "Box 1".into(),
        };
        let edited = runner.edit_item(first, edit.clone()).unwrap().summary;
        assert!(edited.edited && edited.reviewed && !edited.synced);
        assert_eq!(edited.card.name, "Charizard");
        assert_eq!(
            edited.card.full_name,
            format!("Charizard {} #4", card.set_name),
            "the full name follows"
        );
        assert_eq!((&*edited.notes, &*edited.location), ("crease", "Box 1"));
        wait_synced().await;
        let requests = server.received_requests().await.unwrap();
        let patches = bodies(&requests, "/cards-cloud/v2/identification/row-1/");
        assert_eq!(patches.len(), 3);
        assert_eq!(
            (&patches[2]["name"], &patches[2]["price"]),
            (&json!("Charizard"), &json!(300.0))
        );
        let notes_only = ItemEdit {
            card: edited.card.clone(),
            notes: "no crease".into(),
            ..edit
        };
        runner.edit_item(first, notes_only).unwrap();
        assert!(db.item_summary(first).unwrap().synced, "nothing to send");

        // A deleted card takes its thumbnails along and stays deleted when the folder runs again.
        let second = db.item(items[1].id).unwrap();
        assert_eq!(
            runner
                .delete_items(&session.id, &[second.summary.id])
                .unwrap(),
            1
        );
        assert!(!Path::new(second.summary.thumb_path.as_deref().unwrap()).exists());
        assert!(
            Path::new(&second.summary.front_path).exists(),
            "never the user's images"
        );
        runner.start(&session.id).unwrap();
        let started = Instant::now();
        while runner.is_running(&session.id) {
            assert!(
                started.elapsed() < Duration::from_secs(30),
                "rerun did not finish"
            );
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert_eq!(db.session(&session.id).unwrap().counts.total, 1);

        // Analysing a card again doesn't read the source: a new file stays out (for a scanner: no new scan).
        RgbImage::from_pixel(630, 880, Rgb([1, 2, 3]))
            .save(cards.join("c_front.png"))
            .unwrap();
        let calls = db.item(first).unwrap().calls;
        assert_eq!(db.requeue(&session.id, Some(&[first])).unwrap(), 1);
        runner.analyse_queued(&session.id).unwrap();
        let started = Instant::now();
        while runner.is_running(&session.id) {
            assert!(
                started.elapsed() < Duration::from_secs(30),
                "rerun did not finish"
            );
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        let s = db.session(&session.id).unwrap();
        assert_eq!((s.counts.total, s.counts.done), (1, 1), "{s:?}");
        assert!(db.item(first).unwrap().calls > calls, "analysed again");
    }

    /// A session continued from Cards Cloud: the files whose cards are there already are skipped, the rest
    /// are added to the (reopened) remote session under keys made from their names.
    #[tokio::test(flavor = "multi_thread")]
    async fn continuing_a_cards_cloud_session() {
        let server = mock_ximilar().await;
        let ok = |body: Value| ResponseTemplate::new(200).set_body_json(body);
        Mock::given(method("GET"))
            .and(path("/cards-cloud/v2/session/remote-1/"))
            .respond_with(ok(json!({"id": "remote-1", "status": "closed"})))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/cards-cloud/v2/session/remote-1/reopen/"))
            .respond_with(ok(json!({"id": "remote-1", "status": "live"})))
            .mount(&server)
            .await;
        let done_elsewhere = source::file_key(Path::new("a.png"));
        Mock::given(method("GET"))
            .and(path("/cards-cloud/v2/identification/"))
            .respond_with(ok(json!({"count": 2, "next": null, "previous": null, "results": [
                {"id": "row-0", "event_id": done_elsewhere}, {"id": "row-9", "event_id": "from-a-stream"}]})))
            .mount(&server)
            .await;

        let dir = tempfile::tempdir().unwrap();
        let cards = dir.path().join("cards");
        std::fs::create_dir_all(&cards).unwrap();
        for name in ["a", "b", "c"] {
            RgbImage::from_pixel(630, 880, Rgb([200, 30, 30]))
                .save(cards.join(format!("{name}.png")))
                .unwrap();
        }
        let db = Db::memory().unwrap();
        let settings = Settings {
            api_base_url: server.uri(),
            ..Settings::default()
        };
        let store = Arc::new(SettingsStore::for_test(
            dir.path().join("settings.json"),
            settings,
            "token",
        ));
        let runner = Runner::new(
            None,
            Arc::new(Recorder::default()),
            db.clone(),
            store,
            dir.path().join("data"),
        );
        let session = db
            .create_session(&NewSession {
                name: "Continued".into(),
                source_kind: SourceKind::Folder,
                source_ref: cards.to_string_lossy().into_owned(),
                id_mode: IdMode::Tcg,
                options: SessionOptions {
                    sync_remote: true,
                    ..SessionOptions::default()
                },
            })
            .unwrap();
        db.set_remote(&session.id, Some("remote-1"), None).unwrap();
        runner.start(&session.id).unwrap();

        let started = Instant::now();
        let session = loop {
            let s = db.session(&session.id).unwrap();
            if !runner.is_running(&s.id) && s.status != SessionStatus::Running {
                break s;
            }
            assert!(started.elapsed() < Duration::from_secs(30), "{s:?}");
            tokio::time::sleep(Duration::from_millis(100)).await;
        };
        assert_eq!(session.status, SessionStatus::Completed, "{session:?}");
        assert_eq!(session.counts.total, 2, "a.png is in Cards Cloud already");
        let fronts: Vec<String> = db
            .items(&session.id)
            .unwrap()
            .iter()
            .map(|i| i.front_path.clone())
            .collect();
        assert!(
            fronts[0].ends_with("b.png") && fronts[1].ends_with("c.png"),
            "{fronts:?}"
        );

        let requests = server.received_requests().await.unwrap();
        assert!(
            bodies(&requests, "/cards-cloud/v2/session/").is_empty(),
            "no new remote session"
        );
        assert_eq!(
            bodies(&requests, "/cards-cloud/v2/session/remote-1/reopen/").len(),
            1
        );
        let mut keys: Vec<String> = bodies(
            &requests,
            "/cards-cloud/v2/session/remote-1/identifications/",
        )
        .iter()
        .flat_map(|b| b["identifications"].as_array().unwrap().clone())
        .map(|r| r["event_id"].as_str().unwrap().to_string())
        .collect();
        keys.sort();
        keys.dedup();
        let mut expected = vec![
            source::file_key(Path::new("b.png")),
            source::file_key(Path::new("c.png")),
        ];
        expected.sort();
        assert_eq!(keys, expected);
    }

    /// Sports cards named `_F`/`_B`, sent front and back together with `magic_ai` to a custom endpoint.
    #[tokio::test(flavor = "multi_thread")]
    async fn both_sides_to_a_custom_endpoint() {
        let server = MockServer::start().await;
        let side = |name: &str, d: f64| {
            json!({"_objects": [{"name": "Card", "_identification": {"best_match": {"name": name,
                "full_name": format!("1986 Fleer {name} #57"), "year": 1986, "card_number": "57"},
                "distances": [d]}}]})
        };
        Mock::given(method("POST"))
            .and(path("/collectibles/v2/acme_sport_id"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "records": [side("Michael Jordan", 0.0), side("Michael Jordan", 0.0)],
                "magic_ai_used": true})))
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let cards = dir.path().join("cards");
        std::fs::create_dir_all(&cards).unwrap();
        for name in ["mj_F.jpg", "mj_B.jpg", "rookie-f.png", "rookie-b.png"] {
            let back = name.contains("_B") || name.contains("-b");
            RgbImage::from_pixel(630, 880, Rgb([30, if back { 20 } else { 120 }, 30]))
                .save(cards.join(name))
                .unwrap();
        }
        let db = Db::memory().unwrap();
        let store = Arc::new(SettingsStore::for_test(
            dir.path().join("settings.json"),
            Settings {
                api_base_url: server.uri(),
                ..Settings::default()
            },
            "token",
        ));
        let runner = Runner::new(
            None,
            Arc::new(Recorder::default()),
            db.clone(),
            store,
            dir.path().join("data"),
        );
        let session = db
            .create_session(&NewSession {
                name: "Sports".into(),
                source_kind: SourceKind::Folder,
                source_ref: cards.to_string_lossy().into_owned(),
                id_mode: IdMode::Sport,
                options: SessionOptions {
                    pairing: Pairing::FrontBack,
                    both_sides: true,
                    magic_ai: true,
                    custom_endpoint: "/collectibles/v2/acme_sport_id".into(),
                    skip_detection: true,
                    known_category: true,
                    ..SessionOptions::default()
                },
            })
            .unwrap();
        runner.start(&session.id).unwrap();
        let started = Instant::now();
        while runner.is_running(&session.id) {
            assert!(
                started.elapsed() < Duration::from_secs(20),
                "run did not finish"
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        let items = db.items(&session.id).unwrap();
        assert_eq!(items.len(), 2, "two cards, not four images");
        for item in &items {
            assert_eq!(item.status, ItemStatus::Done, "{item:?}");
            assert_eq!(item.card.name, "Michael Jordan");
            assert_eq!(item.id_type.as_deref(), Some("sport"));
            let detail = db.item(item.id).unwrap();
            assert_eq!(detail.calls, 2, "the request + magic_ai");
            let raw = detail.recognition.unwrap();
            assert_eq!(raw["endpoint"], "collectibles/v2/acme_sport_id");
            assert_eq!(raw["sides"], json!(["front", "back"]));
        }
        let requests = server.received_requests().await.unwrap();
        let sent = bodies(&requests, "/collectibles/v2/acme_sport_id");
        assert_eq!(sent.len(), 2);
        for body in sent {
            assert_eq!(body["magic_ai"], true);
            let records = body["records"].as_array().unwrap();
            assert_eq!(records.len(), 2);
            assert_eq!(records[0]["_id"], "front");
            assert_eq!(records[1]["_id"], "back");
            assert_ne!(records[0]["_base64"], records[1]["_base64"]);
            for r in records {
                assert_eq!(
                    r["_objects"],
                    json!([{"name": "Card", "prob": 1.0, "bound_box": [0, 0, 630, 880],
                        "Top Category": "Card", "Category": "Card/Sport Card"}]),
                    "each side is one card over the whole image, of the session's type"
                );
            }
        }
    }

    /// A CSV with one identified card and one image to analyse: the card is imported as is, the image is
    /// analysed by the run, and an export of the session imports back to the same cards.
    #[tokio::test(flavor = "multi_thread")]
    async fn csv_import_end_to_end() {
        let server = mock_ximilar().await;
        let dir = tempfile::tempdir().unwrap();
        for name in ["known", "new"] {
            RgbImage::from_pixel(630, 880, Rgb([10, 90, 200]))
                .save(dir.path().join(format!("{name}.png")))
                .unwrap();
        }
        let csv = dir.path().join("inventory.csv");
        std::fs::write(
            &csv,
            "Image;Card Name;Set;No.;Price\nknown.png;Black Lotus;Alpha;232;18500,5\nnew.png;;;;\n",
        )
        .unwrap();

        let db = Db::memory().unwrap();
        let settings = Settings {
            api_base_url: server.uri(),
            ..Settings::default()
        };
        let store = Arc::new(SettingsStore::for_test(
            dir.path().join("settings.json"),
            settings,
            "token",
        ));
        let runner = Runner::new(
            None,
            Arc::new(Recorder::default()),
            db.clone(),
            store,
            dir.path().join("data"),
        );
        let session = db
            .create_session(&NewSession {
                name: "Import".into(),
                source_kind: SourceKind::Csv,
                source_ref: csv.to_string_lossy().into_owned(),
                id_mode: IdMode::Tcg,
                options: SessionOptions::default(),
            })
            .unwrap();
        let rows = source::csv_file::read(&csv).unwrap().rows;
        assert_eq!(
            runner.import_rows(&session.id, rows, false).await.unwrap(),
            1
        );
        let imported = db.items(&session.id).unwrap();
        assert_eq!(imported.len(), 1);
        assert_eq!(imported[0].status, ItemStatus::Done);
        assert_eq!(imported[0].price, Some(18500.5));
        assert!(
            imported[0].thumb_path.is_some(),
            "thumbnail of the imported card"
        );

        runner.start(&session.id).unwrap();
        let started = Instant::now();
        while runner.is_running(&session.id) {
            assert!(
                started.elapsed() < Duration::from_secs(20),
                "run did not finish"
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        let items = db.items(&session.id).unwrap();
        assert_eq!(
            items.len(),
            2,
            "the image-only row was analysed, the known one not again"
        );
        assert_eq!(items[0].card.name, "Black Lotus");
        assert_eq!(items[1].card.name, "Charizard ex");
        assert_eq!(
            db.session(&session.id).unwrap().status,
            SessionStatus::Completed
        );

        // Round trip: export, then import into a new session.
        let exported = dir.path().join("export.csv");
        crate::export::write_csv(&items, &exported).unwrap();
        let again = source::csv_file::read(&exported).unwrap().rows;
        assert_eq!(again.len(), 2);
        assert!(again.iter().all(|r| r.has_card()));
        assert_eq!(again[1].card.full_name, "Charizard ex OBF 125");
        assert_eq!(again[1].price, Some(12.5));
    }

    /// A session identified by the on-device model: no token, no API calls; the answer becomes the card.
    #[tokio::test(flavor = "multi_thread")]
    async fn local_model_session_without_a_token() {
        let dir = tempfile::tempdir().unwrap();
        let cards = dir.path().join("cards");
        std::fs::create_dir_all(&cards).unwrap();
        RgbImage::from_pixel(630, 880, Rgb([240, 200, 20]))
            .save(cards.join("pikachu.png"))
            .unwrap();
        let db = Db::memory().unwrap();
        // No API base, no token: any call to Ximilar would fail.
        let store = Arc::new(SettingsStore::for_test(
            dir.path().join("settings.json"),
            Settings {
                api_base_url: "http://127.0.0.1:9".into(),
                ..Settings::default()
            },
            "",
        ));
        let runner = Runner::new(
            None,
            Arc::new(Recorder::default()),
            db.clone(),
            store,
            dir.path().join("data"),
        );
        let new = NewSession {
            name: "Offline".into(),
            source_kind: SourceKind::Folder,
            source_ref: cards.to_string_lossy().into_owned(),
            id_mode: IdMode::Auto,
            options: SessionOptions {
                engine: Engine::Local,
                grading: GradingMode::Grade,
                ..SessionOptions::default()
            },
        };
        let session = db.create_session(&new).unwrap();
        let err = runner.start(&session.id).unwrap_err();
        assert!(err.to_string().contains("isn't downloaded"), "{err}");

        crate::local::testing::install_fake(
            &runner.local,
            r#"{"name": "Pikachu", "year": "1999", "game": "pokemon", "card_number": "58/102", "set_code": "",
                "corners": 9, "edges": 8.5, "surface": 7, "centering": 9}"#,
        );
        runner.start(&session.id).unwrap();
        let started = Instant::now();
        while runner.is_running(&session.id) {
            assert!(
                started.elapsed() < Duration::from_secs(20),
                "run did not finish"
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        let s = db.session(&session.id).unwrap();
        assert_eq!(s.status, SessionStatus::Completed, "{s:?}");
        let item = db.item(db.items(&session.id).unwrap()[0].id).unwrap();
        assert_eq!(item.summary.status, ItemStatus::Done);
        // Graded by the local model: mean 8.375, capped at the surface's 7 + 1.
        assert_eq!(item.summary.grade, Some(8.0));
        // On the default scale, eBay's for trading cards.
        assert_eq!(
            item.summary.condition.as_deref(),
            Some("Near Mint or Better")
        );
        let grading = item.grading.clone().unwrap();
        assert_eq!(grading["engine"], "local/lfm2.5-vl-3b");
        assert_eq!(grading["response"]["records"][0]["grades"]["surface"], 7.0);
        assert_eq!(item.summary.card.full_name, "Pikachu #58/102");
        assert_eq!(item.summary.confidence.as_deref(), Some("local"));
        assert_eq!(item.summary.price, None);
        assert_eq!(item.calls, 0);
        assert_eq!(item.recognition.unwrap()["endpoint"], "local/lfm2.5-vl-3b");

        // Syncing still needs Ximilar.
        db.update_session_meta(
            &s.id,
            &s.name,
            s.id_mode,
            &SessionOptions {
                sync_remote: true,
                ..s.options.clone()
            },
        )
        .unwrap();
        assert!(runner.start(&s.id).is_err(), "no token to sync with");
    }

    /// A rejected token stops the whole session instead of failing every card.
    #[tokio::test(flavor = "multi_thread")]
    async fn bad_token_fails_the_session() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(401).set_body_json(json!({"detail": "Invalid token."})),
            )
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        RgbImage::from_pixel(100, 140, Rgb([1, 2, 3]))
            .save(dir.path().join("x.png"))
            .unwrap();
        let db = Db::memory().unwrap();
        let settings = Settings {
            api_base_url: server.uri(),
            ..Settings::default()
        };
        let store = Arc::new(SettingsStore::for_test(
            dir.path().join("s.json"),
            settings,
            "bad",
        ));
        let runner = Runner::new(
            None,
            Arc::new(Recorder::default()),
            db.clone(),
            store,
            dir.path().join("data"),
        );
        let session = db
            .create_session(&NewSession {
                name: "T".into(),
                source_kind: SourceKind::Folder,
                source_ref: dir.path().to_string_lossy().into_owned(),
                id_mode: IdMode::Tcg,
                options: SessionOptions::default(),
            })
            .unwrap();
        runner.start(&session.id).unwrap();
        let started = Instant::now();
        while runner.is_running(&session.id) {
            assert!(started.elapsed() < Duration::from_secs(20));
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        let s = db.session(&session.id).unwrap();
        assert_eq!(s.status, SessionStatus::Failed);
        assert!(s.error.unwrap().contains("Invalid token"));
        assert_eq!(s.counts.queued, 1, "the card stays queued for the next run");
    }

    /// Without a connection the cards stay queued and the session pauses, saying why; it doesn't fail them.
    #[tokio::test(flavor = "multi_thread")]
    async fn no_connection_pauses_the_session() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["x.png", "y.png"] {
            RgbImage::from_pixel(100, 140, Rgb([1, 2, 3]))
                .save(dir.path().join(name))
                .unwrap();
        }
        let db = Db::memory().unwrap();
        let settings = Settings {
            // Nothing listens on port 9.
            api_base_url: "http://127.0.0.1:9".into(),
            ..Settings::default()
        };
        let store = Arc::new(SettingsStore::for_test(
            dir.path().join("s.json"),
            settings,
            "token",
        ));
        let runner = Runner::new(
            None,
            Arc::new(Recorder::default()),
            db.clone(),
            store,
            dir.path().join("data"),
        );
        let session = db
            .create_session(&NewSession {
                name: "T".into(),
                source_kind: SourceKind::Folder,
                source_ref: dir.path().to_string_lossy().into_owned(),
                id_mode: IdMode::Tcg,
                options: SessionOptions::default(),
            })
            .unwrap();
        runner.start(&session.id).unwrap();
        let started = Instant::now();
        let s = loop {
            let s = db.session(&session.id).unwrap();
            if s.status == SessionStatus::Paused && s.counts.in_progress == 0 && s.counts.total == 2
            {
                break s;
            }
            assert!(started.elapsed() < Duration::from_secs(20), "{s:?}");
            tokio::time::sleep(Duration::from_millis(50)).await;
        };
        assert!(s.error.unwrap().contains("can’t be reached"));
        assert_eq!((s.counts.errors, s.counts.queued), (0, 2));
        assert!(runner.is_running(&session.id), "paused, not ended");
        runner.start(&session.id).unwrap();
        let resumed = db.session(&session.id).unwrap();
        assert_eq!(
            (resumed.status, resumed.error),
            (SessionStatus::Running, None)
        );
        runner.stop(&session.id).unwrap();
    }

    /// eBay's Inventory and Media APIs for two cards: the first is accepted, the second's offer is refused.
    async fn mock_ebay() -> MockServer {
        let server = MockServer::start().await;
        let ok = |status: u16, body: Value| ResponseTemplate::new(status).set_body_json(body);
        Mock::given(method("POST"))
            .and(path("/identity/v1/oauth2/token"))
            .respond_with(ok(
                200,
                json!({"access_token": "access", "expires_in": 7200}),
            ))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/commerce/media/v1_beta/image/create_image_from_file"))
            .respond_with(ok(
                201,
                json!({"imageUrl": "https://i.example.com/card.jpg"}),
            ))
            .mount(&server)
            .await;
        let skus = |r: &Request| -> Vec<String> {
            let body: Value = serde_json::from_slice(&r.body).unwrap();
            body["requests"]
                .as_array()
                .unwrap()
                .iter()
                .map(|e| e["sku"].as_str().unwrap().to_string())
                .collect()
        };
        Mock::given(method("POST"))
            .and(path(
                "/sell/inventory/v1/bulk_create_or_replace_inventory_item",
            ))
            .respond_with(move |r: &Request| {
                let entries: Vec<Value> = skus(r)
                    .into_iter()
                    .map(|sku| json!({"statusCode": 200, "sku": sku, "locale": "en_US"}))
                    .collect();
                ResponseTemplate::new(207).set_body_json(json!({"responses": entries}))
            })
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/sell/inventory/v1/bulk_create_offer"))
            .respond_with(move |r: &Request| {
                let entries: Vec<Value> = skus(r)
                    .into_iter()
                    .enumerate()
                    .map(|(i, sku)| match i {
                        0 => json!({"statusCode": 200, "sku": sku, "offerId": "offer-1"}),
                        _ => json!({"statusCode": 400, "sku": sku,
                            "errors": [{"errorId": 25005, "message": "Invalid category."}]}),
                    })
                    .collect();
                ResponseTemplate::new(207).set_body_json(json!({"responses": entries}))
            })
            .mount(&server)
            .await;
        Mock::given(method("PUT"))
            .and(path("/sell/inventory/v1/offer/offer-1"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/sell/inventory/v1/bulk_publish_offer"))
            .respond_with(ok(
                207,
                json!({"responses": [{"statusCode": 200, "offerId": "offer-1", "listingId": "1100"}]}),
            ))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/sell/inventory/v1/offer/offer-1"))
            .respond_with(ok(
                200,
                json!({"offerId": "offer-1", "status": "PUBLISHED",
                    "listing": {"listingId": "1100", "listingStatus": "ACTIVE", "soldQuantity": 1}}),
            ))
            .mount(&server)
            .await;
        server
    }

    async fn wait_ebay(runner: &Runner, id: &str) {
        let started = Instant::now();
        while runner.ebay_job(id).is_some() {
            assert!(
                started.elapsed() < Duration::from_secs(20),
                "eBay job did not end"
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    /// A finished session's cards become eBay drafts, one is repriced, published, and read back as sold.
    #[tokio::test(flavor = "multi_thread")]
    async fn ebay_listing_end_to_end() {
        use crate::ebay::listing::CardCondition;
        use crate::ebay::sync::{DraftItem, Job};
        use crate::settings::{EbaySettings, Secret};

        let ximilar = mock_ximilar().await;
        let ebay = mock_ebay().await;
        let dir = tempfile::tempdir().unwrap();
        let cards = dir.path().join("cards");
        std::fs::create_dir_all(&cards).unwrap();
        for name in ["a", "b"] {
            RgbImage::from_pixel(630, 880, Rgb([20, 30, 200]))
                .save(cards.join(format!("{name}.png")))
                .unwrap();
        }
        let db = Db::memory().unwrap();
        let settings = Settings {
            api_base_url: ximilar.uri(),
            ebay: EbaySettings {
                app_id: "Demo-App".into(),
                ru_name: "Demo-RuName".into(),
                api_base: ebay.uri(),
                fulfillment_policy_id: "ship-1".into(),
                payment_policy_id: "pay-1".into(),
                return_policy_id: "return-1".into(),
                location_key: "home".into(),
                ..EbaySettings::default()
            },
            ..Settings::default()
        };
        let store = Arc::new(SettingsStore::for_test(
            dir.path().join("settings.json"),
            settings,
            "token",
        ));
        store.set_secret(Secret::EbayCert, "cert").unwrap();
        store.set_secret(Secret::EbayRefresh, "refresh").unwrap();
        let runner = Runner::new(
            None,
            Arc::new(Recorder::default()),
            db.clone(),
            store,
            dir.path().join("data"),
        );
        let session = db
            .create_session(&NewSession {
                name: "To sell".into(),
                source_kind: SourceKind::Folder,
                source_ref: cards.to_string_lossy().into_owned(),
                id_mode: IdMode::Auto,
                options: SessionOptions::default(),
            })
            .unwrap();
        runner.start(&session.id).unwrap();
        let started = Instant::now();
        while runner.is_running(&session.id) {
            assert!(started.elapsed() < Duration::from_secs(30));
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        let items = db.items(&session.id).unwrap();
        assert_eq!(items.len(), 2);
        let (first, second) = (items[0].id, items[1].id);

        let drafts = |prices: &[(i64, f64)]| Job::Drafts {
            items: prices
                .iter()
                .map(|&(item_id, price)| DraftItem { item_id, price })
                .collect(),
            condition: CardCondition::NearMint,
        };
        assert!(runner
            .ebay_start(&session.id, drafts(&[(first, 12.5), (second, 30.0)]))
            .unwrap());
        wait_ebay(&runner, &session.id).await;

        let requests = ebay.received_requests().await.unwrap();
        let token = requests
            .iter()
            .find(|r| r.url.path() == "/identity/v1/oauth2/token")
            .unwrap();
        assert!(String::from_utf8_lossy(&token.body).contains("grant_type=refresh_token"));
        assert!(token.headers["authorization"]
            .to_str()
            .unwrap()
            .starts_with("Basic "));
        assert_eq!(
            bodies(
                &requests,
                "/commerce/media/v1_beta/image/create_image_from_file"
            )
            .len(),
            2,
            "one picture per card"
        );
        let inventory = bodies(
            &requests,
            "/sell/inventory/v1/bulk_create_or_replace_inventory_item",
        );
        let entry = &inventory[0]["requests"][0];
        assert_eq!(entry["condition"], "LIKE_NEW", "the cards are in PSA slabs");
        assert_eq!(
            entry["conditionDescriptors"],
            json!([{"name": "27501", "values": ["275010"]}, {"name": "27502", "values": ["275020"]},
                   {"name": "27503", "additionalInfo": "87654321"}])
        );
        assert_eq!(
            entry["product"]["imageUrls"],
            json!(["https://i.example.com/card.jpg"])
        );
        assert_eq!(entry["product"]["aspects"]["Game"], json!(["Pokémon TCG"]));
        assert_eq!(
            entry["product"]["title"],
            "2023 Pokemon Obsidian Flames Charizard ex #125 PSA 10"
        );
        let offers = bodies(&requests, "/sell/inventory/v1/bulk_create_offer");
        let offer = &offers[0]["requests"][0];
        assert_eq!(offer["pricingSummary"]["price"]["value"], "12.50");
        assert_eq!(offer["listingPolicies"]["fulfillmentPolicyId"], "ship-1");
        assert_eq!(offer["merchantLocationKey"], "home");
        assert_eq!(offer["categoryId"], "183454");

        let listing = db.item_summary(first).unwrap().ebay.unwrap();
        assert_eq!(
            (listing.status, listing.price),
            (Some(EbayStatus::Draft), Some(12.5))
        );
        let refused = db.item_summary(second).unwrap().ebay.unwrap();
        assert_eq!(refused.status, None);
        assert_eq!(refused.error.as_deref(), Some("Invalid category. (25005)"));
        assert_eq!(db.session(&session.id).unwrap().counts.ebay_drafts, 1);

        // A new price updates the offer, with the pictures already on eBay.
        runner
            .ebay_start(&session.id, drafts(&[(first, 15.0)]))
            .unwrap();
        wait_ebay(&runner, &session.id).await;
        let requests = ebay.received_requests().await.unwrap();
        let updates = bodies(&requests, "/sell/inventory/v1/offer/offer-1");
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0]["pricingSummary"]["price"]["value"], "15.00");
        assert_eq!(
            bodies(
                &requests,
                "/commerce/media/v1_beta/image/create_image_from_file"
            )
            .len(),
            2
        );
        assert_eq!(
            requests
                .iter()
                .filter(|r| r.url.path() == "/identity/v1/oauth2/token")
                .count(),
            1,
            "the access token is reused"
        );

        runner.ebay_start(&session.id, Job::Publish(None)).unwrap();
        wait_ebay(&runner, &session.id).await;
        let requests = ebay.received_requests().await.unwrap();
        let published = bodies(&requests, "/sell/inventory/v1/bulk_publish_offer");
        assert_eq!(published[0]["requests"], json!([{"offerId": "offer-1"}]));
        let listing = db.item_summary(first).unwrap().ebay.unwrap();
        assert_eq!(listing.status, Some(EbayStatus::Listed));
        assert_eq!(listing.listing_id.as_deref(), Some("1100"));
        assert_eq!(listing.price, Some(15.0));

        runner
            .ebay_start(&session.id, Job::Refresh { force: true })
            .unwrap();
        wait_ebay(&runner, &session.id).await;
        let session = db.session(&session.id).unwrap();
        assert_eq!(
            db.item_summary(first).unwrap().ebay.unwrap().status,
            Some(EbayStatus::Sold)
        );
        assert_eq!(session.counts.ebay_sold, 1);
        assert!(session.ebay_checked_at.is_some());
        assert_eq!(session.ebay_error, None);
    }

    /// A finished session as Whatnot's CSV: pictures go to ImgBB once, and the rows follow Whatnot's template.
    #[tokio::test(flavor = "multi_thread")]
    async fn whatnot_export_uploads_pictures_once() {
        use crate::market::whatnot::{self, ListingType, Options};
        use crate::market::PricedItem;
        use crate::settings::Secret;

        let ximilar = mock_ximilar().await;
        let imgbb = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/1/upload"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                json!({"data": {"url": "https://i.ibb.co/x/card.jpg"}, "success": true}),
            ))
            .mount(&imgbb)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let cards = dir.path().join("cards");
        std::fs::create_dir_all(&cards).unwrap();
        for name in ["a_front", "a_back"] {
            RgbImage::from_pixel(630, 880, Rgb([90, 30, 20]))
                .save(cards.join(format!("{name}.png")))
                .unwrap();
        }
        let db = Db::memory().unwrap();
        let settings = Settings {
            api_base_url: ximilar.uri(),
            image_host_base: imgbb.uri(),
            ..Settings::default()
        };
        let store = Arc::new(SettingsStore::for_test(
            dir.path().join("settings.json"),
            settings,
            "token",
        ));
        store.set_secret(Secret::ImgbbKey, "key").unwrap();
        let runner = Runner::new(
            None,
            Arc::new(Recorder::default()),
            db.clone(),
            store,
            dir.path().join("data"),
        );
        let session = db
            .create_session(&NewSession {
                name: "Show".into(),
                source_kind: SourceKind::Folder,
                source_ref: cards.to_string_lossy().into_owned(),
                id_mode: IdMode::Auto,
                options: SessionOptions {
                    pairing: Pairing::FrontBack,
                    ..SessionOptions::default()
                },
            })
            .unwrap();
        runner.start(&session.id).unwrap();
        let started = Instant::now();
        while runner.is_running(&session.id) {
            assert!(started.elapsed() < Duration::from_secs(30));
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        let item = db.items(&session.id).unwrap()[0].id;
        let options = Options {
            listing_type: ListingType::Auction,
            shipping_profile: "0-1 oz".into(),
            offerable: false,
            condition: CardCondition::NearMint,
            upload_images: true,
        };
        let priced = [PricedItem {
            item_id: item,
            price: 12.5,
        }];
        let cancel = CancellationToken::new();
        let steps = Mutex::new(Vec::new());
        let (text, export) = whatnot::export(&runner, &priced, &options, &cancel, |done, total| {
            steps.lock().unwrap().push((done, total))
        })
        .await
        .unwrap();
        assert_eq!((export.rows, export.without_images), (1, 0));
        assert_eq!(steps.into_inner().unwrap(), vec![(1, 1)]);
        let mut rows = csv::Reader::from_reader(text.as_bytes());
        let row = rows.records().next().unwrap().unwrap();
        assert_eq!(&row[0], "Trading Card Games");
        assert_eq!(&row[1], "Pokémon Cards");
        assert_eq!(
            &row[2],
            "2023 Pokemon Obsidian Flames Charizard ex #125 PSA 10"
        );
        assert_eq!((&row[5], &row[6], &row[10]), ("Auction", "13", "Graded"));
        assert_eq!(
            (&row[13], &row[14]),
            ("https://i.ibb.co/x/card.jpg", "https://i.ibb.co/x/card.jpg")
        );
        assert_eq!(
            imgbb.received_requests().await.unwrap().len(),
            2,
            "front and back"
        );

        // Exported again: the pictures are on ImgBB already.
        let (_, again) = whatnot::export(&runner, &priced, &options, &cancel, |_, _| {})
            .await
            .unwrap();
        assert_eq!(again.without_images, 0);
        assert_eq!(imgbb.received_requests().await.unwrap().len(), 2);
    }
}
