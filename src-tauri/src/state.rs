//! Общее состояние приложения.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;

use tokio::sync::Notify;

use crate::api::Api;
use crate::autologin::Guard;
use crate::store::{self, Settings, SyncState};

pub struct AppState {
    pub api: Api,
    pub settings: Mutex<Settings>,
    pub sync_state: Mutex<SyncState>,
    pub autologin: Mutex<Guard>,
    /// Напоминания, уже поставленные в этом запуске (ключ: id@start).
    pub scheduled: Mutex<HashSet<String>>,
    /// Разбудить фоновую синхронизацию раньше срока.
    pub wake: Notify,
    settings_path: PathBuf,
    sync_state_path: PathBuf,
}

impl AppState {
    pub fn new(config_dir: PathBuf) -> Self {
        let settings_path = config_dir.join("settings.json");
        let sync_state_path = config_dir.join("sync-state.json");
        Self {
            api: Api::new(),
            settings: Mutex::new(store::read_json(&settings_path)),
            sync_state: Mutex::new(store::read_json(&sync_state_path)),
            autologin: Mutex::new(Guard::default()),
            scheduled: Mutex::new(HashSet::new()),
            wake: Notify::new(),
            settings_path,
            sync_state_path,
        }
    }

    pub fn settings(&self) -> Settings {
        self.settings.lock().unwrap().clone()
    }

    pub fn save_settings(&self, settings: Settings) {
        store::write_json(&self.settings_path, &settings);
        *self.settings.lock().unwrap() = settings;
    }

    pub fn update_settings(&self, f: impl FnOnce(&mut Settings)) {
        let mut settings = self.settings();
        f(&mut settings);
        self.save_settings(settings);
    }

    pub fn update_sync_state(&self, f: impl FnOnce(&mut SyncState)) {
        let mut state = self.sync_state.lock().unwrap();
        f(&mut state);
        store::write_json(&self.sync_state_path, &*state);
    }

    /// После смены аккаунта: заново показать «первую синхронизацию».
    pub fn reset_account_data(&self) {
        self.update_sync_state(|s| *s = SyncState::default());
        self.scheduled.lock().unwrap().clear();
    }
}
