//! Tauri-команды: мост между UI (TS) и печатным бэкендом (winspool/CUPS),
//! discovery и config.

use tauri::State;

use crate::config::{self as config, AppConfig};
use crate::discovery::Discovery;
// `backend` — winspool.rs на Windows или cups.rs на Linux; алиас сохраняет
// прежние имена `winspool::*` внутри этого модуля.
use crate::backend::{self as winspool, JobInfo, PrinterInfo, PrinterProperties};

/// Выполняет блокирующую работу в пуле потоков, а не в главном.
///
/// Tauri выполняет синхронные команды прямо в главном потоке: пока идёт
/// `lpstat`/`lpadmin`/`avahi-browse`, окно не перерисовывается и не реагирует
/// на нажатия — при старте это выглядело как «окно появилось, но пустое
/// несколько секунд», а во время работы — как зависание. Команды, которые
/// ходят в CUPS/Spooler или файловую систему, объявлены `async` и уходят сюда.
async fn blocking<T, F>(job: F) -> Result<T, String>
where
    F: FnOnce() -> Result<T, String> + Send + 'static,
    T: Send + 'static,
{
    match tauri::async_runtime::spawn_blocking(job).await {
        Ok(result) => result,
        Err(e) => Err(format!("Фоновый поток завершился с ошибкой: {e}")),
    }
}

// ---------------------------------------------------------------------------
// Принтеры
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn list_printers() -> Result<Vec<PrinterInfo>, String> {
    blocking(winspool::list_printers).await
}

#[tauri::command]
pub async fn set_default_printer(name: String) -> Result<(), String> {
    blocking(move || winspool::set_default_printer(&name)).await
}

#[tauri::command]
pub async fn set_printer_paused(name: String, paused: bool) -> Result<(), String> {
    blocking(move || winspool::set_printer_paused(&name, paused)).await
}

#[tauri::command]
pub async fn set_printer_shared(name: String, shared: bool) -> Result<(), String> {
    blocking(move || winspool::set_printer_shared(&name, shared)).await
}

#[tauri::command]
pub async fn delete_printer(name: String) -> Result<(), String> {
    blocking(move || winspool::delete_printer(&name)).await
}

/// Свойства принтера для диалога «Свойства»: место, описание, общий доступ и
/// базовые опции драйвера (размер бумаги, двусторонняя печать, качество).
#[tauri::command]
pub async fn get_printer_properties(name: String) -> Result<PrinterProperties, String> {
    blocking(move || winspool::get_printer_properties(&name)).await
}

/// Применение свойств принтера (одна lpadmin-команда с повышением прав).
#[tauri::command]
pub async fn set_printer_properties(
    name: String,
    location: String,
    description: String,
    shared: bool,
    options: std::collections::HashMap<String, String>,
) -> Result<(), String> {
    blocking(move || {
        winspool::set_printer_properties(&name, &location, &description, shared, &options)
    })
    .await
}

#[tauri::command]
pub async fn add_printer_connection(path: String) -> Result<(), String> {
    let path = path.trim().to_string();
    if path.is_empty() {
        return Err("Укажите путь к принтеру, например \\\\сервер\\имя".into());
    }
    // На Windows путь обязан быть UNC; на Linux бэкенд принимает UNC либо
    // готовый device-uri (ipp://, socket://, smb:// …) — валидация там.
    #[cfg(windows)]
    {
        if !path.starts_with("\\\\") {
            return Err("Путь должен начинаться с \\\\ (двойного обратного слэша)".into());
        }
    }
    blocking(move || winspool::add_printer_connection(&path)).await
}

// ---------------------------------------------------------------------------
// Задания
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn list_jobs(printer: String) -> Result<Vec<JobInfo>, String> {
    blocking(move || winspool::list_jobs(&printer)).await
}

#[tauri::command]
pub async fn job_action(printer: String, job_id: u32, action: String) -> Result<(), String> {
    blocking(move || winspool::job_action(&printer, job_id, &action)).await
}

// ---------------------------------------------------------------------------
// Системные диалоги / открытие ссылок
// ---------------------------------------------------------------------------

/// Виды системных окон:
/// `properties` | `queue` | `preferences` | `test-page` | `add-wizard` | `settings`.
#[cfg(windows)]
#[tauri::command]
pub fn open_system(kind: String, name: Option<String>) -> Result<(), String> {
    use std::process::Command;

    let name = name.unwrap_or_default();
    // Имя принтера передаётся в кавычках: у принтеров с пробелами в имени
    // (`/nHP LaserJet`) PrintUIEntry не находит очередь.
    let args: Vec<String> = match kind.as_str() {
        "properties" => vec![
            "printui.dll,PrintUIEntry".into(),
            "/p".into(),
            format!("/n\"{name}\""),
        ],
        "queue" => vec![
            "printui.dll,PrintUIEntry".into(),
            "/o".into(),
            format!("/n\"{name}\""),
        ],
        "preferences" => vec![
            "printui.dll,PrintUIEntry".into(),
            "/e".into(),
            format!("/n\"{name}\""),
        ],
        "test-page" => vec![
            "printui.dll,PrintUIEntry".into(),
            "/k".into(),
            format!("/n\"{name}\""),
        ],
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

/// На Linux системные диалоги заменяются веб-интерфейсом CUPS (localhost:631),
/// а тестовая страница печатается напрямую через `lp`.
#[cfg(unix)]
#[tauri::command]
pub fn open_system(kind: String, name: Option<String>) -> Result<(), String> {
    use std::process::Command;

    let name = name.unwrap_or_default();
    let url = match kind.as_str() {
        "properties" | "preferences" | "queue" => format!("http://localhost:631/printers/{name}"),
        "add-wizard" => "http://localhost:631/admin".to_string(),
        "settings" => "http://localhost:631".to_string(),
        "test-page" => return print_test_page_unix(&name),
        other => return Err(format!("Неизвестный системный диалог: {other}")),
    };
    Command::new("xdg-open")
        .arg(&url)
        .spawn()
        .map_err(|e| format!("Не удалось открыть {url}: {e}"))?;
    Ok(())
}

/// Печатает простую тестовую страницу через `lp -d <принтер>`.
#[cfg(unix)]
fn print_test_page_unix(printer: &str) -> Result<(), String> {
    use std::process::Command;

    if printer.is_empty() {
        return Err("Не выбран принтер".into());
    }
    let mut path = std::env::temp_dir();
    path.push("printer-config-testpage.txt");
    std::fs::write(
        &path,
        "Printer Config — тестовая страница\n\nПечать работает.\n",
    )
    .map_err(|e| format!("Не удалось создать файл: {e}"))?;

    let out = Command::new("lp")
        .args(["-d", printer])
        .arg(&path)
        .output()
        .map_err(|e| format!("Не удалось запустить lp: {e}"))?;
    let _ = std::fs::remove_file(&path);

    if out.status.success() {
        Ok(())
    } else {
        let msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(if msg.is_empty() {
            "Не удалось отправить тестовую страницу".into()
        } else {
            msg
        })
    }
}

/// Печать тестовой страницы из диалога свойств: Linux — `lp -d <принтер>`,
/// Windows — системный диалог печати (`PrintUIEntry /k`).
#[tauri::command]
pub async fn print_test_page(name: String) -> Result<(), String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("Не выбран принтер".into());
    }
    blocking(move || {
        #[cfg(windows)]
        {
            open_system("test-page".to_string(), Some(name))
        }
        #[cfg(unix)]
        {
            print_test_page_unix(&name)
        }
    })
    .await
}

/// Открывает URL (веб-интерфейс найденного принтера).
#[cfg(windows)]
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

/// Открывает URL через `xdg-open` (Linux).
#[cfg(unix)]
#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("Открывать можно только http/https адреса".into());
    }
    std::process::Command::new("xdg-open")
        .arg(&url)
        .spawn()
        .map_err(|e| format!("Не удалось открыть адрес: {e}"))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Сведения о платформе
// ---------------------------------------------------------------------------

/// Целевая платформа сборки: `windows` | `linux` | `macos`.
///
/// Нужна UI, чтобы не показывать Windows-специфичные подсказки на Linux
/// (и наоборот): раньше подпись ОС в сайдбаре была захардкожена словом «Windows».
#[tauri::command]
pub fn platform() -> &'static str {
    std::env::consts::OS
}

/// Сводка о системе и состоянии принтеров — для отчётов об ошибках.
///
/// Формируется целиком на бэкенде, чтобы в отчёт попали реальные данные
/// (версия приложения, ОС, путь к файлу настроек, список очередей), а не
/// то, что напечатал пользователь по памяти.
#[tauri::command]
pub async fn diagnostics() -> String {
    match tauri::async_runtime::spawn_blocking(diagnostics_text).await {
        Ok(text) => text,
        Err(e) => format!("Не удалось собрать диагностику: {e}\n"),
    }
}

fn diagnostics_text() -> String {
    let mut out = String::new();

    out.push_str(&format!("Printer Config {}\n", env!("CARGO_PKG_VERSION")));
    out.push_str(&format!(
        "ОС: {} ({})\n",
        std::env::consts::OS,
        std::env::consts::ARCH
    ));
    out.push_str(&format!(
        "Время запуска (unix): {}\n",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    ));

    match config::config_path() {
        Ok(path) => out.push_str(&format!("Файл настроек: {}\n", path.display())),
        Err(e) => out.push_str(&format!("Файл настроек: недоступен ({e})\n")),
    }

    match winspool::list_printers() {
        Ok(list) => {
            out.push_str(&format!("Принтеров: {}\n", list.len()));
            for p in list {
                out.push_str(&format!(
                    "  - {}{} [порт: {}] [драйвер: {}]\n",
                    p.name,
                    if p.is_default {
                        " (по умолчанию)"
                    } else {
                        ""
                    },
                    if p.port.is_empty() { "—" } else { &p.port },
                    if p.driver.is_empty() {
                        "—"
                    } else {
                        &p.driver
                    },
                ));
            }
        }
        Err(e) => out.push_str(&format!("Список принтеров: ошибка ({e})\n")),
    }

    out
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
pub async fn load_config() -> AppConfig {
    match tauri::async_runtime::spawn_blocking(config::load).await {
        Ok(cfg) => cfg,
        Err(e) => {
            log::warn!("не удалось прочитать настройки: {e}");
            AppConfig::default()
        }
    }
}

#[tauri::command]
pub async fn save_config(cfg: AppConfig) -> Result<(), String> {
    blocking(move || config::save(&cfg)).await
}
