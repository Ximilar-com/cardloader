//! eBay jobs of one session, run in the background (one at a time per session, see `Runner::ebay_start`):
//!
//! * **drafts**: upload each card's pictures, create (or replace) its inventory item, then create its offer —
//!   or update the offer it has, e.g. with a new price. An unpublished offer is a draft on eBay.
//! * **publish**: publish the session's drafts; each becomes a live listing.
//! * **refresh**: read the offers back: listed, sold, or ended.
//! * **remove**: delete the offers (ending live listings) and inventory items.
//!
//! The bulk calls answer per card (HTTP 207), so one bad card doesn't stop the others: its error is kept on the
//! card. Failures every card would hit (login, network) stop the job and are kept on the session.

use std::collections::HashMap;
use std::sync::Arc;

use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use super::client::EbayClient;
use super::listing::{self, CardCondition, Listing, Marketplace, MAX_BATCH};
use crate::model::{EbayStatus, ItemDetail};
use crate::runner::Runner;
use crate::ximilar::http::{errors_text, ApiError, Failure};

/// Listings are read back at most this often unless asked.
const REFRESH_EVERY: chrono::TimeDelta = chrono::TimeDelta::minutes(10);

#[derive(Debug, Clone, Copy, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DraftItem {
    pub item_id: i64,
    pub price: f64,
}

pub enum Job {
    Drafts {
        items: Vec<DraftItem>,
        condition: CardCondition,
    },
    /// The given cards' drafts, or all of the session's.
    Publish(Option<Vec<i64>>),
    Refresh {
        force: bool,
    },
    Remove(Vec<i64>),
}

impl Job {
    fn action(&self) -> &'static str {
        match self {
            Job::Drafts { .. } => "drafts",
            Job::Publish(_) => "publish",
            Job::Refresh { .. } => "refresh",
            Job::Remove(_) => "remove",
        }
    }
}

/// How far a session's eBay job is (the `ebay-progress` event).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EbayProgress {
    pub session_id: String,
    pub action: String,
    pub done: usize,
    pub total: usize,
    pub running: bool,
}

pub async fn run(runner: Arc<Runner>, session_id: String, job: Job, cancel: CancellationToken) {
    let _ = runner.db.set_ebay_error(&session_id, None);
    runner.emit_session(&session_id);
    let action = job.action();
    let result = match runner.ebay.client(&runner.settings).await {
        Ok(client) => {
            let mut work = Work {
                runner: runner.clone(),
                session_id: session_id.clone(),
                client,
                cancel,
                progress: EbayProgress {
                    session_id: session_id.clone(),
                    action: action.into(),
                    running: true,
                    ..Default::default()
                },
            };
            match job {
                Job::Drafts { items, condition } => work.drafts(items, condition).await,
                Job::Publish(items) => work.publish(items).await,
                Job::Refresh { force } => work.refresh(force).await,
                Job::Remove(items) => work.remove(items).await,
            }
        }
        Err(e) => Err(e),
    };
    match result {
        Ok(()) | Err(ApiError::Cancelled) => {}
        Err(e) => {
            tracing::warn!("eBay {action} of session {session_id} stopped: {e}");
            let _ = runner.db.set_ebay_error(&session_id, Some(&e.to_string()));
        }
    }
    runner.emit_session(&session_id);
}

/// Failures no other card would get past.
fn fatal(e: &ApiError) -> bool {
    matches!(e.failure(), Failure::Auth | Failure::Retry) || matches!(e, ApiError::Cancelled)
}

/// A bulk response entry's failure, if it failed.
fn entry_error(entry: &Value) -> Option<String> {
    let status = entry["statusCode"].as_u64().unwrap_or(200);
    let errors = errors_text(&entry["errors"]);
    if (200..300).contains(&status) && errors.is_none() {
        None
    } else {
        Some(errors.unwrap_or_else(|| format!("HTTP {status}")))
    }
}

/// A bulk response's entries by `key` (`sku`, `offerId`).
fn by_key(resp: &Value, key: &str) -> HashMap<String, Value> {
    resp["responses"]
        .as_array()
        .map(|entries| {
            entries
                .iter()
                .filter_map(|e| Some((e[key].as_str()?.to_string(), e.clone())))
                .collect()
        })
        .unwrap_or_default()
}

/// The offer eBay says the SKU already has ("Offer entity already exists", 25002).
fn existing_offer(entry: &Value) -> Option<String> {
    entry["errors"]
        .as_array()?
        .iter()
        .filter_map(|e| e["parameters"].as_array())
        .flatten()
        .find(|p| p["name"] == "offerId")?["value"]
        .as_str()
        .map(str::to_string)
}

struct Ready {
    item: ItemDetail,
    listing: Listing,
    sku: String,
    price: f64,
    images: Vec<String>,
}

struct Work {
    runner: Arc<Runner>,
    session_id: String,
    client: EbayClient,
    cancel: CancellationToken,
    progress: EbayProgress,
}

impl Work {
    fn item_error(&self, id: i64, error: &str) {
        let _ = self.runner.db.set_ebay_item_error(id, Some(error));
        self.runner.emit_item(id);
    }

    fn advance(&mut self, by: usize) {
        self.progress.done = (self.progress.done + by).min(self.progress.total);
        self.runner.ebay_progress(&self.progress);
    }

    fn start(&mut self, total: usize) {
        self.progress.total = total;
        self.runner.ebay_progress(&self.progress);
    }

    /// The session's cards: those asked for, else every card.
    fn items(&self, ids: Option<&[i64]>) -> Vec<ItemDetail> {
        let summaries = self.runner.db.items(&self.session_id).unwrap_or_default();
        summaries
            .into_iter()
            .filter(|i| ids.is_none_or(|ids| ids.contains(&i.id)))
            .filter_map(|i| self.runner.db.item(i.id).ok())
            .collect()
    }

    async fn call(
        &self,
        method: Method,
        path: &str,
        body: Option<&Value>,
    ) -> Result<Value, ApiError> {
        self.client.call(method, path, body, &self.cancel).await
    }

    async fn drafts(
        &mut self,
        items: Vec<DraftItem>,
        condition: CardCondition,
    ) -> Result<(), ApiError> {
        let cfg = self.runner.settings.get().ebay;
        let market = Marketplace::of(&cfg.marketplace);
        let prices: HashMap<i64, f64> = items.iter().map(|d| (d.item_id, d.price)).collect();
        let ids: Vec<i64> = items.iter().map(|d| d.item_id).collect();
        let cards = self.items(Some(&ids));
        self.start(cards.len());
        for chunk in cards.chunks(MAX_BATCH) {
            let mut ready = Vec::new();
            for item in chunk {
                if self.cancel.is_cancelled() {
                    return Err(ApiError::Cancelled);
                }
                if let Some(r) = self.prepare(item, prices[&item.summary.id]).await? {
                    ready.push(r);
                }
            }
            let entries: Vec<Value> = ready
                .iter()
                .map(|r| {
                    listing::inventory_item(
                        &r.item.summary,
                        &r.listing,
                        &r.sku,
                        market,
                        condition,
                        &r.images,
                    )
                })
                .collect();
            let stored = self
                .bulk(
                    "sell/inventory/v1/bulk_create_or_replace_inventory_item",
                    json!({"requests": entries}),
                    &ready,
                    "sku",
                    |r| r.sku.clone(),
                )
                .await?;
            let ready: Vec<Ready> = ready
                .into_iter()
                .filter(|r| {
                    let error = stored.get(&r.sku).cloned().flatten();
                    if let Some(e) = &error {
                        self.item_error(r.item.summary.id, e);
                    }
                    stored.contains_key(&r.sku) && error.is_none()
                })
                .collect();
            self.offers(ready, &cfg).await?;
            self.advance(chunk.len());
            self.runner.emit_session(&self.session_id);
        }
        Ok(())
    }

    /// The card's SKU and pictures, or `None` when it can't be listed (the reason is kept on the card).
    async fn prepare(&self, item: &ItemDetail, price: f64) -> Result<Option<Ready>, ApiError> {
        let id = item.summary.id;
        let status = item.summary.ebay.as_ref().and_then(|e| e.status);
        if status == Some(EbayStatus::Sold) {
            self.item_error(id, "this card was sold on eBay");
            return Ok(None);
        }
        if !(price.is_finite() && price > 0.0) {
            self.item_error(id, "the card needs a price");
            return Ok(None);
        }
        let listing = match listing::plan(&item.summary) {
            Ok(l) => l,
            Err(why) => {
                self.item_error(id, &why);
                return Ok(None);
            }
        };
        let sku = match &item.ebay_sku {
            Some(sku) => sku.clone(),
            None => {
                let sku = listing::sku(&self.session_id, &item.event_id);
                let _ = self.runner.db.set_ebay_sku(id, &sku);
                sku
            }
        };
        let images = if item.ebay_images.is_empty() {
            match self.upload_images(item).await {
                Ok(urls) => {
                    let _ = self.runner.db.set_ebay_images(id, &urls);
                    urls
                }
                Err(e) if fatal(&e) => return Err(e),
                Err(e) => {
                    self.item_error(id, &format!("pictures not uploaded: {e}"));
                    return Ok(None);
                }
            }
        } else {
            item.ebay_images.clone()
        };
        Ok(Some(Ready {
            item: item.clone(),
            listing,
            sku,
            price: (price * 100.0).round() / 100.0,
            images,
        }))
    }

    /// The front, and the back when there is one.
    async fn upload_images(&self, item: &ItemDetail) -> Result<Vec<String>, ApiError> {
        let mut urls = Vec::new();
        for side in crate::market::pictures(item) {
            let jpeg = crate::market::read_picture(side).await?;
            urls.push(self.client.upload_image(jpeg, &self.cancel).await?);
        }
        Ok(urls)
    }

    /// A bulk call; per card (by `key`): `None` = stored, else the error. A failure of the whole call fails
    /// every card, unless no card would get past it.
    async fn bulk<T>(
        &self,
        path: &str,
        body: Value,
        cards: &[T],
        key: &str,
        key_of: impl Fn(&T) -> String,
    ) -> Result<HashMap<String, Option<String>>, ApiError> {
        if cards.is_empty() {
            return Ok(HashMap::new());
        }
        match self.call(Method::POST, path, Some(&body)).await {
            Ok(resp) => Ok(by_key(&resp, key)
                .into_iter()
                .map(|(k, entry)| (k, entry_error(&entry)))
                .collect()),
            Err(e) if fatal(&e) => Err(e),
            Err(e) => Ok(cards
                .iter()
                .map(|c| (key_of(c), Some(e.to_string())))
                .collect()),
        }
    }

    /// Updates the offers cards have; creates the others'.
    async fn offers(
        &self,
        ready: Vec<Ready>,
        cfg: &crate::settings::EbaySettings,
    ) -> Result<(), ApiError> {
        let mut new = Vec::new();
        for r in ready {
            match r.item.ebay_offer_id.clone() {
                Some(offer_id) => {
                    if !self.update_offer(&r, &offer_id, cfg).await? {
                        new.push(r);
                    }
                }
                None => new.push(r),
            }
        }
        if new.is_empty() {
            return Ok(());
        }
        let offers: Vec<Value> = new
            .iter()
            .map(|r| listing::offer(&r.item.summary, &r.listing, &r.sku, cfg, r.price))
            .collect();
        let resp = match self
            .call(
                Method::POST,
                "sell/inventory/v1/bulk_create_offer",
                Some(&json!({"requests": offers})),
            )
            .await
        {
            Ok(resp) => resp,
            Err(e) if fatal(&e) => return Err(e),
            Err(e) => {
                for r in &new {
                    self.item_error(r.item.summary.id, &e.to_string());
                }
                return Ok(());
            }
        };
        let entries = by_key(&resp, "sku");
        for r in &new {
            let id = r.item.summary.id;
            let Some(entry) = entries.get(&r.sku) else {
                self.item_error(id, "eBay didn't answer for this card");
                continue;
            };
            match (entry_error(entry), entry["offerId"].as_str()) {
                (None, Some(offer_id)) => {
                    let _ = self.runner.db.set_ebay_offer(id, offer_id, r.price);
                    self.runner.emit_item(id);
                }
                (error, _) => match existing_offer(entry) {
                    Some(offer_id) => {
                        self.update_offer(r, &offer_id, cfg).await?;
                    }
                    None => {
                        self.item_error(id, &error.unwrap_or_else(|| "no offer was created".into()))
                    }
                },
            }
        }
        Ok(())
    }

    /// Replaces an offer with the card's current one. `false`: eBay no longer has it.
    async fn update_offer(
        &self,
        r: &Ready,
        offer_id: &str,
        cfg: &crate::settings::EbaySettings,
    ) -> Result<bool, ApiError> {
        let id = r.item.summary.id;
        let body = listing::offer(&r.item.summary, &r.listing, &r.sku, cfg, r.price);
        match self
            .call(
                Method::PUT,
                &format!("sell/inventory/v1/offer/{offer_id}"),
                Some(&body),
            )
            .await
        {
            Ok(_) => {
                let _ = self.runner.db.set_ebay_offer(id, offer_id, r.price);
                self.runner.emit_item(id);
                Ok(true)
            }
            Err(e) if e.status() == Some(404) => Ok(false),
            Err(e) if fatal(&e) => Err(e),
            Err(e) => {
                self.item_error(id, &e.to_string());
                Ok(true)
            }
        }
    }

    async fn publish(&mut self, ids: Option<Vec<i64>>) -> Result<(), ApiError> {
        let drafts: Vec<(i64, String)> = self
            .items(ids.as_deref())
            .into_iter()
            .filter(|i| i.summary.ebay.as_ref().and_then(|e| e.status) == Some(EbayStatus::Draft))
            .filter_map(|i| Some((i.summary.id, i.ebay_offer_id?)))
            .collect();
        self.start(drafts.len());
        for chunk in drafts.chunks(MAX_BATCH) {
            let requests: Vec<Value> = chunk
                .iter()
                .map(|(_, offer)| json!({"offerId": offer}))
                .collect();
            let entries: HashMap<String, Value> = match self
                .call(
                    Method::POST,
                    "sell/inventory/v1/bulk_publish_offer",
                    Some(&json!({"requests": requests})),
                )
                .await
            {
                Ok(resp) => by_key(&resp, "offerId"),
                Err(e) if fatal(&e) => return Err(e),
                Err(e) => {
                    for (id, _) in chunk {
                        self.item_error(*id, &e.to_string());
                    }
                    self.advance(chunk.len());
                    continue;
                }
            };
            for (id, offer) in chunk {
                let Some(entry) = entries.get(offer) else {
                    self.item_error(*id, "eBay didn't answer for this card");
                    continue;
                };
                match entry_error(entry) {
                    None => {
                        let _ = self.runner.db.set_ebay_status(
                            *id,
                            EbayStatus::Listed,
                            entry["listingId"].as_str(),
                        );
                        self.runner.emit_item(*id);
                    }
                    Some(e) => self.item_error(*id, &e),
                }
            }
            self.advance(chunk.len());
            self.runner.emit_session(&self.session_id);
        }
        Ok(())
    }

    async fn refresh(&mut self, force: bool) -> Result<(), ApiError> {
        let session = self
            .runner
            .db
            .session(&self.session_id)
            .map_err(|e| ApiError::Decode(e.to_string()))?;
        let recent = session
            .ebay_checked_at
            .as_deref()
            .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
            .is_some_and(|t| chrono::Utc::now().signed_duration_since(t) < REFRESH_EVERY);
        if recent && !force {
            return Ok(());
        }
        let tracked: Vec<ItemDetail> = self
            .items(None)
            .into_iter()
            .filter(|i| {
                i.ebay_offer_id.is_some()
                    && matches!(
                        i.summary.ebay.as_ref().and_then(|e| e.status),
                        Some(EbayStatus::Draft | EbayStatus::Listed)
                    )
            })
            .collect();
        self.start(tracked.len());
        for item in tracked {
            let id = item.summary.id;
            let offer_id = item.ebay_offer_id.clone().unwrap_or_default();
            let was = item.summary.ebay.as_ref().and_then(|e| e.status);
            match self
                .call(
                    Method::GET,
                    &format!("sell/inventory/v1/offer/{offer_id}"),
                    None,
                )
                .await
            {
                Ok(offer) => {
                    let listing = &offer["listing"];
                    let sold = listing["soldQuantity"].as_i64().unwrap_or(0) > 0;
                    let status = match (offer["status"].as_str(), listing["listingStatus"].as_str())
                    {
                        _ if sold => EbayStatus::Sold,
                        (Some("PUBLISHED"), Some("OUT_OF_STOCK")) => EbayStatus::Sold,
                        (
                            Some("PUBLISHED"),
                            Some("ENDED" | "EBAY_ENDED" | "INACTIVE" | "NOT_LISTED"),
                        ) => EbayStatus::Ended,
                        (Some("PUBLISHED"), _) => EbayStatus::Listed,
                        // Withdrawn: the listing ended.
                        _ if was == Some(EbayStatus::Listed) => EbayStatus::Ended,
                        _ => EbayStatus::Draft,
                    };
                    let _ =
                        self.runner
                            .db
                            .set_ebay_status(id, status, listing["listingId"].as_str());
                }
                Err(e) if e.status() == Some(404) => {
                    let _ = self.runner.db.clear_ebay(id);
                    let _ = self
                        .runner
                        .db
                        .set_ebay_item_error(id, Some("the offer was deleted on eBay"));
                }
                Err(e) if fatal(&e) => return Err(e),
                Err(e) => {
                    let _ = self.runner.db.set_ebay_item_error(id, Some(&e.to_string()));
                }
            }
            self.runner.emit_item(id);
            self.advance(1);
        }
        let _ = self.runner.db.set_ebay_checked(&self.session_id);
        Ok(())
    }

    async fn remove(&mut self, ids: Vec<i64>) -> Result<(), ApiError> {
        let cards = self.items(Some(&ids));
        self.start(cards.len());
        for item in cards {
            let id = item.summary.id;
            if item.summary.ebay.as_ref().and_then(|e| e.status) == Some(EbayStatus::Sold) {
                self.item_error(id, "a sold card stays on eBay");
                self.advance(1);
                continue;
            }
            let mut deletes = Vec::new();
            deletes.extend(
                item.ebay_offer_id
                    .map(|o| format!("sell/inventory/v1/offer/{o}")),
            );
            deletes.extend(
                item.ebay_sku
                    .map(|s| format!("sell/inventory/v1/inventory_item/{s}")),
            );
            let mut error = None;
            for path in deletes {
                match self.call(Method::DELETE, &path, None).await {
                    Ok(_) => {}
                    Err(e) if e.status() == Some(404) => {}
                    Err(e) if fatal(&e) => return Err(e),
                    Err(e) => {
                        error = Some(e.to_string());
                        break;
                    }
                }
            }
            match error {
                Some(e) => self.item_error(id, &e),
                None => {
                    let _ = self.runner.db.clear_ebay(id);
                    self.runner.emit_item(id);
                }
            }
            self.advance(1);
        }
        self.runner.emit_session(&self.session_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bulk_entries_report_their_own_errors() {
        assert_eq!(entry_error(&json!({"statusCode": 200, "sku": "a"})), None);
        assert_eq!(
            entry_error(
                &json!({"statusCode": 400, "errors": [{"errorId": 25005, "message": "Invalid category."}]})
            ),
            Some("Invalid category. (25005)".into())
        );
        assert_eq!(
            entry_error(&json!({"statusCode": 500})),
            Some("HTTP 500".into())
        );
        let exists = json!({"statusCode": 400, "errors": [{"errorId": 25002, "message": "Offer entity already exists",
            "parameters": [{"name": "offerId", "value": "9"}]}]});
        assert_eq!(existing_offer(&exists).as_deref(), Some("9"));
    }
}
