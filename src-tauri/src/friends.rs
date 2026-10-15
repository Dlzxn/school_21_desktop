//! «Мои друзья»: список логинов и где они сейчас в кампусе. Только чтение Open API.

use std::collections::HashMap;

use futures_util::stream::{self, StreamExt};
use serde::Serialize;

use crate::api::{Api, Workstation};

pub const MAX_FRIENDS: usize = 30;

/// Сколько запросов к API одновременно — чтобы не упереться в лимит.
const PARALLEL: usize = 3;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FriendInfo {
    pub login: String,
    pub workstation: Option<Workstation>,
    pub level: Option<i64>,
    /// Не удалось получить данные (сеть, лимит запросов).
    pub failed: bool,
}

pub fn normalize(login: &str) -> String {
    login.trim().split('@').next().unwrap_or_default().to_lowercase()
}

pub fn place_label(ws: &Workstation) -> String {
    format!("{} · {}{}", ws.cluster_name, ws.row.to_uppercase(), ws.number)
}

/// Место и уровень каждого друга (для окна «Мои друзья»).
pub async fn details(api: &Api, logins: &[String]) -> Vec<FriendInfo> {
    stream::iter(logins.iter().cloned())
        .map(|login| async move {
            match api.workstation(&login).await {
                Ok(workstation) => {
                    let level = api.participant(&login).await.ok().map(|p| p.level);
                    FriendInfo { login, workstation, level, failed: false }
                }
                Err(_) => FriendInfo { login, workstation: None, level: None, failed: true },
            }
        })
        .buffered(PARALLEL)
        .collect()
        .await
}

/// Только места — для фоновой проверки. В ответе лишь те, для кого запрос удался.
pub async fn places(api: &Api, logins: &[String]) -> HashMap<String, Option<Workstation>> {
    stream::iter(logins.iter().cloned())
        .map(|login| async move { api.workstation(&login).await.ok().map(|ws| (login, ws)) })
        .buffer_unordered(PARALLEL)
        .filter_map(|r| async move { r })
        .collect()
        .await
}

/// Кто пришёл в кампус с прошлой проверки (был вне кампуса, теперь за компьютером).
pub fn arrivals<'a>(
    before: &HashMap<String, String>,
    now: &'a HashMap<String, Option<Workstation>>,
) -> Vec<(&'a String, &'a Workstation)> {
    now.iter()
        .filter_map(|(login, ws)| {
            let ws = ws.as_ref()?;
            before.get(login).is_some_and(String::is_empty).then_some((login, ws))
        })
        .collect()
}
