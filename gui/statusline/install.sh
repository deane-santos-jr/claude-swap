#!/bin/zsh
set -eu -o pipefail
HERE="${0:A:h}"
HOOK="$HERE/cswap-live-usage.sh"
SETTINGS="$HOME/.claude/settings.json"
BASE_FILE="$HOME/.claude-swap-backup/statusline-base"
mkdir -p "$HOME/.claude-swap-backup/cache"
chmod +x "$HOOK"
current=$(jq -r '.statusLine.command // empty' "$SETTINGS" 2>/dev/null || true)
if [[ $current != "$HOOK" ]]; then
  print -r -- "$current" > "$BASE_FILE"
  cp "$SETTINGS" "$SETTINGS.pre-cswap-live.bak"
  tmp=$(mktemp)
  jq --arg c "$HOOK" '.statusLine = ((.statusLine // {}) + {type: "command", command: $c})' "$SETTINGS" > "$tmp" && mv "$tmp" "$SETTINGS"
fi
print -- "status line now feeds live usage into cswap (wrapping: ${current:-nothing})"
