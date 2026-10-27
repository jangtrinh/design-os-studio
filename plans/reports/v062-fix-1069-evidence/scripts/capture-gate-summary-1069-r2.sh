#!/bin/bash
# Renders the 1069 round-2 evidence logs into a terminal-style page and captures it with headless Chrome.
# Usage: capture-gate-summary-1069-r2.sh <head sha> <pr number> [window height]
WT=/Users/jangtrinh/Products/design-os-studio/.claude/worktrees/agent-a1db67d4742d92bb1
EV="$WT/plans/reports/v062-fix-1069-evidence"
CHROME="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
mkdir -p "$WT/out"
python3 -I "$EV/scripts/render-gate-summary-1069-r2.py" "$EV" "$WT/out/gate-summary-1069-r2.html" "$1" "$2"
"$CHROME" --headless=new --disable-gpu --hide-scrollbars --user-data-dir="$WT/out/chrome-profile-1069" --window-size=1600,"${3:-1900}" \
  --screenshot="$EV/r2-gate-summary-capture.png" "file://$WT/out/gate-summary-1069-r2.html" 2>&1 | tail -2
rm -rf "$WT/out/chrome-profile-1069"
