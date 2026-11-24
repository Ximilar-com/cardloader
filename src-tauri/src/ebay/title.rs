//! A card's listing title: the parts a template names, in its order, fitted into the characters a listing takes.
//!
//! The user's templates (`Settings::titles`) come first, the first that is for the card; without one the built-in
//! lists make the titles Cardloader always made. eBay, Whatnot and Shopify take the same title
//! (`listing::plan`), unless someone wrote the card its own (`ItemSummary::title`).

use crate::ebay::listing::{self, clip_words, title_game, Graded, Kind, MAX_TITLE};
use crate::model::{
    BulkEdit, CardFields, EditField, EditPlace, ItemEdit, ItemStatus, ItemSummary, SlabLabel,
    TitleField, TitleKind, TitleOnly, TitlePreview, TitleTemplate,
};

use TitleField::*;

const TCG: &[TitleField] = &[Year, Game, Set, Name, Number, Grade];
const SPORT: &[TitleField] = &[Year, Set, Name, Number, Grade];

/// What a title that is too long can do without, what it needs most first. The name, the number and a slab's
/// grade aren't here: they are never left out.
const BY_IMPORTANCE: &[TitleField] = &[
    Serial, Features, Series, Set, SetCode, Brand, Year, Game, Team,
];

/// Parts another part can already say ("Topps" in the set "Topps Chrome", the year in "2023-24 Prizm"), and so
/// would only repeat. Not the set, which names the product even when the parallel repeats it ("Prizm", "Silver
/// Prizm"). Numbers aren't compared: a serial "10/10" isn't said by "PSA 10".
const CAN_REPEAT: &[TitleField] = &[Year, Game, Brand, SetCode, Series, Team, Features];
const SAYS: &[TitleField] = &[Game, Brand, Set, Series, Name, Team, Features];

fn kept(field: TitleField) -> bool {
    matches!(field, Name | Number | Grade)
}

/// The titles without a template of the user's.
fn builtin(kind: Kind) -> &'static [TitleField] {
    match kind {
        Kind::Tcg => TCG,
        Kind::Sport => SPORT,
    }
}

/// A title, and how it came to be.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Composed {
    pub text: String,
    /// The name of the template that made it; empty = the built-in one.
    pub template: String,
    /// Parts the card has that didn't fit.
    pub left_out: Vec<TitleField>,
    /// Even the name, number and grade were too long: the end is cut.
    pub clipped: bool,
}

/// Templates as they are stored: named, with an id, each part once and none this version doesn't know.
pub fn normalized(titles: Vec<TitleTemplate>) -> Vec<TitleTemplate> {
    titles
        .into_iter()
        .map(|mut t| {
            t.name = t.name.trim().to_string();
            t.id = t.id.trim().to_string();
            if t.id.is_empty() {
                t.id = uuid::Uuid::new_v4().to_string();
            }
            let mut fields = Vec::new();
            for field in t.fields {
                if field != Unknown && !fields.contains(&field) {
                    fields.push(field);
                }
            }
            t.fields = fields;
            t
        })
        .collect()
}

/// Whether the features name an autograph: "Auto", "Autograph", "On-Card Auto".
fn autographed(card: &CardFields) -> bool {
    words(&card.features).iter().any(|w| w.starts_with("auto"))
}

fn is_for(t: &TitleTemplate, card: &CardFields, kind: Kind, graded: bool) -> bool {
    let kind = match t.kind {
        TitleKind::Any => true,
        TitleKind::Tcg => kind == Kind::Tcg,
        TitleKind::Sport => kind == Kind::Sport,
        TitleKind::Unknown => false,
    };
    let only = match t.only {
        TitleOnly::Any => true,
        TitleOnly::Graded => graded,
        TitleOnly::Autograph => autographed(card),
        TitleOnly::Unknown => false,
    };
    kind && only
}

fn text_of(field: TitleField, card: &CardFields, kind: Kind, graded: Option<&Graded>) -> String {
    let text = match field {
        Year => card.year.clone(),
        Game => match kind {
            Kind::Tcg => title_game(&card.subcategory).to_string(),
            Kind::Sport => card.subcategory.clone(),
        },
        Brand => card.brand.clone(),
        Set => card.set_name.clone(),
        SetCode => card.set_code.clone(),
        Name => card.name.clone(),
        Series => card.series.clone(),
        Features => card.features.clone(),
        Serial => card.serial.clone(),
        Number => match card.card_number.trim() {
            "" => String::new(),
            number => format!("#{number}"),
        },
        Team => card.team.clone(),
        Grade => graded
            .map(|g| format!("{} {}", g.grader, g.grade))
            .unwrap_or_default(),
        Unknown => String::new(),
    };
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// The parts of `fields` the card has, without those another part already says.
fn parts(
    fields: &[TitleField],
    card: &CardFields,
    kind: Kind,
    graded: Option<&Graded>,
) -> Vec<(TitleField, String)> {
    let mut parts: Vec<(TitleField, String)> = Vec::new();
    for &field in fields {
        let text = text_of(field, card, kind, graded);
        if !text.is_empty() && parts.iter().all(|(f, _)| *f != field) {
            parts.push((field, text));
        }
    }
    // The least important first, so of two parts saying the same the more important stays.
    for field in BY_IMPORTANCE
        .iter()
        .rev()
        .filter(|f| CAN_REPEAT.contains(f))
    {
        let Some(at) = parts.iter().position(|(f, _)| f == field) else {
            continue;
        };
        let mine = words(&parts[at].1);
        let said = parts.iter().enumerate().any(|(i, (other, text))| {
            let theirs = words(text);
            i != at && SAYS.contains(other) && mine.iter().all(|w| theirs.contains(w))
        });
        if said && !mine.is_empty() {
            parts.remove(at);
        }
    }
    parts
}

/// The card's title in at most `room` characters. Too long, it keeps the name, the number and the grade and then
/// takes the other parts while they fit, the most important first, instead of losing its end.
pub fn compose(
    card: &CardFields,
    kind: Kind,
    graded: Option<&Graded>,
    titles: &[TitleTemplate],
    room: usize,
) -> Composed {
    let own = titles
        .iter()
        .filter(|t| is_for(t, card, kind, graded.is_some()))
        .map(|t| (parts(&t.fields, card, kind, graded), t.name.clone()))
        // A template that says nothing about this card (only a team, and it has none) isn't for it.
        .find(|(parts, _)| !parts.is_empty());
    let (parts, template) =
        own.unwrap_or_else(|| (parts(builtin(kind), card, kind, graded), String::new()));

    let length = |text: &str| text.chars().count();
    let mut inside: Vec<bool> = parts.iter().map(|(f, _)| kept(*f)).collect();
    let joined = |inside: &[bool]| {
        parts
            .iter()
            .zip(inside)
            .filter(|(_, inside)| **inside)
            .map(|((_, text), _)| text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    };
    let mut total = length(&joined(&inside));
    let clipped = total > room;
    if !clipped {
        for field in BY_IMPORTANCE {
            if let Some(at) = parts.iter().position(|(f, _)| f == field) {
                let more = length(&parts[at].1) + usize::from(total > 0);
                if total + more <= room {
                    inside[at] = true;
                    total += more;
                }
            }
        }
    }
    let left_out = parts
        .iter()
        .zip(&inside)
        .filter(|(_, inside)| !**inside)
        .map(|((field, _), _)| *field)
        .collect();
    Composed {
        text: clip_words(&joined(&inside), room),
        template,
        left_out,
        clipped,
    }
}

fn collapsed(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Whether `text` already says `added`, as whole words.
fn says(text: &str, added: &str) -> bool {
    format!(" {} ", text.to_lowercase()).contains(&format!(" {} ", added.to_lowercase()))
}

fn joined(place: EditPlace, added: &str, text: &str) -> String {
    match place {
        EditPlace::Start => collapsed(&format!("{added} {text}")),
        EditPlace::End => collapsed(&format!("{text} {added}")),
        EditPlace::Replace => added.to_string(),
    }
}

/// What the card's fields are after `edit`: `None` when it changes nothing (the text is there already), `Err`
/// when the card can't take it. A text added to an automatic title makes it the card's own: the template's
/// title fitted into the room the text leaves, so nothing is cut.
pub fn apply(
    edit: &BulkEdit,
    item: &ItemSummary,
    titles: &[TitleTemplate],
) -> Result<Option<ItemEdit>, String> {
    if !matches!(item.status, ItemStatus::Done | ItemStatus::Grading)
        || item.card.name.trim().is_empty()
    {
        return Err("the card isn't identified".into());
    }
    let added = collapsed(&edit.text);
    if added.is_empty() && edit.place != EditPlace::Replace {
        return Err("no text to add".into());
    }
    let mut after = ItemEdit {
        card: item.card.clone(),
        price: item.price,
        notes: item.notes.clone(),
        location: item.location.clone(),
        title: item.title.clone(),
    };
    let field = match edit.field {
        EditField::Title => {
            let too_long = |title: &str| {
                let length = title.chars().count();
                (length > MAX_TITLE).then(|| {
                    format!(
                        "the title would be {length} characters long; a listing takes {MAX_TITLE}"
                    )
                })
            };
            after.title = if edit.place == EditPlace::Replace {
                added
            } else if !item.title.is_empty() {
                if says(&item.title, &added) {
                    return Ok(None);
                }
                joined(edit.place, &added, &item.title)
            } else {
                let plan = listing::plan(item, titles)?;
                if says(&plan.automatic, &added) {
                    return Ok(None);
                }
                let room = MAX_TITLE.saturating_sub(added.chars().count() + 1);
                let made = compose(&item.card, plan.kind, plan.graded.as_ref(), titles, room);
                if made.clipped || made.text.is_empty() {
                    return Err(
                        "the text leaves no room for the card's name, number and grade".into(),
                    );
                }
                joined(edit.place, &added, &made.text)
            };
            if let Some(why) = too_long(&after.title) {
                return Err(why);
            }
            return Ok((after.title != item.title).then_some(after));
        }
        EditField::Features => &mut after.card.features,
        EditField::Brand => &mut after.card.brand,
        EditField::Team => &mut after.card.team,
        EditField::Series => &mut after.card.series,
    };
    if edit.place != EditPlace::Replace && says(field, &added) {
        return Ok(None);
    }
    *field = joined(edit.place, &added, field);
    Ok((after.card != item.card).then_some(after))
}

/// The card's listing title as it is, and as `edit` would make it.
pub fn preview(
    item: &ItemSummary,
    edit: Option<&BulkEdit>,
    titles: &[TitleTemplate],
) -> TitlePreview {
    let now = listing::plan(item, titles);
    let mut shown = TitlePreview {
        item_id: Some(item.id),
        seq: item.seq,
        thumb_path: item.thumb_path.clone(),
        before: now.as_ref().map(|p| p.title.clone()).unwrap_or_default(),
        ..Default::default()
    };
    let (after, changed) = match edit.map(|e| apply(e, item, titles)) {
        Some(Ok(Some(after))) => {
            let item = ItemSummary {
                card: after.card,
                title: after.title,
                ..item.clone()
            };
            (listing::plan(&item, titles), true)
        }
        Some(Err(why)) => {
            shown.problem = Some(why);
            (now, false)
        }
        Some(Ok(None)) | None => (now, false),
    };
    shown.changed = changed;
    match after {
        Ok(plan) => {
            shown.title = plan.title;
            shown.custom = plan.custom;
            shown.automatic = plan.automatic;
            shown.template = plan.template;
            if !plan.custom {
                shown.left_out = plan.left_out;
            }
        }
        // A card that can't be listed has no title to show; why it can't take a change says more.
        Err(why) => shown.problem = shown.problem.or(Some(why)),
    }
    shown
}

/// Made-up cards for the template editor while the user has none: a sports card with everything a title can
/// say, a graded one and a trading card.
pub fn samples() -> Vec<ItemSummary> {
    let sample =
        |seq: i64, id_type: &str, card: CardFields, slab: Option<(&str, &str)>| ItemSummary {
            id: 0,
            session_id: String::new(),
            seq,
            status: ItemStatus::Done,
            front_path: String::new(),
            back_path: None,
            thumb_path: None,
            id_type: Some(id_type.into()),
            card,
            confidence: None,
            distance: None,
            price: None,
            price_currency: crate::model::USD.into(),
            grade: None,
            condition: None,
            slab: slab.map(|(company, grade)| SlabLabel {
                company: company.into(),
                grade: grade.into(),
                certificate: String::new(),
            }),
            error: None,
            synced: false,
            updated_at: String::new(),
            picked: false,
            edited: false,
            reviewed: false,
            agent_reviewed: false,
            review_note: String::new(),
            notes: String::new(),
            location: String::new(),
            title: String::new(),
            ebay: None,
            market_range: None,
        };
    let card = |fields: [&str; 10]| {
        let [name, set_name, card_number, series, year, subcategory, brand, team, serial, features] =
            fields.map(str::to_string);
        CardFields {
            name,
            set_name,
            card_number,
            series,
            year,
            subcategory,
            brand,
            team,
            serial,
            features,
            ..Default::default()
        }
    };
    vec![
        sample(
            1,
            "sport",
            card([
                "Victor Wembanyama",
                "Prizm",
                "136",
                "Silver Prizm",
                "2023",
                "Basketball",
                "Panini",
                "San Antonio Spurs",
                "",
                "RC",
            ]),
            None,
        ),
        sample(
            2,
            "sport",
            card([
                "Jazz Chisholm Jr.",
                "Finest",
                "AU-9",
                "Lava",
                "2026",
                "Baseball",
                "Topps",
                "New York Yankees",
                "43/75",
                "Auto",
            ]),
            Some(("PSA", "9")),
        ),
        sample(
            3,
            "tcg",
            card([
                "Charizard ex",
                "Obsidian Flames",
                "125",
                "",
                "2023",
                "Pokemon",
                "",
                "",
                "",
                "",
            ]),
            None,
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card() -> CardFields {
        CardFields {
            name: "Jazz Chisholm Jr.".into(),
            set_name: "Finest".into(),
            card_number: "AU-9".into(),
            series: "Lava".into(),
            year: "2026".into(),
            subcategory: "Baseball".into(),
            brand: "Topps".into(),
            team: "New York Yankees".into(),
            serial: "43/75".into(),
            ..Default::default()
        }
    }

    fn template(name: &str, fields: &[TitleField]) -> TitleTemplate {
        TitleTemplate {
            name: name.into(),
            fields: fields.to_vec(),
            ..Default::default()
        }
    }

    fn psa(grade: &str) -> Graded {
        Graded {
            grader: "PSA".into(),
            grader_id: "275010",
            grade: grade.into(),
            grade_id: String::new(),
            certificate: String::new(),
        }
    }

    fn title(card: &CardFields, titles: &[TitleTemplate]) -> String {
        compose(card, Kind::Sport, None, titles, MAX_TITLE).text
    }

    #[test]
    fn without_a_template_titles_are_the_built_in_ones() {
        let made = compose(&card(), Kind::Sport, None, &[], MAX_TITLE);
        assert_eq!(made.text, "2026 Finest Jazz Chisholm Jr. #AU-9");
        assert_eq!((made.template.as_str(), made.left_out.len()), ("", 0));
        let tcg = CardFields {
            subcategory: "Magic The Gathering".into(),
            ..card()
        };
        assert_eq!(
            compose(&tcg, Kind::Tcg, Some(&psa("9")), &[], MAX_TITLE).text,
            "2026 MTG Finest Jazz Chisholm Jr. #AU-9 PSA 9"
        );
    }

    #[test]
    fn a_template_names_the_parts_and_their_order() {
        let all = template(
            "Sports",
            &[
                Year, Brand, Set, Name, Series, Features, Serial, Number, Team, Grade, Game,
            ],
        );
        let made = compose(
            &card(),
            Kind::Sport,
            Some(&psa("10")),
            std::slice::from_ref(&all),
            MAX_TITLE,
        );
        assert_eq!(
            made.text,
            "2026 Topps Finest Jazz Chisholm Jr. Lava 43/75 #AU-9 PSA 10 Baseball"
        );
        // The team comes last in what a title needs, and no longer fits.
        assert_eq!(
            (made.template.as_str(), made.left_out),
            ("Sports", vec![Team])
        );
        assert_eq!(
            title(&card(), &[template("Short", &[Number, Name, Serial])]),
            "#AU-9 Jazz Chisholm Jr. 43/75"
        );
    }

    #[test]
    fn the_first_template_for_the_card_makes_its_title() {
        let graded = TitleTemplate {
            only: TitleOnly::Graded,
            ..template("Graded", &[Grade, Name])
        };
        let autographs = TitleTemplate {
            only: TitleOnly::Autograph,
            kind: TitleKind::Sport,
            ..template("Autographs", &[Name, Features])
        };
        let tcg = TitleTemplate {
            kind: TitleKind::Tcg,
            ..template("TCG", &[Name])
        };
        let newer = TitleTemplate {
            only: TitleOnly::Unknown,
            ..template("From a newer version", &[Name])
        };
        let teams = template("Teams", &[Team]);
        let titles = [newer, tcg, graded, autographs, teams];
        let made = |card: &CardFields, graded: Option<&Graded>| {
            let made = compose(card, Kind::Sport, graded, &titles, MAX_TITLE);
            (made.text, made.template)
        };
        assert_eq!(
            made(&card(), Some(&psa("9"))),
            ("PSA 9 Jazz Chisholm Jr.".into(), "Graded".into())
        );
        let signed = CardFields {
            features: "RC On-Card Auto".into(),
            ..card()
        };
        assert_eq!(
            made(&signed, None),
            (
                "Jazz Chisholm Jr. RC On-Card Auto".into(),
                "Autographs".into()
            )
        );
        assert_eq!(
            made(&card(), None),
            ("New York Yankees".into(), "Teams".into())
        );
        // A template with nothing to say about the card leaves it to the next, in the end the built-in one.
        let no_team = CardFields {
            team: String::new(),
            ..card()
        };
        assert_eq!(
            made(&no_team, None),
            ("2026 Finest Jazz Chisholm Jr. #AU-9".into(), String::new())
        );
    }

    #[test]
    fn a_long_title_keeps_what_matters_and_fills_the_room() {
        let all = [template(
            "All",
            &[
                Year, Brand, Set, Name, Series, Features, Serial, Number, Team,
            ],
        )];
        let long = CardFields {
            set_name: "Finest Flashbacks Anniversary".into(),
            brand: "Upper Deck".into(),
            features: "RC Auto".into(),
            ..card()
        };
        let made = compose(&long, Kind::Sport, None, &all, MAX_TITLE);
        // The brand doesn't fit any more; the year after it, shorter, still does.
        assert_eq!(
            made.text,
            "2026 Finest Flashbacks Anniversary Jazz Chisholm Jr. Lava RC Auto 43/75 #AU-9"
        );
        assert_eq!(made.text.chars().count(), 77);
        assert_eq!(made.left_out, vec![Brand, Team]);
        assert!(!made.clipped);

        // In less room (a text goes in front of it), more is left out, the least needed first.
        let made = compose(&long, Kind::Sport, None, &all, 40);
        assert_eq!(made.text, "Jazz Chisholm Jr. RC Auto 43/75 #AU-9");
        let made = compose(&long, Kind::Sport, None, &all, 30);
        assert_eq!(made.text, "Jazz Chisholm Jr. 43/75 #AU-9");

        // Only a name too long for a title loses its end.
        let endless = CardFields {
            name: "word ".repeat(30),
            ..card()
        };
        let made = compose(&endless, Kind::Sport, None, &all, MAX_TITLE);
        assert!(made.clipped && made.text.chars().count() <= MAX_TITLE);
        assert!(made.text.starts_with("word word"));
    }

    #[test]
    fn a_part_another_one_says_isnt_repeated() {
        let fields = [Year, Game, Brand, Set, Name, Series, Serial, Grade];
        let said = CardFields {
            year: "2023".into(),
            set_name: "2023-24 Topps Chrome".into(),
            // Without a sub-set Ximilar's answer is read with the brand as the series.
            series: "topps".into(),
            serial: "10/10".into(),
            subcategory: "Basketball".into(),
            ..card()
        };
        let made = compose(
            &said,
            Kind::Sport,
            Some(&psa("10")),
            &[template("All", &fields)],
            MAX_TITLE,
        );
        assert_eq!(
            made.text,
            "Basketball 2023-24 Topps Chrome Jazz Chisholm Jr. 10/10 PSA 10"
        );
        assert!(made.left_out.is_empty(), "nothing was left out for room");
        // The built-in titles too: the game in the set's name.
        let pokemon = CardFields {
            name: "Mew ex".into(),
            set_name: "Pokemon 151".into(),
            card_number: "151".into(),
            year: "2023".into(),
            subcategory: "Pokemon".into(),
            ..Default::default()
        };
        assert_eq!(
            compose(&pokemon, Kind::Tcg, None, &[], MAX_TITLE).text,
            "2023 Pokemon 151 Mew ex #151"
        );
    }

    #[test]
    fn a_text_for_many_cards_goes_into_the_title_or_a_field() {
        let cards = samples();
        let (rookie, slab) = (&cards[0], &cards[1]);
        let add = |field, place, text: &str| BulkEdit {
            field,
            place,
            text: text.into(),
        };
        let titled = |item: &ItemSummary, edit: &BulkEdit, titles: &[TitleTemplate]| {
            apply(edit, item, titles).map(|after| after.map(|a| a.title))
        };
        let all = [template(
            "All",
            &[
                Year, Brand, Set, Name, Series, Features, Number, Team, Grade,
            ],
        )];
        assert_eq!(
            preview(rookie, None, &all).title,
            "2023 Panini Prizm Victor Wembanyama Silver Prizm RC #136 San Antonio Spurs"
        );
        // The text takes its room first, and the title is fitted into what is left: the team goes, not the end.
        let flagship = add(EditField::Title, EditPlace::End, "Flagship Edition");
        let shown = preview(rookie, Some(&flagship), &all);
        assert_eq!(
            shown.title,
            "2023 Panini Prizm Victor Wembanyama Silver Prizm RC #136 Flagship Edition"
        );
        assert!(shown.changed && shown.custom && shown.left_out.is_empty());
        assert_eq!(shown.before, shown.automatic);
        assert_eq!(
            titled(rookie, &add(EditField::Title, EditPlace::Start, "HOT"), &[]),
            Ok(Some("HOT 2023 Prizm Victor Wembanyama #136".into()))
        );
        // What the title says already isn't said twice, and a card's own title only grows while it fits.
        let psa = add(EditField::Title, EditPlace::End, "psa 9");
        assert_eq!(titled(slab, &psa, &[]), Ok(None));
        let own = ItemSummary {
            title: "x".repeat(76),
            ..rookie.clone()
        };
        assert!(titled(&own, &psa, &[])
            .unwrap_err()
            .contains("82 characters"));
        assert_eq!(
            titled(&own, &add(EditField::Title, EditPlace::Replace, ""), &[]),
            Ok(Some(String::new()))
        );
        let crowded = add(EditField::Title, EditPlace::Start, &"x".repeat(70));
        assert!(titled(rookie, &crowded, &[])
            .unwrap_err()
            .contains("no room"));

        // A field: the template decides where it shows. The series is what tells the card from another.
        let auto = add(EditField::Features, EditPlace::End, "Auto");
        let after = apply(&auto, rookie, &all).unwrap().unwrap();
        assert_eq!(
            (after.card.features.as_str(), after.title.as_str()),
            ("RC Auto", "")
        );
        assert_eq!(apply(&auto, slab, &all), Ok(None), "it says Auto already");
        assert!(preview(rookie, Some(&auto), &all)
            .title
            .contains("Silver Prizm RC Auto #136 San"));

        let queued = ItemSummary {
            status: ItemStatus::Queued,
            ..rookie.clone()
        };
        assert!(apply(&auto, &queued, &all).is_err());
        let comic = ItemSummary {
            id_type: Some("comics".into()),
            ..rookie.clone()
        };
        assert!(preview(&comic, Some(&flagship), &all).problem.is_some());
    }

    #[test]
    fn stored_templates_are_tidied() {
        let stored = normalized(vec![TitleTemplate {
            name: "  Mine ".into(),
            fields: vec![Name, Unknown, Year, Name],
            ..Default::default()
        }]);
        assert_eq!(stored[0].name, "Mine");
        assert_eq!(stored[0].fields, vec![Name, Year]);
        assert!(!stored[0].id.is_empty());
    }
}
