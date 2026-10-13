import {
  useState,
  useEffect,
  useCallback,
  useRef,
  useMemo,
} from "react";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import { api, errText, type PrinterInfo, type JobInfo, type FoundPrinter, type AppConfig } from "./lib/api";

import PrintersView from "./views/PrintersView";
import JobsView from "./views/JobsView";
import DiscoveryView from "./views/DiscoveryView";
import AboutView from "./views/AboutView";
import Toast from "./components/Toast";

type Tab = "printers" | "jobs" | "discovery" | "about";

/** Подпись ОС для сайдбара и вкладки «О программе». */
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

interface ToastMessage {
  id: number;
  kind: "success" | "error" | "info" | "warn";
  text: string;
}

interface NavItem {
  id: Tab;
  label: string;
  icon: string;
}

const NAV_ITEMS: NavItem[] = [
  { id: "printers", label: "Принтеры", icon: "🖨️" },
  { id: "jobs", label: "Задания", icon: "📄" },
  { id: "discovery", label: "Обнаружение", icon: "🔍" },
  { id: "about", label: "О программе", icon: "ℹ️" },
];

/** Коротко форматирует массив текстов статуса. */
export function statusText(items: readonly string[], fallback = "Готов") {
  const trimmed = items.filter((s) => s.trim().length > 0);
  return trimmed.length > 0 ? trimmed.join(", ") : fallback;
}

export default function App() {
  const [config, setConfig] = useState<AppConfig>({
    tab: "printers",
    selectedPrinter: null,
    search: "",
    jobsPrinter: null,
    autoDiscovery: false,
    windowWidth: 920,
    windowHeight: 620,
    maximized: false,
  });
  const [activeTab, setActiveTab] = useState<Tab>("printers");
  const [printers, setPrinters] = useState<PrinterInfo[]>([]);
  const [jobs, setJobs] = useState<JobInfo[]>([]);
  const [discovered, setDiscovered] = useState<FoundPrinter[]>([]);
  const [discoveryRunning, setDiscoveryRunning] = useState(false);
  const [busy, setBusy] = useState(false);
  const [toasts, setToasts] = useState<ToastMessage[]>([]);
  const [modalOpen, setModalOpen] = useState(false);
  const [addPath, setAddPath] = useState("");
  const [returnTab, setReturnTab] = useState<Tab>("printers");
  const [os, setOs] = useState<string>("");
  const toastTimer = useRef<number | null>(null);

  // --- платформа сборки (для подписей ОС) ---
  useEffect(() => {
    api
      .platform()
      .then(setOs)
      .catch(() => setOs(""));
  }, []);

  // --- загрузить настройки из бэкенда ---
  useEffect(() => {
    api
      .loadConfig()
      .then((cfg) => {
        // Компактное окно по умолчанию: зажимаем размер, сохранённый старыми
        // версиями (1280×820), чтобы компактная вёрстка применилась и у
        // существующих пользователей. Пользовательский ресайз при этом
        // сохраняется (ресайз не записывает размер обратно в конфиг).
        const w = Math.min(Math.round(cfg.windowWidth) || 920, 920);
        const h = Math.min(Math.round(cfg.windowHeight) || 620, 620);
        setConfig({ ...cfg, windowWidth: w, windowHeight: h });
        setActiveTab(cfg.tab as Tab);
      })
      .catch(() => {
        /* настройки отсутствуют — берём значения по умолчанию */
      });
  }, []);

  // --- применение размера окна ---
  useEffect(() => {
    const win = getCurrentWindow();
    win.setSize?.(new LogicalSize(Math.round(config.windowWidth), Math.round(config.windowHeight)));
  }, [config.windowWidth, config.windowHeight]);

  // --- сохранение настроек (дебаунс) ---
  useEffect(() => {
    if (toastTimer.current !== null) {
      window.clearTimeout(toastTimer.current);
    }
    toastTimer.current = window.setTimeout(() => {
      saveConfig();
    }, 300);
    return () => {
      if (toastTimer.current !== null) {
        window.clearTimeout(toastTimer.current);
      }
    };
  }, [config]);

  const saveConfig = useCallback(async () => {
    try {
      await api.saveConfig(config);
    } catch {
      /* silent */
    }
  }, [config]);

  // --- тосты ---
  const showToast = useCallback((kind: ToastMessage["kind"], text: string) => {
    const id = Date.now() + Math.random();
    setToasts((prev) => [...prev, { id, kind, text }]);
    if (toastTimer.current !== null) {
      window.clearTimeout(toastTimer.current);
    }
    toastTimer.current = window.setTimeout(() => {
      setToasts((prev) => prev.filter((t) => t.id !== id));
    }, 3500);
  }, []);

  // --- принтеры ---
  const refreshPrinters = useCallback(async () => {
    setBusy(true);
    try {
      const list = await api.listPrinters();
      setPrinters(list);
    } catch (err) {
      showToast("error", errText(err, "Не удалось получить принтеры"));
    } finally {
      setBusy(false);
    }
  }, [showToast]);

  useEffect(() => {
    refreshPrinters();
  }, [refreshPrinters]);

  const handleSetDefault = useCallback(
    async (name: string) => {
      try {
        await api.setDefaultPrinter(name);
        showToast("success", "Теперь используется по умолчанию");
        await refreshPrinters();
      } catch (err) {
        showToast("error", errText(err, "Не удалось установить принтер по умолчанию"));
      }
    },
    [showToast, refreshPrinters]
  );

  const handleTogglePaused = useCallback(
    async (name: string) => {
      try {
        const printer = printers.find((p) => p.name === name);
        const paused = printer?.statusText.some((s) => /пауза/.test(s)) ?? false;
        await api.setPrinterPaused(name, !paused);
        showToast("success", !paused ? "Очередь приостановлена" : "Печать продолжена");
        await refreshPrinters();
      } catch (err) {
        showToast("error", errText(err, "Не удалось изменить состояние принтера"));
      }
    },
    [showToast, refreshPrinters, printers]
  );

  const handleToggleShared = useCallback(
    async (name: string) => {
      try {
        const printer = printers.find((p) => p.name === name);
        const shared = printer?.attributeText.includes("Общий доступ") ?? false;
        await api.setPrinterShared(name, !shared);
        showToast("success", !shared ? "Принтер сделать общим" : "Общий доступ отключен");
        await refreshPrinters();
      } catch (err) {
        showToast("error", errText(err, "Не удалось изменить общий доступ"));
      }
    },
    [showToast, refreshPrinters, printers]
  );

  const handleDelete = useCallback(
    async (name: string) => {
      if (!window.confirm(`Удалить принтер «${name}»?`)) return;
      try {
        await api.deletePrinter(name);
        showToast("success", "Принтер удалён");
        await refreshPrinters();
      } catch (err) {
        showToast("error", errText(err, "Не удалось удалить принтер"));
      }
    },
    [showToast, refreshPrinters]
  );

  // --- задания ---
  const refreshJobs = useCallback(
    async (printer: string | null) => {
      if (!printer) {
        setJobs([]);
        return;
      }
      setBusy(true);
      try {
        const list = await api.listJobs(printer);
        setJobs(list);
      } catch (err) {
        showToast("error", errText(err, "Не удалось получить задания"));
      } finally {
        setBusy(false);
      }
    },
    [showToast]
  );

  useEffect(() => {
    refreshJobs(config.jobsPrinter);
  }, [config.jobsPrinter, refreshJobs, activeTab]);

  const handleSelectPrinter = useCallback((name: string) => {
    setConfig((prev) => ({ ...prev, jobsPrinter: name }));
    setActiveTab("jobs");
  }, []);

  const handleJobAction = useCallback(
    async (printer: string, jobId: number, action: string) => {
      try {
        await api.jobAction(printer, jobId, action);
        showToast("success", action === "cancel" ? "Задание отменено" : `Задание ${action}нно`);
        await refreshJobs(printer);
      } catch (err) {
        showToast("error", errText(err, "Не удалось выполнить действие"));
      }
    },
    [showToast, refreshJobs]
  );

  // --- обнаружение ---
  const discoveryListenersRef = useRef<{ found?: UnlistenFn; removed?: UnlistenFn }>({});

  useEffect(() => {
    (async () => {
      try {
        discoveryListenersRef.current.found = await listen<FoundPrinter>(
          "discovery://found",
          (e) => {
            setDiscovered((prev) => {
              if (prev.some((p) => p.fullname === e.payload.fullname)) return prev;
              return [...prev, e.payload];
            });
          }
        );
        discoveryListenersRef.current.removed = await listen<string>(
          "discovery://removed",
          (e) => {
            setDiscovered((prev) => prev.filter((p) => p.fullname !== e.payload));
          }
        );
      } catch {
        /* ignore */
      }
    })();
    return () => {
      (discoveryListenersRef.current.found as UnlistenFn | undefined)?.();
      (discoveryListenersRef.current.removed as UnlistenFn | undefined)?.();
    };
  }, []);

  const pollDiscovery = useCallback(async () => {
    try {
      const running = await api.discoveryRunning();
      setDiscoveryRunning(running);
    } catch {
      /* ignore */
    }
  }, []);

  useEffect(() => {
    pollDiscovery();
    const id = window.setInterval(pollDiscovery, 1000);
    return () => window.clearInterval(id);
  }, [pollDiscovery]);

  useEffect(() => {
    if (config.autoDiscovery && !discoveryRunning) {
      startDiscovery();
    }
  }, [config.autoDiscovery, discoveryRunning]);

  const startDiscovery = useCallback(async () => {
    try {
      await api.startDiscovery();
      showToast("success", "Обнаружение запущено");
    } catch (err) {
      showToast("error", errText(err, "Не удалось запустить обнаружение"));
    }
  }, [showToast]);

  const stopDiscovery = useCallback(async () => {
    try {
      await api.stopDiscovery();
      showToast("success", "Обнаружение остановлено");
    } catch (err) {
      showToast("error", errText(err, "Не удалось остановить обнаружение"));
    }
  }, [showToast]);

  // --- навигация ---
  const goBack = useCallback(() => {
    setActiveTab(returnTab);
    setConfig((prev) => ({ ...prev, tab: returnTab }));
  }, [returnTab]);

  const handleTabChange = useCallback(
    (tab: Tab) => {
      if (tab === "jobs" || tab === "discovery") {
        setReturnTab(activeTab);
      }
      setActiveTab(tab);
      setConfig((prev) => ({ ...prev, tab }));
      setJobs([]);
      setDiscovered([]);
    },
    [activeTab]
  );

  const addPrinterPath = useCallback(async () => {
    const path = addPath.trim();
    if (!path) {
      showToast("warn", "Введите адрес принтера");
      return;
    }
    // Windows: только UNC \\хост\очередь.
    // Linux (CUPS): UNC, device-uri (ipp://, socket:// …) или «голый» адрес —
    // 192.168.1.50, printer.local:631, 192.168.1.50/Очередь. Разбор, пробу
    // портов и подбор device-uri делает бэкенд (cups.rs) и возвращает
    // понятную ошибку, если принтер не отвечает.
    if (!path.startsWith("\\\\") && os !== "linux") {
      showToast("error", "Путь должен начинаться с двойного обратного слеша");
      return;
    }
    setModalOpen(false);
    try {
      await api.addPrinterConnection(path);
      showToast("success", "Принтер подключён");
      setAddPath("");
      await refreshPrinters();
    } catch (err) {
      showToast("error", errText(err, "Не удалось подключить принтер"));
    }
  }, [addPath, os, showToast, refreshPrinters]);

  const filteredPrinters = useMemo(() => {
    const q = config.search.trim().toLowerCase();
    if (!q) return printers;
    return printers.filter(
      (p) =>
        p.name.toLowerCase().includes(q) ||
        p.shareName.toLowerCase().includes(q) ||
        p.driver.toLowerCase().includes(q) ||
        p.comment.toLowerCase().includes(q) ||
        p.location.toLowerCase().includes(q)
    );
  }, [printers, config.search]);

  const pendingPrinter = config.selectedPrinter;

  // --- рендер ---
  return (
    <div className="window">
      <div className="titlebar">
        <span className="titlebar__title">Printer Config</span>
        <button
          type="button"
          className="titlebar__btn titlebar__btn--close"
          aria-label="Закрыть"
          onClick={() => window.close()}
        >
          ✕
        </button>
      </div>

      <div className="body">
        <nav className="sidebar">
          <div className="sidebar__brand">
            <span className="sidebar__brand-name">Printer Config</span>
            <span className="sidebar__brand-sub">{os ? platformLabel(os) : "…"}</span>
          </div>

          {NAV_ITEMS.map((item) => {
            const active = activeTab === item.id;
            let badge: string | null = null;
            if (item.id === "printers") {
              badge = printers.length > 0 ? String(printers.length) : null;
            } else if (item.id === "jobs") {
              const count = jobs.filter((j) => j.status !== 0).length;
              badge = count > 0 ? String(count) : null;
            } else if (item.id === "discovery") {
              if (discoveryRunning) badge = "0";
            }
            return (
              <button
                key={item.id}
                type="button"
                className={`nav-item${active ? " nav-item--active" : ""}`}
                onClick={() => handleTabChange(item.id)}
              >
                <span>{item.icon}</span>
                <span>{item.label}</span>
                {badge !== null && <span className="nav-item__badge">{badge}</span>}
              </button>
            );
          })}

          <div className="sidebar__footer">
            <label className="checkbox">
              <input
                type="checkbox"
                checked={config.autoDiscovery}
                onChange={(e) =>
                  setConfig((prev) => ({ ...prev, autoDiscovery: e.target.checked }))
                }
              />
              <span>Автообнаружение при запуске</span>
            </label>
          </div>
        </nav>

        <main className="content">
          {activeTab === "printers" && (
            <PrintersView
              printers={filteredPrinters}
              pendingPrinter={pendingPrinter}
              search={config.search}
              onSearchChange={(v) => setConfig((prev) => ({ ...prev, search: v }))}
              onSetDefault={handleSetDefault}
              onTogglePaused={handleTogglePaused}
              onToggleShared={handleToggleShared}
              onDelete={handleDelete}
              onRefresh={refreshPrinters}
              onOpenJobs={handleSelectPrinter}
              onAddClick={() => setModalOpen(true)}
              busy={busy}
            />
          )}


          {activeTab === "jobs" && (
            <JobsView
              printers={printers}
              jobs={jobs}
              busy={busy}
              selectedPrinter={config.jobsPrinter}
              onRefresh={() => refreshJobs(config.jobsPrinter)}
              onSelectPrinter={handleSelectPrinter}
              onJobAction={handleJobAction}
              returnTab={returnTab}
              onBack={goBack}
            />
          )}
          {activeTab === "discovery" && (
            <DiscoveryView
              discovered={discovered}
              running={discoveryRunning}
              onStart={startDiscovery}
              onStop={stopDiscovery}
              onBack={goBack}
              onAddDiscovered={(host, instance) => {
                const path = `\\${host}\\${instance}`;
                api.addPrinterConnection(path)
                  .then(() => {
                    showToast("success", "Принтер подключён");
                    refreshPrinters();
                  })
                  .catch((err) => {
                    showToast("error", errText(err, "Не удалось подключить принтер"));
                  });
              }}
            />
          )}
          {activeTab === "about" && <AboutView os={os} />}
        </main>
      </div>

      {modalOpen && (
        <div className="modal" role="dialog" aria-modal="true">
          <div className="modal__panel">
            <h2 className="modal__title">Добавить сетевой принтер</h2>
            <p className="modal__hint">
              Достаточно IP-адреса — приложение само найдёт IPP или raw-порт.
              Также можно указать очередь или полный адрес:{" "}
              <code>192.168.1.50</code>, <code>192.168.1.50/Очередь</code>,{" "}
              <code>\\сервер\имя</code>, <code>ipp://хост/ipp/print</code>.
            </p>
            <input
              className="input"
              type="text"
              value={addPath}
              placeholder="192.168.1.50"
              onChange={(e) => setAddPath(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  addPrinterPath();
                }
                if (e.key === "Escape") {
                  setModalOpen(false);
                }
              }}
            />
            <div className="modal__actions">
              <button type="button" className="button button--primary" onClick={addPrinterPath}>
                Добавить
              </button>
              <button type="button" className="button" onClick={() => setModalOpen(false)}>
                Отмена
              </button>
            </div>
          </div>
        </div>
      )}

      <Toast toasts={toasts} />
    </div>
  );
}
