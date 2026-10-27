//! Ready-made tasks clients offer as slash commands: they spell out how to do the work with the tools.

use rmcp::model::{GetPromptResult, JsonObject, Prompt, PromptArgument, PromptMessage, Role};

pub fn list() -> Vec<Prompt> {
    vec![
        Prompt::new(
            "review_session",
            Some("Check every card of a session against its picture and accept the right matches"),
            Some(vec![PromptArgument::new("session")
                .with_description("The session id; empty for the newest session")
                .with_required(false)]),
        )
        .with_title("Review a session"),
        Prompt::new(
            "new_session",
            Some("Analyse a folder of card images, asking only what can't be told from the folder"),
            Some(vec![
                PromptArgument::new("path")
                    .with_description("The folder of card images")
                    .with_required(true),
                PromptArgument::new("about")
                    .with_description(
                        "What the cards are, e.g. \"English Pokémon, fronts only, store in Ximilar\"",
                    )
                    .with_required(false),
            ]),
        )
        .with_title("New session from a folder"),
    ]
}

pub fn get(name: &str, arguments: Option<&JsonObject>) -> Option<GetPromptResult> {
    let arg = |key: &str| {
        arguments
            .and_then(|a| a.get(key))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_string)
    };
    let text = match name {
        "review_session" => review(&arg("session").unwrap_or_else(|| "latest".into())),
        "new_session" => new_session(&arg("path")?, arg("about").as_deref()),
        _ => return None,
    };
    Some(GetPromptResult::new(vec![PromptMessage::new_text(
        Role::User,
        text,
    )]))
}

fn review(session: &str) -> String {
    format!(
        "Review the cards of the Cardloader session \"{session}\".

1. Call list_cards with session_id \"{session}\" and filter to_review (page with offset while `more` is true).
2. For each card, call get_card and look at the front picture (and the back, if there is one). Read the printed \
name, collector number, set symbol or code, and year.
3. If the picture shows the card's match, accept it with accept_cards and a short note of what matched, e.g. \
\"name and 4/102 match\". Accept several cards in one call when you can.
4. If it shows one of the other candidates instead, call select_candidate with that candidate's index and a note.
5. If it matches none of them, or you can't read it, leave the card for the user: don't accept it.
6. Cards with high confidence are usually right: check a few of them, and if those match, look at the rest \
more quickly. Every picture costs tokens, so don't open a card twice.

At the end, say how many cards you accepted, how many you changed to another candidate, and list the ones left \
for the user with why."
    )
}

fn new_session(path: &str, about: Option<&str>) -> String {
    let about = about
        .map(|a| format!("The user says about the cards: \"{a}\".\n\n"))
        .unwrap_or_default();
    format!(
        "Analyse the card images in \"{path}\" with Cardloader.

{about}1. Call inspect_folder on the path: how many images, the file names, and whether a session already uses \
the folder (if one does, ask whether to continue it with run_session instead).
2. Work out the session's options: card_type (tcg, sport, comics, slab or auto), game, alphabet, whether the \
images are fronts only (known_side) or alternate front and back (pairing front_back), and whether to store the \
session in Cards Cloud (store_in_cards_cloud). Ask the user only about what you can't tell; call get_settings for \
their defaults.
3. Call create_session, then wait_for_session until it reports finished.
4. Report how many cards were identified, how many failed (and why), how many wait for review, and their total \
value."
    )
}
