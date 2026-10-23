#!/usr/bin/env bash
# Capture the high-resolution Retina 2x visual proof screenshots for design:os studio.
# Runs headlessly via `--screenshot <path>` with `with_active(false)`.
#
# Every capture is made from the Untitled UI PRO v8.0 kit, never from a built-in fixture, a blank
# file or a client file. Point DESIGNOS_PROOF_DOC at a .designos copy of the kit:
#
#   DESIGNOS_PROOF_DOC=/path/to/untitled-ui-kit.designos scripts/capture-visual-proof.sh
#
# Make the copy once from your licensed kit .fig (the original is only read, never written):
# run `designos-mcp --workspace <dir holding the .fig>`, call `designos_open_file` with the .fig and
# `designos_save_file` with a new .designos name (or open the .fig in Studio and Save as .designos).
#
# Optional: DESIGNOS_PROOF_OUT (default docs/images) is the output folder, and
# DESIGNOS_PROOF_ONLY="designos-hero,v0.3/text-wrap-balance" re-captures only those names.
# The designos-agents-chat step starts a live agent session; leave it out of ONLY to skip it.
# Captures that need an edited kit (interaction editor, agent spec, wrap, balanced text) edit a
# temporary copy.

set -euo pipefail

export PATH="$HOME/.cargo/bin:$PATH"

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

DOC="${DESIGNOS_PROOF_DOC:-}"
if [[ -z "$DOC" ]]; then
  echo "error: DESIGNOS_PROOF_DOC is not set." >&2
  echo "       Set it to a .designos copy of the Untitled UI PRO v8.0 kit; see the header of $0." >&2
  exit 2
fi
if [[ ! -f "$DOC" || "$DOC" != *.designos ]]; then
  echo "error: DESIGNOS_PROOF_DOC must name an existing .designos file, got: $DOC" >&2
  exit 2
fi
DOC="$(cd "$(dirname "$DOC")" && pwd)/$(basename "$DOC")"

OUT="${DESIGNOS_PROOF_OUT:-docs/images}"
BIN="${CARGO_TARGET_DIR:-$REPO_ROOT/target}/release/designos-studio"
if [[ ! -x "$BIN" ]]; then
  echo "==> Building release binary ($BIN)..."
  cargo build --release -p designos-app
fi

WORK="$(mktemp -d "${TMPDIR:-/tmp}/designos-proof.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT
mkdir -p "$OUT/v0.3"

want() { [[ -z "${DESIGNOS_PROOF_ONLY:-}" || ",$DESIGNOS_PROOF_ONLY," == *",$1,"* ]]; }

# capture NAME DOC|- [studio args...]: writes $OUT/NAME.png (NAME may include a folder) from DOC,
# retrying a window that never delivered a frame. DOC "-" opens no file.
capture() {
  local name="$1" doc="$2" file attempt
  want "$name" || return 0
  shift 2
  file="$OUT/$name.png"
  [[ "$doc" == "-" ]] && doc=()
  echo "==> $file"
  for attempt in 1 2 3; do
    rm -f "$file"
    if "$BIN" ${doc:+"$doc"} "$@" --scale "${CAPTURE_SCALE:-2}" --no-toast --screenshot "$file" && [[ -s "$file" ]]; then
      return 0
    fi
    echo "    attempt $attempt did not produce $file" >&2
  done
  echo "error: could not capture $file" >&2
  exit 1
}

DASHBOARD='Desktop'  # the kit's 1440 x 960 dashboard screen (lowest id top-level frame of that name)

capture designos-hero "$DOC" --select "$DASHBOARD" --zoom 55 --size 1440x900
want designos-hero && cp "$OUT/designos-hero.png" "$OUT/cover.png"
capture designos-agents-chat "$DOC" --select "Avatar group" --zoom 100 --panel agents-chat --size 1440x900
capture designos-assets-panel "$DOC" --select "$DASHBOARD" --zoom 55 --panel assets --size 1440x900
capture designos-actions-cmdk "$DOC" --select "$DASHBOARD" --zoom 55 --panel cmdk --size 1440x900
capture designos-devmode "$DOC" --select "Avatar group" --zoom 100 --dev react --size 1440x900
capture designos-variables "$DOC" --panel variables --size 1440x900
capture designos-components "$DOC" --select "Buttons/Button" --zoom 35 --size 1440x900
capture designos-main-menu "$DOC" --select "$DASHBOARD" --zoom 55 --menu main --size 1440x900
capture designos-color-picker "$DOC" --select "Theme=Gradient strip, Lines=False" --zoom 150 --panel picker-gradient --size 1440x900
capture designos-overview "$DOC" --select none --rulers --size 1440x900
# The interaction editor of the one kit component that carries an interaction (On click, Scroll to
# Button). `--panel interaction` opens the editor on the layer named "Add", and the kit has none, so
# a temporary copy renames that component to "Add"; the interaction itself is the kit's own.
INTERACTION_DOC="$WORK/kit-interaction.designos"
if want designos-prototype; then
  python3 - "$DOC" "$INTERACTION_DOC" <<'PY'
import re, sys
src, dst = sys.argv[1:3]
data = open(src, "rb").read()
needle = b'"name":"State=Open inactive filter, Orientation=Left aligned"'
node = re.compile(rb'"(\d+)":\{"id":\1,')
pos = 0
while True:
    at = data.find(needle, pos)
    if at < 0:
        sys.exit("error: the document is not the Untitled UI PRO v8.0 kit (the component with the interaction was not found)")
    starts = list(node.finditer(data, max(0, at - 400), at))
    begin = starts[-1].start() if starts else -1
    following = node.search(data, at + len(needle), at + 12000)
    layer = data[begin:following.start() if following else at + 12000]
    if begin >= 0 and b'"reactions":[{' in layer:
        break
    pos = at + 1
open(dst, "wb").write(data[:at] + b'"name":"Add"' + data[at + len(needle):])
PY
fi
capture designos-prototype "$INTERACTION_DOC" --select "Add" --panel interaction --tab prototype --zoom 150 --size 1440x900

# Home: the kit copy stored as a project of a temporary library (a symlink, nothing is copied).
# The app skips thumbnails for files over 25,000 nodes, so the card shows the generic file icon.
if want designos-home; then
  python3 - "$WORK/lib" "$DOC" <<'PY'
import json, os, sys, time
lib, doc = sys.argv[1:3]
now = int(time.time() * 1000)
pid, fid = "18e1000000000000a000", "18e1000000000000a001"
pdir = os.path.join(lib, "projects", pid)
os.makedirs(os.path.join(pdir, "files"))
os.symlink(doc, os.path.join(pdir, "files", fid + ".designos"))
entry = {"id": fid, "project": pid, "folder": None, "name": "Untitled UI Figma – PRO VARIABLES (v8.0)", "created": now, "modified": now, "opened": now, "starred": False}
json.dump({"id": pid, "name": "Untitled UI", "folders": [], "created": now, "modified": now, "files": [entry]}, open(os.path.join(pdir, "project.json"), "w"), indent=2)
PY
fi
capture designos-home - --library "$WORK/lib" --home --size 1440x900

# Agent CLI: a kit frame exported as a NodeSpec, edited and applied back, so the capture shows
# Auto Layout 5.0 built from a spec. The frame ids belong to the Untitled UI PRO v8.0 import, and
# the check below refuses another file.
SPEC_DOC="$WORK/kit-with-specs.designos"
if want designos-agent-spec || want v0.3/auto-layout-space-evenly-vertical-wrap; then
  "$BIN" "$DOC" --select 81249 --export-spec "$WORK/badges.spec.json" >/dev/null
  "$BIN" "$DOC" --select 46971 --export-spec "$WORK/metrics.spec.json" >/dev/null
  python3 - "$WORK" <<'PY'
import json, sys
work = sys.argv[1]
def strip(node, keep_origin):
    node.pop("id", None)
    if not keep_origin:
        node.pop("x", None)
        node.pop("y", None)
    for child in node.get("children", []):
        strip(child, False)
badges = json.load(open(f"{work}/badges.spec.json"))
metrics = json.load(open(f"{work}/metrics.spec.json"))
if badges.get("name") != "Badges" or metrics.get("name") != "Metric group":
    sys.exit("error: the document is not the Untitled UI PRO v8.0 kit (expected the frames 'Badges' and 'Metric group')")
strip(badges, True)
strip(metrics, True)
layout = {"direction": "column", "wrap": True, "gap": 24, "counter_gap": 24, "padding": [0, 0, 0, 0], "primary_align": "space-evenly", "counter_align": "start"}
badges.update(name="Badges (space-evenly wrap)", x=-520, y=0, width=300, height=150, width_sizing="fixed", height_sizing="fixed", fill="#FFFFFF", layout=layout)
metrics.update(name="Metric group (from NodeSpec)", x=-1200, y=0)
json.dump(badges, open(f"{work}/badges-wrap.spec.json", "w"), indent=2)
json.dump(metrics, open(f"{work}/metrics-from-spec.spec.json", "w"), indent=2)
PY
  # 79550 is the Modals page and 44616 the Dashboards 01 page of the kit
  "$BIN" "$DOC" --select 79550 --apply-spec "$WORK/badges-wrap.spec.json" --out "$SPEC_DOC" >/dev/null
  "$BIN" "$SPEC_DOC" --select 44616 --apply-spec "$WORK/metrics-from-spec.spec.json" >/dev/null
fi
capture designos-agent-spec "$SPEC_DOC" --select "Metric group (from NodeSpec)" --zoom 100 --size 1440x900

capture v0.3/auto-layout-space-evenly-vertical-wrap "$SPEC_DOC" --select "Badges (space-evenly wrap)" --zoom 200 --size 1440x900
# Dev Mode shots: the Code section sits far down the inspector. A window taller than the display
# (about 1040 points on a 1080p one) is clamped and the PNG silently resized, so these use scale 1.5
# on a 1340 point layout (a 1005 point window, a 1620 x 2010 PNG) and show about 25 lines of code.
CAPTURE_SCALE=1.5 capture v0.3/dev-mode-android-xml "$DOC" --select "Cards" --zoom 120 --dev android-xml --size 1080x1340
CAPTURE_SCALE=1.5 capture v0.3/dev-mode-uikit "$DOC" --select "Cards" --zoom 120 --dev uikit --size 1080x1340
# The effect style picker of a kit shadow card, and the grid style picker of a kit MacBook Pro frame.
capture v0.3/styles-edit-effect-style "$DOC" --select "_Shadow card" --zoom 100 --panel effect-styles --size 1440x900
capture v0.3/styles-effect-and-grid-styles "$DOC" --select "MacBook Pro" --zoom 50 --panel grid-styles --size 1440x900

# A kit heading set to text-wrap Balance, edited in a temporary copy of the kit.
BALANCED_DOC="$WORK/kit-balanced.designos"
if want v0.3/text-wrap-balance; then
  python3 - "$DOC" "$BALANCED_DOC" <<'PY'
import re, sys
src, dst = sys.argv[1:3]
data = open(src, "rb").read()
needle = b'"content":"We design digital experiences that create more happy in the world"'
node = re.compile(rb'"(\d+)":\{"id":\1,')
pos = 0
while True:
    content = data.find(needle, pos)
    if content < 0:
        sys.exit("error: the document is not the Untitled UI PRO v8.0 kit (the 343 px wide heading was not found)")
    starts = list(node.finditer(data, max(0, content - 8000), content))
    begin = starts[-1].start() if starts else -1
    following = node.search(data, begin + 20, begin + 8000) if begin >= 0 else None
    layer = data[begin:following.start() if following else begin + 8000]
    if begin >= 0 and b'"size":{"x":343.0,' in layer:
        break
    pos = content + 1
end = content + 200
win = data[begin:end]
name = re.search(rb'"name":"(?:[^"\\]|\\.)*"', win)
text_open = win.find(b'"Text":{')
if not name or text_open < 0 or name.end() > text_open:
    sys.exit("error: heading layer not found in the expected shape")
text_open += len(b'"Text":{')
patched = win[:name.start()] + b'"name":"Balanced headline"' + win[name.end():text_open] + b'"text_wrap":"Balance",' + win[text_open:]
open(dst, "wb").write(data[:begin] + patched + data[end:])
PY
fi
capture v0.3/text-wrap-balance "$BALANCED_DOC" --select "Balanced headline" --zoom 80 --panel type-details --size 1440x900

echo "==> Done. Captures are in $OUT."
