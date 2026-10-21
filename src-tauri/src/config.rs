//! Сохранение состояния приложения в `%APPDATA%\printer-config\config.json`.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct AppConfig {
    /// Активная вкладка: `printers` | `jobs` | `discovery` | `about`.
    pub tab: String,
    /// Последний выбранный принтер.
    pub selected_printer: Option<String>,
    /// Строка поиска во вкладке принтеров.
    pub search: String,
    /// Последний принтер, для которого показывалась очередь заданий.
    pub jobs_printer: Option<String>,
    /// Скан обнаружения запускать автоматически при старте.
    pub auto_discovery: bool,
    pub window_width: f64,
    pub window_height: f64,
    pub maximized: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            tab: "printers".into(),
            selected_printer: None,
            search: String::new(),
            jobs_printer: None,
            auto_discovery: false,
            window_width: 920.0,
            window_height: 620.0,
            maximized: false,
        }
    }
}

/// Базовый каталог для настроек: `%APPDATA%` (Windows) либо
/// `$XDG_CONFIG_HOME`/`~/.config` (Linux/RedOS).
fn config_base() -> Result<String, String> {
    #[cfg(windows)]
    {
        std::env::var("APPDATA").map_err(|_| "Не удалось определить каталог %APPDATA%".to_string())
    }
    #[cfg(unix)]
    {
        if let Ok(x) = std::env::var("XDG_CONFIG_HOME") {
            Ok(x)
        } else {
            let home =
                std::env::var("HOME").map_err(|_| "Не удалось определить $HOME".to_string())?;
            Ok(format!("{home}/.config"))
        }
    }
}

pub fn config_path() -> Result<PathBuf, String> {
    let base = config_base()?;
    let dir = PathBuf::from(base).join("printer-config");
    fs::create_dir_all(&dir).map_err(|e| format!("Не удалось создать каталог настроек: {e}"))?;
    Ok(dir.join("config.json"))
}

pub fn load() -> AppConfig {
    let Ok(path) = config_path() else {
        return AppConfig::default();
    };
    let Ok(data) = fs::read_to_string(path) else {
        return AppConfig::default();
    };
    serde_json::from_str(&data).unwrap_or_default()
}

pub fn save(config: &AppConfig) -> Result<(), String> {
    let path = config_path()?;
    let data = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Не удалось сериализовать настройки: {e}"))?;
    fs::write(&path, data).map_err(|e| format!("Не удалось записать {path:?}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_tab_is_printers() {
        assert_eq!(AppConfig::default().tab, "printers");
    }

    #[test]
    fn roundtrip_json() {
        let cfg = AppConfig {
            tab: "jobs".into(),
            selected_printer: Some("HP LaserJet".into()),
            search: "hp".into(),
            ..AppConfig::default()
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let back: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back, cfg);
    }

    #[test]
    fn missing_fields_use_defaults() {
        // Совместимость со старыми версиями конфига: новые поля берутся из Default.
        let cfg: AppConfig = serde_json::from_str(r#"{"tab":"discovery"}"#).unwrap();
        assert_eq!(cfg.tab, "discovery");
        assert_eq!(cfg.window_width, 920.0);
        assert!(!cfg.auto_discovery);
    }

    #[test]
    fn malformed_json_gives_default() {
        let parsed: AppConfig = serde_json::from_str("{oops").unwrap_or_default();
        assert_eq!(parsed, AppConfig::default());
    }
}
