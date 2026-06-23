use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use serde_json::{Map, Value};

const LOG_FILE_NAME: &str = "shell.log";
const MAX_LOG_TEXT_CHARS: usize = 600;

#[derive(Clone)]
pub struct ShellDiagnostics {
    inner: Arc<ShellDiagnosticsInner>,
}

struct ShellDiagnosticsInner {
    log_dir: PathBuf,
    log_file: PathBuf,
    writer: Mutex<File>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellLogInfo {
    pub log_dir: String,
    pub log_file: String,
}

impl ShellDiagnostics {
    pub fn open(log_dir: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(&log_dir).map_err(|err| {
            format!(
                "failed to create shell log directory {}: {err}",
                log_dir.display()
            )
        })?;
        let log_file = shell_log_file_path(&log_dir);
        let writer = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file)
            .map_err(|err| format!("failed to open shell log {}: {err}", log_file.display()))?;
        Ok(Self {
            inner: Arc::new(ShellDiagnosticsInner {
                log_dir,
                log_file,
                writer: Mutex::new(writer),
            }),
        })
    }

    pub fn log_info(&self) -> ShellLogInfo {
        ShellLogInfo {
            log_dir: self.inner.log_dir.display().to_string(),
            log_file: self.inner.log_file.display().to_string(),
        }
    }

    pub fn log_dir(&self) -> &Path {
        &self.inner.log_dir
    }

    pub fn log_event(&self, event: &str, fields: Value) {
        if let Err(err) = self.try_log_event(event, fields) {
            eprintln!("maccursor-shell log write failed: {err}");
        }
    }

    fn try_log_event(&self, event: &str, fields: Value) -> Result<(), String> {
        let mut entry = Map::new();
        entry.insert("tsUnixMs".to_string(), Value::from(unix_time_ms()));
        entry.insert("event".to_string(), Value::from(bounded_log_text(event)));
        if let Value::Object(fields) = fields {
            for (key, value) in fields {
                entry.insert(key, value);
            }
        }

        let line = serde_json::to_string(&Value::Object(entry))
            .map_err(|err| format!("failed to serialize shell log event: {err}"))?;
        let mut writer = self
            .inner
            .writer
            .lock()
            .map_err(|_| "shell log writer is poisoned".to_string())?;
        writeln!(writer, "{line}").map_err(|err| format!("failed to write shell log: {err}"))?;
        writer
            .flush()
            .map_err(|err| format!("failed to flush shell log: {err}"))?;
        Ok(())
    }
}

pub fn shell_log_file_path(log_dir: &Path) -> PathBuf {
    log_dir.join(LOG_FILE_NAME)
}

pub fn bounded_log_text(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.chars().count() <= MAX_LOG_TEXT_CHARS {
        return trimmed.to_string();
    }
    let mut truncated: String = trimmed.chars().take(MAX_LOG_TEXT_CHARS).collect();
    truncated.push_str("...");
    truncated
}

fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_log_file_uses_stable_name_inside_log_dir() {
        assert_eq!(
            shell_log_file_path(Path::new("/tmp/rts-shell-logs")),
            Path::new("/tmp/rts-shell-logs").join("shell.log")
        );
    }

    #[test]
    fn bounded_log_text_trims_and_limits_long_values() {
        assert_eq!(bounded_log_text("  startup  "), "startup");
        let long = "a".repeat(MAX_LOG_TEXT_CHARS + 12);
        let bounded = bounded_log_text(&long);
        assert_eq!(bounded.chars().count(), MAX_LOG_TEXT_CHARS + 3);
        assert!(bounded.ends_with("..."));
    }
}
