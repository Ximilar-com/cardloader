//! Asking a vision language model what card a photo shows, and reading its answer.
//!
//! Two short questions, each constrained to a small JSON object by a llama.cpp grammar:
//! 1. the whole card: name, year and game;
//! 2. the bottom strip, enlarged: collector number and set code — the small print a 450M model can't read
//!    from the whole card (asked about the whole card, it reads the HP or attack text instead).
//!
//! The set *name* isn't asked for: small models fill it with whatever text is on the card.

use serde::Deserialize;
use serde_json::{json, Value};

use crate::model::CardFields;
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

pub fn card_grammar() -> String {
    let games = GAMES
        .iter()
        .map(|g| format!(r#""\"{}\"""#, g.0))
        .collect::<Vec<_>>()
        .join(" | ");
    object_grammar(
        &[("name", "s80"), ("year", "s8"), ("game", "game")],
        &format!("game ::= {games}\n"),
    )
}

pub fn strip_grammar() -> String {
    object_grammar(&[("card_number", "s16"), ("set_code", "s16")], "")
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct CardAnswer {
    name: String,
    year: String,
    game: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct StripAnswer {
    card_number: String,
    set_code: String,
}

fn json_object(text: &str) -> Result<Value, String> {
    let start = text.find('{').ok_or("the model gave no answer")?;
    let end = text.rfind('}').ok_or("the model's answer was cut off")?;
    serde_json::from_str(&text[start..=end])
        .map_err(|e| format!("the model's answer is not valid JSON: {e}"))
}

fn clean(text: &str) -> String {
    let t = text.trim();
    if ["unknown", "n/a", "none", "-", "null", "0"].contains(&t.to_lowercase().as_str()) {
        String::new()
    } else {
        t.to_string()
    }
}

/// The collector number without what models copy around it: a leading rarity letter ("C 0123", Magic) or
/// regulation mark ("D025/202", Pokémon). "125197" read without its slash is "125/197": six digits are taken as
/// two halves.
fn collector_number(raw: &str) -> String {
    let n = clean(raw).trim_start_matches('#').trim().to_string();
    let mut chars = n.chars();
    let n = match (chars.next(), chars.as_str().trim_start()) {
        (Some(c), rest)
            if c.is_ascii_alphabetic() && rest.starts_with(|d: char| d.is_ascii_digit()) =>
        {
            rest.to_string()
        }
        _ => n,
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
    let a: CardAnswer = serde_json::from_value(card_raw.clone()).map_err(|e| e.to_string())?;
    let strip_raw = strip.and_then(|s| json_object(s).ok());
    let s: StripAnswer = strip_raw
        .clone()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    let game = GAMES.iter().find(|g| g.0 == a.game.trim());
    let year: String = clean(&a.year)
        .chars()
        .filter(char::is_ascii_digit)
        .take(4)
        .collect();
    let set_code = clean(&s.set_code).to_uppercase();
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
        card_number: collector_number(&s.card_number),
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
    let recognition = Recognition {
        id_type: game.map(|g| g.1).unwrap_or("tcg").to_string(),
        confidence: (!card.name.is_empty()).then(|| LOCAL_CONFIDENCE.to_string()),
        card,
        ..Default::default()
    };
    Ok((recognition, json!({"card": card_raw, "strip": strip_raw})))
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
        assert_eq!(collector_number("C 0123"), "0123");
        assert_eq!(collector_number("D025/202"), "025/202");
        assert_eq!(collector_number("SV107"), "SV107");
        assert_eq!(collector_number("TG05/TG30"), "TG05/TG30");
        assert_eq!(r.card.year, "");
        let (empty, _) = parse_answers(r#"{"name":"","year":"","game":"other"}"#, None).unwrap();
        assert!(empty.card.full_name.is_empty() && empty.confidence.is_none());
        assert!(parse_answers("I cannot see a card", None).is_err());
    }

    #[test]
    fn grammars_keep_the_field_order() {
        let card = card_grammar();
        let root = card.lines().next().unwrap();
        assert!(
            root.starts_with(r#"root ::= "{" space "\"name\"" space ":" space s80"#),
            "{root}"
        );
        assert!(root.find("year").unwrap() < root.find("game").unwrap());
        assert!(card.contains(r#""\"one_piece\"""#));
        assert!(strip_grammar().starts_with(r#"root ::= "{" space "\"card_number\"""#));
    }
}
