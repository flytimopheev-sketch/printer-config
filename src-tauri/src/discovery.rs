//! Фоновое обнаружение сетевых принтеров по mDNS/DNS-SD через `avahi-browse`.
//! Надёжно работает на Linux (RED OS / RHEL). Требует пакета `avahi` в системе.

use serde::Serialize;
use std::collections::HashMap;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::time::Duration;

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

    pub fn start(&self) -> Result<(), String> {
        if self.running.swap(true, Ordering::SeqCst) {
            return Ok(());
        }

        let tx = self.tx.clone();
        let running = Arc::clone(&self.running);

        let handle = thread::spawn(move || {
            while running.load(Ordering::SeqCst) {
                if let Err(e) = scan_once(&tx) {
                    log::warn!("mDNS scan error: {e}");
                }
                thread::sleep(Duration::from_secs(5));
            }
        });

        self.handles.lock().unwrap().push(handle);
        Ok(())
    }

    pub fn stop(&self) -> Result<(), String> {
        self.running.store(false, Ordering::SeqCst);
        self.handles.lock().unwrap().clear();
        Ok(())
    }

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

/// Один проход сканирования через avahi-browse.
/// Формат вывода --parsable:
///   =eth0IPv4HP OfficeJet_ipp._tcp.local-
///   hostname = [hp.local] address = [192.168.1.50] port = [631] txt = []
fn scan_once(tx: &Sender<DiscoveryEvent>) -> Result<(), String> {
    let output = Command::new("avahi-browse")
        .args([
            "--parsable",
            "--terminate",
            "-t",
            "_ipp._tcp",
            "-t",
            "_pdl-datastream._tcp",
            "-t",
            "_printer._tcp",
        ])
        .output()
        .map_err(|e| format!("avahi-browse не найден (установите пакет avahi): {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut current_name: Option<String> = None;
    let mut current_type: Option<String> = None;

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

        // Строка разрешения: \thostname = [...] address = [...] port = [...] txt = [...]
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
                        let txt_str = part.trim_start_matches("txt=").trim_matches('[').trim_matches(']');
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

                let fullname = format!("{name}.{service_type}.local.", name = name, service_type = service_type);

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

    Ok(())
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