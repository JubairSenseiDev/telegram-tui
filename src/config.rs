use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

pub struct Config {
    pub dir: PathBuf,
    pub api_id: Option<i32>,
    pub api_hash: Option<String>,
    pub last_session: Option<String>,
}

#[cfg(unix)]
fn lock_down(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn lock_down(_path: &Path) {}

impl Config {
    pub fn load() -> Result<Self> {
        let dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("telegram-tui");
        fs::create_dir_all(&dir)?;

        let env_path = dir.join(".env");
        if env_path.exists() {
            let _ = dotenvy::from_path(&env_path);
            lock_down(&env_path);
        }

        let api_id = std::env::var("TELEGRAM_API_ID")
            .ok()
            .and_then(|v| v.trim().parse().ok());
        let api_hash = std::env::var("TELEGRAM_API_HASH")
            .ok()
            .filter(|s| !s.is_empty());
        let last_session = std::env::var("TELEGRAM_TUI_SESSION")
            .ok()
            .filter(|s| !s.is_empty())
            .or_else(|| read_last_session(&dir));

        Ok(Self {
            dir,
            api_id,
            api_hash,
            last_session,
        })
    }

    pub fn save_credentials(&self, api_id: i32, api_hash: &str) -> Result<()> {
        let path = self.dir.join(".env");
        fs::write(
            &path,
            format!("TELEGRAM_API_ID={}\nTELEGRAM_API_HASH={}\n", api_id, api_hash),
        )?;
        lock_down(&path);
        Ok(())
    }

    /// Remember the active account so the next launch reconnects to it.
    pub fn set_last_session(&mut self, name: &str) {
        self.last_session = Some(name.to_string());
        let path = self.dir.join("last_session");
        if fs::write(&path, name).is_ok() {
            lock_down(&path);
        }
    }

    pub fn clear_last_session(&mut self) {
        self.last_session = None;
        let _ = fs::remove_file(self.dir.join("last_session"));
    }

    pub fn session_path(&self, name: &str) -> PathBuf {
        let sessions = self.dir.join("sessions");
        fs::create_dir_all(&sessions).ok();
        let path = sessions.join(format!("{}.session", name));
        if path.exists() {
            lock_down(&path);
        }
        path
    }

    pub fn list_sessions(&self) -> Vec<String> {
        let sessions = self.dir.join("sessions");
        let mut out = Vec::new();
        if let Ok(read) = fs::read_dir(&sessions) {
            for entry in read.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if let Some(stripped) = name.strip_suffix(".session") {
                    out.push(stripped.to_string());
                }
            }
        }
        out.sort();
        out
    }

    pub fn remove_session(&mut self, name: &str) {
        let _ = fs::remove_file(self.session_path(name));
        if self.last_session.as_deref() == Some(name) {
            self.clear_last_session();
        }
    }

    pub fn exports_dir(&self) -> PathBuf {
        let exports = self.dir.join("exports");
        fs::create_dir_all(&exports).ok();
        exports
    }

    pub fn downloads_dir(&self) -> PathBuf {
        let downloads = self.dir.join("downloads");
        fs::create_dir_all(&downloads).ok();
        downloads
    }

    pub fn list_downloads(&self) -> Vec<PathBuf> {
        let mut out = Vec::new();
        if let Ok(read) = fs::read_dir(self.downloads_dir()) {
            for entry in read.flatten() {
                out.push(entry.path());
            }
        }
        out.sort();
        out
    }

    pub fn list_exports(&self) -> Vec<PathBuf> {
        let mut out = Vec::new();
        if let Ok(read) = fs::read_dir(self.exports_dir()) {
            for entry in read.flatten() {
                out.push(entry.path());
            }
        }
        out.sort();
        out
    }
}

fn read_last_session(dir: &Path) -> Option<String> {
    fs::read_to_string(dir.join("last_session"))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}
