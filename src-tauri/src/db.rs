//! Local session store: one SQLite file in the app data dir.
//!
//! Every call takes the connection lock briefly; the runner never holds it across an await.

use std::path::Path;
use std::sync::{Arc, Mutex};

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde_json::Value;

use crate::error::{Error, Result};
use crate::model::*;
use crate::ximilar::recognition::{self, Endpoint};

const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/001_init.sql"),
    include_str!("../migrations/002_picks.sql"),
    include_str!("../migrations/003_slab.sql"),
    include_str!("../migrations/004_ebay.sql"),
    include_str!("../migrations/005_hosted_images.sql"),
    include_str!("../migrations/006_review.sql"),
    include_str!("../migrations/007_agent_review.sql"),
    include_str!("../migrations/008_crop.sql"),
    include_str!("../migrations/009_market.sql"),
    include_str!("../migrations/010_jobs.sql"),
    include_str!("../migrations/011_price_note.sql"),
    include_str!("../migrations/012_only_stages.sql"),
    include_str!("../migrations/013_price_currency.sql"),
    include_str!("../migrations/014_titles.sql"),
];

#[derive(Clone)]
pub struct Db(Arc<Mutex<Connection>>);

/// The columns of `items` an `ItemSummary` is read from, in the order every query that reads one selects them
/// (`item_columns`). A column is read by its name (`col!`), so a new one goes anywhere in the list.
const ITEM_SUMMARY: &[&str] = &[
    "id",
    "session_id",
    "seq",
    "status",
    "front_path",
    "back_path",
    "thumb_path",
    "id_type",
    "name",
    "full_name",
    "set_name",
    "set_code",
    "card_number",
    "series",
    "year",
    "subcategory",
    "confidence",
    "distance",
    "price",
    "price_currency",
    "grade",
    "condition",
    "error",
    "synced_rev",
    "rev",
    "updated_at",
    "picks",
    "picks_synced",
    "slab_company",
    "slab_grade",
    "slab_certificate",
    "ebay_status",
    "ebay_price",
    "ebay_listing_id",
    "ebay_error",
    "reviewed",
    "edited",
    "notes",
    "location",
    "agent_reviewed",
    "review_note",
    "market_range",
    "brand",
    "team",
    "serial",
    "features",
    "title",
];

/// Totals by currency, as a query's `json_group_array` gives them.
fn money(json: Option<String>) -> Vec<Money> {
    json.and_then(|j| serde_json::from_str(&j).ok())
        .unwrap_or_default()
}

/// A price's currency as stored: NULL for US dollars, as every price was before currencies were kept.
fn stored_currency(currency: &str) -> Option<String> {
    let c = currency.trim().to_ascii_uppercase();
    (!c.is_empty() && c != USD).then_some(c)
}

/// The place of `name` in `ITEM_SUMMARY`. A name that isn't there doesn't compile (`col!`).
const fn item_column(name: &str) -> usize {
    let name = name.as_bytes();
    let mut i = 0;
    while i < ITEM_SUMMARY.len() {
        let column = ITEM_SUMMARY[i].as_bytes();
        let mut same = column.len() == name.len();
        let mut b = 0;
        while same && b < name.len() {
            same = column[b] == name[b];
            b += 1;
        }
        if same {
            return i;
        }
        i += 1;
    }
    panic!("not a column of ITEM_SUMMARY")
}

/// A column of an `ItemSummary` row by its name, found when compiling.
macro_rules! col {
    ($name:literal) => {
        const { item_column($name) }
    };
}

/// The `ItemSummary` columns for a SELECT, each after `prefix` ("" or "i.").
fn item_columns(prefix: &str) -> String {
    ITEM_SUMMARY
        .iter()
        .map(|column| format!("{prefix}{column}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// An identified card nobody has accepted or corrected yet, whatever its confidence (the same rule as
/// `needsReview` in `src/lib/cardview.ts`). A crop session's cards have no type: there is nothing to review.
/// Columns of `items` as `i`.
const TO_REVIEW: &str =
    "(i.status IN ('done', 'grading') AND i.reviewed = 0 AND i.id_type IS NOT NULL)";

/// The card is another one now (a new recognition, another candidate picked, its fields typed): the eBay listings
/// found for what it was, and the agent's reasons for its price, say nothing about it. Nor does the listing title
/// written for it, which a typed field keeps: correcting a name isn't a reason to lose a title someone wrote.
const OTHER_CARD: &str = "market = NULL, market_range = NULL, price_note = NULL, title = NULL";

/// The async card-grader jobs a card can wait for at once, each a row of `item_jobs` (migration 010). A price
/// is never a job that is out: its row only keeps why the card's price stage failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobKind {
    Price,
    Grading,
    Crop,
}

impl JobKind {
    /// The kind as stored, and as a card's error names it ("price: …", "grading: …", "crop: …").
    pub fn as_str(self) -> &'static str {
        match self {
            JobKind::Price => "price",
            JobKind::Grading => "grading",
            JobKind::Crop => "crop",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        [JobKind::Price, JobKind::Grading, JobKind::Crop]
            .into_iter()
            .find(|k| k.as_str() == text)
    }
}

/// A job of the card `items.id` that is still out.
const JOB_OUT: &str =
    "EXISTS (SELECT 1 FROM item_jobs j WHERE j.item_id = items.id AND j.error IS NULL)";

/// What a card's jobs make of it: `grading` while one is out (whatever failed is said once none is), else `error`
/// with what the failed ones say (in the order of the stages: price, grading, crop), else `done`.
const SETTLED: &str = "status = CASE
        WHEN EXISTS (SELECT 1 FROM item_jobs j WHERE j.item_id = items.id AND j.error IS NULL) THEN 'grading'
        WHEN EXISTS (SELECT 1 FROM item_jobs j WHERE j.item_id = items.id AND j.error IS NOT NULL) THEN 'error'
        ELSE 'done' END,
    error = CASE
        WHEN EXISTS (SELECT 1 FROM item_jobs j WHERE j.item_id = items.id AND j.error IS NULL) THEN NULL
        ELSE (SELECT group_concat(error, '; ') FROM
            (SELECT error FROM item_jobs j WHERE j.item_id = items.id AND j.error IS NOT NULL ORDER BY kind DESC))
        END";

/// A job's result changes the status only of a card the dispatcher has let go. One in the queue, just claimed
/// (`recognizing`) or part-way through its pass (it owes a stage) is settled when the pass ends
/// (`Db::finish_pass`): until then its status is the dispatcher's.
fn settle(c: &Connection, id: i64) -> rusqlite::Result<usize> {
    c.execute(
        &format!(
            "UPDATE items SET {SETTLED}
             WHERE id = ?1 AND stage IS NULL AND status IN ('grading', 'done', 'error')"
        ),
        params![id],
    )
}

/// The job stops being out: gone when it was stored, kept with why when it failed.
fn close_job(
    c: &Connection,
    id: i64,
    kind: JobKind,
    error: Option<&str>,
) -> rusqlite::Result<usize> {
    match error {
        None => c.execute(
            "DELETE FROM item_jobs WHERE item_id = ?1 AND kind = ?2",
            params![id, kind.as_str()],
        ),
        Some(error) => c.execute(
            "INSERT INTO item_jobs (item_id, kind, error) VALUES (?1, ?2, ?3)
             ON CONFLICT (item_id, kind) DO UPDATE SET error = excluded.error",
            params![id, kind.as_str(), error],
        ),
    }
}

/// The migration (014) that added the columns `fill_details` fills.
const DETAILS_MIGRATION: usize = 14;

/// Cards identified before migration 014 get their brand, team and serial number from the answer they kept, which
/// always had them. Only a card that still is what its answer says: one whose fields were typed, or imported
/// with other values, is left alone. Nothing else about the card changes (no revision, no `updated_at`).
fn fill_details(conn: &Connection) -> rusqlite::Result<usize> {
    let mut stmt = conn.prepare(
        "SELECT id, COALESCE(name, ''), COALESCE(card_number, ''), recognition FROM items
         WHERE edited = 0 AND recognition IS NOT NULL AND (recognition LIKE '%\"company\"%'
            OR recognition LIKE '%\"team\"%' OR recognition LIKE '%\"serial_number\"%')",
    )?;
    let cards = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut filled = 0;
    for (id, name, number, raw) in cards {
        let Ok(raw) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        let card = recognition::parse(Endpoint::of_raw(&raw), &raw["response"]).card;
        if card.name.trim() != name.trim() || card.card_number.trim() != number.trim() {
            continue;
        }
        filled += conn.execute(
            "UPDATE items SET brand = ?2, team = ?3, serial = ?4 WHERE id = ?1",
            params![id, card.brand.trim(), card.team.trim(), card.serial.trim()],
        )?;
    }
    Ok(filled)
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        Self::init(conn)
    }

    #[cfg(test)]
    pub fn memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000;",
        )?;
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        for (i, sql) in MIGRATIONS.iter().enumerate().skip(version as usize) {
            conn.execute_batch(sql)?;
            conn.execute_batch(&format!("PRAGMA user_version = {}", i + 1))?;
            if i + 1 == DETAILS_MIGRATION {
                // Never an error: the columns are there, and failing here would add them again on every start.
                if let Err(e) = fill_details(&conn) {
                    tracing::warn!("older cards keep no brand, team and serial number: {e}");
                }
            }
        }
        Ok(Db(Arc::new(Mutex::new(conn))))
    }

    fn with<T>(&self, f: impl FnOnce(&Connection) -> rusqlite::Result<T>) -> Result<T> {
        let conn = self.0.lock().unwrap();
        Ok(f(&conn)?)
    }

    // ---- sessions ----------------------------------------------------------------------------

    pub fn create_session(&self, new: &NewSession) -> Result<Session> {
        let id = uuid::Uuid::new_v4().to_string();
        self.with(|c| {
            c.execute(
                "INSERT INTO sessions (id, name, source_kind, source_ref, id_mode, options, status, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'draft', ?7)",
                params![
                    id,
                    new.name.trim(),
                    new.source_kind.as_str(),
                    new.source_ref,
                    new.id_mode.as_str(),
                    serde_json::to_string(&new.options).unwrap_or_default(),
                    now()
                ],
            )
        })?;
        self.session(&id)
    }

    pub fn session(&self, id: &str) -> Result<Session> {
        self.sessions_where("WHERE s.id = ?1", "", params![id])?
            .pop()
            .ok_or_else(|| Error::msg(format!("session {id} not found")))
    }

    pub fn sessions(&self) -> Result<Vec<Session>> {
        self.sessions_where("", "ORDER BY s.created_at DESC", params![])
    }

    fn sessions_where(
        &self,
        filter: &str,
        order: &str,
        args: impl rusqlite::Params,
    ) -> Result<Vec<Session>> {
        let sql = format!(
            "SELECT s.id, s.name, s.source_kind, s.source_ref, s.id_mode, s.options, s.status, s.error,
                    s.created_at, s.started_at, s.finished_at, s.remote_session_id, s.sync_error,
                    s.ebay_error, s.ebay_checked_at,
                    COUNT(i.id),
                    COALESCE(SUM(i.status = 'done'), 0),
                    COALESCE(SUM(i.status = 'error'), 0),
                    COALESCE(SUM(i.status = 'queued'), 0),
                    COALESCE(SUM(i.status = 'recognizing'), 0),
                    COALESCE(SUM(i.status = 'grading'), 0),
                    COALESCE(SUM(i.synced_rev > 0 AND i.synced_rev = i.rev), 0),
                    COALESCE(SUM(CASE WHEN i.price_currency IS NULL THEN i.price END), 0),
                    COALESCE(SUM(i.ebay_status = 'draft'), 0),
                    COALESCE(SUM(i.ebay_status = 'listed'), 0),
                    COALESCE(SUM(i.ebay_status = 'sold'), 0),
                    COALESCE(SUM({TO_REVIEW}), 0),
                    COALESCE(SUM(i.calls), 0),
                    COALESCE(SUM(i.stage = 'price' AND i.status = 'grading'), 0),
                    COALESCE(SUM(i.stage = 'crop' AND i.status IN ('recognizing', 'grading')), 0),
                    COALESCE(SUM(i.stage IS NULL AND i.status = 'recognizing'), 0),
                    (SELECT json_group_array(json_object('currency', cur, 'total', total)) FROM (
                        SELECT COALESCE(price_currency, 'USD') AS cur, SUM(price) AS total FROM items
                        WHERE session_id = s.id AND price IS NOT NULL GROUP BY cur ORDER BY total DESC))
             FROM sessions s LEFT JOIN items i ON i.session_id = s.id
             {filter} GROUP BY s.id {order}"
        );
        self.with(|c| {
            let mut stmt = c.prepare(&sql)?;
            let rows = stmt.query_map(args, |r| {
                let options: SessionOptions =
                    serde_json::from_str(&r.get::<_, String>(5)?).unwrap_or_default();
                // A pass's first stage is owed by no name (`items.stage` is of the later ones): where the crop
                // comes first, the cards being worked on are in it.
                let crop_first = options.stages().first() == Some(&Stage::Crop);
                let first_stage: i64 = r.get(30)?;
                Ok(Session {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    source_kind: SourceKind::parse(&r.get::<_, String>(2)?),
                    source_ref: r.get(3)?,
                    id_mode: IdMode::parse(&r.get::<_, String>(4)?),
                    options,
                    status: SessionStatus::parse(&r.get::<_, String>(6)?),
                    error: r.get(7)?,
                    created_at: r.get(8)?,
                    started_at: r.get(9)?,
                    finished_at: r.get(10)?,
                    remote_session_id: r.get(11)?,
                    sync_error: r.get(12)?,
                    ebay_error: r.get(13)?,
                    ebay_checked_at: r.get(14)?,
                    counts: SessionCounts {
                        total: r.get(15)?,
                        done: r.get(16)?,
                        errors: r.get(17)?,
                        queued: r.get(18)?,
                        in_progress: r.get(19)?,
                        grading: r.get(20)?,
                        synced: r.get(21)?,
                        total_price: r.get(22)?,
                        ebay_drafts: r.get(23)?,
                        ebay_listed: r.get(24)?,
                        ebay_sold: r.get(25)?,
                        to_review: r.get(26)?,
                        calls: r.get(27)?,
                        pricing: r.get(28)?,
                        cropping: r.get::<_, i64>(29)? + if crop_first { first_stage } else { 0 },
                        values: money(r.get(31)?),
                    },
                })
            })?;
            rows.collect()
        })
    }

    pub fn update_session_meta(
        &self,
        id: &str,
        name: &str,
        id_mode: IdMode,
        options: &SessionOptions,
    ) -> Result<()> {
        self.with(|c| {
            c.execute(
                "UPDATE sessions SET name = ?2, id_mode = ?3, options = ?4 WHERE id = ?1",
                params![
                    id,
                    name.trim(),
                    id_mode.as_str(),
                    serde_json::to_string(options).unwrap_or_default()
                ],
            )
        })?;
        Ok(())
    }

    pub fn rename_session(&self, id: &str, name: &str) -> Result<()> {
        let changed = self.with(|c| {
            c.execute(
                "UPDATE sessions SET name = ?2 WHERE id = ?1",
                params![id, name.trim()],
            )
        })?;
        if changed == 0 {
            return Err(Error::msg("no such session"));
        }
        Ok(())
    }

    /// The remote session was deleted: local sessions linked to it forget it (a later sync starts a new one).
    pub fn unlink_remote(&self, remote_id: &str) -> Result<Vec<String>> {
        let ids: Vec<String> = self.with(|c| {
            let mut stmt = c.prepare("SELECT id FROM sessions WHERE remote_session_id = ?1")?;
            let rows = stmt.query_map(params![remote_id], |r| r.get(0))?;
            rows.collect()
        })?;
        for id in &ids {
            self.with(|c| {
                c.execute(
                    "UPDATE sessions SET remote_session_id = NULL, sync_error = NULL WHERE id = ?1",
                    params![id],
                )
            })?;
            self.reset_sync(id)?;
        }
        Ok(ids)
    }

    pub fn set_session_status(
        &self,
        id: &str,
        status: SessionStatus,
        error: Option<&str>,
    ) -> Result<()> {
        let ts = now();
        self.with(|c| {
            c.execute(
                "UPDATE sessions SET status = ?2, error = ?3,
                    started_at = CASE WHEN ?2 = 'running' AND started_at IS NULL THEN ?4 ELSE started_at END,
                    finished_at = CASE WHEN ?2 IN ('completed', 'failed', 'stopped') THEN ?4 ELSE finished_at END
                 WHERE id = ?1",
                params![id, status.as_str(), error, ts],
            )
        })?;
        Ok(())
    }

    pub fn set_remote(
        &self,
        id: &str,
        remote_id: Option<&str>,
        sync_error: Option<&str>,
    ) -> Result<()> {
        self.with(|c| {
            c.execute(
                "UPDATE sessions SET remote_session_id = COALESCE(?2, remote_session_id), sync_error = ?3 WHERE id = ?1",
                params![id, remote_id, sync_error],
            )
        })?;
        Ok(())
    }

    pub fn delete_session(&self, id: &str) -> Result<()> {
        self.with(|c| c.execute("DELETE FROM sessions WHERE id = ?1", params![id]))?;
        Ok(())
    }

    /// After a restart nothing is running: running sessions become paused, half-done items queued. An item
    /// waiting for grading (or its crop) without a job out (the app quit before it was submitted) has nothing to
    /// poll, so it is half-done too, as is one that still owes a stage of its pass.
    pub fn recover_interrupted(&self) -> Result<()> {
        self.with(|c| {
            c.execute_batch(&format!(
                "UPDATE sessions SET status = 'paused' WHERE status = 'running';
                 UPDATE items SET status = 'queued' WHERE status = 'recognizing'
                    OR (status = 'grading' AND NOT {JOB_OUT})
                    OR (stage IS NOT NULL AND status != 'queued');"
            ))
        })
    }

    // ---- items -------------------------------------------------------------------------------

    /// Adds a queued item at the end of the session; returns its id. Its images are prepared before,
    /// so a worker never sees an item without a preview. `key` becomes its `event_id` unless the session
    /// already has it (then it gets a random one).
    pub fn insert_item(
        &self,
        session_id: &str,
        front: &str,
        back: Option<&str>,
        images: &ItemImages,
        key: Option<&str>,
    ) -> Result<i64> {
        let ts = now();
        let (status, error) = match &images.error {
            Some(e) => ("error", Some(e.as_str())),
            None => ("queued", None),
        };
        self.with(|c| {
            let event_id = new_event_id(c, session_id, key)?;
            c.execute(
                "INSERT INTO items (session_id, seq, event_id, front_path, back_path, thumb_path, preview_path,
                    back_preview_path, status, error, created_at, updated_at)
                 VALUES (?1, (SELECT COALESCE(MAX(seq), 0) + 1 FROM items WHERE session_id = ?1),
                    ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
                params![
                    session_id,
                    event_id,
                    front,
                    back,
                    images.thumb,
                    images.preview,
                    images.back_preview,
                    status,
                    error,
                    ts
                ],
            )?;
            Ok(c.last_insert_rowid())
        })
    }

    /// A card identified elsewhere (an imported CSV row): stored as done, ready to sync, and reviewed — it is
    /// the user's own list.
    #[allow(clippy::too_many_arguments)]
    pub fn insert_imported(
        &self,
        session_id: &str,
        front: &str,
        back: Option<&str>,
        images: &ItemImages,
        id_type: &str,
        card: &CardFields,
        distance: Option<f64>,
        confidence: Option<&str>,
        price: Option<f64>,
        price_currency: Option<&str>,
        grade: Option<f64>,
        condition: Option<&str>,
        grading: Option<&Value>,
        title: &str,
    ) -> Result<i64> {
        let ts = now();
        let key = (!front.is_empty()).then(|| crate::source::file_key(std::path::Path::new(front)));
        self.with(|c| {
            let event_id = new_event_id(c, session_id, key.as_deref())?;
            c.execute(
                "INSERT INTO items (session_id, seq, event_id, front_path, back_path, thumb_path, preview_path,
                    back_preview_path, status, id_type, name, full_name, set_name, set_code, card_number, series,
                    year, subcategory, confidence, distance, price, grade, condition, grading, rev,
                    analyzed_at, created_at, updated_at, reviewed, price_currency, brand, team, serial,
                    features, title)
                 VALUES (?1, (SELECT COALESCE(MAX(seq), 0) + 1 FROM items WHERE session_id = ?1),
                    ?2, ?3, ?4, ?5, ?6, ?7, 'done', ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19,
                    ?20, ?21, ?22, 1, ?23, ?23, ?23, 1, ?24, ?25, ?26, ?27, ?28, NULLIF(?29, ''))",
                params![
                    session_id,
                    event_id,
                    front,
                    back,
                    images.thumb,
                    images.preview,
                    images.back_preview,
                    id_type,
                    card.name,
                    card.full_name,
                    card.set_name,
                    card.set_code,
                    card.card_number,
                    card.series,
                    card.year,
                    card.subcategory,
                    confidence,
                    distance,
                    price,
                    grade,
                    condition,
                    grading.map(|g| g.to_string()),
                    ts,
                    price_currency.and_then(stored_currency),
                    card.brand,
                    card.team,
                    card.serial,
                    card.features,
                    title.trim()
                ],
            )?;
            Ok(c.last_insert_rowid())
        })
    }

    /// Cards analysed per local day since `since` (an RFC 3339 UTC time, as stored), oldest first.
    pub fn analysed_per_day(&self, since: &str) -> Result<Vec<DayCount>> {
        self.with(|c| {
            let mut stmt = c.prepare(
                "SELECT date(analyzed_at, 'localtime') AS day, COUNT(*), COALESCE(SUM(calls), 0) FROM items
                 WHERE analyzed_at >= ?1 GROUP BY day ORDER BY day",
            )?;
            let rows = stmt.query_map(params![since], |r| {
                Ok(DayCount {
                    date: r.get(0)?,
                    cards: r.get(1)?,
                    calls: r.get(2)?,
                })
            })?;
            rows.collect()
        })
    }

    /// Identified cards of every session matching `query` (name, set, number, location, notes) and `filter`
    /// (`review`, `graded`, `duplicates`, else all), newest first: the first `limit` and the totals.
    pub fn search_cards(&self, query: &str, filter: &str, limit: usize) -> Result<CardSearch> {
        let condition = match filter {
            "review" => TO_REVIEW.to_string(),
            "graded" => "(i.grade IS NOT NULL OR i.slab_company IS NOT NULL)".into(),
            "agent" => "(i.reviewed = 1 AND i.agent_reviewed = 1)".into(),
            "duplicates" => "copies > 1".into(),
            _ => "1".into(),
        };
        let columns = item_columns("i.");
        // Copies are counted over all identified cards, before the search narrows them.
        let sql = format!(
            "WITH i AS (
                SELECT *, COUNT(*) OVER (PARTITION BY lower(COALESCE(name, '')), lower(COALESCE(set_name, '')),
                    lower(COALESCE(card_number, ''))) AS copies
                FROM items WHERE status IN ('done', 'grading')
                    AND (COALESCE(name, '') != '' OR COALESCE(full_name, '') != '')
             ), found AS (
                SELECT i.*, s.name AS session_name FROM i JOIN sessions s ON s.id = i.session_id
                WHERE {condition} AND (?1 = '' OR i.name LIKE ?1 OR i.full_name LIKE ?1 OR i.set_name LIKE ?1
                    OR i.card_number LIKE ?1 OR i.location LIKE ?1 OR i.notes LIKE ?1)
             )
             SELECT {columns}, i.session_name, i.copies, COUNT(*) OVER (),
                COALESCE(SUM(CASE WHEN i.price_currency IS NULL THEN i.price END) OVER (), 0),
                (SELECT json_group_array(json_object('currency', cur, 'total', total)) FROM (
                    SELECT COALESCE(price_currency, 'USD') AS cur, SUM(price) AS total FROM found
                    WHERE price IS NOT NULL GROUP BY cur ORDER BY total DESC))
             FROM found i ORDER BY i.analyzed_at DESC, i.id DESC LIMIT ?2"
        );
        let pattern = match query.trim() {
            "" => String::new(),
            q => format!("%{q}%"),
        };
        // What the query selects after the card's own columns.
        const MORE: usize = ITEM_SUMMARY.len();
        self.with(|c| {
            let mut stmt = c.prepare(&sql)?;
            let mut search = CardSearch::default();
            let rows = stmt.query_map(params![pattern, limit as i64], |r| {
                Ok((
                    CardRow {
                        item: item_summary(r)?,
                        session_name: r.get(MORE)?,
                        copies: r.get(MORE + 1)?,
                    },
                    r.get::<_, i64>(MORE + 2)?,
                    r.get::<_, f64>(MORE + 3)?,
                    r.get::<_, Option<String>>(MORE + 4)?,
                ))
            })?;
            for row in rows {
                let (card, total, value, values) = row?;
                search.total = total;
                search.value = value;
                search.values = money(values);
                search.cards.push(card);
            }
            Ok(search)
        })
    }

    /// Source paths already in the session (or whose cards were deleted from it), so a re-run of a folder
    /// only adds new files.
    pub fn known_paths(&self, session_id: &str) -> Result<std::collections::HashSet<String>> {
        self.with(|c| {
            let mut stmt =
                c.prepare("SELECT front_path, back_path FROM items WHERE session_id = ?1")?;
            let rows = stmt.query_map(params![session_id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?))
            })?;
            let mut set = std::collections::HashSet::new();
            for row in rows {
                let (front, back) = row?;
                set.insert(front);
                if let Some(back) = back {
                    set.insert(back);
                }
            }
            let mut stmt = c.prepare("SELECT path FROM removed_files WHERE session_id = ?1")?;
            for path in stmt.query_map(params![session_id], |r| r.get::<_, String>(0))? {
                set.insert(path?);
            }
            Ok(set)
        })
    }

    /// Atomically takes the next queued item of a session and marks it as being recognized.
    pub fn claim_next(&self, session_id: &str) -> Result<Option<i64>> {
        self.with(|c| {
            c.query_row(
                "UPDATE items SET status = 'recognizing', error = NULL, updated_at = ?2
                 WHERE id = (SELECT id FROM items WHERE session_id = ?1 AND status = 'queued' ORDER BY seq LIMIT 1)
                 RETURNING id",
                params![session_id, now()],
                |r| r.get(0),
            )
            .optional()
        })
    }

    /// The status the dispatcher gives a card. A failed card owes nothing: Analyse again is a pass from the start.
    pub fn set_item_status(&self, id: i64, status: ItemStatus, error: Option<&str>) -> Result<()> {
        self.with(|c| {
            c.execute(
                "UPDATE items SET status = ?2, error = ?3, updated_at = ?4,
                    stage = CASE WHEN ?2 = 'error' THEN NULL ELSE stage END,
                    only_stages = CASE WHEN ?2 = 'error' THEN NULL ELSE only_stages END
                 WHERE id = ?1",
                params![id, status.as_str(), error, now()],
            )
        })?;
        Ok(())
    }

    /// Re-queues failed items (and, with `all`, finished ones too) for a pass from the start: what their last
    /// pass still owed or had out is dropped.
    pub fn requeue(&self, session_id: &str, item_ids: Option<&[i64]>) -> Result<usize> {
        const AGAIN: &str =
            "UPDATE items SET status = 'queued', error = NULL, stage = NULL, only_stages = NULL";
        self.with(|c| {
            let n = match item_ids {
                Some(ids) => {
                    let mut n = 0;
                    for id in ids {
                        n += c.execute(
                            &format!(
                                "{AGAIN} WHERE id = ?1 AND session_id = ?2 AND status IN ('error', 'done')"
                            ),
                            params![id, session_id],
                        )?;
                    }
                    n
                }
                None => c.execute(
                    &format!("{AGAIN} WHERE session_id = ?1 AND status = 'error'"),
                    params![session_id],
                )?,
            };
            c.execute(
                "DELETE FROM item_jobs WHERE item_id IN
                    (SELECT id FROM items WHERE session_id = ?1 AND status = 'queued' AND stage IS NULL)",
                params![session_id],
            )?;
            Ok(n)
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn save_recognition(
        &self,
        id: i64,
        id_type: &str,
        card: &CardFields,
        confidence: Option<&str>,
        distance: Option<f64>,
        price: Option<f64>,
        slab: Option<&SlabLabel>,
        raw: &Value,
        calls: i64,
        next: ItemStatus,
    ) -> Result<()> {
        let ts = now();
        self.with(|c| {
            c.execute(
                &format!(
                "UPDATE items SET id_type = ?2, name = ?3, full_name = ?4, set_name = ?5, set_code = ?6,
                    card_number = ?7, series = ?8, year = ?9, subcategory = ?10, confidence = ?11,
                    distance = ?12, price = ?13, price_currency = NULL, recognition = ?14, calls = calls + ?15, status = ?16,
                    error = NULL, analyzed_at = ?17, updated_at = ?17, rev = rev + 1, slab_company = ?18,
                    slab_grade = ?19, slab_certificate = ?20, reviewed = 0, edited = 0,
                    agent_reviewed = 0, review_note = NULL, brand = ?21, team = ?22, serial = ?23,
                    features = ?24, {OTHER_CARD}
                 WHERE id = ?1"
                ),
                params![
                    id,
                    id_type,
                    card.name,
                    card.full_name,
                    card.set_name,
                    card.set_code,
                    card.card_number,
                    card.series,
                    card.year,
                    card.subcategory,
                    confidence,
                    distance,
                    price,
                    raw.to_string(),
                    calls,
                    next.as_str(),
                    ts,
                    slab.map(|s| &s.company),
                    slab.map(|s| &s.grade),
                    slab.map(|s| &s.certificate),
                    card.brand,
                    card.team,
                    card.serial,
                    card.features,
                ],
            )
        })?;
        Ok(())
    }

    /// The user (or an agent) picked another candidate: the card fields follow it. The revision stays
    /// (re-uploading a known row changes nothing in Cards Cloud); the pick reaches Cards Cloud as a PATCH instead.
    #[allow(clippy::too_many_arguments)]
    pub fn save_pick(
        &self,
        id: i64,
        card: &CardFields,
        confidence: Option<&str>,
        distance: Option<f64>,
        price: Option<f64>,
        raw: &Value,
        by: &Reviewer,
    ) -> Result<()> {
        let ts = now();
        self.with(|c| {
            c.execute(
                &format!(
                    "UPDATE items SET name = ?2, full_name = ?3, set_name = ?4, set_code = ?5, card_number = ?6,
                        series = ?7, year = ?8, subcategory = ?9, confidence = ?10, distance = ?11, price = ?12,
                        price_currency = NULL,
                        recognition = ?13, picks = picks + 1, reviewed = 1, edited = 0, updated_at = ?14,
                        agent_reviewed = ?15, review_note = ?16, brand = ?17, team = ?18, serial = ?19,
                        features = ?20, {OTHER_CARD}
                     WHERE id = ?1"
                ),
                params![
                    id,
                    card.name,
                    card.full_name,
                    card.set_name,
                    card.set_code,
                    card.card_number,
                    card.series,
                    card.year,
                    card.subcategory,
                    confidence,
                    distance,
                    price,
                    raw.to_string(),
                    ts,
                    by.is_agent(),
                    by.note(),
                    card.brand,
                    card.team,
                    card.serial,
                    card.features,
                ],
            )
        })?;
        Ok(())
    }

    /// The user typed the card's fields or price. Like a pick, it reaches Cards Cloud as a PATCH (`picks`
    /// counts it); notes and the location stay local. Only a changed card or price accepts it: notes or a location
    /// alone leave the review as it was (a card to review stays so, the user's accept stays theirs). Changed card
    /// fields drop its eBay listings (`OTHER_CARD`) unless `keep_market` (someone's own query found them, see
    /// `browse::searched_from`); a new price alone doesn't. The card's details (brand, team,
    /// serial number, features) and its listing title are saved like notes: they are what many cards get at once,
    /// which must not accept matches nobody looked at, and Cards Cloud takes none of them.
    pub fn save_edit(
        &self,
        id: i64,
        edit: &ItemEdit,
        by: &Reviewer,
        keep_market: bool,
    ) -> Result<()> {
        let card = &edit.card;
        self.with(|c| {
            c.execute(
                "WITH fields AS (SELECT COALESCE(name, '') IS NOT ?2 OR COALESCE(full_name, '') IS NOT ?3
                    OR COALESCE(set_name, '') IS NOT ?4 OR COALESCE(set_code, '') IS NOT ?5
                    OR COALESCE(card_number, '') IS NOT ?6 OR COALESCE(series, '') IS NOT ?7
                    OR COALESCE(year, '') IS NOT ?8 OR COALESCE(subcategory, '') IS NOT ?9 AS other,
                    price IS NOT ?10 AS priced FROM items WHERE id = ?1),
                 card AS (SELECT other OR priced AS changed, other AND NOT ?21 AS stale FROM fields)
                 UPDATE items SET
                    market = CASE WHEN (SELECT stale FROM card) THEN NULL ELSE market END,
                    market_range = CASE WHEN (SELECT stale FROM card) THEN NULL ELSE market_range END,
                    picks = picks + (SELECT changed FROM card), edited = edited OR (SELECT changed FROM card),
                    name = ?2, full_name = ?3, set_name = ?4, set_code = ?5, card_number = ?6, series = ?7,
                    year = ?8, subcategory = ?9, price = ?10, notes = ?11, location = ?12,
                    brand = ?16, team = ?17, serial = ?18, features = ?19, title = NULLIF(?20, ''),
                    reviewed = reviewed OR (SELECT changed FROM card), updated_at = ?13,
                    agent_reviewed = CASE WHEN (SELECT changed FROM card) THEN ?14 ELSE agent_reviewed END,
                    review_note = CASE WHEN (SELECT changed FROM card) THEN ?15 ELSE review_note END
                 WHERE id = ?1",
                params![
                    id,
                    card.name,
                    card.full_name,
                    card.set_name,
                    card.set_code,
                    card.card_number,
                    card.series,
                    card.year,
                    card.subcategory,
                    edit.price,
                    edit.notes,
                    edit.location,
                    now(),
                    by.is_agent(),
                    by.note(),
                    card.brand,
                    card.team,
                    card.serial,
                    card.features,
                    edit.title,
                    keep_market
                ],
            )
        })?;
        Ok(())
    }

    pub fn set_notes(&self, id: i64, notes: &str, location: &str) -> Result<()> {
        self.with(|c| {
            c.execute(
                "UPDATE items SET notes = ?2, location = ?3 WHERE id = ?1",
                params![id, notes, location],
            )
        })?;
        Ok(())
    }

    /// The card's images were turned upright: new thumbnail and preview (and back preview, when the back turned
    /// too), the response says it is upright, and pictures hosted for exports and eBay drafts are uploaded again.
    pub fn save_turned(
        &self,
        id: i64,
        thumb: &str,
        preview: &str,
        back_preview: Option<&str>,
        raw: &Value,
    ) -> Result<()> {
        self.with(|c| {
            c.execute(
                "UPDATE items SET thumb_path = ?2, preview_path = ?3, recognition = ?4, hosted_images = NULL,
                    ebay_images = NULL, updated_at = ?5, back_preview_path = COALESCE(?6, back_preview_path)
                 WHERE id = ?1",
                params![id, thumb, preview, raw.to_string(), now(), back_preview],
            )
        })?;
        Ok(())
    }

    /// Accepts the cards' matches (or takes that back); returns how many changed. The user accepting a card an
    /// agent accepted makes it theirs; an agent never takes over the user's accept, nor takes it back.
    pub fn set_reviewed(
        &self,
        session_id: &str,
        ids: &[i64],
        reviewed: bool,
        by: &Reviewer,
    ) -> Result<usize> {
        self.with(|c| {
            let mut n = 0;
            for id in ids {
                n += if reviewed {
                    c.execute(
                        "UPDATE items SET reviewed = 1, agent_reviewed = ?3, review_note = ?4
                         WHERE id = ?1 AND session_id = ?2 AND (reviewed = 0 OR agent_reviewed = 1)",
                        params![id, session_id, by.is_agent(), by.note()],
                    )?
                } else {
                    c.execute(
                        "UPDATE items SET reviewed = 0, agent_reviewed = 0, review_note = NULL
                         WHERE id = ?1 AND session_id = ?2 AND reviewed = 1 AND (agent_reviewed = 1 OR NOT ?3)",
                        params![id, session_id, by.is_agent()],
                    )?
                };
            }
            Ok(n)
        })
    }

    /// Sets where the cards are kept; returns how many changed.
    pub fn set_location(&self, session_id: &str, ids: &[i64], location: &str) -> Result<usize> {
        self.with(|c| {
            let mut n = 0;
            for id in ids {
                n += c.execute(
                    "UPDATE items SET location = ?3 WHERE id = ?1 AND session_id = ?2",
                    params![id, session_id, location],
                )?;
            }
            Ok(n)
        })
    }

    /// Deletes the session's cards and remembers their files, so running the folder again skips them.
    /// Returns the deleted items, whose own files (thumbnails, previews) the caller removes.
    pub fn delete_items(&self, session_id: &str, ids: &[i64]) -> Result<Vec<ItemDetail>> {
        let mut deleted = Vec::new();
        for &id in ids {
            match self.item(id) {
                Ok(item) if item.summary.session_id == session_id => deleted.push(item),
                _ => {}
            }
        }
        let mut conn = self.0.lock().unwrap();
        let tx = conn.transaction()?;
        for item in &deleted {
            let s = &item.summary;
            for path in std::iter::once(&s.front_path).chain(&s.back_path) {
                tx.execute(
                    "INSERT OR IGNORE INTO removed_files (session_id, path) VALUES (?1, ?2)",
                    params![session_id, path],
                )?;
            }
            tx.execute("DELETE FROM items WHERE id = ?1", params![s.id])?;
        }
        tx.commit()?;
        Ok(deleted)
    }

    /// Picked items whose row is in Cards Cloud but doesn't have the latest pick yet: (id, picks).
    pub fn picks_to_sync(&self, session_id: &str) -> Result<Vec<(i64, i64)>> {
        self.with(|c| {
            let mut stmt = c.prepare(
                "SELECT id, picks FROM items
                 WHERE session_id = ?1 AND picks > picks_synced AND synced_rev > 0 ORDER BY seq",
            )?;
            let rows = stmt.query_map(params![session_id], |r| Ok((r.get(0)?, r.get(1)?)))?;
            rows.collect()
        })
    }

    pub fn mark_pick_synced(&self, id: i64, picks: i64) -> Result<()> {
        self.with(|c| {
            c.execute(
                "UPDATE items SET picks_synced = MAX(picks_synced, ?2) WHERE id = ?1",
                params![id, picks],
            )
        })?;
        Ok(())
    }

    pub fn set_remote_id(&self, id: i64, remote_id: &str) -> Result<()> {
        self.with(|c| {
            c.execute(
                "UPDATE items SET remote_id = ?2 WHERE id = ?1",
                params![id, remote_id],
            )
        })?;
        Ok(())
    }

    /// The stage of its pass the card still owes (`items.stage`), if the pass was cut short.
    pub fn stage(&self, id: i64) -> Result<Option<Stage>> {
        self.with(|c| {
            c.query_row("SELECT stage FROM items WHERE id = ?1", params![id], |r| {
                r.get::<_, Option<String>>(0)
            })
        })
        .map(|stage| stage.as_deref().and_then(Stage::find))
    }

    /// The card's pass now owes `stage` and what follows it: where a pass cut short goes on.
    pub fn set_stage(&self, id: i64, stage: Stage) -> Result<()> {
        self.with(|c| {
            c.execute(
                "UPDATE items SET stage = ?2 WHERE id = ?1",
                params![id, stage.as_str()],
            )
        })?;
        Ok(())
    }

    /// The stages a card's pass is limited to (prices or grading added to it), if it is.
    pub fn only_stages(&self, id: i64) -> Result<Option<Vec<Stage>>> {
        let text: Option<String> = self.with(|c| {
            c.query_row(
                "SELECT only_stages FROM items WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
        })?;
        Ok(text.map(|t| t.split(',').filter_map(Stage::find).collect()))
    }

    /// Queues a session's identified cards (`item_ids`, else all of them) for a pass of only `stages`, which they
    /// owe from the first: they keep their identification, review and whatever else they have; what those stages
    /// last failed with is dropped. Only finished cards: done, or failed after being identified. Their ids.
    pub fn add_stages(
        &self,
        session_id: &str,
        item_ids: Option<&[i64]>,
        stages: &[Stage],
    ) -> Result<Vec<i64>> {
        let Some(first) = stages.first() else {
            return Ok(Vec::new());
        };
        let only = stages
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(",");
        let kinds: Vec<&str> = stages
            .iter()
            .filter_map(|s| match s {
                Stage::Price => Some(JobKind::Price.as_str()),
                Stage::Grade => Some(JobKind::Grading.as_str()),
                Stage::Crop => Some(JobKind::Crop.as_str()),
                Stage::Identify => None,
            })
            .collect();
        self.with(|c| {
            let mut stmt = c.prepare(
                "SELECT id FROM items WHERE session_id = ?1 AND id_type IS NOT NULL AND status IN ('done', 'error')",
            )?;
            let ids: Vec<i64> = stmt
                .query_map(params![session_id], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?;
            let ids: Vec<i64> = ids
                .into_iter()
                .filter(|id| item_ids.is_none_or(|wanted| wanted.contains(id)))
                .collect();
            for id in &ids {
                c.execute(
                    "UPDATE items SET status = 'queued', error = NULL, stage = ?2, only_stages = ?3, updated_at = ?4
                     WHERE id = ?1",
                    params![id, first.as_str(), only, now()],
                )?;
                for kind in &kinds {
                    c.execute(
                        "DELETE FROM item_jobs WHERE item_id = ?1 AND kind = ?2 AND error IS NOT NULL",
                        params![id, kind],
                    )?;
                }
            }
            Ok(ids)
        })
    }

    /// A pass from the start: nothing of an earlier one is out or failed any more.
    pub fn clear_jobs(&self, id: i64) -> Result<()> {
        self.with(|c| c.execute("DELETE FROM item_jobs WHERE item_id = ?1", params![id]))?;
        Ok(())
    }

    /// The card's pass is over: it owes nothing, and is what its jobs make of it (`SETTLED`).
    pub fn finish_pass(&self, id: i64) -> Result<()> {
        self.with(|c| {
            c.execute(
                &format!(
                    "UPDATE items SET stage = NULL, only_stages = NULL, {SETTLED}, updated_at = ?2 WHERE id = ?1"
                ),
                params![id, now()],
            )
        })?;
        Ok(())
    }

    /// A submitted async job of the card (one call): it waits in `grading` until every job it has is stored.
    pub fn set_job(&self, id: i64, kind: JobKind, job_id: &str) -> Result<()> {
        self.with(|c| {
            c.execute(
                "INSERT INTO item_jobs (item_id, kind, job_id) VALUES (?1, ?2, ?3)
                 ON CONFLICT (item_id, kind) DO UPDATE SET job_id = excluded.job_id, error = NULL",
                params![id, kind.as_str(), job_id],
            )?;
            c.execute(
                "UPDATE items SET calls = calls + 1, status = 'grading', updated_at = ?2 WHERE id = ?1",
                params![id, now()],
            )
        })?;
        Ok(())
    }

    /// The card's job failed (or couldn't be submitted): only that part of the card fails. Its other job, if one is
    /// out, is still polled and stored, and the card's error says what failed.
    pub fn fail_job(&self, id: i64, kind: JobKind, error: &str) -> Result<()> {
        self.with(|c| {
            close_job(c, id, kind, Some(error))?;
            settle(c, id)
        })?;
        Ok(())
    }

    /// A grading result, with `error` when the grader answered but couldn't grade the card. The card is done unless
    /// its crop is still to come.
    pub fn save_grading(
        &self,
        id: i64,
        grade: Option<f64>,
        condition: Option<&str>,
        raw: &Value,
        images: &[String],
        error: Option<&str>,
    ) -> Result<()> {
        self.with(|c| {
            c.execute(
                "UPDATE items SET grade = ?2, condition = ?3, grading = ?4, grading_images = ?5, updated_at = ?6,
                    rev = rev + 1, analyzed_at = COALESCE(analyzed_at, ?6)
                 WHERE id = ?1",
                params![id, grade, condition, raw.to_string(), serde_json::to_string(images).unwrap_or_default(), now()],
            )?;
            close_job(c, id, JobKind::Grading, error)?;
            settle(c, id)
        })?;
        Ok(())
    }

    /// A crop job's result: the cropped pictures saved, and the error of a side that has none. The card is done
    /// unless its grading is still to come. Nothing Cards Cloud has changes, so the revision stays.
    pub fn save_crop(
        &self,
        id: i64,
        raw: &Value,
        images: &[String],
        error: Option<&str>,
    ) -> Result<()> {
        self.with(|c| {
            c.execute(
                "UPDATE items SET crop = ?2, crop_images = ?3, analyzed_at = COALESCE(analyzed_at, ?4),
                    updated_at = ?4
                 WHERE id = ?1",
                params![id, raw.to_string(), serde_json::to_string(images).unwrap_or_default(), now()],
            )?;
            close_job(c, id, JobKind::Crop, error)?;
            settle(c, id)
        })?;
        Ok(())
    }

    /// A card's price from its price stage (eBay listings, an AI agent) in `currency`, with why when an agent set
    /// it. A result change: Cards Cloud gets the price with the card (when it is in US dollars).
    pub fn save_price(
        &self,
        id: i64,
        price: f64,
        currency: &str,
        note: Option<&str>,
    ) -> Result<()> {
        self.with(|c| {
            c.execute(
                "UPDATE items SET price = ?2, price_currency = ?3, price_note = ?4, rev = rev + 1, updated_at = ?5
                 WHERE id = ?1",
                params![id, price, stored_currency(currency), note, now()],
            )
        })?;
        Ok(())
    }

    /// A card's active eBay listings and their range (local only: nothing Cards Cloud has changes).
    pub fn save_market(&self, id: i64, market: &EbayMarket) -> Result<()> {
        self.with(|c| {
            c.execute(
                "UPDATE items SET market = ?2, market_range = ?3, updated_at = ?4 WHERE id = ?1",
                params![
                    id,
                    serde_json::to_string(market).unwrap_or_default(),
                    market
                        .range
                        .as_ref()
                        .map(|r| serde_json::to_string(r).unwrap_or_default()),
                    now()
                ],
            )
        })?;
        Ok(())
    }

    /// The async jobs a session's cards wait for, a card's grading before its crop: (item id, kind, job id).
    pub fn jobs(&self, session_id: &str) -> Result<Vec<(i64, JobKind, String)>> {
        self.with(|c| {
            let mut stmt = c.prepare(
                "SELECT j.item_id, j.kind, j.job_id FROM item_jobs j JOIN items i ON i.id = j.item_id
                 WHERE i.session_id = ?1 AND j.error IS NULL AND j.job_id IS NOT NULL
                 ORDER BY i.seq, j.kind DESC",
            )?;
            let rows = stmt.query_map(params![session_id], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
            })?;
            let mut jobs = Vec::new();
            for row in rows {
                let (id, kind, job) = row?;
                jobs.extend(JobKind::parse(&kind).map(|kind| (id, kind, job)));
            }
            Ok(jobs)
        })
    }

    pub fn items(&self, session_id: &str) -> Result<Vec<ItemSummary>> {
        self.with(|c| {
            let mut stmt = c.prepare(&format!(
                "SELECT {} FROM items WHERE session_id = ?1 ORDER BY seq",
                item_columns("")
            ))?;
            let rows = stmt.query_map(params![session_id], item_summary)?;
            rows.collect()
        })
    }

    pub fn item_summary(&self, id: i64) -> Result<ItemSummary> {
        self.with(|c| {
            c.query_row(
                &format!("SELECT {} FROM items WHERE id = ?1", item_columns("")),
                params![id],
                item_summary,
            )
        })
    }

    pub fn item(&self, id: i64) -> Result<ItemDetail> {
        let summary = self.item_summary(id)?;
        self.with(|c| {
            c.query_row(
                "SELECT event_id, preview_path, back_preview_path, recognition, grading, grading_images, calls, analyzed_at,
                    remote_id, ebay_sku, ebay_offer_id, ebay_images, hosted_images, crop, crop_images, market,
                    price_note
                 FROM items WHERE id = ?1",
                params![id],
                |r| {
                    Ok(ItemDetail {
                        summary,
                        event_id: r.get("event_id")?,
                        preview_path: r.get("preview_path")?,
                        back_preview_path: r.get("back_preview_path")?,
                        recognition: json_col(r.get("recognition")?),
                        grading: json_col(r.get("grading")?),
                        grading_images: json_col(r.get("grading_images")?)
                            .and_then(|v| serde_json::from_value(v).ok())
                            .unwrap_or_default(),
                        calls: r.get("calls")?,
                        analyzed_at: r.get("analyzed_at")?,
                        remote_id: r.get("remote_id")?,
                        ebay_sku: r.get("ebay_sku")?,
                        ebay_offer_id: r.get("ebay_offer_id")?,
                        ebay_images: json_col(r.get("ebay_images")?)
                            .and_then(|v| serde_json::from_value(v).ok())
                            .unwrap_or_default(),
                        hosted_images: json_col(r.get("hosted_images")?)
                            .and_then(|v| serde_json::from_value(v).ok())
                            .unwrap_or_default(),
                        crop: json_col(r.get("crop")?),
                        crop_images: json_col(r.get("crop_images")?)
                            .and_then(|v| serde_json::from_value(v).ok())
                            .unwrap_or_default(),
                        market: json_col(r.get("market")?).and_then(|v| serde_json::from_value(v).ok()),
                        price_note: r.get("price_note")?,
                    })
                },
            )
        })
    }

    /// Finished items whose latest revision Cards Cloud hasn't stored yet: (id, rev).
    pub fn unsynced(&self, session_id: &str, limit: usize) -> Result<Vec<(i64, i64)>> {
        self.with(|c| {
            let mut stmt = c.prepare(
                "SELECT id, rev FROM items
                 WHERE session_id = ?1 AND status IN ('done', 'grading') AND rev > synced_rev
                 ORDER BY seq LIMIT ?2",
            )?;
            let rows = stmt.query_map(params![session_id, limit as i64], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })?;
            rows.collect()
        })
    }

    pub fn mark_synced(&self, id: i64, rev: i64) -> Result<()> {
        self.with(|c| {
            c.execute(
                "UPDATE items SET synced_rev = MAX(synced_rev, ?2) WHERE id = ?1",
                params![id, rev],
            )
        })?;
        Ok(())
    }

    /// Synced items whose image hasn't been uploaded yet.
    pub fn images_to_upload(&self, session_id: &str, limit: usize) -> Result<Vec<i64>> {
        self.with(|c| {
            let mut stmt = c.prepare(
                "SELECT id FROM items WHERE session_id = ?1 AND synced_rev > 0 AND image_synced = 0
                 AND preview_path IS NOT NULL ORDER BY seq LIMIT ?2",
            )?;
            let rows = stmt.query_map(params![session_id, limit as i64], |r| r.get(0))?;
            rows.collect()
        })
    }

    pub fn mark_image_synced(&self, id: i64) -> Result<()> {
        self.with(|c| {
            c.execute(
                "UPDATE items SET image_synced = 1 WHERE id = ?1",
                params![id],
            )
        })?;
        Ok(())
    }

    // ---- eBay --------------------------------------------------------------------------------

    pub fn set_ebay_error(&self, session_id: &str, error: Option<&str>) -> Result<()> {
        self.with(|c| {
            c.execute(
                "UPDATE sessions SET ebay_error = ?2 WHERE id = ?1",
                params![session_id, error],
            )
        })?;
        Ok(())
    }

    pub fn set_ebay_checked(&self, session_id: &str) -> Result<()> {
        self.with(|c| {
            c.execute(
                "UPDATE sessions SET ebay_checked_at = ?2 WHERE id = ?1",
                params![session_id, now()],
            )
        })?;
        Ok(())
    }

    pub fn set_ebay_sku(&self, id: i64, sku: &str) -> Result<()> {
        self.with(|c| {
            c.execute(
                "UPDATE items SET ebay_sku = ?2 WHERE id = ?1",
                params![id, sku],
            )
        })?;
        Ok(())
    }

    pub fn set_ebay_images(&self, id: i64, urls: &[String]) -> Result<()> {
        self.with(|c| {
            c.execute(
                "UPDATE items SET ebay_images = ?2 WHERE id = ?1",
                params![id, serde_json::to_string(urls).unwrap_or_default()],
            )
        })?;
        Ok(())
    }

    pub fn set_hosted_images(&self, id: i64, urls: &[String]) -> Result<()> {
        self.with(|c| {
            c.execute(
                "UPDATE items SET hosted_images = ?2 WHERE id = ?1",
                params![id, serde_json::to_string(urls).unwrap_or_default()],
            )
        })?;
        Ok(())
    }

    /// The card's offer exists at `price`: a draft, unless it is already live.
    pub fn set_ebay_offer(&self, id: i64, offer_id: &str, price: f64) -> Result<()> {
        self.with(|c| {
            c.execute(
                "UPDATE items SET ebay_offer_id = ?2, ebay_price = ?3, ebay_error = NULL,
                    ebay_status = CASE WHEN ebay_status = 'listed' THEN 'listed' ELSE 'draft' END
                 WHERE id = ?1",
                params![id, offer_id, price],
            )
        })?;
        Ok(())
    }

    pub fn set_ebay_status(
        &self,
        id: i64,
        status: EbayStatus,
        listing_id: Option<&str>,
    ) -> Result<()> {
        self.with(|c| {
            c.execute(
                "UPDATE items SET ebay_status = ?2, ebay_listing_id = COALESCE(?3, ebay_listing_id),
                    ebay_error = NULL
                 WHERE id = ?1",
                params![id, status.as_str(), listing_id],
            )
        })?;
        Ok(())
    }

    pub fn set_ebay_item_error(&self, id: i64, error: Option<&str>) -> Result<()> {
        self.with(|c| {
            c.execute(
                "UPDATE items SET ebay_error = ?2 WHERE id = ?1",
                params![id, error],
            )
        })?;
        Ok(())
    }

    /// The card is no longer on eBay (its pictures there expire with it; the SKU stays the card's).
    pub fn clear_ebay(&self, id: i64) -> Result<()> {
        self.with(|c| {
            c.execute(
                "UPDATE items SET ebay_offer_id = NULL, ebay_listing_id = NULL, ebay_status = NULL,
                    ebay_price = NULL, ebay_images = NULL, ebay_error = NULL
                 WHERE id = ?1",
                params![id],
            )
        })?;
        Ok(())
    }

    /// Forget the remote state of a session's items (a new remote session will get them all again).
    pub fn reset_sync(&self, session_id: &str) -> Result<()> {
        self.with(|c| {
            c.execute(
                "UPDATE items SET synced_rev = 0, image_synced = 0, picks_synced = 0, remote_id = NULL
                 WHERE session_id = ?1",
                params![session_id],
            )
        })?;
        Ok(())
    }
}

/// `key` unless the session already has an item with it (two files of the same name), else a random id.
fn new_event_id(c: &Connection, session_id: &str, key: Option<&str>) -> rusqlite::Result<String> {
    if let Some(key) = key {
        let taken: bool = c.query_row(
            "SELECT EXISTS(SELECT 1 FROM items WHERE session_id = ?1 AND event_id = ?2)",
            params![session_id, key],
            |r| r.get(0),
        )?;
        if !taken {
            return Ok(key.to_string());
        }
    }
    Ok(uuid::Uuid::new_v4().simple().to_string())
}

fn json_col(text: Option<String>) -> Option<Value> {
    text.and_then(|t| serde_json::from_str(&t).ok())
}

fn item_summary(r: &Row) -> rusqlite::Result<ItemSummary> {
    let text = |i: usize| -> rusqlite::Result<String> {
        Ok(r.get::<_, Option<String>>(i)?.unwrap_or_default())
    };
    let synced_rev: i64 = r.get(col!("synced_rev"))?;
    let rev: i64 = r.get(col!("rev"))?;
    let picks: i64 = r.get(col!("picks"))?;
    let picks_synced: i64 = r.get(col!("picks_synced"))?;
    Ok(ItemSummary {
        id: r.get(col!("id"))?,
        session_id: r.get(col!("session_id"))?,
        seq: r.get(col!("seq"))?,
        status: ItemStatus::parse(&r.get::<_, String>(col!("status"))?),
        front_path: r.get(col!("front_path"))?,
        back_path: r.get(col!("back_path"))?,
        thumb_path: r.get(col!("thumb_path"))?,
        id_type: r.get(col!("id_type"))?,
        card: CardFields {
            name: text(col!("name"))?,
            full_name: text(col!("full_name"))?,
            set_name: text(col!("set_name"))?,
            set_code: text(col!("set_code"))?,
            card_number: text(col!("card_number"))?,
            series: text(col!("series"))?,
            year: text(col!("year"))?,
            subcategory: text(col!("subcategory"))?,
            brand: text(col!("brand"))?,
            team: text(col!("team"))?,
            serial: text(col!("serial"))?,
            features: text(col!("features"))?,
        },
        confidence: r.get(col!("confidence"))?,
        distance: r.get(col!("distance"))?,
        price: r.get(col!("price"))?,
        price_currency: r
            .get::<_, Option<String>>(col!("price_currency"))?
            .unwrap_or_else(|| USD.into()),
        grade: r.get(col!("grade"))?,
        condition: r.get(col!("condition"))?,
        slab: slab_label(r)?,
        error: r.get(col!("error"))?,
        synced: synced_rev > 0 && synced_rev == rev && picks == picks_synced,
        updated_at: r.get(col!("updated_at"))?,
        picked: picks > 0,
        edited: r.get(col!("edited"))?,
        reviewed: r.get(col!("reviewed"))?,
        agent_reviewed: r.get(col!("agent_reviewed"))?,
        review_note: text(col!("review_note"))?,
        notes: text(col!("notes"))?,
        location: text(col!("location"))?,
        title: text(col!("title"))?,
        ebay: ebay_listing(r)?,
        market_range: json_col(r.get(col!("market_range"))?)
            .and_then(|v| serde_json::from_value(v).ok()),
    })
}

/// The eBay columns: a listing when any of them is set.
fn ebay_listing(r: &Row) -> rusqlite::Result<Option<EbayListing>> {
    let status: Option<String> = r.get(col!("ebay_status"))?;
    let listing = EbayListing {
        status: status.as_deref().map(EbayStatus::parse),
        price: r.get(col!("ebay_price"))?,
        listing_id: r.get(col!("ebay_listing_id"))?,
        error: r.get(col!("ebay_error"))?,
    };
    Ok((listing != EbayListing::default()).then_some(listing))
}

/// The slab columns: a label when any of them is set.
fn slab_label(r: &Row) -> rusqlite::Result<Option<SlabLabel>> {
    let [company, grade, certificate] = [
        col!("slab_company"),
        col!("slab_grade"),
        col!("slab_certificate"),
    ]
    .map(|i| r.get::<_, Option<String>>(i));
    let (company, grade, certificate) = (company?, grade?, certificate?);
    if company.is_none() && grade.is_none() && certificate.is_none() {
        return Ok(None);
    }
    Ok(Some(SlabLabel {
        company: company.unwrap_or_default(),
        grade: grade.unwrap_or_default(),
        certificate: certificate.unwrap_or_default(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn session(db: &Db) -> Session {
        db.create_session(&NewSession {
            name: "Test".into(),
            source_kind: SourceKind::Folder,
            source_ref: "/tmp/cards".into(),
            id_mode: IdMode::Auto,
            options: SessionOptions::default(),
        })
        .unwrap()
    }

    #[test]
    fn counts_analysed_cards_per_day() {
        let db = Db::memory().unwrap();
        let s = session(&db);
        for (i, at) in [
            Some("2026-10-19T12:00:00.000Z"),
            Some("2026-10-20T12:00:00.000Z"),
            Some("2026-10-20T12:30:00.000Z"),
            None,
        ]
        .into_iter()
        .enumerate()
        {
            let id = db
                .insert_item(
                    &s.id,
                    &format!("/{i}.jpg"),
                    None,
                    &ItemImages::default(),
                    None,
                )
                .unwrap();
            db.with(|c| {
                c.execute(
                    "UPDATE items SET analyzed_at = ?2 WHERE id = ?1",
                    params![id, at],
                )
            })
            .unwrap();
        }
        let days = db.analysed_per_day("2026-10-20T00:00:00.000Z").unwrap();
        assert_eq!(
            days,
            vec![DayCount {
                date: "2026-10-20".into(),
                cards: 2,
                calls: 0
            }]
        );
        assert_eq!(
            db.analysed_per_day("2026-01-01T00:00:00.000Z")
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn rename_edit_and_unlink() {
        let db = Db::memory().unwrap();
        let s = session(&db);
        db.rename_session(&s.id, "  Binder 4 ").unwrap();
        assert_eq!(db.session(&s.id).unwrap().name, "Binder 4");
        assert!(db.rename_session("missing", "x").is_err());

        let options = SessionOptions {
            grading: GradingMode::Condition,
            ..SessionOptions::default()
        };
        db.update_session_meta(&s.id, "Binder 5", IdMode::Sport, &options)
            .unwrap();
        let edited = db.session(&s.id).unwrap();
        assert_eq!(
            (edited.name.as_str(), edited.id_mode, edited.options.grading),
            ("Binder 5", IdMode::Sport, GradingMode::Condition)
        );

        let id = db
            .insert_item(&s.id, "/a.jpg", None, &ItemImages::default(), None)
            .unwrap();
        db.set_remote(&s.id, Some("remote-1"), Some("old error"))
            .unwrap();
        db.mark_synced(id, 0).unwrap();
        db.set_remote_id(id, "row-1").unwrap();
        assert_eq!(db.unlink_remote("remote-1").unwrap(), vec![s.id.clone()]);
        let unlinked = db.session(&s.id).unwrap();
        assert_eq!(
            (unlinked.remote_session_id, unlinked.sync_error),
            (None, None)
        );
        assert_eq!(db.item(id).unwrap().remote_id, None);
        assert!(db.unlink_remote("remote-1").unwrap().is_empty());
    }

    #[test]
    fn items_are_claimed_in_order_once() {
        let db = Db::memory().unwrap();
        let s = session(&db);
        let a = db
            .insert_item(&s.id, "/a.jpg", None, &ItemImages::default(), None)
            .unwrap();
        let b = db
            .insert_item(
                &s.id,
                "/b.jpg",
                Some("/b2.jpg"),
                &ItemImages::default(),
                None,
            )
            .unwrap();
        assert_eq!(db.claim_next(&s.id).unwrap(), Some(a));
        assert_eq!(db.claim_next(&s.id).unwrap(), Some(b));
        assert_eq!(db.claim_next(&s.id).unwrap(), None);
        let counts = db.session(&s.id).unwrap().counts;
        assert_eq!((counts.total, counts.in_progress, counts.queued), (2, 2, 0));
        assert_eq!(db.known_paths(&s.id).unwrap().len(), 3);
    }

    /// Cards read before the slab columns existed get them from their stored response.
    #[test]
    fn slab_columns_are_filled_from_stored_responses() {
        let conn = Connection::open_in_memory().unwrap();
        for sql in &MIGRATIONS[..2] {
            conn.execute_batch(sql).unwrap();
        }
        conn.execute_batch("PRAGMA user_version = 2").unwrap();
        conn.execute(
            "INSERT INTO sessions (id, name, source_kind, id_mode, options, status, created_at)
             VALUES ('s', 'Old', 'folder', 'auto', '{}', 'completed', '')",
            [],
        )
        .unwrap();
        let slabbed = json!({"response": {"records": [{"_objects": [
            {"name": "Card", "_identification": {"best_match": {"name": "Charizard"}}},
            {"name": "Slab Label", "_tags": {"Company": [{"name": "PSA"}], "Grade": [{"name": "8"}]},
             "_identification": {"best_match": {"grade": 10, "certificate_number": "12345678"}}}]}]}});
        let plain = json!({"response": {"records": [{"_objects": [{"name": "Card"}]}]}});
        for (seq, recognition) in [(1, Some(slabbed)), (2, Some(plain)), (3, None)] {
            conn.execute(
                "INSERT INTO items (session_id, seq, event_id, front_path, status, recognition, created_at, updated_at)
                 VALUES ('s', ?1, ?1, '/a.jpg', 'done', ?2, '', '')",
                params![seq, recognition.map(|r| r.to_string())],
            )
            .unwrap();
        }

        let db = Db::init(conn).unwrap();
        let items = db.items("s").unwrap();
        assert_eq!(
            items[0].slab,
            Some(SlabLabel {
                company: "PSA".into(),
                grade: "10".into(),
                certificate: "12345678".into(),
            })
        );
        assert_eq!(items[1].slab, None);
        assert_eq!(items[2].slab, None);
    }

    #[test]
    fn recognition_bumps_revision_and_sync_tracks_it() {
        let db = Db::memory().unwrap();
        let s = session(&db);
        let id = db
            .insert_item(&s.id, "/a.jpg", None, &ItemImages::default(), None)
            .unwrap();
        db.claim_next(&s.id).unwrap();
        let card = CardFields {
            name: "Charizard".into(),
            ..Default::default()
        };
        db.save_recognition(
            id,
            "tcg",
            &card,
            Some("high"),
            Some(0.1),
            Some(12.5),
            None,
            &Value::Null,
            1,
            ItemStatus::Done,
        )
        .unwrap();
        assert_eq!(db.unsynced(&s.id, 10).unwrap(), vec![(id, 1)]);
        db.mark_synced(id, 1).unwrap();
        assert!(db.unsynced(&s.id, 10).unwrap().is_empty());
        let item = db.item(id).unwrap();
        assert!(item.summary.synced);
        assert_eq!(item.summary.card.name, "Charizard");
        assert_eq!(db.session(&s.id).unwrap().counts.total_price, 12.5);
    }

    #[test]
    fn identified_cards_wait_for_review() {
        let db = Db::memory().unwrap();
        let s = session(&db);
        let mut ids = Vec::new();
        for (i, (name, confidence)) in [("Pikachu", "high"), ("Mew", "low"), ("", "high")]
            .into_iter()
            .enumerate()
        {
            let id = db
                .insert_item(
                    &s.id,
                    &format!("/{i}.jpg"),
                    None,
                    &ItemImages::default(),
                    None,
                )
                .unwrap();
            db.claim_next(&s.id).unwrap();
            let card = CardFields {
                name: name.into(),
                ..Default::default()
            };
            db.save_recognition(
                id,
                "tcg",
                &card,
                Some(confidence),
                None,
                None,
                None,
                &Value::Null,
                3,
                ItemStatus::Done,
            )
            .unwrap();
            ids.push(id);
        }
        let counts = db.session(&s.id).unwrap().counts;
        assert_eq!(
            (counts.to_review, counts.calls),
            (3, 9),
            "every card until accepted"
        );
        assert_eq!(
            db.set_reviewed(&s.id, &ids, true, &Reviewer::User).unwrap(),
            3
        );
        assert_eq!(db.session(&s.id).unwrap().counts.to_review, 0);
        assert!(db.item_summary(ids[1]).unwrap().reviewed);
    }

    #[test]
    fn cards_are_searched_across_sessions() {
        let db = Db::memory().unwrap();
        let (a, b) = (session(&db), session(&db));
        let add = |s: &Session, name: &str, number: &str, price: f64, confidence: &str| {
            let n = db.items(&s.id).unwrap().len();
            let id = db
                .insert_item(
                    &s.id,
                    &format!("/{n}.jpg"),
                    None,
                    &ItemImages::default(),
                    None,
                )
                .unwrap();
            db.claim_next(&s.id).unwrap();
            let card = CardFields {
                name: name.into(),
                set_name: "Base Set".into(),
                card_number: number.into(),
                ..Default::default()
            };
            db.save_recognition(
                id,
                "tcg",
                &card,
                Some(confidence),
                None,
                Some(price),
                None,
                &Value::Null,
                1,
                ItemStatus::Done,
            )
            .unwrap();
            id
        };
        let pikachu = add(&a, "Pikachu", "58", 4.0, "high");
        add(&b, "pikachu", "58", 5.0, "low");
        let charizard = add(&b, "Charizard", "4", 300.0, "high");
        db.set_location(&a.id, &[pikachu], "Box 7").unwrap();

        let all = db.search_cards("", "", 100).unwrap();
        assert_eq!((all.total, all.value), (3, 309.0));
        let copies: Vec<i64> = all.cards.iter().map(|c| c.copies).collect();
        assert_eq!(
            copies,
            vec![1, 2, 2],
            "newest first; the two Pikachus are copies"
        );
        let found = db.search_cards("box 7", "", 100).unwrap();
        assert_eq!(found.cards[0].item.id, pikachu);
        assert_eq!(found.cards[0].session_name, "Test");
        assert_eq!(db.search_cards("", "duplicates", 100).unwrap().total, 2);
        db.set_reviewed(&b.id, &[charizard], true, &Reviewer::User)
            .unwrap();
        assert_eq!(db.search_cards("", "review", 100).unwrap().total, 2);
        let first = db.search_cards("", "", 1).unwrap();
        assert_eq!(
            (first.cards.len(), first.total),
            (1, 3),
            "totals count past the limit"
        );
    }

    #[test]
    fn ebay_listing_is_tracked() {
        let db = Db::memory().unwrap();
        let s = session(&db);
        let id = db
            .insert_item(&s.id, "/a.jpg", None, &ItemImages::default(), None)
            .unwrap();
        assert_eq!(db.item_summary(id).unwrap().ebay, None);
        db.set_ebay_item_error(id, Some("no price")).unwrap();
        assert_eq!(db.item_summary(id).unwrap().ebay.unwrap().status, None);

        db.set_ebay_sku(id, "cl-1").unwrap();
        db.set_ebay_images(id, &["https://i.example.com/1.jpg".into()])
            .unwrap();
        db.set_ebay_offer(id, "offer-1", 9.5).unwrap();
        let item = db.item(id).unwrap();
        assert_eq!(
            item.summary.ebay,
            Some(EbayListing {
                status: Some(EbayStatus::Draft),
                price: Some(9.5),
                listing_id: None,
                error: None,
            })
        );
        assert_eq!(item.ebay_images.len(), 1);
        assert_eq!(db.session(&s.id).unwrap().counts.ebay_drafts, 1);

        db.set_ebay_status(id, EbayStatus::Listed, Some("1100"))
            .unwrap();
        db.set_ebay_offer(id, "offer-1", 11.0).unwrap();
        let listing = db.item_summary(id).unwrap().ebay.unwrap();
        assert_eq!(
            listing.status,
            Some(EbayStatus::Listed),
            "a new price keeps it live"
        );
        assert_eq!(listing.listing_id.as_deref(), Some("1100"));
        assert_eq!(db.session(&s.id).unwrap().counts.ebay_listed, 1);

        db.clear_ebay(id).unwrap();
        let item = db.item(id).unwrap();
        assert_eq!(item.summary.ebay, None);
        assert_eq!(item.ebay_sku.as_deref(), Some("cl-1"), "the SKU stays");
        assert!(item.ebay_images.is_empty());
    }

    #[test]
    fn recover_requeues_interrupted_work() {
        let db = Db::memory().unwrap();
        let s = session(&db);
        db.insert_item(&s.id, "/a.jpg", None, &ItemImages::default(), None)
            .unwrap();
        db.claim_next(&s.id).unwrap();
        db.set_session_status(&s.id, SessionStatus::Running, None)
            .unwrap();
        db.recover_interrupted().unwrap();
        let s = db.session(&s.id).unwrap();
        assert_eq!(s.status, SessionStatus::Paused);
        assert_eq!(s.counts.queued, 1);
        assert!(s.started_at.is_some());
    }

    /// The app quit after a card was identified but before its grading job was submitted: no job to poll, so it
    /// is analysed again. A card whose job was submitted waits for the poller.
    #[test]
    fn recover_requeues_cards_without_a_grading_job() {
        let db = Db::memory().unwrap();
        let s = session(&db);
        let mut ids = Vec::new();
        for i in 0..2 {
            let id = db
                .insert_item(
                    &s.id,
                    &format!("/{i}.jpg"),
                    None,
                    &ItemImages::default(),
                    None,
                )
                .unwrap();
            db.claim_next(&s.id).unwrap();
            db.save_recognition(
                id,
                "tcg",
                &CardFields::default(),
                None,
                None,
                None,
                None,
                &Value::Null,
                1,
                ItemStatus::Grading,
            )
            .unwrap();
            ids.push(id);
        }
        db.set_job(ids[1], JobKind::Grading, "job-1").unwrap();
        db.set_job(ids[1], JobKind::Crop, "job-2").unwrap();
        db.recover_interrupted().unwrap();
        assert_eq!(db.item_summary(ids[0]).unwrap().status, ItemStatus::Queued);
        assert_eq!(db.item_summary(ids[1]).unwrap().status, ItemStatus::Grading);
        assert_eq!(
            db.jobs(&s.id).unwrap(),
            vec![
                (ids[1], JobKind::Grading, "job-1".to_string()),
                (ids[1], JobKind::Crop, "job-2".to_string())
            ]
        );
        assert_eq!(db.claim_next(&s.id).unwrap(), Some(ids[0]));

        // The card is done once both its grading and its crop are in, whichever comes first.
        db.save_grading(ids[1], Some(8.0), None, &Value::Null, &[], None)
            .unwrap();
        assert_eq!(db.item_summary(ids[1]).unwrap().status, ItemStatus::Grading);
        assert_eq!(
            db.jobs(&s.id).unwrap(),
            vec![(ids[1], JobKind::Crop, "job-2".to_string())]
        );
        db.save_crop(ids[1], &Value::Null, &["/crops/a.jpg".into()], None)
            .unwrap();
        let item = db.item(ids[1]).unwrap();
        assert_eq!(item.summary.status, ItemStatus::Done);
        assert_eq!(item.crop_images, vec!["/crops/a.jpg".to_string()]);
        assert!(db.jobs(&s.id).unwrap().is_empty());
    }

    fn queued_card(db: &Db, session: &Session) -> i64 {
        let id = db
            .insert_item(&session.id, "/a.jpg", None, &ItemImages::default(), None)
            .unwrap();
        db.claim_next(&session.id).unwrap();
        id
    }

    /// The session's page says how many cards are being cropped (an agent may be asked for their corners): those
    /// owing the crop stage, and in a session that only crops, those being worked on at all.
    #[test]
    fn counts_cards_in_their_crop_stage() {
        let db = Db::memory().unwrap();
        let s = session(&db);
        let cropping = |id: &str| db.session(id).unwrap().counts.cropping;
        let first = queued_card(&db, &s);
        assert_eq!(cropping(&s.id), 0, "being identified");
        db.set_stage(first, Stage::Crop).unwrap();
        assert_eq!(cropping(&s.id), 1);
        // Identified, waiting for its corners.
        db.set_item_status(first, ItemStatus::Grading, None)
            .unwrap();
        assert_eq!(cropping(&s.id), 1);
        let second = queued_card(&db, &s);
        db.set_stage(second, Stage::Grade).unwrap();
        assert_eq!(cropping(&s.id), 1, "the other is being graded");
        db.finish_pass(first).unwrap();
        assert_eq!(cropping(&s.id), 0);

        let only = db
            .create_session(&NewSession {
                name: "Crops".into(),
                source_kind: SourceKind::Folder,
                source_ref: "/tmp/cards".into(),
                id_mode: IdMode::Auto,
                options: SessionOptions {
                    task: Task::Crop,
                    ..SessionOptions::default()
                },
            })
            .unwrap();
        let card = db
            .insert_item(&only.id, "/b.jpg", None, &ItemImages::default(), None)
            .unwrap();
        assert_eq!(cropping(&only.id), 0, "queued");
        assert_eq!(db.claim_next(&only.id).unwrap(), Some(card));
        assert_eq!(cropping(&only.id), 1, "its first stage is the crop");
        db.finish_pass(card).unwrap();
        assert_eq!(cropping(&only.id), 0);
    }

    /// A card's jobs fail apart: a crop without its back leaves the grading out, to be polled and stored, and the
    /// card then says what failed. Analysing it again starts from nothing.
    #[test]
    fn a_failed_job_leaves_the_other_out() {
        let db = Db::memory().unwrap();
        let s = session(&db);
        let id = queued_card(&db, &s);
        db.set_job(id, JobKind::Grading, "job-1").unwrap();
        db.set_job(id, JobKind::Crop, "job-2").unwrap();
        db.save_crop(
            id,
            &Value::Null,
            &["/crops/a.jpg".into()],
            Some("crop: back: No card detected"),
        )
        .unwrap();
        let item = db.item_summary(id).unwrap();
        assert_eq!(
            (item.status, item.error),
            (ItemStatus::Grading, None),
            "not failed while a job is out"
        );
        assert_eq!(
            db.jobs(&s.id).unwrap(),
            vec![(id, JobKind::Grading, "job-1".to_string())]
        );
        db.save_grading(id, Some(8.0), None, &Value::Null, &[], None)
            .unwrap();
        let item = db.item_summary(id).unwrap();
        assert_eq!(item.status, ItemStatus::Error);
        assert_eq!(item.error.as_deref(), Some("crop: back: No card detected"));
        assert_eq!(item.grade, Some(8.0));
        assert!(db.jobs(&s.id).unwrap().is_empty());

        // Both failing: the grading's reason first. A failed job is polled no more.
        db.fail_job(id, JobKind::Grading, "grading failed: blurry")
            .unwrap();
        assert_eq!(
            db.item_summary(id).unwrap().error.as_deref(),
            Some("grading failed: blurry; crop: back: No card detected")
        );
        assert!(db.jobs(&s.id).unwrap().is_empty());

        assert_eq!(db.requeue(&s.id, Some(&[id])).unwrap(), 1);
        assert_eq!(db.claim_next(&s.id).unwrap(), Some(id));
        db.finish_pass(id).unwrap();
        let item = db.item_summary(id).unwrap();
        assert_eq!((item.status, item.error), (ItemStatus::Done, None));
    }

    /// A pass cut short: the card is back in the queue owing a stage. A job of an earlier stage that comes in
    /// meanwhile is stored without taking the card out of the queue, and a restart puts a card that owes a stage
    /// back in it.
    #[test]
    fn a_card_owing_a_stage_stays_queued() {
        let db = Db::memory().unwrap();
        let s = session(&db);
        let id = queued_card(&db, &s);
        db.set_job(id, JobKind::Grading, "job-1").unwrap();
        db.set_stage(id, Stage::Crop).unwrap();
        db.set_item_status(id, ItemStatus::Queued, None).unwrap();
        db.save_grading(id, Some(9.0), None, &Value::Null, &[], None)
            .unwrap();
        let item = db.item_summary(id).unwrap();
        assert_eq!((item.status, item.grade), (ItemStatus::Queued, Some(9.0)));
        assert_eq!(db.stage(id).unwrap(), Some(Stage::Crop));

        // The app quit while the crop was being submitted (the card was `grading` after its grade came in).
        let other = queued_card(&db, &s);
        db.set_job(other, JobKind::Grading, "job-2").unwrap();
        db.set_stage(other, Stage::Crop).unwrap();
        db.recover_interrupted().unwrap();
        assert_eq!(db.item_summary(other).unwrap().status, ItemStatus::Queued);
        assert_eq!(db.stage(other).unwrap(), Some(Stage::Crop));
        assert_eq!(
            db.jobs(&s.id).unwrap(),
            vec![(other, JobKind::Grading, "job-2".to_string())],
            "its grading is still polled"
        );

        // Part-way through its pass the card's status is the dispatcher's: a result that comes in before the next
        // stage is submitted doesn't make the card done, the end of the pass does.
        let third = queued_card(&db, &s);
        db.set_job(third, JobKind::Grading, "job-3").unwrap();
        db.set_stage(third, Stage::Crop).unwrap();
        db.save_grading(third, Some(7.0), None, &Value::Null, &[], None)
            .unwrap();
        assert_eq!(db.item_summary(third).unwrap().status, ItemStatus::Grading);
        db.finish_pass(third).unwrap();
        assert_eq!(db.item_summary(third).unwrap().status, ItemStatus::Done);
        assert_eq!(db.stage(third).unwrap(), None);

        // A failed card owes nothing, and analysing it again is a pass from the start.
        db.set_stage(other, Stage::Crop).unwrap();
        db.set_item_status(other, ItemStatus::Error, Some("x"))
            .unwrap();
        assert_eq!(db.stage(other).unwrap(), None);
        db.requeue(&s.id, None).unwrap();
        assert_eq!(db.stage(other).unwrap(), None);
        assert!(db.jobs(&s.id).unwrap().is_empty());
    }

    /// The eBay listings found for a card say nothing about another one: a new recognition, another candidate or
    /// typed card fields drop them; a new price, notes or a location don't.
    #[test]
    fn another_card_drops_its_ebay_listings() {
        let db = Db::memory().unwrap();
        let s = session(&db);
        let id = queued_card(&db, &s);
        let card = CardFields {
            name: "Pikachu".into(),
            ..Default::default()
        };
        let recognise = || {
            db.save_recognition(
                id,
                "tcg",
                &card,
                Some("high"),
                None,
                None,
                None,
                &Value::Null,
                1,
                ItemStatus::Done,
            )
            .unwrap()
        };
        let listing = MarketListing {
            id: "v1|1|0".into(),
            price: 5.0,
            currency: "USD".into(),
            counted: true,
            ..Default::default()
        };
        let search = || {
            let market = EbayMarket {
                query: "Pikachu".into(),
                range: PriceRange::of(std::slice::from_ref(&listing)),
                listings: vec![listing.clone()],
                ..Default::default()
            };
            db.save_market(id, &market).unwrap()
        };
        let searched = || db.item(id).unwrap().market.is_some();
        let edit = |name: &str, price, notes: &str| {
            let edit = ItemEdit {
                card: CardFields {
                    name: name.into(),
                    ..Default::default()
                },
                price,
                notes: notes.into(),
                location: String::new(),
                title: String::new(),
            };
            db.save_edit(id, &edit, &Reviewer::User, false).unwrap()
        };
        recognise();
        search();
        edit("Pikachu", Some(4.0), "sleeved");
        assert!(searched(), "a price and notes leave the listings");
        assert!(db.item_summary(id).unwrap().market_range.is_some());

        edit("Raichu", Some(4.0), "sleeved");
        assert!(!searched(), "typed card fields");
        assert!(db.item_summary(id).unwrap().market_range.is_none());

        search();
        db.save_pick(
            id,
            &card,
            Some("high"),
            None,
            None,
            &Value::Null,
            &Reviewer::User,
        )
        .unwrap();
        assert!(!searched(), "another candidate");

        search();
        recognise();
        assert!(!searched(), "a new recognition");
    }

    /// A database of the version before `item_jobs` (migration 010): the jobs its cards wait for are still polled,
    /// and a card that was done or failed has none.
    #[test]
    fn jobs_out_survive_the_migration() {
        let conn = Connection::open_in_memory().unwrap();
        for sql in &MIGRATIONS[..9] {
            conn.execute_batch(sql).unwrap();
        }
        conn.execute_batch(
            "PRAGMA user_version = 9;
             INSERT INTO sessions (id, name, source_kind, id_mode, options, status, created_at)
                VALUES ('s', 'Old', 'folder', 'auto', '{}', 'paused', '');
             INSERT INTO items (session_id, seq, event_id, front_path, status, grading_job_id, crop_job_id,
                    created_at, updated_at)
                VALUES ('s', 1, 'a', '/a.jpg', 'grading', 'job-1', 'job-2', '', ''),
                       ('s', 2, 'b', '/b.jpg', 'grading', NULL, 'job-3', '', ''),
                       ('s', 3, 'c', '/c.jpg', 'error', 'job-4', NULL, '', '');",
        )
        .unwrap();
        let db = Db::init(conn).unwrap();
        let ids: Vec<i64> = db.items("s").unwrap().iter().map(|i| i.id).collect();
        assert_eq!(
            db.jobs("s").unwrap(),
            vec![
                (ids[0], JobKind::Grading, "job-1".to_string()),
                (ids[0], JobKind::Crop, "job-2".to_string()),
                (ids[1], JobKind::Crop, "job-3".to_string()),
            ]
        );
        db.recover_interrupted().unwrap();
        assert_eq!(db.item_summary(ids[1]).unwrap().status, ItemStatus::Grading);
        db.save_crop(ids[1], &Value::Null, &[], None).unwrap();
        assert_eq!(db.item_summary(ids[1]).unwrap().status, ItemStatus::Done);
    }

    #[test]
    fn details_and_a_title_are_saved_like_notes() {
        let db = Db::memory().unwrap();
        let s = session(&db);
        let id = queued_card(&db, &s);
        let card = CardFields {
            name: "Jazz Chisholm Jr.".into(),
            brand: "Topps".into(),
            ..Default::default()
        };
        db.save_recognition(
            id,
            "sport",
            &card,
            Some("high"),
            None,
            None,
            None,
            &Value::Null,
            1,
            ItemStatus::Done,
        )
        .unwrap();
        let listing = MarketListing {
            id: "v1|1|0".into(),
            price: 5.0,
            currency: "USD".into(),
            counted: true,
            ..Default::default()
        };
        let market = EbayMarket {
            range: PriceRange::of(std::slice::from_ref(&listing)),
            listings: vec![listing],
            ..Default::default()
        };
        db.save_market(id, &market).unwrap();

        let mut edit = ItemEdit {
            card: CardFields {
                features: "RC Auto".into(),
                team: "New York Yankees".into(),
                serial: "43/75".into(),
                ..card.clone()
            },
            title: "2026 Topps Jazz Chisholm Jr. RC Auto 43/75".into(),
            ..Default::default()
        };
        db.save_edit(id, &edit, &Reviewer::User, false).unwrap();
        let saved = db.item_summary(id).unwrap();
        assert_eq!(saved.card, edit.card);
        assert_eq!(saved.title, edit.title);
        // What many cards get at once accepts none of them, and isn't a correction Cards Cloud would take.
        assert!(!saved.reviewed && !saved.edited && !saved.picked);
        assert!(saved.market_range.is_some());

        // A typed name is a correction, and the title someone wrote stays.
        edit.card.name = "Jazz Chisholm".into();
        db.save_edit(id, &edit, &Reviewer::User, false).unwrap();
        let saved = db.item_summary(id).unwrap();
        assert!(saved.reviewed && saved.edited);
        assert_eq!(saved.title, edit.title);

        edit.title.clear();
        db.save_edit(id, &edit, &Reviewer::User, false).unwrap();
        assert_eq!(db.item_summary(id).unwrap().title, "");

        // Another candidate is another card: its details are the candidate's, and the title is automatic again.
        edit.title = "A title".into();
        db.save_edit(id, &edit, &Reviewer::User, false).unwrap();
        db.save_pick(
            id,
            &card,
            Some("high"),
            None,
            None,
            &Value::Null,
            &Reviewer::User,
        )
        .unwrap();
        let picked = db.item_summary(id).unwrap();
        assert_eq!(picked.title, "");
        assert_eq!(
            (picked.card.brand.as_str(), picked.card.features.as_str()),
            ("Topps", "")
        );
    }

    #[test]
    fn older_cards_get_their_details_from_the_answer_they_kept() {
        let conn = Connection::open_in_memory().unwrap();
        for sql in &MIGRATIONS[..DETAILS_MIGRATION - 1] {
            conn.execute_batch(sql).unwrap();
        }
        let answer = |name: &str| {
            json!({"endpoint": "collectibles/v2/sport_id", "id_type": "sport", "response": {"records": [{
                "_id": "front",
                "_objects": [{"name": "Card", "bound_box": [0, 0, 10, 10], "_identification": {
                    "best_match": {"name": name, "card_number": "AU-9", "company": "Topps",
                        "team": "New York Yankees", "serial_number": "43/75"},
                    "distances": [0.2]}}]}]}})
            .to_string()
        };
        conn.execute_batch(&format!(
            "PRAGMA user_version = {};
             INSERT INTO sessions (id, name, source_kind, id_mode, options, status, created_at)
                VALUES ('s', 'Old', 'folder', 'sport', '{{}}', 'completed', '');",
            DETAILS_MIGRATION - 1
        ))
        .unwrap();
        let insert = "INSERT INTO items (session_id, seq, event_id, front_path, status, id_type, name, card_number,
                edited, recognition, rev, created_at, updated_at)
            VALUES ('s', ?1, ?1, '/a.jpg', 'done', 'sport', ?2, 'AU-9', ?3, ?4, 3, '', 'before')";
        let rows = [
            (1, "Jazz Chisholm Jr.", 0, Some(answer("Jazz Chisholm Jr."))),
            // Typed since, imported as another card, never answered, an answer that doesn't parse.
            (2, "Jazz Chisholm", 1, Some(answer("Jazz Chisholm"))),
            (3, "Aaron Judge", 0, Some(answer("Jazz Chisholm Jr."))),
            (4, "Jazz Chisholm Jr.", 0, None),
            (5, "Jazz Chisholm Jr.", 0, Some("{\"company\"".to_string())),
        ];
        for (seq, name, edited, raw) in rows {
            conn.execute(insert, params![seq, name, edited, raw])
                .unwrap();
        }
        let db = Db::init(conn).unwrap();
        let cards = db.items("s").unwrap();
        let details = |i: &ItemSummary| {
            [
                i.card.brand.clone(),
                i.card.team.clone(),
                i.card.serial.clone(),
            ]
        };
        assert_eq!(details(&cards[0]), ["Topps", "New York Yankees", "43/75"]);
        assert_eq!(
            (cards[0].updated_at.as_str(), cards[0].synced),
            ("before", false)
        );
        for untouched in &cards[1..] {
            assert_eq!(details(untouched), ["", "", ""], "card {}", untouched.seq);
        }
    }

    /// Writes sessions and their cards as the UI's commands return them, for the browser UI to show instead of its
    /// demo data (`VITE_DEMO_SNAPSHOT=<file> npm run dev`): real results, made by the app, in screenshots taken in a
    /// browser. Reads `CARDLOADER_SNAPSHOT_DB`, a copy of `cardloader.db` (with its `-wal`): opening the one a
    /// running app uses would pause its sessions. `CARDLOADER_SNAPSHOT_SESSIONS` (ids or names, comma-separated)
    /// limits it to those sessions. Writes `CARDLOADER_SNAPSHOT_TO`, with the home folder in paths as `/Users/demo`.
    #[test]
    #[ignore]
    fn snapshot_for_the_browser_ui() {
        let var = |name: &str| std::env::var(name).unwrap_or_else(|_| panic!("{name}"));
        let db = Db::open(Path::new(&var("CARDLOADER_SNAPSHOT_DB"))).unwrap();
        let only: Vec<String> = std::env::var("CARDLOADER_SNAPSHOT_SESSIONS")
            .map(|s| {
                s.split(',')
                    .map(|x| x.trim().to_string())
                    .filter(|x| !x.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        let sessions: Vec<Session> = db
            .sessions()
            .unwrap()
            .into_iter()
            .filter(|s| only.is_empty() || only.contains(&s.id) || only.contains(&s.name))
            .collect();
        let mut items = serde_json::Map::new();
        for s in &sessions {
            let cards: Vec<ItemDetail> = db
                .items(&s.id)
                .unwrap()
                .iter()
                .map(|i| db.item(i.id).unwrap())
                .collect();
            items.insert(s.id.clone(), serde_json::to_value(cards).unwrap());
        }
        // Paths show in the UI, so the user's home folder is written as /Users/demo; `home` says which folder that is
        // for loading the pictures, and stays on this computer.
        let home = var("HOME");
        let out = json!({ "sessions": sessions, "items": items })
            .to_string()
            .replace(&home, "/Users/demo");
        let out = format!("{{\"home\":{},{}", json!(home), &out[1..]);
        std::fs::write(var("CARDLOADER_SNAPSHOT_TO"), out).unwrap();
        println!(
            "{} sessions, {} cards",
            sessions.len(),
            items
                .values()
                .map(|v| v.as_array().unwrap().len())
                .sum::<usize>()
        );
    }
}
