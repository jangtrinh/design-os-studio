#!/bin/bash
# Renders the 1072 evidence logs into a terminal-style page and captures it with headless Chrome.
# Usage: capture-gate-summary-1072.sh <head sha> <pr number> [window height]
WT=/Users/jangtrinh/Products/design-os-studio/.claude/worktrees/agent-a1db67d4742d92bb1
EV="$WT/plans/reports/v062-fix-1072-evidence"
CHROME="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
mkdir -p "$WT/out"
python3 -I "$EV/scripts/render-gate-summary-1072.py" "$EV" "$WT/out/gate-summary-1072.html" "$1" "$2"
"$CHROME" --headless=new --disable-gpu --hide-scrollbars --user-data-dir="$WT/out/chrome-profile-1072" --window-size=1600,"${3:-1900}" \
  --screenshot="$EV/gate-summary-capture.png" "file://$WT/out/gate-summary-1072.html" 2>&1 | tail -2
rm -rf "$WT/out/chrome-profile-1072"
