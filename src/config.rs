use anyhow::Result;
use std::fs;
use std::path::PathBuf;

pub struct Config {
    pub dir: PathBuf,
    pub api_id: Option<i32>,
    pub api_hash: Option<String>,
    pub last_session: Option<String>,
}

impl Config {
    pub fn load() -> Result<Self> {
        let dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("telegram-tui");
        fs::create_dir_all(&dir)?;

        let env_path = dir.join(".env");
        if env_path.exists() {
            let _ = dotenvy::from_path(&env_path);
        }

        let api_id = std::env::var("TELEGRAM_API_ID")
            .ok()
            .and_then(|v| v.trim().parse().ok());
        let api_hash = std::env::var("TELEGRAM_API_HASH")
            .ok()
            .filter(|s| !s.is_empty());
        let last_session = std::env::var("TELEGRAM_TUI_SESSION").ok();

        Ok(Self {
            dir,
            api_id,
            api_hash,
            last_session,
        })
    }

    pub fn save_credentials(&self, api_id: i32, api_hash: &str) -> Result<()> {
        fs::write(
            self.dir.join(".env"),
            format!("TELEGRAM_API_ID={}\nTELEGRAM_API_HASH={}\n", api_id, api_hash),
        )?;
        Ok(())
    }

    pub fn session_path(&self, name: &str) -> PathBuf {
        let sessions = self.dir.join("sessions");
        fs::create_dir_all(&sessions).ok();
        sessions.join(format!("{}.session", name))
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

    pub fn remove_session(&self, name: &str) {
        let _ = fs::remove_file(self.session_path(name));
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
