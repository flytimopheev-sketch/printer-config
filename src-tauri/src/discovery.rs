//! Фоновое обнаружение сетевых принтеров по mDNS/DNS-SD.
//!
//! Платформенные реализации вынесены в отдельные модули:
//!   * Linux / РЕД ОС — `avahi-browse --parsable --terminate` (пакет `avahi`);
//!   * Windows — чистый Rust-крейт [`mdns`], без системных зависимостей.
//!
//! Обе реализации отдают одинаковые события в очередь, которую разбирает
//! фоновой цикл в `lib.rs` и отправляет в окно как `discovery://found` и
//! `discovery://removed`.

use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

#[cfg(unix)]
mod avahi;
#[cfg(windows)]
mod mdns_browser;

/// Событие обнаружения, отправляемое в UI.
#[derive(Clone, Debug)]
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
    /// Рекомендуемый веб-адрес найденного принтера (аналог `rp` из TXT).
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
pub struct Discovery {
    running: Arc<AtomicBool>,
    tx: Sender<DiscoveryEvent>,
    rx: Mutex<Receiver<DiscoveryEvent>>,
    handles: Mutex<Vec<thread::JoinHandle<()>>>,
}

impl Discovery {
    pub fn new() -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
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

    /// Запускает фоновой сканер. Повторный вызов, пока сканер уже работает,
    /// ничем не мешает.
    pub fn start(&self) -> Result<(), String> {
        if self.running.swap(true, Ordering::SeqCst) {
            return Ok(());
        }

        let tx = self.tx.clone();
        let running = Arc::clone(&self.running);

        let handle = thread::spawn(move || {
            #[cfg(unix)]
            let result = avahi::run(tx, running);
            #[cfg(windows)]
            let result = mdns_browser::run(tx, running);
            if let Err(e) = result {
                log::warn!("discovery: {e}");
            }
        });

        self.handles.lock().unwrap().push(handle);
        Ok(())
    }

    /// Останавливает сканер. Поток завершается сам после текущего прохода
    /// (не дольше нескольких секунд), поэтому `stop` не блокирует UI.
    pub fn stop(&self) -> Result<(), String> {
        self.running.store(false, Ordering::SeqCst);
        self.handles.lock().unwrap().clear();
        Ok(())
    }

    /// Забирает накопленные события (вызывается фоновым циклом в `lib.rs`).
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

#[cfg(test)]
mod tests {
    use super::*;

    fn printer(fullname: &str) -> FoundPrinter {
        FoundPrinter {
            fullname: fullname.into(),
            service_type: "_ipp._tcp.local.".into(),
            instance: "HP OfficeJet".into(),
            host: "hp.local.".into(),
            port: 631,
            addresses: vec!["192.168.1.50".into()],
            txt: HashMap::new(),
        }
    }

    #[test]
    fn web_url_builds_from_address_and_rp() {
        let mut p = printer("HP OfficeJet._ipp._tcp.local.");
        p.txt.insert("rp".to_string(), "ipp/print".to_string());
        assert_eq!(p.web_url().unwrap(), "http://192.168.1.50:631/ipp/print");
    }

    #[test]
    fn web_url_none_without_address() {
        let mut p = printer("x._ipp._tcp.local.");
        p.addresses.clear();
        assert!(p.web_url().is_none());
    }
}
