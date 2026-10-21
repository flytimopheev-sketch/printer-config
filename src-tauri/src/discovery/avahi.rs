//! Обнаружение принтеров по mDNS/DNS-SD на Linux / РЕД ОС.
//!
//! Используется системная утилита `avahi-browse` (пакет `avahi`): она
//! корректно работает в сетях с несколькими интерфейсами и не требует
//! дополнительных библиотек в приложении.
//!
//! Формат вывода `--parsable`:
//!   =eth0IPv4HP OfficeJet_ipp._tcp.local-
//!   hostname = [hp.local] address = [192.168.1.50] port = [631] txt = []

use std::collections::HashMap;
use std::collections::HashSet;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use super::{DiscoveryEvent, FoundPrinter};

/// Типы сервисов, которые ищутся: IPP, «сырой» порт и старый протокол JetDirect.
const SERVICE_TYPES: [&str; 3] = ["_ipp._tcp", "_pdl-datastream._tcp", "_printer._tcp"];

/// Периодичность полного прохода по сети.
const SCAN_INTERVAL: Duration = Duration::from_secs(5);

/// Шаг ожидания: сканирование делится на короткие интервалы, чтобы команда
/// «Остановить» срабатывала быстро.
const TICK: Duration = Duration::from_millis(500);

/// Основной цикл сканера: пока флаг `running` установлен — повторяем
/// проходы по сети и сообщаем UI о найденных и исчезнувших принтерах.
pub(super) fn run(tx: Sender<DiscoveryEvent>, running: Arc<AtomicBool>) -> Result<(), String> {
    let mut known: HashSet<String> = HashSet::new();

    while running.load(Ordering::SeqCst) {
        match scan_once(&tx) {
            Ok(found) => {
                // Принтеры, которые были в прошлом проходе, но пропали сейчас.
                for fullname in known.difference(&found) {
                    let _ = tx.send(DiscoveryEvent::Removed {
                        fullname: fullname.clone(),
                    });
                }
                known = found;
            }
            Err(e) => log::warn!("mDNS scan error: {e}"),
        }

        // Ждём, но небольшими шагами: так команда «Остановить» не ждёт
        // конца текущего прохода.
        let ticks = SCAN_INTERVAL.as_millis() / TICK.as_millis();
        for _ in 0..ticks.max(1) {
            if !running.load(Ordering::SeqCst) {
                break;
            }
            thread::sleep(TICK);
        }
    }

    Ok(())
}

/// Один проход сканирования через `avahi-browse`.
/// Возвращает множество найденных полных имён (`instance._type.local.`).
fn scan_once(tx: &Sender<DiscoveryEvent>) -> Result<HashSet<String>, String> {
    let mut args: Vec<&str> = vec!["--parsable", "--terminate"];
    for service in SERVICE_TYPES {
        args.push("-t");
        args.push(service);
    }

    let output = Command::new("avahi-browse")
        .args(&args)
        .output()
        .map_err(|e| format!("avahi-browse не найден (установите пакет avahi): {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut current_name: Option<String> = None;
    let mut current_type: Option<String> = None;
    let mut found: HashSet<String> = HashSet::new();

    for line in stdout.lines() {
        let trimmed = line.trim();

        // Строка службы: =interface  protocol  name  type  domain  -
        if trimmed.starts_with('=') && !trimmed.starts_with("==") {
            let fields: Vec<&str> = trimmed.split('\t').collect();
            if fields.len() >= 5 {
                current_name = Some(fields[3].to_string());
                current_type = Some(fields[4].to_string());
            }
            continue;
        }

        // Строка разрешения: hostname = [...] address = [...] port = [...] txt = [...]
        if trimmed.starts_with("hostname") || trimmed.contains("hostname =") {
            if let Some(name) = current_name.take() {
                let service_type = current_type.take().unwrap_or_default();

                // Извлекаем поля из строки вида: hostname = [hp.local] address = [1.2.3.4] port = [631] txt = []
                let mut host = String::new();
                let mut addresses: Vec<String> = Vec::new();
                let mut port: u16 = 0;
                let mut txt: HashMap<String, String> = HashMap::new();

                for part in trimmed.split_whitespace() {
                    if part.starts_with("hostname=") {
                        host = part
                            .trim_start_matches("hostname=")
                            .trim_matches('[')
                            .trim_matches(']')
                            .trim_end_matches('.')
                            .to_string();
                    } else if part.starts_with("address=") {
                        let addr = part
                            .trim_start_matches("address=")
                            .trim_matches('[')
                            .trim_matches(']')
                            .to_string();
                        addresses.push(addr);
                    } else if part.starts_with("port=") {
                        port = part
                            .trim_start_matches("port=")
                            .trim_matches('[')
                            .trim_matches(']')
                            .parse()
                            .unwrap_or(0);
                    } else if part.starts_with("txt=") {
                        let txt_str = part
                            .trim_start_matches("txt=")
                            .trim_matches('[')
                            .trim_matches(']');
                        if !txt_str.is_empty() && txt_str != "[]" {
                            for pair in txt_str.split(';') {
                                let pair = pair.trim();
                                if let Some((k, v)) = pair.split_once('=') {
                                    txt.insert(k.to_string(), v.to_string());
                                } else if !pair.is_empty() {
                                    txt.insert(pair.to_string(), String::new());
                                }
                            }
                        }
                    }
                }

                let fullname = format!("{name}.{service_type}.local.");

                found.insert(fullname.clone());
                let _ = tx.send(DiscoveryEvent::Found(FoundPrinter {
                    fullname,
                    service_type,
                    instance: name.clone(),
                    host,
                    port,
                    addresses,
                    txt,
                }));
            }
        }
    }

    Ok(found)
}
