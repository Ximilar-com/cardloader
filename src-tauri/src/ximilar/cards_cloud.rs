//! Ximilar Cards Cloud (`/cards-cloud/v2/`): the remote copy of a local session.
//!
//! Rows are kept within the API's field limits (one bad field rejects a whole batch, so the payload is
//! made safe here). Failures: network, 429 and 5xx are retried; 401/402/403/404/409 stop syncing the
//! session; anything else drops the batch.

use reqwest::Method;
use serde_json::{json, Map, Value};

use super::http::{ApiError, XimilarClient};
use super::recognition::{self, Endpoint};
use crate::model::ItemDetail;

const BASE: &str = "cards-cloud/v2";
pub const MAX_BATCH: usize = 500;
pub const MAX_SESSION_NAME: usize = 255;
const MAX_LIST: usize = 10;
const MAX_LINK: usize = 2048;
/// The API refuses more than 64 KiB of grading per row; a trimmed copy goes instead.
const MAX_GRADING_BYTES: usize = 60 * 1024;
const TEXT_LIMITS: &[(&str, usize)] = &[
    ("name", 255),
    ("full_name", 500),
    ("set_name", 255),
    ("set_code", 50),
    ("card_number", 50),
    ("series", 255),
    ("year", 20),
    ("subcategory", 100),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Stored,
    Retry(String),
    Rejected(String),
    Stopped(String),
}

pub fn outcome<T>(result: &Result<T, ApiError>) -> Outcome {
    match result {
        Ok(_) => Outcome::Stored,
        Err(e) => match e {
            ApiError::Network(_) => Outcome::Retry(e.to_string()),
            ApiError::Status { status, .. } => match status {
                429 | 500..=599 => Outcome::Retry(e.to_string()),
                401..=404 | 409 => Outcome::Stopped(e.to_string()),
                _ => Outcome::Rejected(e.to_string()),
            },
            ApiError::NoToken => Outcome::Stopped(e.to_string()),
            _ => Outcome::Rejected(e.to_string()),
        },
    }
}

fn clip(text: &str, limit: usize) -> String {
    text.chars().take(limit).collect()
}

/// One local item as an upload row.
pub fn session_item(item: &ItemDetail, include_grading: bool) -> Value {
    let s = &item.summary;
    let card = &s.card;
    let fields = [
        &card.name,
        &card.full_name,
        &card.set_name,
        &card.set_code,
        &card.card_number,
        &card.series,
        &card.year,
        &card.subcategory,
    ];
    let mut out = Map::new();
    out.insert("event_id".into(), json!(item.event_id));
    out.insert(
        "seen".into(),
        json!(item
            .analyzed_at
            .clone()
            .unwrap_or_else(|| s.updated_at.clone())),
    );
    out.insert("calls".into(), json!(item.calls.max(0)));
    let id_type = s
        .id_type
        .as_deref()
        .filter(|t| ["tcg", "sport", "slab", "comics"].contains(t))
        .unwrap_or("tcg");
    out.insert("id_type".into(), json!(id_type));
    for ((key, limit), value) in TEXT_LIMITS.iter().zip(fields) {
        out.insert((*key).into(), json!(clip(value, *limit)));
    }
    if let Some(d) = s.distance.filter(|d| d.is_finite() && *d >= 0.0) {
        out.insert("distance".into(), json!(d));
    }
    if let Some(c) = s
        .confidence
        .as_deref()
        .filter(|c| ["high", "medium", "low"].contains(c))
    {
        out.insert("confidence".into(), json!(c));
    }

    // links, alternatives and price stats are re-read from the stored response.
    if let Some(rec) = &item.recognition {
        let endpoint = Endpoint::of_raw(rec);
        let parsed = recognition::parse(endpoint, &rec["response"]);
        let links: Map<String, Value> = parsed
            .links
            .into_iter()
            .filter_map(|(k, v)| v.as_str().map(|u| (k, json!(clip(u, MAX_LINK)))))
            .collect();
        out.insert("links".into(), Value::Object(links));
        let alternatives: Vec<Value> = parsed
            .alternatives
            .into_iter()
            .take(MAX_LIST)
            .map(slim_alternative)
            .collect();
        out.insert("alternatives".into(), json!(alternatives));
        out.insert(
            "price_stats".into(),
            json!(parsed
                .price_stats
                .into_iter()
                .take(MAX_LIST)
                .collect::<Vec<_>>()),
        );
    }

    if include_grading {
        if let Some(grading) = upload_grading(item) {
            out.insert("grading".into(), grading);
        }
    }
    Value::Object(out)
}

/// A picked match as a `PATCH identification/{id}/` body: the card fields and price, which Cards Cloud
/// then keeps as the user's correction.
pub fn pick_patch(item: &ItemDetail) -> Value {
    let card = &item.summary.card;
    let fields = [
        &card.name,
        &card.full_name,
        &card.set_name,
        &card.set_code,
        &card.card_number,
        &card.series,
        &card.year,
        &card.subcategory,
    ];
    let mut out = Map::new();
    for ((key, limit), value) in TEXT_LIMITS.iter().zip(fields) {
        out.insert((*key).into(), json!(clip(value, *limit)));
    }
    out.insert(
        "price".into(),
        json!(item.summary.price.filter(|p| p.is_finite() && *p >= 0.0)),
    );
    Value::Object(out)
}

/// An alternative without its (possibly large) price stats.
fn slim_alternative(alt: Value) -> Value {
    match alt {
        Value::Object(mut map) => {
            map.remove("price_stats");
            Value::Object(map)
        }
        other => other,
    }
}

/// The grading as Cards Cloud stores it: `grades` on top (the server derives `grade` from `grades.final`),
/// then the endpoint's records — or only the grades if the whole thing is too large.
fn upload_grading(item: &ItemDetail) -> Option<Value> {
    let grading = item.grading.as_ref()?;
    let records = grading["response"]["records"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let mut grades = records
        .iter()
        .map(|r| r["grades"].clone())
        .find(Value::is_object)
        .unwrap_or_else(|| json!({}));
    if let Some(c) = &item.summary.condition {
        if grades.get("condition").is_none() {
            grades["condition"] = json!(c);
        }
    }
    let full = json!({"endpoint": grading["endpoint"], "grades": grades, "records": records});
    if serde_json::to_vec(&full)
        .map(|b| b.len())
        .unwrap_or(usize::MAX)
        <= MAX_GRADING_BYTES
    {
        Some(full)
    } else {
        Some(json!({"endpoint": grading["endpoint"], "grades": full["grades"]}))
    }
}

#[derive(Clone)]
pub struct CardsCloudApi {
    client: XimilarClient,
}

impl CardsCloudApi {
    pub fn new(client: XimilarClient) -> Self {
        CardsCloudApi { client }
    }

    fn query(&self, extra: &[(&str, String)]) -> Vec<(&'static str, String)> {
        let mut q: Vec<(&'static str, String)> = Vec::new();
        if let Some(ws) = self.client.workspace() {
            q.push(("workspace", ws.to_string()));
        }
        for (k, v) in extra {
            let key: &'static str = match *k {
                "page" => "page",
                "session" => "session",
                "page_size" => "page_size",
                "ordering" => "ordering",
                "event_id" => "event_id",
                "search" => "search",
                _ => continue,
            };
            q.push((key, v.clone()));
        }
        q
    }

    async fn call(
        &self,
        method: Method,
        path: &str,
        extra: &[(&str, String)],
        body: Option<&Value>,
    ) -> Result<Value, ApiError> {
        let path = format!("{BASE}/{path}");
        self.client
            .request(method, &path, &self.query(extra), body)
            .await
    }

    pub async fn ping(&self) -> Result<Value, ApiError> {
        self.call(Method::GET, "ping/", &[], None).await
    }

    pub async fn create(&self, payload: &Value) -> Result<Value, ApiError> {
        self.call(Method::POST, "session/", &[], Some(payload))
            .await
    }

    pub async fn get(&self, id: &str) -> Result<Value, ApiError> {
        self.call(Method::GET, &format!("session/{id}/"), &[], None)
            .await
    }

    pub async fn reopen(&self, id: &str) -> Result<Value, ApiError> {
        self.call(Method::POST, &format!("session/{id}/reopen/"), &[], None)
            .await
    }

    pub async fn close(&self, id: &str) -> Result<Value, ApiError> {
        self.call(Method::POST, &format!("session/{id}/close/"), &[], None)
            .await
    }

    pub async fn upload(&self, id: &str, items: &[Value]) -> Result<Value, ApiError> {
        self.call(
            Method::POST,
            &format!("session/{id}/identifications/"),
            &[],
            Some(&json!({"identifications": items})),
        )
        .await
    }

    pub async fn upload_image(
        &self,
        id: &str,
        event_id: &str,
        image_b64: &str,
    ) -> Result<Value, ApiError> {
        self.call(
            Method::POST,
            &format!("session/{id}/images/"),
            &[],
            Some(&json!({"event_id": event_id, "image": image_b64})),
        )
        .await
    }

    pub async fn list_sessions(&self, page: u32, search: &str) -> Result<Value, ApiError> {
        let mut query = vec![("page", page.to_string())];
        if !search.trim().is_empty() {
            query.push(("search", search.trim().to_string()));
        }
        self.call(Method::GET, "session/", &query, None).await
    }

    pub async fn rename(&self, id: &str, name: &str) -> Result<Value, ApiError> {
        self.call(
            Method::PATCH,
            &format!("session/{id}/"),
            &[],
            Some(&json!({"name": clip(name, MAX_SESSION_NAME)})),
        )
        .await
    }

    /// Deletes the remote session with its rows and images.
    pub async fn delete(&self, id: &str) -> Result<Value, ApiError> {
        self.call(Method::DELETE, &format!("session/{id}/"), &[], None)
            .await
    }

    pub async fn summary(&self, id: &str) -> Result<Value, ApiError> {
        self.call(Method::GET, &format!("session/{id}/summary/"), &[], None)
            .await
    }

    /// The id of the row with this `event_id`. Servers without the `event_id` filter return the whole
    /// session, so the rows are matched here and the pages walked.
    pub async fn find_identification(
        &self,
        session: &str,
        event_id: &str,
    ) -> Result<Option<String>, ApiError> {
        for page in 1..=200 {
            let resp = self
                .call(
                    Method::GET,
                    "identification/",
                    &[
                        ("session", session.to_string()),
                        ("event_id", event_id.to_string()),
                        ("page", page.to_string()),
                        ("page_size", "100".into()),
                    ],
                    None,
                )
                .await?;
            let found = resp["results"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|r| r["event_id"] == event_id)
                .and_then(|r| r["id"].as_str());
            if let Some(id) = found {
                return Ok(Some(id.to_string()));
            }
            if resp["next"].is_null() {
                break;
            }
        }
        Ok(None)
    }

    pub async fn patch_identification(&self, id: &str, body: &Value) -> Result<Value, ApiError> {
        self.call(
            Method::PATCH,
            &format!("identification/{id}/"),
            &[],
            Some(body),
        )
        .await
    }

    /// The `event_id` of every row of a session.
    pub async fn event_ids(
        &self,
        session: &str,
    ) -> Result<std::collections::HashSet<String>, ApiError> {
        let mut ids = std::collections::HashSet::new();
        for page in 1.. {
            let resp = self
                .call(
                    Method::GET,
                    "identification/",
                    &[
                        ("session", session.to_string()),
                        ("page", page.to_string()),
                        ("page_size", "500".into()),
                    ],
                    None,
                )
                .await?;
            let rows = resp["results"].as_array().cloned().unwrap_or_default();
            ids.extend(
                rows.iter()
                    .filter_map(|r| r["event_id"].as_str().map(String::from)),
            );
            if rows.is_empty() || resp["next"].is_null() {
                break;
            }
        }
        Ok(ids)
    }

    pub async fn list_identifications(&self, session: &str, page: u32) -> Result<Value, ApiError> {
        self.call(
            Method::GET,
            "identification/",
            &[
                ("session", session.to_string()),
                ("page", page.to_string()),
                ("page_size", "100".into()),
            ],
            None,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CardFields, ItemStatus, ItemSummary};

    fn item() -> ItemDetail {
        ItemDetail {
            summary: ItemSummary {
                id: 1,
                session_id: "s".into(),
                seq: 1,
                status: ItemStatus::Done,
                front_path: "/a.jpg".into(),
                back_path: None,
                thumb_path: None,
                id_type: Some("weird".into()),
                card: CardFields {
                    name: "x".repeat(300),
                    year: "2000".into(),
                    ..Default::default()
                },
                confidence: Some("certain".into()),
                distance: Some(f64::NAN),
                price: None,
                grade: Some(7.5),
                condition: Some("Near Mint".into()),
                slab: None,
                error: None,
                synced: false,
                updated_at: "2026-01-01T00:00:00Z".into(),
                picked: false,
                ebay: None,
            },
            event_id: "abc".into(),
            preview_path: None,
            back_preview_path: None,
            recognition: Some(
                json!({"endpoint": "collectibles/v2/tcg_id", "response": {"records": [{"_objects": [
                {"name": "Card", "_identification": {"best_match": {"name": "x",
                    "links": {"ebay.com": "https://e/".to_string() + &"a".repeat(3000)},
                    "price_stats": [{"stats_type": "ungraded", "value": {"median": 1.0}}]},
                 "alternatives": (0..15).map(|i| json!({"name": i, "price_stats": []})).collect::<Vec<_>>(),
                 "distances": [0.2]}}]}]}}),
            ),
            grading: Some(
                json!({"endpoint": "grade", "response": {"records": [{"grades": {"final": 7.5}}]}}),
            ),
            grading_images: vec![],
            calls: 3,
            analyzed_at: Some("2026-01-01T00:00:01Z".into()),
            remote_id: None,
            ebay_sku: None,
            ebay_offer_id: None,
            ebay_images: vec![],
            hosted_images: vec![],
        }
    }

    #[tokio::test]
    async fn rename_delete_and_search_sessions() {
        use wiremock::matchers::{body_json, method, path, query_param};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let server = MockServer::start().await;
        Mock::given(method("PATCH"))
            .and(path("/cards-cloud/v2/session/r1/"))
            .and(body_json(json!({"name": "x".repeat(MAX_SESSION_NAME)})))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "r1"})))
            .mount(&server)
            .await;
        Mock::given(method("DELETE"))
            .and(path("/cards-cloud/v2/session/r1/"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/cards-cloud/v2/session/"))
            .and(query_param("search", "binder"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({"count": 0, "results": []})),
            )
            .mount(&server)
            .await;
        let api = CardsCloudApi::new(XimilarClient::new(&server.uri(), "t", None).unwrap());
        api.rename("r1", &"x".repeat(300)).await.unwrap();
        assert_eq!(api.delete("r1").await.unwrap(), Value::Null);
        assert_eq!(api.list_sessions(1, " binder ").await.unwrap()["count"], 0);
    }

    #[test]
    fn pick_patch_has_the_card_fields_and_price() {
        let mut item = item();
        item.summary.price = Some(12.5);
        let body = pick_patch(&item);
        assert_eq!(body["name"].as_str().unwrap().len(), 255);
        assert_eq!(body["year"], "2000");
        assert_eq!(body["price"], 12.5);
        item.summary.price = Some(f64::NAN);
        assert!(pick_patch(&item)["price"].is_null());
        assert!(body.get("distance").is_none());
    }

    #[test]
    fn rows_are_made_safe() {
        let row = session_item(&item(), true);
        assert_eq!(row["name"].as_str().unwrap().len(), 255);
        assert_eq!(row["id_type"], "tcg");
        assert!(row.get("confidence").is_none());
        assert!(row.get("distance").is_none());
        assert_eq!(row["links"]["ebay.com"].as_str().unwrap().len(), MAX_LINK);
        assert_eq!(row["alternatives"].as_array().unwrap().len(), MAX_LIST);
        assert!(row["alternatives"][0].get("price_stats").is_none());
        assert_eq!(row["price_stats"].as_array().unwrap().len(), 1);
        assert_eq!(row["grading"]["grades"]["final"], 7.5);
        assert_eq!(row["grading"]["grades"]["condition"], "Near Mint");
        assert_eq!(row["seen"], "2026-01-01T00:00:01Z");
        assert!(session_item(&item(), false).get("grading").is_none());
    }

    #[test]
    fn failures_map_to_outcomes() {
        let st = |status| -> Result<(), ApiError> {
            Err(ApiError::Status {
                status,
                detail: String::new(),
            })
        };
        assert!(matches!(outcome(&st(503)), Outcome::Retry(_)));
        assert!(matches!(outcome(&st(409)), Outcome::Stopped(_)));
        assert!(matches!(outcome(&st(402)), Outcome::Stopped(_)));
        assert!(matches!(outcome(&st(400)), Outcome::Rejected(_)));
        assert_eq!(outcome::<()>(&Ok(())), Outcome::Stored);
    }
}
