use std::process::Command;

fn applescript_quote(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

pub fn open_in_terminal(command_line: &str) -> Result<(), String> {
    let script = format!(
        "tell application \"Terminal\"\nactivate\ndo script \"{}\"\nend tell",
        applescript_quote(command_line)
    );
    let output = Command::new("osascript")
        .args(["-e", &script])
        .output()
        .map_err(|e| e.to_string())?;
    if output.status.success() {
        return Ok(());
    }
    Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
}

pub fn command_in_dir(dir: &str, program_and_args: &[String]) -> String {
    let quoted: Vec<String> = program_and_args.iter().map(|a| shell_quote(a)).collect();
    format!("cd {} && {}", shell_quote(dir), quoted.join(" "))
}
