#!/bin/bash
# Renders the round-4 evidence logs into a terminal-style page and captures it with headless Chrome.
# Usage: capture-gate-summary-round4.sh <head sha> <pr number> [window height]
WT=/Users/jangtrinh/Products/design-os-studio/.claude/worktrees/agent-a1db67d4742d92bb1
EV="$WT/plans/reports/v062-fix-1061-evidence"
CHROME="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
python3 -I "$EV/scripts/render-gate-summary-round4.py" "$EV" "$WT/out/gate-summary-round4.html" "$1" "$2"
"$CHROME" --headless=new --disable-gpu --hide-scrollbars --user-data-dir="$WT/out/chrome-profile-round4" --window-size=1600,"${3:-1500}" \
  --screenshot="$EV/gate-summary-capture-round4.png" "file://$WT/out/gate-summary-round4.html" 2>&1 | tail -2
rm -rf "$WT/out/chrome-profile-round4"
