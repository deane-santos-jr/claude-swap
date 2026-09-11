#!/bin/zsh
set -u
input=$(cat)
BASE_FILE="$HOME/.claude-swap-backup/statusline-base"
STAMP_FILE="$HOME/.claude-swap-backup/cache/.live-usage-stamp"
CSWAP="${CSWAP_BIN:-$HOME/.local/bin/cswap}"

limits=$(printf '%s' "$input" | jq -c '.rate_limits // empty' 2>/dev/null)
if [[ -n $limits && $limits != null ]]; then
  stamp=$(printf '%s' "$limits" | md5)
  if [[ ! -f $STAMP_FILE || $(<"$STAMP_FILE") != "$stamp" ]]; then
    print -r -- "$stamp" > "$STAMP_FILE"
    (printf '%s' "$input" | "$CSWAP" ingest-usage >/dev/null 2>&1 &)
  fi
fi

if [[ -f $BASE_FILE ]]; then
  base=$(<"$BASE_FILE")
  [[ -n $base ]] && printf '%s' "$input" | eval "$base"
fi
