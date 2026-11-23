//! Finding a card's corners with the on-device model, for the crop: one question per picture, answered as four
//! points. Cardloader cuts the card out at them (`crate::crop`); the model never sees or makes the cut.

use serde_json::Value;

use crate::crop::{Point, Quad};

/// The points are asked for in thousandths of the picture, whatever size the model is shown it at: vision models
/// resize and tile what they get, and a frame the prompt spells out doesn't depend on how.
const FRAME: f64 = 1000.0;

pub const CORNERS_PROMPT: &str = "Find the collectible card in this picture and give the position of its four \
corners, each as [x, y]. x goes from 0 at the picture's left edge to 1000 at its right edge, y from 0 at the \
picture's top edge to 1000 at its bottom edge. Give the corners of the card itself, not of the picture: \
top_left, top_right, bottom_right, bottom_left.";

/// The corners' names, in the order the answer gives them.
const CORNERS: [&str; 4] = ["top_left", "top_right", "bottom_right", "bottom_left"];

pub fn corners_grammar() -> String {
    let fields = CORNERS
        .iter()
        .map(|k| format!(r#""\"{k}\"" space ":" space point"#))
        .collect::<Vec<_>>()
        .join(r#" "," space "#);
    // No leading zeros: JSON has none.
    format!(
        r#"root ::= "{{" space {fields} space "}}"
point ::= "[" space n space "," space n space "]"
n ::= "1000" | [1-9] [0-9]? [0-9]? | "0"
space ::= | " " | "\n"{{1,2}} [ \t]{{0,20}}
"#
    )
}

/// What the model found in a card's pictures, front first.
#[derive(Debug, Clone)]
pub struct LocalCrop {
    /// Each picture's corners, or why it has none: one side's bad answer doesn't lose the other's.
    pub corners: Vec<Result<Quad, String>>,
    /// Who answered, what, and how long it took: kept in the card's crop record.
    pub raw: Value,
}

/// Reads an answer into the card's corners. Which point the model called which isn't trusted (a small model
/// mixes them up): the four are taken as the card lies in the picture.
pub fn parse(answer: &str) -> Result<Quad, String> {
    let start = answer.find('{').ok_or("the model gave no answer")?;
    let end = answer.rfind('}').ok_or("the model's answer was cut off")?;
    let v: Value = serde_json::from_str(&answer[start..=end])
        .map_err(|e| format!("the model's answer is not valid JSON: {e}"))?;
    let mut points = [[0.0; 2]; 4];
    for (point, name) in points.iter_mut().zip(CORNERS) {
        let read = |i: usize| v[name][i].as_f64().map(|n| n / FRAME);
        *point = match (read(0), read(1)) {
            (Some(x), Some(y)) => [x, y] as Point,
            _ => return Err(format!("the model gave no {name} corner")),
        };
    }
    Quad::in_picture_order(points)
        .map_err(|why| format!("the model's corners can't be a card: {why}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grammar_names_four_corners() {
        let g = corners_grammar();
        let at = |k: &str| {
            g.find(&format!("\\\"{k}\\\""))
                .unwrap_or_else(|| panic!("{k}: {g}"))
        };
        assert!(at("top_left") < at("top_right"));
        assert!(at("top_right") < at("bottom_right"));
        assert!(at("bottom_right") < at("bottom_left"));
        assert!(g.starts_with("root ::= \"{\" space"), "{g}");
        assert!(
            g.contains("point ::=") && g.contains("n ::= \"1000\""),
            "{g}"
        );
        assert!(CORNERS_PROMPT.contains("1000"));
    }

    #[test]
    fn parses_corners() {
        let want = [[0.1, 0.05], [0.9, 0.06], [0.91, 0.95], [0.09, 0.94]];
        let answer = r#"{"top_left": [100, 50], "top_right": [900, 60],
            "bottom_right": [910, 950], "bottom_left": [90, 940]}"#;
        assert_eq!(parse(answer).unwrap().points(), want);
        // The names mixed up, other fields beside them, text around the object: the same card.
        let mixed = r#"Here: {"name": "Pikachu", "top_left": [910, 950], "top_right": [90, 940],
            "bottom_right": [100, 50], "bottom_left": [900, 60]} done"#;
        assert_eq!(parse(mixed).unwrap().points(), want);

        assert!(parse("no card").unwrap_err().contains("no answer"));
        let missing = r#"{"top_left": [100, 50], "top_right": [900, 60]}"#;
        assert!(parse(missing).unwrap_err().contains("bottom_right"));
        let line = r#"{"top_left": [0, 0], "top_right": [500, 0], "bottom_right": [1000, 0],
            "bottom_left": [500, 1]}"#;
        assert!(parse(line).unwrap_err().contains("can't be a card"));
    }
}
