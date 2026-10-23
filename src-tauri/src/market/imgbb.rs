//! ImgBB (api.imgbb.com), a free public image host: the user's API key uploads a picture and gets a permanent
//! https URL that marketplaces' CSV imports can fetch.

use std::time::Duration;

use serde_json::Value;
use tokio_util::sync::CancellationToken;

use super::{pictures, read_picture};
use crate::model::ItemDetail;
use crate::runner::Runner;
use crate::settings::Secret;
use crate::ximilar::http::{retry, truncate, ApiError, Failure, USER_AGENT};

pub const DEFAULT_BASE: &str = "https://api.imgbb.com";

pub struct Imgbb {
    http: reqwest::Client,
    base: String,
    key: String,
}

impl Imgbb {
    /// `base`: empty for ImgBB itself.
    pub fn new(base: &str, key: &str) -> Result<Self, ApiError> {
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(|e| ApiError::Network(e.to_string()))?;
        let base = if base.trim().is_empty() {
            DEFAULT_BASE
        } else {
            base.trim()
        };
        Ok(Imgbb {
            http,
            base: base.trim_end_matches('/').to_string(),
            key: key.trim().to_string(),
        })
    }

    /// Uploads a JPEG; returns its public URL.
    pub async fn upload(
        &self,
        jpeg: &[u8],
        name: &str,
        cancel: &CancellationToken,
    ) -> Result<String, ApiError> {
        let url = format!("{}/1/upload", self.base);
        let image = crate::imaging::b64(jpeg);
        let body = retry("imgbb upload", cancel, || {
            let form = reqwest::multipart::Form::new()
                .text("image", image.clone())
                .text("name", name.to_string());
            let req = self
                .http
                .post(&url)
                .query(&[("key", self.key.as_str())])
                .multipart(form);
            async move {
                let resp = req
                    .send()
                    .await
                    .map_err(|e| ApiError::Network(e.to_string()))?;
                let status = resp.status();
                let text = resp
                    .text()
                    .await
                    .map_err(|e| ApiError::Network(e.to_string()))?;
                let body: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
                if !status.is_success() {
                    // {"status_code": 400, "error": {"message": "Invalid API v1 key.", "code": 100}}
                    let detail = body["error"]["message"]
                        .as_str()
                        .map(str::to_string)
                        .unwrap_or_else(|| truncate(&text, 200));
                    return Err(ApiError::Status {
                        status: status.as_u16(),
                        detail,
                    });
                }
                Ok(body)
            }
        })
        .await?;
        body["data"]["url"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| ApiError::Decode("ImgBB sent no picture URL".into()))
    }
}

/// One export's pictures as public URLs: the card's hosted ones, else uploaded now (and kept on the card).
pub struct Hosting {
    host: Option<Imgbb>,
    /// Why pictures stopped being uploaded; once set, none are.
    pub error: Option<String>,
}

impl Hosting {
    /// `wanted`: the user asked for pictures (an API key is needed too).
    pub fn new(runner: &Runner, wanted: bool) -> Result<Self, ApiError> {
        if !wanted {
            return Ok(Hosting {
                host: None,
                error: None,
            });
        }
        match runner.settings.secret(Secret::ImgbbKey) {
            Some(key) => Ok(Hosting {
                host: Some(Imgbb::new(&runner.settings.get().image_host_base, &key)?),
                error: None,
            }),
            None => Ok(Hosting {
                host: None,
                error: Some("no ImgBB API key — add it in Settings".into()),
            }),
        }
    }

    /// The card's picture URLs; empty when it has none. Fails only when cancelled.
    pub async fn urls(
        &mut self,
        runner: &Runner,
        item: &ItemDetail,
        cancel: &CancellationToken,
    ) -> Result<Vec<String>, ApiError> {
        if !item.hosted_images.is_empty() {
            return Ok(item.hosted_images.clone());
        }
        let Some(host) = self.host.as_ref().filter(|_| self.error.is_none()) else {
            return Ok(Vec::new());
        };
        match upload(host, item, cancel).await {
            Ok(urls) => {
                let _ = runner.db.set_hosted_images(item.summary.id, &urls);
                Ok(urls)
            }
            Err(ApiError::Cancelled) => Err(ApiError::Cancelled),
            // A bad key or an unreachable host fails every picture: stop asking.
            Err(e)
                if matches!(e.failure(), Failure::Auth | Failure::Retry)
                    || e.status() == Some(400) =>
            {
                self.error = Some(format!("pictures not uploaded: {e}"));
                Ok(Vec::new())
            }
            Err(e) => {
                tracing::warn!("pictures of card #{} not uploaded: {e}", item.summary.seq);
                Ok(Vec::new())
            }
        }
    }
}

async fn upload(
    host: &Imgbb,
    item: &ItemDetail,
    cancel: &CancellationToken,
) -> Result<Vec<String>, ApiError> {
    let mut urls = Vec::new();
    for (side, path) in pictures(item).into_iter().enumerate() {
        let jpeg = read_picture(path).await?;
        let name = format!(
            "card-{}-{}",
            item.summary.seq,
            if side == 0 { "front" } else { "back" }
        );
        urls.push(host.upload(&jpeg, &name, cancel).await?);
    }
    Ok(urls)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn uploads_with_the_key_and_explains_errors() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/1/upload"))
            .and(query_param("key", "good"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!(
                {"data": {"url": "https://i.ibb.co/abc/card.jpg"}, "success": true, "status": 200}
            )))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/1/upload"))
            .and(query_param("key", "bad"))
            .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!(
                {"status_code": 400, "error": {"message": "Invalid API v1 key.", "code": 100}}
            )))
            .mount(&server)
            .await;
        let cancel = CancellationToken::new();
        let good = Imgbb::new(&server.uri(), "good").unwrap();
        assert_eq!(
            good.upload(b"jpeg", "card", &cancel).await.unwrap(),
            "https://i.ibb.co/abc/card.jpg"
        );
        let bad = Imgbb::new(&server.uri(), "bad").unwrap();
        assert_eq!(
            bad.upload(b"jpeg", "card", &cancel)
                .await
                .unwrap_err()
                .to_string(),
            "HTTP 400: Invalid API v1 key."
        );
        let requests = server.received_requests().await.unwrap();
        let body = String::from_utf8_lossy(&requests[0].body);
        assert!(
            body.contains(&crate::imaging::b64(b"jpeg")),
            "the picture goes as base64"
        );
    }
}
