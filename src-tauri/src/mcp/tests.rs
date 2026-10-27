use std::sync::Arc;
use std::time::{Duration, Instant};

use image::{Rgb, RgbImage};
use serde_json::{json, Value};

use super::*;
use crate::db::Db;
use crate::model::Reviewer;
use crate::runner::tests::{mock_ximilar, Recorder};
use crate::runner::Runner;
use crate::settings::{McpSettings, Settings, SettingsStore};

struct Agent {
    http: reqwest::Client,
    url: String,
    token: String,
}

impl Agent {
    async fn rpc(&self, method: &str, params: Value) -> Value {
        let response = self
            .http
            .post(&self.url)
            .bearer_auth(&self.token)
            .header("Accept", "application/json, text/event-stream")
            .json(&json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}))
            .send()
            .await
            .unwrap();
        assert!(
            response.status().is_success(),
            "{method}: {}",
            response.status()
        );
        let text = response.text().await.unwrap();
        let reply: Value = serde_json::from_str(&text)
            .or_else(|_| bridge_events(&text).into_iter().next().ok_or(()))
            .unwrap_or_else(|_| panic!("{method}: {text}"));
        assert!(reply.get("error").is_none(), "{method}: {reply}");
        reply["result"].clone()
    }

    /// A tool's JSON text, and whether it failed.
    async fn tool(&self, name: &str, arguments: Value) -> (Value, bool) {
        let result = self
            .rpc("tools/call", json!({"name": name, "arguments": arguments}))
            .await;
        let text = result["content"][0]["text"].as_str().unwrap_or_default();
        let value = serde_json::from_str(text).unwrap_or(Value::String(text.to_string()));
        (value, result["isError"] == true)
    }

    async fn ok(&self, name: &str, arguments: Value) -> Value {
        let (value, failed) = self.tool(name, arguments).await;
        assert!(!failed, "{name}: {value}");
        value
    }
}

fn bridge_events(text: &str) -> Vec<Value> {
    super::bridge::events(text)
}

/// An agent reads a folder, analyses it as English Pokémon cards, waits for the run, looks at a card with its
/// picture, accepts one and picks another candidate for the other — over HTTP, against a mock Ximilar.
#[tokio::test(flavor = "multi_thread")]
async fn agent_runs_and_reviews_a_folder() {
    let server = mock_ximilar().await;
    let dir = tempfile::tempdir().unwrap();
    let cards = dir.path().join("pokemon-en");
    std::fs::create_dir_all(&cards).unwrap();
    for name in ["a", "b"] {
        RgbImage::from_pixel(630, 880, Rgb([200, 30, 30]))
            .save(cards.join(format!("{name}.png")))
            .unwrap();
    }
    let db = Db::memory().unwrap();
    let config = dir.path().join("config");
    let settings = Arc::new(SettingsStore::for_test(
        config.join("settings.json"),
        Settings {
            api_base_url: server.uri(),
            mcp: McpSettings {
                enabled: true,
                port: 0,
                ..McpSettings::default()
            },
            ..Settings::default()
        },
        "token",
    ));
    let runner = Runner::new(
        None,
        Arc::new(Recorder::default()),
        db.clone(),
        settings.clone(),
        dir.path().join("data"),
    );
    let state = AppState {
        db: db.clone(),
        settings: settings.clone(),
        runner,
    };
    let mcp = McpServer::new(state, config.clone());
    mcp.apply().await;
    let status = mcp.status().await;
    assert!(status.running, "{status:?}");
    let written: Endpoint =
        serde_json::from_str(&std::fs::read_to_string(endpoint_file(&config)).unwrap()).unwrap();
    assert_eq!(Some(written.url()), status.url);
    let agent = Agent {
        http: reqwest::Client::new(),
        url: status.url.clone().unwrap(),
        token: status.token.clone().unwrap(),
    };

    // No token, or a web page elsewhere: refused.
    let refused = agent
        .http
        .post(&agent.url)
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(refused.status(), 401);
    let foreign = agent
        .http
        .post(&agent.url)
        .bearer_auth(&agent.token)
        .header("Origin", "https://evil.example")
        .header("Accept", "application/json, text/event-stream")
        .json(&json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}))
        .send()
        .await
        .unwrap();
    assert_eq!(foreign.status(), 403);

    let init = agent
        .rpc(
            "initialize",
            json!({"protocolVersion": "2025-06-18", "capabilities": {},
                   "clientInfo": {"name": "test", "version": "1"}}),
        )
        .await;
    assert_eq!(init["serverInfo"]["name"], "cardloader");

    // Run & review is allowed by default; deleting isn't.
    let tools = agent.rpc("tools/list", json!({})).await;
    let names: Vec<&str> = tools["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert!(names.contains(&"create_session") && names.contains(&"get_card"));
    assert!(!names.contains(&"delete_session") && !names.contains(&"ebay_publish"));
    let (refusal, failed) = agent
        .tool("delete_session", json!({"session_id": "x"}))
        .await;
    assert!(failed);
    assert!(refusal.as_str().unwrap().contains("Settings → AI agents"));

    let folder = agent
        .ok("inspect_folder", json!({"path": cards.to_string_lossy()}))
        .await;
    assert_eq!(folder["images"], 2);

    let created = agent
        .ok(
            "create_session",
            json!({"path": cards.to_string_lossy(), "card_type": "tcg", "game": "pokemon",
                   "alphabet": "English", "known_side": true}),
        )
        .await;
    let session_id = created["id"].as_str().unwrap().to_string();
    assert!(created["name"]
        .as_str()
        .unwrap()
        .starts_with("pokemon-en · "));
    assert_eq!(created["options"]["subcategory"], "Pokemon");
    assert_eq!(created["options"]["alphabet"], "latin");
    assert_eq!(created["options"]["knownSide"], true);
    assert_eq!(created["options"]["knownCategory"], true);

    let started = Instant::now();
    let finished = loop {
        let progress = agent
            .ok(
                "wait_for_session",
                json!({"session_id": "latest", "timeout_seconds": 5}),
            )
            .await;
        if progress["finished"] == true {
            break progress;
        }
        assert!(started.elapsed() < Duration::from_secs(30), "{progress}");
    };
    assert_eq!(finished["status"], "completed", "{finished}");
    assert_eq!(finished["counts"]["identified"], 2);
    assert_eq!(finished["counts"]["toReview"], 2);

    let listed = agent
        .ok(
            "list_cards",
            json!({"session_id": session_id, "filter": "to_review"}),
        )
        .await;
    let ids: Vec<i64> = listed["cards"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_i64().unwrap())
        .collect();
    assert_eq!(ids.len(), 2);
    assert_eq!(listed["cards"][0]["name"], "Charizard ex");
    assert_eq!(listed["cards"][0]["confidence"], "high");

    // The card comes with its candidates and its picture.
    let card = agent
        .rpc(
            "tools/call",
            json!({"name": "get_card", "arguments": {"item_id": ids[0]}}),
        )
        .await;
    let content = card["content"].as_array().unwrap();
    let details: Value = serde_json::from_str(content[0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(details["candidates"][0]["index"], 0);
    assert_eq!(details["candidates"][0]["number"], Value::Null);
    assert_eq!(details["candidates"][0]["cardNumber"], "223");
    assert_eq!(details["candidates"][0]["confidence"], "medium");
    assert!(content
        .iter()
        .any(|c| c["type"] == "image" && c["mimeType"] == "image/jpeg"));

    let accepted = agent
        .ok(
            "accept_cards",
            json!({"item_ids": [ids[0]], "note": "name and 125 match"}),
        )
        .await;
    assert_eq!(accepted["changed"], 1);
    let first = db.item_summary(ids[0]).unwrap();
    assert!(first.reviewed && first.agent_reviewed);
    assert_eq!(first.review_note, "name and 125 match");

    let picked = agent
        .ok(
            "select_candidate",
            json!({"item_id": ids[1], "index": 0, "note": "it says 223"}),
        )
        .await;
    assert_eq!(picked["number"], "223");
    assert_eq!(picked["acceptedBy"], "agent");
    let ai = agent
        .ok(
            "list_cards",
            json!({"session_id": "latest", "filter": "accepted_by_ai"}),
        )
        .await;
    assert_eq!(ai["matching"], 2);

    // The user accepting a card the agent accepted makes it theirs; the agent can't take it over again.
    db.set_reviewed(&session_id, &[ids[0]], true, &Reviewer::User)
        .unwrap();
    assert!(!db.item_summary(ids[0]).unwrap().agent_reviewed);
    let again = agent
        .ok("accept_cards", json!({"item_ids": [ids[0]]}))
        .await;
    assert_eq!(again["changed"], 0);
    assert!(!db.item_summary(ids[0]).unwrap().agent_reviewed);

    // `cardloader mcp` relays stdio lines to the server, finding it and the token from the config folder.
    let bridge = bridge::Bridge::new(config.clone()).unwrap();
    let replies = bridge
        .handle(r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"list_sessions","arguments":{}}}"#)
        .await;
    assert_eq!(replies.len(), 1, "{replies:?}");
    assert_eq!(replies[0]["id"], 7);
    assert!(replies[0]["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains(&session_id));
    let notification = bridge
        .handle(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
        .await;
    assert!(notification.is_empty(), "{notification:?}");

    // Prompts.
    let prompts = agent.rpc("prompts/list", json!({})).await;
    assert_eq!(prompts["prompts"][0]["name"], "review_session");
    let prompt = agent
        .rpc(
            "prompts/get",
            json!({"name": "review_session", "arguments": {}}),
        )
        .await;
    assert!(prompt["messages"][0]["content"]["text"]
        .as_str()
        .unwrap()
        .contains("filter to_review"));

    // Turning agents off stops the server and forgets its address.
    settings
        .update(Settings {
            mcp: McpSettings::default(),
            ..settings.get()
        })
        .unwrap();
    mcp.apply().await;
    assert!(!mcp.status().await.running);
    assert!(!endpoint_file(&config).exists());
    let off = bridge
        .handle(r#"{"jsonrpc":"2.0","id":8,"method":"tools/list"}"#)
        .await;
    assert!(
        off[0]["error"]["message"]
            .as_str()
            .unwrap()
            .contains("turned off"),
        "{off:?}"
    );
}

#[test]
fn compares_tokens() {
    assert!(same(b"cl_abc", b"cl_abc"));
    assert!(!same(b"cl_abc", b"cl_abd"));
    assert!(!same(b"cl_abc", b"cl_ab"));
}
