import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";

interface AboutViewProps {
  /** `windows` | `linux` | `macos` — пробрасывается из App. */
  os: string;
}

/** Подпись ОС, симметричная platformLabel() в App.tsx. */
function platformLabel(os: string): string {
  switch (os) {
    case "windows":
      return "Windows";
    case "linux":
      return "Linux / RedOS";
    case "macos":
      return "macOS";
    default:
      return os;
  }
}

export default function AboutView({ os }: AboutViewProps) {
  // Версия берётся из tauri.conf.json, а не хардкодится: раньше здесь было
  // застывшее «0.1.0», расходящееся с реальным номером релиза.
  const [version, setVersion] = useState("…");

  useEffect(() => {
    getVersion().then(setVersion).catch(() => setVersion("—"));
  }, []);

  return (
    <div className="view">
      <div className="view__header">
        <div>
          <h2 className="view__title">О программе</h2>
          <p className="view__sub">Краткая справка по приложению</p>
        </div>
      </div>

      <div className="about-card">
        <p className="about-card__lead">
          <b>Printer Config</b> — это локальное десктоп-приложение для управления
          печатными устройствами на Windows и Linux (Red Hat / RedOS).
        </p>

        <ul className="about-card__list">
          <li>
            <strong>Список принтеров:</strong> чтение и изменение параметров через
            Windows Spooler API либо CUPS (lpstat / lpadmin).
          </li>
          <li>
            <strong>Очередь заданий:</strong> просмотр, отмена, пауза и перезапуск
            заданий на выбранном принтере.
          </li>
          <li>
            <strong>Обнаружение:</strong> автономное сканирование сети по протоколу
            mDNS/DNS-SD для поиска сетевых принтеров.
          </li>
          <li>
            <strong>Настройки:</strong> сохранение состояния приложения в
            %APPDATA%\printer-config (Windows) или ~/.config/printer-config (Linux).
          </li>
        </ul>

        <p className="about-card__meta">
          Версия: {version} · Платформа: {platformLabel(os)} · UI: React 19 + Tauri 2 · Backend: Rust
        </p>
      </div>
    </div>
  );
}
