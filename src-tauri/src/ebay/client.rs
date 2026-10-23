//! Calls to eBay's REST APIs for the connected seller: bearer token, marketplace and language headers, and
//! eBay's error format (`errors[]`). Failures are classified like Ximilar's (`ApiError::failure`).

use std::time::Duration;

use reqwest::Method;
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use super::listing::Marketplace;
use super::Hosts;
use crate::settings::EbaySettings;
use crate::ximilar::http::{detail, retry, truncate, ApiError, USER_AGENT};

pub fn http() -> Result<reqwest::Client, ApiError> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| ApiError::Network(e.to_string()))
}

#[derive(Clone)]
pub struct EbayClient {
    http: reqwest::Client,
    hosts: Hosts,
    token: String,
    market: &'static Marketplace,
}

impl EbayClient {
    pub fn new(cfg: &EbaySettings, token: &str) -> Result<Self, ApiError> {
        Ok(EbayClient {
            http: http()?,
            hosts: Hosts::of(cfg),
            token: token.to_string(),
            market: Marketplace::of(&cfg.marketplace),
        })
    }

    pub fn hosts(&self) -> &Hosts {
        &self.hosts
    }

    pub fn marketplace(&self) -> &'static str {
        self.market.id
    }

    /// One request, no retries. `path` is relative to the API host unless it is an absolute URL.
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
            format!("{}/{}", self.hosts.api, path.trim_start_matches('/'))
        };
        let mut req = self
            .http
            .request(method, url)
            .bearer_auth(&self.token)
            .header("X-EBAY-C-MARKETPLACE-ID", self.market.id)
            .header("Accept-Language", self.market.language)
            .query(query);
        if let Some(body) = body {
            req = req
                .header("Content-Language", self.market.language)
                .json(body);
        }
        read(req.send().await).await
    }

    /// `request`, retried while the failure is transient.
    pub async fn call(
        &self,
        method: Method,
        path: &str,
        body: Option<&Value>,
        cancel: &CancellationToken,
    ) -> Result<Value, ApiError> {
        retry(path, cancel, || {
            self.request(method.clone(), path, &[], body)
        })
        .await
    }

    /// Uploads a picture to eBay (Media API); returns its URL for `product.imageUrls`.
    pub async fn upload_image(
        &self,
        jpeg: Vec<u8>,
        cancel: &CancellationToken,
    ) -> Result<String, ApiError> {
        let url = format!(
            "{}/commerce/media/v1_beta/image/create_image_from_file",
            self.hosts.media
        );
        let created = retry("create_image_from_file", cancel, || {
            let part = reqwest::multipart::Part::bytes(jpeg.clone())
                .file_name("card.jpg")
                .mime_str("image/jpeg")
                .expect("a valid mime type");
            let req = self
                .http
                .post(&url)
                .bearer_auth(&self.token)
                .multipart(reqwest::multipart::Form::new().part("image", part));
            async move { read(req.send().await).await }
        })
        .await?;
        created["imageUrl"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| ApiError::Decode("eBay sent no picture URL".into()))
    }
}

async fn read(sent: reqwest::Result<reqwest::Response>) -> Result<Value, ApiError> {
    let resp = sent.map_err(|e| ApiError::Network(e.to_string()))?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn sends_token_and_marketplace_and_explains_errors() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/sell/inventory/v1/offer"))
            .and(header("Authorization", "Bearer access"))
            .and(header("X-EBAY-C-MARKETPLACE-ID", "EBAY_US"))
            .and(header("Content-Language", "en-US"))
            .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!(
                {"errors": [{"errorId": 25005, "message": "Invalid category."}]}
            )))
            .mount(&server)
            .await;
        let cfg = EbaySettings {
            api_base: server.uri(),
            ..Default::default()
        };
        let client = EbayClient::new(&cfg, "access").unwrap();
        let err = client
            .call(
                Method::POST,
                "sell/inventory/v1/offer",
                Some(&serde_json::json!({})),
                &CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert_eq!(err.to_string(), "HTTP 400: Invalid category. (25005)");
    }
}
