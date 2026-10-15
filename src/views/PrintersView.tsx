import { type ChangeEvent } from "react";
import { type PrinterInfo } from "../lib/api";
import PrinterCard from "../components/PrinterCard";

interface PrintersViewProps {
  printers: PrinterInfo[];
  pendingPrinter: string | null;
  search: string;
  onSearchChange: (value: string) => void;
  onSetDefault: (name: string) => void;
  onTogglePaused: (name: string) => void;
  onToggleShared: (name: string) => void;
  onDelete: (name: string) => void;
  onRefresh: () => void;
  onOpenJobs: (name: string) => void;
  onShowProperties: (name: string) => void;
  onOpenWebConsole: (name: string) => void;
  onAddClick: () => void;
  busy: boolean;
}

export default function PrintersView({
  printers,
  pendingPrinter,
  search,
  onSearchChange,
  onSetDefault,
  onTogglePaused,
  onToggleShared,
  onDelete,
  onRefresh,
  onOpenJobs,
  onShowProperties,
  onOpenWebConsole,
  onAddClick,
  busy,
}: PrintersViewProps) {
  return (
    <div className="view">
      <div className="view__header">
        <div>
          <h2 className="view__title">Принтеры</h2>
          <p className="view__sub">Управление локальными и сетевыми принтерами</p>
        </div>
        <div className="view__actions">
          <button
            type="button"
            className="button"
            onClick={onRefresh}
            disabled={busy}
            title="Обновить список (F5)"
          >
            {busy ? "Загрузка…" : "Обновить"}
          </button>
          <button type="button" className="button button--primary" onClick={onAddClick}>
            + Добавить сетевой принтер
          </button>
        </div>
      </div>

      <div className="field">
        <label htmlFor="search-printers" className="field__label">
          Поиск
        </label>
        <input
          id="search-printers"
          className="input input--search"
          type="search"
          value={search}
          placeholder="Фильтр по имени, драйверу, комментарию…"
          onChange={(e: ChangeEvent<HTMLInputElement>) => onSearchChange(e.target.value)}
        />
      </div>

      {busy && <div className="skeleton">Загрузка принтеров…</div>}

      {!busy && printers.length === 0 && (
        <div className="empty">
          <p className="empty__title">Нет найденных принтеров</p>
          <p className="empty__hint">
            Добавьте сетевой принтер через кнопку выше или проверьте подключение.
          </p>
        </div>
      )}

      <div className="printer-list">
        {printers.map((printer) => (
          <PrinterCard
            key={printer.name}
            printer={printer}
            pending={pendingPrinter === printer.name ? printer.name : null}
            onSetDefault={onSetDefault}
            onTogglePaused={onTogglePaused}
            onToggleShared={onToggleShared}
            onDelete={onDelete}
            onOpenJobs={onOpenJobs}
            onShowProperties={onShowProperties}
            onOpenWebConsole={onOpenWebConsole}
          />
        ))}
      </div>
    </div>
  );
}
