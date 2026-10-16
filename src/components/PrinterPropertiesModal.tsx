import { useState } from "react";
import type { PrinterOption, PrinterProperties } from "../lib/api";

export interface PropertiesForm {
  location: string;
  description: string;
  shared: boolean;
  options: Record<string, string>;
}

interface PrinterPropertiesModalProps {
  properties: PrinterProperties;
  saving: boolean;
  onClose: () => void;
  onSave: (form: PropertiesForm) => void;
  /** Печать тестовой страницы (кнопка в подвале диалога). */
  onPrintTestPage: () => void;
  /** Переключение «Активен» (cupsenable/cupsdisable) — применяется сразу. */
  onSetActive: (active: boolean) => void;
}

/** Человеческие подписи состояния принтера (IPP printer-state). */
const STATUS_LABEL: Record<string, string> = {
  idle: "Готов",
  processing: "Печать",
  stopped: "Остановлен",
  unknown: "Нет данных",
};

/** Подписи уровня расходника (IPP marker status). */
const SUPPLY_LABEL: Record<string, string> = {
  ok: "Норма",
  low: "Заканчивается",
  empty: "Пусто",
  "not-installed": "Не установлен",
  unknown: "Нет данных",
};

/** Одна опция: выпадающий список при известных вариантах, иначе поле ввода. */
function OptionField({
  option,
  value,
  idPrefix,
  onChange,
}: {
  option: PrinterOption;
  value: string;
  idPrefix: string;
  onChange: (v: string) => void;
}) {
  const id = `${idPrefix}-${option.key}`;
  return (
    <div className="field">
      <label className="field__label" htmlFor={id}>
        {option.label || option.key}
      </label>
      {option.choices.length > 0 ? (
        <select id={id} className="input" value={value} onChange={(e) => onChange(e.target.value)}>
          {option.choices.map((c) => (
            <option key={c.value} value={c.value}>
              {c.label || c.value}
            </option>
          ))}
          {value && !option.choices.some((c) => c.value === value) && (
            <option value={value}>{value}</option>
          )}
        </select>
      ) : (
        <input
          id={id}
          className="input"
          type="text"
          value={value}
          onChange={(e) => onChange(e.target.value)}
        />
      )}
    </div>
  );
}

/** Диалог «Свойства принтера» (аналог system-config-printer). Шапка с
 * состоянием и подвал с кнопками закреплены — прокручивается только тело. */
export default function PrinterPropertiesModal({
  properties,
  saving,
  onClose,
  onSave,
  onPrintTestPage,
  onSetActive,
}: PrinterPropertiesModalProps) {
  const [location, setLocation] = useState(properties.location);
  const [description, setDescription] = useState(properties.description);
  const [shared, setShared] = useState(properties.shared);
  const [active, setActive] = useState(properties.enabled);
  const [options, setOptions] = useState<Record<string, string>>(() => {
    const init: Record<string, string> = {};
    for (const o of properties.jobOptions) init[o.key] = o.value;
    for (const o of properties.printerOptions) init[o.key] = o.value;
    return init;
  });

  const setOption = (key: string, value: string) =>
    setOptions((prev) => ({ ...prev, [key]: value }));

  const statusLabel = STATUS_LABEL[properties.status] ?? properties.status;

  return (
    <div
      className="modal"
      role="dialog"
      aria-modal="true"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="modal__panel modal__panel--wide">
        {/* Шапка — закреплена, состояние видно всегда */}
        <div className="modal__header">
          <h2 className="modal__title">Свойства — {properties.name}</h2>
          <div className="prop-status">
            <span className={`prop-status__dot prop-status__dot--${properties.status}`} />
            <span className="prop-status__label">
              Состояние принтера: <strong>{statusLabel}</strong>
            </span>
            {!active && <span className="badge badge--stopped">Неактивен</span>}
            {properties.isDefault && <span className="badge badge--default">По умолчанию</span>}
          </div>
          {properties.statusReasons.length > 0 && (
            <p className="modal__hint">{properties.statusReasons.join(", ")}</p>
          )}
        </div>

        <div className="modal__body">
          <div className="field">
            <span className="field__label">Марка и модель</span>
            <div className="prop-static">{properties.makeModel || "—"}</div>
          </div>

          <div className="field">
            <label className="field__label" htmlFor="prop-location">
              Размещение
            </label>
            <input
              id="prop-location"
              className="input"
              type="text"
              value={location}
              placeholder="Кабинет 101"
              onChange={(e) => setLocation(e.target.value)}
            />
          </div>

          <div className="field">
            <label className="field__label" htmlFor="prop-description">
              Описание
            </label>
            <input
              id="prop-description"
              className="input"
              type="text"
              value={description}
              placeholder="Принтер в бухгалтерии"
              onChange={(e) => setDescription(e.target.value)}
            />
          </div>

          <label className="checkbox">
            <input
              type="checkbox"
              checked={shared}
              onChange={(e) => setShared(e.target.checked)}
            />
            <span>Общий доступ к принтеру по сети</span>
          </label>

          <label className="checkbox">
            <input
              type="checkbox"
              checked={active}
              onChange={(e) => {
                setActive(e.target.checked);
                onSetActive(e.target.checked);
              }}
            />
            <span>Активен (принтер принимает задания)</span>
          </label>

          {/* Параметры задания */}
          {properties.jobOptions.length > 0 && (
            <div className="modal__options">
              <h3 className="modal__subtitle">Параметры задания</h3>
              {properties.jobOptions.map((o) => (
                <OptionField
                  key={o.key}
                  option={o}
                  idPrefix="job-opt"
                  value={options[o.key] ?? ""}
                  onChange={(v) => setOption(o.key, v)}
                />
              ))}
            </div>
          )}

          {/* Все параметры принтера (драйвер/PPD) */}
          {properties.printerOptions.length > 0 && (
            <div className="modal__options">
              <h3 className="modal__subtitle">
                Параметры принтера ({properties.printerOptions.length})
              </h3>
              {properties.printerOptions.map((o) => (
                <OptionField
                  key={o.key}
                  option={o}
                  idPrefix="prop-opt"
                  value={options[o.key] ?? ""}
                  onChange={(v) => setOption(o.key, v)}
                />
              ))}
            </div>
          )}

          {/* Уровень чернил/тонера */}
          {properties.supplyLevels.length > 0 && (
            <div className="modal__options">
              <h3 className="modal__subtitle">Уровень чернил/тонера</h3>
              {properties.supplyLevels.map((s, i) => (
                <div className="supply" key={`${s.name}-${i}`}>
                  <div className="supply__head">
                    <span className="supply__name">{s.name}</span>
                    <span className={`supply__status supply__status--${s.status}`}>
                      {s.level != null ? `${s.level} % · ` : ""}
                      {SUPPLY_LABEL[s.status] ?? s.status}
                    </span>
                  </div>
                  {s.level != null && (
                    <div className="supply__track">
                      <div
                        className={`supply__fill supply__fill--${s.status}`}
                        style={{ width: `${Math.max(0, Math.min(100, s.level))}%` }}
                      />
                    </div>
                  )}
                </div>
              ))}
            </div>
          )}

          {(properties.driver || properties.uri) && (
            <div className="modal__hint">
              {properties.driver && (
                <div>
                  Драйвер: <code>{properties.driver}</code>
                </div>
              )}
              {properties.uri && (
                <div>
                  Адрес: <code>{properties.uri}</code>
                </div>
              )}
            </div>
          )}
        </div>

        {/* Подвал — закреплён: пробная страница и кнопки всегда видны */}
        <div className="modal__actions modal__actions--split">
          <button type="button" className="button" onClick={onPrintTestPage}>
            Печать пробной страницы
          </button>
          <div className="modal__actions-group">
            <button
              type="button"
              className="button button--primary"
              disabled={saving}
              onClick={() => onSave({ location, description, shared, options })}
            >
              {saving ? "Сохранение…" : "Сохранить"}
            </button>
            <button type="button" className="button" onClick={onClose}>
              Отмена
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
