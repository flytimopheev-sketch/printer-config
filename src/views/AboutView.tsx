import { useEffect, useRef, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";

import { api } from "../lib/api";

const REPO_URL = "https://github.com/flytimopheev-sketch/printer-config";

export default function AboutView() {
  const [version, setVersion] = useState<string>("");
  const [diag, setDiag] = useState<string | null>(null);
  const [diagError, setDiagError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const diagRef = useRef<HTMLTextAreaElement | null>(null);

  useEffect(() => {
    getVersion()
      .then(setVersion)
      .catch(() => setVersion(""));
  }, []);

  /** Собирает сводку о системе и принтерах для отчёта об ошибке. */
  const collect = async () => {
    setDiagError(null);
    setCopied(false);
    try {
      setDiag(await api.diagnostics());
    } catch (err) {
      setDiag(null);
      setDiagError(
        typeof err === "string" ? err : "Не удалось собрать диагностику"
      );
    }
  };

  /** Копирование в буфер обмена. Если API буфера недоступен (нет
   *  разрешения или контекста) — выделяем текст, чтобы скопировать вручную. */
  const copy = async () => {
    if (!diag) return;
    try {
      await navigator.clipboard.writeText(diag);
      setCopied(true);
    } catch {
      diagRef.current?.focus();
      diagRef.current?.select();
      setCopied(false);
    }
  };

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

        <div className="about-card__actions">
          <button
            type="button"
            className="button button--sm"
            onClick={() => void collect()}
          >
            Собрать диагностику
          </button>
          <span className="about-card__hint">
            Версия, ОС и список принтеров — приложите их к отчёту об ошибке.
          </span>
        </div>

        {diagError && (
          <p className="about-card__meta about-card__meta--error">
            {diagError}
          </p>
        )}
      </div>

      {diag !== null && (
        <div
          className="modal"
          role="dialog"
          aria-modal="true"
          onClick={(e) => {
            if (e.target === e.currentTarget) setDiag(null);
          }}
        >
          <div className="modal__panel modal__panel--wide">
            <div className="modal__header">
              <h2 className="modal__title">Диагностика</h2>
              <p className="modal__hint">
                Скопируйте текст и приложите его к issue — так проще разобраться
                в проблеме.
              </p>
            </div>

            <div className="modal__body">
              <textarea
                ref={diagRef}
                className="input about-card__diagnostics"
                readOnly
                value={diag}
                spellCheck={false}
                onFocus={(e) => e.currentTarget.select()}
              />
            </div>

            <div className="modal__actions modal__actions--split">
              <span className="about-card__hint">
                {copied ? "Скопировано в буфер обмена" : "Ctrl+C тоже работает"}
              </span>
              <div className="modal__actions-group">
                <button
                  type="button"
                  className="button button--primary"
                  onClick={() => void copy()}
                >
                  Скопировать
                </button>
                <button type="button" className="button" onClick={() => setDiag(null)}>
                  Закрыть
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
