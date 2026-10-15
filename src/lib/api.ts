/** Типизированные обёртки над Tauri invoke/event. */

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

// ---------------------------------------------------------------------------
// Модели (должны совпадать с serde-типами backend'а)
// ---------------------------------------------------------------------------

export interface PrinterInfo {
  name: string;
  shareName: string;
  port: string;
  driver: string;
  comment: string;
  location: string;
  attributes: number;
  status: number;
  jobs: number;
  isDefault: boolean;
  statusText: string[];
  attributeText: string[];
}

export interface JobInfo {
  id: number;
  printer: string;
  document: string;
  user: string;
  datatype: string;
  status: number;
  statusText: string[];
  totalPages: number;
  pagesPrinted: number;
  priority: number;
  position: number;
  submitted: string;
}

export interface FoundPrinter {
  fullname: string;
  serviceType: string;
  instance: string;
  host: string;
  port: number;
  addresses: string[];
  txt: Record<string, string>;
}

export interface PrinterChoice {
  value: string;
  label: string;
}

export interface PrinterOption {
  key: string;
  label: string;
  value: string;
  choices: PrinterChoice[];
}

export interface PrinterProperties {
  name: string;
  uri: string;
  driver: string;
  location: string;
  description: string;
  shared: boolean;
  options: PrinterOption[];
}

export interface AppConfig {
  tab: string;
  selectedPrinter: string | null;
  search: string;
  jobsPrinter: string | null;
  autoDiscovery: boolean;
  windowWidth: number;
  windowHeight: number;
  maximized: boolean;
}

// ---------------------------------------------------------------------------
// Команды
// ---------------------------------------------------------------------------

/** Текст ошибки из reject'а Tauri `invoke`: команда отклоняется строкой
 * (не Error), поэтому `(err as Error).message` даёт undefined и причина
 * теряется за запасным сообщением. */
export function errText(err: unknown, fallback: string): string {
  if (typeof err === "string" && err.trim()) return err;
  if (err instanceof Error && err.message) return err.message;
  if (err && typeof err === "object" && "message" in err) {
    const m = (err as { message?: unknown }).message;
    if (typeof m === "string" && m) return m;
  }
  return fallback;
}

export const api = {
  listPrinters: () => invoke<PrinterInfo[]>("list_printers"),
  setDefaultPrinter: (name: string) => invoke<void>("set_default_printer", { name }),
  setPrinterPaused: (name: string, paused: boolean) =>
    invoke<void>("set_printer_paused", { name, paused }),
  setPrinterShared: (name: string, shared: boolean) =>
    invoke<void>("set_printer_shared", { name, shared }),
  deletePrinter: (name: string) => invoke<void>("delete_printer", { name }),
  addPrinterConnection: (path: string) => invoke<void>("add_printer_connection", { path }),

  getPrinterProperties: (name: string) =>
    invoke<PrinterProperties>("get_printer_properties", { name }),
  setPrinterProperties: (
    name: string,
    location: string,
    description: string,
    shared: boolean,
    options: Record<string, string>
  ) => invoke<void>("set_printer_properties", { name, location, description, shared, options }),

  listJobs: (printer: string) => invoke<JobInfo[]>("list_jobs", { printer }),
  jobAction: (printer: string, jobId: number, action: string) =>
    invoke<void>("job_action", { printer, jobId, action }),

  openSystem: (kind: string, name?: string) => invoke<void>("open_system", { kind, name }),
  openUrl: (url: string) => invoke<void>("open_url", { url }),

  startDiscovery: () => invoke<void>("start_discovery"),
  stopDiscovery: () => invoke<void>("stop_discovery"),
  discoveryRunning: () => invoke<boolean>("discovery_running"),

  loadConfig: () => invoke<AppConfig>("load_config"),
  saveConfig: (cfg: AppConfig) => invoke<void>("save_config", { cfg }),

  /** `windows` | `linux` | `macos` — целевая платформа сборки. */
  platform: () => invoke<string>("platform"),
};

// ---------------------------------------------------------------------------
// События обнаружения
// ---------------------------------------------------------------------------

export function onPrinterFound(cb: (p: FoundPrinter) => void): Promise<UnlistenFn> {
  return listen<FoundPrinter>("discovery://found", (e) => cb(e.payload));
}

export function onPrinterRemoved(cb: (fullname: string) => void): Promise<UnlistenFn> {
  return listen<string>("discovery://removed", (e) => cb(e.payload));
}

/** Рекомендуемый веб-адрес найденного принтера (аналог web_url в Rust). */
export function webUrl(p: FoundPrinter): string | null {
  const addr = p.addresses[0];
  if (!addr) return null;
  const raw = p.txt["rp"] ?? "";
  const path = raw === "" ? "/" : raw.startsWith("/") ? raw : `/${raw}`;
  return `http://${addr}:${p.port}${path}`;
}

/** Хост из device-uri принтера: `ipp://192.168.1.50:631/ipp/print` → `192.168.1.50`.
 * IPv6-литерал в скобках возвращается без них. */
export function printerHost(uri: string): string | null {
  if (!uri) return null;
  const m = uri.match(/^[a-z][a-z0-9+.-]*:\/\/(\[[^\]]+\]|[^/ :]+)/i);
  if (!m) return null;
  let host = m[1];
  if (host.startsWith("[") && host.endsWith("]")) host = host.slice(1, -1);
  return host || null;
}

/** Веб-консоль (embedded web server) принтера: `http://<хост>/`. */
export function printerWebUrl(uri: string): string | null {
  const host = printerHost(uri);
  return host ? `http://${host}/` : null;
}
