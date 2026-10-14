//! Сетевой слой: токены Keycloak, официальный Open API и внутренний GraphQL сайта (только чтение).

use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::{json, Value};
use tokio::sync::Mutex;

use crate::config::*;
use crate::store;

#[derive(Debug)]
pub enum ApiError {
    /// Автовход не настроен — нечем авторизоваться.
    NotConfigured,
    /// Сохранённый пароль не подошёл.
    InvalidCredentials,
    Http(u16),
    Network(String),
    Parse(String),
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::NotConfigured => write!(f, "not_configured"),
            ApiError::InvalidCredentials => write!(f, "invalid_credentials"),
            ApiError::Http(code) => write!(f, "http_{code}"),
            ApiError::Network(e) => write!(f, "network: {e}"),
            ApiError::Parse(e) => write!(f, "parse: {e}"),
        }
    }
}

impl From<reqwest::Error> for ApiError {
    fn from(e: reqwest::Error) -> Self {
        ApiError::Network(e.to_string())
    }
}

type Result<T> = std::result::Result<T, ApiError>;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Participant {
    pub login: String,
    pub class_name: Option<String>,
    pub parallel_name: Option<String>,
    pub level: i64,
    pub exp_value: i64,
    pub exp_to_next_level: i64,
    pub campus: Option<String>,
    pub status: Option<String>,
}

#[derive(Clone, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Workstation {
    pub cluster_name: String,
    pub row: String,
    pub number: i64,
}

#[derive(Clone, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Points {
    pub peer_review_points: i64,
    pub code_review_points: i64,
    pub coins: i64,
}

#[derive(Clone, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Coalition {
    pub name: String,
    pub rank: Option<i64>,
}

#[derive(Clone)]
pub struct CampusEvent {
    pub id: i64,
    pub name: String,
    pub location: Option<String>,
    /// RFC 3339, UTC.
    pub start: String,
}

#[derive(Clone)]
pub struct PlatformNotification {
    pub id: String,
    pub message: String,
    pub was_read: bool,
}

#[derive(Clone)]
pub struct AgendaItem {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
    pub start: String,
}

struct Token {
    value: String,
    expires_at: Instant,
}

pub struct Api {
    http: reqwest::Client,
    tokens: Mutex<HashMap<String, Token>>,
    school_id: Mutex<Option<String>>,
}

fn str_field(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_owned)
}

fn encode_path(segment: &str) -> String {
    url::form_urlencoded::byte_serialize(segment.trim().to_lowercase().as_bytes()).collect::<String>().replace('+', "%20")
}

impl Api {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .user_agent(format!("School21Desktop/{}", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(20))
            .build()
            .expect("http client");
        Self { http, tokens: Mutex::new(HashMap::new()), school_id: Mutex::new(None) }
    }

    /// После смены или удаления логина.
    pub async fn reset(&self) {
        self.tokens.lock().await.clear();
        *self.school_id.lock().await = None;
    }

    // ---------- Токены Keycloak ----------

    async fn token_request(&self, params: &[(&str, &str)]) -> Result<Value> {
        let resp = self.http.post(KEYCLOAK_TOKEN_URL).form(params).send().await?;
        let status = resp.status();
        let body = resp.text().await?;
        if !status.is_success() {
            if body.contains("invalid_grant") && params.iter().any(|(k, v)| *k == "grant_type" && *v == "password") {
                return Err(ApiError::InvalidCredentials);
            }
            if body.contains("invalid_scope") {
                return Err(ApiError::Parse("invalid_scope".into()));
            }
            return Err(ApiError::Http(status.as_u16()));
        }
        serde_json::from_str(&body).map_err(|e| ApiError::Parse(e.to_string()))
    }

    async fn password_grant(&self, client: &str, creds: &store::Credentials) -> Result<Value> {
        let base = [
            ("client_id", client),
            ("grant_type", "password"),
            ("username", creds.login.as_str()),
            ("password", creds.password.as_str()),
        ];
        // offline_access — долгоживущий refresh-токен: пароль нужен редко, меньше новых сессий на сервере.
        let mut with_scope = base.to_vec();
        with_scope.push(("scope", "openid offline_access"));
        match self.token_request(&with_scope).await {
            Err(ApiError::Parse(e)) if e == "invalid_scope" => self.token_request(&base).await,
            other => other,
        }
    }

    pub async fn access_token(&self, client: &str) -> Result<String> {
        let mut tokens = self.tokens.lock().await;
        if let Some(t) = tokens.get(client) {
            if t.expires_at > Instant::now() + Duration::from_secs(30) {
                return Ok(t.value.clone());
            }
        }
        let refresh_key = format!("refresh_{client}");
        let mut response = None;
        if let Some(refresh) = store::get_secret(&refresh_key) {
            response = self
                .token_request(&[("client_id", client), ("grant_type", "refresh_token"), ("refresh_token", &refresh)])
                .await
                .ok();
        }
        let response = match response {
            Some(r) => r,
            None => {
                let creds = store::load_credentials().ok_or(ApiError::NotConfigured)?;
                self.password_grant(client, &creds).await?
            }
        };
        let access = str_field(&response, "access_token").ok_or_else(|| ApiError::Parse("access_token".into()))?;
        let expires_in = response.get("expires_in").and_then(Value::as_u64).unwrap_or(60);
        if let Some(refresh) = str_field(&response, "refresh_token") {
            store::set_secret(&refresh_key, Some(&refresh));
        }
        tokens.insert(
            client.to_owned(),
            Token { value: access.clone(), expires_at: Instant::now() + Duration::from_secs(expires_in) },
        );
        Ok(access)
    }

    async fn invalidate(&self, client: &str) {
        self.tokens.lock().await.remove(client);
    }

    /// Проверка логина и пароля до сохранения. Токен не сохраняется.
    pub async fn check_credentials(&self, login: &str, password: &str) -> Result<()> {
        self.token_request(&[
            ("client_id", CLIENT_PLATFORM),
            ("grant_type", "password"),
            ("username", login),
            ("password", password),
        ])
        .await
        .map(|_| ())
    }

    // ---------- Open API ----------

    /// GET с одним повтором на 401 (токен мог истечь на сервере раньше срока).
    async fn open_get(&self, path_and_query: &str) -> Result<String> {
        for attempt in 0..2 {
            let token = self.access_token(CLIENT_OPEN_API).await?;
            let resp = self.http.get(format!("{OPEN_API_URL}{path_and_query}")).bearer_auth(token).send().await?;
            let status = resp.status();
            if status.as_u16() == 401 && attempt == 0 {
                self.invalidate(CLIENT_OPEN_API).await;
                continue;
            }
            if !status.is_success() {
                return Err(ApiError::Http(status.as_u16()));
            }
            return Ok(resp.text().await?);
        }
        Err(ApiError::Http(401))
    }

    fn parse(text: &str) -> Result<Value> {
        serde_json::from_str(text).map_err(|e| ApiError::Parse(e.to_string()))
    }

    pub async fn participant(&self, login: &str) -> Result<Participant> {
        let v = Self::parse(&self.open_get(&format!("/v1/participants/{}", encode_path(login))).await?)?;
        Ok(Participant {
            login: str_field(&v, "login").unwrap_or_else(|| login.to_owned()),
            class_name: str_field(&v, "className"),
            parallel_name: str_field(&v, "parallelName"),
            level: v.get("level").and_then(Value::as_i64).unwrap_or(0),
            exp_value: v.get("expValue").and_then(Value::as_i64).unwrap_or(0),
            exp_to_next_level: v.get("expToNextLevel").and_then(Value::as_i64).unwrap_or(0),
            campus: v.get("campus").and_then(|c| str_field(c, "shortName")),
            status: str_field(&v, "status"),
        })
    }

    /// None — пир не в кампусе. Вопреки документации (404) API отвечает 200 с пустым телом.
    pub async fn workstation(&self, login: &str) -> Result<Option<Workstation>> {
        match self.open_get(&format!("/v1/participants/{}/workstation", encode_path(login))).await {
            Ok(body) if body.trim().is_empty() => Ok(None),
            Ok(body) => {
                let v = Self::parse(&body)?;
                Ok(Some(Workstation {
                    cluster_name: str_field(&v, "clusterName").unwrap_or_default(),
                    row: str_field(&v, "row").unwrap_or_default(),
                    number: v.get("number").and_then(Value::as_i64).unwrap_or(0),
                }))
            }
            Err(ApiError::Http(404)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub async fn points(&self, login: &str) -> Result<Points> {
        let v = Self::parse(&self.open_get(&format!("/v1/participants/{}/points", encode_path(login))).await?)?;
        Ok(Points {
            peer_review_points: v.get("peerReviewPoints").and_then(Value::as_i64).unwrap_or(0),
            code_review_points: v.get("codeReviewPoints").and_then(Value::as_i64).unwrap_or(0),
            coins: v.get("coins").and_then(Value::as_i64).unwrap_or(0),
        })
    }

    /// Средний логтайм за неделю, часов.
    pub async fn logtime(&self, login: &str) -> Result<f64> {
        let text = self.open_get(&format!("/v1/participants/{}/logtime", encode_path(login))).await?;
        text.trim().parse::<f64>().map_err(|e| ApiError::Parse(e.to_string()))
    }

    pub async fn coalition(&self, login: &str) -> Result<Option<Coalition>> {
        match self.open_get(&format!("/v1/participants/{}/coalition", encode_path(login))).await {
            Ok(body) if body.trim().is_empty() => Ok(None),
            Ok(body) => {
                let v = Self::parse(&body)?;
                Ok(str_field(&v, "name").map(|name| Coalition { name, rank: v.get("rank").and_then(Value::as_i64) }))
            }
            Err(ApiError::Http(404)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// События кампуса пользователя в интервале [from, to] (RFC 3339).
    pub async fn events(&self, from: &str, to: &str) -> Result<Vec<CampusEvent>> {
        let q = |s: &str| url::form_urlencoded::byte_serialize(s.as_bytes()).collect::<String>();
        let v = Self::parse(&self.open_get(&format!("/v1/events?from={}&to={}&limit=100", q(from), q(to))).await?)?;
        Ok(v.get("events")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|e| {
                        Some(CampusEvent {
                            id: e.get("id")?.as_i64()?,
                            name: str_field(e, "name").unwrap_or_default(),
                            location: str_field(e, "location"),
                            start: str_field(e, "startDateTime")?,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    // ---------- Внутренний GraphQL сайта (только query) ----------

    async fn school_id(&self, token: &str) -> Result<String> {
        if let Some(id) = self.school_id.lock().await.clone() {
            return Ok(id);
        }
        let resp = self
            .http
            .get(format!("{PLATFORM_ORIGIN}/services/rest/v1/users/getCurrentUserSchoolRoles"))
            .header("Cookie", format!("tokenId={token}"))
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(ApiError::Http(resp.status().as_u16()));
        }
        let v: Value = Self::parse(&resp.text().await?)?;
        fn find(v: &Value) -> Option<String> {
            match v {
                Value::Object(map) => map
                    .get("schoolId")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                    .or_else(|| map.values().find_map(find)),
                Value::Array(items) => items.iter().find_map(find),
                _ => None,
            }
        }
        let id = find(&v).ok_or_else(|| ApiError::Parse("schoolId".into()))?;
        *self.school_id.lock().await = Some(id.clone());
        Ok(id)
    }

    async fn graphql(&self, operation: &str, query: &str, variables: Value) -> Result<Value> {
        let body = json!({ "operationName": operation, "variables": variables, "query": query });
        for attempt in 0..2 {
            let token = self.access_token(CLIENT_PLATFORM).await?;
            let school = self.school_id(&token).await?;
            let resp = self
                .http
                .post(format!("{PLATFORM_ORIGIN}/services/graphql?operation={operation}"))
                .header("Cookie", format!("tokenId={token}"))
                .header("schoolid", &school)
                .header("userrole", "STUDENT")
                .header("X-EDU-ORG-UNIT-ID", &school)
                .header("X-EDU-PRODUCT-ID", EDU_PRODUCT_ID)
                .json(&body)
                .send()
                .await?;
            let status = resp.status();
            if status.as_u16() == 401 && attempt == 0 {
                self.invalidate(CLIENT_PLATFORM).await;
                continue;
            }
            if !status.is_success() {
                return Err(ApiError::Http(status.as_u16()));
            }
            let v = Self::parse(&resp.text().await?)?;
            if let Some(err) = v.get("errors").and_then(|e| e.get(0)) {
                return Err(ApiError::Parse(str_field(err, "message").unwrap_or_else(|| "graphql error".into())));
            }
            return v.get("data").cloned().ok_or_else(|| ApiError::Parse("no data".into()));
        }
        Err(ApiError::Http(401))
    }

    pub async fn notifications(&self, limit: i64) -> Result<Vec<PlatformNotification>> {
        let data = self
            .graphql("getUserNotifications", QUERY_NOTIFICATIONS, json!({ "paging": { "offset": 0, "limit": limit } }))
            .await?;
        let list = data
            .pointer("/s21Notification/getS21Notifications/notifications")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        Ok(list
            .iter()
            .filter_map(|n| {
                Some(PlatformNotification {
                    id: n.get("id").map(|id| id.as_str().map(str::to_owned).unwrap_or_else(|| id.to_string()))?,
                    message: str_field(n, "message").unwrap_or_default(),
                    was_read: n.get("wasRead").and_then(Value::as_bool).unwrap_or(false),
                })
            })
            .collect())
    }

    pub async fn agenda(&self, from: &str, to: &str, limit: i64) -> Result<Vec<AgendaItem>> {
        let data = self.graphql("getAgendaEvents", QUERY_AGENDA, json!({ "from": from, "to": to, "limit": limit })).await?;
        let list = data.pointer("/calendarEventS21/getMyAgendaEvents").and_then(Value::as_array).cloned().unwrap_or_default();
        Ok(list
            .iter()
            .filter_map(|e| {
                let start = str_field(e, "start")?;
                let label = str_field(e, "label").unwrap_or_default();
                let id = e
                    .pointer("/agendaItemContext/entityId")
                    .and_then(|v| v.as_str().map(str::to_owned).or_else(|| Some(v.to_string())))
                    .unwrap_or_else(|| format!("{label}@{start}"));
                Some(AgendaItem { id, label, description: str_field(e, "description"), start })
            })
            .collect())
    }

    /// Последний релиз десктоп-версии на GitHub: (версия без «v», ссылка на страницу).
    pub async fn latest_release(&self) -> Result<(String, String)> {
        let resp = self
            .http
            .get(format!("https://api.github.com/repos/{GITHUB_REPO}/releases/latest"))
            .header("Accept", "application/vnd.github+json")
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(ApiError::Http(resp.status().as_u16()));
        }
        let v = Self::parse(&resp.text().await?)?;
        let tag = str_field(&v, "tag_name").ok_or_else(|| ApiError::Parse("tag_name".into()))?;
        let url = str_field(&v, "html_url").unwrap_or_else(|| format!("https://github.com/{GITHUB_REPO}/releases"));
        Ok((tag.trim_start_matches('v').to_owned(), url))
    }
}

/// Сравнение версий вида 1.2.3 по числам.
pub fn is_newer(candidate: &str, current: &str) -> bool {
    let parse = |s: &str| s.split('.').map(|p| p.parse::<u64>().unwrap_or(0)).collect::<Vec<_>>();
    let (a, b) = (parse(candidate), parse(current));
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (a.get(i).copied().unwrap_or(0), b.get(i).copied().unwrap_or(0));
        if x != y {
            return x > y;
        }
    }
    false
}
