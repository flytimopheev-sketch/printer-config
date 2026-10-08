//! Tauri-команды: мост между UI (TS) и winspool/discovery/config.

use tauri::State;

use crate::config::{self as config, AppConfig};
use crate::discovery::Discovery;
use crate::winspool::{self, JobInfo, PrinterInfo};

// ---------------------------------------------------------------------------
// Принтеры
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn list_printers() -> Result<Vec<PrinterInfo>, String> {
    winspool::list_printers()
}

#[tauri::command]
pub fn set_default_printer(name: String) -> Result<(), String> {
    winspool::set_default_printer(&name)
}

#[tauri::command]
pub fn set_printer_paused(name: String, paused: bool) -> Result<(), String> {
    winspool::set_printer_paused(&name, paused)
}

#[tauri::command]
pub fn set_printer_shared(name: String, shared: bool) -> Result<(), String> {
    winspool::set_printer_shared(&name, shared)
}

#[tauri::command]
pub fn delete_printer(name: String) -> Result<(), String> {
    winspool::delete_printer(&name)
}

#[tauri::command]
pub fn add_printer_connection(path: String) -> Result<(), String> {
    let path = path.trim().to_string();
    if path.is_empty() {
        return Err("Укажите путь к принтеру, например \\\\сервер\\имя".into());
    }
    if !path.starts_with("\\\\") {
        return Err("Путь должен начинаться с \\\\ (двойного обратного слэша)".into());
    }
    winspool::add_printer_connection(&path)
}

// ---------------------------------------------------------------------------
// Задания
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn list_jobs(printer: String) -> Result<Vec<JobInfo>, String> {
    winspool::list_jobs(&printer)
}

#[tauri::command]
pub fn job_action(printer: String, job_id: u32, action: String) -> Result<(), String> {
    winspool::job_action(&printer, job_id, &action)
}

// ---------------------------------------------------------------------------
// Системные диалоги Windows (printui.dll)
// ---------------------------------------------------------------------------

/// Виды системных окон:
/// `properties` | `queue` | `preferences` | `test-page` | `add-wizard` | `settings`.
#[tauri::command]
pub fn open_system(kind: String, name: Option<String>) -> Result<(), String> {
    use std::process::Command;

    let name = name.unwrap_or_default();
    let args: Vec<String> = match kind.as_str() {
        "properties" => vec!["printui.dll,PrintUIEntry".into(), "/p".into(), format!("/n{name}")],
        "queue" => vec!["printui.dll,PrintUIEntry".into(), "/o".into(), format!("/n{name}")],
        "preferences" => vec!["printui.dll,PrintUIEntry".into(), "/e".into(), format!("/n{name}")],
        "test-page" => vec!["printui.dll,PrintUIEntry".into(), "/k".into(), format!("/n{name}")],
        "add-wizard" => vec!["printui.dll,PrintUIEntry".into(), "/il".into()],
        "settings" => {
            Command::new("cmd")
                .args(["/c", "start", "", "ms-settings:printers"])
                .spawn()
                .map_err(|e| format!("Не удалось открыть параметры печати: {e}"))?;
            return Ok(());
        }
        other => return Err(format!("Неизвестный системный диалог: {other}")),
    };

    Command::new("rundll32")
        .args(&args)
        .spawn()
        .map_err(|e| format!("Не удалось запустить системный диалог: {e}"))?;
    Ok(())
}

/// Открывает URL (веб-интерфейс найденного принтера).
#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("Открывать можно только http/https адреса".into());
    }
    std::process::Command::new("cmd")
        .args(["/c", "start", "", &url])
        .spawn()
        .map_err(|e| format!("Не удалось открыть адрес: {e}"))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Обнаружение (mDNS)
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn start_discovery(discovery: State<'_, Discovery>) -> Result<(), String> {
    discovery.start()
}

#[tauri::command]
pub fn stop_discovery(discovery: State<'_, Discovery>) -> Result<(), String> {
    discovery.stop()
}

#[tauri::command]
pub fn discovery_running(discovery: State<'_, Discovery>) -> bool {
    discovery.is_running()
}

// Перенос событий mDNS в окно реализован в lib.rs (фоновый цикл с State<Discovery>).

// ---------------------------------------------------------------------------
// Настройки
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn load_config() -> AppConfig {
    config::load()
}

#[tauri::command]
pub fn save_config(cfg: AppConfig) -> Result<(), String> {
    config::save(&cfg)
}
