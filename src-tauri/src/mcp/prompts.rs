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
        Prompt::new(
            "identify_session",
            Some("Identify the cards yourself from their pictures: Cardloader scans and keeps them, you read them"),
            Some(vec![
                PromptArgument::new("path")
                    .with_description("A folder of card images for a new session; empty to identify a running one")
                    .with_required(false),
                PromptArgument::new("session")
                    .with_description("A running session whose cards wait for you; empty for any")
                    .with_required(false),
            ]),
        )
        .with_title("Identify cards yourself"),
        Prompt::new(
            "ebay_prices",
            Some("Find each identified card's active eBay listings and its price range"),
            Some(vec![PromptArgument::new("session")
                .with_description("The session id; empty for the newest session")
                .with_required(false)]),
        )
        .with_title("Prices from eBay"),
        Prompt::new(
            "price_session",
            Some("Price the cards of a session yourself as they are identified"),
            Some(vec![PromptArgument::new("session")
                .with_description("A running session whose cards wait for your prices; empty for any")
                .with_required(false)]),
        )
        .with_title("Price cards yourself"),
        Prompt::new(
            "crop_session",
            Some("Find each card in its pictures yourself: Cardloader cuts it out at the corners you give"),
            Some(vec![
                PromptArgument::new("path")
                    .with_description("A folder of card images for a new session; empty to crop a running one")
                    .with_required(false),
                PromptArgument::new("session")
                    .with_description("A running session whose cards wait for your corners; empty for any")
                    .with_required(false),
            ]),
        )
        .with_title("Crop cards yourself"),
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
        "identify_session" => identify(arg("path").as_deref(), arg("session").as_deref()),
        "ebay_prices" => ebay_prices(&arg("session").unwrap_or_else(|| "latest".into())),
        "price_session" => price(arg("session").as_deref()),
        "crop_session" => crop(arg("path").as_deref(), arg("session").as_deref()),
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

fn identify(path: Option<&str>, session: Option<&str>) -> String {
    let start = match (path, session) {
        (Some(path), _) => format!(
            "1. Call inspect_folder on \"{path}\", then create_session for it with engine \"agent\" (and card_type, \
pairing front_back when the images alternate front and back, grading if the user wants it). Cardloader reads the \
images and hands each card to you."
        ),
        (None, Some(session)) => format!(
            "1. The session \"{session}\" is waiting for you (start it with run_session if it isn't running)."
        ),
        (None, None) => "1. Call get_status: sessions identified by an AI agent that are running show cards \
waitingForAgent. If none runs, ask the user for a folder and create_session with engine \"agent\"."
            .to_string(),
    };
    let session_arg = session
        .map(|s| format!(" with session_id \"{s}\""))
        .unwrap_or_default();
    format!(
        "Identify the cards of a Cardloader session yourself, from their pictures.

{start}
2. Call next_cards_to_identify{session_arg}. It returns the next card (several if you pass max) with the front (and back) picture of each, and \
waits while the next cards are scanned.
3. Read each card (from its pictures alone, unless next_cards_to_identify says webSearch is true): the name (the \
player, character or comic title), the set or product (e.g. \"2023 Panini Prizm\"), \
the card number as printed (with its letters), the year, the variant or parallel (e.g. \"Silver Prizm\", \
\"Holo\"), and the game or sport. Sports cards often print the number, year and set only on the back.
4. Send them with identify_cards: several cards in one call when you have them, with certainty high, medium or low and a short note of \
what you read. If you can't tell a card, send it with unidentified and say why: never guess a name.
5. Repeat from 2 until next_cards_to_identify says finished.

The user reviews every card you identify. At the end, say how many cards you identified, which ones you were \
unsure of, and which you couldn't identify."
    )
}

fn ebay_prices(session: &str) -> String {
    format!(
        "Find what the identified cards of the Cardloader session \"{session}\" ask on eBay now.

1. Call list_cards with session_id \"{session}\" (page with offset while `more` is true). Skip cards that aren't \
identified (no name), and cards that already have an ebayRange unless the user asks again.
2. For each card, call find_ebay_listings. It searches by the card's name, set and number; if it finds nothing or \
mostly other cards, call it again with a better query (e.g. the player, year, set and parallel, or fewer words).
3. Read the listing titles and keep_ebay_listings with the ids of those that are this card: the same card and number, \
the same parallel or variant, the same grade if the card is graded; no lots, no reprints or proxies. Say in the note \
what you left out.
4. If no listing is this card, keep none and say so in the note.

At the end, list each card with its price range (min–max), average and median and how many listings count, and \
point out cards whose range is wide or that had few listings."
    )
}

fn price(session: Option<&str>) -> String {
    let start = match session {
        Some(session) => format!(
            "1. The session \"{session}\" is waiting for you (start it with run_session if it isn't running)."
        ),
        None => "1. Call list_sessions: a running session priced by an AI agent counts the cards waitingForPrice. \
If none runs, ask the user for a folder and create_session with prices \"agent\"."
            .to_string(),
    };
    let session_arg = session
        .map(|s| format!(" with session_id \"{s}\""))
        .unwrap_or_default();
    format!(
        "Price the cards of a Cardloader session yourself, as they are identified.

{start}
2. Call next_cards_to_price{session_arg}. It returns the next identified card (several if you pass max): name, set, number, year, variant, \
game, the slab's grade if the card is graded, an eBay range if one was found, and its pictures.
3. Work out what each card sells for now, as it is (raw, or in its slab's grade). If you may use them, \
find_ebay_listings and keep_ebay_listings give its active eBay listings. Search the web only when \
next_cards_to_price says webSearch is true.
4. Send the prices with price_cards (in US dollars, or give each price's currency, e.g. EUR for ebay.de): several cards in one call when you have them, each with a short note of how you got \
it (e.g. \"median of 6 listings, no lots\"). If you can't find a price, send the card with unpriced and say why: \
never guess.
5. Repeat from 2 until next_cards_to_price says finished.

At the end, list the cards you priced with their prices, and the ones you couldn't price with why."
    )
}

fn crop(path: Option<&str>, session: Option<&str>) -> String {
    let start = match (path, session) {
        (Some(path), _) => format!(
            "1. Call create_session with path \"{path}\", task \"crop\" and crop_engine \"agent\" (ask the user \
for a crop_folder if they want the pictures somewhere of their own; pairing front_back when the images alternate \
front and back)."
        ),
        (None, Some(session)) => format!(
            "1. The session \"{session}\" is waiting for you (start it with run_session if it isn't running)."
        ),
        (None, None) => "1. Call list_sessions: a running session cropped by an AI agent counts the cards \
waitingForCrop. If none runs, ask the user for a folder and create_session with task \"crop\" and crop_engine \
\"agent\"."
            .to_string(),
    };
    let session_arg = session
        .map(|s| format!(" with session_id \"{s}\""))
        .unwrap_or_default();
    format!(
        "Find the cards of a Cardloader session in their pictures yourself, so Cardloader can cut them out.

{start}
2. Call next_cards_to_crop{session_arg}. It returns the next card (several if you pass max), each with its front (and back) picture and that \
picture's size in pixels.
3. In each picture, find the card's four corners as [x, y] pixels from the picture's top-left: on the card's own \
edge (a slab's or sleeve's outer edge when it is in one), not inside its border and not out on the table. Start \
at the card's own top-left corner (where its top-left is when you read the card upright) and go clockwise: a card \
lying sideways or upside down then comes out upright. A corner hidden by the picture's edge goes on that edge.
4. Check a card with preview_crop when you aren't sure: it shows the cut your corners give and saves nothing. \
The whole card should be in it, upright, with no more than a sliver of background; correct the corners and look \
again if not. Every picture costs tokens, so don't preview what is plainly right.
5. Send the corners with crop_cards: several cards in one call when you have them, each side with the size of its picture as given. A \
card with a back picture needs the back's corners too, or no_back with why. If a picture shows no card, send the \
card with uncropped and say why: never guess.
6. Repeat from 2 until next_cards_to_crop says finished.

At the end, say how many cards you cropped, where the pictures are (cropsFolder), and list the ones you couldn't \
crop with why."
    )
}
