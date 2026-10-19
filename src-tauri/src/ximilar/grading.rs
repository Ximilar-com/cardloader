//! Card grading through the async request API (`account/v2/request/`, type `card-grader`).

use reqwest::Method;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use super::http::{ApiError, XimilarClient};
use crate::model::GradingMode;

const REQUEST_PATH: &str = "account/v2/request/";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobStatus {
    Pending,
    Done,
    Failed(String),
}

/// Records for one card: front (+ back for `grade`/`centering`, which judge both sides together).
pub fn build_request(
    mode: GradingMode,
    condition_mode: &str,
    front_b64: &str,
    back_b64: Option<&str>,
) -> Value {
    let endpoint = mode.as_str();
    let mut records = vec![json!({"_id": "front", "_base64": front_b64, "Side": "Front"})];
    if matches!(mode, GradingMode::Grade | GradingMode::Centering) {
        if let Some(back) = back_b64 {
            records.push(json!({"_id": "back", "_base64": back, "Side": "Back"}));
        }
    }
    let mut body = json!({"type": "card-grader", "endpoint": endpoint, "records": records});
    if mode == GradingMode::Condition {
        body["mode"] = json!(condition_mode);
    }
    body
}

pub async fn submit(
    client: &XimilarClient,
    body: &Value,
    cancel: &CancellationToken,
) -> Result<String, ApiError> {
    let resp = client
        .request_retry(Method::POST, REQUEST_PATH, &[], Some(body), cancel)
        .await?;
    resp["id"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| ApiError::Decode(format!("grading request has no id: {resp}")))
}

pub async fn status(
    client: &XimilarClient,
    job_id: &str,
    cancel: &CancellationToken,
) -> Result<JobStatus, ApiError> {
    let resp = client
        .request_retry(
            Method::GET,
            &format!("{REQUEST_PATH}{job_id}/status"),
            &[],
            None,
            cancel,
        )
        .await?;
    Ok(parse_status(&resp))
}

pub fn parse_status(resp: &Value) -> JobStatus {
    match resp["status"]
        .as_str()
        .unwrap_or_default()
        .to_ascii_uppercase()
        .as_str()
    {
        "DONE" => JobStatus::Done,
        "FAILED" | "ERROR" => JobStatus::Failed(
            resp["error"]
                .as_str()
                .or(resp["response"]["status"]["text"].as_str())
                .unwrap_or("grading failed")
                .into(),
        ),
        _ => JobStatus::Pending,
    }
}

/// The finished job's `response`.
pub async fn result(
    client: &XimilarClient,
    job_id: &str,
    cancel: &CancellationToken,
) -> Result<Value, ApiError> {
    let resp = client
        .request_retry(
            Method::GET,
            &format!("{REQUEST_PATH}{job_id}"),
            &[],
            None,
            cancel,
        )
        .await?;
    let mut response = resp["response"].clone();
    if let Some(records) = response["records"].as_array_mut() {
        for r in records {
            if let Some(obj) = r.as_object_mut() {
                obj.remove("_base64");
            }
        }
    }
    Ok(response)
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct GradingSummary {
    /// `grades.final` (grade), or `grades.centering` (centering only).
    pub grade: Option<f64>,
    /// `grades.condition` (grade) or the condition label (condition).
    pub condition: Option<String>,
    /// Result images on temporary storage, to download: (record index, key, url).
    pub images: Vec<(usize, String, String)>,
    /// A record-level error, if the job ran but a record failed.
    pub error: Option<String>,
}

const IMAGE_KEYS: &[&str] = &[
    "_full_url_card",
    "_exact_url_card",
    "_clean_url_card",
    "_img_url",
];

pub fn summarize(mode: GradingMode, response: &Value) -> GradingSummary {
    let records = response["records"].as_array().cloned().unwrap_or_default();
    let mut out = GradingSummary::default();
    for (i, r) in records.iter().enumerate() {
        for key in IMAGE_KEYS {
            if let Some(url) = r[*key].as_str() {
                out.images
                    .push((i, key.trim_start_matches('_').to_string(), url.to_string()));
            }
        }
        if let Some(code) = r["_status"]["code"].as_u64() {
            if code >= 300 && out.error.is_none() {
                out.error = r["_status"]["text"]
                    .as_str()
                    .map(str::to_owned)
                    .or(Some(format!("HTTP {code}")));
            }
        }
    }
    let first_grades = records.iter().map(|r| &r["grades"]).find(|g| g.is_object());
    let grades = if response["grades"].is_object() {
        Some(&response["grades"])
    } else {
        first_grades
    };
    match mode {
        GradingMode::Grade => {
            out.grade = grades.and_then(|g| g["final"].as_f64());
            out.condition = grades
                .and_then(|g| g["condition"].as_str())
                .map(str::to_owned);
        }
        GradingMode::Centering => {
            out.grade = grades.and_then(|g| g["centering"].as_f64());
        }
        GradingMode::Condition => {
            let cond = records
                .first()
                .and_then(|r| r["_objects"].as_array())
                .and_then(|objs| {
                    objs.iter()
                        .find_map(|o| o["Condition"].as_array().and_then(|c| c.first()))
                });
            if let Some(c) = cond {
                out.condition = c["label"].as_str().map(str::to_owned);
            }
        }
        GradingMode::Off => {}
    }
    out.grade = out.grade.filter(|g| g.is_finite());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grade_request_sends_both_sides() {
        let body = build_request(GradingMode::Grade, "ebay", "F", Some("B"));
        assert_eq!(body["type"], "card-grader");
        assert_eq!(body["endpoint"], "grade");
        assert_eq!(body["records"].as_array().unwrap().len(), 2);
        assert_eq!(body["records"][1]["Side"], "Back");
        let cond = build_request(GradingMode::Condition, "tcgplayer", "F", Some("B"));
        assert_eq!(cond["records"].as_array().unwrap().len(), 1);
        assert_eq!(cond["mode"], "tcgplayer");
    }

    #[test]
    fn summarizes_grade() {
        let response = json!({"records": [
            {"grades": {"corners": 8.0, "edges": 8.0, "surface": 6.0, "centering": 9.0, "final": 7.5, "condition": "Near Mint"},
             "_full_url_card": "https://x/full.webp", "_exact_url_card": "https://x/exact.webp"},
            {"grades": {"final": 6.0}}]});
        let s = summarize(GradingMode::Grade, &response);
        assert_eq!(s.grade, Some(7.5));
        assert_eq!(s.condition.as_deref(), Some("Near Mint"));
        assert_eq!(s.images.len(), 2);
        assert_eq!(s.images[0].1, "full_url_card");
    }

    #[test]
    fn summarizes_condition_and_centering() {
        let response = json!({"records": [{"_objects": [{"name": "Card",
            "Condition": [{"name": "Card Condition Exact", "value": 6.0, "label": "Excellent", "mode": "ebay"}]}]}]});
        assert_eq!(
            summarize(GradingMode::Condition, &response)
                .condition
                .as_deref(),
            Some("Excellent")
        );
        let centering = json!({"records": [{"grades": {"centering": 9.0}, "_clean_url_card": "https://x/c.webp"}]});
        let s = summarize(GradingMode::Centering, &centering);
        assert_eq!(s.grade, Some(9.0));
        assert_eq!(s.images.len(), 1);
    }

    #[test]
    fn status_parsing() {
        assert_eq!(parse_status(&json!({"status": "DONE"})), JobStatus::Done);
        assert_eq!(
            parse_status(&json!({"status": "PROCESSING"})),
            JobStatus::Pending
        );
        assert!(matches!(
            parse_status(&json!({"status": "FAILED"})),
            JobStatus::Failed(_)
        ));
    }
}
