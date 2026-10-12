import { type MouseEvent } from "react";
import { api, type FoundPrinter } from "../lib/api";

interface DiscoveryViewProps {
  discovered: FoundPrinter[];
  running: boolean;
  onStart: () => void;
  onStop: () => void;
  onBack: () => void;
  onAddDiscovered: (host: string, instance: string) => void;
}

interface InfoLineProps {
  label: string;
  children: React.ReactNode;
}

function InfoLine({ label, children }: InfoLineProps) {
  return (
    <div className="discovery-card__info">
      <span className="discovery-card__info-label">{label}</span>
      <span className="discovery-card__info-value">{children}</span>
    </div>
  );
}

interface ActionButtonProps {
  children: React.ReactNode;
  onClick: (e: MouseEvent<HTMLButtonElement>) => void;
}

function ActionButton({ children, onClick }: ActionButtonProps) {
  return (
    <button type="button" className="button button--sm" onClick={onClick}>
      {children}
    </button>
  );
}

/** Встроенный адрес, если none */
function buildUrl(p: FoundPrinter): string | null {
  if (!p.addresses[0]) return null;
  const raw = p.txt["rp"] ?? "";
  const path = raw === "" ? "/" : raw.startsWith("/") ? raw : `/${raw}`;
  return `http://${p.addresses[0]}:${p.port}${path}`;
}

export default function DiscoveryView({
  discovered,
  running,
  onStart,
  onStop,
  onBack,
  onAddDiscovered,
}: DiscoveryViewProps) {
  return (
    <div className="view">
      <div className="view__header">
        <div className="view__header-left">
          <button type="button" className="button button--ghost button--back" onClick={onBack}>
            ← Назад
          </button>
          <div>
            <h2 className="view__title">Обнаружение</h2>
            <p className="view__sub">Многоадресные принтеры по mDNS/DNS-SD</p>
          </div>
        </div>

        <button
          type="button"
          className={`button button--primary button--big${running ? " button--pulse" : ""}`}
          onClick={running ? onStop : onStart}
        >
          {running ? "Остановить" : "Запустить"}
        </button>
      </div>

      <p className="view__hint">
        {running
          ? "Идёт сканирование сети. Новые принтеры появятся в списке."
          : "Нажмите «Запустить», чтобы начать сканирование сети."}{" "}
        Сканирование ищет только принтеры, объявляющие себя по mDNS/DNS-SD.
        Если вы знаете IP-адрес принтера — добавьте его напрямую: раздел
        «Принтеры» → «Добавить».
      </p>

      {running && (
        <div className="discovery-card discovery-card--empty">
          <span className="discovery-card__dot" />
          <span className="discovery-card__text">Поиск устройств…</span>
        </div>
      )}

      {!running && discovered.length === 0 && (
        <div className="discovery-card discovery-card--empty">
          <span className="discovery-card__dot" />
          <span className="discovery-card__text">Результаты отсутствуют</span>
        </div>
      )}

      <div className="discovery-list">
        {discovered.map((printer) => {
          const url = buildUrl(printer);
          return (
            <article key={printer.fullname} className="discovery-card">
              <div className="discovery-card__head">
                <h3 className="discovery-card__name">{printer.instance}</h3>
                <span className="discovery-card__fullname">{printer.fullname}</span>
              </div>

              <div className="discovery-card__meta">
                <InfoLine label="Хост">{printer.host}</InfoLine>
                <InfoLine label="Порт">{printer.port}</InfoLine>
                <InfoLine label="Тип сервиса">{printer.serviceType}</InfoLine>
              </div>

              {printer.addresses.length > 0 && (
                <div className="discovery-card__addresses">
                  {printer.addresses.map((addr) => (
                    <span key={addr} className="chip">
                      {addr}
                    </span>
                  ))}
                </div>
              )}

              {url && (
                <div className="discovery-card__url">
                  <a
                    className="button button--sm button--primary"
                    href={url}
                    onClick={(e) => {
                      e.preventDefault();
                      api.openUrl(url);
                    }}
                  >
                    Открыть веб-интерфейс
                  </a>
                </div>
              )}

              {printer.txt["rp"] && (
                <div className="discovery-card__rp">
                  <span className="chip">rp: {printer.txt["rp"]}</span>
                </div>
              )}

              <div className="discovery-card__actions">
                <ActionButton
                  onClick={() => onAddDiscovered(printer.host, printer.instance)}
                >
                  Добавить
                </ActionButton>
              </div>
            </article>
          );
        })}
      </div>
    </div>
  );
}
