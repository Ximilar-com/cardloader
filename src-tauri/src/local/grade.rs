//! Grading and condition with the local model: what to ask about each side of a card, and how the answers
//! become a grade and a condition.
//!
//! Each side is asked one question, constrained by a llama.cpp grammar:
//! - grade / centering: the four subgrades from 1 to 10 (half points allowed);
//! - condition: one label of the chosen marketplace's scale.
//!
//! The card takes the worse side (corners, edges and surface wear show on either side; centering is the
//! front's). The final grade is the subgrades' mean, capped one point above the weakest, as a grader's
//! final grade is held down by the worst attribute. These are a small model's estimates, not a grader's.

use serde::Deserialize;
use serde_json::{json, Value};

use crate::model::GradingMode;

/// A condition scale: its labels from best to worst, each with what it means (for the prompt) and the
/// lowest final grade it covers (to name the condition of a graded card).
pub struct Scale {
    pub id: &'static str,
    pub name: &'static str,
    pub labels: &'static [(&'static str, &'static str, f64)],
}

pub const SCALES: &[Scale] = &[
    Scale {
        id: "tcgplayer",
        name: "TCGplayer",
        labels: &[
            (
                "Near Mint",
                "minimal or no wear: sharp corners, clean edges, no scratches",
                8.0,
            ),
            (
                "Lightly Played",
                "minor whitening on edges or corners, a few light scratches",
                6.5,
            ),
            (
                "Moderately Played",
                "clear edge and corner wear, scratches, small creases",
                4.5,
            ),
            (
                "Heavily Played",
                "major wear: heavy whitening, creases or bends",
                2.5,
            ),
            (
                "Damaged",
                "tears, water damage, heavy creases or missing pieces",
                0.0,
            ),
        ],
    },
    Scale {
        id: "cardmarket",
        name: "Cardmarket",
        labels: &[
            ("Mint", "perfect, no wear at all", 9.5),
            (
                "Near Mint",
                "nearly perfect, tiny imperfections at most",
                8.0,
            ),
            (
                "Excellent",
                "light wear on a few edges or corners, a few light scratches",
                7.0,
            ),
            (
                "Good",
                "visible wear: whitening on edges and corners, scratches",
                5.5,
            ),
            ("Light Played", "obvious wear, small creases or bends", 4.0),
            ("Played", "heavy wear, creases", 2.5),
            ("Poor", "tears, water damage or missing pieces", 0.0),
        ],
    },
    Scale {
        id: "ebay_tcg",
        name: "eBay trading cards",
        labels: &[
            (
                "Near Mint or Better",
                "minimal or no wear: sharp corners, clean edges",
                8.0,
            ),
            (
                "Lightly Played (Excellent)",
                "minor edge or corner wear, light scratches",
                6.5,
            ),
            (
                "Moderately Played (Very Good)",
                "clear wear, scratches, small creases",
                4.5,
            ),
            (
                "Heavily Played (Poor)",
                "major wear, creases, bends or damage",
                0.0,
            ),
        ],
    },
    Scale {
        id: "ebay_sport",
        name: "eBay sports cards",
        labels: &[
            (
                "Near Mint or Better",
                "minimal or no wear: sharp corners, clean edges",
                8.0,
            ),
            (
                "Excellent",
                "minor edge or corner wear, light scratches",
                6.5,
            ),
            (
                "Very Good",
                "clear wear, rounded corners, small creases",
                4.5,
            ),
            ("Poor", "major wear, creases, bends or damage", 0.0),
        ],
    },
];

/// The scale a session's `condition_mode` names. Ximilar's own modes map to the closest one.
pub fn scale(mode: &str) -> &'static Scale {
    let id = match mode {
        "ebay" => "ebay_sport",
        "ximilar" => "tcgplayer",
        m => m,
    };
    SCALES.iter().find(|s| s.id == id).unwrap_or(&SCALES[0])
}

/// The steps are spelled out: told only "1 to 10" (or that a crease is 4 or lower), the small models answer
/// the same low numbers for every card.
pub const GRADE_PROMPT: &str = "Look closely at the corners, edges and surface of this trading card. \
Rate each from 1 to 10: 10 = like new from a pack, 9 = nearly perfect, 8 = one tiny flaw, 7 = minor wear, \
6 = light wear in several places, 5 = clear wear, 3 = heavy wear or a crease, 1 = badly damaged. \
centering: 10 when the borders are equally wide on all sides, lower the more uneven they are.";

pub fn condition_prompt(scale: &Scale) -> String {
    let labels = scale
        .labels
        .iter()
        .map(|(label, meaning, _)| format!("{label}: {meaning}"))
        .collect::<Vec<_>>()
        .join("; ");
    format!(
        "This is a photo of one side of a collectible card. \
Judge the wear on its corners, edges and surface and pick its condition on the {} scale: {labels}. \
Judge only what you can see.",
        scale.name
    )
}

const SUBGRADE: &str = r#"subgrade ::= "10" | [1-9] | [1-9] ".5"
space ::= | " " | "\n"{1,2} [ \t]{0,20}
"#;

pub fn grade_grammar() -> String {
    let fields = ["corners", "edges", "surface", "centering"]
        .iter()
        .map(|k| format!(r#""\"{k}\"" space ":" space subgrade"#))
        .collect::<Vec<_>>()
        .join(r#" "," space "#);
    format!("root ::= \"{{\" space {fields} space \"}}\"\n{SUBGRADE}")
}

pub fn condition_grammar(scale: &Scale) -> String {
    let labels = scale
        .labels
        .iter()
        .map(|(label, _, _)| format!(r#""\"{label}\"""#))
        .collect::<Vec<_>>()
        .join(" | ");
    format!(
        "root ::= \"{{\" space \"\\\"condition\\\"\" space \":\" space label space \"}}\"\nlabel ::= {labels}\n\
space ::= | \" \" | \"\\n\"{{1,2}} [ \\t]{{0,20}}\n"
    )
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Subgrades {
    pub corners: f64,
    pub edges: f64,
    pub surface: f64,
    pub centering: f64,
}

impl Subgrades {
    fn clamped(self) -> Self {
        let c = |v: f64| {
            if v.is_finite() {
                v.clamp(1.0, 10.0)
            } else {
                1.0
            }
        };
        Subgrades {
            corners: c(self.corners),
            edges: c(self.edges),
            surface: c(self.surface),
            centering: c(self.centering),
        }
    }

    /// The mean, capped one point above the weakest subgrade, rounded down to half a point.
    pub fn final_grade(&self) -> f64 {
        let all = [self.corners, self.edges, self.surface, self.centering];
        let mean = all.iter().sum::<f64>() / 4.0;
        let worst = all.iter().cloned().fold(f64::INFINITY, f64::min);
        (mean.min(worst + 1.0) * 2.0).floor() / 2.0
    }

    /// The card from its sides: the worse corners, edges and surface, the front's centering.
    fn card(sides: &[Subgrades]) -> Subgrades {
        let worst = |f: fn(&Subgrades) -> f64| sides.iter().map(f).fold(f64::INFINITY, f64::min);
        Subgrades {
            corners: worst(|s| s.corners),
            edges: worst(|s| s.edges),
            surface: worst(|s| s.surface),
            centering: sides[0].centering,
        }
    }
}

/// The condition label of a final grade on a scale.
pub fn condition_of(scale: &Scale, grade: f64) -> &'static str {
    scale
        .labels
        .iter()
        .find(|(_, _, min)| grade >= *min)
        .or(scale.labels.last())
        .map(|(label, _, _)| *label)
        .unwrap_or_default()
}

/// The result of local grading, in the shape the app reads from Ximilar's (`records[0].grades`, the condition
/// in `records[0]._objects[0].Condition`).
#[derive(Debug, Clone, PartialEq)]
pub struct LocalGrading {
    pub grade: Option<f64>,
    pub condition: Option<String>,
    pub raw: Value,
}

fn json_object(text: &str) -> Result<Value, String> {
    let start = text.find('{').ok_or("the model gave no answer")?;
    let end = text.rfind('}').ok_or("the model's answer was cut off")?;
    serde_json::from_str(&text[start..=end])
        .map_err(|e| format!("the model's answer is not valid JSON: {e}"))
}

/// Reads the answers for each side (front first) into a grade or a condition.
pub fn parse(mode: GradingMode, scale: &Scale, answers: &[String]) -> Result<LocalGrading, String> {
    if answers.is_empty() {
        return Err("no image to grade".into());
    }
    let parsed = answers
        .iter()
        .map(|a| json_object(a))
        .collect::<Result<Vec<_>, _>>()?;
    match mode {
        GradingMode::Condition => {
            // The worse side, by its place on the scale.
            let rank = |v: &Value| {
                let label = v["condition"].as_str().unwrap_or_default();
                scale.labels.iter().position(|(l, _, _)| *l == label)
            };
            let worst = parsed
                .iter()
                .filter_map(rank)
                .max()
                .ok_or("the model gave no condition")?;
            let label = scale.labels[worst].0;
            Ok(LocalGrading {
                grade: None,
                condition: Some(label.into()),
                raw: json!({"records": [{
                    "_id": "card",
                    "_objects": [{"Condition": [{"label": label, "mode": scale.name}]}],
                }], "sides": parsed}),
            })
        }
        _ => {
            let sides = parsed
                .iter()
                .map(|v| serde_json::from_value::<Subgrades>(v.clone()).map(Subgrades::clamped))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| format!("the model's grades are incomplete: {e}"))?;
            let card = Subgrades::card(&sides);
            if mode == GradingMode::Centering {
                return Ok(LocalGrading {
                    grade: Some(card.centering),
                    condition: None,
                    raw: json!({"records": [{"_id": "card", "grades": {"centering": card.centering}}],
                                "sides": parsed}),
                });
            }
            let fin = card.final_grade();
            let condition = condition_of(scale, fin);
            Ok(LocalGrading {
                grade: Some(fin),
                condition: Some(condition.into()),
                raw: json!({"records": [{"_id": "card", "grades": {
                    "corners": card.corners, "edges": card.edges, "surface": card.surface,
                    "centering": card.centering, "final": fin, "condition": condition,
                }}], "sides": parsed}),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grammars_list_the_fields_and_labels() {
        let g = grade_grammar();
        for k in ["corners", "edges", "surface", "centering"] {
            assert!(g.contains(&format!("\\\"{k}\\\"")), "{g}");
        }
        assert!(g.find("corners").unwrap() < g.find("centering").unwrap());
        let c = condition_grammar(scale("ebay_tcg"));
        assert!(c.contains(r#""\"Lightly Played (Excellent)\"""#), "{c}");
        assert!(condition_prompt(scale("cardmarket")).contains("Light Played: obvious wear"));
    }

    #[test]
    fn maps_session_modes_to_scales() {
        assert_eq!(scale("tcgplayer").id, "tcgplayer");
        assert_eq!(scale("ebay_sport").id, "ebay_sport");
        assert_eq!(scale("ebay").id, "ebay_sport");
        assert_eq!(scale("anything").id, "tcgplayer");
    }

    #[test]
    fn grades_the_card_from_its_worse_side() {
        let front = r#"{"corners": 9, "edges": 8.5, "surface": 9, "centering": 8}"#.to_string();
        let back = r#"{"corners": 7, "edges": 9, "surface": 9.5, "centering": 4}"#.to_string();
        let g = parse(GradingMode::Grade, scale("tcgplayer"), &[front, back]).unwrap();
        let grades = &g.raw["records"][0]["grades"];
        assert_eq!(grades["corners"], 7.0);
        assert_eq!(grades["edges"], 8.5);
        assert_eq!(grades["centering"], 8.0, "the front's centering");
        // mean (7 + 8.5 + 9 + 8) / 4 = 8.125, capped at 7 + 1 = 8.
        assert_eq!(g.grade, Some(8.0));
        assert_eq!(g.condition.as_deref(), Some("Near Mint"));
    }

    #[test]
    fn final_grade_is_held_down_by_the_weakest() {
        let s = Subgrades {
            corners: 10.0,
            edges: 10.0,
            surface: 3.0,
            centering: 10.0,
        };
        assert_eq!(s.final_grade(), 4.0);
        let s = Subgrades {
            corners: 9.5,
            edges: 9.0,
            surface: 9.0,
            centering: 8.5,
        };
        assert_eq!(s.final_grade(), 9.0);
        assert_eq!(condition_of(scale("cardmarket"), 9.0), "Near Mint");
        assert_eq!(condition_of(scale("ebay_sport"), 5.0), "Very Good");
        assert_eq!(condition_of(scale("ebay_sport"), 1.0), "Poor");
    }

    #[test]
    fn condition_is_the_worse_side_on_the_scale() {
        let g = parse(
            GradingMode::Condition,
            scale("tcgplayer"),
            &[
                r#"{"condition": "Near Mint"}"#.into(),
                r#"{"condition": "Moderately Played"}"#.into(),
            ],
        )
        .unwrap();
        assert_eq!(g.condition.as_deref(), Some("Moderately Played"));
        assert_eq!(g.grade, None);
        assert_eq!(
            g.raw["records"][0]["_objects"][0]["Condition"][0]["mode"],
            "TCGplayer"
        );
    }

    #[test]
    fn centering_only_and_bad_answers() {
        let g = parse(
            GradingMode::Centering,
            scale("tcgplayer"),
            &[r#"{"corners": 9, "edges": 9, "surface": 9, "centering": 7.5}"#.into()],
        )
        .unwrap();
        assert_eq!(g.grade, Some(7.5));
        assert!(parse(GradingMode::Grade, scale("tcgplayer"), &["no".into()]).is_err());
        assert!(parse(
            GradingMode::Condition,
            scale("tcgplayer"),
            &[r#"{"condition": "Mint"}"#.into()]
        )
        .is_err());
        let s: Subgrades = serde_json::from_str(r#"{"corners": 14, "edges": 0}"#).unwrap();
        assert_eq!((s.clamped().corners, s.clamped().edges), (10.0, 1.0));
    }
}
