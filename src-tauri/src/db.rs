//! Local session store: one SQLite file in the app data dir.
//!
//! Every call takes the connection lock briefly; the runner never holds it across an await.

use std::path::Path;
use std::sync::{Arc, Mutex};

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde_json::Value;

use crate::error::{Error, Result};
use crate::model::*;

const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/001_init.sql"),
    include_str!("../migrations/002_picks.sql"),
    include_str!("../migrations/003_slab.sql"),
    include_str!("../migrations/004_ebay.sql"),
    include_str!("../migrations/005_hosted_images.sql"),
    include_str!("../migrations/006_review.sql"),
    include_str!("../migrations/007_agent_review.sql"),
];

#[derive(Clone)]
pub struct Db(Arc<Mutex<Connection>>);

const ITEM_SUMMARY_COLUMNS: &str =
    "id, session_id, seq, status, front_path, back_path, thumb_path, id_type, \
     name, full_name, set_name, set_code, card_number, series, year, subcategory, \
     confidence, distance, price, grade, condition, error, synced_rev, rev, updated_at, \
     picks, picks_synced, slab_company, slab_grade, slab_certificate, \
     ebay_status, ebay_price, ebay_listing_id, ebay_error, reviewed, edited, notes, location, \
     agent_reviewed, review_note";

/// An identified card nobody has accepted or corrected yet, whatever its confidence (the same rule as
/// `needsReview` in `src/lib/cardview.ts`). Columns of `items` as `i`.
const TO_REVIEW: &str = "(i.status IN ('done', 'grading') AND i.reviewed = 0)";

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
                    COALESCE(SUM(i.price), 0),
                    COALESCE(SUM(i.ebay_status = 'draft'), 0),
                    COALESCE(SUM(i.ebay_status = 'listed'), 0),
                    COALESCE(SUM(i.ebay_status = 'sold'), 0),
                    COALESCE(SUM({TO_REVIEW}), 0),
                    COALESCE(SUM(i.calls), 0)
             FROM sessions s LEFT JOIN items i ON i.session_id = s.id
             {filter} GROUP BY s.id {order}"
        );
        self.with(|c| {
            let mut stmt = c.prepare(&sql)?;
            let rows = stmt.query_map(args, |r| {
                Ok(Session {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    source_kind: SourceKind::parse(&r.get::<_, String>(2)?),
                    source_ref: r.get(3)?,
                    id_mode: IdMode::parse(&r.get::<_, String>(4)?),
                    options: serde_json::from_str(&r.get::<_, String>(5)?).unwrap_or_default(),
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

    /// After a restart nothing is running: running sessions become paused, half-done items queued.
    pub fn recover_interrupted(&self) -> Result<()> {
        self.with(|c| {
            c.execute_batch(
                "UPDATE sessions SET status = 'paused' WHERE status = 'running';
                 UPDATE items SET status = 'queued' WHERE status = 'recognizing';",
            )
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
        grade: Option<f64>,
        condition: Option<&str>,
        grading: Option<&Value>,
    ) -> Result<i64> {
        let ts = now();
        let key = (!front.is_empty()).then(|| crate::source::file_key(std::path::Path::new(front)));
        self.with(|c| {
            let event_id = new_event_id(c, session_id, key.as_deref())?;
            c.execute(
                "INSERT INTO items (session_id, seq, event_id, front_path, back_path, thumb_path, preview_path,
                    back_preview_path, status, id_type, name, full_name, set_name, set_code, card_number, series,
                    year, subcategory, confidence, distance, price, grade, condition, grading, rev,
                    analyzed_at, created_at, updated_at, reviewed)
                 VALUES (?1, (SELECT COALESCE(MAX(seq), 0) + 1 FROM items WHERE session_id = ?1),
                    ?2, ?3, ?4, ?5, ?6, ?7, 'done', ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19,
                    ?20, ?21, ?22, 1, ?23, ?23, ?23, 1)",
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
                    ts
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
        let columns = ITEM_SUMMARY_COLUMNS
            .split(',')
            .map(|c| format!("i.{}", c.trim()))
            .collect::<Vec<_>>()
            .join(", ");
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
             SELECT {columns}, i.session_name, i.copies, COUNT(*) OVER (), COALESCE(SUM(i.price) OVER (), 0)
             FROM found i ORDER BY i.analyzed_at DESC, i.id DESC LIMIT ?2"
        );
        let pattern = match query.trim() {
            "" => String::new(),
            q => format!("%{q}%"),
        };
        self.with(|c| {
            let mut stmt = c.prepare(&sql)?;
            let mut search = CardSearch::default();
            let rows = stmt.query_map(params![pattern, limit as i64], |r| {
                Ok((
                    CardRow {
                        item: item_summary(r)?,
                        session_name: r.get(40)?,
                        copies: r.get(41)?,
                    },
                    r.get::<_, i64>(42)?,
                    r.get::<_, f64>(43)?,
                ))
            })?;
            for row in rows {
                let (card, total, value) = row?;
                search.total = total;
                search.value = value;
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

    pub fn set_item_status(&self, id: i64, status: ItemStatus, error: Option<&str>) -> Result<()> {
        self.with(|c| {
            c.execute(
                "UPDATE items SET status = ?2, error = ?3, updated_at = ?4 WHERE id = ?1",
                params![id, status.as_str(), error, now()],
            )
        })?;
        Ok(())
    }

    /// Re-queues failed items (and, with `all`, finished ones too).
    pub fn requeue(&self, session_id: &str, item_ids: Option<&[i64]>) -> Result<usize> {
        self.with(|c| match item_ids {
            Some(ids) => {
                let mut n = 0;
                for id in ids {
                    n += c.execute(
                        "UPDATE items SET status = 'queued', error = NULL, grading_job_id = NULL
                         WHERE id = ?1 AND session_id = ?2 AND status IN ('error', 'done')",
                        params![id, session_id],
                    )?;
                }
                Ok(n)
            }
            None => c.execute(
                "UPDATE items SET status = 'queued', error = NULL, grading_job_id = NULL
                 WHERE session_id = ?1 AND status = 'error'",
                params![session_id],
            ),
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
                "UPDATE items SET id_type = ?2, name = ?3, full_name = ?4, set_name = ?5, set_code = ?6,
                    card_number = ?7, series = ?8, year = ?9, subcategory = ?10, confidence = ?11,
                    distance = ?12, price = ?13, recognition = ?14, calls = calls + ?15, status = ?16,
                    error = NULL, analyzed_at = ?17, updated_at = ?17, rev = rev + 1, slab_company = ?18,
                    slab_grade = ?19, slab_certificate = ?20, reviewed = 0, edited = 0,
                    agent_reviewed = 0, review_note = NULL
                 WHERE id = ?1",
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
                "UPDATE items SET name = ?2, full_name = ?3, set_name = ?4, set_code = ?5, card_number = ?6,
                    series = ?7, year = ?8, subcategory = ?9, confidence = ?10, distance = ?11, price = ?12,
                    recognition = ?13, picks = picks + 1, reviewed = 1, edited = 0, updated_at = ?14,
                    agent_reviewed = ?15, review_note = ?16
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
                    confidence,
                    distance,
                    price,
                    raw.to_string(),
                    ts,
                    by.is_agent(),
                    by.note()
                ],
            )
        })?;
        Ok(())
    }

    /// The user typed the card's fields or price. Like a pick, it reaches Cards Cloud as a PATCH (`picks`
    /// counts it); notes and the location stay local.
    pub fn save_edit(&self, id: i64, edit: &ItemEdit, by: &Reviewer) -> Result<()> {
        let card = &edit.card;
        self.with(|c| {
            c.execute(
                "WITH card AS (SELECT COALESCE(name, '') IS NOT ?2 OR COALESCE(full_name, '') IS NOT ?3
                    OR COALESCE(set_name, '') IS NOT ?4 OR COALESCE(set_code, '') IS NOT ?5
                    OR COALESCE(card_number, '') IS NOT ?6 OR COALESCE(series, '') IS NOT ?7
                    OR COALESCE(year, '') IS NOT ?8 OR COALESCE(subcategory, '') IS NOT ?9
                    OR price IS NOT ?10 AS changed FROM items WHERE id = ?1)
                 UPDATE items SET
                    picks = picks + (SELECT changed FROM card), edited = edited OR (SELECT changed FROM card),
                    name = ?2, full_name = ?3, set_name = ?4, set_code = ?5, card_number = ?6, series = ?7,
                    year = ?8, subcategory = ?9, price = ?10, notes = ?11, location = ?12, reviewed = 1,
                    updated_at = ?13, agent_reviewed = ?14, review_note = ?15
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
                    by.note()
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
    /// too), the response says it is upright, and pictures hosted for exports are uploaded again.
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
                    updated_at = ?5, back_preview_path = COALESCE(?6, back_preview_path)
                 WHERE id = ?1",
                params![id, thumb, preview, raw.to_string(), now(), back_preview],
            )
        })?;
        Ok(())
    }

    /// Accepts the cards' matches (or takes that back); returns how many changed. The user accepting a card an
    /// agent accepted makes it theirs; an agent never takes over the user's accept.
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
                         WHERE id = ?1 AND session_id = ?2 AND reviewed = 1",
                        params![id, session_id],
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

    pub fn set_grading_job(&self, id: i64, job_id: &str) -> Result<()> {
        self.with(|c| {
            c.execute(
                "UPDATE items SET grading_job_id = ?2, calls = calls + 1, status = 'grading', updated_at = ?3 WHERE id = ?1",
                params![id, job_id, now()],
            )
        })?;
        Ok(())
    }

    pub fn save_grading(
        &self,
        id: i64,
        grade: Option<f64>,
        condition: Option<&str>,
        raw: &Value,
        images: &[String],
    ) -> Result<()> {
        self.with(|c| {
            c.execute(
                "UPDATE items SET grade = ?2, condition = ?3, grading = ?4, grading_images = ?5, status = 'done',
                    error = NULL, updated_at = ?6, rev = rev + 1
                 WHERE id = ?1",
                params![id, grade, condition, raw.to_string(), serde_json::to_string(images).unwrap_or_default(), now()],
            )
        })?;
        Ok(())
    }

    /// Items waiting for an async grading result: (item id, job id).
    pub fn grading_jobs(&self, session_id: &str) -> Result<Vec<(i64, String)>> {
        self.with(|c| {
            let mut stmt = c.prepare(
                "SELECT id, grading_job_id FROM items
                 WHERE session_id = ?1 AND status = 'grading' AND grading_job_id IS NOT NULL ORDER BY seq",
            )?;
            let rows = stmt.query_map(params![session_id], |r| Ok((r.get(0)?, r.get(1)?)))?;
            rows.collect()
        })
    }

    pub fn items(&self, session_id: &str) -> Result<Vec<ItemSummary>> {
        self.with(|c| {
            let mut stmt = c.prepare(&format!(
                "SELECT {ITEM_SUMMARY_COLUMNS} FROM items WHERE session_id = ?1 ORDER BY seq"
            ))?;
            let rows = stmt.query_map(params![session_id], item_summary)?;
            rows.collect()
        })
    }

    pub fn item_summary(&self, id: i64) -> Result<ItemSummary> {
        self.with(|c| {
            c.query_row(
                &format!("SELECT {ITEM_SUMMARY_COLUMNS} FROM items WHERE id = ?1"),
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
                    remote_id, ebay_sku, ebay_offer_id, ebay_images, hosted_images
                 FROM items WHERE id = ?1",
                params![id],
                |r| {
                    Ok(ItemDetail {
                        summary,
                        event_id: r.get(0)?,
                        preview_path: r.get(1)?,
                        back_preview_path: r.get(2)?,
                        recognition: json_col(r.get(3)?),
                        grading: json_col(r.get(4)?),
                        grading_images: json_col(r.get(5)?)
                            .and_then(|v| serde_json::from_value(v).ok())
                            .unwrap_or_default(),
                        calls: r.get(6)?,
                        analyzed_at: r.get(7)?,
                        remote_id: r.get(8)?,
                        ebay_sku: r.get(9)?,
                        ebay_offer_id: r.get(10)?,
                        ebay_images: json_col(r.get(11)?)
                            .and_then(|v| serde_json::from_value(v).ok())
                            .unwrap_or_default(),
                        hosted_images: json_col(r.get(12)?)
                            .and_then(|v| serde_json::from_value(v).ok())
                            .unwrap_or_default(),
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
    let synced_rev: i64 = r.get(22)?;
    let rev: i64 = r.get(23)?;
    let picks: i64 = r.get(25)?;
    let picks_synced: i64 = r.get(26)?;
    Ok(ItemSummary {
        id: r.get(0)?,
        session_id: r.get(1)?,
        seq: r.get(2)?,
        status: ItemStatus::parse(&r.get::<_, String>(3)?),
        front_path: r.get(4)?,
        back_path: r.get(5)?,
        thumb_path: r.get(6)?,
        id_type: r.get(7)?,
        card: CardFields {
            name: text(8)?,
            full_name: text(9)?,
            set_name: text(10)?,
            set_code: text(11)?,
            card_number: text(12)?,
            series: text(13)?,
            year: text(14)?,
            subcategory: text(15)?,
        },
        confidence: r.get(16)?,
        distance: r.get(17)?,
        price: r.get(18)?,
        grade: r.get(19)?,
        condition: r.get(20)?,
        slab: slab_label(r)?,
        error: r.get(21)?,
        synced: synced_rev > 0 && synced_rev == rev && picks == picks_synced,
        updated_at: r.get(24)?,
        picked: picks > 0,
        edited: r.get(35)?,
        reviewed: r.get(34)?,
        agent_reviewed: r.get(38)?,
        review_note: text(39)?,
        notes: text(36)?,
        location: text(37)?,
        ebay: ebay_listing(r)?,
    })
}

/// The eBay columns (30–33): a listing when any of them is set.
fn ebay_listing(r: &Row) -> rusqlite::Result<Option<EbayListing>> {
    let status: Option<String> = r.get(30)?;
    let listing = EbayListing {
        status: status.as_deref().map(EbayStatus::parse),
        price: r.get(31)?,
        listing_id: r.get(32)?,
        error: r.get(33)?,
    };
    Ok((listing != EbayListing::default()).then_some(listing))
}

/// The slab columns (27–29): a label when any of them is set.
fn slab_label(r: &Row) -> rusqlite::Result<Option<SlabLabel>> {
    let [company, grade, certificate] = [27, 28, 29].map(|i| r.get::<_, Option<String>>(i));
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
}
