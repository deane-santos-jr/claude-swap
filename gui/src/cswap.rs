use serde::Deserialize;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct CommandFailure {
    pub command: String,
    pub message: String,
}

impl std::fmt::Display for CommandFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.command, self.message)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ErrorPayload {
    pub error: ErrorBody,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ErrorBody {
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub message: String,
}

pub fn binary() -> String {
    std::env::var("CSWAP_BIN").unwrap_or_else(|_| "cswap".to_string())
}

fn describe(args: &[&str]) -> String {
    format!("cswap {}", args.join(" "))
}

pub fn run_json<T: for<'de> Deserialize<'de>>(args: &[&str]) -> Result<T, CommandFailure> {
    let output = Command::new(binary())
        .args(args)
        .output()
        .map_err(|e| CommandFailure { command: describe(args), message: e.to_string() })?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if let Ok(payload) = serde_json::from_str::<ErrorPayload>(&stdout) {
        if !payload.error.message.is_empty() || !payload.error.kind.is_empty() {
            return Err(CommandFailure {
                command: describe(args),
                message: format!("{} {}", payload.error.kind, payload.error.message).trim().to_string(),
            });
        }
    }
    if !output.status.success() {
        let message = if stderr.is_empty() { stdout.trim().to_string() } else { stderr };
        return Err(CommandFailure { command: describe(args), message });
    }
    serde_json::from_str::<T>(&stdout).map_err(|e| CommandFailure {
        command: describe(args),
        message: format!("unexpected output ({e}): {}", stdout.trim()),
    })
}

pub fn run_plain(args: &[&str]) -> Result<String, CommandFailure> {
    run_plain_with_input(args, "")
}

pub fn run_plain_with_input(args: &[&str], stdin_text: &str) -> Result<String, CommandFailure> {
    use std::io::Write;
    use std::process::Stdio;
    let mut child = Command::new(binary())
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| CommandFailure { command: describe(args), message: e.to_string() })?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(stdin_text.as_bytes());
    }
    let output = child
        .wait_with_output()
        .map_err(|e| CommandFailure { command: describe(args), message: e.to_string() })?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if output.status.success() {
        return Ok(if stdout.is_empty() { stderr } else { stdout });
    }
    Err(CommandFailure {
        command: describe(args),
        message: if stderr.is_empty() { stdout } else { stderr },
    })
}
