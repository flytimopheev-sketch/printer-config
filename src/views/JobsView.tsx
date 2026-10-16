import { type ChangeEvent } from "react";
import { type JobInfo, type PrinterInfo } from "../lib/api";
import JobCard from "../components/JobCard";

type Tab = "printers" | "jobs" | "discovery" | "about";

interface JobsViewProps {
  printers: PrinterInfo[];
  jobs: JobInfo[];
  busy: boolean;
  selectedPrinter: string | null;
  onSelectPrinter: (name: string) => void;
  onJobAction: (printer: string, jobId: number, action: string) => void;
  onRefresh: () => void;
  returnTab: Tab;
  onBack: () => void;
}

export default function JobsView({
  printers,
  jobs,
  busy,
  selectedPrinter,
  onSelectPrinter,
  onJobAction,
  onRefresh,
  onBack,
}: JobsViewProps) {
  return (
    <div className="view">
      <div className="view__header">
        <div className="view__header-left">
          <button
            type="button"
            className="button button--ghost button--back"
            onClick={onBack}
          >
            ← Назад
          </button>
          <div>
            <h2 className="view__title">Задания</h2>
            <p className="view__sub">Управление очередями печати</p>
          </div>
        </div>

        <div className="view__actions">
          <select
            className="select"
            value={selectedPrinter ?? ""}
            onChange={(e: ChangeEvent<HTMLSelectElement>) => {
              const value = e.target.value;
              if (value) onSelectPrinter(value);
            }}
          >
            <option value="">Любой принтер…</option>
            {printers.map((printer) => (
              <option key={printer.name} value={printer.name}>
                {printer.name}
              </option>
            ))}
          </select>
          <button
            type="button"
            className="button"
            onClick={onRefresh}
            disabled={busy}
          >
            {busy ? "Загрузка…" : "Обновить"}
          </button>
        </div>
      </div>

      {!busy && jobs.length === 0 && (
        <div className="empty">
          <p className="empty__title">Нет заданий</p>
          <p className="empty__hint">
            {selectedPrinter
              ? `Задания для «${selectedPrinter}» отсутствуют.`
              : "Выберите принтер в списке, чтобы просмотреть его очередь."}
          </p>
        </div>
      )}

      <div className="job-list">
        {jobs.map((job) => (
          <JobCard key={job.id} job={job} onAction={onJobAction} />
        ))}
      </div>
    </div>
  );
}
