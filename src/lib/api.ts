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

export const api = {
  listPrinters: () => invoke<PrinterInfo[]>("list_printers"),
  setDefaultPrinter: (name: string) => invoke<void>("set_default_printer", { name }),
  setPrinterPaused: (name: string, paused: boolean) =>
    invoke<void>("set_printer_paused", { name, paused }),
  setPrinterShared: (name: string, shared: boolean) =>
    invoke<void>("set_printer_shared", { name, shared }),
  deletePrinter: (name: string) => invoke<void>("delete_printer", { name }),
  addPrinterConnection: (path: string) => invoke<void>("add_printer_connection", { path }),

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
