//! Tauri-приложение «Printer Config»: точка входа и регистрация команд.

mod commands;
mod config;
mod discovery;
mod sys;
mod winspool;

use discovery::Discovery;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();

    // Self-test без окна: печатает JSON списка принтеров и завершается.
    if std::env::args().any(|a| a == "--self-test") {
        let result = winspool::list_printers()
            .map(|list| serde_json::to_string_pretty(&list).unwrap_or_default())
            .unwrap_or_else(|e| format!("{{\"error\": {e:?}}}"));
        println!("{result}");
        return;
    }

    tauri::Builder::default()
        .manage(Discovery::new())
        .invoke_handler(tauri::generate_handler![
            commands::list_printers,
            commands::set_default_printer,
            commands::set_printer_paused,
            commands::set_printer_shared,
            commands::delete_printer,
            commands::add_printer_connection,
            commands::list_jobs,
            commands::job_action,
            commands::open_system,
            commands::open_url,
            commands::start_discovery,
            commands::stop_discovery,
            commands::discovery_running,
            commands::load_config,
            commands::save_config,
        ])
        .setup(|app| {
            // Фоновый перенос событий mDNS в окно.
            let handle = app.handle().clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(std::time::Duration::from_millis(500));
                let discovery = handle.state::<Discovery>();
                for event in discovery.poll() {
                    match &event {
                        discovery::DiscoveryEvent::Found(p) => {
                            use tauri::Emitter;
                            let _ = handle.emit("discovery://found", p);
                        }
                        discovery::DiscoveryEvent::Removed { fullname } => {
                            use tauri::Emitter;
                            let _ = handle.emit("discovery://removed", fullname);
                        }
                    }
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("ошибка при запуске Tauri-приложения");
}
