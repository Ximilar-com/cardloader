//! Selling a session's cards on eBay, with the user's own eBay developer keyset (an open-source app can't
//! ship a client secret).
//!
//! The seller connects once: eBay's consent page opens in a window of its own, and the authorization code is
//! read from the address eBay sends that window to (eBay only redirects to https pages, so nothing can listen
//! for it locally). The refresh token (≈ 18 months) is kept with the other secrets; access tokens (2 h) only
//! in memory. Cards become unpublished offers (drafts, see [`sync`]) and go live when the user publishes them.

pub mod client;
pub mod listing;
pub mod sync;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use reqwest::Method;
use serde::Serialize;
use serde_json::Value;

use crate::settings::{EbaySettings, Secret, SettingsStore};
use crate::ximilar::http::{detail, ApiError};
use client::EbayClient;

pub const SCOPES: &[&str] = &[
    "https://api.ebay.com/oauth/api_scope/sell.inventory",
    "https://api.ebay.com/oauth/api_scope/sell.account",
    "https://api.ebay.com/oauth/api_scope/commerce.identity.readonly",
];
const LOGIN_WINDOW: &str = "ebay-login";
/// An access token this close to expiry is renewed first.
const RENEW_BEFORE: Duration = Duration::from_secs(5 * 60);

/// eBay's hosts for the environment, or one base for all of them (tests).
#[derive(Debug, Clone)]
pub struct Hosts {
    pub api: String,
    /// The Identity API.
    pub apiz: String,
    /// The Media API (pictures).
    pub media: String,
    pub auth: String,
    /// The site the listings are on.
    pub web: String,
}

impl Hosts {
    pub fn of(cfg: &EbaySettings) -> Hosts {
        if !cfg.api_base.is_empty() {
            let base = cfg.api_base.clone();
            return Hosts {
                api: base.clone(),
                apiz: base.clone(),
                media: base.clone(),
                auth: base.clone(),
                web: base,
            };
        }
        let env = if cfg.sandbox { ".sandbox" } else { "" };
        Hosts {
            api: format!("https://api{env}.ebay.com"),
            apiz: format!("https://apiz{env}.ebay.com"),
            media: format!("https://apim{env}.ebay.com"),
            auth: format!("https://auth{env}.ebay.com"),
            web: if cfg.sandbox {
                "https://sandbox.ebay.com".into()
            } else {
                "https://www.ebay.com".into()
            },
        }
    }
}

pub fn listing_url(cfg: &EbaySettings, listing_id: &str) -> String {
    format!("{}/itm/{listing_id}", Hosts::of(cfg).web)
}

/// The keyset is filled in.
pub fn keyset_missing(cfg: &EbaySettings, settings: &SettingsStore) -> Option<&'static str> {
    if cfg.app_id.is_empty()
        || cfg.ru_name.is_empty()
        || settings.secret(Secret::EbayCert).is_none()
    {
        Some("enter your eBay App ID, Cert ID and RuName in Settings first")
    } else {
        None
    }
}

/// Where eBay's consent page starts.
pub fn authorize_url(cfg: &EbaySettings, state: &str) -> Result<reqwest::Url, ApiError> {
    reqwest::Url::parse_with_params(
        &format!("{}/oauth2/authorize", Hosts::of(cfg).auth),
        &[
            ("client_id", cfg.app_id.as_str()),
            ("redirect_uri", cfg.ru_name.as_str()),
            ("response_type", "code"),
            ("scope", &SCOPES.join(" ")),
            ("state", state),
        ],
    )
    .map_err(|e| ApiError::Decode(e.to_string()))
}

/// What the address eBay sends the consent window to says: the authorization code, or why there is none.
/// `None`: not that address (a page of the sign-in itself). With `state`, only an address carrying it counts.
pub fn consent_result(url: &str, state: Option<&str>) -> Option<Result<String, String>> {
    let url = reqwest::Url::parse(url).ok()?;
    let query: HashMap<String, String> = url.query_pairs().into_owned().collect();
    if let Some(expected) = state {
        if query.get("state").map(String::as_str) != Some(expected) {
            return None;
        }
    }
    if let Some(code) = query.get("code").filter(|c| !c.is_empty()) {
        return Some(Ok(code.clone()));
    }
    let declined = query.get("isAuthSuccessful").map(String::as_str) == Some("false")
        || query.contains_key("error");
    declined.then(|| Err("eBay access wasn't granted".to_string()))
}

/// The authorization code in what the user pasted: the address eBay showed after consent, or the code.
pub fn code_from_text(text: &str) -> Option<String> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    if text.contains("://") {
        return consent_result(text, None)?.ok();
    }
    let text = text.strip_prefix("code=").unwrap_or(text);
    Some(if text.contains('%') {
        percent_decode(text)
    } else {
        text.to_string()
    })
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes
            .get(i + 1..i + 3)
            .and_then(|h| std::str::from_utf8(h).ok())
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match (bytes[i], hex) {
            (b'%', Some(byte)) => {
                out.push(byte);
                i += 3;
            }
            (byte, _) => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Opens eBay's consent page in a window and waits for the authorization code.
pub async fn consent_in_window(
    app: &tauri::AppHandle,
    url: reqwest::Url,
    state: String,
) -> crate::error::Result<String> {
    use crate::error::Error;
    use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

    if let Some(old) = app.get_webview_window(LOGIN_WINDOW) {
        let _ = old.destroy();
    }
    type Answer = tokio::sync::oneshot::Sender<std::result::Result<String, String>>;
    let (tx, rx) = tokio::sync::oneshot::channel();
    let tx: Arc<Mutex<Option<Answer>>> = Arc::new(Mutex::new(Some(tx)));
    let answer = tx.clone();
    let url = tauri::Url::parse(url.as_str()).map_err(|e| Error::msg(e.to_string()))?;
    // The window shows eBay's pages only: it isn't in any capability, so they can't call the app.
    let window = WebviewWindowBuilder::new(app, LOGIN_WINDOW, WebviewUrl::External(url))
        .title("Connect eBay")
        .inner_size(560.0, 780.0)
        .on_navigation(
            move |url| match consent_result(url.as_str(), Some(&state)) {
                Some(result) => {
                    if let Some(tx) = answer.lock().unwrap().take() {
                        let _ = tx.send(result);
                    }
                    false
                }
                None => true,
            },
        )
        .build()?;
    window.on_window_event(move |event| {
        if matches!(
            event,
            tauri::WindowEvent::CloseRequested { .. } | tauri::WindowEvent::Destroyed
        ) {
            if let Some(tx) = tx.lock().unwrap().take() {
                let _ = tx.send(Err("the eBay window was closed".into()));
            }
        }
    });
    let answer = tokio::time::timeout(Duration::from_secs(15 * 60), rx).await;
    let _ = window.destroy();
    match answer {
        Ok(Ok(Ok(code))) => Ok(code),
        Ok(Ok(Err(why))) => Err(Error::msg(why)),
        Ok(Err(_)) => Err(Error::msg("the eBay window was closed")),
        Err(_) => Err(Error::msg("eBay sign-in timed out")),
    }
}

pub struct Tokens {
    pub access: String,
    pub expires_in: u64,
    pub refresh: Option<String>,
}

async fn token_request(
    cfg: &EbaySettings,
    cert: &str,
    form: &[(&str, &str)],
) -> Result<Tokens, ApiError> {
    let resp = client::http()?
        .post(format!("{}/identity/v1/oauth2/token", Hosts::of(cfg).api))
        .basic_auth(&cfg.app_id, Some(cert))
        .form(form)
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
    let body: Value = serde_json::from_str(&text).map_err(|e| ApiError::Decode(e.to_string()))?;
    Ok(Tokens {
        access: body["access_token"]
            .as_str()
            .ok_or_else(|| ApiError::Decode("eBay sent no access token".into()))?
            .to_string(),
        expires_in: body["expires_in"].as_u64().unwrap_or(7200),
        refresh: body["refresh_token"].as_str().map(str::to_string),
    })
}

/// Trades the authorization code from the consent page for tokens.
pub async fn exchange_code(cfg: &EbaySettings, cert: &str, code: &str) -> Result<Tokens, ApiError> {
    token_request(
        cfg,
        cert,
        &[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", &cfg.ru_name),
        ],
    )
    .await
}

async fn renew(cfg: &EbaySettings, cert: &str, refresh: &str) -> Result<Tokens, ApiError> {
    token_request(
        cfg,
        cert,
        &[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh),
            ("scope", &SCOPES.join(" ")),
        ],
    )
    .await
    .map_err(|e| match e {
        // The refresh token expired or was revoked: only connecting again helps.
        ApiError::Status {
            status: 400 | 401,
            detail,
        } => ApiError::Status {
            status: 401,
            detail: format!("the eBay login expired — connect eBay again in Settings ({detail})"),
        },
        e => e,
    })
}

struct Access {
    refresh: String,
    token: String,
    until: Instant,
}

/// The current access token, renewed from the stored refresh token when needed.
#[derive(Default)]
pub struct EbayAuth {
    access: tokio::sync::Mutex<Option<Access>>,
}

impl EbayAuth {
    pub async fn remember(&self, refresh: &str, tokens: &Tokens) {
        *self.access.lock().await = Some(Access {
            refresh: refresh.to_string(),
            token: tokens.access.clone(),
            until: Instant::now() + Duration::from_secs(tokens.expires_in),
        });
    }

    pub async fn forget(&self) {
        *self.access.lock().await = None;
    }

    /// A client for the connected seller.
    pub async fn client(&self, settings: &SettingsStore) -> Result<EbayClient, ApiError> {
        let cfg = settings.get().ebay;
        let refresh = settings
            .secret(Secret::EbayRefresh)
            .ok_or(ApiError::NotConnected)?;
        // Held while renewing, so parallel callers renew once.
        let mut access = self.access.lock().await;
        let fresh = access
            .as_ref()
            .filter(|a| a.refresh == refresh && a.until > Instant::now() + RENEW_BEFORE);
        if let Some(a) = fresh {
            return EbayClient::new(&cfg, &a.token);
        }
        let cert = settings
            .secret(Secret::EbayCert)
            .ok_or(ApiError::NotConnected)?;
        let tokens = renew(&cfg, &cert, &refresh).await?;
        let client = EbayClient::new(&cfg, &tokens.access)?;
        *access = Some(Access {
            refresh,
            token: tokens.access,
            until: Instant::now() + Duration::from_secs(tokens.expires_in),
        });
        Ok(client)
    }
}

/// The connected seller's eBay username.
pub async fn username(client: &EbayClient) -> Option<String> {
    let url = format!("{}/commerce/identity/v1/user/", client.hosts().apiz);
    let user = client.request(Method::GET, &url, &[], None).await.ok()?;
    user["username"].as_str().map(str::to_string)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Named {
    pub id: String,
    pub name: String,
}

/// What the seller's account offers for listings: business policies and inventory locations.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EbaySetup {
    pub username: Option<String>,
    /// Business policies are turned on (`SELLING_POLICY_MANAGEMENT`); listings need them.
    pub opted_in: bool,
    pub fulfillment_policies: Vec<Named>,
    pub payment_policies: Vec<Named>,
    pub return_policies: Vec<Named>,
    pub locations: Vec<Named>,
}

pub async fn setup(client: &EbayClient, username: Option<String>) -> Result<EbaySetup, ApiError> {
    let programs = client
        .request(
            Method::GET,
            "sell/account/v1/program/get_opted_in_programs",
            &[],
            None,
        )
        .await?;
    let opted_in = programs["programs"].as_array().is_some_and(|p| {
        p.iter()
            .any(|p| p["programType"] == "SELLING_POLICY_MANAGEMENT")
    });
    let market = [("marketplace_id", client.marketplace().to_string())];
    let mut policies = Vec::new();
    for (path, list, id) in [
        (
            "fulfillment_policy",
            "fulfillmentPolicies",
            "fulfillmentPolicyId",
        ),
        ("payment_policy", "paymentPolicies", "paymentPolicyId"),
        ("return_policy", "returnPolicies", "returnPolicyId"),
    ] {
        policies.push(if opted_in {
            let found = client
                .request(
                    Method::GET,
                    &format!("sell/account/v1/{path}"),
                    &market,
                    None,
                )
                .await?;
            named(
                &found[list],
                |p| p[id].as_str(),
                |p| p["name"].as_str().map(str::to_string),
            )
        } else {
            Vec::new()
        });
    }
    let found = client
        .request(
            Method::GET,
            "sell/inventory/v1/location",
            &[("limit", "100".into())],
            None,
        )
        .await?;
    let locations = named(
        &found["locations"],
        |l| l["merchantLocationKey"].as_str(),
        |l| {
            let address = &l["location"]["address"];
            let place: Vec<&str> = ["city", "postalCode", "country"]
                .iter()
                .filter_map(|k| address[*k].as_str())
                .collect();
            match l["name"].as_str() {
                Some(name) if !place.is_empty() => Some(format!("{name} ({})", place.join(" "))),
                Some(name) => Some(name.to_string()),
                None => Some(place.join(" ")),
            }
        },
    );
    let mut policies = policies.into_iter();
    Ok(EbaySetup {
        username,
        opted_in,
        fulfillment_policies: policies.next().unwrap_or_default(),
        payment_policies: policies.next().unwrap_or_default(),
        return_policies: policies.next().unwrap_or_default(),
        locations,
    })
}

fn named(
    list: &Value,
    id: impl Fn(&Value) -> Option<&str>,
    name: impl Fn(&Value) -> Option<String>,
) -> Vec<Named> {
    list.as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|v| {
                    let id = id(v)?.to_string();
                    let name = name(v)
                        .filter(|n| !n.is_empty())
                        .unwrap_or_else(|| id.clone());
                    Some(Named { id, name })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub async fn opt_in(client: &EbayClient) -> Result<(), ApiError> {
    client
        .request(
            Method::POST,
            "sell/account/v1/program/opt_in",
            &[],
            Some(&serde_json::json!({"programType": "SELLING_POLICY_MANAGEMENT"})),
        )
        .await
        .map(|_| ())
}

/// Adds the place cards ship from; returns its key.
pub async fn create_location(
    client: &EbayClient,
    postal_code: &str,
    country: &str,
) -> Result<String, ApiError> {
    let key = format!(
        "cardloader-{}",
        &uuid::Uuid::new_v4().simple().to_string()[..8]
    );
    let body = serde_json::json!({
        "name": "Cardloader",
        "location": {"address": {"postalCode": postal_code, "country": country}},
        "locationTypes": ["WAREHOUSE"],
        "merchantLocationStatus": "ENABLED",
    });
    client
        .request(
            Method::POST,
            &format!("sell/inventory/v1/location/{key}"),
            &[],
            Some(&body),
        )
        .await?;
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosts_follow_the_environment() {
        let mut cfg = EbaySettings::default();
        assert_eq!(Hosts::of(&cfg).api, "https://api.ebay.com");
        assert_eq!(listing_url(&cfg, "1"), "https://www.ebay.com/itm/1");
        cfg.sandbox = true;
        assert_eq!(Hosts::of(&cfg).media, "https://apim.sandbox.ebay.com");
        assert_eq!(listing_url(&cfg, "1"), "https://sandbox.ebay.com/itm/1");
        cfg.api_base = "http://127.0.0.1:1".into();
        assert_eq!(Hosts::of(&cfg).auth, "http://127.0.0.1:1");
    }

    #[test]
    fn consent_url_asks_for_every_scope() {
        let cfg = EbaySettings {
            app_id: "Demo-App".into(),
            ru_name: "Demo-RuName".into(),
            ..Default::default()
        };
        let url = authorize_url(&cfg, "s1").unwrap();
        let query: HashMap<String, String> = url.query_pairs().into_owned().collect();
        assert_eq!(url.host_str(), Some("auth.ebay.com"));
        assert_eq!(query["redirect_uri"], "Demo-RuName");
        assert_eq!(query["scope"].split(' ').count(), SCOPES.len());
    }

    #[test]
    fn the_code_is_read_from_ebays_redirect() {
        let accepted = "https://signin.ebay.com/ws/eBayISAPI.dll?ThirdPartyAuthSucessFailure&isAuthSuccessful=true&state=s1&code=v%5E1.1%23i%5E1&expires_in=299";
        assert_eq!(
            consent_result(accepted, Some("s1")),
            Some(Ok("v^1.1#i^1".into()))
        );
        assert_eq!(
            consent_result(accepted, Some("other")),
            None,
            "not our request"
        );
        let declined = "https://signin.ebay.com/ws/eBayISAPI.dll?ThirdPartyAuthSucessFailure&isAuthSuccessful=false&state=s1";
        assert!(matches!(consent_result(declined, Some("s1")), Some(Err(_))));
        assert_eq!(
            consent_result("https://signin.ebay.com/signin?sgfl=1", Some("s1")),
            None
        );

        assert_eq!(code_from_text(accepted).as_deref(), Some("v^1.1#i^1"));
        assert_eq!(
            code_from_text(" v%5E1.1%23i%5E1 ").as_deref(),
            Some("v^1.1#i^1")
        );
        assert_eq!(code_from_text("v^1.1#i^1").as_deref(), Some("v^1.1#i^1"));
        assert_eq!(code_from_text("  "), None);
    }
}
