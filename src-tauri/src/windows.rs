//! Окна приложения и иконка в трее.

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::webview::PageLoadEvent;
use tauri::window::Color;
use tauri::{AppHandle, Manager, Url, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_opener::OpenerExt;

use crate::autologin;
use crate::config::{is_internal_host, START_URL};
use crate::state::AppState;
use crate::store::Bookmark;

/// Фон окон — как фон сайта, чтобы при загрузке не было белой вспышки.
const BACKGROUND: Color = Color(29, 38, 51, 255);

/// Ссылки на платформу остаются в окне, остальное — во внешнем браузере.
fn is_allowed(url: &Url) -> bool {
    match url.scheme() {
        "https" => url.host_str().is_some_and(is_internal_host),
        "about" | "data" | "blob" => true,
        _ => false,
    }
}

/// Кнопка «⋯» поверх сайта: меню приложения прямо в окне (значок в трее Windows часто прячет).
const MENU_BUTTON_SCRIPT: &str = include_str!("menu_button.js");
/// Пункты этого меню переходят по school21app://<действие> — сайту не нужен доступ к IPC.
const APP_SCHEME: &str = "school21app";

/// Сколько закладок можно сохранить.
const MAX_BOOKMARKS: usize = 12;

fn handle_app_link(app: &AppHandle, link: &Url) {
    let app = app.clone();
    let action = format!("{}{}", link.host_str().unwrap_or_default(), link.path().trim_end_matches('/'));
    let param = |name: &str| link.query_pairs().find(|(k, _)| k == name).map(|(_, v)| v.into_owned());
    let (url, title) = (param("url"), param("title"));
    // Окна создаём и настройки меняем вне колбэка навигации WebView.
    tauri::async_runtime::spawn(async move {
        match action.as_str() {
            "browser" => {
                if let Some(url) = app.get_webview_window("main").and_then(|w| w.url().ok()) {
                    open_external(&app, url.as_str());
                }
            }
            "bookmark/add" => {
                let (Some(url), Some(title)) = (url.filter(|u| is_platform_url(u)), title) else { return };
                app.state::<AppState>().update_settings(|s| {
                    if !s.bookmarks.iter().any(|b| b.url == url) && s.bookmarks.len() < MAX_BOOKMARKS {
                        let title = title.chars().take(80).collect();
                        s.bookmarks.push(Bookmark { title, url });
                    }
                });
                push_bookmarks(&app);
            }
            "bookmark/remove" => {
                let Some(url) = url else { return };
                app.state::<AppState>().update_settings(|s| {
                    s.bookmarks.retain(|b| b.url != url);
                    if s.start_url.as_deref() == Some(url.as_str()) {
                        s.start_url = None;
                    }
                });
                push_bookmarks(&app);
            }
            "bookmark/start" => {
                // Пустой url — снова открывать главную.
                let url = url.filter(|u| is_platform_url(u));
                app.state::<AppState>().update_settings(|s| s.start_url = url);
                push_bookmarks(&app);
            }
            label => open_local(&app, label),
        }
    });
}

/// Закладки можно ставить только на страницы платформы.
fn is_platform_url(url: &str) -> bool {
    url.parse::<Url>().is_ok_and(|u| u.scheme() == "https" && u.host_str().is_some_and(is_internal_host))
}

/// Передать в страницу список закладок для меню «⋯».
pub fn push_bookmarks(app: &AppHandle) {
    let Some(main) = app.get_webview_window("main") else { return };
    let settings = app.state::<AppState>().settings();
    let data = serde_json::json!({ "bookmarks": settings.bookmarks, "start": settings.start_url });
    let _ = main.eval(format!("window.__s21SetBookmarks && window.__s21SetBookmarks({data})"));
}

pub fn open_external(app: &AppHandle, url: &str) {
    let _ = app.opener().open_url(url, None::<&str>);
}

pub fn create_main(app: &AppHandle, visible: bool) -> tauri::Result<WebviewWindow> {
    let nav_app = app.clone();
    let popup_app = app.clone();
    // Стартовая страница из закладок (⌂ в меню «⋯»), иначе главная.
    let start = app
        .state::<AppState>()
        .settings()
        .start_url
        .filter(|u| is_platform_url(u))
        .and_then(|u| u.parse().ok())
        .unwrap_or_else(|| START_URL.parse().expect("start url"));
    let window = WebviewWindowBuilder::new(app, "main", WebviewUrl::External(start))
        .title("School 21")
        .inner_size(1280.0, 860.0)
        .min_inner_size(900.0, 600.0)
        .visible(visible)
        .background_color(BACKGROUND)
        .initialization_script(MENU_BUTTON_SCRIPT)
        .on_navigation(move |url| {
            if url.scheme() == APP_SCHEME {
                handle_app_link(&nav_app, url);
                return false;
            }
            if is_allowed(url) {
                return true;
            }
            open_external(&nav_app, url.as_str());
            false
        })
        // target=_blank / window.open: свои ссылки — в этом же окне, чужие — в браузер.
        .on_new_window(move |url, _features| {
            if is_allowed(&url) {
                if let Some(main) = popup_app.get_webview_window("main") {
                    let _ = main.navigate(url);
                }
            } else {
                open_external(&popup_app, url.as_str());
            }
            tauri::webview::NewWindowResponse::Deny
        })
        .on_page_load(|webview, payload| {
            if payload.event() == PageLoadEvent::Finished {
                push_bookmarks(webview.app_handle());
                autologin::on_page_finished(webview.app_handle(), &webview, payload.url());
            }
        })
        .build()?;

    let close_app = app.clone();
    let main = window.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { api, .. } = event {
            if close_app.state::<AppState>().settings().close_to_tray {
                api.prevent_close();
                let _ = main.hide();
            }
        }
    });
    Ok(window)
}

pub fn show_main(app: &AppHandle) {
    let window = match app.get_webview_window("main") {
        Some(w) => w,
        None => match create_main(app, true) {
            Ok(w) => w,
            Err(_) => return,
        },
    };
    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();
}

/// Открыть страницу платформы в главном окне.
pub fn open_platform(app: &AppHandle, path: &str) {
    show_main(app);
    if let (Some(main), Ok(url)) = (app.get_webview_window("main"), format!("https://platform.21-school.ru{path}").parse()) {
        let _ = main.navigate(url);
    }
}

/// Локальные окна: settings, peer. Повторное открытие — просто фокус.
pub fn open_local(app: &AppHandle, label: &str) {
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
        return;
    }
    let (file, title, width, height) = match label {
        "settings" => ("settings.html", "Настройки — School 21", 600.0, 820.0),
        "peer" => ("peer.html", "Где сидит пир — School 21", 560.0, 760.0),
        _ => return,
    };
    let _ = WebviewWindowBuilder::new(app, label, WebviewUrl::App(file.into()))
        .title(title)
        .inner_size(width, height)
        .background_color(BACKGROUND)
        .min_inner_size(440.0, 520.0)
        .build();
}

pub fn create_tray(app: &AppHandle) -> tauri::Result<()> {
    let menu = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, "open", "Открыть School 21", true, None::<&str>)?,
            &MenuItem::with_id(app, "peer", "Где сидит пир", true, None::<&str>)?,
            &MenuItem::with_id(app, "settings", "Настройки", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "quit", "Выход", true, None::<&str>)?,
        ],
    )?;
    let mut tray = TrayIconBuilder::with_id("tray")
        .tooltip("School 21")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => show_main(app),
            "quit" => app.exit(0),
            other => open_local(app, other),
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}
