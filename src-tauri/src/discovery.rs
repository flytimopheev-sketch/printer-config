//! Фоновое обнаружение сетевых принтеров по mDNS/DNS-SD (`_ipp._tcp`, `_pdl-datastream._tcp`).
//! Чистый-Rust стек: крейт `mdns` — без системных зависимостей от Avahi/Bonjour.

use serde::Serialize;
use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use async_std::stream::StreamExt;
use async_std::task;
use mdns::{RecordKind, Response};

/// Событие обнаружения, отправляемое в UI.
#[derive(Serialize, Clone, Debug)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum DiscoveryEvent {
    Found(FoundPrinter),
    Removed { fullname: String },
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FoundPrinter {
    pub fullname: String,
    pub service_type: String,
    pub instance: String,
    pub host: String,
    pub port: u16,
    pub addresses: Vec<String>,
    pub txt: HashMap<String, String>,
}

impl FoundPrinter {
    /// Рекомендуемый адрес веб-интерфейса принтера.
    ///
    /// Фронтенд строит URL своей реализацией (`webUrl` в `lib/api.ts`); метод
    /// сохранён для полноты Rust API и потому помечен `allow(dead_code)`.
    #[allow(dead_code)]
    pub fn web_url(&self) -> Option<String> {
        let addr = self.addresses.first()?;
        let rp = self.txt.get("rp").map(String::as_str).unwrap_or("");
        let path = if rp.is_empty() {
            "/".to_string()
        } else if rp.starts_with('/') {
            rp.to_string()
        } else {
            format!("/{rp}")
        };
        Some(format!("http://{addr}:{}{path}", self.port))
    }
}

/// Состояние mDNS-сканера: команда start/stop + очередь событий для UI.
///
/// Запуск каждого типа сервиса (`_ipp._tcp`, `_pdl-datastream._tcp`,
/// `_printer._tcp`) ведётся в отдельной задаче async-std, которая крутит
/// поток ответов `mdns::discover::all()`. Остановка переводит флаг `running`
/// в `false` — задачи видят его на следующем тайм-ауте потока и выходят.
///
/// `rx` обёрнут в `Mutex`, чтобы `Discovery: Sync` — этого требует `State<T>`
/// в Tauri-командах и `Manager::state` в фоновом потоке.
pub struct Discovery {
    running: Arc<AtomicBool>,
    tx: Sender<DiscoveryEvent>,
    rx: Mutex<Receiver<DiscoveryEvent>>,
    handles: Mutex<Vec<task::JoinHandle<()>>>,
}

impl Discovery {
    pub fn new() -> Self {
        let (tx, rx) = channel();
        Self {
            running: Arc::new(AtomicBool::new(false)),
            tx,
            rx: Mutex::new(rx),
            handles: Mutex::new(Vec::new()),
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Запускает прослушивание `_ipp._tcp`, `_pdl-datastream._tcp`,
    /// `_printer._tcp` через чистый-Rust стек `mdns` (без Avahi).
    pub fn start(&self) -> Result<(), String> {
        if self.running.swap(true, Ordering::SeqCst) {
            return Ok(());
        }

        let tx = self.tx.clone();
        let running = Arc::clone(&self.running);

        let service_types = [
            "_ipp._tcp.local.",
            "_pdl-datastream._tcp.local.",
            "_printer._tcp.local.",
        ];

        for service_type in service_types {
            let tx = tx.clone();
            let running = Arc::clone(&running);
            let st = service_type.to_string();

            let handle = task::spawn(async move {
                match mdns::discover::all(&st, Duration::from_secs(30)) {
                    Ok(discovery) => {
                        let mut stream = Box::pin(discovery.listen());
                        while running.load(Ordering::SeqCst) {
                            match async_std::future::timeout(
                                Duration::from_millis(500),
                                StreamExt::next(&mut stream),
                            )
                            .await
                            {
                                Ok(Some(Ok(response))) => {
                                    if let Some(printer) = parse_response(&response, &st) {
                                        let _ = tx.send(DiscoveryEvent::Found(printer));
                                    }
                                }
                                Ok(Some(Err(e))) => {
                                    log::error!("mDNS discovery error for {st}: {e}");
                                }
                                Ok(None) => break,
                                Err(_) => continue, // тайм-аут — проверяем флаг running
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("Не удалось запустить обнаружение mDNS для {st}: {e}");
                    }
                }
            });

            self.handles.lock().unwrap().push(handle);
        }

        Ok(())
    }

    pub fn stop(&self) -> Result<(), String> {
        self.running.store(false, Ordering::SeqCst);
        // Задачи увидят флаг `running == false` на следующем тайм-ауте
        // потока (через ≤500 мс) и выйдут сами. Хендлы просто сбрасываем.
        self.handles.lock().unwrap().clear();
        Ok(())
    }

    /// Неблокирующий приём событий для вызова из UI-потока.
    pub fn poll(&self) -> Vec<DiscoveryEvent> {
        let rx = self.rx.lock().unwrap();
        let mut out = Vec::new();
        while let Ok(ev) = rx.try_recv() {
            out.push(ev);
        }
        out
    }
}

impl Default for Discovery {
    fn default() -> Self {
        Self::new()
    }
}

/// Разбирает один mDNS-ответ в `FoundPrinter`.
///
/// Ответ содержит набор DNS-записей (PTR, SRV, TXT, A/AAAA). Берём из них
/// имя сервиса, хост, порт, адреса и TXT-поля. Если PTR-записи нет —
/// ответ не является объявлением нужного типа сервиса и пропускается.
fn parse_response(response: &Response, service_type: &str) -> Option<FoundPrinter> {
    let mut fullname: Option<String> = None;
    let mut instance: Option<String> = None;
    let mut host: Option<String> = None;
    let mut port: Option<u16> = None;
    let mut srv_target: Option<String> = None;
    let mut addresses: Vec<String> = Vec::new();
    let mut txt: HashMap<String, String> = HashMap::new();

    let service_type_dot = format!("{}.", service_type.trim_end_matches('.'));

    for record in response.records() {
        match &record.kind {
            RecordKind::PTR(target) => {
                // PTR: имя сервиса (например, _ipp._tcp.local.) -> экземпляр
                // (например, MyPrinter._ipp._tcp.local.)
                if record.name == service_type || record.name == service_type_dot {
                    fullname = Some(target.clone());
                    let name = target.trim_end_matches('.');
                    if let Some(rest) = name.strip_suffix(service_type.trim_end_matches('.')) {
                        instance = Some(rest.trim_end_matches('.').to_string());
                    } else {
                        instance = Some(name.to_string());
                    }
                }
            }
            RecordKind::SRV { port: p, target, .. } => {
                port = Some(*p);
                srv_target = Some(target.clone());
                if host.is_none() {
                    host = Some(target.trim_end_matches('.').to_string());
                }
            }
            RecordKind::TXT(entries) => {
                for entry in entries {
                    if let Some((k, v)) = entry.split_once('=') {
                        txt.insert(k.to_string(), v.to_string());
                    } else {
                        txt.insert(entry.clone(), String::new());
                    }
                }
            }
            RecordKind::A(addr) => {
                addresses.push(addr.to_string());
            }
            RecordKind::AAAA(addr) => {
                addresses.push(addr.to_string());
            }
            _ => {}
        }
    }

    let fullname = fullname?;
    let instance = instance.unwrap_or_else(|| fullname.clone());
    let host = host
        .or_else(|| srv_target.clone().map(|t| t.trim_end_matches('.').to_string()))
        .unwrap_or_default();
    let port = port.unwrap_or(0);

    Some(FoundPrinter {
        fullname,
        service_type: service_type.to_string(),
        instance,
        host,
        port,
        addresses,
        txt,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_url_builds_from_address_and_rp() {
        let mut txt = HashMap::new();
        txt.insert("rp".to_string(), "ipp/print".to_string());
        let p = FoundPrinter {
            fullname: "HP OfficeJet._ipp._tcp.local.".into(),
            service_type: "_ipp._tcp.local.".into(),
            instance: "HP OfficeJet".into(),
            host: "hp.local.".into(),
            port: 631,
            addresses: vec!["192.168.1.50".into()],
            txt,
        };
        assert_eq!(p.web_url().unwrap(), "http://192.168.1.50:631/ipp/print");
    }

    #[test]
    fn web_url_without_rp_points_to_root() {
        let p = FoundPrinter {
            fullname: "x._ipp._tcp.local.".into(),
            service_type: "_ipp._tcp.local.".into(),
            instance: "x".into(),
            host: "x.local.".into(),
            port: 80,
            addresses: vec!["10.0.0.1".into()],
            txt: HashMap::new(),
        };
        assert_eq!(p.web_url().unwrap(), "http://10.0.0.1:80/");
    }

    #[test]
    fn web_url_none_without_address() {
        let p = FoundPrinter {
            fullname: "x._ipp._tcp.local.".into(),
            service_type: "_ipp._tcp.local.".into(),
            instance: "x".into(),
            host: "x.local.".into(),
            port: 80,
            addresses: vec![],
            txt: HashMap::new(),
        };
        assert!(p.web_url().is_none());
    }
}