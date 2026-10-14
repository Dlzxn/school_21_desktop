//! Хранение: пароль и токены — в хранилище ОС (keyring), настройки и кэш — JSON в папке приложения.

use std::path::PathBuf;

use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::config::KEYRING_SERVICE;

#[derive(Clone)]
pub struct Credentials {
    pub login: String,
    pub password: String,
}

impl Credentials {
    /// Логин для Open API: без домена, если введён как почта.
    pub fn api_login(&self) -> String {
        self.login.split('@').next().unwrap_or_default().trim().to_lowercase()
    }
}

fn entry(name: &str) -> Option<keyring::Entry> {
    keyring::Entry::new(KEYRING_SERVICE, name).ok()
}

pub fn get_secret(name: &str) -> Option<String> {
    entry(name)?.get_password().ok()
}

pub fn set_secret(name: &str, value: Option<&str>) {
    if let Some(e) = entry(name) {
        match value {
            Some(v) => {
                let _ = e.set_password(v);
            }
            None => {
                let _ = e.delete_credential();
            }
        }
    }
}

pub fn load_credentials() -> Option<Credentials> {
    Some(Credentials { login: get_secret("login")?, password: get_secret("password")? })
}

pub fn save_credentials(c: &Credentials) {
    clear_credentials();
    set_secret("login", Some(&c.login));
    set_secret("password", Some(&c.password));
}

pub fn clear_credentials() {
    for name in ["login", "password", "refresh_school21", "refresh_s21-open-api"] {
        set_secret(name, None);
    }
}

/// Пользовательские настройки.
#[derive(Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub notify_platform: bool,
    pub notify_agenda: bool,
    pub notify_events: bool,
    /// Крестик окна прячет приложение в трей, а не закрывает.
    pub close_to_tray: bool,
    /// Закреплённые страницы платформы (меню «⋯»).
    pub bookmarks: Vec<Bookmark>,
    /// С какой страницы открывать приложение; None — главная.
    pub start_url: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Bookmark {
    pub title: String,
    pub url: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            notify_platform: true,
            notify_agenda: true,
            notify_events: false,
            close_to_tray: true,
            bookmarks: Vec::new(),
            start_url: None,
        }
    }
}

/// Служебное состояние между запусками: уже показанные уведомления/события.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SyncState {
    /// None — ещё не было первой синхронизации (старое не показываем, только запоминаем).
    pub seen_platform: Option<Vec<String>>,
    pub seen_events: Option<Vec<String>>,
}

pub fn read_json<T: DeserializeOwned + Default>(path: &PathBuf) -> T {
    std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

pub fn write_json<T: Serialize>(path: &PathBuf, value: &T) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(text) = serde_json::to_string_pretty(value) {
        let _ = std::fs::write(path, text);
    }
}

/// Ограничиваем список «увиденного»: старые id всё равно больше не придут.
pub fn merge_seen(old: Option<Vec<String>>, new: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut all = old.unwrap_or_default();
    for id in new {
        if !all.contains(&id) {
            all.push(id);
        }
    }
    let excess = all.len().saturating_sub(300);
    all.drain(..excess);
    all
}
