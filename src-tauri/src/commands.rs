//! Everything the UI can call. Errors reach the UI as strings.

use std::sync::Arc;

use reqwest::Method;
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, State};

use crate::db::Db;
use crate::error::{Error, Result};
use crate::export;
use crate::local::compare::{self, CompareReport, CompareRow};
use crate::local::{self, ModelStatus};
use crate::model::*;
use crate::runner::Runner;
use crate::settings::{Settings, SettingsStore, SettingsView};
use crate::source::csv_file::{self, CsvPreview};
use crate::source::{self, scanner};
use crate::ximilar::cards_cloud::{CardsCloudApi, MAX_SESSION_NAME};
use tokio_util::sync::CancellationToken;

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

#[tauri::command]
pub fn save_settings(state: S, settings: Settings) -> Result<SettingsView> {
    state.settings.update(settings)?;
    Ok(state.settings.view())
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
    let client = state.runner.client()?;
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
            match cards_cloud(&state)?.get(&id).await {
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
    if remote.is_none() {
        return state.db.create_session(&session);
    }
    session.options.sync_remote = true;
    let created = state.db.create_session(&session)?;
    state.db.set_remote(&created.id, remote.as_deref(), None)?;
    state.db.session(&created.id)
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
    let session = state.db.session(&id)?;
    state.runner.stop(&id)?;
    state.runner.wait_idle(&id).await;
    if let (true, Some(remote)) = (delete_remote.unwrap_or(false), &session.remote_session_id) {
        match cards_cloud(&state)?.delete(remote).await {
            Err(e) if e.status() != Some(404) => {
                return Err(Error::msg(format!(
                    "could not delete the Cards Cloud copy, nothing was deleted: {e}"
                )))
            }
            _ => {
                state.db.unlink_remote(remote)?;
            }
        }
    }
    state.db.delete_session(&id)?;
    let dir = state.runner.session_dir(&id);
    if dir.exists() {
        std::fs::remove_dir_all(dir)?;
    }
    Ok(())
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
    let created = state.db.create_session(&session)?;
    if let Err(e) = state
        .runner
        .import_rows(&created.id, file.rows, reanalyze)
        .await
    {
        let _ = state.db.delete_session(&created.id);
        return Err(e);
    }
    let imported = state.db.session(&created.id)?;
    // Nothing left to analyse: the session is complete as imported.
    if imported.counts.total > 0 && !reanalyze && !has_images_to_analyse(&session.source_ref)? {
        state
            .db
            .set_session_status(&imported.id, SessionStatus::Completed, None)?;
    }
    state.runner.emit_session(&imported.id);
    state.db.session(&imported.id)
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

#[tauri::command]
pub fn run_session(state: S, id: String) -> Result<()> {
    state.runner.start(&id)
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
    let n = state.db.requeue(&id, item_ids.as_deref())?;
    state.runner.emit_session(&id);
    Ok(n)
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
    state.runner.pick_match(id, index)
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

// ---- remote (Cards Cloud) ----------------------------------------------------------------------

fn cards_cloud(state: &S) -> Result<CardsCloudApi> {
    Ok(CardsCloudApi::new(state.runner.client()?))
}

#[tauri::command]
pub async fn list_remote_sessions(
    state: S<'_>,
    page: u32,
    search: Option<String>,
) -> Result<Value> {
    Ok(cards_cloud(&state)?
        .list_sessions(page.max(1), search.as_deref().unwrap_or_default())
        .await?)
}

#[tauri::command]
pub async fn rename_remote_session(state: S<'_>, id: String, name: String) -> Result<Value> {
    let name = valid_name(&name)?;
    Ok(cards_cloud(&state)?.rename(&id, name).await?)
}

/// Deletes a session in Cards Cloud; local sessions linked to it keep their cards and forget the link.
#[tauri::command]
pub async fn delete_remote_session(state: S<'_>, id: String) -> Result<()> {
    match cards_cloud(&state)?.delete(&id).await {
        Err(e) if e.status() != Some(404) => return Err(e.into()),
        _ => {}
    }
    for local in state.db.unlink_remote(&id)? {
        state.runner.emit_session(&local);
    }
    Ok(())
}

#[tauri::command]
pub async fn get_remote_session(state: S<'_>, id: String) -> Result<Value> {
    let api = cards_cloud(&state)?;
    let session = api.get(&id).await?;
    let summary = api.summary(&id).await.unwrap_or(Value::Null);
    Ok(serde_json::json!({"session": session, "summary": summary}))
}

#[tauri::command]
pub async fn list_remote_items(state: S<'_>, session_id: String, page: u32) -> Result<Value> {
    Ok(cards_cloud(&state)?
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
