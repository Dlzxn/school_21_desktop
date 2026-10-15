//! Фоновая синхронизация: уведомления «колокольчика», новые события кампуса,
//! напоминания о личном календаре. Каждая часть независима: ошибка одной не мешает остальным.

use std::time::Duration as StdDuration;

use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;
use time::format_description::well_known::Rfc3339;
use time::{Duration, OffsetDateTime, UtcOffset};

use crate::api::{AgendaItem, ApiError};
use crate::config::{REMIND_BEFORE_MINUTES, SYNC_INTERVAL_MINUTES};
use crate::friends;
use crate::state::AppState;
use crate::store;

pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            run_once(&app).await;
            let state = app.state::<AppState>();
            tokio::select! {
                _ = tokio::time::sleep(StdDuration::from_secs(SYNC_INTERVAL_MINUTES * 60)) => {}
                _ = state.wake.notified() => {}
            }
        }
    });
}

/// Запустить синхронизацию сейчас (после смены настроек, пароля, по кнопке «Обновить»).
pub fn wake(app: &AppHandle) {
    app.state::<AppState>().wake.notify_one();
}

fn rfc3339(t: OffsetDateTime) -> String {
    t.format(&Rfc3339).unwrap_or_default()
}

fn parse_time(s: &str) -> Option<OffsetDateTime> {
    OffsetDateTime::parse(s, &Rfc3339).ok()
}

fn local_hm(t: OffsetDateTime) -> String {
    let offset = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);
    let t = t.to_offset(offset);
    format!("{:02}:{:02}", t.hour(), t.minute())
}

fn notify(app: &AppHandle, title: &str, body: &str) {
    let _ = app.notification().builder().title(title).body(body).show();
}

/// Сообщения «колокольчика» приходят с HTML-разметкой.
fn strip_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_tag = false;
    for c in text.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&nbsp;", " ").replace("&quot;", "\"").replace("&amp;", "&").trim().to_owned()
}

pub async fn run_once(app: &AppHandle) {
    let state = app.state::<AppState>();
    if store::load_credentials().is_none() {
        return;
    }
    let settings = state.settings();
    let now = OffsetDateTime::now_utc();
    let (from, to) = (rfc3339(now), rfc3339(now + Duration::days(14)));

    if settings.notify_platform {
        let _ = check_platform(app, &state).await;
    }
    if settings.notify_events {
        let _ = check_events(app, &state, &from, &to).await;
    }
    if settings.notify_agenda {
        if let Ok(agenda) = state.api.agenda(&from, &rfc3339(now + Duration::days(1)), 30).await {
            schedule_reminders(app, &state, &agenda, now);
        }
    }
    if settings.notify_friends && !settings.friends.is_empty() {
        check_friends(app, &state, &settings.friends).await;
    }
}

/// «Друг пришёл в кампус». Первая проверка только запоминает, кто где.
async fn check_friends(app: &AppHandle, state: &AppState, friends: &[String]) {
    let now = friends::places(&state.api, friends).await;
    let before = state.sync_state.lock().unwrap().friend_places.clone();
    if let Some(before) = &before {
        for (login, ws) in friends::arrivals(before, &now).into_iter().take(5) {
            notify(
                app,
                &format!("{login} теперь в кампусе"),
                &format!("{} · ряд {}, место {}", ws.cluster_name, ws.row.to_uppercase(), ws.number),
            );
        }
    }
    // Для друзей, по которым запрос не удался, оставляем прежнее значение.
    let mut merged = before.unwrap_or_default();
    for (login, ws) in &now {
        merged.insert(login.clone(), ws.as_ref().map(friends::place_label).unwrap_or_default());
    }
    merged.retain(|login, _| friends.contains(login));
    state.update_sync_state(|s| s.friend_places = Some(merged));
}

async fn check_platform(app: &AppHandle, state: &AppState) -> Result<(), ApiError> {
    let list = state.api.notifications(20).await?;
    let seen = state.sync_state.lock().unwrap().seen_platform.clone();
    // Первая синхронизация: старые уведомления не вываливаем, только запоминаем.
    if let Some(seen) = &seen {
        for n in list.iter().filter(|n| !n.was_read && !seen.contains(&n.id)).take(5) {
            notify(app, "School 21", &strip_html(&n.message));
        }
    }
    state.update_sync_state(|s| s.seen_platform = Some(store::merge_seen(seen, list.into_iter().map(|n| n.id))));
    Ok(())
}

async fn check_events(app: &AppHandle, state: &AppState, from: &str, to: &str) -> Result<(), ApiError> {
    let events = state.api.events(from, to).await?;
    let seen = state.sync_state.lock().unwrap().seen_events.clone();
    if let Some(seen) = &seen {
        for e in events.iter().filter(|e| !seen.contains(&e.id.to_string())).take(5) {
            let when = parse_time(&e.start).map(|t| {
                let offset = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);
                let t = t.to_offset(offset);
                format!("{:02}.{:02} в {}", t.day(), u8::from(t.month()), local_hm(t))
            });
            let body = [when, e.location.clone()].into_iter().flatten().collect::<Vec<_>>().join(" · ");
            notify(app, &format!("Новое событие: {}", e.name), &body);
        }
    }
    state.update_sync_state(|s| s.seen_events = Some(store::merge_seen(seen, events.into_iter().map(|e| e.id.to_string()))));
    Ok(())
}

/// Напоминание за REMIND_BEFORE_MINUTES до начала каждого пункта календаря на ближайшие сутки.
fn schedule_reminders(app: &AppHandle, state: &AppState, agenda: &[AgendaItem], now: OffsetDateTime) {
    for item in agenda {
        let Some(start) = parse_time(&item.start) else { continue };
        if start <= now || start > now + Duration::days(1) {
            continue;
        }
        let key = format!("{}@{}", item.id, start.unix_timestamp());
        if !state.scheduled.lock().unwrap().insert(key) {
            continue;
        }
        let remind_at = start - Duration::minutes(REMIND_BEFORE_MINUTES);
        let delay = (remind_at - now).whole_seconds().max(0) as u64;
        let (app, title) = (app.clone(), item.label.clone());
        let body = [Some(format!("Начало в {}", local_hm(start))), item.description.clone()]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" · ");
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(StdDuration::from_secs(delay)).await;
            notify(&app, &title, &body);
        });
    }
}
