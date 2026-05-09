use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;

use chrono::Utc;
use tauri::{AppHandle, Manager};

static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();

const MAX_LOG_BYTES: u64 = 1_048_576; // 1 MiB

pub(crate) fn init(app: &AppHandle) {
    let Ok(dir) = app.path().app_data_dir() else {
        return;
    };
    let logs = dir.join("logs");
    if fs::create_dir_all(&logs).is_err() {
        return;
    }
    let path = logs.join("knightlauncher.log");
    rotate_if_oversized(&path);
    let _ = LOG_PATH.set(path);
}

pub(crate) fn path() -> Option<PathBuf> {
    LOG_PATH.get().cloned()
}

fn rotate_if_oversized(path: &PathBuf) {
    let Ok(meta) = fs::metadata(path) else {
        return;
    };
    if meta.len() <= MAX_LOG_BYTES {
        return;
    }
    let archived = path.with_extension("log.old");
    let _ = fs::rename(path, &archived);
}

fn write_line(level: &str, msg: &str) {
    eprintln!("[{level}] {msg}");
    let Some(path) = LOG_PATH.get() else {
        return;
    };
    let ts = Utc::now().to_rfc3339();
    let line = format!("[{ts}] [{level}] {msg}\n");
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = file.write_all(line.as_bytes());
    }
}

pub(crate) fn warn(msg: impl AsRef<str>) {
    write_line("WARN", msg.as_ref());
}

pub(crate) fn error(msg: impl AsRef<str>) {
    write_line("ERROR", msg.as_ref());
}

#[tauri::command]
pub(crate) fn get_log_path() -> Option<String> {
    path().map(|p| p.to_string_lossy().to_string())
}
