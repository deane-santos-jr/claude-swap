# Claude Swap desktop app

A native macOS window for claude-swap. Every account's 5-hour, 7-day, per-model and extra-usage meters, one-click switching, an auto-switch toggle, and a way to rescue a Claude desktop app session that hit its limit.

It is a thin shell: every action runs the `cswap` CLI (`cswap list --json`, `cswap switch N --json`, `cswap auto --once --json`, …), so the CLI, TUI, menu bar and this window always agree.

## Build

Needs Rust 1.80+ and a working `cswap` on `PATH` (or set `CSWAP_BIN`).

```bash
cd gui
./bundle.sh
cp -R "target/Claude Swap.app" ~/Applications/
```

`bundle.sh` builds a release binary and wraps it in `Claude Swap.app`. The bundle's `PATH` includes `~/.local/bin`, `/opt/homebrew/bin` and `/usr/local/bin`, which is where `uv tool`, `pipx` and a venv symlink usually put `cswap`.

## What each control does

- **Switch** makes that account the login every Claude Code terminal and the VS Code extension use. Running sessions pick it up within about 30 seconds.
- **Terminal** opens a Terminal window running Claude Code as that account only (`cswap run N`), leaving the active login alone.
- **Hold out / Rejoin rotation** keeps an account out of auto-switch and rotate (`cswap disable` / `enable`).
- **Rotate / Most quota / Next not limited** are `cswap switch` with the matching strategy.
- **Auto-switch in the background** installs a login item (`~/Library/LaunchAgents/com.cswap.auto.plist`) that runs `cswap auto` permanently, so it keeps working with the window closed and after a reboot. It polls every minute and moves the active login to the account with the most quota before the current one reaches the threshold. The threshold slider writes `autoswitch.threshold`. **Check now** runs one decision immediately. Its log is `~/Library/Logs/com.cswap.auto.log`.
- **Add current login** snapshots whatever account Claude Code is logged in as right now (`cswap add`). Run `/login` in Claude Code first to add a different account.
- **Resume in Terminal** takes a Claude desktop app Code session and reopens it in Terminal under the chosen account (`cswap run N --share-history -- --resume <id>`). The desktop app keeps its own login and cannot be switched from outside.

Usage numbers refresh every minute and after every action. Each card says when its numbers were measured.

## Keeping the numbers live

Anthropic's usage endpoint has a small request budget and answers HTTP 429 for up to an hour once it is spent, at which point cswap can only serve its last measurement. Claude Code, however, receives the account's live 5-hour and 7-day usage with every reply and hands it to the status line. `statusline/install.sh` wraps your status-line command with a hook that pipes that JSON into `cswap ingest-usage`, which writes it into the same cache the window, the CLI and the auto-switcher read. From then on the active account is exact to the last reply, and the endpoint is only needed for idle accounts.

```bash
gui/statusline/install.sh
```

The hook runs `cswap ingest-usage` only when the numbers change, in the background, so status-line rendering stays instant.

## Xirp, VS Code and other hosts that spawn `claude` themselves

They all read the default login from the macOS Keychain, so a switch reaches every running session within about 30 seconds without a restart or a `--resume`. Nothing has to be configured per host. The one exception is the Claude desktop app, which signs in on its own; use **Resume in Terminal** for those sessions.
