use crate::cswap;
use crate::launchd;
use crate::model::{AutoEvent, ListPayload};
use crate::terminal;
use std::sync::mpsc::Sender;

#[derive(Debug, Clone)]
pub enum Action {
    Refresh,
    SwitchTo(u32),
    Rotate,
    SwitchBest,
    SwitchNextAvailable,
    AddCurrentLogin,
    Remove(u32),
    Disable(u32),
    Enable(u32),
    OpenTerminal(u32),
    ResumeDesktopSession { account: u32, session_id: String, cwd: String },
    AutoOnce { dry_run: bool },
    SetThreshold(f64),
    EnableBackgroundAuto,
    DisableBackgroundAuto,
}

impl Action {
    pub fn label(&self) -> String {
        match self {
            Action::Refresh => "refreshing usage".into(),
            Action::SwitchTo(n) => format!("switching to account {n}"),
            Action::Rotate => "rotating to the next account".into(),
            Action::SwitchBest => "switching to the account with the most quota".into(),
            Action::SwitchNextAvailable => "switching to the next account that is not limited".into(),
            Action::AddCurrentLogin => "adding the current login".into(),
            Action::Remove(n) => format!("removing account {n}"),
            Action::Disable(n) => format!("holding account {n} out of rotation"),
            Action::Enable(n) => format!("returning account {n} to rotation"),
            Action::OpenTerminal(n) => format!("opening a terminal as account {n}"),
            Action::ResumeDesktopSession { account, .. } => format!("resuming the desktop session as account {account}"),
            Action::AutoOnce { dry_run: true } => "checking what auto-switch would do".into(),
            Action::AutoOnce { dry_run: false } => "running an auto-switch check".into(),
            Action::SetThreshold(t) => format!("setting the auto-switch threshold to {t:.0}%"),
            Action::EnableBackgroundAuto => "starting the background auto-switcher".into(),
            Action::DisableBackgroundAuto => "stopping the background auto-switcher".into(),
        }
    }

    pub fn reloads_accounts(&self) -> bool {
        !matches!(
            self,
            Action::OpenTerminal(_)
                | Action::ResumeDesktopSession { .. }
                | Action::SetThreshold(_)
                | Action::EnableBackgroundAuto
                | Action::DisableBackgroundAuto
        )
    }
}

#[derive(Debug)]
pub enum Outcome {
    Accounts(ListPayload),
    Notice(String),
    AutoEvents(Vec<AutoEvent>),
    Failure(String),
    Done(Action),
}

pub fn spawn(action: Action, tx: Sender<Outcome>) {
    std::thread::spawn(move || {
        let result = perform(&action);
        match result {
            Ok(outcome) => {
                if let Some(o) = outcome {
                    let _ = tx.send(o);
                }
            }
            Err(message) => {
                let _ = tx.send(Outcome::Failure(message));
            }
        }
        if action.reloads_accounts() {
            match cswap::run_json::<ListPayload>(&["list", "--json"]) {
                Ok(payload) => {
                    let _ = tx.send(Outcome::Accounts(payload));
                }
                Err(e) => {
                    let _ = tx.send(Outcome::Failure(e.to_string()));
                }
            }
        }
        let _ = tx.send(Outcome::Done(action));
    });
}

fn perform(action: &Action) -> Result<Option<Outcome>, String> {
    match action {
        Action::Refresh => Ok(None),
        Action::SwitchTo(n) => switch(&[&n.to_string()]),
        Action::Rotate => switch(&[]),
        Action::SwitchBest => switch(&["--strategy", "best"]),
        Action::SwitchNextAvailable => switch(&["--strategy", "next-available"]),
        Action::AddCurrentLogin => plain(&["add"]),
        Action::Remove(n) => plain(&["remove", &n.to_string()]),
        Action::Disable(n) => plain(&["disable", &n.to_string()]),
        Action::Enable(n) => plain(&["enable", &n.to_string()]),
        Action::OpenTerminal(n) => {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
            let line = terminal::command_in_dir(&home, &[cswap::binary(), "run".into(), n.to_string()]);
            terminal::open_in_terminal(&line)?;
            Ok(Some(Outcome::Notice(format!("Terminal opened as account {n}"))))
        }
        Action::ResumeDesktopSession { account, session_id, cwd } => {
            let line = terminal::command_in_dir(
                cwd,
                &[
                    cswap::binary(),
                    "run".into(),
                    account.to_string(),
                    "--share-history".into(),
                    "--".into(),
                    "--resume".into(),
                    session_id.clone(),
                ],
            );
            terminal::open_in_terminal(&line)?;
            Ok(Some(Outcome::Notice("Terminal opened; close the limited desktop session once it has picked up".into())))
        }
        Action::AutoOnce { dry_run } => auto_once(*dry_run),
        Action::SetThreshold(t) => plain(&["config", "set", "autoswitch.threshold", &format!("{t:.0}")]),
        Action::EnableBackgroundAuto => {
            launchd::install()?;
            Ok(Some(Outcome::Notice("Auto-switch now runs in the background and starts at login".into())))
        }
        Action::DisableBackgroundAuto => {
            launchd::uninstall()?;
            Ok(Some(Outcome::Notice("Background auto-switch stopped".into())))
        }
    }
}

fn switch(extra: &[&str]) -> Result<Option<Outcome>, String> {
    let mut args = vec!["switch"];
    args.extend_from_slice(extra);
    args.push("--json");
    let value: serde_json::Value = cswap::run_json(&args).map_err(|e| e.to_string())?;
    let switched = value.get("switched").and_then(|v| v.as_bool()).unwrap_or(true);
    let to = value
        .get("to")
        .and_then(|v| v.get("email").and_then(|e| e.as_str()))
        .map(str::to_string);
    let reason = value.get("reason").and_then(|v| v.as_str()).map(str::to_string);
    let text = match (switched, to, reason) {
        (true, Some(email), _) => format!("Now on {email}. Running Claude Code sessions pick it up within ~30s."),
        (true, None, _) => "Switched.".to_string(),
        (false, _, Some(reason)) => format!("Not switched: {reason}"),
        (false, _, None) => "Not switched.".to_string(),
    };
    Ok(Some(Outcome::Notice(text)))
}

fn plain(args: &[&str]) -> Result<Option<Outcome>, String> {
    let text = cswap::run_plain(args).map_err(|e| e.to_string())?;
    Ok(Some(Outcome::Notice(text)))
}

fn auto_once(dry_run: bool) -> Result<Option<Outcome>, String> {
    let mut args = vec!["auto", "--once", "--json"];
    if dry_run {
        args.push("--dry-run");
    }
    let output = std::process::Command::new(cswap::binary())
        .args(&args)
        .output()
        .map_err(|e| e.to_string())?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let events: Vec<AutoEvent> = stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<AutoEvent>(line).ok())
        .collect();
    if events.is_empty() && !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(Some(Outcome::AutoEvents(events)))
}
