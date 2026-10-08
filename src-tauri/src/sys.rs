//! Работа с кодами ошибок Windows: `FormatMessageW` + локализация частых ошибок.

use windows_sys::Win32::System::Diagnostics::Debug::{
    FormatMessageW, FORMAT_MESSAGE_FROM_SYSTEM, FORMAT_MESSAGE_IGNORE_INSERTS,
};

/// Локализованные сообщения для часто встречающихся кодов ошибок спooler'а.
pub fn localized(code: u32) -> Option<&'static str> {
    Some(match code {
        5 => "Доступ запрещён (требуются права администратора)",
        6 => "Недействительный дескриптор",
        87 => "Некорректный параметр",
        122 => "Недостаточно данных для буфера",
        1722 => "Служба печати недоступна",
        1726 => "Служба печати не отвечает",
        1801 => "Принтер не существует",
        1802 => "Принтер не установлен",
        1805 => "Подходящий драйвер принтера не найден",
        2202 => "Учётное имя пользователя неверно",
        2250 => "Сетевой путь не найден",
        _ => return None,
    })
}

/// Человекочитаемое сообщение об ошибке Windows.
pub fn last_error_message() -> String {
    let code = unsafe { windows_sys::Win32::Foundation::GetLastError() };
    message_for(code)
}

pub fn message_for(code: u32) -> String {
    if let Some(text) = localized(code) {
        return format!("{text} (код {code})");
    }

    let mut buf = [0u16; 512];
    let len = unsafe {
        FormatMessageW(
            FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS,
            std::ptr::null(),
            code,
            0,
            buf.as_mut_ptr(),
            buf.len() as u32,
            std::ptr::null(),
        )
    };
    if len == 0 {
        return format!("Ошибка Windows {code}");
    }
    let mut msg = String::from_utf16_lossy(&buf[..len as usize]);
    while msg.ends_with(['\r', '\n', ' ']) {
        msg.pop();
    }
    format!("{msg} (код {code})")
}
