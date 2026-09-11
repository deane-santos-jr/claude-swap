use crate::cswap;
use std::path::PathBuf;
use std::process::Command;

const LABEL: &str = "com.cswap.auto";

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into()))
}

fn plist_path() -> PathBuf {
    home().join("Library/LaunchAgents").join(format!("{LABEL}.plist"))
}

pub fn log_path() -> PathBuf {
    home().join("Library/Logs").join(format!("{LABEL}.log"))
}

fn domain() -> String {
    let uid = String::from_utf8_lossy(&Command::new("id").arg("-u").output().map(|o| o.stdout).unwrap_or_default())
        .trim()
        .to_string();
    format!("gui/{uid}")
}

fn cswap_path() -> Result<String, String> {
    let output = Command::new("sh")
        .args(["-c", &format!("command -v {}", cswap::binary())])
        .output()
        .map_err(|e| e.to_string())?;
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() {
        return Err("cswap is not on PATH".into());
    }
    Ok(path)
}

fn plist(cswap: &str) -> String {
    let log = log_path().display().to_string();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>{LABEL}</string>
  <key>ProgramArguments</key>
  <array><string>{cswap}</string><string>auto</string></array>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><true/>
  <key>ProcessType</key><string>Background</string>
  <key>StandardOutPath</key><string>{log}</string>
  <key>StandardErrorPath</key><string>{log}</string>
</dict>
</plist>
"#
    )
}

fn launchctl(args: &[&str]) -> Result<String, String> {
    let output = Command::new("launchctl").args(args).output().map_err(|e| e.to_string())?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    if output.status.success() {
        return Ok(stdout);
    }
    Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
}


pub fn is_running() -> bool {
    launchctl(&["print", &format!("{}/{LABEL}", domain())])
        .map(|out| out.contains("state = running"))
        .unwrap_or(false)
}

pub fn install() -> Result<(), String> {
    let cswap = cswap_path()?;
    let path = plist_path();
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(&path, plist(&cswap)).map_err(|e| e.to_string())?;
    let _ = launchctl(&["bootout", &format!("{}/{LABEL}", domain())]);
    launchctl(&["bootstrap", &domain(), &path.display().to_string()])?;
    launchctl(&["kickstart", "-k", &format!("{}/{LABEL}", domain())])?;
    Ok(())
}

pub fn uninstall() -> Result<(), String> {
    let _ = launchctl(&["bootout", &format!("{}/{LABEL}", domain())]);
    let path = plist_path();
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn last_log_line() -> Option<String> {
    let text = std::fs::read_to_string(log_path()).ok()?;
    text.lines().rev().find(|l| !l.trim().is_empty()).map(|l| l.trim().to_string())
}
