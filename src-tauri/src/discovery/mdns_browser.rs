//! Обнаружение принтеров по mDNS/DNS-SD на Windows.
//!
//! Используется чистый Rust-крейт [`mdns`] — он не требует установки
//! Bonjour/Avahi в системе и работает на Windows «из коробки».
//!
//! Из ответа mDNS собираются три группы записей:
//!   * `PTR` — имя типа сервиса → полное имя экземпляра;
//!   * `SRV` — порт и целевой хост экземпляра;
//!   * `TXT`/`A`/`AAAA` — дополнительные параметры и адреса.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use mdns::{RecordKind, Response};

use super::{DiscoveryEvent, FoundPrinter};

/// Типы сервисов, которые ищутся: IPP, «сырой» порт и старый протокол JetDirect.
const SERVICE_TYPES: [&str; 3] = [
    "_ipp._tcp.local",
    "_pdl-datastream._tcp.local",
    "_printer._tcp.local",
];

/// Как часто повторять mDNS-запрос.
const QUERY_INTERVAL: Duration = Duration::from_secs(5);

/// Таймаут ожидания ответа: нужен, чтобы заметить команду «Остановить».
const POLL_TIMEOUT: Duration = Duration::from_millis(500);

/// Через сколько считать принтер исчезнувшим, если его больше нет в ответах.
const STALE_TIMEOUT: Duration = Duration::from_secs(60);

/// Основной цикл сканера: держим открытыми браузеры всех типов сервисов и
/// разбираем приходящие ответы, пока установлен флаг `running`.
pub(super) fn run(tx: Sender<DiscoveryEvent>, running: Arc<AtomicBool>) -> Result<(), String> {
    // Браузеры создаём до входа в async-блок: `mdns::discover::all` открывает
    // сокет и может вернуть ошибку, а `?` нельзя использовать внутри блока,
    // который ничего не возвращает.
    let mut streams = Vec::new();
    for service in SERVICE_TYPES {
        let discovery = mdns::discover::all(service, QUERY_INTERVAL)
            .map_err(|e| format!("Не удалось запустить mDNS-браузер: {e}"))?;
        streams.push(Box::pin(discovery.listen()));
    }

    // Кэш последнегоSeen: имя принтера -> когда его видели в последний раз.
    let mut seen: HashMap<String, Instant> = HashMap::new();

    async_std::task::block_on(async {
        loop {
            if !running.load(Ordering::SeqCst) {
                break;
            }

            for stream in streams.iter_mut() {
                match async_std::future::timeout(POLL_TIMEOUT, stream.next()).await {
                    Ok(Some(Ok(response))) => publish(&response, &tx, &mut seen),
                    Ok(Some(Err(e))) => log::warn!("mDNS: {e}"),
                    // Поток оборвался (ошибка сокета) — логируем и ждём
                    // следующего ответа по остальным типам сервисов.
                    Ok(None) => log::warn!("mDNS: поток браузера прерван"),
                    // Таймаут — просто ждём следующего ответа.
                    Err(_) => {}
                }
            }

            // Принтеры, которые перестали отвечать, убираем из списка.
            let stale: Vec<String> = seen
                .iter()
                .filter(|(_, at)| at.elapsed() > STALE_TIMEOUT)
                .map(|(name, _)| name.clone())
                .collect();
            for fullname in stale {
                seen.remove(&fullname);
                let _ = tx.send(DiscoveryEvent::Removed { fullname });
            }
        }
    });

    Ok(())
}

/// Разбирает один mDNS-ответ и отправляет найденные принтеры в UI.
fn publish(response: &Response, tx: &Sender<DiscoveryEvent>, seen: &mut HashMap<String, Instant>) {
    // Экземпляры: (тип сервиса, полное имя экземпляра).
    let mut instances: Vec<(String, String)> = Vec::new();
    // SRV: полное имя экземпляра -> (порт, хост).
    let mut srv: HashMap<String, (u16, String)> = HashMap::new();
    // TXT: полное имя экземпляра -> параметры.
    let mut txt: HashMap<String, HashMap<String, String>> = HashMap::new();
    // A/AAAA: имя хоста -> адреса.
    let mut addrs: HashMap<String, Vec<String>> = HashMap::new();

    for record in response.records() {
        let name = normalize(&record.name);
        match &record.kind {
            RecordKind::PTR(target) => {
                instances.push((name, normalize(target)));
            }
            RecordKind::SRV { port, target, .. } => {
                srv.insert(name, (*port, normalize(target)));
            }
            RecordKind::TXT(entries) => {
                let params = txt.entry(name).or_default();
                for entry in entries {
                    let (key, value) = entry.split_once('=').unwrap_or((entry.as_str(), ""));
                    params.insert(key.to_string(), value.to_string());
                }
            }
            RecordKind::A(ip) => {
                addrs.entry(name).or_default().push(ip.to_string());
            }
            RecordKind::AAAA(ip) => {
                addrs.entry(name).or_default().push(ip.to_string());
            }
            _ => {}
        }
    }

    for (service, fullname) in instances {
        // Без SRV-записи экземпляр бесполезен: неизвестны ни порт, ни хост.
        let Some((port, host)) = srv.get(&fullname).cloned() else {
            continue;
        };

        let mut addresses = addrs.get(&host).cloned().unwrap_or_default();
        if addresses.is_empty() {
            // Хост мог не ответить A-записью (CNAME, кэш) — берём любые адреса.
            addresses = addrs.values().flatten().cloned().collect();
        }

        let instance = short_instance(&fullname, &service);
        seen.insert(fullname.clone(), Instant::now());

        let _ = tx.send(DiscoveryEvent::Found(FoundPrinter {
            fullname: fullname.clone(),
            service_type: service,
            instance,
            host,
            port,
            addresses,
            txt: txt.get(&fullname).cloned().unwrap_or_default(),
        }));
    }
}

/// Убирает завершающую точку: `hp.local.` -> `hp.local`.
fn normalize(name: &str) -> String {
    name.trim_end_matches('.').to_string()
}

/// Короткое имя экземпляра: `HP OfficeJet._ipp._tcp.local` -> `HP OfficeJet`.
fn short_instance(fullname: &str, service: &str) -> String {
    let fullname = normalize(fullname);
    let service = normalize(service);
    match fullname.strip_suffix(&format!(".{service}")) {
        Some(short) => short.to_string(),
        None => fullname,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_trailing_dot() {
        assert_eq!(normalize("hp.local."), "hp.local");
    }

    #[test]
    fn short_instance_strips_service_suffix() {
        assert_eq!(
            short_instance("HP OfficeJet._ipp._tcp.local.", "_ipp._tcp.local."),
            "HP OfficeJet"
        );
    }

    #[test]
    fn short_instance_keeps_unexpected_names() {
        assert_eq!(
            short_instance("HP._usb._tcp.local.", "_ipp._tcp.local."),
            "HP"
        );
    }
}
