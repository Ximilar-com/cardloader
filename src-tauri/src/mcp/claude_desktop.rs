//! Claude Desktop's MCP configuration (`claude_desktop_config.json`): Cardloader adds itself there, so the user
//! doesn't edit JSON. Claude Desktop reads it when it starts and then runs `<this program> mcp` (`bridge.rs`).

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{json, Map, Value};

use crate::error::{Error, Result};

/// The server's name in Claude Desktop's list.
const SERVER: &str = "cardloader";

/// Its configuration file, in the folder it keeps its data in.
const FILE: &str = "claude_desktop_config.json";

/// Claude Desktop's Windows packages (their family names), which name its folders there.
const WINDOWS_PACKAGES: [&str; 2] = ["Claude_pzs8sxrjxfjjc", "AnthropicPBC.Claude_fnn82j28hfe8t"];

/// The folders Claude Desktop may keep its configuration in, the likeliest first:
/// `~/Library/Application Support/Claude` on macOS, `~/.config/Claude` on Linux, several on Windows.
pub fn candidates() -> Vec<PathBuf> {
    if cfg!(windows) {
        match (dirs::data_local_dir(), dirs::config_dir()) {
            (Some(local), Some(roaming)) => windows_candidates(&local, &roaming),
            _ => Vec::new(),
        }
    } else {
        dirs::config_dir()
            .map(|d| d.join("Claude"))
            .into_iter()
            .collect()
    }
}

/// On Windows Claude Desktop is an MSIX package, and where it keeps its data has moved with its versions (and
/// moves on an update): `%LOCALAPPDATA%\Claude-Data\<package>` now, the package's own
/// `LocalCache\Roaming\Claude` before that, `%APPDATA%\Claude` on a PC that had the old installer, and
/// `%LOCALAPPDATA%\Claude-3p` when it runs on another provider (that folder exists, empty, on every install, so
/// it comes last). Claude's own "Edit Config" opens `%APPDATA%\Claude` whichever it reads.
fn windows_candidates(local: &Path, roaming: &Path) -> Vec<PathBuf> {
    let data = local.join("Claude-Data");
    let mut inside: Vec<PathBuf> = std::fs::read_dir(&data)
        .map(|entries| {
            entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect()
        })
        .unwrap_or_default();
    inside.sort();
    // The folder is named for the package, in lower case on the PCs seen.
    let (packages, others): (Vec<_>, Vec<_>) = inside.into_iter().partition(|p| {
        p.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| WINDOWS_PACKAGES.iter().any(|w| w.eq_ignore_ascii_case(n)))
    });
    let mut dirs = packages;
    dirs.push(data);
    dirs.extend(WINDOWS_PACKAGES.iter().map(|package| {
        local
            .join("Packages")
            .join(package)
            .join("LocalCache")
            .join("Roaming")
            .join("Claude")
    }));
    dirs.push(roaming.join("Claude"));
    dirs.extend(others);
    dirs.push(local.join("Claude-3p"));
    dirs
}

/// The files to keep Cardloader in: the likeliest folder there is, and every other one that has a configuration
/// already (one made by hand through "Edit Config", or left where Claude kept its data before an update). Which
/// of them Claude reads can't be told from outside, and an entry in one it doesn't read does nothing.
fn targets(dirs: &[PathBuf]) -> Vec<PathBuf> {
    let likeliest = dirs.iter().position(|d| d.is_dir());
    dirs.iter()
        .enumerate()
        .filter(|(i, d)| Some(*i) == likeliest || d.join(FILE).is_file())
        .map(|(_, d)| d.join(FILE))
        .collect()
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeDesktop {
    /// Claude Desktop has its folder on this computer.
    pub installed: bool,
    /// Its configuration starts this program as the `cardloader` server.
    pub configured: bool,
    /// The configuration file: the likeliest, where there are several (or where it would be).
    pub path: String,
}

pub fn status(dirs: &[PathBuf], command: &str) -> ClaudeDesktop {
    let files = targets(dirs);
    let path = files
        .first()
        .cloned()
        .or_else(|| dirs.first().map(|d| d.join(FILE)));
    ClaudeDesktop {
        installed: !files.is_empty(),
        configured: !files.is_empty() && files.iter().all(|file| starts(file, command)),
        path: path
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default(),
    }
}

/// Whether this configuration starts `command` as the `cardloader` server.
fn starts(path: &Path, command: &str) -> bool {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .is_some_and(|config| config["mcpServers"][SERVER] == entry(command))
}

/// Adds (or points again at this program) the `cardloader` server in every configuration there is (`targets`).
/// One that can't be changed doesn't keep the others from it; the first such error is what is returned.
pub fn add(dirs: &[PathBuf], command: &str) -> Result<ClaudeDesktop> {
    let files = targets(dirs);
    if files.is_empty() {
        return Err(Error::msg(
            "Claude Desktop isn't installed on this computer",
        ));
    }
    let mut failed = None;
    for file in &files {
        if let Err(e) = add_to(file, command) {
            failed.get_or_insert(e);
        }
    }
    match failed {
        Some(e) => Err(e),
        None => Ok(status(dirs, command)),
    }
}

/// Adds the server to one file, keeping everything else in it. A file that isn't a JSON object is left alone:
/// overwriting it would lose the user's other settings.
fn add_to(path: &Path, command: &str) -> Result<()> {
    let dir = path
        .parent()
        .filter(|d| d.is_dir())
        .ok_or_else(|| Error::msg("Claude Desktop isn't installed on this computer"))?;
    let mut config = match std::fs::read_to_string(path) {
        Ok(text) if !text.trim().is_empty() => serde_json::from_str::<Value>(&text).map_err(|e| {
            Error::msg(format!(
                "Claude Desktop's configuration ({}) isn't valid JSON ({e}): fix it or remove it, then try again",
                path.display()
            ))
        })?,
        Ok(_) => json!({}),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => json!({}),
        Err(e) => return Err(e.into()),
    };
    let root = config.as_object_mut().ok_or_else(|| {
        Error::msg(format!(
            "Claude Desktop's configuration ({}) isn't a JSON object",
            path.display()
        ))
    })?;
    let servers = root
        .entry("mcpServers")
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or_else(|| {
            Error::msg("mcpServers in Claude Desktop's configuration isn't a JSON object")
        })?;
    servers.insert(SERVER.into(), entry(command));
    // Written beside it and moved over it: Claude never reads half a file.
    let tmp = dir.join(".claude_desktop_config.json.cardloader");
    std::fs::write(&tmp, serde_json::to_string_pretty(&config)? + "\n")?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

fn entry(command: &str) -> Value {
    json!({"command": command, "args": ["mcp"]})
}

#[cfg(test)]
mod tests {
    use super::*;

    const APP: &str = "/Applications/Cardloader.app/Contents/MacOS/cardloader";

    fn read(path: &Path) -> Value {
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    fn adds_cardloader_and_keeps_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        let dirs = [dir.path().join("Claude")];
        let path = dirs[0].join(FILE);

        let err = add(&dirs, APP).unwrap_err();
        assert!(err.to_string().contains("isn't installed"), "{err}");
        let s = status(&dirs, APP);
        assert!(!s.installed && !s.configured);
        assert_eq!(s.path, path.to_string_lossy(), "where it would be");

        // Installed, never configured: the file is made.
        std::fs::create_dir_all(&dirs[0]).unwrap();
        let s = add(&dirs, APP).unwrap();
        assert!(s.installed && s.configured);
        assert_eq!(
            read(&path)["mcpServers"]["cardloader"]["args"],
            json!(["mcp"])
        );

        // The user's preferences and other servers stay; another program path is pointed at this one.
        std::fs::write(
            &path,
            r#"{"preferences": {"menuBar": true}, "mcpServers": {"other": {"command": "x"},
                "cardloader": {"command": "/old/cardloader", "args": ["mcp"]}}}"#,
        )
        .unwrap();
        assert!(!status(&dirs, APP).configured, "another program");
        assert!(add(&dirs, APP).unwrap().configured);
        let config = read(&path);
        assert_eq!(config["preferences"]["menuBar"], true);
        assert_eq!(config["mcpServers"]["other"]["command"], "x");
        assert_eq!(config["mcpServers"]["cardloader"]["command"], APP);
        assert!(!dirs[0]
            .join(".claude_desktop_config.json.cardloader")
            .exists());
    }

    #[test]
    fn never_overwrites_a_file_it_cant_read() {
        let dir = tempfile::tempdir().unwrap();
        let dirs = [dir.path().to_path_buf()];
        let path = dir.path().join(FILE);
        for broken in [r#"{"preferences": "#, "[1, 2]", r#"{"mcpServers": []}"#] {
            std::fs::write(&path, broken).unwrap();
            assert!(add(&dirs, APP).is_err(), "{broken}");
            assert_eq!(std::fs::read_to_string(&path).unwrap(), broken);
        }
    }

    /// The folders of a Windows PC, made in a temporary folder: `%LOCALAPPDATA%` and `%APPDATA%`.
    struct Windows {
        _dir: tempfile::TempDir,
        local: PathBuf,
        roaming: PathBuf,
    }

    impl Windows {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let (local, roaming) = (dir.path().join("Local"), dir.path().join("Roaming"));
            std::fs::create_dir_all(&local).unwrap();
            std::fs::create_dir_all(&roaming).unwrap();
            Windows {
                _dir: dir,
                local,
                roaming,
            }
        }

        fn folder(&self, base: &Path, path: &str) -> PathBuf {
            // Joined part by part: Windows would keep the `/` in the path, and paths are compared as text.
            let dir = path
                .split('/')
                .fold(base.to_path_buf(), |dir, part| dir.join(part));
            std::fs::create_dir_all(&dir).unwrap();
            dir
        }

        fn dirs(&self) -> Vec<PathBuf> {
            windows_candidates(&self.local, &self.roaming)
        }
    }

    #[test]
    fn finds_claude_where_windows_keeps_it() {
        // Nothing of Claude's: not installed, and nothing is made.
        let pc = Windows::new();
        assert!(!status(&pc.dirs(), APP).installed);
        assert!(add(&pc.dirs(), APP).is_err());
        assert_eq!(std::fs::read_dir(&pc.local).unwrap().count(), 0);

        // The old installer: `%APPDATA%\Claude` alone.
        let roaming = pc.folder(&pc.roaming, "Claude");
        assert!(add(&pc.dirs(), APP).unwrap().configured);
        assert_eq!(
            read(&roaming.join(FILE))["mcpServers"]["cardloader"]["command"],
            APP
        );

        // An earlier MSIX build: the package's own folder, where nothing is written yet.
        let pc = Windows::new();
        let cache = pc.folder(
            &pc.local,
            "Packages/Claude_pzs8sxrjxfjjc/LocalCache/Roaming/Claude",
        );
        let s = add(&pc.dirs(), APP).unwrap();
        assert!(s.installed && s.configured);
        assert_eq!(s.path, cache.join(FILE).to_string_lossy());

        // A current build: the data moved to Claude-Data (the folder named for the package, in lower case), the
        // package's folder left behind without a configuration, Claude-3p there and empty.
        let pc = Windows::new();
        let data = pc.folder(&pc.local, "Claude-Data/claude_pzs8sxrjxfjjc");
        std::fs::write(data.join(FILE), r#"{"preferences": {"menuBar": true}}"#).unwrap();
        let cache = pc.folder(
            &pc.local,
            "Packages/Claude_pzs8sxrjxfjjc/LocalCache/Roaming/Claude",
        );
        let third = pc.folder(&pc.local, "Claude-3p");
        let s = status(&pc.dirs(), APP);
        assert!(s.installed && !s.configured);
        assert_eq!(s.path, data.join(FILE).to_string_lossy());
        assert!(add(&pc.dirs(), APP).unwrap().configured);
        assert_eq!(read(&data.join(FILE))["preferences"]["menuBar"], true);
        assert!(!cache.join(FILE).exists() && !third.join(FILE).exists());
    }

    #[test]
    fn a_configuration_made_by_hand_elsewhere_is_kept_up_too() {
        // "Edit Config" made `%APPDATA%\Claude\claude_desktop_config.json`, which this Claude doesn't read: both
        // get Cardloader, so it starts whichever Claude reads.
        let pc = Windows::new();
        let data = pc.folder(&pc.local, "Claude-Data/claude_pzs8sxrjxfjjc");
        let roaming = pc.folder(&pc.roaming, "Claude");
        std::fs::write(
            roaming.join(FILE),
            r#"{"mcpServers": {"other": {"command": "x"}, "cardloader": {"command": "C:\\old.exe", "args": ["mcp"]}}}"#,
        )
        .unwrap();
        let s = status(&pc.dirs(), APP);
        assert!(s.installed && !s.configured);
        assert!(add(&pc.dirs(), APP).unwrap().configured);
        assert_eq!(
            read(&data.join(FILE))["mcpServers"]["cardloader"]["command"],
            APP
        );
        let by_hand = read(&roaming.join(FILE));
        assert_eq!(by_hand["mcpServers"]["cardloader"]["command"], APP);
        assert_eq!(by_hand["mcpServers"]["other"]["command"], "x");

        // One that can't be read is said, and doesn't keep the other from being written.
        let pc = Windows::new();
        let data = pc.folder(&pc.local, "Claude-Data/claude_pzs8sxrjxfjjc");
        let roaming = pc.folder(&pc.roaming, "Claude");
        std::fs::write(roaming.join(FILE), "{").unwrap();
        let err = add(&pc.dirs(), APP).unwrap_err();
        assert!(err.to_string().contains("isn't valid JSON"), "{err}");
        assert_eq!(
            read(&data.join(FILE))["mcpServers"]["cardloader"]["command"],
            APP
        );
        assert!(!status(&pc.dirs(), APP).configured);

        // A package this program doesn't know by name, with its configuration: found by the file.
        let pc = Windows::new();
        let data = pc.folder(&pc.local, "Claude-Data/another_package");
        pc.folder(&pc.local, "Claude-Data/another_package/Cache");
        std::fs::write(data.join(FILE), "{}").unwrap();
        assert!(add(&pc.dirs(), APP).unwrap().configured);
        assert_eq!(
            read(&data.join(FILE))["mcpServers"]["cardloader"]["command"],
            APP
        );
    }
}
