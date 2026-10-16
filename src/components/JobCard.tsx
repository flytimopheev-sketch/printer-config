import { type MouseEvent } from "react";
import { type JobInfo } from "../lib/api";
import { statusText } from "../App";

interface JobCardProps {
  job: JobInfo;
  onAction: (printer: string, jobId: number, action: string) => void;
}

interface InfoLineProps {
  label: string;
  children: React.ReactNode;
}

function InfoLine({ label, children }: InfoLineProps) {
  return (
    <>
      <span className="job-card__info-label">{label}</span>
      <span className="job-card__info-value">{children}</span>
    </>
  );
}

interface ActionButtonProps {
  tone?: "primary" | "danger" | "ghost";
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

export default function JobCard({ job, onAction }: JobCardProps) {
  const total = job.totalPages;
  const printed = job.pagesPrinted;

  return (
    <article className="job-card">
      <div className="job-card__head">
        <div className="job-card__doc">{job.document || "Безымянное задание"}</div>
        <span className="job-card__id">#{job.id}</span>
      </div>

      <div className="job-card__meta">
        <InfoLine label="Принтер">{job.printer}</InfoLine>
        <InfoLine label="Пользователь">{job.user}</InfoLine>
        <InfoLine label="Тип">{job.datatype}</InfoLine>
        <InfoLine label="Дата">{job.submitted}</InfoLine>
      </div>

      <div className="job-card__status">{statusText(job.statusText)}</div>

      <div className="job-card__pages">
        <span className="job-card__pages-total">Всего: {total || "—"}</span>
        <span className="job-card__pages-printed">Напечатано: {printed || "—"}</span>
      </div>

      <div className="job-card__actions">
        {job.status !== 0 && (
          <>
            <ActionButton tone="danger" onClick={() => onAction(job.printer, job.id, "cancel")}>
              Отменить
            </ActionButton>
            <ActionButton
              tone={job.status === 1 ? "ghost" : "primary"}
              onClick={() => onAction(job.printer, job.id, "pause")}
            >
              {job.status === 1 ? "Продолжить" : "Пауза"}
            </ActionButton>
          </>
        )}
        <ActionButton tone="ghost" onClick={() => onAction(job.printer, job.id, "restart")}>
          Перезапустить
        </ActionButton>
      </div>
    </article>
  );
}
