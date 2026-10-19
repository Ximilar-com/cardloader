//! Turning images into cards: one image per card, or front + back pairs.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Front,
    Back,
    Unknown,
}

/// One card's images.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unit {
    pub front: PathBuf,
    pub back: Option<PathBuf>,
}

/// Suffix pairs recognised in file names, e.g. `card12_front.jpg` + `card12_back.jpg`.
const SUFFIXES: &[(&str, &str)] = &[
    ("front", "back"),
    ("recto", "verso"),
    ("f", "b"),
    ("a", "b"),
];
const SEPARATORS: &[char] = &['_', '-', ' ', '.'];

/// Every (base, suffix family, side) reading of a file stem that ends with a known front/back suffix
/// (`card_b` is the back of both `card_f` and `card_a`).
fn split_side(stem: &str) -> Vec<(String, &'static str, Side)> {
    let lower = stem.to_lowercase();
    let mut out = Vec::new();
    for (front, back) in SUFFIXES {
        for (suffix, side) in [(*front, Side::Front), (*back, Side::Back)] {
            if let Some(base) = lower.strip_suffix(suffix) {
                if base.ends_with(SEPARATORS) && base.len() > 1 {
                    out.push((base.trim_end_matches(SEPARATORS).to_string(), *front, side));
                }
            }
        }
    }
    out
}

/// Pairs a folder listing (sorted). Files named `<base>_front` / `<base>_back` (or recto/verso, f/b, a/b)
/// are paired by name; if no file follows such a naming, consecutive files are paired instead.
pub fn pair_files(files: &[PathBuf]) -> Vec<Unit> {
    let stem = |p: &Path| {
        p.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string()
    };
    let mut fronts: HashMap<(String, &str), usize> = HashMap::new();
    let mut backs: HashMap<(String, &str), usize> = HashMap::new();
    for (i, f) in files.iter().enumerate() {
        for (base, family, side) in split_side(&stem(f)) {
            match side {
                Side::Front => fronts.insert((base, family), i),
                _ => backs.insert((base, family), i),
            };
        }
    }
    let named: HashMap<usize, usize> = fronts
        .iter()
        .filter_map(|(key, f)| backs.get(key).map(|b| (*f, *b)))
        .collect();

    if named.is_empty() {
        return files
            .chunks(2)
            .map(|c| Unit {
                front: c[0].clone(),
                back: c.get(1).cloned(),
            })
            .collect();
    }
    let paired_backs: std::collections::HashSet<usize> = named.values().copied().collect();
    files
        .iter()
        .enumerate()
        .filter(|(i, _)| !paired_backs.contains(i))
        .map(|(i, f)| Unit {
            front: f.clone(),
            back: named.get(&i).map(|b| files[*b].clone()),
        })
        .collect()
}

/// Pairs a stream of pages (scanner, watched folder) as they arrive.
#[derive(Debug, Default)]
pub struct StreamPairer {
    pending: Option<PathBuf>,
    /// Named files waiting for their other side, under each reading of their name.
    named: HashMap<(String, &'static str, bool), PathBuf>,
}

impl StreamPairer {
    /// Takes the next file of a watched folder: files named as fronts and backs (`card_F.jpg`, `card_B.jpg`)
    /// pair by name in whichever order they arrive; other files pair with the next one.
    pub fn push_file(&mut self, path: PathBuf) -> Option<Unit> {
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
        let readings = split_side(&stem);
        if readings.is_empty() {
            return self.push(path, Side::Unknown);
        }
        for (base, family, side) in &readings {
            let front = *side == Side::Front;
            if let Some(other) = self.named.remove(&(base.clone(), *family, !front)) {
                self.named.retain(|_, p| *p != other);
                let (front, back) = if front { (path, other) } else { (other, path) };
                return Some(Unit {
                    front,
                    back: Some(back),
                });
            }
        }
        for (base, family, side) in readings {
            self.named
                .insert((base, family, side == Side::Front), path.clone());
        }
        None
    }

    /// Named files still waiting for their other side, each as a card of its own.
    pub fn flush_named(&mut self) -> Vec<Unit> {
        let mut paths: Vec<PathBuf> = self.named.drain().map(|(_, p)| p).collect();
        paths.sort();
        paths.dedup();
        paths
            .into_iter()
            .map(|front| Unit { front, back: None })
            .collect()
    }

    /// Takes the next page; returns a finished card when this page completes one.
    pub fn push(&mut self, path: PathBuf, side: Side) -> Option<Unit> {
        match (self.pending.take(), side) {
            (Some(front), Side::Back | Side::Unknown) => Some(Unit {
                front,
                back: Some(path),
            }),
            // A new front while one waits: the waiting card had no back.
            (Some(front), Side::Front) => {
                self.pending = Some(path);
                Some(Unit { front, back: None })
            }
            (None, Side::Back) => Some(Unit {
                front: path,
                back: None,
            }),
            (None, _) => {
                self.pending = Some(path);
                None
            }
        }
    }

    pub fn flush(&mut self) -> Option<Unit> {
        self.pending.take().map(|front| Unit { front, back: None })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(names: &[&str]) -> Vec<PathBuf> {
        names.iter().map(PathBuf::from).collect()
    }

    #[test]
    fn pairs_by_name() {
        let units = pair_files(&p(&[
            "c1_back.jpg",
            "c1_front.jpg",
            "c2-F.png",
            "c2-B.png",
            "lonely.jpg",
            "c3_a.jpg",
            "c3_b.jpg",
        ]));
        assert_eq!(units.len(), 4);
        assert_eq!(
            units[3],
            Unit {
                front: "c3_a.jpg".into(),
                back: Some("c3_b.jpg".into())
            }
        );
        assert_eq!(
            units[0],
            Unit {
                front: "c1_front.jpg".into(),
                back: Some("c1_back.jpg".into())
            }
        );
        assert_eq!(
            units[1],
            Unit {
                front: "c2-F.png".into(),
                back: Some("c2-B.png".into())
            }
        );
        assert_eq!(
            units[2],
            Unit {
                front: "lonely.jpg".into(),
                back: None
            }
        );
    }

    #[test]
    fn pairs_consecutively_without_names() {
        let units = pair_files(&p(&["scan_0001.png", "scan_0002.png", "scan_0003.png"]));
        assert_eq!(units.len(), 2);
        assert_eq!(units[0].back.as_deref(), Some(Path::new("scan_0002.png")));
        assert_eq!(units[1].back, None);
    }

    #[test]
    fn suffix_needs_a_separator() {
        assert!(split_side("tab").is_empty());
        assert!(split_side("b").is_empty());
        assert_eq!(split_side("Card_Back")[0].2, Side::Back);
    }

    #[test]
    fn watched_files_pair_by_name_in_any_order() {
        let mut s = StreamPairer::default();
        assert_eq!(s.push_file("c1_B.webp".into()), None);
        assert_eq!(s.push_file("c2_F.jpg".into()), None);
        assert_eq!(
            s.push_file("c1_F.webp".into()),
            Some(Unit {
                front: "c1_F.webp".into(),
                back: Some("c1_B.webp".into())
            })
        );
        // unnamed files pair consecutively meanwhile
        assert_eq!(s.push_file("scan1.png".into()), None);
        assert_eq!(
            s.push_file("scan2.png".into()),
            Some(Unit {
                front: "scan1.png".into(),
                back: Some("scan2.png".into())
            })
        );
        assert_eq!(
            s.flush_named(),
            vec![Unit {
                front: "c2_F.jpg".into(),
                back: None
            }]
        );
        // `_b` is the back of `_a` too; the waiting back is used once.
        assert_eq!(s.push_file("c3_b.jpg".into()), None);
        assert_eq!(
            s.push_file("c3_a.jpg".into()),
            Some(Unit {
                front: "c3_a.jpg".into(),
                back: Some("c3_b.jpg".into())
            })
        );
        assert_eq!(s.push_file("c3_f.jpg".into()), None);
        assert_eq!(s.flush_named().len(), 1);
    }

    #[test]
    fn stream_pairs_pages() {
        let mut s = StreamPairer::default();
        assert_eq!(s.push("1".into(), Side::Unknown), None);
        assert_eq!(
            s.push("2".into(), Side::Unknown),
            Some(Unit {
                front: "1".into(),
                back: Some("2".into())
            })
        );
        assert_eq!(s.push("3".into(), Side::Front), None);
        assert_eq!(
            s.push("4".into(), Side::Front),
            Some(Unit {
                front: "3".into(),
                back: None
            })
        );
        assert_eq!(
            s.flush(),
            Some(Unit {
                front: "4".into(),
                back: None
            })
        );
        assert_eq!(s.flush(), None);
    }
}
