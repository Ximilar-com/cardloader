//! Where a session's images come from. Every source sends finished cards ([`Unit`]s) into a channel
//! until it runs out (folder), is stopped (watch folder) or the scanner's feeder is empty.

pub mod csv_file;
pub mod scanner;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use notify::{EventKind, RecursiveMode, Watcher};
use sha2::{Digest, Sha256};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::error::{Error, Result};
use crate::imaging;
use crate::model::Pairing;
use crate::pairing::{pair_files, StreamPairer, Unit};

/// A card's Cards Cloud key (`event_id`), from its front image's file name: the same file gets the same key
/// on any computer, so a session continued elsewhere can tell which files Cards Cloud already has. Only
/// the hash leaves the computer, not the name.
pub fn file_key(front: &Path) -> String {
    let name = front.file_name().unwrap_or_default().to_string_lossy();
    let digest = Sha256::digest(format!("cardloader-file:{name}").as_bytes());
    format!("{digest:x}")[..32].to_string()
}

/// The images of `dir` whose card (paired as a run would pair them) has one of `keys`: the files to skip.
pub fn files_with_keys(
    dir: &Path,
    pairing: Pairing,
    keys: &HashSet<String>,
) -> Result<HashSet<String>> {
    let files = list_images(dir)?;
    let units = match pairing {
        Pairing::Single => files
            .into_iter()
            .map(|front| Unit { front, back: None })
            .collect(),
        Pairing::FrontBack => pair_files(&files),
    };
    Ok(units
        .into_iter()
        .filter(|u| keys.contains(&file_key(&u.front)))
        .flat_map(|u| std::iter::once(u.front).chain(u.back))
        .map(|p| p.to_string_lossy().into_owned())
        .collect())
}

/// Supported images directly in `dir`, sorted by name.
pub fn list_images(dir: &Path) -> Result<Vec<PathBuf>> {
    if !dir.is_dir() {
        return Err(Error::msg(format!("folder not found: {}", dir.display())));
    }
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_file() && imaging::is_supported(p) && !is_hidden(p))
        .collect();
    files.sort_by_key(|p| natural_key(p));
    Ok(files)
}

fn is_hidden(p: &Path) -> bool {
    p.file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.starts_with('.'))
        .unwrap_or(true)
}

/// Sort key that orders `scan_2` before `scan_10`.
fn natural_key(p: &Path) -> Vec<(String, u64)> {
    let name = p
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_lowercase();
    let mut out = Vec::new();
    let mut text = String::new();
    let mut digits = String::new();
    for ch in name.chars() {
        if ch.is_ascii_digit() {
            digits.push(ch);
        } else {
            if !digits.is_empty() {
                out.push((
                    std::mem::take(&mut text),
                    digits.parse().unwrap_or(u64::MAX),
                ));
                digits.clear();
            }
            text.push(ch);
        }
    }
    out.push((text, digits.parse().unwrap_or(0)));
    out
}

/// A folder: every image not yet in the session, paired as the session says.
pub async fn folder(
    dir: PathBuf,
    pairing: Pairing,
    known: HashSet<String>,
    tx: mpsc::Sender<Unit>,
) -> Result<()> {
    for unit in folder_units(&dir, pairing, &known)? {
        if tx.send(unit).await.is_err() {
            break;
        }
    }
    Ok(())
}

/// The cards of `dir`'s images that aren't in the session (`known`).
fn folder_units(dir: &Path, pairing: Pairing, known: &HashSet<String>) -> Result<Vec<Unit>> {
    let files: Vec<PathBuf> = list_images(dir)?
        .into_iter()
        .filter(|p| !known.contains(&p.to_string_lossy().into_owned()))
        .collect();
    Ok(match pairing {
        Pairing::Single => files
            .into_iter()
            .map(|front| Unit { front, back: None })
            .collect(),
        Pairing::FrontBack => pair_files(&files),
    })
}

/// The rows of a CSV that have an image and aren't in the session yet (identified rows were imported
/// with the session, unless their images are to be analysed again).
pub async fn csv(path: PathBuf, known: HashSet<String>, tx: mpsc::Sender<Unit>) -> Result<()> {
    let file = tokio::task::spawn_blocking(move || csv_file::read(&path))
        .await
        .map_err(|e| Error::msg(e.to_string()))??;
    for row in file.rows {
        let Some(front) = row.front else { continue };
        if known.contains(&front.to_string_lossy().into_owned()) {
            continue;
        }
        if tx
            .send(Unit {
                front,
                back: row.back,
            })
            .await
            .is_err()
        {
            break;
        }
    }
    Ok(())
}

/// A folder another app scans into (e.g. PaperStream Capture): what is there now, then every new image
/// once its size has settled, until the session is stopped. A paused session keeps queueing new images,
/// so none is missed; it only stops analysing them.
pub async fn watch_folder(
    dir: PathBuf,
    pairing: Pairing,
    known: HashSet<String>,
    tx: mpsc::Sender<Unit>,
    cancel: CancellationToken,
) -> Result<()> {
    // A missing folder fails as in `list_images`, before the watcher would say it its own way.
    if !dir.is_dir() {
        return Err(Error::msg(format!("folder not found: {}", dir.display())));
    }
    let (ev_tx, mut ev_rx) = mpsc::unbounded_channel::<PathBuf>();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res {
            if matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_)) {
                for path in event.paths {
                    let _ = ev_tx.send(path);
                }
            }
        }
    })
    .map_err(|e| Error::msg(format!("cannot watch {}: {e}", dir.display())))?;
    watcher
        .watch(&dir, RecursiveMode::NonRecursive)
        .map_err(|e| Error::msg(format!("cannot watch {}: {e}", dir.display())))?;

    // Listed once the watch runs: a file that lands while the backlog is sent waits among the events, and one in
    // both the listing and the events is taken once (`seen`).
    let backlog = folder_units(&dir, pairing, &known)?;
    let mut seen: HashSet<PathBuf> = backlog
        .iter()
        .flat_map(|u| std::iter::once(u.front.clone()).chain(u.back.clone()))
        .chain(known.iter().map(PathBuf::from))
        .collect();
    for unit in backlog {
        if tx.send(unit).await.is_err() {
            break;
        }
    }

    let mut pairer = StreamPairer::default();
    loop {
        let path = tokio::select! {
            p = ev_rx.recv() => match p { Some(p) => p, None => break },
            _ = cancel.cancelled() => break,
        };
        if seen.contains(&path) || !imaging::is_supported(&path) || is_hidden(&path) {
            continue;
        }
        if !wait_until_stable(&path, &cancel).await {
            continue;
        }
        seen.insert(path.clone());
        let unit = match pairing {
            Pairing::Single => Some(Unit {
                front: path,
                back: None,
            }),
            Pairing::FrontBack => pairer.push_file(path),
        };
        if let Some(unit) = unit {
            if tx.send(unit).await.is_err() {
                break;
            }
        }
    }
    for unit in pairer.flush_named().into_iter().chain(pairer.flush()) {
        if tx.send(unit).await.is_err() {
            break;
        }
    }
    Ok(())
}

/// A file being written keeps growing; wait until two size readings agree.
async fn wait_until_stable(path: &Path, cancel: &CancellationToken) -> bool {
    let mut last = None;
    for _ in 0..40 {
        let size = std::fs::metadata(path).map(|m| m.len()).ok();
        if size.is_none() {
            return false;
        }
        if size == last && size > Some(0) {
            return true;
        }
        last = size;
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_millis(400)) => {},
            _ = cancel.cancelled() => return false,
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn natural_order() {
        let mut files = [
            PathBuf::from("scan_10.png"),
            PathBuf::from("scan_2.png"),
            PathBuf::from("Scan_1.png"),
        ];
        files.sort_by_key(|p| natural_key(p));
        assert_eq!(files[0], PathBuf::from("Scan_1.png"));
        assert_eq!(files[2], PathBuf::from("scan_10.png"));
    }

    #[tokio::test]
    async fn folder_skips_known_and_pairs() {
        let dir = tempfile::tempdir().unwrap();
        for name in [
            "a_front.jpg",
            "a_back.jpg",
            "b.jpg",
            "notes.txt",
            ".hidden.jpg",
        ] {
            std::fs::write(dir.path().join(name), b"x").unwrap();
        }
        let known: HashSet<String> =
            [dir.path().join("b.jpg").to_string_lossy().into_owned()].into();
        let (tx, mut rx) = mpsc::channel(10);
        folder(dir.path().to_path_buf(), Pairing::FrontBack, known, tx)
            .await
            .unwrap();
        let unit = rx.recv().await.unwrap();
        assert!(unit.front.ends_with("a_front.jpg"));
        assert!(unit.back.unwrap().ends_with("a_back.jpg"));
        assert!(rx.recv().await.is_none());
    }

    /// A card whose files land while the folder's backlog is still going in arrives once, paired.
    #[tokio::test(flavor = "multi_thread")]
    async fn watch_folder_takes_files_that_land_during_the_backlog() {
        let tmp = tempfile::tempdir().unwrap();
        // Watch events name the real path (macOS: /private/var/…), as the listing then does too.
        let dir = tmp.path().canonicalize().unwrap();
        for i in 0..6 {
            for side in ["front", "back"] {
                std::fs::write(dir.join(format!("c{i}_{side}.jpg")), b"x").unwrap();
            }
        }
        let (tx, mut rx) = mpsc::channel(1);
        let cancel = CancellationToken::new();
        let source = tokio::spawn(watch_folder(
            dir.clone(),
            Pairing::FrontBack,
            HashSet::new(),
            tx,
            cancel.clone(),
        ));
        // A slow consumer: the backlog is still being sent when the new card lands.
        let mut units = vec![rx.recv().await.unwrap()];
        std::fs::write(dir.join("new_front.jpg"), b"front").unwrap();
        std::fs::write(dir.join("new_back.jpg"), b"back").unwrap();
        let started = std::time::Instant::now();
        while !units.iter().any(|u| u.front.ends_with("new_front.jpg")) {
            assert!(
                started.elapsed() < Duration::from_secs(15),
                "the new card never came: {units:?}"
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
            if let Ok(Some(unit)) =
                tokio::time::timeout(Duration::from_millis(200), rx.recv()).await
            {
                units.push(unit);
            }
        }
        // A repeat would follow within a moment.
        tokio::time::sleep(Duration::from_secs(2)).await;
        cancel.cancel();
        while let Some(unit) = rx.recv().await {
            units.push(unit);
        }
        source.await.unwrap().unwrap();

        assert_eq!(units.len(), 7, "{units:?}");
        let new: Vec<&Unit> = units
            .iter()
            .filter(|u| u.front.ends_with("new_front.jpg"))
            .collect();
        assert_eq!(new.len(), 1, "{units:?}");
        assert!(new[0].back.as_ref().unwrap().ends_with("new_back.jpg"));
        for i in 0..6 {
            let front = format!("c{i}_front.jpg");
            assert_eq!(
                units.iter().filter(|u| u.front.ends_with(&front)).count(),
                1,
                "{units:?}"
            );
        }
    }

    #[test]
    fn files_cards_cloud_has_are_found_by_name() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["a_F.jpg", "a_B.jpg", "b_F.jpg", "b_B.jpg"] {
            std::fs::write(dir.path().join(name), b"x").unwrap();
        }
        // The key depends on the name only, not the folder.
        assert_eq!(
            file_key(&dir.path().join("a_F.jpg")),
            file_key(Path::new("/elsewhere/a_F.jpg"))
        );
        assert_ne!(
            file_key(Path::new("a_F.jpg")),
            file_key(Path::new("b_F.jpg"))
        );
        assert_eq!(file_key(Path::new("a_F.jpg")).len(), 32);

        let keys: HashSet<String> = [file_key(Path::new("a_F.jpg"))].into();
        let mut skip: Vec<String> = files_with_keys(dir.path(), Pairing::FrontBack, &keys)
            .unwrap()
            .into_iter()
            .collect();
        skip.sort();
        assert_eq!(skip.len(), 2, "the front and its back: {skip:?}");
        assert!(skip[0].ends_with("a_B.jpg") && skip[1].ends_with("a_F.jpg"));
        let single = files_with_keys(dir.path(), Pairing::Single, &keys).unwrap();
        assert_eq!(single.len(), 1);
    }
}
