use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
pub struct DesktopSession {
    pub pid: u32,
    #[serde(rename = "sessionId")]
    pub session_id: String,
    #[serde(default)]
    pub name: String,
    pub cwd: String,
    #[serde(rename = "startedAt", default)]
    pub started_at_ms: i64,
    #[serde(default)]
    pub entrypoint: String,
}

impl DesktopSession {
    pub fn is_alive(&self) -> bool {
        std::process::Command::new("kill")
            .args(["-0", &self.pid.to_string()])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    pub fn short_folder(&self) -> String {
        let home = std::env::var("HOME").unwrap_or_default();
        self.cwd.replacen(&home, "~", 1)
    }
}

fn sessions_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".claude").join("sessions")
}

pub fn list_desktop_sessions() -> Vec<DesktopSession> {
    let Ok(entries) = std::fs::read_dir(sessions_dir()) else {
        return Vec::new();
    };
    let mut sessions: Vec<DesktopSession> = entries
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| std::fs::read_to_string(e.path()).ok())
        .filter_map(|text| serde_json::from_str::<DesktopSession>(&text).ok())
        .filter(|s| s.entrypoint == "claude-desktop")
        .collect();
    sessions.sort_by_key(|s| std::cmp::Reverse(s.started_at_ms));
    sessions
}
