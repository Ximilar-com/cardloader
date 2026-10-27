//! The MCP server AI agents (Claude Desktop, Claude Code, Cursor, …) drive the app through.
//!
//! It runs inside the app, on 127.0.0.1 only, so agents share the open app's database, runs and events: what an
//! agent does shows up in the UI as it happens. A second process opening the database would pause every running
//! session (`Db::recover_interrupted`). Every request carries the token from Settings (`Secret::McpToken`).
//! Clients that only start programs (Claude Desktop) run `cardloader mcp` (`bridge`), which relays stdio here.

pub mod bridge;
mod prompts;
mod tools;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use axum::extract::{Request, State};
use axum::http::{header, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::commands::AppState;
use crate::error::{Error, Result};
use crate::settings::Secret;

/// The bundle identifier: Tauri keeps the config folder under it, and the bridge looks there.
pub const APP_ID: &str = "com.ximilar.cardloader";
/// Ports tried after the one in Settings, when it is taken.
const PORT_TRIES: u16 = 10;

/// Where the running server listens; `cardloader mcp` reads it from `<config>/mcp.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Endpoint {
    pub port: u16,
    pub pid: u32,
}

impl Endpoint {
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/mcp", self.port)
    }
}

pub fn endpoint_file(config_dir: &Path) -> PathBuf {
    config_dir.join("mcp.json")
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpStatus {
    pub enabled: bool,
    pub running: bool,
    pub url: Option<String>,
    pub token: Option<String>,
    /// What clients that start a program run (with the argument `mcp`): this app.
    pub command: String,
    pub error: Option<String>,
    /// When an agent last called a tool (RFC 3339).
    pub last_used: Option<String>,
}

struct Running {
    /// The port Settings asked for (the one used may be a later one).
    requested: u16,
    endpoint: Endpoint,
    cancel: CancellationToken,
}

pub struct McpServer {
    state: AppState,
    config_dir: PathBuf,
    running: tokio::sync::Mutex<Option<Running>>,
    error: Mutex<Option<String>>,
    last_used: Arc<Mutex<Option<String>>>,
}

impl McpServer {
    pub fn new(state: AppState, config_dir: PathBuf) -> Arc<Self> {
        Arc::new(McpServer {
            state,
            config_dir,
            running: tokio::sync::Mutex::new(None),
            error: Mutex::new(None),
            last_used: Arc::new(Mutex::new(None)),
        })
    }

    /// Starts or stops the server as Settings say (and moves it when the port changed). What agents may do is
    /// read from Settings on every call, so changing that needs no restart.
    pub async fn apply(&self) {
        let wanted = self.state.settings.get().mcp;
        let mut running = self.running.lock().await;
        if let Some(r) = running.as_ref() {
            if wanted.enabled && r.requested == wanted.port {
                return;
            }
            r.cancel.cancel();
            let _ = std::fs::remove_file(endpoint_file(&self.config_dir));
            *running = None;
        }
        *self.error.lock().unwrap() = None;
        if !wanted.enabled {
            return;
        }
        match self.start(wanted.port).await {
            Ok(r) => *running = Some(r),
            Err(e) => {
                tracing::warn!("MCP server: {e}");
                *self.error.lock().unwrap() = Some(e.to_string());
            }
        }
    }

    async fn start(&self, requested: u16) -> Result<Running> {
        self.token()?;
        let mut listener = None;
        for port in requested..requested.saturating_add(PORT_TRIES) {
            if let Ok(l) = tokio::net::TcpListener::bind(("127.0.0.1", port)).await {
                listener = Some(l);
                break;
            }
        }
        let listener = listener.ok_or_else(|| {
            Error::msg(format!(
                "ports {requested}–{} are taken: choose another port",
                requested.saturating_add(PORT_TRIES - 1)
            ))
        })?;
        let port = listener.local_addr()?.port();
        let cancel = CancellationToken::new();
        let router = self.router(cancel.child_token());
        let stop = cancel.clone();
        tokio::spawn(async move {
            let served = axum::serve(listener, router)
                .with_graceful_shutdown(async move { stop.cancelled_owned().await })
                .await;
            if let Err(e) = served {
                tracing::warn!("MCP server stopped: {e}");
            }
        });
        let endpoint = Endpoint {
            port,
            pid: std::process::id(),
        };
        std::fs::create_dir_all(&self.config_dir)?;
        std::fs::write(
            endpoint_file(&self.config_dir),
            serde_json::to_vec_pretty(&endpoint)?,
        )?;
        tracing::info!("MCP server on {}", endpoint.url());
        Ok(Running {
            requested,
            endpoint,
            cancel,
        })
    }

    fn router(&self, cancel: CancellationToken) -> axum::Router {
        let handler = tools::Handler::new(self.state.clone(), self.last_used.clone());
        // Stateless JSON answers: every request stands alone, which the bridge relays simply. Hosts are loopback
        // only (rmcp's default); a browser page may only call from localhost, and still needs the token.
        let config = StreamableHttpServerConfig::default()
            .with_legacy_session_mode(false)
            .with_json_response(true)
            .with_sse_keep_alive(None)
            .with_allowed_origins(["http://localhost:*", "http://127.0.0.1:*"])
            .with_cancellation_token(cancel);
        let service: StreamableHttpService<tools::Handler, LocalSessionManager> =
            StreamableHttpService::new(
                move || Ok(handler.clone()),
                Arc::new(LocalSessionManager::default()),
                config,
            );
        axum::Router::new()
            .nest_service("/mcp", service)
            .layer(middleware::from_fn_with_state(
                self.state.clone(),
                authorize,
            ))
    }

    /// The token agents send, made on first use.
    pub fn token(&self) -> Result<String> {
        if let Some(token) = self.state.settings.secret(Secret::McpToken) {
            return Ok(token);
        }
        self.regenerate_token()
    }

    /// A new token: agents set up with the old one must be given this one.
    pub fn regenerate_token(&self) -> Result<String> {
        let token = format!(
            "cl_{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        self.state.settings.set_secret(Secret::McpToken, &token)?;
        Ok(token)
    }

    pub async fn status(&self) -> McpStatus {
        let enabled = self.state.settings.get().mcp.enabled;
        let running = self.running.lock().await;
        McpStatus {
            enabled,
            running: running.is_some(),
            url: running.as_ref().map(|r| r.endpoint.url()),
            token: if enabled { self.token().ok() } else { None },
            command: std::env::current_exe()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default(),
            error: self.error.lock().unwrap().clone(),
            last_used: self.last_used.lock().unwrap().clone(),
        }
    }

    /// The app is quitting: the bridge shouldn't find an address nobody answers on.
    pub fn forget_endpoint(&self) {
        let _ = std::fs::remove_file(endpoint_file(&self.config_dir));
    }
}

/// Lets a request through only with `Authorization: Bearer <token>`.
async fn authorize(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let expected = state.settings.secret(Secret::McpToken);
    let sent = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::trim);
    match (expected, sent) {
        (Some(expected), Some(sent)) if same(expected.as_bytes(), sent.as_bytes()) => {
            next.run(request).await
        }
        _ => (
            StatusCode::UNAUTHORIZED,
            "Cardloader: send the token from Settings → AI agents as `Authorization: Bearer <token>`",
        )
            .into_response(),
    }
}

/// Compares without stopping at the first difference, so timing says nothing about the token.
fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests;
