//! Linux/CUPS-бэкенд: реализует тот же API, что и Windows-модуль `winspool.rs`,
//! но поверх утилит CUPS (`lpstat`, `lpadmin`, `cupsenable`, `cancel`, `lpq`, …).
//!
//! Административные операции сначала выполняются напрямую, а при нехватке прав
//! повторяются через `pkexec` (polkit) — так же, как это делает
//! system-config-printer. Значения `statusText`/`attributeText` совпадают с
//! Windows-версией («Готов», «Пауза», «Общий доступ», «Сетевой» …), чтобы UI
//! работал без изменений.

use serde::Serialize;
use std::collections::HashMap;
use std::process::Command;

// ---------------------------------------------------------------------------
// Модели данных — должны совпадать с winspool.rs (serde camelCase).
// ---------------------------------------------------------------------------

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PrinterInfo {
    pub name: String,
    pub share_name: String,
    pub port: String,
    pub driver: String,
    pub comment: String,
    pub location: String,
    pub attributes: u32,
    pub status: u32,
    pub jobs: u32,
    pub is_default: bool,
    pub status_text: Vec<String>,
    pub attribute_text: Vec<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct JobInfo {
    pub id: u32,
    pub printer: String,
    pub document: String,
    pub user: String,
    pub datatype: String,
    pub status: u32,
    pub status_text: Vec<String>,
    pub total_pages: u32,
    pub pages_printed: u32,
    pub priority: u32,
    pub position: u32,
    pub submitted: String,
}

// Биты атрибутов/статусов: UI использует только текстовые поля, поэтому значения
// произвольны, но семантически согласованы с Windows-версией.
const ATTR_LOCAL: u32 = 0x0000_0002;
const ATTR_SHARED: u32 = 0x0000_0008;
const ATTR_NETWORK: u32 = 0x0000_0100;
const STATUS_PAUSED: u32 = 0x0000_0001;
const STATUS_BUSY: u32 = 0x0000_0010;

// ---------------------------------------------------------------------------
// Запуск процессов и повышение привилегий
// ---------------------------------------------------------------------------

fn run(prog: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new(prog)
        .args(args)
        .output()
        .map_err(|e| format!("Не удалось запустить {prog}: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let msg = if !stderr.is_empty() { stderr } else { stdout };
        Err(if msg.is_empty() {
            format!("{prog} завершился с кодом {}", out.status.code().unwrap_or(-1))
        } else {
            msg
        })
    }
}

/// Полный путь к бинарнику (для pkexec, которому нужен абсолютный путь).
fn resolve(prog: &str) -> String {
    if let Ok(out) = run("sh", &["-c", &format!("command -v {prog}")]) {
        let p = out.trim().to_string();
        if !p.is_empty() {
            return p;
        }
    }
    for dir in ["/usr/bin", "/usr/sbin", "/bin", "/sbin"] {
        let candidate = format!("{dir}/{prog}");
        if std::path::Path::new(&candidate).exists() {
            return candidate;
        }
    }
    format!("/usr/bin/{prog}")
}

fn looks_like_permission_error(err: &str) -> bool {
    let e = err.to_lowercase();
    e.contains("access denied")
        || e.contains("not authorized")
        || e.contains("unauthorized")
        || e.contains("forbidden")
        || e.contains("permission denied")
        || e.contains("interactive authentication required")
        || e.contains("не авторизован")
        || e.contains("доступ запрещ")
        || e.contains("login as root")
}

/// Административная команда: сначала напрямую, при нехватке прав — через pkexec.
fn run_admin(prog: &str, args: &[&str]) -> Result<String, String> {
    match run(prog, args) {
        Ok(out) => Ok(out),
        Err(e) if looks_like_permission_error(&e) => {
            let full = resolve(prog);
            let mut pk_args: Vec<&str> = vec![&full];
            pk_args.extend_from_slice(args);
            match run("pkexec", &pk_args) {
                Ok(out) => Ok(out),
                Err(e2) => Err(format!("{e}; pkexec: {e2}")),
            }
        }
        Err(e) => Err(e),
    }
}

fn run_admin_quiet(prog: &str, args: &[&str]) -> Result<(), String> {
    run_admin(prog, args).map(|_| ())
}

// ---------------------------------------------------------------------------
// Разбор вывода lpstat / lpq
// ---------------------------------------------------------------------------

/// Имя принтера по умолчанию из `lpstat -d`.
fn parse_default(text: &str) -> Option<String> {
    for line in text.lines() {
        if let Some(rest) = line.trim().strip_prefix("system default destination:") {
            let name = rest.trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

/// Разбор `lpstat -v` → список (имя, device-uri).
fn parse_devices(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in text.lines() {
        if let Some(rest) = line.trim().strip_prefix("device for ") {
            if let Some((name, uri)) = rest.split_once(":") {
                out.push((name.trim().to_string(), uri.trim().to_string()));
            }
        }
    }
    out
}

/// Разбор `lpstat -l -p` → имя → (описание, местоположение, драйвер/PPD).
fn parse_details(text: &str) -> HashMap<String, (String, String, String)> {
    let mut map: HashMap<String, (String, String, String)> = HashMap::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        if line.starts_with("printer ") {
            let rest = line.trim_start().strip_prefix("printer ").unwrap_or("");
            let name = rest.split_whitespace().next().unwrap_or("").to_string();
            if !name.is_empty() {
                map.entry(name.clone()).or_default();
                current = Some(name);
            }
            continue;
        }
        let Some(key) = current.as_ref() else { continue };
        let trimmed = line.trim();
        let Some((k, v)) = trimmed.split_once(':') else { continue };
        let k = k.trim().to_lowercase();
        let v = v.trim().to_string();
        let entry = map.entry(key.clone()).or_default();
        match k.as_str() {
            "description" => entry.0 = v,
            "location" => entry.1 = v,
            "ppd" | "driver" => {
                entry.2 = std::path::Path::new(&v)
                    .file_name()
                    .map(|f| f.to_string_lossy().into_owned())
                    .unwrap_or(v);
            }
            _ => {}
        }
    }
    map
}

/// Текущее значение опции принтера (`lpoptions -p <name>`).
fn printer_option(name: &str, key: &str) -> Option<String> {
    let out = run("lpoptions", &["-p", name]).ok()?;
    out.split_whitespace().find_map(|tok| {
        let (k, v) = tok.split_once('=')?;
        (k == key).then(|| v.to_string())
    })
}

fn is_shared(name: &str) -> bool {
    printer_option(name, "printer-is-shared")
        .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
        .unwrap_or(false)
}

/// Сетевой ли device-uri (ipp/socket/lpd/smb/dnssd/http…).
fn is_network_uri(uri: &str) -> bool {
    let u = uri.to_lowercase();
    u.starts_with("ipp://")
        || u.starts_with("ipps://")
        || u.starts_with("socket://")
        || u.starts_with("lpd://")
        || u.starts_with("smb://")
        || u.starts_with("dnssd://")
        || u.starts_with("http://")
        || u.starts_with("https://")
        || u.starts_with("ipp14://")
}

/// Разбор `lpstat -o` (все задания) → счётчик заданий по каждому принтеру.
fn count_jobs_map(text: &str) -> HashMap<String, u32> {
    let mut map: HashMap<String, u32> = HashMap::new();
    for line in text.lines() {
        let first = line.split_whitespace().next().unwrap_or("");
        if let Some((printer, id)) = first.rsplit_once('-') {
            if id.parse::<u32>().is_ok() && !printer.is_empty() {
                *map.entry(printer.to_string()).or_insert(0) += 1;
            }
        }
    }
    map
}

// ---------------------------------------------------------------------------
// Публичный API (совпадает по сигнатуре с winspool.rs)
// ---------------------------------------------------------------------------

pub fn list_printers() -> Result<Vec<PrinterInfo>, String> {
    let states = match run("lpstat", &["-p"]) {
        Ok(s) => s,
        Err(e) if e.to_lowercase().contains("no destinations") => String::new(),
        Err(e) => return Err(e),
    };
    let devices = run("lpstat", &["-v"]).unwrap_or_default();
    let details = run("lpstat", &["-l", "-p"]).unwrap_or_default();
    let default = run("lpstat", &["-d"]).unwrap_or_default();
    let all_jobs = run("lpstat", &["-o"]).unwrap_or_default();

    let default_name = parse_default(&default);
    let uris = parse_devices(&devices);
    let details_map = parse_details(&details);
    let jobs_map = count_jobs_map(&all_jobs);

    let mut out = Vec::new();
    for line in states.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("printer ") else {
            continue;
        };
        let name = rest.split_whitespace().next().unwrap_or("").to_string();
        if name.is_empty() {
            continue;
        }

        let is_paused = rest.contains("disabled");
        let is_printing = rest.contains("is printing") || rest.contains("is processing");

        let uri = uris
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, u)| u.clone())
            .unwrap_or_default();
        let (comment, location, driver) =
            details_map.get(&name).cloned().unwrap_or_default();
        let shared = is_shared(&name);
        let network = is_network_uri(&uri);

        let status_text = if is_paused {
            vec!["Пауза".to_string()]
        } else if is_printing {
            vec!["Печать".to_string()]
        } else {
            vec!["Готов".to_string()]
        };

        let mut attribute_text = Vec::new();
        if shared {
            attribute_text.push("Общий доступ".to_string());
        }
        attribute_text.push(if network { "Сетевой" } else { "Локальный" }.to_string());

        let mut attributes = if network { ATTR_NETWORK } else { ATTR_LOCAL };
        if shared {
            attributes |= ATTR_SHARED;
        }
        let status = if is_paused {
            STATUS_PAUSED
        } else if is_printing {
            STATUS_BUSY
        } else {
            0
        };

        out.push(PrinterInfo {
            is_default: default_name.as_deref() == Some(name.as_str()),
            jobs: jobs_map.get(&name).copied().unwrap_or(0),
            share_name: name.clone(),
            name,
            port: uri,
            driver,
            comment,
            location,
            attributes,
            status,
            status_text,
            attribute_text,
        });
    }
    Ok(out)
}

pub fn set_default_printer(name: &str) -> Result<(), String> {
    run_admin_quiet("lpadmin", &["-d", name])
}

pub fn set_printer_paused(name: &str, paused: bool) -> Result<(), String> {
    if paused {
        run_admin_quiet("cupsdisable", &[name])
    } else {
        run_admin_quiet("cupsenable", &[name])
    }
}

pub fn set_printer_shared(name: &str, shared: bool) -> Result<(), String> {
    let value = if shared { "true" } else { "false" };
    run_admin_quiet("lpadmin", &["-p", name, "-o", &format!("printer-is-shared={value}")])
}

pub fn delete_printer(name: &str) -> Result<(), String> {
    run_admin_quiet("lpadmin", &["-x", name])
}

/// Санитизация имени принтера для CUPS (допустимы буквы, цифры, `-`, `_`).
fn sanitize_printer_name(raw: &str) -> String {
    let s: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let s = s.trim_matches('-').to_string();
    if s.is_empty() {
        "printer".to_string()
    } else {
        s
    }
}

fn add_with_uri(name: &str, uri: &str) -> Result<(), String> {
    // Драйвер: для IPP-принтеров — driverless (everywhere), иначе пробуем без -m.
    if uri.starts_with("ipp://") || uri.starts_with("ipps://") || uri.starts_with("dnssd://") {
        run_admin_quiet("lpadmin", &["-p", name, "-v", uri, "-E", "-m", "everywhere"])
            .or_else(|_| run_admin_quiet("lpadmin", &["-p", name, "-v", uri, "-E"]))
    } else {
        run_admin_quiet("lpadmin", &["-p", name, "-v", uri, "-E"])
    }
}

/// Подключение сетевого принтера: принимает `\\хост\очередь` (Windows-путь,
/// присылаемый UI) или готовый device-uri (`ipp://`, `socket://`, `smb://` …).
pub fn add_printer_connection(path: &str) -> Result<(), String> {
    let path = path.trim();
    if path.is_empty() {
        return Err("Укажите адрес принтера".into());
    }

    // Уже готовый URI.
    if let Some((scheme, _)) = path.split_once("://") {
        if matches!(
            scheme,
            "ipp" | "ipps" | "socket" | "lpd" | "smb" | "dnssd" | "http" | "https"
        ) {
            let name = sanitize_printer_name(path.rsplit('/').next().unwrap_or("printer"));
            return add_with_uri(&name, path);
        }
    }

    // Windows UNC: \\хост\очередь
    if let Some(rest) = path.strip_prefix("\\\\") {
        let mut parts = rest.split('\\');
        let host = parts.next().unwrap_or("").trim().to_string();
        let instance = parts.next().unwrap_or("").trim().to_string();
        if host.is_empty() {
            return Err("Не указан хост принтера".into());
        }
        let (name, uri) = if instance.is_empty() {
            (sanitize_printer_name(&host), format!("socket://{host}:9100"))
        } else {
            (
                sanitize_printer_name(&instance),
                format!("ipp://{host}/{instance}"),
            )
        };
        return add_with_uri(&name, &uri);
    }

    Err("Ожидается путь \\\\хост\\очередь или URI (ipp://, socket://, smb:// …)".into())
}

// ---------------------------------------------------------------------------
// Задания
// ---------------------------------------------------------------------------

/// Индекс, с которого начинается «мусорный» хвост строки `lpq` (размер файла).
///
/// Последние токены строки `lpq` — это размер задания, и его формат зависит от
/// версии CUPS: `1024 bytes` (два токена), `1024` (один) либо слитный `1.5M`.
/// Без отсечения хвоста размер попадает в имя документа («(stdin) 1024»).
fn size_tail_start(t: &[&str]) -> usize {
    let mut end = t.len();
    while end > 3 {
        let last = t[end - 1].to_lowercase();
        if matches!(
            last.as_str(),
            "bytes" | "byte" | "b" | "k" | "kb" | "kib" | "m" | "mb" | "mib" | "g" | "gb" | "gib"
        ) {
            end -= 1;
            continue;
        }
        // Число («1024») либо число со слитным суффиксом («1.5M»).
        let is_number = last.parse::<f64>().is_ok();
        let is_number_with_suffix = last.len() > 1
            && last[..last.len() - 1].parse::<f64>().is_ok()
            && last.ends_with(['b', 'k', 'm', 'g', 't']);
        if is_number || is_number_with_suffix {
            end -= 1;
        }
        break;
    }
    end
}

/// Разбор `lpq -P <printer>`: колонки rank / owner / job / file(s) / size.
fn parse_lpq(printer: &str, out: &str) -> Vec<JobInfo> {
    let mut jobs: Vec<JobInfo> = Vec::new();
    for line in out.lines() {
        let t: Vec<&str> = line.split_whitespace().collect();
        // Заголовок («Rank Owner Job …») и баннер («<имя> is ready») пропускаем.
        if t.len() < 5 || t[0].eq_ignore_ascii_case("rank") || t[2].eq_ignore_ascii_case("job") {
            continue;
        }
        let Ok(id) = t[2].parse::<u32>() else { continue };
        let rank = t[0].to_ascii_lowercase();
        let user = t[1].to_string();
        let document = t[3..size_tail_start(&t)].join(" ");

        let (status, status_text) = match rank.as_str() {
            "active" | "processing" => (STATUS_BUSY, "Печать"),
            "held" | "stopped" | "paused" => (STATUS_PAUSED, "Пауза"),
            _ => (0, "В очереди"),
        };

        jobs.push(JobInfo {
            id,
            printer: printer.to_string(),
            document,
            user,
            datatype: "application/octet-stream".to_string(),
            status,
            status_text: vec![status_text.to_string()],
            total_pages: 0,
            pages_printed: 0,
            priority: 0,
            position: jobs.len() as u32 + 1,
            submitted: String::new(),
        });
    }
    jobs
}

pub fn list_jobs(printer: &str) -> Result<Vec<JobInfo>, String> {
    let out = match run("lpq", &["-P", printer]) {
        Ok(o) => o,
        Err(e) if e.to_lowercase().contains("no entries") => String::new(),
        Err(e) => return Err(e),
    };
    Ok(parse_lpq(printer, &out))
}

/// Действия над заданием: `cancel` | `pause` | `resume` | `restart`.
///
/// Пауза/продолжение/перезапуск выполняются через `lp -i <очередь-№> -H <hold|release|restart>`.
/// Утилит `lppause`/`lpresume` (из LPRng) в CUPS нет, поэтому прежний вызов всегда
/// заканчивался ошибкой «Не удалось запустить lppause».
pub fn job_action(printer: &str, job_id: u32, action: &str) -> Result<(), String> {
    let target = format!("{printer}-{job_id}");
    match action {
        "cancel" => run_admin_quiet("cancel", &[&target]),
        "pause" => run_admin_quiet("lp", &["-i", &target, "-H", "hold"]),
        "resume" => run_admin_quiet("lp", &["-i", &target, "-H", "release"]),
        "restart" => run_admin_quiet("lp", &["-i", &target, "-H", "restart"]),
        other => Err(format!("Неизвестное действие над заданием: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lpq_parses_active_job() {
        let out = "\
HP is ready
 no rank   owner   job  file(s)                         size
 active    root    5    test document.pdf               1024
";
        let jobs = parse_lpq("HP", out);
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].id, 5);
        assert_eq!(jobs[0].user, "root");
        assert_eq!(jobs[0].document, "test document.pdf");
        assert_eq!(jobs[0].status_text, vec!["Печать"]);
    }

    #[test]
    fn lpq_skips_banner_and_header() {
        let out = "\
My Printer is ready
 no rank   owner   job  file(s)                         size
";
        assert!(parse_lpq("My Printer", out).is_empty());
    }

    #[test]
    fn lpq_strips_size_from_document() {
        // Реальный вывод CUPS 2.x: размер занимает ДВА токена («1024 bytes»),
        // и раньше «1024» попадало в имя документа.
        let out = "\
FilePrn is not ready
Rank    Owner   Job     File(s)                         Total Size
1st     root    3       (stdin)                         1024 bytes
2nd     root    6       my report.pdf                   2048 bytes
";
        let jobs = parse_lpq("FilePrn", out);
        assert_eq!(jobs.len(), 2);
        assert_eq!(jobs[0].document, "(stdin)");
        assert_eq!(jobs[1].document, "my report.pdf");
        // Очередь (не активное задание) — «В очереди» со статус-кодом 0.
        assert_eq!(jobs[0].status_text, vec!["В очереди"]);
        assert_eq!(jobs[0].status, 0);
        assert_eq!(jobs[0].position, 1);
        assert_eq!(jobs[1].position, 2);
    }

    #[test]
    fn lpq_handles_compact_size_token() {
        // Слитный размер («1.5M») не должен попадать в имя документа.
        let out = "ready\nRank Owner Job File(s) Total Size\nactive root 9 invoice.pdf 1.5M\n";
        let jobs = parse_lpq("P", out);
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].document, "invoice.pdf");
    }

    #[test]
    fn lpq_empty_queue_has_no_jobs() {
        let out = "FilePrn is ready\nRank    Owner   Job     File(s)   Total Size\n";
        assert!(parse_lpq("FilePrn", out).is_empty());
    }

    #[test]
    fn parse_devices_extracts_uri() {
        let out = "device for HP: ipp://192.168.1.10/ipp/print\n";
        let d = parse_devices(out);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].0, "HP");
        assert_eq!(d[0].1, "ipp://192.168.1.10/ipp/print");
    }

    #[test]
    fn parse_default_reads_name() {
        assert_eq!(parse_default("system default destination: HP\n").as_deref(), Some("HP"));
        assert_eq!(parse_default("no system default destination\n"), None);
    }

    #[test]
    fn network_uri_detection() {
        assert!(is_network_uri("ipp://host/ipp/print"));
        assert!(is_network_uri("socket://host:9100"));
        assert!(!is_network_uri("usb://HP/Deskjet"));
        assert!(!is_network_uri("file:///dev/null"));
    }

    #[test]
    fn sanitize_printer_name_matches_cups_rules() {
        assert_eq!(sanitize_printer_name("HP LaserJet 400"), "hp-laserjet-400");
        assert_eq!(sanitize_printer_name(""), "printer");
        // Разделители по краям обрезаются, иначе имя начиналось бы с «-».
        assert_eq!(sanitize_printer_name("!!HP!!"), "hp");
        assert_eq!(sanitize_printer_name("..."), "printer");
    }

    #[test]
    fn unc_path_parses_host_and_queue() {
        let path = "\\\\server\\printqueue";
        let rest = path.strip_prefix("\\\\").unwrap();
        let mut parts = rest.split('\\');
        assert_eq!(parts.next(), Some("server"));
        assert_eq!(parts.next(), Some("printqueue"));
    }

    #[test]
    fn permission_error_detection() {
        assert!(looks_like_permission_error("lpadmin: Unauthorized"));
        assert!(looks_like_permission_error("Interactive authentication required."));
        assert!(!looks_like_permission_error("lpadmin: Unknown destination \"x\""));
    }
}
