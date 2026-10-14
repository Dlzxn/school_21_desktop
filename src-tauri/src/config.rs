//! Все адреса и константы приложения в одном месте.

pub const PLATFORM_ORIGIN: &str = "https://platform.21-school.ru";
pub const START_URL: &str = "https://platform.21-school.ru/";

/// Домен платформы: всё на нём (и поддоменах) открывается в окне приложения, остальное — в браузере.
pub const INTERNAL_DOMAIN: &str = "21-school.ru";
pub const AUTH_HOST: &str = "auth.21-school.ru";

pub const KEYCLOAK_TOKEN_URL: &str =
    "https://auth.21-school.ru/auth/realms/EduPowerKeycloak/protocol/openid-connect/token";
/// Клиент сайта — для внутреннего GraphQL и проверки пароля.
pub const CLIENT_PLATFORM: &str = "school21";
/// Клиент официального Open API.
pub const CLIENT_OPEN_API: &str = "s21-open-api";

pub const OPEN_API_URL: &str = "https://platform.21-school.ru/services/21-school/api";

/// Константа из фронтенда платформы (заголовок X-EDU-PRODUCT-ID).
pub const EDU_PRODUCT_ID: &str = "96098f4b-5708-4c42-a62c-6893419169b3";

pub const GITHUB_REPO: &str = "Dlzxn/school_21_desktop";

/// Как часто фоновая синхронизация обновляет данные и проверяет уведомления.
pub const SYNC_INTERVAL_MINUTES: u64 = 30;
/// За сколько минут до начала события из личного календаря напоминать.
pub const REMIND_BEFORE_MINUTES: i64 = 15;

/// Имя записи в хранилище паролей ОС.
pub const KEYRING_SERVICE: &str = "ru.school21.desktop";

pub fn is_internal_host(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    host == INTERNAL_DOMAIN || host.ends_with(&format!(".{INTERNAL_DOMAIN}"))
}

// Шлюз платформы пропускает только запросы, которые есть во фронтенде сайта (allowlist):
// изменённый текст (другие поля, другое имя операции) получает 400. Поэтому тексты — дословно с сайта.

pub const QUERY_NOTIFICATIONS: &str = "\n    query getUserNotifications($paging: PagingInput!) {\n  s21Notification {\n    getS21Notifications(paging: $paging) {\n      notifications {\n        id\n        relatedObjectType\n        relatedObjectId\n        message\n        time\n        wasRead\n        groupName\n      }\n      totalCount\n      groupNames\n    }\n  }\n}\n    ";

pub const QUERY_AGENDA: &str = "query getAgendaEvents($from: DateTime!, $to: DateTime!, $limit: Int!) {\n  calendarEventS21 {\n    getMyAgendaEvents(from: $from, to: $to, limit: $limit) {\n      agendaItemContext {\n        entityId\n        entityType\n        __typename\n      }\n      start\n      end\n      label\n      description\n      agendaEventType\n      additionalInfo {\n        key\n        value\n        __typename\n      }\n      __typename\n    }\n    __typename\n  }\n}\n";
