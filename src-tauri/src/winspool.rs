//! Типы и функции для работы с Windows Spooler API (`winspool.drv`).

use serde::Serialize;
use std::ptr;
use windows_sys::Win32::Graphics::Printing::*;

use crate::sys;

// ---------------------------------------------------------------------------
// Буфер с выравниванием под Win32-структуры (Vec<u8> имеет align = 1).
// ---------------------------------------------------------------------------
struct WinBuffer {
    words: Vec<u64>,
}

impl WinBuffer {
    fn new(bytes: usize) -> Self {
        Self {
            words: vec![0; bytes.div_ceil(8)],
        }
    }

    fn as_mut_ptr(&mut self) -> *mut u8 {
        self.words.as_mut_ptr().cast()
    }

    fn as_ptr(&self) -> *const u8 {
        self.words.as_ptr().cast()
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe fn from_wide(p: *const u16) -> String {
    if p.is_null() {
        return String::new();
    }
    let mut len = 0;
    while *p.add(len) != 0 {
        len += 1;
    }
    String::from_utf16_lossy(std::slice::from_raw_parts(p, len))
}

/// RAII-обёртка над дескриптором принтера.
struct PrinterHandle(PRINTER_HANDLE);

impl Drop for PrinterHandle {
    fn drop(&mut self) {
        unsafe { ClosePrinter(self.0) };
    }
}

fn open_printer(name: &str, access: u32) -> Result<PrinterHandle, String> {
    let w = wide(name);
    let mut handle = PRINTER_HANDLE::default();
    let defaults = PRINTER_DEFAULTSW {
        pDatatype: ptr::null_mut(),
        pDevMode: ptr::null_mut(),
        DesiredAccess: access,
    };
    let ok = unsafe { OpenPrinterW(w.as_ptr(), &mut handle, &defaults) };
    if ok == 0 {
        return Err(format!(
            "Не удалось открыть «{name}»: {}",
            sys::last_error_message()
        ));
    }
    Ok(PrinterHandle(handle))
}

// ---------------------------------------------------------------------------
// Модели данных
// ---------------------------------------------------------------------------

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PrinterInfo {
    pub name: String,
    pub share_name: String,
    pub port: String,
    pub driver: String,
    pub comment: String,
    pub location: String,
    pub attributes: u32,
    pub status: u32,
    pub jobs: u32,
    pub is_default: bool,
    pub status_text: Vec<String>,
    pub attribute_text: Vec<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct JobInfo {
    pub id: u32,
    pub printer: String,
    pub document: String,
    pub user: String,
    pub datatype: String,
    pub status: u32,
    pub status_text: Vec<String>,
    pub total_pages: u32,
    pub pages_printed: u32,
    pub priority: u32,
    pub position: u32,
    pub submitted: String,
}

/// Опция принтера (виден в диалоге «Свойства»); на Windows диалог пока
/// реализован только для чтения через CUPS-бэкенд, см. get_printer_properties.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PrinterChoice {
    pub value: String,
    pub label: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PrinterOption {
    pub key: String,
    pub label: String,
    pub value: String,
    pub choices: Vec<PrinterChoice>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PrinterProperties {
    pub name: String,
    pub uri: String,
    pub driver: String,
    pub location: String,
    pub description: String,
    pub shared: bool,
    pub options: Vec<PrinterOption>,
}

// ---------------------------------------------------------------------------
// Декодирование флагов (чистые функции — покрываются юнит-тестами)
// ---------------------------------------------------------------------------

pub fn printer_status_text(status: u32) -> Vec<String> {
    let all: &[(u32, &str)] = &[
        (PRINTER_STATUS_PAUSED, "Пауза"),
        (PRINTER_STATUS_PENDING_DELETION, "Удаляется"),
        (PRINTER_STATUS_PAPER_JAM, "Замятие бумаги"),
        (PRINTER_STATUS_PAPER_OUT, "Нет бумаги"),
        (PRINTER_STATUS_MANUAL_FEED, "Ручная подача"),
        (PRINTER_STATUS_PAPER_PROBLEM, "Проблема с бумагой"),
        (PRINTER_STATUS_OFFLINE, "Не в сети"),
        (PRINTER_STATUS_IO_ACTIVE, "Ввод-вывод активен"),
        (PRINTER_STATUS_BUSY, "Занят"),
        (PRINTER_STATUS_PRINTING, "Печать"),
        (PRINTER_STATUS_OUTPUT_BIN_FULL, "Отсек полон"),
        (PRINTER_STATUS_NOT_AVAILABLE, "Недоступен"),
        (PRINTER_STATUS_DOOR_OPEN, "Открыта крышка"),
        (PRINTER_STATUS_TONER_LOW, "Мало тонера"),
        (PRINTER_STATUS_NO_TONER, "Нет тонера"),
        (PRINTER_STATUS_PAGE_PUNT, "Страница отклонена"),
        (PRINTER_STATUS_USER_INTERVENTION, "Требуется вмешательство"),
        (PRINTER_STATUS_OUT_OF_MEMORY, "Нет памяти"),
        (PRINTER_STATUS_WARMING_UP, "Прогрев"),
        (PRINTER_STATUS_INITIALIZING, "Инициализация"),
        (PRINTER_STATUS_WAITING, "Ожидание"),
        (PRINTER_STATUS_PROCESSING, "Обработка"),
        (PRINTER_STATUS_POWER_SAVE, "Энергосбережение"),
        (PRINTER_STATUS_SERVER_OFFLINE, "Сервер офлайн"),
        (PRINTER_STATUS_SERVER_UNKNOWN, "Сервер неизвестен"),
        (PRINTER_STATUS_DRIVER_UPDATE_NEEDED, "Нужно обновление драйвера"),
    ];
    let mut out: Vec<String> = all
        .iter()
        .filter(|(bit, _)| status & bit != 0)
        .map(|(_, text)| text.to_string())
        .collect();
    if out.is_empty() {
        out.push(if status == 0 {
            "Готов".to_string()
        } else {
            format!("Неизвестный статус (0x{status:X})")
        });
    }
    out
}

pub fn attribute_text(attributes: u32) -> Vec<String> {
    let list: &[(u32, &str)] = &[
        (PRINTER_ATTRIBUTE_QUEUED, "В очереди"),
        (PRINTER_ATTRIBUTE_DIRECT, "Прямая печать"),
        (PRINTER_ATTRIBUTE_DEFAULT, "По умолчанию"),
        (PRINTER_ATTRIBUTE_SHARED, "Общий доступ"),
        (PRINTER_ATTRIBUTE_NETWORK, "Сетевой"),
        (PRINTER_ATTRIBUTE_LOCAL, "Локальный"),
        (PRINTER_ATTRIBUTE_HIDDEN, "Скрыт"),
        (PRINTER_ATTRIBUTE_KEEPPRINTEDJOBS, "Хранить задания"),
        (PRINTER_ATTRIBUTE_DO_COMPLETE_FIRST, "Сначала завершённые"),
        (PRINTER_ATTRIBUTE_WORK_OFFLINE, "Работает офлайн"),
        (PRINTER_ATTRIBUTE_ENABLE_BIDI, "Двусторонняя связь"),
        (PRINTER_ATTRIBUTE_RAW_ONLY, "Только RAW"),
        (PRINTER_ATTRIBUTE_PUBLISHED, "Опубликован"),
        (PRINTER_ATTRIBUTE_FAX, "Факс"),
        (PRINTER_ATTRIBUTE_ENABLE_DEVQ, "Очередь на устройстве"),
    ];
    list.iter()
        .filter(|(bit, _)| attributes & bit != 0)
        .map(|(_, text)| text.to_string())
        .collect()
}

pub fn job_status_text(status: u32) -> Vec<String> {
    let list: &[(u32, &str)] = &[
        (JOB_STATUS_PAUSED, "Пауза"),
        (JOB_STATUS_ERROR, "Ошибка"),
        (JOB_STATUS_DELETING, "Удаляется"),
        (JOB_STATUS_SPOOLING, "Постановка в очередь"),
        (JOB_STATUS_PRINTING, "Печать"),
        (JOB_STATUS_OFFLINE, "Не в сети"),
        (JOB_STATUS_PAPEROUT, "Нет бумаги"),
        (JOB_STATUS_PRINTED, "Напечатано"),
        (JOB_STATUS_DELETED, "Удалено"),
        (JOB_STATUS_BLOCKED_DEVQ, "Заблокировано"),
        (JOB_STATUS_USER_INTERVENTION, "Требуется вмешательство"),
        (JOB_STATUS_RESTART, "Перезапуск"),
        (JOB_STATUS_COMPLETE, "Завершено"),
        (JOB_STATUS_RETAINED, "Сохранено"),
    ];
    let mut out: Vec<String> = list
        .iter()
        .filter(|(bit, _)| status & bit != 0)
        .map(|(_, text)| text.to_string())
        .collect();
    if out.is_empty() {
        out.push(if status == 0 {
            "В очереди".to_string()
        } else {
            format!("Флаги 0x{status:X}")
        });
    }
    out
}

// ---------------------------------------------------------------------------
// Операции
// ---------------------------------------------------------------------------

fn get_default_printer() -> Option<String> {
    let mut len: u32 = 0;
    unsafe { GetDefaultPrinterW(ptr::null_mut(), &mut len) };
    if len == 0 {
        return None;
    }
    let mut buf = vec![0u16; len as usize];
    if unsafe { GetDefaultPrinterW(buf.as_mut_ptr(), &mut len) } == 0 {
        return None;
    }
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Some(String::from_utf16_lossy(&buf[..end]))
}

pub fn list_printers() -> Result<Vec<PrinterInfo>, String> {
    let flags = PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS;
    let mut needed: u32 = 0;
    let mut count: u32 = 0;
    unsafe {
        EnumPrintersW(flags, ptr::null(), 2, ptr::null_mut(), 0, &mut needed, &mut count)
    };
    if needed == 0 {
        return Err(sys::last_error_message());
    }

    let mut buf = WinBuffer::new(needed as usize);
    let ok = unsafe {
        EnumPrintersW(flags, ptr::null(), 2, buf.as_mut_ptr(), needed, &mut needed, &mut count)
    };
    if ok == 0 {
        return Err(sys::last_error_message());
    }

    let infos =
        unsafe { std::slice::from_raw_parts(buf.as_ptr().cast::<PRINTER_INFO_2W>(), count as usize) };
    let default = get_default_printer();

    Ok(infos
        .iter()
        .map(|p| {
            let name = unsafe { from_wide(p.pPrinterName) };
            PrinterInfo {
                is_default: default.as_deref() == Some(name.as_str()),
                name,
                share_name: unsafe { from_wide(p.pShareName) },
                port: unsafe { from_wide(p.pPortName) },
                driver: unsafe { from_wide(p.pDriverName) },
                comment: unsafe { from_wide(p.pComment) },
                location: unsafe { from_wide(p.pLocation) },
                attributes: p.Attributes,
                status: p.Status,
                jobs: p.cJobs,
                status_text: printer_status_text(p.Status),
                attribute_text: attribute_text(p.Attributes),
            }
        })
        .collect())
}

pub fn set_default_printer(name: &str) -> Result<(), String> {
    let w = wide(name);
    if unsafe { SetDefaultPrinterW(w.as_ptr()) } == 0 {
        return Err(sys::last_error_message());
    }
    Ok(())
}

/// Пауза/возобновление очереди (`SetPrinter` с `PRINTER_CONTROL_*`).
pub fn set_printer_paused(name: &str, paused: bool) -> Result<(), String> {
    let handle = open_printer(name, PRINTER_ALL_ACCESS)?;
    let command = if paused { PRINTER_CONTROL_PAUSE } else { PRINTER_CONTROL_RESUME };
    if unsafe { SetPrinterW(handle.0, 0, ptr::null(), command) } == 0 {
        return Err(format!(
            "Не удалось изменить состояние «{name}»: {}",
            sys::last_error_message()
        ));
    }
    Ok(())
}

fn get_printer_level2(handle: PRINTER_HANDLE) -> Result<WinBuffer, String> {
    let mut needed: u32 = 0;
    unsafe { GetPrinterW(handle, 2, ptr::null_mut(), 0, &mut needed) };
    if needed == 0 {
        return Err(sys::last_error_message());
    }
    let mut buf = WinBuffer::new(needed as usize);
    if unsafe { GetPrinterW(handle, 2, buf.as_mut_ptr(), needed, &mut needed) } == 0 {
        return Err(sys::last_error_message());
    }
    Ok(buf)
}

/// Включает/выключает общий доступ к принтеру.
pub fn set_printer_shared(name: &str, shared: bool) -> Result<(), String> {
    let handle = open_printer(name, PRINTER_ALL_ACCESS)?;
    let mut buf = get_printer_level2(handle.0)?;
    let info = unsafe { &mut *buf.as_mut_ptr().cast::<PRINTER_INFO_2W>() };
    if shared {
        info.Attributes |= PRINTER_ATTRIBUTE_SHARED;
        if info.pShareName.is_null() {
            info.pShareName = info.pPrinterName;
        }
    } else {
        info.Attributes &= !PRINTER_ATTRIBUTE_SHARED;
    }
    if unsafe { SetPrinterW(handle.0, 2, buf.as_ptr(), 0) } == 0 {
        return Err(format!(
            "Не удалось изменить общий доступ «{name}»: {}",
            sys::last_error_message()
        ));
    }
    Ok(())
}

pub fn delete_printer(name: &str) -> Result<(), String> {
    let handle = open_printer(name, PRINTER_ALL_ACCESS)?;
    if unsafe { DeletePrinter(handle.0) } == 0 {
        return Err(format!(
            "Не удалось удалить «{name}»: {}",
            sys::last_error_message()
        ));
    }
    Ok(())
}

pub fn add_printer_connection(path: &str) -> Result<(), String> {
    let w = wide(path);
    if unsafe { AddPrinterConnectionW(w.as_ptr()) } == 0 {
        return Err(format!(
            "Не удалось подключить «{path}»: {}",
            sys::last_error_message()
        ));
    }
    Ok(())
}

/// Диалог «Свойства» (место, описание, опции драйвера) реализован поверх CUPS.
/// На Windows возвращаем понятную ошибку — UI скроет кнопку/покажет тост.
pub fn get_printer_properties(_name: &str) -> Result<PrinterProperties, String> {
    Err("Свойства принтера доступны пока только в Linux/CUPS-версии".into())
}

pub fn set_printer_properties(
    _name: &str,
    _location: &str,
    _description: &str,
    _shared: bool,
    _options: &std::collections::HashMap<String, String>,
) -> Result<(), String> {
    Err("Свойства принтера доступны пока только в Linux/CUPS-версии".into())
}

fn format_systemtime(st: &windows_sys::Win32::Foundation::SYSTEMTIME) -> String {
    if st.wYear == 0 {
        return String::new();
    }
    format!(
        "{:02}.{:02}.{:04} {:02}:{:02}",
        st.wDay, st.wMonth, st.wYear, st.wHour, st.wMinute
    )
}

pub fn list_jobs(printer: &str) -> Result<Vec<JobInfo>, String> {
    let handle = open_printer(printer, 0)?;
    let mut needed: u32 = 0;
    let mut count: u32 = 0;
    unsafe { EnumJobsW(handle.0, 0, 0xFFFF, 1, ptr::null_mut(), 0, &mut needed, &mut count) };
    if needed == 0 {
        // Нет заданий — это не ошибка.
        return Ok(Vec::new());
    }
    let mut buf = WinBuffer::new(needed as usize);
    let ok = unsafe {
        EnumJobsW(handle.0, 0, 0xFFFF, 1, buf.as_mut_ptr(), needed, &mut needed, &mut count)
    };
    if ok == 0 {
        return Err(format!(
            "Не удалось получить задания «{printer}»: {}",
            sys::last_error_message()
        ));
    }

    let jobs =
        unsafe { std::slice::from_raw_parts(buf.as_ptr().cast::<JOB_INFO_1W>(), count as usize) };
    Ok(jobs
        .iter()
        .map(|j| JobInfo {
            id: j.JobId,
            printer: unsafe { from_wide(j.pPrinterName) },
            document: unsafe { from_wide(j.pDocument) },
            user: unsafe { from_wide(j.pUserName) },
            datatype: unsafe { from_wide(j.pDatatype) },
            status: j.Status,
            status_text: job_status_text(j.Status),
            total_pages: j.TotalPages,
            pages_printed: j.PagesPrinted,
            priority: j.Priority,
            position: j.Position,
            submitted: format_systemtime(&j.Submitted),
        })
        .collect())
}

/// Действия над заданием: `cancel` | `pause` | `resume` | `restart`.
pub fn job_action(printer: &str, job_id: u32, action: &str) -> Result<(), String> {
    let command = match action {
        "cancel" => JOB_CONTROL_DELETE,
        "pause" => JOB_CONTROL_PAUSE,
        "resume" => JOB_CONTROL_RELEASE,
        "restart" => JOB_CONTROL_RESTART,
        other => return Err(format!("Неизвестное действие над заданием: {other}")),
    };
    let handle = open_printer(printer, PRINTER_ALL_ACCESS)?;
    if unsafe { SetJobW(handle.0, job_id, 0, ptr::null(), command) } == 0 {
        return Err(format!(
            "Не удалось выполнить действие над заданием №{job_id}: {}",
            sys::last_error_message()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_zero_is_ready() {
        assert_eq!(printer_status_text(0), vec!["Готов".to_string()]);
    }

    #[test]
    fn status_decodes_multiple_bits() {
        let status = PRINTER_STATUS_PAUSED | PRINTER_STATUS_PAPER_OUT;
        let text = printer_status_text(status);
        assert!(text.contains(&"Пауза".to_string()));
        assert!(text.contains(&"Нет бумаги".to_string()));
    }

    #[test]
    fn attributes_decodes_shared_and_local() {
        let attrs = PRINTER_ATTRIBUTE_SHARED | PRINTER_ATTRIBUTE_LOCAL;
        let text = attribute_text(attrs);
        assert!(text.contains(&"Общий доступ".to_string()));
        assert!(text.contains(&"Локальный".to_string()));
    }

    #[test]
    fn attributes_empty_when_none() {
        assert!(attribute_text(0).is_empty());
    }

    #[test]
    fn job_status_defaults_to_queued() {
        assert_eq!(job_status_text(0), vec!["В очереди".to_string()]);
    }

    #[test]
    fn job_status_printing() {
        assert_eq!(job_status_text(JOB_STATUS_PRINTING), vec!["Печать".to_string()]);
    }

    #[test]
    fn wide_roundtrip() {
        let s = "Принтер HP";
        let w = wide(s);
        let back = unsafe { from_wide(w.as_ptr()) };
        assert_eq!(back, s);
    }

    #[test]
    fn empty_wide_is_empty_string() {
        let back = unsafe { from_wide(std::ptr::null()) };
        assert_eq!(back, "");
    }
}
