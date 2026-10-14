//! Автовход: когда сайт выкидывает на форму Keycloak, заполняет логин/пароль и отправляет форму.
//! Защита от петли: если сразу после нашей отправки снова показана форма (неверный пароль, капча),
//! повторно не пробуем — показываем уведомление, дальше пользователь входит сам.

use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager, Url, WebviewWindow};
use tauri_plugin_notification::NotificationExt;

use crate::config::AUTH_HOST;
use crate::state::AppState;
use crate::store;

/// Форма снова показана в течение этого времени после отправки — вход не удался.
const RETRY_GUARD: Duration = Duration::from_secs(60);
/// Повторное событие загрузки той же страницы — не повторный показ формы.
const DUPLICATE_EVENT: Duration = Duration::from_secs(3);

#[derive(Default)]
pub struct Guard {
    last_attempt: Option<Instant>,
    last_url: Option<String>,
    failure_notified: bool,
    offer_notified: bool,
}

fn is_login_page(url: &Url) -> bool {
    let path = url.path();
    path.ends_with("/protocol/openid-connect/auth") || path.ends_with("/login-actions/authenticate")
}

pub fn on_page_finished(app: &AppHandle, webview: &WebviewWindow, url: &Url) {
    let state = app.state::<AppState>();
    let mut guard = state.autologin.lock().unwrap();

    if url.host_str() != Some(AUTH_HOST) {
        // Ушли со страницы входа — вход удался (или пользователь ушёл сам).
        guard.last_attempt = None;
        guard.failure_notified = false;
        return;
    }
    if !is_login_page(url) {
        return;
    }

    let Some(creds) = store::load_credentials() else {
        if !guard.offer_notified {
            guard.offer_notified = true;
            let _ = app
                .notification()
                .builder()
                .title("School 21: включите автовход")
                .body("Платформа завершает сессию пару раз в день. Трей → «Настройки» → «Автовход».")
                .show();
        }
        return;
    };

    let now = Instant::now();
    let url_text = url.to_string();
    if let Some(last) = guard.last_attempt {
        if guard.last_url.as_deref() == Some(url_text.as_str()) && now - last < DUPLICATE_EVENT {
            return;
        }
        if now - last < RETRY_GUARD {
            guard.last_attempt = None;
            if !guard.failure_notified {
                guard.failure_notified = true;
                let _ = app
                    .notification()
                    .builder()
                    .title("School 21: автовход не удался")
                    .body("Возможно, сменился пароль. Обновите его: трей → «Настройки» → «Автовход».")
                    .show();
            }
            return;
        }
    }
    guard.last_attempt = Some(now);
    guard.last_url = Some(url_text);
    drop(guard);
    let _ = webview.eval(fill_script(&creds));
}

/// Форма рендерится React'ом после загрузки страницы — ждём полей до ~8 с.
/// Значения ставим через нативный setter + событие input (иначе React их затрёт), отправляем
/// обычным form.submit(). Проверка hostname внутри скрипта: если страница сменилась, данные никуда не уйдут.
fn fill_script(creds: &store::Credentials) -> String {
    let q = |s: &str| serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into());
    format!(
        r#"(function (host, login, password) {{
  if (location.hostname !== host || window.__s21AutoLogin) return;
  window.__s21AutoLogin = true;
  var tries = 0;
  var timer = setInterval(function () {{
    var u = document.querySelector('input[name="username"]');
    var p = document.querySelector('input[name="password"]');
    if (!u || !p || !p.form) {{ if (++tries > 40) clearInterval(timer); return; }}
    clearInterval(timer);
    var setValue = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set;
    [[u, login], [p, password]].forEach(function (pair) {{
      setValue.call(pair[0], pair[1]);
      pair[0].dispatchEvent(new Event('input', {{ bubbles: true }}));
    }});
    setTimeout(function () {{ HTMLFormElement.prototype.submit.call(p.form); }}, 300);
  }}, 200);
}})({}, {}, {});"#,
        q(AUTH_HOST),
        q(&creds.login),
        q(&creds.password)
    )
}
