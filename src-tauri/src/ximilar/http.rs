//! The one HTTP client every Ximilar call goes through: auth, user agent, error classification, retries.
//! The error classes and retries also serve the eBay client (`crate::ebay::client`).

use std::future::Future;
use std::time::Duration;

use reqwest::{Method, StatusCode};
use serde_json::Value;
use tokio_util::sync::CancellationToken;

pub const USER_AGENT: &str = concat!("cardloader/", env!("CARGO_PKG_VERSION"));
const MAX_ATTEMPTS: u32 = 5;

#[derive(Debug, Clone, thiserror::Error)]
pub enum ApiError {
    #[error("connection error: {0}")]
    Network(String),
    #[error("HTTP {status}: {detail}")]
    Status { status: u16, detail: String },
    #[error("unexpected response: {0}")]
    Decode(String),
    #[error("cancelled")]
    Cancelled,
    #[error("no Ximilar API token — set it in Settings")]
    NoToken,
    #[error("eBay is not connected — connect it in Settings")]
    NotConnected,
}

/// What a failure means for the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    /// Network, 429, 5xx: try again later.
    Retry,
    /// 401/402/403: nothing will pass with this token — stop.
    Auth,
    /// Anything else: this request will never pass.
    Reject,
}

impl ApiError {
    pub fn failure(&self) -> Failure {
        match self {
            ApiError::Network(_) => Failure::Retry,
            ApiError::Status { status, .. } => match *status {
                429 | 500..=599 => Failure::Retry,
                401..=403 => Failure::Auth,
                _ => Failure::Reject,
            },
            ApiError::NoToken | ApiError::NotConnected => Failure::Auth,
            ApiError::Decode(_) | ApiError::Cancelled => Failure::Reject,
        }
    }

    pub fn status(&self) -> Option<u16> {
        match self {
            ApiError::Status { status, .. } => Some(*status),
            _ => None,
        }
    }
}

#[derive(Clone)]
pub struct XimilarClient {
    http: reqwest::Client,
    base: String,
    token: String,
    workspace: Option<String>,
}

impl XimilarClient {
    pub fn new(base: &str, token: &str, workspace: Option<String>) -> Result<Self, ApiError> {
        if token.trim().is_empty() {
            return Err(ApiError::NoToken);
        }
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(|e| ApiError::Network(e.to_string()))?;
        Ok(XimilarClient {
            http,
            base: base.trim_end_matches('/').to_string(),
            token: token.trim().to_string(),
            workspace,
        })
    }

    pub fn workspace(&self) -> Option<&str> {
        self.workspace.as_deref()
    }

    pub fn url(&self, path: &str) -> String {
        format!("{}/{}", self.base, path.trim_start_matches('/'))
    }

    /// One request, no retries. `path` is relative to the API base unless it is an absolute URL.
    pub async fn request(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, String)],
        body: Option<&Value>,
    ) -> Result<Value, ApiError> {
        let url = if path.starts_with("http") {
            path.to_string()
        } else {
            self.url(path)
        };
        let mut req = self
            .http
            .request(method, url)
            .header("Authorization", format!("Token {}", self.token))
            .query(query);
        if let Some(body) = body {
            req = req.json(body);
        }
        let resp = req
            .send()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))?;
        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))?;
        if !status.is_success() {
            return Err(ApiError::Status {
                status: status.as_u16(),
                detail: detail(status, &text),
            });
        }
        if text.trim().is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_str(&text)
            .map_err(|e| ApiError::Decode(format!("{e}: {}", truncate(&text, 200))))
    }

    /// `request`, retried with exponential backoff while the failure is transient. Stops early on cancel.
    pub async fn request_retry(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, String)],
        body: Option<&Value>,
        cancel: &CancellationToken,
    ) -> Result<Value, ApiError> {
        retry(path, cancel, || {
            self.request(method.clone(), path, query, body)
        })
        .await
    }

    /// Downloads a (public, temporary) result file, e.g. a grading overlay.
    pub async fn download(&self, url: &str) -> Result<Vec<u8>, ApiError> {
        let resp = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))?;
        let status = resp.status();
        if !status.is_success() {
            return Err(ApiError::Status {
                status: status.as_u16(),
                detail: "download failed".into(),
            });
        }
        Ok(resp
            .bytes()
            .await
            .map_err(|e| ApiError::Network(e.to_string()))?
            .to_vec())
    }
}

/// `call`, retried with exponential backoff while the failure is transient. Stops early on cancel.
pub async fn retry<T, F, Fut>(
    label: &str,
    cancel: &CancellationToken,
    mut call: F,
) -> Result<T, ApiError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, ApiError>>,
{
    let mut delay = Duration::from_secs(1);
    let mut attempt = 1;
    loop {
        let result = tokio::select! {
            r = call() => r,
            _ = cancel.cancelled() => return Err(ApiError::Cancelled),
        };
        match result {
            Err(e) if e.failure() == Failure::Retry && attempt < MAX_ATTEMPTS => {
                tracing::warn!("{label}: {e} — retry {attempt}/{MAX_ATTEMPTS} in {delay:?}");
                tokio::select! {
                    _ = tokio::time::sleep(delay) => {},
                    _ = cancel.cancelled() => return Err(ApiError::Cancelled),
                }
                delay = (delay * 2).min(Duration::from_secs(30));
                attempt += 1;
            }
            other => return other,
        }
    }
}

/// The messages of an `errors` list as eBay sends it (`[{errorId, message, longMessage}]`), if any.
pub fn errors_text(errors: &Value) -> Option<String> {
    let messages: Vec<String> = errors
        .as_array()?
        .iter()
        .filter_map(|e| {
            let text = e["longMessage"].as_str().or(e["message"].as_str())?;
            Some(match e["errorId"].as_i64() {
                Some(id) => format!("{text} ({id})"),
                None => text.to_string(),
            })
        })
        .collect();
    (!messages.is_empty()).then(|| messages.join("; "))
}

/// The API's own explanation when it gave one, else the start of the body.
pub(crate) fn detail(status: StatusCode, text: &str) -> String {
    if let Ok(Value::Object(body)) = serde_json::from_str::<Value>(text) {
        if let Some(Value::String(d)) = body.get("detail") {
            return d.clone();
        }
        if let Some(errors) = body.get("errors").and_then(errors_text) {
            return errors;
        }
        // OAuth errors (eBay's token endpoint).
        if let Some(Value::String(d)) = body.get("error_description") {
            return d.clone();
        }
        if let Some(Value::Object(s)) = body.get("status") {
            if let Some(Value::String(t)) = s.get("text") {
                return t.clone();
            }
        }
        return truncate(&Value::Object(body).to_string(), 300);
    }
    if text.trim().is_empty() {
        status.canonical_reason().unwrap_or("error").to_string()
    } else {
        truncate(text, 200)
    }
}

pub(crate) fn truncate(text: &str, n: usize) -> String {
    text.chars().take(n).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn failures_are_classified() {
        let s = |status| ApiError::Status {
            status,
            detail: String::new(),
        };
        assert_eq!(s(429).failure(), Failure::Retry);
        assert_eq!(s(503).failure(), Failure::Retry);
        assert_eq!(s(401).failure(), Failure::Auth);
        assert_eq!(s(402).failure(), Failure::Auth);
        assert_eq!(s(400).failure(), Failure::Reject);
        assert_eq!(s(404).failure(), Failure::Reject);
        assert_eq!(ApiError::Network("x".into()).failure(), Failure::Retry);
    }

    #[tokio::test]
    async fn sends_token_and_reports_detail() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/collectibles/v2/tcg_id"))
            .and(header("Authorization", "Token secret"))
            .respond_with(
                ResponseTemplate::new(400)
                    .set_body_json(serde_json::json!({"detail": "bad records"})),
            )
            .mount(&server)
            .await;
        let client = XimilarClient::new(&server.uri(), "secret", None).unwrap();
        let err = client
            .request(
                Method::POST,
                "collectibles/v2/tcg_id",
                &[],
                Some(&serde_json::json!({})),
            )
            .await
            .unwrap_err();
        assert_eq!(err.to_string(), "HTTP 400: bad records");
    }

    #[test]
    fn ebay_and_oauth_errors_are_explained() {
        let ebay = r#"{"errors": [{"errorId": 25002, "message": "A user error", "longMessage": "Offer entity already exists."},
            {"message": "Second"}]}"#;
        assert_eq!(
            detail(StatusCode::BAD_REQUEST, ebay),
            "Offer entity already exists. (25002); Second"
        );
        let oauth = r#"{"error": "invalid_grant", "error_description": "the provided authorization grant is invalid"}"#;
        assert_eq!(
            detail(StatusCode::BAD_REQUEST, oauth),
            "the provided authorization grant is invalid"
        );
    }

    #[tokio::test]
    async fn retries_transient_failures() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(503))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok": true})))
            .mount(&server)
            .await;
        let client = XimilarClient::new(&server.uri(), "t", None).unwrap();
        let value = client
            .request_retry(Method::GET, "ping/", &[], None, &CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(value["ok"], true);
    }
}
