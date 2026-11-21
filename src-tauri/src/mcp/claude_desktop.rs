//! Claude Desktop's MCP configuration (`claude_desktop_config.json`): Cardloader adds itself there, so the user
//! doesn't edit JSON. Claude Desktop reads it when it starts and then runs `<this program> mcp` (`bridge.rs`).

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{json, Map, Value};

use crate::error::{Error, Result};

/// The server's name in Claude Desktop's list.
const SERVER: &str = "cardloader";

/// Where Claude Desktop keeps it: `~/Library/Application Support/Claude` on macOS, `%APPDATA%\Claude` on Windows,
/// `~/.config/Claude` on Linux.
pub fn config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("Claude").join("claude_desktop_config.json"))
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeDesktop {
    /// Claude Desktop has its folder on this computer.
    pub installed: bool,
    /// Its configuration starts this program as the `cardloader` server.
    pub configured: bool,
    /// The configuration file.
    pub path: String,
}

pub fn status(path: &Path, command: &str) -> ClaudeDesktop {
    let configured = std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .is_some_and(|config| config["mcpServers"][SERVER] == entry(command));
    ClaudeDesktop {
        installed: path.parent().is_some_and(Path::is_dir),
        configured,
        path: path.to_string_lossy().into_owned(),
    }
}

/// Adds (or points again at this program) the `cardloader` server, keeping everything else in the file. A file
/// that isn't a JSON object is left alone: overwriting it would lose the user's other settings.
pub fn add(path: &Path, command: &str) -> Result<ClaudeDesktop> {
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
    Ok(status(path, command))
}

fn entry(command: &str) -> Value {
    json!({"command": command, "args": ["mcp"]})
}

#[cfg(test)]
mod tests {
    use super::*;

    const APP: &str = "/Applications/Cardloader.app/Contents/MacOS/cardloader";

    #[test]
    fn adds_cardloader_and_keeps_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Claude").join("claude_desktop_config.json");

        let err = add(&path, APP).unwrap_err();
        assert!(err.to_string().contains("isn't installed"), "{err}");
        assert!(!status(&path, APP).installed);

        // Installed, never configured: the file is made.
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let s = add(&path, APP).unwrap();
        assert!(s.installed && s.configured);
        let config: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(config["mcpServers"]["cardloader"]["args"], json!(["mcp"]));

        // The user's preferences and other servers stay; another program path is pointed at this one.
        std::fs::write(
            &path,
            r#"{"preferences": {"menuBar": true}, "mcpServers": {"other": {"command": "x"},
                "cardloader": {"command": "/old/cardloader", "args": ["mcp"]}}}"#,
        )
        .unwrap();
        assert!(!status(&path, APP).configured, "another program");
        assert!(add(&path, APP).unwrap().configured);
        let config: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(config["preferences"]["menuBar"], true);
        assert_eq!(config["mcpServers"]["other"]["command"], "x");
        assert_eq!(config["mcpServers"]["cardloader"]["command"], APP);
        assert!(!dir
            .path()
            .join("Claude")
            .join(".claude_desktop_config.json.cardloader")
            .exists());
    }

    #[test]
    fn never_overwrites_a_file_it_cant_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("claude_desktop_config.json");
        for broken in [r#"{"preferences": "#, "[1, 2]", r#"{"mcpServers": []}"#] {
            std::fs::write(&path, broken).unwrap();
            assert!(add(&path, APP).is_err(), "{broken}");
            assert_eq!(std::fs::read_to_string(&path).unwrap(), broken);
        }
    }
}
