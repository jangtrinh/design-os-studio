#!/bin/zsh
# Usage: capture-png.sh <page.html> <out.png> <width> <height>
# Headless Chrome screenshot of a local HTML page, with a throwaway profile that is removed afterwards.
set -u
HTML="$1"; PNG="$2"; W="$3"; H="$4"
PROFILE="$(mktemp -d "${TMPDIR:-/tmp}/chrome-profile.XXXXXX")"
"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --headless=new --disable-gpu --hide-scrollbars --no-first-run \
  --user-data-dir="$PROFILE" --window-size="$W,$H" --screenshot="$PNG" "file://$HTML" > /dev/null 2>&1
rc=$?
rm -rf "$PROFILE"
echo "exit=$rc profile_removed=$([ -e "$PROFILE" ] && echo no || echo yes)"
