import { type MouseEvent } from "react";
import { type PrinterInfo } from "../lib/api";
import { statusText } from "../App";

interface PrinterCardProps {
  printer: PrinterInfo;
  pending: string | null;
  onSetDefault: (name: string) => void;
  onTogglePaused: (name: string) => void;
  onToggleShared: (name: string) => void;
  onDelete: (name: string) => void;
  onOpenJobs: (name: string) => void;
}

interface InfoLineProps {
  label: string;
  children: React.ReactNode;
}

function InfoLine({ label, children }: InfoLineProps) {
  return (
    <div className="printer-card__info">
      <span className="printer-card__info-label">{label}</span>
      <span className="printer-card__info-value">{children}</span>
    </div>
  );
}

interface ActionButtonProps {
  tone?: "primary" | "danger" | "ghost" | "paused";
  children: React.ReactNode;
  onClick: (e: MouseEvent<HTMLButtonElement>) => void;
}

function ActionButton({ tone = "primary", children, onClick }: ActionButtonProps) {
  return (
    <button
      type="button"
      className={`button button--sm button--${tone}`}
      onClick={onClick}
    >
      {children}
    </button>
  );
}

export default function PrinterCard({
  printer,
  pending,
  onSetDefault,
  onTogglePaused,
  onToggleShared,
  onDelete,
  onOpenJobs,
}: PrinterCardProps) {
  const isDefault = printer.isDefault;
  const isShared = printer.attributeText.includes("Общий доступ");
  const paused = printer.statusText.some((s) => /пауза/.test(s));

  return (
    <article className="printer-card">
      <div className="printer-card__header">
        <div className="printer-card__name-wrap">
          <h3 className="printer-card__name">{printer.name}</h3>
          <p className="printer-card__status">{statusText(printer.statusText)}</p>
        </div>
        <div className="printer-card__badges">
          {isDefault && <span className="badge badge--default">По умолчанию</span>}
          {isShared && <span className="badge badge--shared">Общий доступ</span>}
          {printer.attributeText.includes("Сетевой") && (
            <span className="badge badge--network">Сетевой</span>
          )}
        </div>
      </div>

      <div className="printer-card__meta">
        <InfoLine label="Папка доступа">{printer.shareName || "—"}</InfoLine>
        <InfoLine label="Порт">{printer.port || "—"}</InfoLine>
        <InfoLine label="Драйвер">{printer.driver || "—"}</InfoLine>
        {printer.comment && <InfoLine label="Комментарий">{printer.comment}</InfoLine>}
        {printer.location && <InfoLine label="Местонахождение">{printer.location}</InfoLine>}
      </div>

      {printer.attributeText.length > 0 && (
        <div className="printer-card__chips">
          {printer.attributeText.map((t) => (
            <span key={t} className="chip">
              {t}
            </span>
          ))}
        </div>
      )}

      {pending && <span className="printer-card__hint">Подключается…</span>}

      <div className="printer-card__actions">
        <ActionButton tone={isDefault ? "ghost" : "primary"} onClick={() => onSetDefault(printer.name)}>
          {isDefault ? "✓ Установлено по умолчанию" : "Установить по умолчанию"}
        </ActionButton>
        <ActionButton
          tone={paused ? "paused" : "primary"}
          onClick={() => onTogglePaused(printer.name)}
        >
          {paused ? "Продолжить печать" : "Пауза"}
        </ActionButton>
        <ActionButton
          tone={isShared ? "ghost" : "primary"}
          onClick={() => onToggleShared(printer.name)}
        >
          {isShared ? "Убрать общий доступ" : "Сделать общим"}
        </ActionButton>
        <ActionButton tone="danger" onClick={() => onDelete(printer.name)}>
          Удалить
        </ActionButton>
        <ActionButton onClick={() => onOpenJobs(printer.name)}>Открыть очередь</ActionButton>
      </div>
    </article>
  );
}

