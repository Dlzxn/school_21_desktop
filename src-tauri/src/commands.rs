//! Команды для локальных окон (настройки, пир, друзья). Окну с сайтом платформы они недоступны.

use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_autostart::ManagerExt;

use crate::api::{self, ApiError, Coalition, Participant, Points, Workstation};
use crate::config::{AUTH_HOST, GITHUB_REPO};
use crate::state::AppState;
use crate::store::{self, Credentials, Settings};
use crate::sync;
use crate::{friends, update};
use crate::windows;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub login: Option<String>,
    pub settings: Settings,
    pub autostart: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeerInfo {
    pub participant: Participant,
    pub workstation: Option<Workstation>,
    pub points: Option<Points>,
    pub logtime: Option<f64>,
    pub coalition: Option<Coalition>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub url: String,
    /// Можно поставить из приложения (иначе — открыть страницу релиза).
    pub installable: bool,
}

fn err(e: ApiError) -> String {
    e.to_string()
}

#[tauri::command]
pub fn get_info(app: AppHandle, state: State<'_, AppState>) -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        login: store::load_credentials().map(|c| c.login),
        settings: state.settings(),
        autostart: app.autolaunch().is_enabled().unwrap_or(false),
    }
}

#[tauri::command]
pub fn save_settings(app: AppHandle, state: State<'_, AppState>, settings: Settings) {
    // Закладки и стартовую страницу меняет меню «⋯» — из окна настроек их не перезаписываем.
    let current = state.settings();
    // Список друзей меняет окно «Мои друзья».
    state.save_settings(Settings {
        bookmarks: current.bookmarks,
        start_url: current.start_url,
        friends: current.friends,
        ..settings
    });
    sync::wake(&app);
}

#[tauri::command]
pub async fn save_credentials(app: AppHandle, state: State<'_, AppState>, login: String, password: String) -> Result<(), String> {
    let login = login.trim().to_owned();
    state.api.check_credentials(&login, &password).await.map_err(err)?;
    store::save_credentials(&Credentials { login, password });
    state.api.reset().await;
    state.reset_account_data();
    // Если сейчас открыта форма входа — перезагрузить, чтобы автовход сработал сразу.
    if let Some(main) = app.get_webview_window("main") {
        if main.url().ok().and_then(|u| u.host_str().map(str::to_owned)).as_deref() == Some(AUTH_HOST) {
            let _ = main.reload();
        }
    }
    sync::wake(&app);
    Ok(())
}

#[tauri::command]
pub async fn clear_credentials(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    store::clear_credentials();
    state.api.reset().await;
    state.reset_account_data();
    sync::wake(&app);
    Ok(())
}

#[tauri::command]
pub async fn peer_lookup(state: State<'_, AppState>, login: String) -> Result<PeerInfo, String> {
    let login = login.trim().split('@').next().unwrap_or_default().to_lowercase();
    let api = &state.api;
    let participant = api.participant(&login).await.map_err(err)?;
    let workstation = api.workstation(&login).await.map_err(err)?;
    Ok(PeerInfo {
        participant,
        workstation,
        points: api.points(&login).await.ok(),
        logtime: api.logtime(&login).await.ok(),
        coalition: api.coalition(&login).await.ok().flatten(),
    })
}

#[tauri::command]
pub fn own_login() -> Option<String> {
    store::load_credentials().map(|c| c.api_login())
}

#[tauri::command]
pub fn refresh_now(app: AppHandle) {
    sync::wake(&app);
}

#[tauri::command]
pub fn set_autostart(app: AppHandle, enabled: bool) -> Result<(), String> {
    let launcher = app.autolaunch();
    if enabled { launcher.enable() } else { launcher.disable() }.map_err(|e| e.to_string())
}

/// Новая версия: через updater (latest.json), а если он недоступен — по API релизов GitHub.
#[tauri::command]
pub async fn check_update(app: AppHandle, state: State<'_, AppState>) -> Result<Option<UpdateInfo>, String> {
    if let Ok(found) = update::check(&app).await {
        return Ok(found.map(|version| UpdateInfo { version, url: String::new(), installable: true }));
    }
    let (version, url) = state.api.latest_release().await.map_err(err)?;
    Ok(api::is_newer(&version, env!("CARGO_PKG_VERSION")).then_some(UpdateInfo { version, url, installable: false }))
}

/// Скачать и поставить новую версию; приложение перезапустится.
#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<bool, String> {
    update::install(&app).await
}

#[tauri::command]
pub async fn friends_list(state: State<'_, AppState>) -> Result<Vec<friends::FriendInfo>, String> {
    let logins = state.settings().friends;
    Ok(friends::details(&state.api, &logins).await)
}

/// Добавить друга: логин проверяется по Open API. Возвращает логин в каноническом виде.
#[tauri::command]
pub async fn friends_add(app: AppHandle, state: State<'_, AppState>, login: String) -> Result<String, String> {
    let login = friends::normalize(&login);
    let current = state.settings().friends;
    if current.contains(&login) {
        return Err("already".into());
    }
    if current.len() >= friends::MAX_FRIENDS {
        return Err("limit".into());
    }
    let participant = state.api.participant(&login).await.map_err(err)?;
    let login = participant.login.to_lowercase();
    state.update_settings(|s| {
        if !s.friends.contains(&login) {
            s.friends.push(login.clone());
        }
    });
    // Новый состав — заново «первая проверка», чтобы не прислать лишнего.
    state.update_sync_state(|s| s.friend_places = None);
    sync::wake(&app);
    Ok(login)
}

#[tauri::command]
pub fn friends_remove(state: State<'_, AppState>, login: String) {
    state.update_settings(|s| s.friends.retain(|f| *f != login));
}

#[tauri::command]
pub fn open_window(app: AppHandle, label: String) {
    windows::open_local(&app, &label);
}

/// Открыть раздел платформы в главном окне (например, профиль пира).
#[tauri::command]
pub fn open_platform(app: AppHandle, path: String) {
    if path.starts_with('/') {
        windows::open_platform(&app, &path);
    }
}

/// Внешние ссылки — только https (GitHub, issues).
#[tauri::command]
pub fn open_external(app: AppHandle, url: String) {
    if url.starts_with("https://") {
        windows::open_external(&app, &url);
    }
}

#[tauri::command]
pub fn repo_url() -> String {
    format!("https://github.com/{GITHUB_REPO}")
}
