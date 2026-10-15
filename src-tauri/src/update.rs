//! Автообновление через плагин updater: latest.json и подписанные сборки лежат в релизе на GitHub.
//! При запуске (если включено) новая версия ставится сама; пока приложение работает — пункт в трее.

use std::sync::Mutex;
use std::time::Duration;

use tauri::menu::MenuItem;
use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_updater::UpdaterExt;

use crate::state::AppState;

/// Как часто проверять, пока приложение запущено.
const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

/// Пункт меню трея «Проверить обновления» / «Обновить до X».
pub struct TrayItem(pub Mutex<Option<MenuItem<Wry>>>);

fn notify(app: &AppHandle, title: &str, body: &str) {
    let _ = app.notification().builder().title(title).body(body).show();
}

fn set_tray_text(app: &AppHandle, text: &str) {
    if let Some(item) = app.state::<TrayItem>().0.lock().unwrap().as_ref() {
        let _ = item.set_text(text);
    }
}

/// Новая версия, если есть.
pub async fn check(app: &AppHandle) -> Result<Option<String>, String> {
    let update = app.updater().map_err(|e| e.to_string())?.check().await.map_err(|e| e.to_string())?;
    Ok(update.map(|u| u.version))
}

/// Скачать, поставить и перезапустить. Ok(false) — обновлений нет.
pub async fn install(app: &AppHandle) -> Result<bool, String> {
    let updater = app.updater().map_err(|e| e.to_string())?;
    let Some(update) = updater.check().await.map_err(|e| e.to_string())? else {
        return Ok(false);
    };
    set_tray_text(app, &format!("Скачиваю версию {}…", update.version));
    update.download_and_install(|_, _| {}, || {}).await.map_err(|e| e.to_string())?;
    app.restart();
}

/// Пункт трея: обновить, если есть что, иначе сообщить.
pub fn from_tray(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        match install(&app).await {
            Ok(false) => notify(&app, "School 21", "У вас последняя версия"),
            Ok(true) => {}
            Err(_) => {
                set_tray_text(&app, "Проверить обновления");
                notify(&app, "Не удалось обновить", "Скачайте новую версию со страницы релизов на GitHub.");
            }
        }
    });
}

/// Фоновая проверка: при запуске — поставить сразу (если включено), дальше — раз в 6 часов подсказать.
pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut first = true;
        let mut notified: Option<String> = None;
        loop {
            if let Ok(Some(version)) = check(&app).await {
                if first && app.state::<AppState>().settings().auto_update {
                    notify(&app, "Обновляю School 21", &format!("Ставлю версию {version}, приложение перезапустится."));
                    let _ = install(&app).await;
                }
                // Сюда попадаем, если автообновление выключено или не удалось.
                set_tray_text(&app, &format!("Обновить до {version}"));
                if notified.as_deref() != Some(version.as_str()) {
                    notify(&app, &format!("Доступна версия {version}"), "Трей → «Обновить до …».");
                    notified = Some(version);
                }
            }
            first = false;
            tokio::time::sleep(CHECK_INTERVAL).await;
        }
    });
}
