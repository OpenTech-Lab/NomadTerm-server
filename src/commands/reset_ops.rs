use std::fs;

use crate::db::HcomDb;
use crate::paths::{ARCHIVE_DIR, FLAGS_DIR, LAUNCH_DIR, LOGS_DIR, hcom_dir};
use crate::shared::shorten_path;

/// Get timestamp for archive directory names.
pub(crate) fn get_archive_timestamp() -> String {
    chrono::Local::now().format("%Y-%m-%d_%H%M%S").to_string()
}

/// Archive the current database to ~/.nomadterm/archive/session-{timestamp}/.
pub(crate) fn archive_and_clear_db() -> Result<Option<String>, String> {
    let base = hcom_dir();
    let db_file = base.join("nomadterm.db");
    let db_wal = base.join("nomadterm.db-wal");
    let db_shm = base.join("nomadterm.db-shm");

    if !db_file.exists() {
        return Ok(None);
    }

    let has_content = {
        let conn = rusqlite::Connection::open(&db_file).map_err(|e| e.to_string())?;
        let event_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM events", [], |r| r.get(0))
            .unwrap_or(0);
        let instance_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM instances", [], |r| r.get(0))
            .unwrap_or(0);
        event_count > 0 || instance_count > 0
    };

    if !has_content {
        let _ = fs::remove_file(&db_file);
        let _ = fs::remove_file(&db_wal);
        let _ = fs::remove_file(&db_shm);
        return Ok(None);
    }

    if let Ok(conn) = rusqlite::Connection::open(&db_file) {
        let _ = conn.execute_batch("PRAGMA wal_checkpoint(PASSIVE)");
    }

    let timestamp = get_archive_timestamp();
    let session_archive = base.join(ARCHIVE_DIR).join(format!("session-{timestamp}"));
    fs::create_dir_all(&session_archive).map_err(|e| e.to_string())?;

    fs::copy(&db_file, session_archive.join("nomadterm.db")).map_err(|e| e.to_string())?;
    if db_wal.exists() {
        let _ = fs::copy(&db_wal, session_archive.join("nomadterm.db-wal"));
    }
    if db_shm.exists() {
        let _ = fs::copy(&db_shm, session_archive.join("nomadterm.db-shm"));
    }

    let _ = fs::remove_file(&db_file);
    let _ = fs::remove_file(&db_wal);
    let _ = fs::remove_file(&db_shm);

    Ok(Some(session_archive.to_string_lossy().to_string()))
}

/// Clean temp files (launch scripts, prompts, old logs).
pub(crate) fn clean_temp_files() {
    let base = hcom_dir();
    let cutoff_24h = crate::shared::time::now_epoch_f64() - 86400.0;
    let cutoff_30d = crate::shared::time::now_epoch_f64() - 30.0 * 86400.0;

    let launch_dir = base.join(LAUNCH_DIR);
    if launch_dir.exists() {
        if let Ok(rd) = fs::read_dir(&launch_dir) {
            for entry in rd.filter_map(|e| e.ok()) {
                if entry.path().is_file() {
                    if let Ok(meta) = entry.metadata() {
                        if let Ok(mtime) = meta.modified() {
                            let secs = crate::shared::system_time_to_epoch_f64(mtime);
                            if secs < cutoff_24h {
                                let _ = fs::remove_file(entry.path());
                            }
                        }
                    }
                }
            }
        }
    }

    let prompts_dir = base.join(".tmp").join("prompts");
    if prompts_dir.exists() {
        if let Ok(rd) = fs::read_dir(&prompts_dir) {
            for entry in rd.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("md") {
                    if let Ok(meta) = entry.metadata() {
                        if let Ok(mtime) = meta.modified() {
                            let secs = crate::shared::system_time_to_epoch_f64(mtime);
                            if secs < cutoff_24h {
                                let _ = fs::remove_file(path);
                            }
                        }
                    }
                }
            }
        }
    }

    let logs_dir = base.join(LOGS_DIR);
    if logs_dir.exists() {
        if let Ok(rd) = fs::read_dir(&logs_dir) {
            for entry in rd.filter_map(|e| e.ok()) {
                let path = entry.path();
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if name.starts_with("background_") && name.ends_with(".log") {
                    if let Ok(meta) = entry.metadata() {
                        if let Ok(mtime) = meta.modified() {
                            let secs = crate::shared::system_time_to_epoch_f64(mtime);
                            if secs < cutoff_30d {
                                let _ = fs::remove_file(path);
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Archive and reset config files.
pub(crate) fn reset_config() -> i32 {
    let base = hcom_dir();
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
    let archive_config_dir = base.join(ARCHIVE_DIR).join("config");

    let mut archived = false;

    let toml_path = base.join("config.toml");
    if toml_path.exists() {
        let _ = fs::create_dir_all(&archive_config_dir);
        if fs::copy(
            &toml_path,
            archive_config_dir.join(format!("config.toml.{timestamp}")),
        )
        .is_ok()
        {
            let _ = fs::remove_file(&toml_path);
            println!("Config archived to archive/config/config.toml.{timestamp}");
            archived = true;
        }
    }

    let env_path = base.join("config.env");
    if env_path.exists() {
        let _ = fs::create_dir_all(&archive_config_dir);
        if fs::copy(
            &env_path,
            archive_config_dir.join(format!("env.{timestamp}")),
        )
        .is_ok()
        {
            let _ = fs::remove_file(&env_path);
            if !archived {
                println!("Env archived to archive/config/env.{timestamp}");
            }
        }
    }

    if !archived {
        println!("No config file to reset");
    }
    0
}

pub(crate) fn clear_full_reset_artifacts() {
    let pidtrack = hcom_dir().join(".tmp").join("launched_pids.json");
    let _ = fs::remove_file(pidtrack);

    let device_id_file = hcom_dir().join(".tmp").join("device_id");
    let _ = fs::remove_file(&device_id_file);

    let instance_count_file = hcom_dir().join(FLAGS_DIR).join("instance_count");
    let _ = fs::remove_file(&instance_count_file);
}

pub(crate) fn bootstrap_fresh_db() {
    if let Ok(fresh_db) = HcomDb::open() {
        let _ = fresh_db.init_db();
        let _ = fresh_db.log_reset_event();
    }
}

pub(crate) fn print_archive_result(result: Result<Option<String>, String>) -> i32 {
    match result {
        Ok(Some(path)) => {
            println!("Archived to {}/", shorten_path(&path));
            println!("Started fresh NOMADTERM conversation");
            0
        }
        Ok(None) => {
            println!("No NOMADTERM conversation to clear");
            0
        }
        Err(e) => {
            eprintln!("Error: Failed to archive: {e}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_archive_timestamp_format() {
        let ts = get_archive_timestamp();
        assert!(ts.len() >= 15);
        assert!(ts.contains('-'));
        assert!(ts.contains('_'));
    }
}
