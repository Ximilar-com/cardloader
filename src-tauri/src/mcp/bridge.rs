//! `cardloader mcp`: for clients that start a program and talk to it over stdio (Claude Desktop, Cursor, …).
//!
//! It never opens the database: it relays each JSON-RPC line to the running app's server (address in
//! `<config>/mcp.json`, token from Settings) and writes the answers back, one per line. Requests run side by
//! side, so a long `wait_for_session` doesn't hold up the others. On macOS it opens the app when it isn't running.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use reqwest::StatusCode;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;

use super::{endpoint_file, Endpoint, APP_ID};
use crate::error::{Error, Result};
use crate::settings::{Secret, SettingsStore};

/// How long the app may take to start and open its server.
const LAUNCH_WAIT: Duration = Duration::from_secs(30);

pub fn run() -> i32 {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("cardloader mcp: {e}");
            return 1;
        }
    };
    match runtime.block_on(relay()) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("cardloader mcp: {e}");
            1
        }
    }
}

struct Target {
    url: String,
    token: String,
}

pub(super) struct Bridge {
    config_dir: PathBuf,
    http: reqwest::Client,
    target: Mutex<Option<Arc<Target>>>,
    /// The protocol version the client and server agreed on, sent with every later request.
    version: std::sync::Mutex<Option<String>>,
    launched: std::sync::Mutex<bool>,
}

async fn relay() -> Result<()> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| Error::msg("no config folder"))?
        .join(APP_ID);
    let bridge = Arc::new(Bridge::new(config_dir)?);
    let stdout = Arc::new(Mutex::new(tokio::io::stdout()));
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut tasks = tokio::task::JoinSet::new();
    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let bridge = bridge.clone();
        let stdout = stdout.clone();
        tasks.spawn(async move {
            for reply in bridge.handle(&line).await {
                let mut out = stdout.lock().await;
                let _ = out.write_all(format!("{reply}\n").as_bytes()).await;
                let _ = out.flush().await;
            }
        });
    }
    while tasks.join_next().await.is_some() {}
    Ok(())
}

impl Bridge {
    pub(super) fn new(config_dir: PathBuf) -> Result<Self> {
        Ok(Bridge {
            config_dir,
            // Longer than `wait_for_session` may wait.
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(super::tools::WAIT_MAX + 60))
                .build()
                .map_err(|e| Error::msg(e.to_string()))?,
            target: Mutex::new(None),
            version: std::sync::Mutex::new(None),
            launched: std::sync::Mutex::new(false),
        })
    }

    /// The answers to one message: none for a notification, a JSON-RPC error when the app can't be reached.
    pub(super) async fn handle(&self, line: &str) -> Vec<Value> {
        let message: Value = match serde_json::from_str(line) {
            Ok(m) => m,
            Err(e) => return vec![rpc_error(Value::Null, -32700, &format!("parse error: {e}"))],
        };
        let id = message.get("method").and(message.get("id")).cloned();
        match self.forward(line).await {
            Ok(replies) => {
                for reply in &replies {
                    if let Some(v) = reply
                        .pointer("/result/protocolVersion")
                        .and_then(Value::as_str)
                    {
                        *self.version.lock().unwrap() = Some(v.to_string());
                    }
                }
                replies
            }
            Err(e) => id
                .map(|id| vec![rpc_error(id, -32000, &e.to_string())])
                .unwrap_or_default(),
        }
    }

    async fn forward(&self, body: &str) -> Result<Vec<Value>> {
        let mut retried = false;
        loop {
            let target = self.target().await?;
            let mut request = self
                .http
                .post(&target.url)
                .header(AUTHORIZATION, format!("Bearer {}", target.token))
                .header(CONTENT_TYPE, "application/json")
                .header(ACCEPT, "application/json, text/event-stream")
                .body(body.to_string());
            if let Some(v) = self.version.lock().unwrap().clone() {
                request = request.header("MCP-Protocol-Version", v);
            }
            let response = match request.send().await {
                Ok(r) => r,
                // The app quit or moved: look for it again, once.
                Err(_) if !retried => {
                    retried = true;
                    *self.target.lock().await = None;
                    continue;
                }
                Err(e) => return Err(Error::msg(format!("Cardloader didn't answer: {e}"))),
            };
            let status = response.status();
            // The server was stopping (agents turned off, the app quitting or moving) and dropped the request: look
            // for it again, once. Only when it is gone from mcp.json, so a call that failed in a running app isn't
            // sent twice.
            if status.is_server_error() && !retried && !self.serves(&target).await {
                retried = true;
                *self.target.lock().await = None;
                continue;
            }
            // A new token: read it again, once.
            if status == StatusCode::UNAUTHORIZED && !retried {
                retried = true;
                *self.target.lock().await = None;
                continue;
            }
            let event_stream = response
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .is_some_and(|v| v.starts_with("text/event-stream"));
            let text = response.text().await.unwrap_or_default();
            if status == StatusCode::ACCEPTED || status == StatusCode::NO_CONTENT {
                return Ok(Vec::new());
            }
            if event_stream {
                return Ok(events(&text));
            }
            return match serde_json::from_str::<Value>(&text) {
                Ok(reply) => Ok(vec![reply]),
                Err(_) if text.trim().is_empty() && status.is_success() => Ok(Vec::new()),
                Err(_) => Err(Error::msg(format!(
                    "Cardloader answered {status}: {}",
                    text.trim()
                ))),
            };
        }
    }

    /// Where the app's server is and its token; opens the app first when it isn't running (macOS).
    async fn target(&self) -> Result<Arc<Target>> {
        let mut cached = self.target.lock().await;
        if let Some(t) = cached.as_ref() {
            return Ok(t.clone());
        }
        let settings = SettingsStore::load(self.config_dir.join("settings.json"));
        if !settings.get().mcp.enabled {
            return Err(Error::msg(
                "AI agents are turned off: turn them on in Cardloader → Settings → AI agents",
            ));
        }
        let token = settings.secret(Secret::McpToken).ok_or_else(|| {
            Error::msg("open Cardloader → Settings → AI agents once, which makes the token")
        })?;
        let endpoint = match self.reachable().await {
            Some(e) => e,
            None => self.launch().await?,
        };
        let target = Arc::new(Target {
            url: endpoint.url(),
            token,
        });
        *cached = Some(target.clone());
        Ok(target)
    }

    /// The running app's address, if it answers there.
    async fn reachable(&self) -> Option<Endpoint> {
        let text = std::fs::read_to_string(endpoint_file(&self.config_dir)).ok()?;
        let endpoint: Endpoint = serde_json::from_str(&text).ok()?;
        tokio::net::TcpStream::connect(("127.0.0.1", endpoint.port))
            .await
            .ok()
            .map(|_| endpoint)
    }

    /// Whether the app still serves agents where `target` points.
    async fn serves(&self, target: &Target) -> bool {
        self.reachable()
            .await
            .is_some_and(|e| e.url() == target.url)
    }

    async fn launch(&self) -> Result<Endpoint> {
        let not_running = || Error::msg("Cardloader isn't running: open it and try again");
        let already = std::mem::replace(&mut *self.launched.lock().unwrap(), true);
        if already {
            return Err(not_running());
        }
        let bundle = std::env::current_exe()
            .ok()
            .and_then(|exe| app_bundle(&exe));
        // Only a macOS app bundle: `open` brings up the running app rather than starting a second one, which
        // would share the database with it.
        let Some(bundle) = bundle else {
            return Err(not_running());
        };
        std::process::Command::new("open")
            .arg("-g")
            .arg(&bundle)
            .status()
            .map_err(|e| Error::msg(format!("could not open Cardloader: {e}")))?;
        let started = Instant::now();
        while started.elapsed() < LAUNCH_WAIT {
            if let Some(endpoint) = self.reachable().await {
                return Ok(endpoint);
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        Err(not_running())
    }
}

/// The `.app` folder the program is in (macOS), e.g. `/Applications/Cardloader.app`.
fn app_bundle(exe: &Path) -> Option<PathBuf> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    exe.ancestors()
        .find(|p| p.extension().is_some_and(|e| e == "app"))
        .map(Path::to_path_buf)
}

/// The JSON messages in a server-sent event stream (`data:` lines, an event per blank line).
pub(super) fn events(text: &str) -> Vec<Value> {
    let mut out = Vec::new();
    let mut data = String::new();
    for line in text.lines().chain(std::iter::once("")) {
        if let Some(rest) = line.strip_prefix("data:") {
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(rest.strip_prefix(' ').unwrap_or(rest));
        } else if line.is_empty() && !data.is_empty() {
            if let Ok(v) = serde_json::from_str(&data) {
                out.push(v);
            }
            data.clear();
        }
    }
    out
}

fn rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_event_streams() {
        let text = "id: 1\nretry: 3000\ndata:\n\nevent: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}\n\n";
        assert_eq!(
            events(text),
            vec![json!({"jsonrpc": "2.0", "id": 1, "result": {}})]
        );
    }

    #[test]
    fn finds_the_app_bundle() {
        let exe = Path::new("/Applications/Cardloader.app/Contents/MacOS/cardloader");
        let expected =
            cfg!(target_os = "macos").then(|| PathBuf::from("/Applications/Cardloader.app"));
        assert_eq!(app_bundle(exe), expected);
        assert_eq!(
            app_bundle(Path::new("/Users/demo/target/debug/cardloader")),
            None
        );
    }
}
