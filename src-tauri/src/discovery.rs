//! Фоновое обнаружение сетевых принтеров по mDNS/DNS-SD (`_ipp._tcp`, `_pdl-datastream._tcp`).

use serde::Serialize;
use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;

use mdns_sd::{ServiceDaemon, ServiceEvent};

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
    daemon: Mutex<Option<ServiceDaemon>>,
    tx: Sender<DiscoveryEvent>,
    rx: Receiver<DiscoveryEvent>,
}

impl Discovery {
    pub fn new() -> Self {
        let (tx, rx) = channel();
        Self {
            daemon: Mutex::new(None),
            tx,
            rx,
        }
    }

    pub fn is_running(&self) -> bool {
        self.daemon.lock().unwrap().is_some()
    }

    /// Запускает прослушивание `_ipp._tcp` и `_pdl-datastream._tcp`.
    pub fn start(&self) -> Result<(), String> {
        let mut guard = self.daemon.lock().unwrap();
        if guard.is_some() {
            return Ok(());
        }
        let daemon = ServiceDaemon::new().map_err(|e| format!("mDNS-демон недоступен: {e}"))?;

        for service_type in ["_ipp._tcp.local.", "_pdl-datastream._tcp.local."] {
            let receiver = daemon
                .browse(service_type)
                .map_err(|e| format!("Не удалось прослушивать {service_type}: {e}"))?;
            let tx = self.tx.clone();
            std::thread::spawn(move || run_listener(receiver, tx));
        }

        *guard = Some(daemon);
        Ok(())
    }

    pub fn stop(&self) -> Result<(), String> {
        let mut guard = self.daemon.lock().unwrap();
        if let Some(daemon) = guard.take() {
            daemon
                .shutdown()
                .map_err(|e| format!("Не удалось остановить mDNS: {e}"))?;
        }
        Ok(())
    }

    /// Неблокирующий приём событий для вызова из UI-потока.
    pub fn poll(&self) -> Vec<DiscoveryEvent> {
        let mut out = Vec::new();
        while let Ok(ev) = self.rx.try_recv() {
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

fn run_listener(receiver: flume::Receiver<ServiceEvent>, tx: Sender<DiscoveryEvent>) {
    while let Ok(event) = receiver.recv() {
        match event {
            ServiceEvent::ServiceResolved(info) => {
                let fullname = info.get_fullname().to_string();
                let service_type = info.get_type().to_string();
                let instance = fullname
                    .strip_suffix(&service_type)
                    .unwrap_or(&fullname)
                    .trim_end_matches('.')
                    .to_string();
                let addresses: Vec<String> = info
                    .get_addresses_v4()
                    .iter()
                    .map(|a| a.to_string())
                    .collect();
                let txt = info
                    .get_properties()
                    .iter()
                    .map(|p| (p.key().to_string(), p.val_str().to_string()))
                    .collect();
                let _ = tx.send(DiscoveryEvent::Found(FoundPrinter {
                    fullname,
                    service_type,
                    instance,
                    host: info.get_hostname().trim_end_matches('.').to_string(),
                    port: info.get_port(),
                    addresses,
                    txt,
                }));
            }
            ServiceEvent::ServiceRemoved(_, fullname) => {
                let _ = tx.send(DiscoveryEvent::Removed { fullname });
            }
            _ => {}
        }
    }
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
