"""Ingest the rate-limit windows Claude Code hands to its status line.

Every reply Claude Code receives carries the account's live 5h/7d usage; the
status-line hook pipes that JSON here so the usage store (and with it the
dashboards and the auto-switcher) reflects the account as it is right now,
independent of the usage endpoint's own request budget.
"""

from __future__ import annotations

import json
import sys
from datetime import datetime, timezone

from claude_swap.paths import get_backup_root, get_global_config_path
from claude_swap.usage_store import FetchRecord, UsageStore


class LiveUsageError(Exception):
    pass


def _iso(resets_at: object) -> str | None:
    if isinstance(resets_at, (int, float)):
        return datetime.fromtimestamp(float(resets_at), tz=timezone.utc).isoformat()
    if isinstance(resets_at, str) and resets_at:
        return datetime.fromisoformat(resets_at.replace("Z", "+00:00")).isoformat()
    return None


def _window(raw: object) -> dict | None:
    if not isinstance(raw, dict) or "used_percentage" not in raw:
        return None
    window: dict = {"pct": float(raw["used_percentage"])}
    resets = _iso(raw.get("resets_at"))
    if resets:
        window["resets_at"] = resets
    return window


def windows_from_statusline(payload: dict) -> dict:
    limits = payload.get("rate_limits")
    if not isinstance(limits, dict):
        raise LiveUsageError("status-line payload carries no rate_limits")
    windows = {}
    five = _window(limits.get("five_hour"))
    seven = _window(limits.get("seven_day"))
    if five:
        windows["five_hour"] = five
    if seven:
        windows["seven_day"] = seven
    if not windows:
        raise LiveUsageError("rate_limits has neither five_hour nor seven_day")
    return windows


def current_identity() -> tuple[str, str]:
    path = get_global_config_path()
    if not path.exists():
        raise LiveUsageError(f"no Claude config at {path}")
    account = json.loads(path.read_text(encoding="utf-8")).get("oauthAccount") or {}
    email = account.get("emailAddress")
    if not email:
        raise LiveUsageError("Claude config has no oauthAccount.emailAddress")
    return email, account.get("organizationUuid") or ""


def slot_for(identity: tuple[str, str]) -> str:
    from claude_swap.switcher import ClaudeAccountSwitcher

    switcher = ClaudeAccountSwitcher()
    data = switcher._get_sequence_data_migrated() or {}
    slot = switcher._find_account_slot(data, identity[0], identity[1])
    if slot is None:
        raise LiveUsageError(f"{identity[0]} is not a managed account")
    return slot


def merged_usage(previous: dict | None, windows: dict) -> dict:
    usage = dict(previous or {})
    usage.update(windows)
    return usage


def ingest(payload: dict) -> str:
    windows = windows_from_statusline(payload)
    identity = current_identity()
    slot = slot_for(identity)
    store = UsageStore(get_backup_root() / "cache")
    previous = store.entries({slot: identity})[slot].last_good
    record = FetchRecord(usage=merged_usage(previous, windows))
    accepted = store.record({slot: record}, {slot: identity})
    if slot not in accepted:
        raise LiveUsageError("a live fetch holds this slot; try again later")
    return slot


def main(argv: list[str]) -> int:
    if argv and argv[0] in ("-h", "--help"):
        print("usage: cswap ingest-usage < statusline.json", file=sys.stderr)
        return 0
    try:
        payload = json.load(sys.stdin)
    except json.JSONDecodeError as e:
        print(f"ingest-usage: stdin is not JSON: {e}", file=sys.stderr)
        return 1
    try:
        slot = ingest(payload)
    except LiveUsageError as e:
        print(f"ingest-usage: {e}", file=sys.stderr)
        return 2
    print(f"recorded live usage for account {slot}")
    return 0
