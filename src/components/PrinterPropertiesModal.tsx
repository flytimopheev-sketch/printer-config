import { useState } from "react";
import type { PrinterProperties } from "../lib/api";

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
}

/** Диалог «Свойства принтера»: местонахождение, описание, общий доступ и базовые
 * опции драйвера (размер бумаги, двусторонняя печать, качество). */
export default function PrinterPropertiesModal({
  properties,
  saving,
  onClose,
  onSave,
}: PrinterPropertiesModalProps) {
  const [location, setLocation] = useState(properties.location);
  const [description, setDescription] = useState(properties.description);
  const [shared, setShared] = useState(properties.shared);
  const [options, setOptions] = useState<Record<string, string>>(() => {
    const init: Record<string, string> = {};
    for (const o of properties.options) init[o.key] = o.value;
    return init;
  });

  const setOption = (key: string, value: string) =>
    setOptions((prev) => ({ ...prev, [key]: value }));

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
        <h2 className="modal__title">Свойства — {properties.name}</h2>

        <div className="field">
          <label className="field__label" htmlFor="prop-location">
            Местонахождение
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

        {properties.options.length > 0 && (
          <div className="modal__options">
            <h3 className="modal__subtitle">Параметры печати</h3>
            {properties.options.map((o) => (
              <div className="field" key={o.key}>
                <label className="field__label" htmlFor={`prop-opt-${o.key}`}>
                  {o.label || o.key}
                </label>
                {o.choices.length > 0 ? (
                  <select
                    id={`prop-opt-${o.key}`}
                    className="input"
                    value={options[o.key] ?? ""}
                    onChange={(e) => setOption(o.key, e.target.value)}
                  >
                    {o.choices.map((c) => (
                      <option key={c.value} value={c.value}>
                        {c.label || c.value}
                      </option>
                    ))}
                    {o.value && !o.choices.some((c) => c.value === o.value) && (
                      <option value={o.value}>{o.value}</option>
                    )}
                  </select>
                ) : (
                  <input
                    id={`prop-opt-${o.key}`}
                    className="input"
                    type="text"
                    value={options[o.key] ?? ""}
                    onChange={(e) => setOption(o.key, e.target.value)}
                  />
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

        <div className="modal__actions">
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
  );
}
