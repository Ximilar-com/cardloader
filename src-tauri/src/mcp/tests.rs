use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use image::{Rgb, RgbImage};
use serde_json::{json, Value};

use super::*;
use crate::db::Db;
use crate::model::{Engine, GradingMode, Prices, Reviewer};
use crate::runner::tests::{mock_ximilar, Recorder};
use crate::runner::{AgentTask, Runner};
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
    for tool in [
        "next_cards_to_crop",
        "preview_crop",
        "crop_cards",
        "listing_pictures",
    ] {
        assert!(names.contains(&tool), "{tool}");
    }
    assert!(!names.contains(&"delete_session") && !names.contains(&"ebay_publish"));
    let (refusal, failed) = agent.tool("delete_session", json!({"session": "x"})).await;
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
                json!({"session": "latest", "timeout_seconds": 5}),
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
            json!({"session": session_id, "filter": "to_review"}),
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

    // One text for many cards: shown first, then added. What many cards get at once accepts none of them.
    assert!(names.contains(&"edit_cards"));
    let gem = json!({"item_ids": ids, "field": "title", "text": " GEM  MINT "});
    let mut asked = gem.clone();
    asked["preview"] = json!(true);
    let shown = agent.ok("edit_cards", asked).await;
    assert_eq!(shown["changed"], 0);
    let would_be = shown["titles"][0]["title"].as_str().unwrap().to_string();
    assert!(would_be.ends_with(" GEM MINT") && would_be.contains("Charizard ex"));
    assert_eq!(shown["titles"][0]["length"], would_be.chars().count());
    assert_eq!(db.item_summary(ids[0]).unwrap().title, "");
    let edited = agent.ok("edit_cards", gem.clone()).await;
    assert_eq!(
        (&edited["changed"], &edited["skipped"]),
        (&json!(2), &json!(0))
    );
    assert_eq!(db.item_summary(ids[0]).unwrap().title, would_be);
    // The text is there now: asked again, nothing changes.
    assert_eq!(agent.ok("edit_cards", gem).await["changed"], 0);
    let featured = agent
        .ok(
            "edit_cards",
            json!({"item_ids": ids, "field": "features", "place": "start", "text": "Flagship"}),
        )
        .await;
    assert_eq!(featured["changed"], 2);
    let first = db.item_summary(ids[0]).unwrap();
    assert_eq!(first.card.features, "Flagship");
    assert!(!first.reviewed && !first.edited);
    let (refusal, failed) = agent
        .tool(
            "edit_cards",
            json!({"item_ids": ids, "field": "title", "place": "start", "text": "x".repeat(80)}),
        )
        .await;
    assert!(!failed, "{refusal}");
    assert_eq!(
        (&refusal["changed"], &refusal["skipped"]),
        (&json!(0), &json!(2))
    );
    assert!(refusal["titles"][0]["problem"]
        .as_str()
        .unwrap()
        .contains("80"));
    let own = agent.ok("get_card", json!({"item_id": ids[0]})).await;
    assert_eq!(own["listingTitle"], json!(would_be));
    assert_eq!(own["customTitle"], json!(would_be));
    let back = agent
        .ok(
            "edit_cards",
            json!({"item_ids": [ids[0]], "field": "title", "place": "replace", "text": ""}),
        )
        .await;
    assert_eq!(back["changed"], 1);
    assert_eq!(back["titles"][0]["custom"], false);

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
            json!({"session": "latest", "filter": "accepted_by_ai"}),
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

    // A client keeps the bridge for days and asks for the tools once: the bridge promises to say when they
    // change, and does when another app (an update) or other permissions are behind it.
    let hello = bridge
        .handle(r#"{"jsonrpc":"2.0","id":8,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}"#)
        .await;
    assert_eq!(
        hello[0]["result"]["capabilities"]["tools"]["listChanged"],
        true
    );
    assert!(!bridge.tools_changed(), "the client has no list yet");
    let listed = bridge
        .handle(r#"{"jsonrpc":"2.0","id":9,"method":"tools/list"}"#)
        .await;
    assert!(listed[0]["result"]["tools"].as_array().unwrap().len() > 20);
    assert!(!bridge.tools_changed());
    let allowed = settings.get();
    settings
        .update(Settings {
            mcp: crate::settings::McpSettings {
                allow_delete: true,
                ..allowed.mcp.clone()
            },
            ..allowed.clone()
        })
        .unwrap();
    mcp.apply().await;
    assert!(bridge.tools_changed(), "deleting is allowed now");
    assert!(!bridge.tools_changed(), "said once");
    settings.update(allowed).unwrap();
    mcp.apply().await;
    assert!(bridge.tools_changed());
    let mut restarted: Endpoint =
        serde_json::from_str(&std::fs::read_to_string(endpoint_file(&config)).unwrap()).unwrap();
    restarted.pid += 1;
    std::fs::write(
        endpoint_file(&config),
        serde_json::to_vec(&restarted).unwrap(),
    )
    .unwrap();
    assert!(bridge.tools_changed(), "another app serves");

    // A session is named by `session`; clients set up when it was `session_id` still work.
    let schema = listed[0]["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "get_session")
        .unwrap()["inputSchema"]
        .clone();
    assert_eq!(schema["required"], json!(["session"]));
    assert!(schema["properties"]["session_id"].is_null());
    let old = agent
        .ok("get_session", json!({"session_id": session_id}))
        .await;
    assert_eq!(old["id"], json!(session_id));

    // A crop-only session: its pictures may go to a folder of the user's, never into the app's own.
    let (refusal, failed) = agent
        .tool(
            "create_session",
            json!({"path": cards.to_string_lossy(), "task": "crop",
                   "crop_folder": dir.path().join("data").join("crops").to_string_lossy(), "start": false}),
        )
        .await;
    assert!(failed, "{refusal}");
    let crops = dir.path().join("cropped");
    let cropping = agent
        .ok(
            "create_session",
            json!({"path": cards.to_string_lossy(), "task": "crop",
                   "crop_folder": crops.to_string_lossy(), "start": false}),
        )
        .await;
    assert_eq!(cropping["task"], "crop");
    assert_eq!(
        cropping["cropsFolder"],
        crops.canonicalize().unwrap().to_string_lossy().as_ref()
    );

    // Prompts.
    let prompts = agent.rpc("prompts/list", json!({})).await;
    assert_eq!(prompts["prompts"][0]["name"], "review_session");
    assert!(prompts["prompts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p["name"] == "crop_session"));
    let prompt = agent
        .rpc(
            "prompts/get",
            json!({"name": "crop_session", "arguments": {"session": "Binder"}}),
        )
        .await;
    let text = prompt["messages"][0]["content"]["text"].as_str().unwrap();
    assert!(
        text.contains("next_cards_to_crop with session \"Binder\"")
            && text.contains("preview_crop"),
        "{text}"
    );
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

/// An agent prices an identified card on eBay: the search goes out with the user's keyset (an application token),
/// in the card's category, fixed price and ungraded; the listings and their range are kept on the card, and only
/// those the agent keeps count. Without a keyset the tool says what to set up.
#[tokio::test(flavor = "multi_thread")]
async fn agent_prices_a_card_on_ebay() {
    use crate::settings::Secret;
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let ebay = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/identity/v1/oauth2/token"))
        // base64("Demo-App:cert")
        .and(header("Authorization", "Basic RGVtby1BcHA6Y2VydA=="))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"access_token": "app", "expires_in": 7200})),
        )
        .expect(1)
        .mount(&ebay)
        .await;
    Mock::given(method("GET"))
        .and(path("/buy/browse/v1/item_summary/search"))
        .and(header("Authorization", "Bearer app"))
        .and(header("X-EBAY-C-MARKETPLACE-ID", "EBAY_US"))
        .and(query_param("category_ids", "183454"))
        .and(query_param(
            "filter",
            "buyingOptions:{FIXED_PRICE},conditionIds:{4000}",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"total": 57, "itemSummaries": [
            {"itemId": "v1|1|0", "title": "Charizard ex 125/197 Obsidian Flames", "price": {"value": "18.00", "currency": "USD"},
             "itemWebUrl": "https://www.ebay.com/itm/1", "condition": "Ungraded", "buyingOptions": ["FIXED_PRICE"]},
            {"itemId": "v1|2|0", "title": "Charizard ex 125/197 NM", "price": {"value": "22.00", "currency": "USD"},
             "itemWebUrl": "https://www.ebay.com/itm/2", "condition": "Ungraded", "buyingOptions": ["FIXED_PRICE"]},
            {"itemId": "v1|3|0", "title": "Pokemon lot 50 cards Charizard ex", "price": {"value": "95.00", "currency": "USD"},
             "itemWebUrl": "https://www.ebay.com/itm/3", "condition": "Ungraded", "buyingOptions": ["FIXED_PRICE"]}]})))
        .mount(&ebay)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let (_mcp, state, agent) =
        app_with_agent(dir.path(), "http://127.0.0.1:9", McpSettings::default()).await;
    let (_, ids) = identified_cards(&state.db, &["Charizard ex"]);
    let card = ids[0];

    let (refusal, failed) = agent
        .tool("find_ebay_listings", json!({"item_id": card}))
        .await;
    assert!(failed);
    assert!(
        refusal.as_str().unwrap().contains("Settings → Selling"),
        "{refusal}"
    );

    state
        .settings
        .update_ebay(|e| {
            e.app_id = "Demo-App".into();
            e.api_base = ebay.uri();
        })
        .unwrap();
    state.settings.set_secret(Secret::EbayCert, "cert").unwrap();
    let found = agent
        .ok("find_ebay_listings", json!({"item_id": card}))
        .await;
    assert_eq!(found["query"], "Charizard ex");
    assert_eq!(found["totalOnEbay"], 57);
    assert_eq!(found["listings"].as_array().unwrap().len(), 3);
    assert_eq!(found["range"]["max"], 95.0);

    let kept = agent
        .ok(
            "keep_ebay_listings",
            json!({"item_id": card, "listing_ids": ["v1|1|0", "v1|2|0"], "note": "left out a lot of 50"}),
        )
        .await;
    assert_eq!(
        (kept["kept"].as_i64(), kept["leftOut"].as_i64()),
        (Some(2), Some(1))
    );
    assert_eq!(kept["range"]["min"], 18.0);
    assert_eq!(kept["range"]["max"], 22.0);
    assert_eq!(kept["range"]["average"], 20.0);

    // The session table's row and the detail panel read it back; a second search reuses the token.
    let row = state.db.item_summary(card).unwrap();
    assert_eq!(row.market_range.as_ref().map(|r| r.count), Some(2));
    let market = state.db.item(card).unwrap().market.unwrap();
    assert_eq!(market.note, "left out a lot of 50");
    assert!(!market.listings[2].counted);
    let listed = agent.ok("list_cards", json!({"session": "latest"})).await;
    assert_eq!(listed["cards"][0]["ebayRange"]["median"], 20.0);
    agent
        .ok(
            "find_ebay_listings",
            json!({"item_id": card, "query": "Charizard ex 125/197"}),
        )
        .await;
    assert_eq!(
        state.db.item(card).unwrap().market.unwrap().query,
        "Charizard ex 125/197",
        "a search starts over: every listing counts again"
    );

    // An agent that searched for the card it sees and then corrects the misread fields keeps those listings;
    // what Cardloader searched from the fields as they were goes with them.
    agent
        .ok(
            "edit_card",
            json!({"item_id": card, "card_number": "199", "note": "number misread"}),
        )
        .await;
    assert!(state.db.item(card).unwrap().market.is_some());
    agent
        .ok("find_ebay_listings", json!({"item_id": card}))
        .await;
    agent
        .ok(
            "edit_card",
            json!({"item_id": card, "name": "Charizard ex SIR", "note": "the special art"}),
        )
        .await;
    assert!(state.db.item(card).unwrap().market.is_none());
    assert!(state.db.item_summary(card).unwrap().market_range.is_none());
}

/// The cards waiting for the agent to identify them, once `n` of them are (scanning the folder takes a moment).
async fn waiting_cards(agent: &Agent, session: &str, n: usize) -> (Vec<Value>, Value) {
    cards_from(agent, "next_cards_to_identify", session, n).await
}

/// The cards `tool` offers the agent, once `n` of them wait.
async fn cards_from(agent: &Agent, tool: &str, session: &str, n: usize) -> (Vec<Value>, Value) {
    let started = Instant::now();
    loop {
        let result = agent
            .rpc(
                "tools/call",
                json!({"name": tool,
                       "arguments": {"session": session, "max": 10, "wait_seconds": 5}}),
            )
            .await;
        assert!(result["isError"] != true, "{result}");
        let text: Value =
            serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
        let cards = text["cards"].as_array().cloned().unwrap_or_default();
        if cards.len() >= n {
            return (cards, result);
        }
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "only {} card(s) came: {text}",
            cards.len()
        );
    }
}

/// An agent identifies a session's cards itself: Cardloader reads the folder and hands each card over with its
/// picture, the answers are stored for the user to review and graded through Ximilar, a card the agent can't tell
/// fails with its note, and no card is identified twice. Stopping hands waiting cards back to the queue.
#[tokio::test(flavor = "multi_thread")]
async fn agent_identifies_a_session() {
    let server = mock_ximilar().await;
    let dir = tempfile::tempdir().unwrap();
    let cards = dir.path().join("football");
    std::fs::create_dir_all(&cards).unwrap();
    for name in ["a", "b"] {
        RgbImage::from_pixel(630, 880, Rgb([20, 90, 160]))
            .save(cards.join(format!("{name}.png")))
            .unwrap();
    }
    let (_mcp, state, agent, events) =
        app_with_events(dir.path(), &server.uri(), McpSettings::default()).await;
    let created = agent
        .ok(
            "create_session",
            json!({"path": cards.to_string_lossy(), "engine": "agent", "card_type": "sport", "grading": "grade"}),
        )
        .await;
    assert_eq!(created["engine"], "agent");
    assert!(created["next"]
        .as_str()
        .unwrap()
        .contains("next_cards_to_identify"));
    let session = created["id"].as_str().unwrap().to_string();

    // Agents pass what the user told them: the session's name, or the copied prompt's "(id …)".
    let name = created["name"].as_str().unwrap().to_string();
    cards_from(&agent, "next_cards_to_identify", &name, 2).await;
    cards_from(
        &agent,
        "next_cards_to_identify",
        &format!("(id {session})"),
        2,
    )
    .await;
    let (waiting, result) = waiting_cards(&agent, &session, 2).await;
    // The session page counts the cards waiting from the session's own updates, not only the cards'.
    assert_eq!(
        events.last_session(&session).map(|s| s.counts.in_progress),
        Some(2),
        "the session's counts follow the cards handed to the agent"
    );
    let images = result["content"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["type"] == "image")
        .count();
    assert_eq!(images, 2, "a front picture per card");
    let item_of = |file: &str| {
        waiting
            .iter()
            .find(|c| c["files"]["front"].as_str().unwrap().ends_with(file))
            .unwrap()["itemId"]
            .as_i64()
            .unwrap()
    };
    let (mahomes, blurry) = (item_of("a.png"), item_of("b.png"));
    let answered = agent
        .ok(
            "identify_cards",
            json!({"cards": [
                {"item_id": mahomes, "name": "Patrick Mahomes II", "set_name": "2017 Panini Prizm",
                 "card_number": "269", "year": "2017", "variant": "Silver Prizm", "game": "football",
                 "certainty": "high", "note": "name, #269 and the silver refractor are clear"},
                {"item_id": blurry, "unidentified": true, "note": "too blurry to read"},
                {"item_id": 999_999, "name": "Nobody"}]}),
        )
        .await;
    let results = answered["results"].as_array().unwrap();
    assert_eq!(results[0]["ok"], true, "{answered}");
    assert_eq!(results[1]["ok"], true, "{answered}");
    assert_eq!(results[2]["ok"], false, "{answered}");
    let again = agent
        .ok(
            "identify_cards",
            json!({"cards": [{"item_id": mahomes, "name": "Someone else"}]}),
        )
        .await;
    assert_eq!(
        again["results"][0]["ok"], false,
        "a card is identified once"
    );

    let started = Instant::now();
    while state.runner.is_running(&session) {
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "run didn't end"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let requests = server.received_requests().await.unwrap();
    assert!(
        !requests
            .iter()
            .any(|r| r.url.path().starts_with("/collectibles/")),
        "no Ximilar identification"
    );
    let card = state.db.item(mahomes).unwrap();
    let s = &card.summary;
    assert_eq!(s.status, crate::model::ItemStatus::Done);
    assert_eq!(
        (
            s.card.name.as_str(),
            s.card.series.as_str(),
            s.card.subcategory.as_str()
        ),
        ("Patrick Mahomes II", "Silver Prizm", "Football")
    );
    assert_eq!(
        s.card.full_name,
        "Patrick Mahomes II 2017 Panini Prizm #269"
    );
    assert_eq!(
        (s.id_type.as_deref(), s.confidence.as_deref()),
        (Some("sport"), Some("agent"))
    );
    assert_eq!(s.grade, Some(7.5), "graded through Ximilar");
    assert_eq!(card.recognition.unwrap()["certainty"], "high");
    let failed = state.db.item_summary(blurry).unwrap();
    assert_eq!(failed.status, crate::model::ItemStatus::Error);
    assert!(failed.error.unwrap().contains("too blurry"));
    assert_eq!(state.db.session(&session).unwrap().counts.to_review, 1);
    let done = agent
        .ok(
            "next_cards_to_identify",
            json!({"session": session, "wait_seconds": 1}),
        )
        .await;
    assert_eq!(done["finished"], true);

    // Stopping a session while cards wait for the agent puts them back in the queue.
    let second = agent
        .ok(
            "create_session",
            json!({"path": cards.to_string_lossy(), "engine": "agent", "name": "Again"}),
        )
        .await;
    let second = second["id"].as_str().unwrap().to_string();
    waiting_cards(&agent, &second, 2).await;
    // A name two sessions share isn't guessed at: the agent is told their ids.
    let twin = agent
        .ok(
            "create_session",
            json!({"path": cards.to_string_lossy(), "engine": "agent", "name": "Again", "start": false}),
        )
        .await;
    let (refusal, failed) = agent
        .tool("next_cards_to_identify", json!({"session": "Again"}))
        .await;
    assert!(failed);
    let refusal = refusal.as_str().unwrap();
    assert!(
        refusal.contains("2 sessions are named") && refusal.contains(twin["id"].as_str().unwrap()),
        "{refusal}"
    );
    agent.ok("stop_session", json!({"session": second})).await;
    let started = Instant::now();
    while state.runner.is_running(&second) {
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "stop didn't end the run"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let items = state.db.items(&second).unwrap();
    assert_eq!(items.len(), 2);
    assert!(items
        .iter()
        .all(|i| i.status == crate::model::ItemStatus::Queued));
    assert!(state.runner.agent_waiting_count(None) == 0);
}

/// An agent prices a session's cards as Ximilar identifies them: each comes with its fields and picture, a price
/// is stored with the agent's note (Ximilar is asked for no price statistics), a card the agent can't price fails
/// only its price, and a price isn't taken as an identification.
#[tokio::test(flavor = "multi_thread")]
async fn agent_prices_a_session() {
    let server = mock_ximilar().await;
    let dir = tempfile::tempdir().unwrap();
    let cards = dir.path().join("pokemon");
    std::fs::create_dir_all(&cards).unwrap();
    for name in ["a", "b"] {
        RgbImage::from_pixel(630, 880, Rgb([20, 90, 160]))
            .save(cards.join(format!("{name}.png")))
            .unwrap();
    }
    let (_mcp, state, agent) =
        app_with_agent(dir.path(), &server.uri(), McpSettings::default()).await;
    let created = agent
        .ok(
            "create_session",
            json!({"path": cards.to_string_lossy(), "engine": "ximilar", "card_type": "tcg", "prices": "agent"}),
        )
        .await;
    assert_eq!(created["prices"], "agent");
    assert!(created["next"]
        .as_str()
        .unwrap()
        .contains("next_cards_to_price"));
    let session = created["id"].as_str().unwrap().to_string();

    let (waiting, result) = cards_from(&agent, "next_cards_to_price", &session, 2).await;
    assert_eq!(waiting[0]["card"]["name"], "Charizard ex");
    // Pictures and Cardloader's tools only, unless the user lets agents search the web; said with every batch.
    let said = |result: &Value| -> Value {
        serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap()
    };
    let batch = said(&result);
    assert_eq!(batch["webSearch"], false);
    assert!(
        batch["next"]
            .as_str()
            .unwrap()
            .contains("don't search the web"),
        "{batch}"
    );
    let mut settings = state.settings.get();
    settings.mcp.allow_web = true;
    state.settings.update(settings).unwrap();
    let (_, result) = cards_from(&agent, "next_cards_to_price", &session, 2).await;
    let batch = said(&result);
    assert_eq!(batch["webSearch"], true);
    assert!(
        batch["next"]
            .as_str()
            .unwrap()
            .contains("you may search the web"),
        "{batch}"
    );
    assert_eq!(waiting[0]["card"]["cardNumber"], "125");
    let (priced, unpriced) = (
        waiting[0]["itemId"].as_i64().unwrap(),
        waiting[1]["itemId"].as_i64().unwrap(),
    );
    let not_identify = agent
        .ok(
            "identify_cards",
            json!({"cards": [{"item_id": priced, "name": "Pikachu"}]}),
        )
        .await;
    assert_eq!(not_identify["results"][0]["ok"], false, "{not_identify}");
    let answered = agent
        .ok(
            "price_cards",
            json!({"cards": [
                {"item_id": priced, "price": 24.999, "note": "PSA 10s ask 22–28"},
                {"item_id": unpriced, "unpriced": true, "note": "no sales of this grade"}]}),
        )
        .await;
    let results = answered["results"].as_array().unwrap();
    assert!(results.iter().all(|r| r["ok"] == true), "{answered}");

    let started = Instant::now();
    while state.runner.is_running(&session) {
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "run didn't end"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let requests = server.received_requests().await.unwrap();
    assert!(
        requests
            .iter()
            .filter(|r| r.url.path().starts_with("/collectibles/"))
            .all(|r| serde_json::from_slice::<Value>(&r.body).unwrap()["price_stats"].is_null()),
        "Ximilar isn't asked for prices"
    );
    let card = state.db.item(priced).unwrap();
    assert_eq!(card.summary.status, crate::model::ItemStatus::Done);
    assert_eq!(card.summary.price, Some(25.0));
    assert_eq!(card.price_note.as_deref(), Some("PSA 10s ask 22–28"));
    let failed = state.db.item_summary(unpriced).unwrap();
    assert_eq!(failed.status, crate::model::ItemStatus::Error);
    assert_eq!(failed.card.name, "Charizard ex", "still identified");
    assert!(
        failed.error.as_deref().unwrap().starts_with("price: "),
        "{:?}",
        failed.error
    );
    let done = agent
        .ok(
            "next_cards_to_price",
            json!({"session": session, "wait_seconds": 1}),
        )
        .await;
    assert_eq!(done["finished"], true);
    let (refusal, failed) = agent
        .tool("next_cards_to_identify", json!({"session": session}))
        .await;
    assert!(failed);
    assert!(refusal.as_str().unwrap().contains("engine"), "{refusal}");
}

/// A session whose cards the agent both identifies and prices: asked only to identify, it is sent on to the
/// cards waiting for its price instead of waiting for cards to identify that can't come, and back again.
#[tokio::test(flavor = "multi_thread")]
async fn an_agent_is_sent_on_to_its_next_task() {
    let server = mock_ximilar().await;
    let dir = tempfile::tempdir().unwrap();
    let cards = dir.path().join("football");
    std::fs::create_dir_all(&cards).unwrap();
    RgbImage::from_pixel(630, 880, Rgb([20, 90, 160]))
        .save(cards.join("a.png"))
        .unwrap();
    let (_mcp, state, agent) =
        app_with_agent(dir.path(), &server.uri(), McpSettings::default()).await;
    let created = agent
        .ok(
            "create_session",
            json!({"path": cards.to_string_lossy(), "engine": "agent", "card_type": "sport",
                   "prices": "agent", "grading": "off"}),
        )
        .await;
    let next = created["next"].as_str().unwrap();
    assert!(
        next.contains("next_cards_to_identify") && next.contains("next_cards_to_price"),
        "{next}"
    );
    let session = created["id"].as_str().unwrap().to_string();
    let (waiting, _) = cards_from(&agent, "next_cards_to_identify", &session, 1).await;
    let item_id = waiting[0]["itemId"].as_i64().unwrap();
    // Nothing waits for a price yet: the tool for it says where the card is instead of waiting.
    let early = agent
        .ok(
            "next_cards_to_price",
            json!({"session": session, "wait_seconds": 30}),
        )
        .await;
    assert_eq!(early["finished"], false);
    assert!(
        early["next"]
            .as_str()
            .unwrap()
            .contains("call next_cards_to_identify"),
        "{early}"
    );
    let answered = agent
        .ok(
            "identify_cards",
            json!({"cards": [{"item_id": item_id, "name": "Patrick Mahomes II", "set_name": "2017 Panini Prizm",
                              "card_number": "269", "game": "football"}]}),
        )
        .await;
    assert_eq!(answered["results"][0]["ok"], true, "{answered}");
    assert!(
        answered["next"]
            .as_str()
            .unwrap()
            .contains("next_cards_to_identify"),
        "{answered}"
    );
    // The card goes on to its price stage; the identify loop, called again, names the tool for it at once.
    let started = Instant::now();
    let on = loop {
        let on = agent
            .ok(
                "next_cards_to_identify",
                json!({"session": session, "wait_seconds": 5}),
            )
            .await;
        if on["next"].as_str().unwrap().contains("next_cards_to_price") {
            break on;
        }
        assert!(started.elapsed() < Duration::from_secs(30), "{on}");
    };
    assert_eq!(on["finished"], false);
    assert!(
        on["next"]
            .as_str()
            .unwrap()
            .contains("1 card(s) wait for your price"),
        "{on}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(4),
        "said without waiting out the call"
    );
    let (to_price, _) = cards_from(&agent, "next_cards_to_price", &session, 1).await;
    assert_eq!(to_price[0]["card"]["name"], "Patrick Mahomes II");
    let priced = agent
        .ok(
            "price_cards",
            json!({"cards": [{"item_id": item_id, "price": 120.0, "note": "recent raw sales"}]}),
        )
        .await;
    assert_eq!(priced["results"][0]["ok"], true, "{priced}");
    let started = Instant::now();
    while state.runner.is_running(&session) {
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "run didn't end"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    for tool in ["next_cards_to_identify", "next_cards_to_price"] {
        let done = agent
            .ok(tool, json!({"session": session, "wait_seconds": 1}))
            .await;
        assert_eq!(done["finished"], true, "{tool}: {done}");
    }
    assert_eq!(state.db.item_summary(item_id).unwrap().price, Some(120.0));
}

/// Prices from eBay's listings with the agent saying which of them are the card (`ebay_agent`): Cardloader
/// searches, the card comes alone with its listings and small pictures of them, larger ones on request, and the
/// answer is the listings kept: their median is the price. A price of the agent's own isn't what was asked.
#[tokio::test(flavor = "multi_thread")]
async fn agent_keeps_the_listings_that_are_the_card() {
    use crate::runner::tests::mock_browse;
    use crate::settings::Secret;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};
    let server = mock_ximilar().await;
    // eBay's picture host: a listing's picture at the size the search gives, and one of them larger too.
    let pictures = MockServer::start().await;
    let jpeg = |edge: u32| {
        let img = image::DynamicImage::ImageRgb8(RgbImage::from_pixel(
            edge * 5 / 7,
            edge,
            Rgb([200, 60, 40]),
        ));
        crate::imaging::jpeg_bytes(&img, 85).unwrap()
    };
    for (at, edge) in [
        ("/g/one/s-l225.jpg", 225),
        ("/g/one/s-l960.jpg", 960),
        ("/g/two/s-l225.jpg", 225),
    ] {
        Mock::given(method("GET"))
            .and(path(at))
            .respond_with(ResponseTemplate::new(200).set_body_raw(jpeg(edge), "image/jpeg"))
            .mount(&pictures)
            .await;
    }
    let listing = |id: &str, title: &str, price: &str, picture: Option<&str>| {
        let mut l = json!({"itemId": id, "title": title, "price": {"value": price, "currency": "USD"},
            "itemWebUrl": "https://www.ebay.com/itm/1", "condition": "Ungraded", "buyingOptions": ["FIXED_PRICE"]});
        if let Some(at) = picture {
            l["image"] = json!({"imageUrl": format!("{}{at}", pictures.uri())});
        }
        l
    };
    let ebay = mock_browse(
        200,
        json!({"total": 4, "itemSummaries": [
            listing("v1|1|0", "Charizard ex 125/197 Obsidian Flames", "20.00", Some("/g/one/s-l225.jpg")),
            listing("v1|2|0", "Charizard ex 125/197 holo", "26.00", Some("/g/two/s-l225.jpg")),
            listing("v1|3|0", "Obsidian Flames lot of 30 cards", "400.00", None),
            // Not eBay's picture host: never fetched.
            listing("v1|4|0", "Charizard ex 125 proxy", "3.00", None)]}),
    )
    .await;
    let dir = tempfile::tempdir().unwrap();
    let cards = dir.path().join("pokemon");
    std::fs::create_dir_all(&cards).unwrap();
    RgbImage::from_pixel(630, 880, Rgb([20, 90, 160]))
        .save(cards.join("a.png"))
        .unwrap();
    let (_mcp, state, agent) =
        app_with_agent(dir.path(), &server.uri(), McpSettings::default()).await;
    state
        .settings
        .update_ebay(|e| {
            e.app_id = "Demo-App".into();
            e.api_base = ebay.uri();
        })
        .unwrap();
    state.settings.set_secret(Secret::EbayCert, "cert").unwrap();
    let created = agent
        .ok(
            "create_session",
            json!({"path": cards.to_string_lossy(), "engine": "ximilar", "card_type": "tcg",
                   "prices": "ebay_agent"}),
        )
        .await;
    assert_eq!(created["prices"], "ebay_agent");
    assert!(created["next"]
        .as_str()
        .unwrap()
        .contains("next_cards_to_price"));
    let session = created["id"].as_str().unwrap().to_string();

    let (waiting, result) = cards_from(&agent, "next_cards_to_price", &session, 1).await;
    let card = &waiting[0];
    let item_id = card["itemId"].as_i64().unwrap();
    assert_eq!(card["card"]["name"], "Charizard ex");
    let listings = card["listings"].as_array().unwrap();
    assert_eq!(listings.len(), 4);
    assert_eq!(
        listings.iter().map(|l| &l["picture"]).collect::<Vec<_>>(),
        [&json!(true), &json!(true), &json!(false), &json!(false)],
        "{card}"
    );
    assert_eq!(
        (&listings[1]["id"], &listings[1]["price"]),
        (&json!("v1|2|0"), &json!(26.0))
    );
    let content = result["content"].as_array().unwrap();
    let said: Value = serde_json::from_str(content[0]["text"].as_str().unwrap()).unwrap();
    assert!(
        said["next"].as_str().unwrap().contains("listing_ids")
            && said["next"]
                .as_str()
                .unwrap()
                .contains("don't search the web"),
        "{said}"
    );
    // The card's own picture, then the two listings that have one, each named.
    let images = content.iter().filter(|c| c["type"] == "image").count();
    assert_eq!(
        images,
        3,
        "{:?}",
        content.iter().map(|c| &c["type"]).collect::<Vec<_>>()
    );
    assert!(content.iter().any(|c| c["text"]
        .as_str()
        .is_some_and(|t| t.starts_with("Listing v1|2|0 (160×225 px): Charizard ex 125/197 holo"))));

    // Larger pictures on request: the size asked of eBay where it has one, else the one it gave.
    let larger = agent
        .rpc(
            "tools/call",
            json!({"name": "listing_pictures",
                   "arguments": {"item_id": item_id, "listing_ids": ["v1|1|0", "v1|2|0", "v1|3|0"]}}),
        )
        .await;
    let content = larger["content"].as_array().unwrap();
    let shown: Value = serde_json::from_str(content[0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(
        shown["pictures"],
        json!([
            {"id": "v1|1|0", "title": "Charizard ex 125/197 Obsidian Flames", "width": 571, "height": 800},
            {"id": "v1|2|0", "title": "Charizard ex 125/197 holo", "width": 160, "height": 225},
            {"id": "v1|3|0", "title": "Obsidian Flames lot of 30 cards", "error": "the listing has no picture"}]),
    );
    assert_eq!(content.iter().filter(|c| c["type"] == "image").count(), 2);
    let (refusal, failed) = agent
        .tool(
            "listing_pictures",
            json!({"item_id": item_id, "listing_ids": ["v1|9|0"]}),
        )
        .await;
    assert!(
        failed && refusal.as_str().unwrap().contains("v1|9|0"),
        "{refusal}"
    );

    // A price of its own, or a listing the search didn't find, isn't an answer: the card goes on waiting.
    let refused = agent
        .ok(
            "price_cards",
            json!({"cards": [{"item_id": item_id, "price": 23.0, "note": "about that"}]}),
        )
        .await;
    assert_eq!(refused["results"][0]["ok"], false);
    assert!(
        refused["results"][0]["error"]
            .as_str()
            .unwrap()
            .contains("listing_ids"),
        "{refused}"
    );
    let unknown = agent
        .ok(
            "price_cards",
            json!({"cards": [{"item_id": item_id, "listing_ids": ["v1|1|0", "v1|9|0"]}]}),
        )
        .await;
    assert!(
        unknown["results"][0]["error"]
            .as_str()
            .unwrap()
            .contains("v1|9|0"),
        "{unknown}"
    );
    assert_eq!(unknown["stillWaiting"], 1);
    let kept = agent
        .ok(
            "price_cards",
            json!({"cards": [{"item_id": item_id, "listing_ids": ["v1|1|0", "v1|2|0"],
                              "note": "left out a lot and a proxy"}]}),
        )
        .await;
    assert_eq!(kept["results"][0]["ok"], true, "{kept}");
    let wait = async || {
        let started = Instant::now();
        while state.runner.is_running(&session) {
            assert!(
                started.elapsed() < Duration::from_secs(30),
                "run didn't end"
            );
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    };
    wait().await;
    let priced = state.db.item(item_id).unwrap();
    assert_eq!(priced.summary.status, crate::model::ItemStatus::Done);
    assert_eq!(priced.summary.price, Some(23.0), "the median of 20 and 26");
    assert_eq!(
        priced.price_note.as_deref(),
        Some("left out a lot and a proxy")
    );
    let market = priced.market.unwrap();
    assert_eq!(market.note, "left out a lot and a proxy");
    assert_eq!(
        market
            .listings
            .iter()
            .map(|l| l.counted)
            .collect::<Vec<_>>(),
        [true, true, false, false]
    );

    // Added to the finished card: it is searched again and waits for the agent again; none of the listings being
    // the card is an answer too, and leaves the price it had.
    let added = agent
        .ok(
            "add_to_cards",
            json!({"session": session, "prices": "ebay_agent"}),
        )
        .await;
    assert!(
        added["next"]
            .as_str()
            .unwrap()
            .contains("next_cards_to_price"),
        "{added}"
    );
    let (again, _) = cards_from(&agent, "next_cards_to_price", &session, 1).await;
    assert_eq!(again[0]["listings"].as_array().unwrap().len(), 4);
    let none = agent
        .ok(
            "price_cards",
            json!({"cards": [{"item_id": item_id, "listing_ids": [], "note": "none is this print"}]}),
        )
        .await;
    assert_eq!(none["results"][0]["ok"], true, "{none}");
    wait().await;
    let left = state.db.item(item_id).unwrap();
    assert_eq!(
        (left.summary.status, left.summary.price),
        (crate::model::ItemStatus::Done, Some(23.0))
    );
    assert!(left.summary.market_range.is_none());
    assert_eq!(left.market.unwrap().note, "none is this print");
}

/// Prices and grading added to a finished session's cards: they go through those stages alone (no second
/// identification, an accepted card stays accepted), the session keeps the tasks, and Ximilar's prices, which
/// only come with its identification, are refused.
#[tokio::test(flavor = "multi_thread")]
async fn agent_adds_prices_and_grading_to_a_finished_session() {
    use crate::runner::tests::mock_browse;
    use crate::settings::Secret;
    let server = mock_ximilar().await;
    let listing = |id: &str, price: &str| {
        json!({"itemId": id, "title": "Charizard ex 125/197", "price": {"value": price, "currency": "USD"},
               "itemWebUrl": "https://www.ebay.com/itm/1", "condition": "Ungraded", "buyingOptions": ["FIXED_PRICE"]})
    };
    let ebay = mock_browse(
        200,
        json!({"total": 3, "itemSummaries": [listing("1", "10.00"), listing("2", "30.00"), listing("3", "14.00")]}),
    )
    .await;
    let dir = tempfile::tempdir().unwrap();
    let cards = dir.path().join("cards");
    std::fs::create_dir_all(&cards).unwrap();
    for name in ["a", "b"] {
        RgbImage::from_pixel(630, 880, Rgb([20, 90, 160]))
            .save(cards.join(format!("{name}.png")))
            .unwrap();
    }
    let (_mcp, state, agent) =
        app_with_agent(dir.path(), &server.uri(), McpSettings::default()).await;
    let created = agent
        .ok(
            "create_session",
            json!({"path": cards.to_string_lossy(), "engine": "ximilar", "card_type": "tcg", "grading": "off"}),
        )
        .await;
    let session = created["id"].as_str().unwrap().to_string();
    let finished = |state: &AppState| !state.runner.is_running(&session);
    let started = Instant::now();
    while !finished(&state) {
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "run didn't end"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let ids: Vec<i64> = state
        .db
        .items(&session)
        .unwrap()
        .iter()
        .map(|i| i.id)
        .collect();
    agent
        .ok("accept_cards", json!({"item_ids": [ids[0]]}))
        .await;
    let identifications = |requests: &[wiremock::Request]| {
        requests
            .iter()
            .filter(|r| r.url.path().starts_with("/collectibles/"))
            .count()
    };
    let before = identifications(&server.received_requests().await.unwrap());

    let (refusal, failed) = agent
        .tool(
            "add_to_cards",
            json!({"session": session, "prices": "ximilar"}),
        )
        .await;
    assert!(
        failed
            && refusal
                .as_str()
                .unwrap()
                .contains("only with its identification"),
        "{refusal}"
    );
    let (refusal, failed) = agent
        .tool(
            "add_to_cards",
            json!({"session": session, "prices": "ebay"}),
        )
        .await;
    assert!(
        failed && refusal.as_str().unwrap().contains("eBay keyset"),
        "{refusal}"
    );

    state
        .settings
        .update_ebay(|e| {
            e.app_id = "Demo-App".into();
            e.api_base = ebay.uri();
        })
        .unwrap();
    state.settings.set_secret(Secret::EbayCert, "cert").unwrap();
    crate::local::testing::install_fake(
        &state.runner.local,
        r#"{"corners": 9, "edges": 8.5, "surface": 7, "centering": 9}"#,
    );
    let added = agent
        .ok(
            "add_to_cards",
            json!({"session": session, "prices": "ebay", "grading": "grade", "grade_engine": "local"}),
        )
        .await;
    assert_eq!(added["cardsAdded"], 2, "{added}");
    let started = Instant::now();
    while !finished(&state) {
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "run didn't end"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    assert_eq!(
        identifications(&server.received_requests().await.unwrap()),
        before,
        "no card is identified again"
    );
    let items = state.db.items(&session).unwrap();
    for item in &items {
        assert_eq!(item.status, crate::model::ItemStatus::Done, "{item:?}");
        assert_eq!(item.card.name, "Charizard ex");
        assert_eq!(item.price, Some(14.0), "the listings' median");
        assert_eq!(item.grade, Some(8.0), "graded by the local model");
    }
    assert!(items[0].reviewed, "an accepted card stays accepted");
    let options = state.db.session(&session).unwrap().options;
    assert_eq!(
        (options.prices(), options.grade_engine(), options.grading),
        (Prices::Ebay, Engine::Local, GradingMode::Grade),
        "the session's later cards get them too"
    );

    // Crops too, here at corners the agent finds: one card, which keeps everything it has.
    let crops = dir.path().join("crops");
    let added = agent
        .ok(
            "add_to_cards",
            json!({"session": session, "item_ids": [ids[1]], "crops": true, "crop_engine": "agent",
                   "crop_folder": crops.to_string_lossy(), "crop_format": "png"}),
        )
        .await;
    assert_eq!(added["cardsAdded"], 1, "{added}");
    assert_eq!(added["cropEngine"], "agent");
    assert!(
        added["next"]
            .as_str()
            .unwrap()
            .contains("next_cards_to_crop"),
        "{added}"
    );
    let (waiting, _) = cards_from(&agent, "next_cards_to_crop", &session, 1).await;
    assert_eq!(waiting.len(), 1);
    assert_eq!(waiting[0]["itemId"], ids[1]);
    assert_eq!(
        waiting[0]["pictures"]["front"],
        json!({"width": 630, "height": 880})
    );
    let cropped = agent
        .ok(
            "crop_cards",
            json!({"cards": [{"item_id": ids[1], "front": {"size": [630, 880],
                   "corners": [[30, 40], [600, 40], [600, 840], [30, 840]]}}]}),
        )
        .await;
    assert_eq!(cropped["results"][0]["ok"], true, "{cropped}");
    let started = Instant::now();
    while !finished(&state) {
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "run didn't end"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(
        identifications(&server.received_requests().await.unwrap()),
        before,
        "no card is identified again"
    );
    let item = state.db.item(ids[1]).unwrap();
    assert_eq!(
        item.summary.status,
        crate::model::ItemStatus::Done,
        "{:?}",
        item.summary
    );
    assert_eq!(item.summary.card.name, "Charizard ex");
    assert_eq!(
        (item.summary.price, item.summary.grade),
        (Some(14.0), Some(8.0))
    );
    assert_eq!(item.crop_images.len(), 1);
    let cut = image::open(&item.crop_images[0]).unwrap();
    assert_eq!((cut.width(), cut.height()), (570, 800));
    assert!(Path::new(&item.crop_images[0]).starts_with(crops.canonicalize().unwrap()));
    assert!(state.db.item(ids[0]).unwrap().crop_images.is_empty());
    let options = state.db.session(&session).unwrap().options;
    assert!(options.crops_by_agent() && options.grades_locally());
}

/// Clients refuse a tool result over 1 MB (Claude Desktop: "Tool result is too large"), and card pictures are most
/// of one. A batch whose pictures don't fit holds fewer cards (the others come with the next call), and its
/// pictures are made coarser or smaller until they do; the sizes said with them are the ones sent.
#[tokio::test(flavor = "multi_thread")]
async fn agents_get_pictures_that_fit_a_result() {
    use base64::Engine as _;
    let server = mock_ximilar().await;
    let dir = tempfile::tempdir().unwrap();
    let cards = dir.path().join("football");
    std::fs::create_dir_all(&cards).unwrap();
    // Three cards, front and back, grainy all over: pictures that compress badly, as foil and print do.
    let mut seed = 11u32;
    for name in ["a1", "a2", "b1", "b2", "c1", "c2"] {
        RgbImage::from_fn(1000, 1400, |x, y| {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let grain = (seed >> 24) as u8 / 3;
            Rgb([(x / 8) as u8 + grain, (y / 10) as u8 + grain, 60 + grain])
        })
        .save(cards.join(format!("{name}.png")))
        .unwrap();
    }
    let (_mcp, state, agent) =
        app_with_agent(dir.path(), &server.uri(), McpSettings::default()).await;
    let created = agent
        .ok(
            "create_session",
            json!({"path": cards.to_string_lossy(), "engine": "agent", "card_type": "sport",
                   "pairing": "front_back", "prices": "off"}),
        )
        .await;
    let session = created["id"].as_str().unwrap().to_string();
    let started = Instant::now();
    while state.runner.agent_waiting(None, AgentTask::Identify).len() < 3 {
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "cards didn't come"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // What a result holds: its cards, how many wait, and each picture with the size its label says.
    let batch = |arguments: Value| {
        let agent = &agent;
        async move {
            let name = if arguments.get("item_id").is_some() {
                "get_card"
            } else {
                "next_cards_to_identify"
            };
            let result = agent
                .rpc("tools/call", json!({"name": name, "arguments": arguments}))
                .await;
            assert!(result["isError"] != true, "{result}");
            assert!(
                result.to_string().len() < 1_000_000,
                "{} bytes",
                result.to_string().len()
            );
            let content = result["content"].as_array().unwrap().clone();
            let text: Value = serde_json::from_str(content[0]["text"].as_str().unwrap()).unwrap();
            let mut pictures = Vec::new();
            for (n, block) in content.iter().enumerate() {
                if block["type"] != "image" {
                    continue;
                }
                let data = block["data"].as_str().unwrap();
                let jpeg = base64::engine::general_purpose::STANDARD
                    .decode(data)
                    .unwrap();
                let picture = image::load_from_memory(&jpeg).unwrap();
                let label = content[n - 1]["text"].as_str().unwrap();
                let size = format!("({}×{} px)", picture.width(), picture.height());
                assert!(label.contains(&size), "{label} for {size}");
                pictures.push((data.len(), picture.height()));
            }
            (text, pictures)
        }
    };
    let total = |pictures: &[(usize, u32)]| pictures.iter().map(|(len, _)| len).sum::<usize>();

    // Three cards asked for: six such pictures don't fit, four do, made to.
    let (text, pictures) = batch(json!({"session": session, "max": 3})).await;
    assert_eq!(text["cards"].as_array().unwrap().len(), 2, "{text}");
    assert_eq!(text["waiting"], 3);
    assert_eq!(pictures.len(), 4);
    assert!(total(&pictures) <= 750_000, "{pictures:?}");
    assert!(pictures.iter().all(|(_, h)| *h >= 512), "{pictures:?}");
    let first: Vec<i64> = text["cards"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["itemId"].as_i64().unwrap())
        .collect();

    // One card at a time, the default: the same budget for two pictures, so they come larger or finer.
    let (text, one) = batch(json!({"session": session})).await;
    assert_eq!(text["cards"].as_array().unwrap().len(), 1);
    assert_eq!(one.len(), 2);
    assert!(total(&one) <= 750_000, "{one:?}");
    assert!(one[0].0 > pictures[0].0, "{one:?} after {pictures:?}");

    // The cards left out come once the first are answered.
    let identified: Vec<Value> = first
        .iter()
        .map(|id| json!({"item_id": id, "name": "Demo Player"}))
        .collect();
    agent
        .ok("identify_cards", json!({"cards": identified}))
        .await;
    let (text, rest) = batch(json!({"session": session, "max": 10})).await;
    assert_eq!(text["cards"].as_array().unwrap().len(), 1, "{text}");
    assert!(!first.contains(&text["cards"][0]["itemId"].as_i64().unwrap()));
    assert_eq!(rest.len(), 2);

    // A card looked at on its own fits too.
    let (_, card) = batch(json!({"item_id": first[0]})).await;
    assert_eq!(card.len(), 2);
    assert!(total(&card) <= 750_000, "{card:?}");
    state.runner.stop(&session).unwrap();

    // Pictures that fit as they are aren't touched.
    let plain = dir.path().join("plain");
    std::fs::create_dir_all(&plain).unwrap();
    for name in ["a", "b", "c", "d", "e", "f"] {
        RgbImage::from_pixel(630, 880, Rgb([20, 90, 160]))
            .save(plain.join(format!("{name}.png")))
            .unwrap();
    }
    let created = agent
        .ok(
            "create_session",
            json!({"path": plain.to_string_lossy(), "engine": "agent", "prices": "off"}),
        )
        .await;
    let session = created["id"].as_str().unwrap().to_string();
    let (waiting, _) = cards_from(&agent, "next_cards_to_identify", &session, 6).await;
    assert_eq!(waiting.len(), 6);
    let (_, pictures) = batch(json!({"session": session, "max": 10})).await;
    assert_eq!(pictures.len(), 6);
    assert!(pictures.iter().all(|(_, h)| *h == 880), "{pictures:?}");
}

/// An agent crops a session's cards: Cardloader hands each card over with its pictures and their size, the agent
/// says where the card's corners are (looking at the cut first, if it likes), and Cardloader cuts the card out
/// of its scan, straightened. Nothing calls Ximilar. An answer that can't be cut leaves the card waiting, a card
/// with a back needs an answer about it, and a picture with no card fails the card with the agent's note.
#[tokio::test(flavor = "multi_thread")]
async fn agent_crops_a_session() {
    use crate::crop::tests::{card_on_table, quarters};
    let server = mock_ximilar().await;
    let dir = tempfile::tempdir().unwrap();
    let cards = dir.path().join("table");
    std::fs::create_dir_all(&cards).unwrap();
    // Two cards, front and back: lying tilted, the second card's front upside down.
    let lying = [("a1", 20.0), ("a2", -15.0), ("b1", 180.0), ("b2", 0.0)];
    let mut corners = Vec::new();
    for (name, degrees) in lying {
        let (picture, at) = card_on_table(400, 300, degrees);
        picture.save(cards.join(format!("{name}.png"))).unwrap();
        corners.push(at);
    }
    let crops = dir.path().join("cut");
    let (_mcp, state, agent) =
        app_with_agent(dir.path(), &server.uri(), McpSettings::default()).await;
    let created = agent
        .ok(
            "create_session",
            json!({"path": cards.to_string_lossy(), "task": "crop", "crop_engine": "agent",
                   "pairing": "front_back", "crop_folder": crops.to_string_lossy(), "crop_format": "png"}),
        )
        .await;
    assert_eq!(created["cropEngine"], "agent");
    assert!(
        created["next"]
            .as_str()
            .unwrap()
            .contains("next_cards_to_crop"),
        "{created}"
    );
    let session = created["id"].as_str().unwrap().to_string();

    let (waiting, result) = cards_from(&agent, "next_cards_to_crop", &session, 2).await;
    let (a, b) = (
        waiting[0]["itemId"].as_i64().unwrap(),
        waiting[1]["itemId"].as_i64().unwrap(),
    );
    // Each side's picture with the size its corners are given in.
    let size = json!({"width": 400, "height": 300});
    assert_eq!(waiting[0]["pictures"], json!({"front": size, "back": size}));
    let content = result["content"].as_array().unwrap();
    assert_eq!(content.iter().filter(|c| c["type"] == "image").count(), 4);
    assert_eq!(
        content[1]["text"],
        format!("Front of card {a} (400×300 px):")
    );
    let listed = agent.ok("get_session", json!({"session": session})).await;
    assert_eq!(listed["counts"]["waitingForCrop"], 2);
    assert_eq!(state.db.session(&session).unwrap().counts.cropping, 2);

    let side = |n: usize| json!({"size": [400, 300], "corners": corners[n]});
    let sent = |answer: Value| {
        let agent = &agent;
        async move { agent.ok("crop_cards", json!({"cards": [answer]})).await["results"][0].clone() }
    };
    // What can't be cut is refused, and the card goes on waiting.
    let refused = [
        (json!({"item_id": a, "front": side(0)}), "back picture"),
        (json!({"item_id": a, "back": side(1)}), "front's corners"),
        (
            json!({"item_id": a, "front": {"size": [300, 400], "corners": corners[0]}, "back": side(1)}),
            "shape of the picture",
        ),
        (
            json!({"item_id": a, "back": side(1), "front": {"size": [400, 300],
                   "corners": [corners[0][1], corners[0][0], corners[0][3], corners[0][2]]}}),
            "counter-clockwise",
        ),
        (
            json!({"item_id": a, "back": side(1), "front": {"size": [400, 300],
                   "corners": [[10, 10], [900, 10], [900, 290], [10, 290]]}}),
            "outside the picture",
        ),
    ];
    for (answer, why) in refused {
        let result = sent(answer).await;
        assert_eq!(result["ok"], false, "{why}: {result}");
        assert!(result["error"].as_str().unwrap().contains(why), "{result}");
    }
    let identified = agent
        .ok(
            "identify_cards",
            json!({"cards": [{"item_id": a, "name": "Pikachu"}]}),
        )
        .await;
    assert!(
        identified["results"][0]["error"]
            .as_str()
            .unwrap()
            .contains("waits to be cropped, not identified"),
        "{identified}"
    );
    assert_eq!(state.runner.agent_waiting(None, AgentTask::Crop).len(), 2);
    // It waits for corners found on the picture as it is: that picture isn't turned meanwhile.
    assert!(state.runner.turn_item(a).is_err());

    // The cut the corners give, to look at: nothing is saved.
    let preview = agent
        .rpc(
            "tools/call",
            json!({"name": "preview_crop", "arguments": {"item_id": a, "size": [400, 300], "corners": corners[0]}}),
        )
        .await;
    assert!(preview["isError"] != true, "{preview}");
    let said: Value =
        serde_json::from_str(preview["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(
        (&said["side"], &said["saved"]),
        (&json!("front"), &json!(false))
    );
    assert!(said["width"].as_u64().unwrap().abs_diff(100) <= 1, "{said}");
    assert!(
        said["height"].as_u64().unwrap().abs_diff(140) <= 1,
        "{said}"
    );
    assert_eq!(preview["content"][1]["mimeType"], "image/jpeg");
    assert!(state.db.item(a).unwrap().crop_images.is_empty());
    assert!(!crops.join("a1.png").exists());

    // The first card: its back in a frame twice the picture's size (a client may show it enlarged). The
    // second: its front lies upside down, its back has no card.
    let doubled: Vec<Value> = corners[1]
        .iter()
        .map(|[x, y]| json!([x * 2.0, y * 2.0]))
        .collect();
    let answers = agent
        .ok(
            "crop_cards",
            json!({"cards": [
                {"item_id": a, "front": side(0), "back": {"size": [800, 600], "corners": doubled},
                 "note": "both sides clear"},
                {"item_id": b, "front": side(2), "no_back": "only a table"},
            ]}),
        )
        .await;
    assert_eq!(answers["results"][0]["ok"], true, "{answers}");
    assert_eq!(answers["results"][1]["ok"], true, "{answers}");
    assert_eq!(answers["stillWaiting"], 0);
    let again = sent(json!({"item_id": a, "front": side(0), "back": side(1)})).await;
    assert!(
        again["error"].as_str().unwrap().contains("isn't waiting"),
        "{again}"
    );
    let started = Instant::now();
    while state.runner.is_running(&session) {
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "run didn't end"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let colours = [[220, 30, 30], [30, 200, 30], [30, 30, 220], [240, 240, 240]];
    let upright = |file: &str| {
        let cut = image::open(file).unwrap();
        assert!(
            cut.width().abs_diff(100) <= 1 && cut.height().abs_diff(140) <= 1,
            "{file}: {}×{}",
            cut.width(),
            cut.height()
        );
        for (got, want) in quarters(&cut).into_iter().zip(colours) {
            assert!(
                got.iter().zip(want).all(|(g, w)| g.abs_diff(w) < 12),
                "{file}: {got:?} for {want:?}"
            );
        }
    };
    let first = state.db.item(a).unwrap();
    assert_eq!(first.summary.status, crate::model::ItemStatus::Done);
    let folder = crops.canonicalize().unwrap();
    let file = |name: &str| folder.join(name).to_string_lossy().into_owned();
    assert_eq!(first.crop_images, [file("a1.png"), file("a2.png")]);
    first.crop_images.iter().for_each(|f| upright(f));
    let record = first.crop.clone().unwrap();
    assert_eq!(record["endpoint"], "crop");
    assert_eq!(record["engine"], "agent");
    assert_eq!(record["note"], "both sides clear");
    assert_eq!(record["response"]["records"][1]["_id"], "back");
    assert_eq!(first.calls, 0);

    // Upside down on the table, upright in its picture: the agent started at the card's own top-left.
    let second = state.db.item(b).unwrap();
    assert_eq!(second.crop_images, [file("b1.png")]);
    upright(&second.crop_images[0]);
    assert_eq!(second.summary.status, crate::model::ItemStatus::Error);
    assert_eq!(
        second.summary.error.as_deref(),
        Some("crop: back: only a table")
    );

    let (finished, _) = agent
        .tool("next_cards_to_crop", json!({"session": session}))
        .await;
    assert_eq!(finished["finished"], true, "{finished}");
    let (refusal, failed) = agent
        .tool("next_cards_to_identify", json!({"session": session}))
        .await;
    assert!(
        failed && refusal.as_str().unwrap().contains("engine"),
        "{refusal}"
    );
    assert!(
        server.received_requests().await.unwrap().is_empty(),
        "an agent's crops need nothing of Ximilar"
    );

    // A picture with no card in it fails the card with what the agent says.
    agent
        .ok("retry_cards", json!({"session": session, "item_ids": [b]}))
        .await;
    cards_from(&agent, "next_cards_to_crop", &session, 1).await;
    let none = sent(json!({"item_id": b, "uncropped": true, "note": "no card here"})).await;
    assert_eq!(none["ok"], true, "{none}");
    let started = Instant::now();
    while state.runner.is_running(&session) {
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "run didn't end"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let second = state.db.item(b).unwrap();
    assert_eq!(second.summary.status, crate::model::ItemStatus::Error);
    assert_eq!(
        second.summary.error.as_deref(),
        Some("AI agent: no card here")
    );
}

/// Prices from another eBay site: the search goes to that site with eBay's suggested category, is asked again
/// without the condition when that leaves nothing, and the card's price keeps the site's currency, apart from
/// US dollars (totals by currency; USD-only destinations don't take it).
#[tokio::test(flavor = "multi_thread")]
async fn prices_from_another_ebay_site() {
    use crate::settings::Secret;
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let ebay = MockServer::start().await;
    let ok = |body: Value| ResponseTemplate::new(200).set_body_json(body);
    Mock::given(method("POST"))
        .and(path("/identity/v1/oauth2/token"))
        .respond_with(ok(json!({"access_token": "app", "expires_in": 7200})))
        .mount(&ebay)
        .await;
    Mock::given(method("GET"))
        .and(path("/commerce/taxonomy/v1/get_default_category_tree_id"))
        .and(query_param("marketplace_id", "EBAY_DE"))
        .respond_with(ok(json!({"categoryTreeId": "77"})))
        .mount(&ebay)
        .await;
    Mock::given(method("GET"))
        .and(path(
            "/commerce/taxonomy/v1/category_tree/77/get_category_suggestions",
        ))
        .respond_with(ok(json!({"categorySuggestions": [
            {"category": {"categoryId": "2536", "categoryName": "Einzelkarten"}}]})))
        .mount(&ebay)
        .await;
    let search = || {
        Mock::given(method("GET"))
            .and(path("/buy/browse/v1/item_summary/search"))
            .and(header("X-EBAY-C-MARKETPLACE-ID", "EBAY_DE"))
            .and(query_param("category_ids", "2536"))
    };
    search()
        .and(query_param(
            "filter",
            "buyingOptions:{FIXED_PRICE},conditionIds:{4000}",
        ))
        .respond_with(ok(json!({"total": 0})))
        .mount(&ebay)
        .await;
    let listing = |id: &str, price: &str| {
        json!({"itemId": id, "title": "Glurak ex 125/197", "price": {"value": price, "currency": "EUR"},
               "itemWebUrl": "https://www.ebay.de/itm/1", "buyingOptions": ["FIXED_PRICE"]})
    };
    search()
        .and(query_param("filter", "buyingOptions:{FIXED_PRICE}"))
        .respond_with(ok(json!({"total": 3, "itemSummaries": [
            listing("1", "10.00"), listing("2", "30.00"), listing("3", "14.00")]})))
        .mount(&ebay)
        .await;

    let dir = tempfile::tempdir().unwrap();
    let (_mcp, state, agent) =
        app_with_agent(dir.path(), "http://127.0.0.1:9", McpSettings::default()).await;
    let (session, ids) = identified_cards(&state.db, &["Charizard ex"]);
    let card = ids[0];
    state
        .settings
        .update_ebay(|e| {
            e.app_id = "Demo-App".into();
            e.api_base = ebay.uri();
        })
        .unwrap();
    state.settings.set_secret(Secret::EbayCert, "cert").unwrap();

    let found = agent
        .ok(
            "find_ebay_listings",
            json!({"item_id": card, "marketplace": "EBAY_DE"}),
        )
        .await;
    assert_eq!(found["marketplace"], "EBAY_DE");
    assert_eq!(found["totalOnEbay"], 3, "asked again without the condition");
    assert_eq!(found["range"]["currency"], "EUR");
    assert_eq!(found["range"]["median"], 14.0);

    // A session priced from ebay.de (its own site; Settings' stays ebay.com) keeps the euro price apart from
    // dollars.
    let n = state
        .add_to_cards(
            &session,
            None,
            &crate::model::AddToCards {
                prices: Some(Prices::Ebay),
                ebay_site: Some("EBAY_DE".into()),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        state.db.session(&session).unwrap().options.ebay_site,
        "EBAY_DE"
    );
    assert_eq!(n, 1);
    let started = Instant::now();
    while state.runner.is_running(&session) {
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "run didn't end"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let item = state.db.item_summary(card).unwrap();
    assert_eq!(
        (item.price, item.price_currency.as_str()),
        (Some(14.0), "EUR")
    );
    assert_eq!(
        item.usd_price(),
        None,
        "USD-only destinations don't take it"
    );
    let counts = state.db.session(&session).unwrap().counts;
    assert_eq!(counts.total_price, 0.0, "no dollars");
    assert_eq!(
        counts.values,
        vec![crate::model::Money {
            currency: "EUR".into(),
            total: 14.0
        }]
    );
    let row = agent.ok("list_cards", json!({"session": session})).await;
    assert_eq!(row["cards"][0]["priceCurrency"], "EUR");
}

/// The app with agents on (any port, allowed what `mcp` says), Ximilar at `api`, its data and config in `dir`.
async fn app_with_agent(
    dir: &Path,
    api: &str,
    mcp: McpSettings,
) -> (Arc<McpServer>, AppState, Agent) {
    let (server, state, agent, _) = app_with_events(dir, api, mcp).await;
    (server, state, agent)
}

/// `app_with_agent`, with the events the UI gets.
async fn app_with_events(
    dir: &Path,
    api: &str,
    mcp: McpSettings,
) -> (Arc<McpServer>, AppState, Agent, Arc<Recorder>) {
    let settings = Arc::new(SettingsStore::for_test(
        dir.join("config").join("settings.json"),
        Settings {
            api_base_url: api.into(),
            mcp: McpSettings {
                enabled: true,
                port: 0,
                ..mcp
            },
            ..Settings::default()
        },
        "token",
    ));
    let db = Db::memory().unwrap();
    let events = Arc::new(Recorder::default());
    let runner = Runner::new(
        None,
        events.clone(),
        db.clone(),
        settings.clone(),
        dir.join("data"),
    );
    let state = AppState {
        db,
        settings,
        runner,
    };
    let server = McpServer::new(state.clone(), dir.join("config"));
    server.apply().await;
    let status = server.status().await;
    assert!(status.running, "{status:?}");
    let agent = Agent {
        http: reqwest::Client::new(),
        url: status.url.unwrap(),
        token: status.token.unwrap(),
    };
    (server, state, agent, events)
}

/// A session of identified cards waiting for review, as a run leaves them.
fn identified_cards(db: &Db, names: &[&str]) -> (String, Vec<i64>) {
    use crate::model::*;
    let session = db
        .create_session(&NewSession {
            name: "Binder".into(),
            source_kind: SourceKind::Folder,
            source_ref: "/Users/demo/cards".into(),
            id_mode: IdMode::Tcg,
            options: SessionOptions::default(),
        })
        .unwrap();
    let mut ids = Vec::new();
    for (i, name) in names.iter().enumerate() {
        let id = db
            .insert_item(
                &session.id,
                &format!("/Users/demo/cards/{i}.jpg"),
                None,
                &ItemImages::default(),
                None,
            )
            .unwrap();
        db.claim_next(&session.id).unwrap();
        let card = CardFields {
            name: name.to_string(),
            ..Default::default()
        };
        db.save_recognition(
            id,
            "tcg",
            &card,
            Some("high"),
            Some(0.2),
            None,
            None,
            &Value::Null,
            1,
            ItemStatus::Done,
        )
        .unwrap();
        ids.push(id);
    }
    (session.id, ids)
}

/// A Cards Cloud id goes into a URL path: one that isn't a single segment would send the request, with the user's
/// token, to another Ximilar endpoint and hand its answer to the agent (or delete something else).
#[tokio::test(flavor = "multi_thread")]
async fn agents_stay_in_cards_cloud_paths() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};
    let ximilar = MockServer::start().await;
    let dir = tempfile::tempdir().unwrap();
    let (_server, _state, agent) = app_with_agent(
        dir.path(),
        &ximilar.uri(),
        McpSettings {
            allow_delete: true,
            ..McpSettings::default()
        },
    )
    .await;
    for id in [
        "../../../account/v2/details",
        "x/../../../account/v2/details",
        "%2e%2e/%2e%2e/%2e%2e/account/v2/details",
        "x?page=2",
        "x#",
    ] {
        for tool in ["get_cards_cloud_session", "delete_cards_cloud_session"] {
            let (answer, failed) = agent.tool(tool, json!({ "id": id })).await;
            assert!(failed, "{tool} {id}: {answer}");
            assert!(
                answer.as_str().unwrap().contains("not a Cards Cloud id"),
                "{answer}"
            );
        }
    }
    let sent = ximilar.received_requests().await.unwrap();
    assert!(
        sent.is_empty(),
        "{:?}",
        sent.iter().map(|r| r.url.to_string()).collect::<Vec<_>>()
    );

    let id = "1b4e28ba-2fa1-41d2-883f-0016d3cca427";
    Mock::given(method("GET"))
        .and(path(format!("/cards-cloud/v2/session/{id}/")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": id, "name": "Binder"})))
        .mount(&ximilar)
        .await;
    let found = agent
        .ok("get_cards_cloud_session", json!({ "id": id }))
        .await;
    assert_eq!(found["session"]["name"], "Binder");
}

/// Exports write CSV files outside the app's folders: an agent allowed to export can't replace the database,
/// the settings or a card's scan with CSV text.
#[tokio::test(flavor = "multi_thread")]
async fn exports_write_only_csv_files_outside_the_app() {
    let dir = tempfile::tempdir().unwrap();
    let (_server, state, agent) =
        app_with_agent(dir.path(), "http://127.0.0.1:9", McpSettings::default()).await;
    identified_cards(&state.db, &["Pikachu"]);
    let data = dir.path().join("data");
    let config = dir.path().join("config");
    let out = dir.path().join("out");
    for folder in [&data, &config, &out] {
        std::fs::create_dir_all(folder).unwrap();
    }
    let database = data.join("cardloader.db");
    let settings = config.join("settings.json");
    let scan = out.join("scan_0001.jpg");
    let inside = data.join("cards.csv");
    for file in [&database, &settings, &scan, &inside] {
        std::fs::write(file, b"keep").unwrap();
    }
    // Also through `..` from a folder outside.
    let inside_by_dots = out.join("..").join("data").join("cards.csv");
    // Only unix adds to the list: making a link on Windows needs a privilege.
    #[cfg_attr(not(unix), allow(unused_mut))]
    let mut refused = vec![
        (database, ".csv"),
        (settings, ".csv"),
        (scan.clone(), ".csv"),
        (inside, "own folders"),
        (inside_by_dots, "own folders"),
    ];
    #[cfg(unix)]
    {
        let link = out.join("link.csv");
        std::os::unix::fs::symlink(&scan, &link).unwrap();
        refused.push((link, "link"));
    }
    for (path, why) in &refused {
        for tool in ["export_csv", "export_tcgplayer"] {
            let (answer, failed) = agent
                .tool(
                    tool,
                    json!({"session": "latest", "path": path, "overwrite": true}),
                )
                .await;
            assert!(failed, "{tool} {}: {answer}", path.display());
            assert!(answer.as_str().unwrap().contains(why), "{answer}");
        }
        assert_eq!(std::fs::read(path).unwrap(), b"keep", "{}", path.display());
    }

    // A CSV elsewhere is written as before: new, refused when it exists, replaced with overwrite.
    let csv = out.join("Cards.CSV");
    let written = agent
        .ok("export_csv", json!({"session": "latest", "path": csv}))
        .await;
    assert_eq!(written["cards"], 1);
    assert!(std::fs::read_to_string(&csv).unwrap().contains("Pikachu"));
    let (answer, failed) = agent
        .tool("export_csv", json!({"session": "latest", "path": csv}))
        .await;
    assert!(
        failed && answer.as_str().unwrap().contains("overwrite"),
        "{answer}"
    );
    // The columns asked for, in their order; an agent's choice isn't remembered as the user's.
    agent
        .ok(
            "export_csv",
            json!({"session": "latest", "path": csv, "overwrite": true, "columns": ["name", "seq"],
                "headers": {"name": "primary name"}}),
        )
        .await;
    let text = std::fs::read_to_string(&csv).unwrap();
    assert!(
        text.trim_start_matches('\u{feff}')
            .starts_with("primary name,#\r\nPikachu,"),
        "{text}"
    );
    assert!(state.settings.get().csv_columns.is_empty());
    assert!(state.settings.get().csv_headers.is_empty());

    // The app's export remembers the user's columns and folder for the next one.
    use crate::export::CsvColumn;
    let session = state.db.sessions().unwrap()[0].id.clone();
    let mine = out.join("mine.csv");
    let cards = state
        .export_csv(
            &session,
            &mine,
            Some(vec![CsvColumn::Price, CsvColumn::Name]),
            Some([(CsvColumn::Name, "primary name".to_string())].into()),
            true,
        )
        .unwrap();
    assert!(std::fs::read_to_string(&mine)
        .unwrap()
        .contains("price,primary name\r\n"));
    assert_eq!(cards, 1);
    let remembered = state.settings.get();
    assert_eq!(remembered.csv_columns, [CsvColumn::Price, CsvColumn::Name]);
    assert_eq!(remembered.csv_headers[&CsvColumn::Name], "primary name");
    assert_eq!(
        remembered.csv_folder.as_deref(),
        Some(out.to_str().unwrap())
    );
}

/// The user's accept stays theirs: an agent can't take it back, and an edit that changes no card field or price
/// (notes, location) accepts nothing, for an agent or the user.
#[tokio::test(flavor = "multi_thread")]
async fn agents_never_take_over_a_users_accept() {
    use crate::model::ItemEdit;
    let dir = tempfile::tempdir().unwrap();
    let (_server, state, agent) =
        app_with_agent(dir.path(), "http://127.0.0.1:9", McpSettings::default()).await;
    let db = &state.db;
    let (session, ids) = identified_cards(db, &["Pikachu", "Mew", "Eevee", "Snorlax"]);
    let (users, unreviewed, agents, users_notes) = (ids[0], ids[1], ids[2], ids[3]);
    db.set_reviewed(&session, &[users], true, &Reviewer::User)
        .unwrap();
    let user_accepted = |id| {
        let i = db.item_summary(id).unwrap();
        i.reviewed && !i.agent_reviewed
    };

    let taken_back = agent
        .ok(
            "accept_cards",
            json!({"item_ids": [users], "accepted": false}),
        )
        .await;
    assert_eq!(taken_back["changed"], 0, "{taken_back}");
    assert_eq!(taken_back["skipped"][0]["id"], users, "{taken_back}");
    assert!(user_accepted(users));
    let again = agent.ok("accept_cards", json!({"item_ids": [users]})).await;
    assert_eq!(again["changed"], 0);
    assert!(user_accepted(users));

    agent
        .ok(
            "edit_card",
            json!({"item_id": users, "location": "Box 3", "note": "filed"}),
        )
        .await;
    let card = db.item_summary(users).unwrap();
    assert_eq!(card.location, "Box 3");
    assert!(user_accepted(users));
    assert_eq!(card.review_note, "");

    agent
        .ok(
            "edit_card",
            json!({"item_id": unreviewed, "notes": "sleeved"}),
        )
        .await;
    let card = db.item_summary(unreviewed).unwrap();
    assert_eq!(card.notes, "sleeved");
    assert!(!card.reviewed, "still to review");

    // The user's notes don't accept a card either.
    let mut edit = ItemEdit {
        card: db.item_summary(users_notes).unwrap().card,
        price: None,
        notes: "corner ding".into(),
        location: String::new(),
        title: String::new(),
    };
    state
        .runner
        .edit_item(users_notes, edit.clone(), &Reviewer::User)
        .unwrap();
    assert!(!db.item_summary(users_notes).unwrap().reviewed);
    // Correcting the card does.
    edit.card.name = "Snorlax V".into();
    state
        .runner
        .edit_item(users_notes, edit, &Reviewer::User)
        .unwrap();
    assert!(user_accepted(users_notes));

    // An agent takes back its own accept.
    agent
        .ok(
            "accept_cards",
            json!({"item_ids": [agents], "note": "matches"}),
        )
        .await;
    assert!(db.item_summary(agents).unwrap().agent_reviewed);
    let undone = agent
        .ok(
            "accept_cards",
            json!({"item_ids": [agents], "accepted": false}),
        )
        .await;
    assert_eq!(undone["changed"], 1, "{undone}");
    let card = db.item_summary(agents).unwrap();
    assert!(!card.reviewed && !card.agent_reviewed);
}

#[test]
fn compares_tokens() {
    assert!(same(b"cl_abc", b"cl_abc"));
    assert!(!same(b"cl_abc", b"cl_abd"));
    assert!(!same(b"cl_abc", b"cl_ab"));
}

#[test]
fn finds_a_uuid_in_text() {
    let id = "7311776c-6a70-4b7c-a66d-f902282a195d";
    assert_eq!(super::tools::uuid_in(&format!("(id {id})")), Some(id));
    assert_eq!(
        super::tools::uuid_in(&format!("Session „{id}“ via MCP")),
        Some(id)
    );
    assert_eq!(super::tools::uuid_in("Session 2026-11-21 13:57"), None);
    assert_eq!(
        super::tools::uuid_in("7311776c-6a70-4b7c-a66d-f902282a195"),
        None
    );
}
