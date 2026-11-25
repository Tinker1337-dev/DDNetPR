//! Неблокирующий асинхронный файловый логгер для плагинов DDNetPR (`logs/<plugin>.log`).

use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{BufWriter, Write};
use std::sync::OnceLock;
use std::sync::mpsc::{self, SyncSender};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

static LOGGER: OnceLock<SyncSender<String>> = OnceLock::new();
static PLUGIN_NAME: OnceLock<String> = OnceLock::new();

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Уровень важности сообщения.
pub enum LogLevel {
    /// Отладка.
    Debug,
    /// Информация.
    Info,
    /// Предупреждение.
    Warn,
    /// Ошибка.
    Error,
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Debug => f.write_str("DEBUG"),
            Self::Info => f.write_str("INFO"),
            Self::Warn => f.write_str("WARN"),
            Self::Error => f.write_str("ERROR"),
        }
    }
}

fn sanitize_name(name: &str) -> String {
    let trimmed = name.trim().trim_start_matches('!').trim_start_matches('/');
    let clean: String = trimmed
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect();
    if clean.is_empty() { "plugin".to_string() } else { clean }
}

fn format_utc_now() -> String {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (hour, min, sec) = (rem / 3_600, (rem % 3_600) / 60, rem % 60);
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = y + if m <= 2 { 1 } else { 0 };
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}Z", year, m, d, hour, min, sec)
}

/// Инициализирует фоновую запись логов в `logs/<plugin_name>.log` один раз на процесс.
pub fn init(plugin_name: &str) {
    let clean = sanitize_name(plugin_name);
    let _ = PLUGIN_NAME.set(clean.clone());
    LOGGER.get_or_init(|| {
        let (tx, rx) = mpsc::sync_channel::<String>(512);
        let path = format!("logs/{}.log", clean);
        let _ = thread::Builder::new().name(format!("log-{}", clean)).spawn(move || {
            let _ = fs::create_dir_all("logs");
            if let Ok(file) = OpenOptions::new().create(true).append(true).open(&path) {
                let mut writer = BufWriter::new(file);
                while let Ok(line) = rx.recv() {
                    if writeln!(writer, "{}", line).is_err() { break; }
                    while let Ok(next) = rx.try_recv() {
                        if writeln!(writer, "{}", next).is_err() { return; }
                    }
                    let _ = writer.flush();
                }
            }
        });
        tx
    });
}

/// Ставит отформатированную строку лога в очередь без блокировки вызывающего потока.
pub fn write_log(level: LogLevel, msg: &str) {
    if LOGGER.get().is_none() {
        let fallback = PLUGIN_NAME.get().map(|s| s.as_str()).unwrap_or("plugin");
        init(fallback);
    }
    if let Some(tx) = LOGGER.get() {
        let line = format!("[{}] [{}] {}", format_utc_now(), level, msg);
        let _ = tx.try_send(line);
    }
}


/// Записывает информационное сообщение в `logs/<plugin>.log`.
#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => { $crate::log::write_log($crate::log::LogLevel::Info, &format!($($arg)*)) };
}

/// Записывает предупреждение в `logs/<plugin>.log`.
#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => { $crate::log::write_log($crate::log::LogLevel::Warn, &format!($($arg)*)) };
}

/// Записывает сообщение об ошибке в `logs/<plugin>.log`.
#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => { $crate::log::write_log($crate::log::LogLevel::Error, &format!($($arg)*)) };
}

/// Записывает отладочное сообщение в `logs/<plugin>.log`.
#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => { $crate::log::write_log($crate::log::LogLevel::Debug, &format!($($arg)*)) };
}
