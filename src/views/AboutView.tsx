import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";

import { api } from "../lib/api";

const REPO_URL = "https://github.com/flytimopheev-sketch/printer-config";

export default function AboutView() {
  const [version, setVersion] = useState<string>("");

  useEffect(() => {
    getVersion()
      .then(setVersion)
      .catch(() => setVersion(""));
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
          <b>Printer Config</b> — локальное десктоп-приложение для управления
          сетевыми и локальными принтерами (CUPS/IPP, Windows Spooler).
        </p>

        <ul className="about-card__list">
          <li>
            <strong>Управление очередями CUPS:</strong> добавление, удаление,
            включение/выключение, настройка через lpadmin / lpoptions.
          </li>
          <li>
            <strong>Параметры принтера:</strong> состояние, уровень тонера/чернил,
            тестовая страница, опции PPD и опции заданий по умолчанию.
          </li>
          <li>
            <strong>Очередь заданий:</strong> просмотр, отмена, пауза и
            перезапуск заданий на выбранном принтере.
          </li>
          <li>
            <strong>Обнаружение:</strong> сканирование сети по mDNS/DNS-SD для
            поиска сетевых принтеров.
          </li>
        </ul>

        <p className="about-card__meta">
          {version && <>Версия {version} · </>}Лицензия: MIT · Связь:
          <a href="mailto:flytimopheev@gmail.com" className="about-card__email">
            flytimopheev@gmail.com
          </a>
        </p>

        <p className="about-card__meta">
          <a
            href={REPO_URL}
            className="about-card__email"
            onClick={(e) => {
              e.preventDefault();
              api.openUrl(REPO_URL).catch(() => undefined);
            }}
          >
            Исходный код и релизы: {REPO_URL}
          </a>
        </p>
      </div>
    </div>
  );
}
