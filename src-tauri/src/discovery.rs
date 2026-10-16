//! Фоновое обнаружение сетевых принтеров по mDNS/DNS-SD через `avahi-browse`.
//! Надёжно работает на Linux (RED OS / RHEL) без дополнительных Rust-крейтов mDNS.

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
    /// Рекомендуемый адрес веб-интерфейса принтера.
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
/// Запускает `avahi-browse --parsable --terminate` в отдельном потоке с
/// периодом 5 секунд. Каждый найденный сервис отправляется в UI как `Found`.
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
                let _ = scan_once(&tx);
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

/// Один проход сканирования: запускает avahi-browse и шлёт найденные принтеры в tx.
fn scan_once(tx: &std::sync::mpsc::Sender<DiscoveryEvent>) -> Result<(), String> {
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
    let mut services: HashMap<String, FoundPrinter> = HashMap::new();

    for line in stdout.lines() {
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.is_empty() {
            continue;
        }

        // Строка службы: interfaceprotocolnametypedomain-
        if fields.len() >= 5 && !line.starts_with('\t') {
            let _iface = fields[0];
            let _proto = fields[1];
            let name = fields[2].to_string();
            let service_type = fields[3].to_string();

            // Пропускаем не-принтеры (вse же мы фильтруем по -t, но на всякий случай)
            if !service_type.contains("_tcp") {
                continue;
            }

            let fullname = format!("{name}.{service_type}.local.");
            let instance = name.clone();

            services.insert(
                fullname.clone(),
                FoundPrinter {
                    fullname,
                    service_type,
                    instance,
                    host: String::new(),
                    port: 0,
                    addresses: Vec::new(),
                    txt: HashMap::new(),
                },
            );
        }
        // Строка разрешения: \t hostname=... address=... port=... txt=...
        else if line.starts_with('\t') && !services.is_empty() {
            // Берём последнюю добавленную службу
            let last_key = services.keys().last().cloned();
            if let Some(key) = last_key {
                if let Some(printer) = services.get_mut(&key) {
                    for field in line.split('\t').skip(1) {
                        if field.is_empty() {
                            continue;
                        }
                        if let Some((k, v)) = field.split_once('=') {
                            match k {
                                "hostname" => printer.host = v.trim_end_matches('.').to_string(),
                                "address" => {
                                    printer.addresses = vec![v.to_string()];
                                }
                                "port" => printer.port = v.parse().unwrap_or(0),
                                "txt" => {
                                    for pair in v.split(';') {
                                        if let Some((tk, tv)) = pair.split_once('=') {
                                            printer.txt.insert(tk.to_string(), tv.to_string());
                                        } else {
                                            printer.txt.insert(pair.to_string(), String::new());
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    // Отправляем событие о найденном принтере
                    let _ = tx.send(DiscoveryEvent::Found(printer.clone()));
                }
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