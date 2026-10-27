//! Everything the UI can call. Errors reach the UI as strings.

use std::sync::Arc;

use reqwest::Method;
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, State};

use crate::db::Db;
use crate::ebay::listing::{self as ebay_listing, CardCondition};
use crate::ebay::sync::{DraftItem, EbayProgress, Job};
use crate::ebay::{self, EbaySetup};
use crate::error::{Error, Result};
use crate::export;
use crate::local::compare::{self, CompareReport, CompareRow};
use crate::local::{self, ModelStatus};
use crate::market::{self, shopify, tcgplayer, whatnot, Format, PricedItem};
use crate::mcp::{McpServer, McpStatus};
use crate::model::*;
use crate::runner::Runner;
use crate::settings::{Secret, Settings, SettingsStore, SettingsView};
use crate::source::csv_file::{self, CsvPreview};
use crate::source::{self, scanner};
use crate::ximilar::cards_cloud::{CardsCloudApi, MAX_SESSION_NAME};
use tokio_util::sync::CancellationToken;

/// The app's state. Tauri commands and the MCP server both work through it; the operations with more to them
/// than one `Db` or `Runner` call are its methods, so both do them alike.
#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub settings: Arc<SettingsStore>,
    pub runner: Arc<Runner>,
}

type S<'a> = State<'a, AppState>;

// ---- settings ------------------------------------------------------------------------------------

#[tauri::command]
pub fn get_settings(state: S) -> SettingsView {
    state.settings.view()
}

/// Saves the settings; the MCP server starts, stops or moves as they say.
#[tauri::command]
pub async fn save_settings(
    state: S<'_>,
    mcp: State<'_, Arc<McpServer>>,
    settings: Settings,
) -> Result<SettingsView> {
    state.settings.update(settings)?;
    mcp.apply().await;
    Ok(state.settings.view())
}

/// Whether the MCP server runs, where, and the token agents send.
#[tauri::command]
pub async fn mcp_status(mcp: State<'_, Arc<McpServer>>) -> Result<McpStatus> {
    Ok(mcp.status().await)
}

/// A new token for agents; the old one stops working.
#[tauri::command]
pub async fn regenerate_mcp_token(mcp: State<'_, Arc<McpServer>>) -> Result<McpStatus> {
    mcp.regenerate_token()?;
    Ok(mcp.status().await)
}

#[tauri::command]
pub fn set_token(state: S, token: String) -> Result<SettingsView> {
    state.settings.set_token(&token)?;
    Ok(state.settings.view())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionCheck {
    pub ok: bool,
    pub account: Option<String>,
    pub error: Option<String>,
    pub cards_cloud: bool,
    pub cards_cloud_error: Option<String>,
}

#[tauri::command]
pub async fn test_connection(state: S<'_>) -> Result<ConnectionCheck> {
    state.connection().await
}

impl AppState {
    pub async fn connection(&self) -> Result<ConnectionCheck> {
        let client = self.runner.client()?;
        let account = client
            .request(Method::GET, "account/v2/details/", &[], None)
            .await;
        let cards_cloud = CardsCloudApi::new(client).ping().await;
        Ok(ConnectionCheck {
            ok: account.is_ok(),
            account: account.as_ref().ok().map(|a| {
                a["email"]
                    .as_str()
                    .or(a["username"].as_str())
                    .unwrap_or("authenticated")
                    .to_string()
            }),
            error: account.err().map(|e| e.to_string()),
            cards_cloud: cards_cloud.is_ok(),
            cards_cloud_error: cards_cloud.err().map(|e| e.to_string()),
        })
    }
}

// ---- sessions ------------------------------------------------------------------------------------

#[tauri::command]
pub fn list_sessions(state: S) -> Result<Vec<Session>> {
    state.db.sessions()
}

/// Cards analysed per day over the last `days` days (today included), for the dashboard.
#[tauri::command]
pub fn dashboard_activity(state: S, days: u32) -> Result<Vec<DayCount>> {
    use chrono::{Duration, Local, SecondsFormat, TimeZone, Utc};
    let first = Local::now().date_naive() - Duration::days(i64::from(days.max(1)) - 1);
    let since = Local
        .from_local_datetime(&first.and_hms_opt(0, 0, 0).unwrap_or_default())
        .earliest()
        .map(|t| t.with_timezone(&Utc))
        .unwrap_or_else(Utc::now);
    state
        .db
        .analysed_per_day(&since.to_rfc3339_opts(SecondsFormat::Millis, true))
}

#[tauri::command]
pub fn get_session(state: S, id: String) -> Result<Session> {
    state.db.session(&id)
}

/// `remote_session`: the ID (or link) of a Cards Cloud session to continue — the new session syncs into it and
/// skips the files whose cards it has.
#[tauri::command]
pub async fn create_session(
    state: S<'_>,
    session: NewSession,
    remote_session: Option<String>,
) -> Result<Session> {
    state.create_session(session, remote_session).await
}

impl AppState {
    pub async fn create_session(
        &self,
        mut session: NewSession,
        remote_session: Option<String>,
    ) -> Result<Session> {
        if session.name.trim().is_empty() {
            return Err(Error::msg("the session needs a name"));
        }
        if session.source_ref.trim().is_empty() {
            return Err(Error::msg(match session.source_kind {
                SourceKind::Scanner => "pick a scanner",
                _ => "pick a folder",
            }));
        }
        valid_options(&session.options)?;
        let remote = match remote_session.as_deref().map(str::trim) {
            None | Some("") => None,
            Some(text) => {
                let id = remote_session_id(text).ok_or_else(|| {
                    Error::msg(
                        "the Cards Cloud session ID looks like 1b4e28ba-2fa1-11d2-883f-0016d3cca427",
                    )
                })?;
                match self.cards_cloud()?.get(&id).await {
                    Ok(_) => Some(id),
                    Err(e) if e.status() == Some(404) => {
                        return Err(Error::msg(
                            "no Cards Cloud session has this ID (check the workspace in Settings)",
                        ))
                    }
                    Err(e) => return Err(e.into()),
                }
            }
        };
        if remote.is_some() {
            session.options.sync_remote = true;
        }
        let created = self.db.create_session(&session)?;
        if remote.is_some() {
            self.db.set_remote(&created.id, remote.as_deref(), None)?;
        }
        // Sessions an agent creates show up in the open app too.
        self.runner.emit_session(&created.id);
        self.db.session(&created.id)
    }
}

/// The session ID in what was pasted: the ID itself or a link to the session.
fn remote_session_id(text: &str) -> Option<String> {
    text.split(|c: char| !(c.is_ascii_hexdigit() || c == '-'))
        .filter_map(|part| uuid::Uuid::try_parse(part).ok())
        .next_back()
        .map(|id| id.hyphenated().to_string())
}

fn valid_options(options: &SessionOptions) -> Result<()> {
    let custom = options.custom_endpoint.trim();
    if !custom.is_empty() && crate::ximilar::recognition::custom_path(custom).is_none() {
        return Err(Error::msg(
            "the custom endpoint is a path such as collectibles/v2/your_endpoint",
        ));
    }
    use crate::ximilar::recognition::{SPORTS, TCG_GAMES};
    let sub = options.subcategory.as_str();
    if !sub.is_empty() && !TCG_GAMES.contains(&sub) && !SPORTS.contains(&sub) {
        return Err(Error::msg(format!("unknown game or sport: {sub}")));
    }
    let alphabets = crate::ximilar::recognition::ALPHABETS;
    if !options.alphabet.is_empty() && !alphabets.contains(&options.alphabet.as_str()) {
        return Err(Error::msg(format!(
            "the alphabet is one of {}",
            alphabets.join(", ")
        )));
    }
    Ok(())
}

fn valid_name(name: &str) -> Result<&str> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::msg("the session needs a name"));
    }
    if name.chars().count() > MAX_SESSION_NAME {
        return Err(Error::msg(format!(
            "the name is too long (at most {MAX_SESSION_NAME} characters)"
        )));
    }
    Ok(name)
}

/// Changes a session's settings; they apply to the cards analysed from then on.
#[tauri::command]
pub fn update_session(
    state: S,
    id: String,
    name: String,
    id_mode: IdMode,
    options: SessionOptions,
) -> Result<Session> {
    if state.runner.is_running(&id) {
        return Err(Error::msg("stop the session before changing its settings"));
    }
    let before = state.db.session(&id)?;
    let name = valid_name(&name)?;
    valid_options(&options)?;
    state.db.update_session_meta(&id, name, id_mode, &options)?;
    if before.name != name {
        state.runner.push_remote_name(&id);
    }
    state.runner.emit_session(&id);
    state.db.session(&id)
}

/// Renames a session (also while it runs) and its Cards Cloud copy.
#[tauri::command]
pub fn rename_session(state: S, id: String, name: String) -> Result<Session> {
    let name = valid_name(&name)?;
    state.db.rename_session(&id, name)?;
    state.runner.push_remote_name(&id);
    state.runner.emit_session(&id);
    state.db.session(&id)
}

/// Deletes a session with the files cardloader made for it (never the user's originals), and optionally
/// its Cards Cloud copy — first, so a failure there keeps the local session.
#[tauri::command]
pub async fn delete_session(state: S<'_>, id: String, delete_remote: Option<bool>) -> Result<()> {
    state
        .delete_session(&id, delete_remote.unwrap_or(false))
        .await
}

impl AppState {
    pub async fn delete_session(&self, id: &str, delete_remote: bool) -> Result<()> {
        let session = self.db.session(id)?;
        self.runner.stop(id)?;
        self.runner.wait_idle(id).await;
        if let (true, Some(remote)) = (delete_remote, &session.remote_session_id) {
            match self.cards_cloud()?.delete(remote).await {
                Err(e) if e.status() != Some(404) => {
                    return Err(Error::msg(format!(
                        "could not delete the Cards Cloud copy, nothing was deleted: {e}"
                    )))
                }
                _ => {
                    self.db.unlink_remote(remote)?;
                }
            }
        }
        self.db.delete_session(id)?;
        let dir = self.runner.session_dir(id);
        if dir.exists() {
            std::fs::remove_dir_all(dir)?;
        }
        self.runner.emit_session_deleted(id);
        Ok(())
    }
}

/// Where a session's images are: the folder, or for scanner sessions the scans cardloader saved.
#[tauri::command]
pub fn session_folder(state: S, id: String) -> Result<String> {
    let session = state.db.session(&id)?;
    Ok(match session.source_kind {
        SourceKind::Csv => std::path::Path::new(&session.source_ref)
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default(),
        SourceKind::Scanner => state
            .runner
            .session_dir(&id)
            .join("scans")
            .to_string_lossy()
            .into_owned(),
        _ => session.source_ref,
    })
}

/// What importing this CSV would bring in.
#[tauri::command]
pub async fn preview_csv(path: String) -> Result<CsvPreview> {
    tokio::task::spawn_blocking(move || csv_file::preview(std::path::Path::new(&path)))
        .await
        .map_err(|e| Error::msg(e.to_string()))?
}

/// A new session from a CSV: identified rows become done cards right away; rows with only an image (and,
/// with `reanalyze`, identified rows with an image) are analysed when the session runs.
#[tauri::command]
pub async fn import_csv(state: S<'_>, session: NewSession, reanalyze: bool) -> Result<Session> {
    state.import_csv(session, reanalyze).await
}

impl AppState {
    pub async fn import_csv(&self, session: NewSession, reanalyze: bool) -> Result<Session> {
        valid_name(&session.name)?;
        if session.source_kind != SourceKind::Csv {
            return Err(Error::msg("not a CSV session"));
        }
        let path = std::path::PathBuf::from(&session.source_ref);
        let file = tokio::task::spawn_blocking(move || csv_file::read(&path))
            .await
            .map_err(|e| Error::msg(e.to_string()))??;
        if file.rows.is_empty() {
            return Err(Error::msg("the CSV has no cards or images"));
        }
        let created = self.db.create_session(&session)?;
        if let Err(e) = self
            .runner
            .import_rows(&created.id, file.rows, reanalyze)
            .await
        {
            let _ = self.db.delete_session(&created.id);
            return Err(e);
        }
        let imported = self.db.session(&created.id)?;
        // Nothing left to analyse: the session is complete as imported.
        if imported.counts.total > 0 && !reanalyze && !has_images_to_analyse(&session.source_ref)? {
            self.db
                .set_session_status(&imported.id, SessionStatus::Completed, None)?;
        }
        self.runner.emit_session(&imported.id);
        self.db.session(&imported.id)
    }
}

fn has_images_to_analyse(path: &str) -> Result<bool> {
    Ok(csv_file::read(std::path::Path::new(path))?
        .rows
        .iter()
        .any(|r| !r.has_card() && r.front.is_some()))
}

#[tauri::command]
pub fn export_csv(state: S, id: String, path: String) -> Result<usize> {
    let items = state.db.items(&id)?;
    export::write_csv(&items, std::path::Path::new(&path))?;
    Ok(items.len())
}

/// Runs (or resumes) the session; with `source: false` only its queued cards are analysed — no new scan,
/// no new files — which is how cards are analysed again.
#[tauri::command]
pub fn run_session(state: S, id: String, source: Option<bool>) -> Result<()> {
    match source {
        Some(false) => state.runner.analyse_queued(&id),
        _ => state.runner.start(&id),
    }
}

#[tauri::command]
pub fn pause_session(state: S, id: String) -> Result<()> {
    state.runner.pause(&id)
}

#[tauri::command]
pub fn stop_session(state: S, id: String) -> Result<()> {
    state.runner.stop(&id)
}

/// Re-queues failed items (or the given ones) so the next run analyses them again.
#[tauri::command]
pub fn retry_items(state: S, id: String, item_ids: Option<Vec<i64>>) -> Result<usize> {
    state.retry_items(&id, item_ids.as_deref())
}

impl AppState {
    pub fn retry_items(&self, id: &str, item_ids: Option<&[i64]>) -> Result<usize> {
        let n = self.db.requeue(id, item_ids)?;
        self.runner.emit_session(id);
        Ok(n)
    }
}

#[tauri::command]
pub fn sync_session(state: S, id: String) -> Result<()> {
    state.runner.sync_now(&id)
}

#[tauri::command]
pub fn list_items(state: S, session_id: String) -> Result<Vec<ItemSummary>> {
    state.db.items(&session_id)
}

#[tauri::command]
pub fn get_item(state: S, id: i64) -> Result<ItemDetail> {
    state.db.item(id)
}

/// The user picked candidate `index` as the card's match.
#[tauri::command]
pub fn select_match(state: S, id: i64, index: usize) -> Result<ItemDetail> {
    state.runner.pick_match(id, index, &Reviewer::User)
}

/// Identified cards of every session for the Cards page (see `Db::search_cards`).
#[tauri::command]
pub fn search_cards(
    state: S,
    query: String,
    filter: String,
    limit: Option<usize>,
) -> Result<CardSearch> {
    state
        .db
        .search_cards(&query, &filter, limit.unwrap_or(1000).min(5000))
}

/// Saves what the user typed for a card: its fields and price (once identified), notes and location.
#[tauri::command]
pub fn update_item(state: S, id: i64, edit: ItemEdit) -> Result<ItemDetail> {
    state.runner.edit_item(id, edit, &Reviewer::User)
}

/// Turns a card's image upright when Ximilar found it upside down (cardloader's copies, never the user's files).
#[tauri::command]
pub fn turn_item(state: S, id: i64) -> Result<ItemDetail> {
    state.runner.turn_item(id)
}

/// Accepts the cards' matches, or with `reviewed: false` takes that back.
#[tauri::command]
pub fn review_items(
    state: S,
    session_id: String,
    item_ids: Vec<i64>,
    reviewed: bool,
) -> Result<usize> {
    state
        .runner
        .review_items(&session_id, &item_ids, reviewed, &Reviewer::User)
}

/// Sets where the cards are kept (empty clears it).
#[tauri::command]
pub fn locate_items(
    state: S,
    session_id: String,
    item_ids: Vec<i64>,
    location: String,
) -> Result<usize> {
    state.runner.locate_items(&session_id, &item_ids, &location)
}

/// Deletes cards from a stopped session; running the folder again doesn't bring them back.
#[tauri::command]
pub fn delete_items(state: S, session_id: String, item_ids: Vec<i64>) -> Result<usize> {
    state.runner.delete_items(&session_id, &item_ids)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    /// Where sessions, thumbnails and the database are kept.
    pub data_dir: String,
    pub data_bytes: u64,
}

/// The app's version and its data folder with the space it takes.
#[tauri::command]
pub async fn app_info(app: AppHandle, state: S<'_>) -> Result<AppInfo> {
    let dir = state.runner.data_dir.clone();
    let walk = dir.clone();
    let data_bytes = tokio::task::spawn_blocking(move || folder_size(&walk))
        .await
        .unwrap_or(0);
    Ok(AppInfo {
        version: app.package_info().version.to_string(),
        data_dir: dir.to_string_lossy().into_owned(),
        data_bytes,
    })
}

fn folder_size(dir: &std::path::Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .map(|e| match e.metadata() {
            Ok(m) if m.is_dir() => folder_size(&e.path()),
            Ok(m) => m.len(),
            Err(_) => 0,
        })
        .sum()
}

/// How many images a folder holds — shown when picking it.
#[tauri::command]
pub fn count_images(path: String) -> Result<usize> {
    Ok(source::list_images(std::path::Path::new(&path))?.len())
}

// ---- local models -------------------------------------------------------------------------------

#[tauri::command]
pub fn list_models(state: S) -> Vec<ModelStatus> {
    state.runner.local.list()
}

/// Starts (or resumes) a download; progress arrives as `model-updated` events.
#[tauri::command]
pub fn download_model(state: S, id: String) -> Result<ModelStatus> {
    if !local::supported() {
        return Err(Error::msg(
            "this build of cardloader can't run local models",
        ));
    }
    state.runner.local.download(&id)
}

#[tauri::command]
pub fn cancel_model_download(state: S, id: String) {
    state.runner.local.cancel(&id);
}

#[tauri::command]
pub async fn delete_model(state: S<'_>, id: String) -> Result<ModelStatus> {
    state.runner.local.delete(&id).await
}

/// Runs the local model on the cards Ximilar identified in a session and reports how often they agree.
/// Each finished card arrives as a `compare-progress` event; the session itself is not changed.
#[tauri::command]
pub async fn compare_local(
    app: AppHandle,
    state: S<'_>,
    session_id: String,
    run_id: String,
    model: Option<String>,
) -> Result<CompareReport> {
    use tauri::Emitter;
    let model = model.unwrap_or_else(|| state.settings.get().local_model);
    if !state.runner.local.is_ready(&model) {
        return Err(Error::msg("download this model in Settings first"));
    }
    let cancel = CancellationToken::new();
    state
        .runner
        .local
        .comparing
        .lock()
        .unwrap()
        .insert(run_id.clone(), cancel.clone());
    let items: Vec<ItemSummary> = state
        .db
        .items(&session_id)?
        .into_iter()
        .filter(|i| {
            i.status == ItemStatus::Done
                && !i.card.name.is_empty()
                && i.confidence.as_deref() != Some(local::identify::LOCAL_CONFIDENCE)
        })
        .collect();
    let mut report = CompareReport {
        model: model.clone(),
        ..Default::default()
    };
    let total = items.len();
    for item in items {
        if cancel.is_cancelled() {
            report.cancelled = true;
            break;
        }
        let detail = state.db.item(item.id)?;
        let image = detail
            .preview_path
            .clone()
            .unwrap_or_else(|| item.front_path.clone());
        let row = match state
            .runner
            .local
            .identify(&model, std::path::Path::new(&image))
            .await
        {
            Ok((rec, raw)) => CompareRow {
                item_id: item.id,
                seq: item.seq,
                ximilar: item.card.full_name.clone(),
                local: rec.card.full_name.clone(),
                name_matches: compare::names_match(&item.card.name, &rec.card.name),
                number_matches: compare::numbers_match(
                    &item.card.card_number,
                    &rec.card.card_number,
                ),
                seconds: raw["seconds"].as_f64().unwrap_or_default(),
                error: None,
            },
            Err(e) => CompareRow {
                item_id: item.id,
                seq: item.seq,
                ximilar: item.card.full_name.clone(),
                local: String::new(),
                name_matches: false,
                number_matches: false,
                seconds: 0.0,
                error: Some(e.to_string()),
            },
        };
        report.add(row.clone(), &item.card.card_number);
        let _ = app.emit(
            "compare-progress",
            serde_json::json!({"runId": run_id, "done": report.cards, "total": total, "row": row}),
        );
    }
    state.runner.local.comparing.lock().unwrap().remove(&run_id);
    Ok(report)
}

#[tauri::command]
pub fn cancel_compare(state: S, run_id: String) {
    if let Some(cancel) = state.runner.local.comparing.lock().unwrap().get(&run_id) {
        cancel.cancel();
    }
}

// ---- scanners ------------------------------------------------------------------------------------

#[tauri::command]
pub fn scanner_support(app: AppHandle) -> scanner::ScannerSupport {
    scanner::support(&app)
}

#[tauri::command]
pub async fn list_scanners(app: AppHandle) -> Result<Vec<scanner::DeviceInfo>> {
    scanner::list_devices(&app).await
}

/// Opens the scanner driver's own settings dialog (TWAIN); every later scan uses what is set there.
#[tauri::command]
pub async fn scanner_driver_settings(app: AppHandle, device: String) -> Result<bool> {
    scanner::configure(&app, &device).await
}

// ---- eBay ----------------------------------------------------------------------------------------

/// Stores the Cert ID (client secret) of the user's eBay keyset; empty clears it.
#[tauri::command]
pub fn set_ebay_cert(state: S, cert_id: String) -> Result<SettingsView> {
    state.settings.set_secret(Secret::EbayCert, &cert_id)?;
    Ok(state.settings.view())
}

/// Signs the seller in on eBay's consent page (in a window of its own) and keeps the login.
#[tauri::command]
pub async fn ebay_connect(app: AppHandle, state: S<'_>) -> Result<SettingsView> {
    let cfg = state.settings.get().ebay;
    if let Some(missing) = ebay::keyset_missing(&cfg, &state.settings) {
        return Err(Error::msg(missing));
    }
    let nonce = uuid::Uuid::new_v4().simple().to_string();
    let url = ebay::authorize_url(&cfg, &nonce)?;
    let code = ebay::consent_in_window(&app, url, nonce).await?;
    finish_connect(&state, &code).await
}

/// Connects with what the user pasted from eBay's "authorization successful" page (the address or the code),
/// when the consent window can't finish on its own.
#[tauri::command]
pub async fn ebay_connect_with(state: S<'_>, text: String) -> Result<SettingsView> {
    let cfg = state.settings.get().ebay;
    if let Some(missing) = ebay::keyset_missing(&cfg, &state.settings) {
        return Err(Error::msg(missing));
    }
    let code = ebay::code_from_text(&text)
        .ok_or_else(|| Error::msg("paste the address eBay showed after you agreed"))?;
    finish_connect(&state, &code).await
}

async fn finish_connect(state: &S<'_>, code: &str) -> Result<SettingsView> {
    let cfg = state.settings.get().ebay;
    let cert = state
        .settings
        .secret(Secret::EbayCert)
        .ok_or_else(|| Error::msg("enter the Cert ID first"))?;
    let tokens = ebay::exchange_code(&cfg, &cert, code)
        .await
        .map_err(|e| Error::msg(format!("eBay didn't accept the sign-in: {e}")))?;
    let refresh = tokens
        .refresh
        .clone()
        .ok_or_else(|| Error::msg("eBay sent no refresh token"))?;
    state.settings.set_secret(Secret::EbayRefresh, &refresh)?;
    state.runner.ebay.remember(&refresh, &tokens).await;
    let client = state.runner.ebay.client(&state.settings).await?;
    let username = ebay::username(&client).await;
    state.settings.update_ebay(|e| e.username = username)?;
    Ok(state.settings.view())
}

#[tauri::command]
pub async fn ebay_disconnect(state: S<'_>) -> Result<SettingsView> {
    state.settings.set_secret(Secret::EbayRefresh, "")?;
    state.runner.ebay.forget().await;
    state.settings.update_ebay(|e| e.username = None)?;
    Ok(state.settings.view())
}

/// The connected seller's business policies and locations, to choose from.
#[tauri::command]
pub async fn ebay_setup(state: S<'_>) -> Result<EbaySetup> {
    let client = state.runner.ebay.client(&state.settings).await?;
    let username = state.settings.get().ebay.username;
    Ok(ebay::setup(&client, username).await?)
}

/// Turns on business policies for the seller (listings need them).
#[tauri::command]
pub async fn ebay_opt_in(state: S<'_>) -> Result<EbaySetup> {
    let client = state.runner.ebay.client(&state.settings).await?;
    ebay::opt_in(&client).await?;
    let username = state.settings.get().ebay.username;
    Ok(ebay::setup(&client, username).await?)
}

/// Adds the place cards ship from and uses it for listings.
#[tauri::command]
pub async fn ebay_create_location(
    state: S<'_>,
    postal_code: String,
    country: String,
) -> Result<EbaySetup> {
    let postal_code = postal_code.trim();
    let country = country.trim().to_ascii_uppercase();
    if postal_code.is_empty()
        || country.len() != 2
        || !country.chars().all(|c| c.is_ascii_alphabetic())
    {
        return Err(Error::msg(
            "enter a postal code and a two-letter country code such as US",
        ));
    }
    let client = state.runner.ebay.client(&state.settings).await?;
    let key = ebay::create_location(&client, postal_code, &country).await?;
    state.settings.update_ebay(|e| e.location_key = key)?;
    let username = state.settings.get().ebay.username;
    Ok(ebay::setup(&client, username).await?)
}

/// How each identified card of a session would be listed.
#[tauri::command]
pub fn ebay_preview(state: S, session_id: String) -> Result<Vec<ebay_listing::Preview>> {
    state.ebay_preview(&session_id)
}

/// Creates (or updates) unpublished offers for the cards at the given prices.
#[tauri::command]
pub fn ebay_create_drafts(
    state: S,
    session_id: String,
    items: Vec<DraftItem>,
    condition: CardCondition,
) -> Result<()> {
    state.ebay_create_drafts(&session_id, items, condition)
}

/// Publishes the session's drafts (or the given cards'): they go live on eBay.
#[tauri::command]
pub fn ebay_publish(state: S, session_id: String, item_ids: Option<Vec<i64>>) -> Result<()> {
    state.ebay_ready()?;
    state
        .runner
        .ebay_start(&session_id, Job::Publish(item_ids))
        .map(|_| ())
}

/// Reads the session's listings back from eBay (at most every few minutes unless `force`).
#[tauri::command]
pub fn ebay_refresh(state: S, session_id: String, force: bool) -> Result<()> {
    state.ebay_refresh(&session_id, force)
}

impl AppState {
    pub fn ebay_preview(&self, session_id: &str) -> Result<Vec<ebay_listing::Preview>> {
        Ok(self
            .db
            .items(session_id)?
            .iter()
            .filter(|i| i.status == ItemStatus::Done)
            .map(ebay_listing::preview)
            .collect())
    }

    pub fn ebay_ready(&self) -> Result<()> {
        let view = self.settings.view();
        if !view.ebay_connected {
            return Err(Error::msg("connect your eBay account in Settings first"));
        }
        match ebay_listing::setup_missing(&view.settings.ebay) {
            Some(missing) => Err(Error::msg(missing)),
            None => Ok(()),
        }
    }

    pub fn ebay_create_drafts(
        &self,
        session_id: &str,
        items: Vec<DraftItem>,
        condition: CardCondition,
    ) -> Result<()> {
        self.ebay_ready()?;
        if items.is_empty() {
            return Err(Error::msg("choose the cards to list"));
        }
        if let Some(bad) = items
            .iter()
            .find(|i| !(i.price.is_finite() && i.price > 0.0))
        {
            let seq = self
                .db
                .item_summary(bad.item_id)
                .map(|i| i.seq)
                .unwrap_or(0);
            return Err(Error::msg(format!("card #{seq} needs a price")));
        }
        self.runner
            .ebay_start(session_id, Job::Drafts { items, condition })
            .map(|_| ())
    }

    pub fn ebay_refresh(&self, session_id: &str, force: bool) -> Result<()> {
        if !self.settings.view().ebay_connected {
            return Err(Error::msg("connect your eBay account in Settings first"));
        }
        self.runner
            .ebay_start(session_id, Job::Refresh { force })
            .map(|_| ())
    }
}

/// Takes the cards off eBay: drafts are deleted, live listings end.
#[tauri::command]
pub fn ebay_remove(state: S, session_id: String, item_ids: Vec<i64>) -> Result<()> {
    state
        .runner
        .ebay_start(&session_id, Job::Remove(item_ids))?;
    Ok(())
}

#[tauri::command]
pub fn ebay_cancel(state: S, session_id: String) {
    state.runner.ebay_cancel(&session_id);
}

/// The session's running eBay job, if any.
#[tauri::command]
pub fn ebay_job(state: S, session_id: String) -> Option<EbayProgress> {
    state.runner.ebay_job(&session_id)
}

#[tauri::command]
pub fn ebay_listing_url(state: S, listing_id: String) -> String {
    ebay::listing_url(&state.settings.get().ebay, &listing_id)
}

// ---- other marketplaces (files) -----------------------------------------------------------------

/// Stores the ImgBB API key that uploads pictures for the Whatnot export; empty clears it.
#[tauri::command]
pub fn set_imgbb_key(state: S, key: String) -> Result<SettingsView> {
    state.settings.set_secret(Secret::ImgbbKey, &key)?;
    Ok(state.settings.view())
}

/// How each identified card of a session would be exported for a marketplace.
#[tauri::command]
pub fn export_preview(
    state: S,
    session_id: String,
    format: Format,
) -> Result<Vec<ebay_listing::Preview>> {
    Ok(state
        .db
        .items(&session_id)?
        .iter()
        .filter(|i| i.status == ItemStatus::Done)
        .map(|i| match format {
            Format::Tcgplayer => tcgplayer::preview(i),
            Format::Whatnot => whatnot::preview(i),
            Format::Shopify => shopify::preview(i),
        })
        .collect())
}

/// Writes the TCGplayer list of the chosen ungraded cards.
#[tauri::command]
pub fn export_tcgplayer(
    state: S,
    session_id: String,
    items: Vec<PricedItem>,
    condition: CardCondition,
    path: String,
) -> Result<tcgplayer::Export> {
    state.export_tcgplayer(&session_id, &items, condition, &path)
}

/// `export-progress` events for the UI's progress bar.
fn export_progress(app: &AppHandle, session_id: &str) -> impl Fn(usize, usize) {
    use tauri::Emitter;
    let app = app.clone();
    let session_id = session_id.to_string();
    move |done, total| {
        let _ = app.emit(
            "export-progress",
            serde_json::json!({"sessionId": session_id, "done": done, "total": total}),
        );
    }
}

/// Writes Whatnot's bulk listing CSV, uploading the pictures first; `export-progress` events tell how far.
#[tauri::command]
pub async fn export_whatnot(
    app: AppHandle,
    state: S<'_>,
    session_id: String,
    items: Vec<PricedItem>,
    options: whatnot::Options,
    path: String,
) -> Result<whatnot::Export> {
    let progress = export_progress(&app, &session_id);
    state
        .export_whatnot(&session_id, &items, &options, &path, progress)
        .await
}

/// Writes Shopify's product CSV (uploading the pictures first) or its inventory CSV; `export-progress` events tell
/// how far.
#[tauri::command]
pub async fn export_shopify(
    app: AppHandle,
    state: S<'_>,
    session_id: String,
    items: Vec<PricedItem>,
    options: shopify::Options,
    path: String,
) -> Result<shopify::Export> {
    let progress = export_progress(&app, &session_id);
    state
        .export_shopify(&session_id, &items, &options, &path, progress)
        .await
}

impl AppState {
    /// The session's cards among `items`, in session order.
    fn session_cards(&self, session_id: &str, items: &[PricedItem]) -> Result<Vec<PricedItem>> {
        if items.is_empty() {
            return Err(Error::msg("choose the cards to export"));
        }
        Ok(self
            .db
            .items(session_id)?
            .iter()
            .filter_map(|i| items.iter().find(|p| p.item_id == i.id).copied())
            .collect())
    }

    pub fn export_tcgplayer(
        &self,
        session_id: &str,
        items: &[PricedItem],
        condition: CardCondition,
        path: &str,
    ) -> Result<tcgplayer::Export> {
        let cards = self
            .session_cards(session_id, items)?
            .into_iter()
            .map(|p| Ok((self.db.item(p.item_id)?, p)))
            .collect::<Result<Vec<_>>>()?;
        let (text, export) = tcgplayer::csv(&cards, condition);
        market::write(std::path::Path::new(path), &text, true)?;
        Ok(export)
    }

    pub async fn export_whatnot(
        &self,
        session_id: &str,
        items: &[PricedItem],
        options: &whatnot::Options,
        path: &str,
        progress: impl Fn(usize, usize),
    ) -> Result<whatnot::Export> {
        let cards = self.session_cards(session_id, items)?;
        let cancel = self.export_started(session_id);
        let result = whatnot::export(&self.runner, &cards, options, &cancel, progress).await;
        self.runner.exports.lock().unwrap().remove(session_id);
        let (text, export) = result?;
        // No byte-order mark: Whatnot would read it as part of the first header.
        market::write(std::path::Path::new(path), &text, false)?;
        Ok(export)
    }

    pub async fn export_shopify(
        &self,
        session_id: &str,
        items: &[PricedItem],
        options: &shopify::Options,
        path: &str,
        progress: impl Fn(usize, usize),
    ) -> Result<shopify::Export> {
        let cards = self.session_cards(session_id, items)?;
        let cancel = self.export_started(session_id);
        let result = shopify::export(&self.runner, &cards, options, &cancel, progress).await;
        self.runner.exports.lock().unwrap().remove(session_id);
        let (text, export) = result?;
        // UTF-8 without a byte-order mark, as Shopify's importer expects.
        market::write(std::path::Path::new(path), &text, false)?;
        Ok(export)
    }

    /// A running export of the session, which `cancel_export` can stop.
    fn export_started(&self, session_id: &str) -> CancellationToken {
        let cancel = CancellationToken::new();
        self.runner
            .exports
            .lock()
            .unwrap()
            .insert(session_id.to_string(), cancel.clone());
        cancel
    }
}

#[tauri::command]
pub fn cancel_export(state: S, session_id: String) {
    if let Some(cancel) = state.runner.exports.lock().unwrap().get(&session_id) {
        cancel.cancel();
    }
}

// ---- remote (Cards Cloud) ----------------------------------------------------------------------

impl AppState {
    pub fn cards_cloud(&self) -> Result<CardsCloudApi> {
        Ok(CardsCloudApi::new(self.runner.client()?))
    }

    /// Deletes a session in Cards Cloud; local sessions linked to it keep their cards and forget the link.
    pub async fn delete_remote_session(&self, id: &str) -> Result<()> {
        match self.cards_cloud()?.delete(id).await {
            Err(e) if e.status() != Some(404) => return Err(e.into()),
            _ => {}
        }
        for local in self.db.unlink_remote(id)? {
            self.runner.emit_session(&local);
        }
        Ok(())
    }

    /// A Cards Cloud session with its summary.
    pub async fn remote_session(&self, id: &str) -> Result<Value> {
        let api = self.cards_cloud()?;
        let session = api.get(id).await?;
        let summary = api.summary(id).await.unwrap_or(Value::Null);
        Ok(serde_json::json!({"session": session, "summary": summary}))
    }
}

#[tauri::command]
pub async fn list_remote_sessions(
    state: S<'_>,
    page: u32,
    search: Option<String>,
) -> Result<Value> {
    Ok(state
        .cards_cloud()?
        .list_sessions(page.max(1), search.as_deref().unwrap_or_default())
        .await?)
}

#[tauri::command]
pub async fn rename_remote_session(state: S<'_>, id: String, name: String) -> Result<Value> {
    let name = valid_name(&name)?;
    Ok(state.cards_cloud()?.rename(&id, name).await?)
}

/// Deletes a session in Cards Cloud; local sessions linked to it keep their cards and forget the link.
#[tauri::command]
pub async fn delete_remote_session(state: S<'_>, id: String) -> Result<()> {
    state.delete_remote_session(&id).await
}

#[tauri::command]
pub async fn get_remote_session(state: S<'_>, id: String) -> Result<Value> {
    state.remote_session(&id).await
}

#[tauri::command]
pub async fn list_remote_items(state: S<'_>, session_id: String, page: u32) -> Result<Value> {
    Ok(state
        .cards_cloud()?
        .list_identifications(&session_id, page.max(1))
        .await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_session_ids_are_read_from_links() {
        let id = "1b4e28ba-2fa1-41d2-883f-0016d3cca427";
        assert_eq!(remote_session_id(id).as_deref(), Some(id));
        assert_eq!(
            remote_session_id(&format!(
                " https://app.example.com/cards-cloud/sessions/{id}?tab=1 "
            ))
            .as_deref(),
            Some(id)
        );
        assert_eq!(remote_session_id(&id.to_uppercase()).as_deref(), Some(id));
        assert_eq!(remote_session_id("not an id"), None);
    }
}
