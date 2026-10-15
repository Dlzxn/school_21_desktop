mod api;
mod autologin;
mod commands;
mod config;
mod friends;
mod state;
mod store;
mod sync;
mod update;
mod windows;

use tauri::{Manager, RunEvent};
use tauri_plugin_autostart::MacosLauncher;

use state::AppState;

/// Аргумент автозапуска: стартовать свёрнутым в трей.
const MINIMIZED_ARG: &str = "--minimized";

pub fn run() {
    let app = tauri::Builder::default()
        // Второй запуск — просто показать уже работающее окно.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| windows::show_main(app)))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec![MINIMIZED_ARG])))
        .invoke_handler(tauri::generate_handler![
            commands::get_info,
            commands::save_settings,
            commands::save_credentials,
            commands::clear_credentials,
            commands::peer_lookup,
            commands::own_login,
            commands::refresh_now,
            commands::set_autostart,
            commands::check_update,
            commands::install_update,
            commands::friends_list,
            commands::friends_add,
            commands::friends_remove,
            commands::open_window,
            commands::open_platform,
            commands::open_external,
            commands::repo_url,
        ])
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            app.manage(AppState::new(config_dir));
            app.manage(update::TrayItem(Default::default()));

            let minimized = std::env::args().any(|a| a == MINIMIZED_ARG);
            windows::create_main(app.handle(), !minimized)?;
            windows::create_tray(app.handle())?;
            sync::spawn(app.handle().clone());
            update::spawn(app.handle().clone());
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building School 21");

    app.run(|app, event| {
        // Окна спрятаны в трей — приложение продолжает работать, пока не выбрали «Выход».
        if let RunEvent::ExitRequested { api, code: None, .. } = event {
            if app.state::<AppState>().settings().close_to_tray {
                api.prevent_exit();
            }
        }
    });
}
