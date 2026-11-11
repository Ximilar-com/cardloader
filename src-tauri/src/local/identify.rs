//! Asking a vision language model what card a photo shows, and reading its answer. Each answer is a small JSON
//! object held to its fields by a llama.cpp grammar.
//!
//! Small models (LFM) get two short questions:
//! 1. the whole card: name, year and game;
//! 2. the bottom strip, enlarged: collector number and set code — the small print a 450M model can't read
//!    from the whole card (asked about the whole card, it reads the HP or attack text instead).
//!
//! and aren't asked for the set *name*: they fill it with whatever text is on the card. Models that read small print
//! on the whole card (Qwen3.5, `Family::reads_whole_card`) get one question with every field, and with the card's
//! back as a second image when the session has it: sports cards print the number, year and set there. The strip
//! assumes the card fills the picture; the whole card doesn't.
//!
//! A session of comics asks about a cover instead: the series title, the issue number, the publisher and the year,
//! stored where Ximilar's comics_id puts them (name, number, set name, year). Small models read the title off the
//! whole cover and the issue number off its enlarged top, where the number sits by the date and the price.

use serde::Deserialize;
use serde_json::{json, Value};

use crate::model::{CardFields, IdMode};
use crate::ximilar::recognition::Recognition;

/// Marks local results where the Ximilar confidence (high/medium/low) would be.
pub const LOCAL_CONFIDENCE: &str = "local";

/// The bottom strip of the card holding the collector number, as a fraction of the height.
pub const STRIP_FRACTION: f64 = 0.12;

pub const CARD_PROMPT: &str =
    "This is a photo of a collectible card (trading card, sports card or comic). \
Fill in the JSON fields from what is printed on the card: \
name: the card name or player name exactly as printed; \
year: the copyright or release year, only if it is printed; \
game: which game or sport the card belongs to. \
Use an empty string for anything you cannot read clearly. Do not guess.";

pub const STRIP_PROMPT: &str = "This is the bottom edge of a trading card. Read the small print: \
card_number: the collector number exactly as printed (not the power/toughness, HP or damage); \
set_code: the short set code, only if it is printed. \
Use an empty string for anything you cannot read clearly.";

/// The second question of small models when the session reads backs: the back instead of the front's bottom strip.
pub const BACK_PROMPT: &str = "This is the back of a sports card. Read the small print: \
card_number: the card number exactly as printed, with any letters before it (e.g. BCP-155, RA-PC), not the serial \
number of a numbered card, e.g. 12/99; \
set_code: the short set code, only if it is printed. \
Use an empty string for anything you cannot read clearly.";

const WHOLE_FIELDS: &str = "Fill in the JSON fields from what is printed on the card: \
name: the card name or player name exactly as printed; \
card_number: the card number exactly as printed, with any letters before it (e.g. BCP-155, RA-PC); on a trading \
card the collector number, not the power/toughness, HP or damage; on a sports card not the serial number of a \
numbered card, e.g. 12/99; \
set_name: the set or product line as printed, with its maker (e.g. on the back of a sports card); \
set_code: the short set code, only if it is printed; \
year: the copyright or release year, only if it is printed; \
game: which game or sport the card belongs to. \
Use an empty string for anything you cannot read clearly. Do not guess.";

/// The top of a comic's cover holding the issue number, as a fraction of the height.
pub const COVER_TOP_FRACTION: f64 = 0.25;

/// The one question about a comic of models that read small print. The fields are named as a cover says them:
/// asked for a "card number", a model looks for one.
pub const COMIC_PROMPT: &str =
    "This is the front cover of a comic book (a comic, magazine or manga). \
Fill in the JSON fields from what is printed on the cover: \
title: the title of the series as its logo prints it (e.g. The Amazing Spider-Man), not the title of a story; \
issue: the issue or volume number as printed, usually after \"No.\" or \"#\" by the date or the price, not the \
price, the date or the barcode; \
publisher: the publisher, only if it is printed (e.g. Marvel, DC, Image); \
year: the year of the cover date, only if it is printed. \
Use an empty string for anything you cannot read clearly. Do not guess.";

/// Small models' first question about a comic: the whole cover.
pub const COVER_PROMPT: &str =
    "This is the front cover of a comic book (a comic, magazine or manga). \
Fill in the JSON fields from what is printed on the cover: \
title: the title of the series as its logo prints it (e.g. The Amazing Spider-Man), not the title of a story; \
year: the year of the cover date, only if it is printed. \
Use an empty string for anything you cannot read clearly. Do not guess.";

/// Their second: the top of the cover, enlarged. It asks for the price and the month first: a small model asked
/// only for the issue number answers the price, the largest number there ("No. 99 MAY 10¢"), and an example
/// number in the prompt comes back as the answer.
pub const COVER_TOP_PROMPT: &str =
    "This is the top of the front cover of a comic book. Read the small print: \
price: the cover price, e.g. 10¢; \
month: the month of the cover date, e.g. MAY; \
issue: the issue number, printed after \"No.\" or \"#\". \
Use an empty string for anything you cannot read clearly.";

/// The one question of models that read the whole card; `back`: the second image is the card's back.
pub fn whole_prompt(back: bool) -> String {
    let images = if back {
        "The first image is the front of a collectible card (trading card, sports card or comic), the second its \
         back: sports cards print the card number, year and set on the back. "
    } else {
        "This is a photo of a collectible card (trading card, sports card or comic). "
    };
    format!("{images}{WHOLE_FIELDS}")
}

pub const GAMES: &[(&str, &str, &str)] = &[
    // (answer, id_type, category shown in the app)
    ("pokemon", "tcg", "Pokemon"),
    ("magic", "tcg", "Magic The Gathering"),
    ("yugioh", "tcg", "Yu-Gi-Oh!"),
    ("one_piece", "tcg", "One Piece"),
    ("lorcana", "tcg", "Lorcana"),
    ("other_tcg", "tcg", ""),
    ("baseball", "sport", "Baseball"),
    ("basketball", "sport", "Basketball"),
    ("football", "sport", "Football"),
    ("soccer", "sport", "Soccer"),
    ("hockey", "sport", "Hockey"),
    ("other_sport", "sport", ""),
    ("comics", "comics", "Comics"),
    ("other", "tcg", ""),
];

const GRAMMAR_COMMON: &str = r#"s80 ::= "\"" char{0,80} "\""
s24 ::= "\"" char{0,24} "\""
s16 ::= "\"" char{0,16} "\""
s8 ::= "\"" char{0,8} "\""
char ::= [^"\\\x7F\x00-\x1F] | [\\] (["\\bfnrt] | "u" [0-9a-fA-F]{4})
space ::= | " " | "\n"{1,2} [ \t]{0,20}
"#;

/// A GBNF grammar for one JSON object with these fields, in this order (a schema-generated grammar would
/// sort them alphabetically, and the order steers a small model).
fn object_grammar(fields: &[(&str, &str)], extra: &str) -> String {
    let body = fields
        .iter()
        .map(|(key, rule)| format!(r#""\"{key}\"" space ":" space {rule}"#))
        .collect::<Vec<_>>()
        .join(r#" "," space "#);
    format!("root ::= \"{{\" space {body} space \"}}\"\n{extra}{GRAMMAR_COMMON}")
}

/// The games a card of a session of `kind` can be: the session says whether it holds trading cards, sports cards
/// or comics; Auto (and slab labels) leave it to the model.
fn game_rule(kind: IdMode) -> String {
    let id_type = match kind {
        IdMode::Tcg => Some("tcg"),
        IdMode::Sport => Some("sport"),
        IdMode::Comics => Some("comics"),
        IdMode::Auto | IdMode::Slab => None,
    };
    GAMES
        .iter()
        .filter(|g| id_type.is_none_or(|t| g.1 == t))
        .map(|g| format!(r#""\"{}\"""#, g.0))
        .collect::<Vec<_>>()
        .join(" | ")
}

pub fn card_grammar(kind: IdMode) -> String {
    object_grammar(
        &[("name", "s80"), ("year", "s8"), ("game", "game")],
        &format!("game ::= {}\n", game_rule(kind)),
    )
}

pub fn strip_grammar() -> String {
    object_grammar(&[("card_number", "s16"), ("set_code", "s16")], "")
}

/// Every field at once (`whole_prompt`), in the order a model reads a card: name, then the small print.
pub fn whole_grammar(kind: IdMode) -> String {
    object_grammar(
        &[
            ("name", "s80"),
            ("card_number", "s24"),
            ("set_name", "s80"),
            ("set_code", "s16"),
            ("year", "s8"),
            ("game", "game"),
        ],
        &format!("game ::= {}\n", game_rule(kind)),
    )
}

/// One question about a comic's cover (`COMIC_PROMPT`).
pub fn comic_grammar() -> String {
    object_grammar(
        &[
            ("title", "s80"),
            ("issue", "s16"),
            ("publisher", "s80"),
            ("year", "s8"),
        ],
        "",
    )
}

pub fn cover_grammar() -> String {
    object_grammar(&[("title", "s80"), ("year", "s8")], "")
}

pub fn cover_top_grammar() -> String {
    object_grammar(&[("price", "s8"), ("month", "s16"), ("issue", "s16")], "")
}

/// The fields of a card as a model answers them; a question that doesn't ask for a field leaves it empty. A comic's
/// title, issue and publisher are its name, number and set name, as Ximilar's comics_id is read.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Answer {
    #[serde(alias = "title")]
    name: String,
    #[serde(alias = "issue")]
    card_number: String,
    #[serde(alias = "publisher")]
    set_name: String,
    set_code: String,
    year: String,
    game: String,
}

fn json_object(text: &str) -> Result<Value, String> {
    let start = text.find('{').ok_or("the model gave no answer")?;
    let end = text.rfind('}').ok_or("the model's answer was cut off")?;
    serde_json::from_str(&text[start..=end])
        .map_err(|e| format!("the model's answer is not valid JSON: {e}"))
}

fn text_of(value: &Value) -> &str {
    value.as_str().unwrap_or_default()
}

fn clean(text: &str) -> String {
    let t = text.trim();
    if ["unknown", "n/a", "none", "-", "null", "0"].contains(&t.to_lowercase().as_str()) {
        String::new()
    } else {
        t.to_string()
    }
}

/// The collector number without what models copy around it: "#" or "No." (sports cards print "No. 76"), a leading
/// rarity letter set apart ("C 0123", Magic) or a regulation mark on a number out of a plain total ("D025/202",
/// Pokémon). Other letters are the number's ("H25/H32", a sports card's "T87-12"). "125197" read without its slash
/// is "125/197": six digits are taken as two halves. `verbatim` (sports cards, comics): every letter is the number's.
fn collector_number(raw: &str, verbatim: bool) -> String {
    let n = clean(raw);
    let n = n.trim_start_matches('#').trim();
    let n = match n.get(..3).map(str::to_ascii_lowercase).as_deref() {
        Some("no.") => n[3..].trim(),
        _ => n,
    };
    let n = n.trim_start_matches('#').trim().to_string();
    let mut chars = n.chars();
    let (first, rest) = (chars.next(), chars.as_str());
    let digit = |s: &str| s.starts_with(|d: char| d.is_ascii_digit());
    let apart = rest.starts_with(' ') && digit(rest.trim_start());
    let plain_total = rest
        .split_once('/')
        .is_some_and(|(_, total)| !total.is_empty() && total.bytes().all(|b| b.is_ascii_digit()));
    let n = if !verbatim
        && first.is_some_and(|c| c.is_ascii_alphabetic())
        && (apart || (digit(rest) && plain_total))
    {
        rest.trim_start().to_string()
    } else {
        n
    };
    if n.len() == 6 && n.bytes().all(|b| b.is_ascii_digit()) {
        format!("{}/{}", &n[..3], &n[3..])
    } else {
        n
    }
}

/// The two answers as a recognition, plus the parsed answers for the stored raw response.
pub fn parse_answers(card: &str, strip: Option<&str>) -> Result<(Recognition, Value), String> {
    let card_raw = json_object(card)?;
    let a: Answer = serde_json::from_value(card_raw.clone()).map_err(|e| e.to_string())?;
    let strip_raw = strip.and_then(|s| json_object(s).ok());
    let s: Answer = strip_raw
        .clone()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();
    let answer = Answer {
        card_number: s.card_number,
        set_code: s.set_code,
        // Not asked of small models: what they put there is other card text.
        set_name: String::new(),
        ..a
    };
    Ok((
        recognition(answer),
        json!({"card": card_raw, "strip": strip_raw}),
    ))
}

/// Whether the answer to `CARD_PROMPT` names a sport: then the back holds the card number.
pub fn is_sport(card: &str) -> bool {
    let game = json_object(card)
        .ok()
        .and_then(|v| v.get("game").and_then(Value::as_str).map(str::to_string))
        .unwrap_or_default();
    GAMES.iter().any(|g| g.0 == game.trim() && g.1 == "sport")
}

/// The answer to `whole_prompt` as a recognition, plus the parsed answer for the stored raw response.
pub fn parse_whole(text: &str) -> Result<(Recognition, Value), String> {
    let raw = json_object(text)?;
    let answer: Answer = serde_json::from_value(raw.clone()).map_err(|e| e.to_string())?;
    Ok((recognition(answer), json!({ "card": raw })))
}

/// A comic's answers as a recognition: the cover's (`COMIC_PROMPT`), or a small model's about the cover
/// (`COVER_PROMPT`) and then about its top (`COVER_TOP_PROMPT`), which gives the issue number.
pub fn parse_comic(cover: &str, top: Option<&str>) -> Result<(Recognition, Value), String> {
    let cover_raw = json_object(cover)?;
    let mut answer: Answer =
        serde_json::from_value(cover_raw.clone()).map_err(|e| e.to_string())?;
    let mut raw = json!({ "card": cover_raw });
    if let Some(top) = top {
        let top_raw = json_object(top).ok();
        let t: Answer = top_raw
            .clone()
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();
        // The price again is the price, not the issue.
        let digits = |v: &str| -> String { v.chars().filter(char::is_ascii_digit).collect() };
        let price = digits(
            top_raw
                .as_ref()
                .map(|v| text_of(&v["price"]))
                .unwrap_or_default(),
        );
        answer.card_number = if !price.is_empty() && digits(&t.card_number) == price {
            String::new()
        } else {
            t.card_number
        };
        raw["top"] = json!(top_raw);
    }
    // The session says they are comics; the questions don't ask.
    answer.game = "comics".into();
    Ok((recognition(answer), raw))
}

fn recognition(a: Answer) -> Recognition {
    let game = GAMES.iter().find(|g| g.0 == a.game.trim());
    let year: String = clean(&a.year)
        .chars()
        .filter(char::is_ascii_digit)
        .take(4)
        .collect();
    let set_code = clean(&a.set_code).to_uppercase();
    let mut card = CardFields {
        name: clean(&a.name),
        // Set codes are short (OBF, LOB-EN, SV03); anything else is other small print, e.g. the artist.
        set_code: if set_code.chars().any(|c| c.is_ascii_alphabetic())
            && set_code.len() <= 8
            && !set_code.contains(' ')
        {
            set_code
        } else {
            String::new()
        },
        card_number: collector_number(&a.card_number, game.is_some_and(|g| g.1 != "tcg")),
        set_name: clean(&a.set_name),
        year: if year.len() == 4 { year } else { String::new() },
        subcategory: game.map(|g| g.2.to_string()).unwrap_or_default(),
        ..Default::default()
    };
    if !card.name.is_empty() {
        let set = if card.set_code.is_empty() {
            String::new()
        } else {
            format!("({})", card.set_code)
        };
        let number = if card.card_number.is_empty() {
            String::new()
        } else {
            format!("#{}", card.card_number)
        };
        card.full_name = [card.name.as_str(), set.as_str(), number.as_str()]
            .into_iter()
            .filter(|p| !p.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
    }
    Recognition {
        id_type: game.map(|g| g.1).unwrap_or("tcg").to_string(),
        confidence: (!card.name.is_empty()).then(|| LOCAL_CONFIDENCE.to_string()),
        card,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_both_answers() {
        let (r, raw) = parse_answers(
            r#"{"name": "Charizard ex", "year": "©2023 Pokémon", "game": "pokemon"}"#,
            Some(r#"{"card_number": "125197", "set_code": "obf"}"#),
        )
        .unwrap();
        assert_eq!(r.card.full_name, "Charizard ex (OBF) #125/197");
        assert_eq!(
            (r.card.year.as_str(), r.card.subcategory.as_str()),
            ("2023", "Pokemon")
        );
        assert_eq!(
            (r.id_type.as_str(), r.confidence.as_deref()),
            ("tcg", Some("local"))
        );
        assert_eq!((r.distance, r.price), (None, None));
        assert_eq!(raw["strip"]["set_code"], "obf");
    }

    #[test]
    fn noise_and_missing_parts() {
        let (r, _) = parse_answers(
            "```json\n{\"name\":\"Shohei Ohtani\",\"year\":\"18\",\"game\":\"baseball\"}\n```",
            Some("[{\"card_number\": \"#1019\", \"set_code\": \"CHRIS RAHN\"}]"),
        )
        .unwrap();
        assert_eq!(r.id_type, "sport");
        assert_eq!(r.card.full_name, "Shohei Ohtani #1019");
        assert_eq!(collector_number("C 0123", false), "0123");
        assert_eq!(collector_number("D025/202", false), "025/202");
        assert_eq!(collector_number("SV107", false), "SV107");
        assert_eq!(collector_number("TG05/TG30", false), "TG05/TG30");
        assert_eq!(r.card.year, "");
        let (empty, _) = parse_answers(r#"{"name":"","year":"","game":"other"}"#, None).unwrap();
        assert!(empty.card.full_name.is_empty() && empty.confidence.is_none());
        assert!(parse_answers("I cannot see a card", None).is_err());
    }

    #[test]
    fn reads_the_whole_card_in_one_answer() {
        // A sports card's front and back: the number, year and set come from the back.
        let (r, raw) = parse_whole(
            r#"{"name": "Austin Reaves", "card_number": "No. 76", "set_name": "2023 Panini Prizm Basketball",
                "set_code": "", "year": "2023", "game": "basketball"}"#,
        )
        .unwrap();
        assert_eq!(r.id_type, "sport");
        assert_eq!(
            (
                r.card.card_number.as_str(),
                r.card.set_name.as_str(),
                r.card.year.as_str(),
                r.card.subcategory.as_str()
            ),
            ("76", "2023 Panini Prizm Basketball", "2023", "Basketball")
        );
        assert_eq!(r.card.full_name, "Austin Reaves #76");
        assert_eq!(raw["card"]["set_name"], "2023 Panini Prizm Basketball");
        assert_eq!(collector_number("no.SIG-ZG", false), "SIG-ZG");
        assert_eq!(collector_number("#US199", false), "US199");
        // The two-question answer never takes a set name.
        let (two, _) = parse_answers(
            r#"{"name": "Pikachu", "year": "2023", "game": "pokemon", "set_name": "Pikachu"}"#,
            None,
        )
        .unwrap();
        assert!(two.card.set_name.is_empty());
        assert!(whole_prompt(true).contains("the second its back"));
        assert!(whole_prompt(false).contains("not the serial number"));
        assert!(whole_prompt(false).contains("with any letters before it"));
        assert!(!whole_prompt(false).contains("back:"));
    }

    /// Letters before the digits are part of the number, except a rarity letter set apart ("C 0123") or a
    /// regulation mark on a number out of a plain total ("D025/202"); on a sports card they always are.
    #[test]
    fn letters_before_the_number() {
        assert_eq!(collector_number("D 025/202", false), "025/202");
        assert_eq!(collector_number("H25/H32", false), "H25/H32");
        assert_eq!(collector_number("T87-12", false), "T87-12");
        assert_eq!(collector_number("T87-12", true), "T87-12");
        assert_eq!(collector_number("C 0123", true), "C 0123");
        assert_eq!(collector_number("No. 76", true), "76");
        assert_eq!(collector_number("125197", false), "125/197");
        // The answer's game tells a sports card.
        let (r, _) = parse_whole(
            r#"{"name": "Mike Trout", "card_number": "T87-12", "set_name": "2022 Topps", "set_code": "",
                "year": "2022", "game": "baseball"}"#,
        )
        .unwrap();
        assert_eq!(r.card.card_number, "T87-12");
        let (r, _) = parse_whole(
            r#"{"name": "Pikachu", "card_number": "D025/202", "set_name": "", "set_code": "", "year": "",
                "game": "pokemon"}"#,
        )
        .unwrap();
        assert_eq!(r.card.card_number, "025/202");
    }

    #[test]
    fn reads_a_comics_cover() {
        // One question with every field, named as a cover says them.
        let (r, raw) = parse_comic(
            r#"{"title": "Planet Comics", "issue": "No. 34", "publisher": "Fiction House", "year": "1945"}"#,
            None,
        )
        .unwrap();
        assert_eq!(
            (r.id_type.as_str(), r.confidence.as_deref()),
            ("comics", Some("local"))
        );
        assert_eq!(
            (
                r.card.name.as_str(),
                r.card.card_number.as_str(),
                r.card.set_name.as_str(),
                r.card.year.as_str(),
                r.card.subcategory.as_str(),
                r.card.full_name.as_str()
            ),
            (
                "Planet Comics",
                "34",
                "Fiction House",
                "1945",
                "Comics",
                "Planet Comics #34"
            )
        );
        assert_eq!(raw["card"]["issue"], "No. 34");
        assert!(raw.get("top").is_none());
        // A small model's two answers: the issue number is the top's, whatever the first answer put there.
        let (r, raw) = parse_comic(
            r#"{"title": "Fight Comics", "year": "", "issue": "10"}"#,
            Some(r##"{"issue": "#82"}"##),
        )
        .unwrap();
        assert_eq!(r.card.full_name, "Fight Comics #82");
        assert_eq!(raw["top"]["issue"], "#82");
        // A small model's favourite wrong answer is the price: then the issue wasn't read.
        let (r, raw) = parse_comic(
            r#"{"title": "Jumbo Comics", "year": ""}"#,
            Some(r#"{"price": "10¢", "month": "MAY", "issue": "10"}"#),
        )
        .unwrap();
        assert_eq!(r.card.card_number, "");
        assert_eq!(r.card.full_name, "Jumbo Comics");
        assert_eq!(raw["top"]["issue"], "10", "the answer is kept as given");
        let (r, _) = parse_comic(
            r#"{"title": "Jumbo Comics", "year": ""}"#,
            Some(r#"{"price": "10¢", "month": "MAY", "issue": "No. 99"}"#),
        )
        .unwrap();
        assert_eq!(r.card.card_number, "99");
        assert!(!COVER_TOP_PROMPT.contains("129") && !COMIC_PROMPT.contains("129"));
        // An issue's letters are its own: a trading card's "D025/202" would lose its first.
        let (r, _) = parse_comic(r#"{"title": "X", "issue": "A12/3"}"#, None).unwrap();
        assert_eq!(r.card.card_number, "A12/3");
        let (r, _) = parse_comic(r#"{"title": "X", "year": "1999"}"#, Some("no answer")).unwrap();
        assert_eq!(r.card.card_number, "");
        let (empty, _) = parse_comic(r#"{"title": "", "issue": "5"}"#, None).unwrap();
        assert!(empty.confidence.is_none());
        let root = comic_grammar();
        let root = root.lines().next().unwrap();
        let order: Vec<_> = ["title", "issue", "publisher", "year"]
            .iter()
            .map(|f| root.find(&format!(r#"\"{f}\""#)).unwrap())
            .collect();
        assert!(order.windows(2).all(|w| w[0] < w[1]), "{root}");
        assert!(cover_grammar().starts_with(r#"root ::= "{" space "\"title\"""#));
        let top = cover_top_grammar();
        assert!(top.starts_with(r#"root ::= "{" space "\"price\"""#));
        assert!(top.find("month").unwrap() < top.find("issue").unwrap());
    }

    #[test]
    fn grammars_keep_the_field_order() {
        let card = card_grammar(IdMode::Auto);
        let root = card.lines().next().unwrap();
        assert!(
            root.starts_with(r#"root ::= "{" space "\"name\"" space ":" space s80"#),
            "{root}"
        );
        assert!(root.find("year").unwrap() < root.find("game").unwrap());
        assert!(card.contains(r#""\"one_piece\"""#));
        assert!(strip_grammar().starts_with(r#"root ::= "{" space "\"card_number\"""#));
        let whole = whole_grammar(IdMode::Auto);
        let root = whole.lines().next().unwrap();
        let order: Vec<_> = [
            "name",
            "card_number",
            "set_name",
            "set_code",
            "year",
            "game",
        ]
        .iter()
        .map(|f| root.find(&format!(r#"\"{f}\""#)).unwrap())
        .collect();
        assert!(order.windows(2).all(|w| w[0] < w[1]), "{root}");
        assert!(whole.contains("s24 ::=") && whole.contains(r#""\"hockey\"""#));
        // A session of one kind of cards answers only that kind's games.
        let sport = card_grammar(IdMode::Sport);
        assert!(sport.contains(r#""\"hockey\"""#) && !sport.contains(r#""\"pokemon\"""#));
        assert!(whole_grammar(IdMode::Comics).contains("game ::= \"\\\"comics\\\"\"\n"));
    }

    #[test]
    fn tells_sports_cards() {
        assert!(is_sport(
            r#"{"name": "Austin Reaves", "year": "2023", "game": "basketball"}"#
        ));
        assert!(!is_sport(
            r#"{"name": "Pikachu", "year": "1999", "game": "pokemon"}"#
        ));
        assert!(!is_sport("not json"));
    }
}
