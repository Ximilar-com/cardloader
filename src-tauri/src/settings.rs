//! App settings: a JSON file in the app config dir; secrets (the API token, the eBay Cert ID and eBay
//! login) live in the OS keychain (falling back to the settings file where no keychain is available, e.g. a
//! bare Linux box).

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::model::{Engine, GradingMode, IdMode, Pairing, ScanSettings};

pub const DEFAULT_API_BASE: &str = "https://api.ximilar.com";
const KEYRING_SERVICE: &str = "com.ximilar.cardloader";
/// What is kept in the keychain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Secret {
    /// The Ximilar API token.
    ApiToken,
    /// The client secret of the user's eBay keyset.
    EbayCert,
    /// The eBay refresh token: the seller's connected account.
    EbayRefresh,
    /// The ImgBB API key: pictures for exports that link to them (Whatnot).
    ImgbbKey,
}

impl Secret {
    fn keyring_user(self) -> &'static str {
        match self {
            Secret::ApiToken => "ximilar-api-token",
            Secret::EbayCert => "ebay-cert-id",
            Secret::EbayRefresh => "ebay-refresh-token",
            Secret::ImgbbKey => "imgbb-api-key",
        }
    }
}

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
    pub price_stats: bool,
    pub concurrency: u32,
    pub sync_by_default: bool,
    /// What identifies cards in new sessions.
    pub engine: Engine,
    pub local_model: String,
    pub ebay: EbaySettings,
    /// Replaces ImgBB's API host (tests); empty = ImgBB.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub image_host_base: String,
    /// Only used when the OS keychain is unavailable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_token: Option<String>,
    /// The other secrets, by keyring name, when the OS keychain is unavailable.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub fallback_secrets: BTreeMap<String, String>,
}

/// The user's eBay developer keyset (the Cert ID is a secret) and what their listings use.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct EbaySettings {
    pub sandbox: bool,
    /// The keyset's App ID (client id).
    pub app_id: String,
    /// The keyset's RuName ("eBay Redirect URL name").
    pub ru_name: String,
    /// Replaces eBay's hosts (tests); empty = eBay.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub api_base: String,
    pub marketplace: String,
    pub fulfillment_policy_id: String,
    pub payment_policy_id: String,
    pub return_policy_id: String,
    /// The inventory location (`merchantLocationKey`) items ship from.
    pub location_key: String,
    /// The connected seller, set when connecting.
    pub username: Option<String>,
}

impl Default for EbaySettings {
    fn default() -> Self {
        EbaySettings {
            sandbox: false,
            app_id: String::new(),
            ru_name: String::new(),
            api_base: String::new(),
            marketplace: "EBAY_US".into(),
            fulfillment_policy_id: String::new(),
            payment_policy_id: String::new(),
            return_policy_id: String::new(),
            location_key: String::new(),
            username: None,
        }
    }
}

impl EbaySettings {
    /// The keyset or account the connection belongs to changed: the eBay login no longer applies.
    fn other_account(&self, other: &EbaySettings) -> bool {
        self.sandbox != other.sandbox
            || self.app_id != other.app_id
            || self.api_base != other.api_base
    }
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
            price_stats: true,
            concurrency: 4,
            sync_by_default: false,
            engine: Engine::Ximilar,
            local_model: crate::local::catalog::DEFAULT_MODEL.into(),
            ebay: EbaySettings::default(),
            image_host_base: String::new(),
            fallback_token: None,
            fallback_secrets: BTreeMap::new(),
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
    pub ebay_cert_set: bool,
    pub ebay_connected: bool,
    pub imgbb_key_set: bool,
}

pub struct SettingsStore {
    path: PathBuf,
    settings: Mutex<Settings>,
    /// Keychain secrets once read (absent = not read yet). Each keychain read can make macOS ask for the
    /// login password, so each is read once per launch, not on every API call.
    keychain: Mutex<HashMap<Secret, Option<String>>>,
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
            keychain: Mutex::new(HashMap::new()),
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
            keychain: Mutex::new(HashMap::new()),
        }
    }

    pub fn get(&self) -> Settings {
        self.settings.lock().unwrap().clone()
    }

    pub fn view(&self) -> SettingsView {
        let mut settings = self.get();
        let storage = if self.keychain_secret(Secret::ApiToken).is_some() {
            Some("keychain".to_string())
        } else if settings.fallback_token.is_some() {
            Some(if USE_KEYCHAIN { "file" } else { "dev" }.to_string())
        } else {
            None
        };
        settings.fallback_token = None;
        settings.fallback_secrets.clear();
        SettingsView {
            settings,
            token_set: storage.is_some(),
            token_storage: storage,
            ebay_cert_set: self.secret(Secret::EbayCert).is_some(),
            ebay_connected: self.secret(Secret::EbayRefresh).is_some(),
            imgbb_key_set: self.secret(Secret::ImgbbKey).is_some(),
        }
    }

    /// Replaces everything but the secrets and the eBay connection. A changed eBay keyset or environment
    /// disconnects eBay.
    pub fn update(&self, mut new: Settings) -> Result<()> {
        let mut guard = self.settings.lock().unwrap();
        new.fallback_token = guard.fallback_token.clone();
        new.fallback_secrets = guard.fallback_secrets.clone();
        new.concurrency = new.concurrency.clamp(1, 16);
        new.api_base_url = normalize_base(&new.api_base_url);
        new.workspace = new
            .workspace
            .filter(|w| !w.trim().is_empty())
            .map(|w| w.trim().to_string());
        new.ebay.app_id = new.ebay.app_id.trim().to_string();
        new.ebay.ru_name = new.ebay.ru_name.trim().to_string();
        new.ebay.api_base = new.ebay.api_base.trim().trim_end_matches('/').to_string();
        let disconnect = guard.ebay.other_account(&new.ebay);
        new.ebay.username = if disconnect {
            None
        } else {
            guard.ebay.username.clone()
        };
        if disconnect {
            new.ebay.fulfillment_policy_id.clear();
            new.ebay.payment_policy_id.clear();
            new.ebay.return_policy_id.clear();
            new.ebay.location_key.clear();
        }
        *guard = new;
        if disconnect {
            self.store_secret(&mut guard, Secret::EbayRefresh, "")?;
        }
        save(&self.path, &guard)
    }

    /// Changes only the eBay settings (the connection and listing setup), keeping everything else.
    pub fn update_ebay(&self, f: impl FnOnce(&mut EbaySettings)) -> Result<()> {
        let mut guard = self.settings.lock().unwrap();
        f(&mut guard.ebay);
        save(&self.path, &guard)
    }

    pub fn token(&self) -> Option<String> {
        self.secret(Secret::ApiToken)
    }

    pub fn secret(&self, secret: Secret) -> Option<String> {
        if cfg!(test) {
            return self.fallback(&self.get(), secret);
        }
        self.keychain_secret(secret)
            .or_else(|| self.fallback(&self.get(), secret))
    }

    fn fallback(&self, settings: &Settings, secret: Secret) -> Option<String> {
        match secret {
            Secret::ApiToken => settings.fallback_token.clone(),
            _ => settings
                .fallback_secrets
                .get(secret.keyring_user())
                .cloned(),
        }
    }

    fn keychain_secret(&self, secret: Secret) -> Option<String> {
        if !USE_KEYCHAIN || cfg!(test) {
            return None;
        }
        self.keychain
            .lock()
            .unwrap()
            .entry(secret)
            .or_insert_with(|| read_keychain(secret))
            .clone()
    }

    /// Stores the token in the keychain, or in the settings file when there is none. Empty clears it.
    pub fn set_token(&self, token: &str) -> Result<String> {
        self.set_secret(Secret::ApiToken, token)
    }

    /// Stores a secret in the keychain, or in the settings file when there is none; returns where. Empty
    /// clears it.
    pub fn set_secret(&self, secret: Secret, value: &str) -> Result<String> {
        let mut guard = self.settings.lock().unwrap();
        let storage = self.store_secret(&mut guard, secret, value)?;
        save(&self.path, &guard)?;
        Ok(storage)
    }

    fn store_secret(&self, settings: &mut Settings, secret: Secret, value: &str) -> Result<String> {
        let value = value.trim();
        let set_fallback = |settings: &mut Settings, value: Option<String>| match secret {
            Secret::ApiToken => settings.fallback_token = value,
            _ => {
                let key = secret.keyring_user().to_string();
                match value {
                    Some(v) => settings.fallback_secrets.insert(key, v),
                    None => settings.fallback_secrets.remove(&key),
                };
            }
        };
        let use_keychain = USE_KEYCHAIN && !cfg!(test);
        if value.is_empty() {
            if use_keychain {
                if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, secret.keyring_user()) {
                    let _ = entry.delete_credential();
                }
            }
            set_fallback(settings, None);
            self.keychain.lock().unwrap().insert(secret, None);
            return Ok("cleared".into());
        }
        if !use_keychain {
            set_fallback(settings, Some(value.to_string()));
            return Ok("dev".into());
        }
        let stored = keyring::Entry::new(KEYRING_SERVICE, secret.keyring_user())
            .and_then(|entry| entry.set_password(value))
            .is_ok()
            && read_keychain(secret).as_deref() == Some(value);
        self.keychain
            .lock()
            .unwrap()
            .insert(secret, stored.then(|| value.to_string()));
        if stored {
            set_fallback(settings, None);
            Ok("keychain".into())
        } else {
            tracing::warn!("OS keychain unavailable; storing a secret in the settings file");
            set_fallback(settings, Some(value.to_string()));
            Ok("file".into())
        }
    }
}

fn read_keychain(secret: Secret) -> Option<String> {
    keyring::Entry::new(KEYRING_SERVICE, secret.keyring_user())
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
    }

    #[test]
    fn base_url_is_normalized() {
        assert_eq!(
            normalize_base(" https://example.com/ "),
            "https://example.com"
        );
        assert_eq!(normalize_base(""), DEFAULT_API_BASE);
    }

    #[test]
    fn secrets_survive_saving_and_a_new_keyset_disconnects_ebay() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::for_test(dir.path().join("s.json"), Settings::default(), "tok");
        store.set_secret(Secret::EbayCert, " cert ").unwrap();
        store.set_secret(Secret::EbayRefresh, "refresh").unwrap();
        store
            .update_ebay(|e| e.username = Some("seller".into()))
            .unwrap();

        let mut edited = store.view().settings;
        assert!(
            edited.fallback_secrets.is_empty(),
            "the UI never sees secrets"
        );
        edited.ebay.location_key = "home".into();
        edited.ebay.username = None;
        store.update(edited.clone()).unwrap();
        assert_eq!(store.token().as_deref(), Some("tok"));
        assert_eq!(store.secret(Secret::EbayCert).as_deref(), Some("cert"));
        let view = store.view();
        assert!(view.ebay_cert_set && view.ebay_connected);
        assert_eq!(view.settings.ebay.username.as_deref(), Some("seller"));
        assert_eq!(view.settings.ebay.location_key, "home");

        edited.ebay.sandbox = true;
        store.update(edited).unwrap();
        let view = store.view();
        assert!(view.ebay_cert_set && !view.ebay_connected);
        assert_eq!(view.settings.ebay.username, None);
        assert_eq!(view.settings.ebay.location_key, "");

        let reloaded = SettingsStore::load(dir.path().join("s.json"));
        assert_eq!(reloaded.get().fallback_secrets.len(), 1);
    }
}
