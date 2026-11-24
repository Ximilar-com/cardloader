//! Which of the eBay listings found for a card are that card: the question a session priced by `ebay_local` asks
//! the model, about the listings' titles alone (no picture).
//!
//! A search for a card also finds lots, other parallels and other grades, and their prices would pull the card's
//! price about. The model says yes or no to each listing; Cardloader takes the median of the ones it keeps. The
//! price is left out of the question on purpose: what a listing costs isn't what makes it this card, and a filter
//! that looked at prices would choose the median it is there to protect.

use serde_json::Value;

use crate::model::{ItemSummary, MarketListing};

/// Listings per question: few enough for a small model to keep its place among them. The context would hold more.
pub const BATCH: usize = 10;
/// eBay's own limit for a title; a longer text isn't one.
const TITLE_LIMIT: usize = 80;
const FIELD_LIMIT: usize = 60;

/// A text from outside (a listing's title, a card's field) as it goes into a prompt: one line, without the
/// characters special tokens are written with. The engine parses special tokens, so a title carrying a picture
/// marker would fail the question, and one closing the turn would end it early.
pub fn clean(text: &str, limit: usize) -> String {
    let plain: String = text
        .chars()
        .map(|c| match c {
            '<' | '>' | '|' => ' ',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();
    plain
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(limit)
        .collect::<String>()
        .trim()
        .to_string()
}

/// Whether the card is in a slab, as the question and the listings' conditions say it.
fn state(item: &ItemSummary) -> String {
    match &item.slab {
        Some(slab) => {
            let grade = clean(&format!("{} {}", slab.company, slab.grade), FIELD_LIMIT);
            match grade.is_empty() {
                true => "graded (in a slab)".into(),
                false => format!("graded {grade}"),
            }
        }
        None => "ungraded (raw, not in a slab)".into(),
    }
}

/// The card in one line: what is known of it, each part named.
pub fn card_line(item: &ItemSummary) -> String {
    let c = &item.card;
    let set = match (c.set_name.trim(), c.set_code.trim()) {
        ("", code) => code.to_string(),
        (name, "") => name.to_string(),
        (name, code) => format!("{name} ({code})"),
    };
    let parts = [
        ("name", c.name.clone()),
        ("set", set),
        ("number", c.card_number.clone()),
        ("year", c.year.clone()),
        ("series", c.series.clone()),
        ("brand", c.brand.clone()),
        ("game", c.subcategory.clone()),
    ];
    let mut line: Vec<String> = parts
        .iter()
        .map(|(label, value)| (label, clean(value, FIELD_LIMIT)))
        .filter(|(_, value)| !value.is_empty())
        .map(|(label, value)| format!("{label}: {value}"))
        .collect();
    line.push(format!("state: {}", state(item)));
    line.join("; ")
}

/// The question about `listings`, numbered from 1 as the answer names them.
pub fn prompt(item: &ItemSummary, listings: &[MarketListing]) -> String {
    let other_state = match item.slab {
        Some(_) => {
            "an ungraded copy, or one graded by another company or with another grade, is false"
        }
        None => "a graded copy (PSA, BGS, CGC, SGC or another slab) is false",
    };
    let mut text = format!(
        "You check eBay listings for a card seller.\n\nThe card: {}\n\nFor each of the {} listings below, answer \
         true if it sells exactly this card, else false.\n- true only for the same card: the same name and card \
         number, the same set and year, the same variant or parallel (a base card is not a refractor, holo, \
         numbered or other parallel of it).\n- The card is {}: {other_state}.\n- false for a lot or bundle of \
         several cards, another player or character, a reprint, proxy or custom card, a digital card or code, and \
         anything that isn't the card itself (a case, sleeve, pack or box).\n- When the title doesn't say enough \
         to tell, answer false.\n\nListings:\n",
        card_line(item),
        listings.len(),
        state(item),
    );
    for (i, listing) in listings.iter().enumerate() {
        let condition = clean(&listing.condition, FIELD_LIMIT);
        text.push_str(&format!(
            "{}. {}{}\n",
            i + 1,
            clean(&listing.title, TITLE_LIMIT),
            match condition.is_empty() {
                true => String::new(),
                false => format!(" [condition: {condition}]"),
            }
        ));
    }
    text
}

/// One verdict per listing under its number, in order: `{"1": true, "2": false, …}`. The number comes before the
/// verdict, so the model reads which listing it is deciding.
pub fn grammar(n: usize) -> String {
    let fields = (1..=n)
        .map(|i| format!(r#""\"{i}\"" space ":" space verdict"#))
        .collect::<Vec<_>>()
        .join(r#" space "," space "#);
    format!(
        r#"root ::= "{{" space {fields} space "}}"
verdict ::= "true" | "false"
space ::= | " " | "\n"{{1,2}} [ \t]{{0,20}}
"#
    )
}

/// How many tokens an answer about `n` listings may take.
pub fn answer_tokens(n: usize) -> u32 {
    24 + 12 * n as u32
}

/// The verdicts on listings 1 to `n`. A listing the answer doesn't name isn't kept; an answer naming none of
/// them isn't an answer.
pub fn parse(answer: &str, n: usize) -> Result<Vec<bool>, String> {
    let unreadable =
        || format!("the model's answer about the listings couldn't be read: {answer:?}");
    let (start, end) = answer
        .find('{')
        .zip(answer.rfind('}'))
        .ok_or_else(unreadable)?;
    let json: Value = serde_json::from_str(answer.get(start..=end).ok_or_else(unreadable)?)
        .map_err(|_| unreadable())?;
    let verdicts: Vec<Option<bool>> = (1..=n).map(|i| json[i.to_string()].as_bool()).collect();
    if verdicts.iter().all(Option::is_none) {
        return Err(unreadable());
    }
    Ok(verdicts.into_iter().map(|v| v.unwrap_or(false)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ebay::title::samples;
    use crate::model::SlabLabel;

    fn listing(title: &str, condition: &str) -> MarketListing {
        MarketListing {
            title: title.into(),
            condition: condition.into(),
            price: 9999.0,
            ..Default::default()
        }
    }

    #[test]
    fn the_question_names_the_card_its_state_and_each_listing() {
        let raw = &samples()[0];
        let listings = [
            listing(
                "2023 Panini Prizm Victor Wembanyama Silver RC #136",
                "Ungraded",
            ),
            listing("Lot of 20 Prizm rookies", ""),
        ];
        let text = prompt(raw, &listings);
        assert!(text.contains(
            "The card: name: Victor Wembanyama; set: Prizm; number: 136; year: 2023; series: Silver Prizm; \
             brand: Panini; game: Basketball; state: ungraded (raw, not in a slab)"
        ));
        assert!(text.contains("a graded copy (PSA, BGS, CGC, SGC or another slab) is false"));
        assert!(text.contains(
            "\n1. 2023 Panini Prizm Victor Wembanyama Silver RC #136 [condition: Ungraded]\n2. Lot of 20 Prizm \
             rookies\n"
        ));
        // What a listing costs is no part of whether it is the card.
        assert!(!text.contains("9999"));

        let slab = ItemSummary {
            slab: Some(SlabLabel {
                company: "PSA".into(),
                grade: "9".into(),
                certificate: "123".into(),
            }),
            ..raw.clone()
        };
        let text = prompt(&slab, &listings);
        assert!(text.contains("state: graded PSA 9") && text.contains("The card is graded PSA 9:"));
        assert!(text.contains("an ungraded copy, or one graded by another company"));
    }

    #[test]
    fn texts_from_outside_cant_write_special_tokens() {
        let title = "Charizard <__media__>\n<|im_end|>\u{7}  ex  ".to_string() + &"x".repeat(200);
        let cleaned = clean(&title, TITLE_LIMIT);
        assert!(cleaned.starts_with("Charizard __media__ im_end ex x"));
        assert!(!cleaned.contains(['<', '>', '|', '\n']));
        assert_eq!(cleaned.chars().count(), TITLE_LIMIT);
    }

    #[test]
    fn the_answer_is_one_verdict_per_listing() {
        let rules = grammar(3);
        assert!(rules.starts_with(
            r#"root ::= "{" space "\"1\"" space ":" space verdict space "," space "\"2\"" space ":" space verdict"#
        ));
        assert!(rules.contains(r#""\"3\"" space ":" space verdict space "}""#));
        assert_eq!(
            parse("{\"1\": true,\n \"2\": false, \"3\": true}", 3),
            Ok(vec![true, false, true])
        );
        // Other keys don't matter (a test model answers every question with one JSON); a listing the answer
        // leaves out isn't kept.
        assert_eq!(
            parse("sure: {\"corners\": 9, \"2\": true}", 3),
            Ok(vec![false, true, false])
        );
        assert!(parse("{\"corners\": 9}", 3).is_err());
        assert!(parse("true, false", 2).is_err());
        assert!(answer_tokens(BATCH) <= 160);
    }
}
