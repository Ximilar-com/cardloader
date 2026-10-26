//! App settings: a JSON file in the app config dir; the API token lives in the OS keychain
//! (falling back to the settings file where no keychain is available, e.g. a bare Linux box).

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::model::{Engine, GradingMode, IdMode, Pairing, ScanSettings, SourceKind};

pub const DEFAULT_API_BASE: &str = "https://api.ximilar.com";
const KEYRING_SERVICE: &str = "com.ximilar.cardloader";
const KEYRING_USER: &str = "ximilar-api-token";
/// macOS lets only the program that stored a keychain item read it without asking, and every dev build is a new
/// unsigned program: "Always Allow" would never stick. Dev builds on macOS keep the token in the settings file.
const USE_KEYCHAIN: bool = !(cfg!(debug_assertions) && cfg!(target_os = "macos"));

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub api_base_url: String,
    /// Workspace id sent as `?workspace=` to Cards Cloud; none = the token's default workspace.
    pub workspace: Option<String>,
    pub default_folder: Option<String>,
    pub default_scanner: Option<String>,
    pub scan: ScanSettings,
    pub default_id_mode: IdMode,
    pub default_grading: GradingMode,
    pub default_pairing: Pairing,
    /// The game (TCG) new sessions start with; empty = any.
    pub default_subcategory: String,
    /// How the user brings in cards, the main one first: what "New session" opens.
    pub default_sources: Vec<SourceKind>,
    /// The first-run setup was finished or skipped.
    pub setup_done: bool,
    pub price_stats: bool,
    pub concurrency: u32,
    pub sync_by_default: bool,
    /// What identifies cards in new sessions.
    pub engine: Engine,
    pub local_model: String,
    /// Only used when the OS keychain is unavailable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_token: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            api_base_url: DEFAULT_API_BASE.into(),
            workspace: None,
            default_folder: None,
            default_scanner: None,
            scan: ScanSettings::default(),
            default_id_mode: IdMode::Auto,
            default_grading: GradingMode::Off,
            default_pairing: Pairing::Single,
            default_subcategory: String::new(),
            default_sources: vec![SourceKind::Folder],
            setup_done: false,
            price_stats: true,
            concurrency: 4,
            sync_by_default: false,
            engine: Engine::Ximilar,
            local_model: crate::local::catalog::DEFAULT_MODEL.into(),
            fallback_token: None,
        }
    }
}

/// What the UI sees: the settings without the token, plus where the token is kept.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    #[serde(flatten)]
    pub settings: Settings,
    pub token_set: bool,
    pub token_storage: Option<String>,
}

pub struct SettingsStore {
    path: PathBuf,
    settings: Mutex<Settings>,
    /// The keychain token once read (`None` = not read yet). Each keychain read can make macOS ask for
    /// the login password, so it is read once per launch, not on every API call.
    keychain: Mutex<Option<Option<String>>>,
}

impl SettingsStore {
    pub fn load(path: PathBuf) -> Self {
        let settings = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        SettingsStore {
            path,
            settings: Mutex::new(settings),
            keychain: Mutex::new(None),
        }
    }

    #[cfg(test)]
    pub fn for_test(path: PathBuf, settings: Settings, token: &str) -> Self {
        let settings = Settings {
            fallback_token: Some(token.into()),
            ..settings
        };
        SettingsStore {
            path,
            settings: Mutex::new(settings),
            keychain: Mutex::new(None),
        }
    }

    pub fn get(&self) -> Settings {
        self.settings.lock().unwrap().clone()
    }

    pub fn view(&self) -> SettingsView {
        let mut settings = self.get();
        let storage = if self.keychain_token().is_some() {
            Some("keychain".to_string())
        } else if settings.fallback_token.is_some() {
            Some(if USE_KEYCHAIN { "file" } else { "dev" }.to_string())
        } else {
            None
        };
        settings.fallback_token = None;
        SettingsView {
            settings,
            token_set: storage.is_some(),
            token_storage: storage,
        }
    }

    /// Replaces everything but the token.
    pub fn update(&self, mut new: Settings) -> Result<()> {
        let mut guard = self.settings.lock().unwrap();
        new.fallback_token = guard.fallback_token.clone();
        new.concurrency = new.concurrency.clamp(1, 16);
        new.api_base_url = normalize_base(&new.api_base_url);
        new.workspace = new
            .workspace
            .filter(|w| !w.trim().is_empty())
            .map(|w| w.trim().to_string());
        *guard = new;
        save(&self.path, &guard)
    }

    pub fn token(&self) -> Option<String> {
        if cfg!(test) {
            return self.get().fallback_token;
        }
        self.keychain_token().or_else(|| self.get().fallback_token)
    }

    fn keychain_token(&self) -> Option<String> {
        if !USE_KEYCHAIN {
            return None;
        }
        self.keychain
            .lock()
            .unwrap()
            .get_or_insert_with(read_keychain)
            .clone()
    }

    /// Stores the token in the keychain, or in the settings file when there is none. Empty clears it.
    pub fn set_token(&self, token: &str) -> Result<String> {
        let token = token.trim();
        let mut guard = self.settings.lock().unwrap();
        if token.is_empty() {
            if USE_KEYCHAIN {
                if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER) {
                    let _ = entry.delete_credential();
                }
            }
            guard.fallback_token = None;
            *self.keychain.lock().unwrap() = Some(None);
            save(&self.path, &guard)?;
            return Ok("cleared".into());
        }
        if !USE_KEYCHAIN {
            guard.fallback_token = Some(token.to_string());
            save(&self.path, &guard)?;
            return Ok("dev".into());
        }
        let stored = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
            .and_then(|entry| entry.set_password(token))
            .is_ok()
            && read_keychain().as_deref() == Some(token);
        *self.keychain.lock().unwrap() = Some(stored.then(|| token.to_string()));
        if stored {
            guard.fallback_token = None;
            save(&self.path, &guard)?;
            Ok("keychain".into())
        } else {
            tracing::warn!("OS keychain unavailable; storing the API token in the settings file");
            guard.fallback_token = Some(token.to_string());
            save(&self.path, &guard)?;
            Ok("file".into())
        }
    }
}

fn read_keychain() -> Option<String> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .ok()?
        .get_password()
        .ok()
        .filter(|t| !t.is_empty())
}

fn normalize_base(url: &str) -> String {
    let url = url.trim().trim_end_matches('/');
    if url.is_empty() {
        DEFAULT_API_BASE.into()
    } else {
        url.into()
    }
}

fn save(path: &Path, settings: &Settings) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(settings)?)?;
    std::fs::rename(tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_fields_take_defaults() {
        let settings: Settings = serde_json::from_str(r#"{"concurrency": 2}"#).unwrap();
        assert_eq!(settings.concurrency, 2);
        assert_eq!(settings.api_base_url, DEFAULT_API_BASE);
        assert_eq!(settings.scan.dpi, 600);
        // Settings from before the setup wizard: it is shown once.
        assert!(!settings.setup_done);
        assert_eq!(settings.default_sources, vec![SourceKind::Folder]);
        assert_eq!(settings.default_subcategory, "");
    }

    #[test]
    fn base_url_is_normalized() {
        assert_eq!(
            normalize_base(" https://example.com/ "),
            "https://example.com"
        );
        assert_eq!(normalize_base(""), DEFAULT_API_BASE);
    }
}
