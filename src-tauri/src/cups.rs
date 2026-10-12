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
    let attempt = |p: &str| Command::new(p).args(args).output();
    // GUI-приложение может запускаться с урезанным PATH (нет /usr/sbin, где на
    // Fedora/РЕД ОС живёт lpadmin) — тогда пробуем абсолютный путь из resolve().
    let out = match attempt(prog) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            attempt(&resolve(prog)).map_err(|e2| format!("Не удалось запустить {prog}: {e2}"))?
        }
        other => other.map_err(|e| format!("Не удалось запустить {prog}: {e}"))?,
    };
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
        || e.contains("denied")
        || e.contains("interactive authentication required")
        || e.contains("не авторизован")
        || e.contains("доступ запрещ")
        // Русская локаль CUPS на РЕД ОС отвечает одним словом: «lpadmin: Запрещено».
        || e.contains("запрещ")
        || e.contains("отказано")
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
                Err(e2) => Err(format!(
                    "{e}; pkexec: {e2}. \
                     Для изменения настроек печати нужны права администратора — \
                     подтвердите запрос в окне polkit (или запустите приложение с правами root)."
                )),
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

/// Экранирование строки для встраивания в одинарные кавычки шелла.
fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// Root-скрипт добавления очереди — по образцу system-config-printer /
/// cups-pk-helper (`PrinterAdd` → `PrinterSetEnabled` → `PrinterSetAcceptJobs`):
///
/// 1. драйвер каскадом: driverless `everywhere` → `raw` → без модели;
/// 2. затем **явно** `cupsenable` + `cupsaccept` — именно это делает
///    cups-pk-helper после добавления, а одиночный `lpadmin -E` на части сборок
///    CUPS (напр. 2.2.x в РЕД ОС) оставляет очередь неактивной/невидимой;
/// 3. в конце — проверка `lpstat -p`, что очередь **действительно** появилась,
///    и вывод реального текста ошибки `lpadmin`, а не общего «не удалось».
fn admin_add_queue_script(name: &str, uri: &str) -> String {
    let n = sh_quote(name);
    let u = sh_quote(uri);
    format!(
        "err=$(mktemp 2>/dev/null || echo /tmp/.pc_lpadmin.err); \
         if lpadmin -p {n} -v {u} -E -m everywhere 2>\"$err\"; then :; \
         elif lpadmin -p {n} -v {u} -E -m raw 2>>\"$err\"; then :; \
         elif lpadmin -p {n} -v {u} -E 2>>\"$err\"; then :; \
         else cat \"$err\"; rm -f \"$err\"; exit 1; fi; \
         cupsenable {n} 2>>\"$err\"; cupsaccept {n} 2>>\"$err\"; \
         if lpstat -p {n} >/dev/null 2>&1; then rm -f \"$err\"; echo __OK__; \
         else echo 'lpadmin отработал, но очередь не появилась:'; cat \"$err\"; rm -f \"$err\"; exit 1; fi"
    )
}

/// Запуск shell-скрипта с повышением прав: сначала от текущего пользователя
/// (если CUPS разрешён без auth), при ошибке прав — через `pkexec /bin/sh -c`
/// одним запросом polkit (вся последовательность add+enable+accept атомарна).
fn run_admin_script(script: &str) -> Result<String, String> {
    match run("sh", &["-c", script]) {
        Ok(out) => Ok(out),
        Err(e) if looks_like_permission_error(&e) => {
            match run("pkexec", &["/bin/sh", "-c", script]) {
                Ok(out) => Ok(out),
                Err(e2) => Err(format!(
                    "{e}; pkexec: {e2}. Нужны права администратора — подтвердите запрос в окне polkit \
                     или добавьте принтер через «Печать» → «Добавить принтер» (system-config-printer)."
                )),
            }
        }
        Err(e) => Err(e),
    }
}

fn add_with_uri(name: &str, uri: &str) -> Result<(), String> {
    let ctx = |e: String| format!("Не удалось создать очередь «{name}» ({uri}): {e}");
    run_admin_script(&admin_add_queue_script(name, uri))
        .and_then(|out| {
            if out.contains("__OK__") {
                Ok(())
            } else if out.trim().is_empty() {
                Ok(())
            } else {
                Err(out.trim().to_string())
            }
        })
        .map_err(ctx)
}

// ---------------------------------------------------------------------------
// Добавление по «голому» адресу (IP / имя хоста) — то, что вводит пользователь,
// когда знает только IP принтера: «192.168.1.50», «printer.local:631»,
// «192.168.1.50/Очередь», «[fd00::1]».
// ---------------------------------------------------------------------------

/// Разобранная цель вида «хост [:порт] [/очередь]».
#[derive(Debug, PartialEq)]
struct BareTarget {
    host: String,
    /// Явно указанный порт; `None` — используются стандартные (631/9100).
    port: Option<u16>,
    /// Имя очереди CUPS за «/»; `None` — очередь неизвестна, подбираем авто-пробой.
    queue: Option<String>,
}

/// Разбор `host`, `host:port`, `host/queue`, `[ipv6]`, `[ipv6]:port/queue`.
///
/// UNC-пути и URI схем разбираются до вызова этой функции — здесь принимается
/// только «чистый» хост без пробелов, слэшей и схем.
fn parse_bare_target(input: &str) -> Option<BareTarget> {
    let s = input.trim();
    if s.is_empty()
        || s.contains("://")
        || s.starts_with('\\')
        || s.chars().any(char::is_whitespace)
    {
        return None;
    }
    let (hostpart, queue) = match s.split_once('/') {
        Some((h, q)) => {
            if q.is_empty() {
                return None;
            }
            (h, Some(q.to_string()))
        }
        None => (s, None),
    };
    let (host, port) = if let Some(r) = hostpart.strip_prefix('[') {
        // IPv6 в квадратных скобках: [fd00::1]:631
        let (h, r) = r.split_once(']')?;
        let port = match r.strip_prefix(':') {
            Some(p) => Some(p.parse::<u16>().ok()?),
            None => None,
        };
        (h.to_string(), port)
    } else {
        match hostpart.split_once(':') {
            Some((h, p)) => (h.to_string(), Some(p.parse::<u16>().ok()?)),
            None => (hostpart.to_string(), None),
        }
    };
    if host.is_empty()
        || !host.chars().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ':')
        })
    {
        return None;
    }
    Some(BareTarget { host, port, queue })
}

/// IPv6-литерал должен заключаться в скобки внутри URI.
fn fmt_host(host: &str) -> String {
    if host.contains(':') {
        format!("[{host}]")
    } else {
        host.to_string()
    }
}

/// Запрос IPP `Get-Printer-Attributes` (RFC 8010/8011) в байтах.
fn ipp_get_printer_attributes_bytes(printer_uri: &str) -> Vec<u8> {
    fn push_str_attr(buf: &mut Vec<u8>, tag: u8, name: &str, value: &str) {
        buf.push(tag);
        buf.extend_from_slice(&(name.len() as u16).to_be_bytes());
        buf.extend_from_slice(name.as_bytes());
        buf.extend_from_slice(&(value.len() as u16).to_be_bytes());
        buf.extend_from_slice(value.as_bytes());
    }
    let mut buf = vec![0x02, 0x00]; // IPP 2.0
    buf.extend_from_slice(&[0x00, 0x0B]); // operation: Get-Printer-Attributes
    buf.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]); // request-id = 1
    buf.push(0x01); // operation-attributes-tag
    push_str_attr(&mut buf, 0x47, "attributes-charset", "utf-8");
    push_str_attr(&mut buf, 0x48, "attributes-natural-language", "en");
    push_str_attr(&mut buf, 0x45, "printer-uri", printer_uri);
    buf.push(0x03); // end-of-attributes-tag
    buf
}

/// Тело HTTP-ответа после заголовков (`\r\n\r\n`).
fn split_http_body(resp: &[u8]) -> Option<(&[u8], &[u8])> {
    resp.windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|i| (&resp[..i], &resp[i + 4..]))
}

/// Проверка IPP-сервиса: POST `Get-Printer-Attributes` на `http://host:port{path}`.
///
/// Принтер/сервер, у которого путь существует, отвечает HTTP 200 и IPP-кодом
/// статуса < 0x0200 (уровень «ok»). Несуществующий путь даёт HTTP 404 либо
/// IPP client-error — считаем, что очередь не найдена.
fn ipp_probe(host: &str, port: u16, path: &str) -> bool {
    use std::io::{Read, Write};
    use std::net::{TcpStream, ToSocketAddrs};
    use std::time::Duration;

    let Ok(addrs) = (host, port).to_socket_addrs() else {
        return false;
    };
    let printer_uri = format!("ipp://{}:{port}{path}", fmt_host(host));
    let body = ipp_get_printer_attributes_bytes(&printer_uri);
    let req = format!(
        "POST {path} HTTP/1.1\r\nHost: {}:{port}\r\nContent-Type: application/ipp\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        fmt_host(host),
        body.len()
    );
    for addr in addrs {
        let Ok(mut sock) = TcpStream::connect_timeout(&addr, Duration::from_millis(1500)) else {
            continue;
        };
        let _ = sock.set_read_timeout(Some(Duration::from_secs(3)));
        let _ = sock.set_write_timeout(Some(Duration::from_secs(3)));
        if sock.write_all(req.as_bytes()).is_err() || sock.write_all(&body).is_err() {
            continue;
        }
        let mut resp = Vec::new();
        let _ = sock.read_to_end(&mut resp);
        let Some((head, ipp)) = split_http_body(&resp) else {
            continue;
        };
        let ok_200 = head.starts_with(b"HTTP/1.0 200") || head.starts_with(b"HTTP/1.1 200");
        if ok_200 && ipp.len() >= 8 {
            let status = u16::from_be_bytes([ipp[2], ipp[3]]);
            if status < 0x0200 {
                return true;
            }
        }
    }
    false
}

/// Проверка raw-порта принтера (`socket://host:9100`) обычным TCP-connect.
fn socket_probe(host: &str, port: u16) -> bool {
    use std::net::{TcpStream, ToSocketAddrs};
    use std::time::Duration;

    let Ok(addrs) = (host, port).to_socket_addrs() else {
        return false;
    };
    addrs
        .into_iter()
        .any(|a| TcpStream::connect_timeout(&a, Duration::from_millis(1500)).is_ok())
}

/// Подбор рабочего device-uri для «голого» адреса.
///
/// Порядок проб (быстрее → надёжнее):
/// * с очередью — `ipp://host:port/printers/queue`, затем `ipp://host:port/queue`;
/// * без очереди — стандартные IPP-пути современных принтеров `/ipp/print`, `/ipp`;
/// * запасной вариант — сырой socket-порт 9100 (открыт почти у всех сетевых принтеров).
fn resolve_bare_target(t: &BareTarget) -> Result<(String, String), String> {
    let host = &t.host;
    let port = t.port.unwrap_or(631);
    let h = fmt_host(host);
    let name = sanitize_printer_name(t.queue.as_deref().unwrap_or(host));

    if let Some(q) = &t.queue {
        for path in [format!("/printers/{q}"), format!("/{q}")] {
            if ipp_probe(host, port, &path) {
                return Ok((name, format!("ipp://{h}:{port}{path}")));
            }
        }
        return Err(format!(
            "Принтер {h} не ответил на IPP-запрос (порт {port}, очередь «{q}»). \
             Проверьте адрес и имя очереди либо укажите полный URI (ipp://…)."
        ));
    }

    for path in ["/ipp/print", "/ipp"] {
        if ipp_probe(host, port, path) {
            return Ok((name, format!("ipp://{h}:{port}{path}")));
        }
    }
    let mut socket_ports = vec![9100u16];
    if port != 631 && !socket_ports.contains(&port) {
        socket_ports.push(port);
    }
    for p in socket_ports {
        if socket_probe(host, p) {
            return Ok((name, format!("socket://{h}:{p}")));
        }
    }
    Err(format!(
        "Принтер {h} не отвечает: IPP (порт 631) и raw-печать (порт 9100) недоступны. \
         Проверьте, что принтер включён и в той же сети, либо укажите URI вручную \
         (ipp://{h}/…)."
    ))
}

/// Подключение сетевого принтера: принимает `\\хост\очередь` (Windows-путь,
/// присылаемый UI), готовый device-uri (`ipp://`, `socket://`, `smb://` …)
/// либо «голый» адрес — `192.168.1.50`, `printer.local:631`, `192.168.1.50/Очередь`
/// (device-uri подбирается пробой IPP/socket, см. `resolve_bare_target`).
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

    // «Голый» адрес: IP/хост [:порт] [/очередь] — пробуем подобрать device-uri.
    if let Some(target) = parse_bare_target(path) {
        let (name, uri) = resolve_bare_target(&target)?;
        return add_with_uri(&name, &uri);
    }

    Err("Ожидается IP-адрес (192.168.1.50), \\\\хост\\очередь или URI (ipp://, socket://, smb:// …)".into())
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
        // Русская локаль РЕД ОС: одно слово «Запрещено».
        assert!(looks_like_permission_error("lpadmin: Запрещено"));
        assert!(looks_like_permission_error("lpadmin: запрещено"));
        assert!(looks_like_permission_error("Access Denied"));
        assert!(!looks_like_permission_error("lpadmin: Unknown destination \"x\""));
        assert!(!looks_like_permission_error("make_and_model \"Local Raw Printer\""));
    }

    // --- разбор «голого» адреса ---

    #[test]
    fn bare_target_parses_ip_host_port_and_queue() {
        assert_eq!(
            parse_bare_target("192.168.1.50"),
            Some(BareTarget {
                host: "192.168.1.50".into(),
                port: None,
                queue: None
            })
        );
        assert_eq!(
            parse_bare_target("printer.local:631"),
            Some(BareTarget {
                host: "printer.local".into(),
                port: Some(631),
                queue: None
            })
        );
        assert_eq!(
            parse_bare_target("192.168.1.50/LaserJet"),
            Some(BareTarget {
                host: "192.168.1.50".into(),
                port: None,
                queue: Some("LaserJet".into())
            })
        );
        // IPv6 в скобках с портом и очередью.
        assert_eq!(
            parse_bare_target("[fd00::1]:631/printers/a"),
            Some(BareTarget {
                host: "fd00::1".into(),
                port: Some(631),
                queue: Some("printers/a".into())
            })
        );
    }

    #[test]
    fn bare_target_rejects_non_host_input() {
        // URI и UNC разбираются другими ветками — здесь они не должны приниматься.
        assert_eq!(parse_bare_target("ipp://192.168.1.50/ipp/print"), None);
        assert_eq!(parse_bare_target("\\\\server\\queue"), None);
        assert_eq!(parse_bare_target("bad host"), None);
        assert_eq!(parse_bare_target(""), None);
        assert_eq!(parse_bare_target("host/"), None);
        assert_eq!(parse_bare_target("host:notaport"), None);
    }

    #[test]
    fn fmt_host_wraps_ipv6_only() {
        assert_eq!(fmt_host("192.168.1.50"), "192.168.1.50");
        assert_eq!(fmt_host("printer.local"), "printer.local");
        assert_eq!(fmt_host("fd00::1"), "[fd00::1]");
    }

    // --- IPP-обвязка ---

    #[test]
    fn ipp_request_is_valid_get_printer_attributes() {
        let b = ipp_get_printer_attributes_bytes("ipp://192.168.1.50:631/ipp/print");
        // version 2.0, operation Get-Printer-Attributes (0x000B), request-id 1.
        assert_eq!(&b[..2], &[0x02, 0x00]);
        assert_eq!(&b[2..4], &[0x00, 0x0B]);
        assert_eq!(&b[4..8], &[0x00, 0x00, 0x00, 0x01]);
        assert_eq!(b[8], 0x01); // operation-attributes-tag
        assert_eq!(*b.last().unwrap(), 0x03); // end-of-attributes-tag
        // Атрибут printer-uri обязан присутствовать в запросе.
        let needle = b"ipp://192.168.1.50:631/ipp/print";
        assert!(b.windows(needle.len()).any(|w| w == needle));
    }

    #[test]
    fn split_http_body_finds_separator() {
        let raw = b"HTTP/1.1 200 OK\r\nContent-Type: application/ipp\r\n\r\nBODY";
        let (head, body) = split_http_body(raw).unwrap();
        assert!(head.starts_with(b"HTTP/1.1 200"));
        assert_eq!(body, b"BODY");
        assert_eq!(split_http_body(b"no separator here"), None);
    }
}
