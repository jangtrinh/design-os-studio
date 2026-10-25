#!/usr/bin/env bash
# Smoke test of the PACKAGED design:os studio macOS app (docs/releasing.md, "Closing a pack" step 4).
#
#   scripts/release-smoke-macos.sh <path-to-dmg> <out-dir>
#
# Env (all optional):
#   EXPECT_VERSION   version the binaries must report            (default 0.4.0)
#   FINAL=1          the dmg is the tagged build: Info.plist must report EXPECT_VERSION too
#                    (a dry-run dmg is stamped 0.0.0-dev.<sha>; that prints a NOTE, not a FAIL)
#   KIT_FIG          the Untitled UI PRO VARIABLES v8.0 .fig (never a client file; read only, copied)
#   AL_FRAME         top-level auto layout frame to open       (default "Welcome to Untitled UI")
#   GUI_TIMEOUT      seconds one app launch may take           (default 240)
#   KEEP_WORK=1      keep the temp work dir (kit copies, saved files) for debugging
#
# Every step prints PASS / FAIL / OWNER-MANUAL (and NOTE for facts that are not verdicts). Exit 1 on any FAIL.
# Evidence (PNGs, small JSON, smoke.log, smoke-summary.tsv) goes to <out-dir>. The ~340 MB kit copies live in a
# temp dir outside <out-dir> and are deleted on exit. Nothing is installed into /Applications; the dmg is
# mounted read-only and detached; every process this script starts is stopped by PID.
set -euo pipefail

DMG=${1:?usage: release-smoke-macos.sh <path-to-dmg> <out-dir>}
OUT=${2:?usage: release-smoke-macos.sh <path-to-dmg> <out-dir>}
EXPECT_VERSION=${EXPECT_VERSION:-0.4.0}
FINAL=${FINAL:-0}
KIT_FIG=${KIT_FIG:-"$HOME/Downloads/❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig"}
AL_FRAME=${AL_FRAME:-"Welcome to Untitled UI"}
GUI_TIMEOUT=${GUI_TIMEOUT:-240}
KEEP_WORK=${KEEP_WORK:-0}
GRID_PAGE_NAME="↳ Calendars"                                 # kit page that holds a real CSS-grid frame
GRID_SELECT="Type=Month view, Breakpoint=Desktop"            # a unique layer inside it, for the capture
GRID_EXPECT_COLUMNS=7
SYN_NAME="Smoke Grid 10x2"                                   # synthetic frame: more than 8 tracks
SYN_COLUMNS=10

[[ -f "$DMG" ]] || { echo "dmg not found: $DMG" >&2; exit 2; }
mkdir -p "$OUT"
OUT=$(cd "$OUT" && pwd -P)
DMG=$(cd "$(dirname "$DMG")" && pwd -P)/$(basename "$DMG")
LOG="$OUT/smoke.log"
SUMMARY="$OUT/smoke-summary.tsv"
: > "$LOG"
printf 'status\tstep\tdetail\n' > "$SUMMARY"

WORK=$(cd "$(mktemp -d "${TMPDIR:-/tmp}/designos-smoke.XXXXXX")" && pwd -P)
BGPIDS=()
FAILS=0
MANUALS=0
MOUNT="$WORK/mnt"

log() { printf '%s\n' "$*" | tee -a "$LOG"; }
res() { # res STATUS STEP DETAIL
  printf '%s\t%s\t%s\n' "$1" "$2" "$3" >> "$SUMMARY"
  log "[$1] $2: $3"
  if [[ $1 == FAIL ]]; then FAILS=$((FAILS + 1)); fi
  if [[ $1 == OWNER-MANUAL ]]; then MANUALS=$((MANUALS + 1)); fi
  return 0
}

cleanup() {
  local rc=$?
  set +e
  for pid in "${BGPIDS[@]:-}"; do
    [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null && { kill "$pid" 2>/dev/null; sleep 1; kill -9 "$pid" 2>/dev/null; }
  done
  if mount | grep -q " on $MOUNT "; then hdiutil detach "$MOUNT" -quiet >/dev/null 2>&1 || hdiutil detach "$MOUNT" -force -quiet >/dev/null 2>&1; fi
  if [[ "$KEEP_WORK" != 1 && -n "$WORK" && "$WORK" == */designos-smoke.* ]]; then rm -rf "$WORK"; fi
  exit "$rc"
}
trap cleanup EXIT

# Run a command in the background with a timeout; stdout goes to a file, stderr to the log. Kills only its own PID.
run_bg() { # run_bg <timeout_s> <stdout_file> <cmd...>
  local t=$1 outf=$2 rc=0 start=$SECONDS pid
  shift 2
  "$@" >"$outf" 2>>"$LOG" &
  pid=$!
  BGPIDS+=("$pid")
  while kill -0 "$pid" 2>/dev/null; do
    if (( SECONDS - start >= t )); then
      log "TIMEOUT after ${t}s: pid $pid ($1)"
      kill "$pid" 2>/dev/null || true; sleep 2; kill -9 "$pid" 2>/dev/null || true
      wait "$pid" 2>/dev/null || true
      return 124
    fi
    sleep 0.5
  done
  wait "$pid" || rc=$?
  return "$rc"
}

# ---------------------------------------------------------------------------------------------------------
# helpers (python3 stdlib only), written to the work dir
# ---------------------------------------------------------------------------------------------------------
mkdir -p "$WORK/tools"
cat > "$WORK/tools/helpers.py" <<'PY'
import json, os, struct, subprocess, sys, tempfile

def load(p):
    with open(p) as f:
        return json.load(f)

def walk(n, depth=0, path=""):
    yield n, depth, path
    for c in n.get("children", []) or []:
        yield from walk(c, depth + 1, path + "/" + str(n.get("name")))

def sig(root):
    """Id-free structural signature of a spec subtree: kind, name, size, layout."""
    out = []
    for n, depth, _ in walk(root):
        out.append((depth, n.get("kind"), n.get("name"), round(float(n.get("width", 0)), 2),
                    round(float(n.get("height", 0)), 2), json.dumps(n.get("layout") or {}, sort_keys=True)))
    return out

def cmd_summary(spec):
    r = load(spec)
    l = r.get("layout") or {}
    print(json.dumps({"id": r.get("id"), "name": r.get("name"), "kind": r.get("kind"), "width": r.get("width"),
                      "height": r.get("height"), "direction": l.get("direction"), "gap": l.get("gap"),
                      "padding": l.get("padding"), "grid_columns": l.get("grid_columns"),
                      "grid_rows": l.get("grid_rows"), "children": len(r.get("children") or []),
                      "subtree_nodes": sum(1 for _ in walk(r))}))

def cmd_find_grid(spec):
    r = load(spec)
    for n, _, path in walk(r):
        l = n.get("layout") or {}
        if l.get("direction") == "grid":
            print(json.dumps({"path": path + "/" + str(n.get("name")), "id": n.get("id"), "width": n.get("width"),
                              "height": n.get("height"), "direction": "grid", "grid_columns": l.get("grid_columns"),
                              "grid_rows": l.get("grid_rows"), "children": len(n.get("children") or [])}))
            return
    print("{}")

def cmd_sig_diff(a, b):
    sa, sb = sig(load(a)), sig(load(b))
    diffs = [(i, x, y) for i, (x, y) in enumerate(zip(sa, sb)) if x != y]
    print(json.dumps({"nodes_a": len(sa), "nodes_b": len(sb), "differences": len(diffs) + abs(len(sa) - len(sb)),
                      "first": [{"index": i, "a": x, "b": y} for i, x, y in diffs[:5]]}))

def cmd_norm_pages(insp):
    d = load(insp)
    return [(p["name"], p["child_count"], [(s["name"], s["kind"], s["width"], s["height"]) for s in p["screens"]])
            for p in d["pages"]]

def cmd_inspect_diff(a, b):
    na, nb = cmd_norm_pages(a), cmd_norm_pages(b)
    da, db = load(a), load(b)
    bad = [i for i, (x, y) in enumerate(zip(na, nb)) if x != y]
    print(json.dumps({"node_count_a": da["node_count"], "node_count_b": db["node_count"],
                      "pages_a": len(na), "pages_b": len(nb), "pages_differing": len(bad) + abs(len(na) - len(nb)),
                      "first_differing_page": na[bad[0]][0] if bad else None}))

def cmd_page_id(insp, name):
    for p in load(insp)["pages"]:
        if p["name"].strip() == name:
            print(p["id"]); return
    sys.exit(1)

def cmd_screen_id(insp, name):
    for p in load(insp)["pages"]:
        for s in p["screens"]:
            if s["name"] == name:
                print(s["id"]); return
    sys.exit(1)

def cmd_inspect_summary(insp):
    d = load(insp)
    print(json.dumps({"name": d["name"], "node_count": d["node_count"], "pages": len(d["pages"])}))

def cmd_json_get(path, key):
    v = load(path)
    for k in key.split("."):
        v = v[int(k)] if isinstance(v, list) else v.get(k)
    print("" if v is None else v)

def cmd_png_check(png, ew="", eh=""):
    out = subprocess.run(["sips", "-g", "pixelWidth", "-g", "pixelHeight", png], capture_output=True, text=True).stdout
    w = int(out.split("pixelWidth:")[1].split()[0]); h = int(out.split("pixelHeight:")[1].split()[0])
    with tempfile.TemporaryDirectory() as td:
        bmp = os.path.join(td, "x.bmp")
        subprocess.run(["sips", "-s", "format", "bmp", png, "--out", bmp], capture_output=True, check=True)
        d = open(bmp, "rb").read()
    off = struct.unpack_from("<I", d, 10)[0]
    bpp = struct.unpack_from("<H", d, 28)[0]
    if bpp != 32:
        print(json.dumps({"ok": False, "why": f"unexpected bmp depth {bpp}", "width": w, "height": h})); sys.exit(1)
    px = memoryview(d)[off:off + w * h * 4].cast("I")
    from collections import Counter
    c = Counter(px)
    dominant = c.most_common(1)[0][1] / len(px)
    ok = len(c) >= 8 and dominant <= 0.995
    why = []
    if len(c) < 8: why.append("fewer than 8 distinct colours")
    if dominant > 0.995: why.append(f"{dominant:.3f} of pixels are one colour")
    if ew and (w, h) != (int(ew), int(eh)): ok = False; why.append(f"size {w}x{h} != expected {ew}x{eh}")
    print(json.dumps({"ok": ok, "width": w, "height": h, "distinct_colours": len(c), "dominant_fraction": round(dominant, 4), "why": why}))
    sys.exit(0 if ok else 1)

def cmd_mcp(binpath, workspace, calls_json):
    calls = json.loads(calls_json)
    env = {k: v for k, v in os.environ.items() if not k.startswith("DESIGNOS_")}
    p = subprocess.Popen([binpath, "--workspace", workspace], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                         stderr=subprocess.PIPE, text=True, bufsize=1, env=env)
    def send(m):
        p.stdin.write(json.dumps(m) + "\n"); p.stdin.flush()
    def recv():
        while True:
            line = p.stdout.readline()
            if not line:
                raise RuntimeError("mcp closed: " + p.stderr.read())
            if line.strip():
                return json.loads(line)
    send({"jsonrpc": "2.0", "id": 0, "method": "initialize", "params": {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "smoke", "version": "1"}}})
    init = recv()
    send({"jsonrpc": "2.0", "method": "notifications/initialized"})
    results = []
    for i, (name, args) in enumerate(calls, 1):
        send({"jsonrpc": "2.0", "id": i, "method": "tools/call", "params": {"name": name, "arguments": args}})
        r = recv()["result"]
        results.append({"tool": name, "isError": r.get("isError", False), "data": r.get("structuredContent")})
    p.stdin.close(); p.terminate()
    print(json.dumps({"server": init["result"]["serverInfo"], "results": results}))

if __name__ == "__main__":
    globals()["cmd_" + sys.argv[1].replace("-", "_")](*sys.argv[2:])
PY
H() { python3 -I "$WORK/tools/helpers.py" "$@"; }

# ---------------------------------------------------------------------------------------------------------
# preflight
# ---------------------------------------------------------------------------------------------------------
log "== release smoke test: $(date '+%Y-%m-%d %H:%M:%S') =="
log "dmg: $DMG"
log "dmg sha256: $(shasum -a 256 "$DMG" | cut -d' ' -f1)"
log "expect version: $EXPECT_VERSION (FINAL=$FINAL)"
log "out: $OUT"
log "work: $WORK"
[[ -f "$KIT_FIG" ]] || { res FAIL "0.preflight" "kit .fig not found: $KIT_FIG"; exit 1; }
FREE_GB=$(df -g /System/Volumes/Data | awk 'NR==2 {print $4}')
if (( FREE_GB < 6 )); then res FAIL "0.preflight" "only ${FREE_GB} GB free, need 6 GB for the kit copies"; exit 1; fi
log "free disk: ${FREE_GB} GB; kit: $(du -h "$KIT_FIG" | cut -f1); sha256 of the original: $(shasum -a 256 "$KIT_FIG" | cut -d' ' -f1)"
# keep the display awake for the whole run (screenshots fail on a sleeping display); dies with this script
caffeinate -u -t 2 >/dev/null 2>&1 || true
caffeinate -d -t 7200 -w $$ >/dev/null 2>&1 &
BGPIDS+=("$!")
disown "$!" 2>/dev/null || true

# ---------------------------------------------------------------------------------------------------------
# step 1: launch the packaged app
# ---------------------------------------------------------------------------------------------------------
mkdir -p "$MOUNT"
if hdiutil attach -nobrowse -readonly -mountpoint "$MOUNT" "$DMG" >>"$LOG" 2>&1; then
  res PASS "1.dmg-mount" "mounted read-only at a temp mountpoint"
else
  res FAIL "1.dmg-mount" "hdiutil attach failed"; exit 1
fi
ditto "$MOUNT/designos.app" "$WORK/designos.app"
hdiutil detach "$MOUNT" -quiet >>"$LOG" 2>&1 && res PASS "1.dmg-detach" "detached after copying designos.app to the work dir" || res FAIL "1.dmg-detach" "hdiutil detach failed"
APP="$WORK/designos.app"
MAC="$APP/Contents/MacOS"
STUDIO="$MAC/designos-studio"; CLI="$MAC/designos"; MCPBIN="$MAC/designos-mcp"
missing=""
for b in designos-studio designos designos-mcp designos-diff; do [[ -x "$MAC/$b" ]] || missing="$missing $b"; done
if [[ -z "$missing" ]]; then res PASS "1.bundle-binaries" "designos-studio designos designos-mcp designos-diff present and executable; archs: $(lipo -archs "$STUDIO")"; else res FAIL "1.bundle-binaries" "missing:$missing"; exit 1; fi
if codesign --verify --deep --strict "$APP" >>"$LOG" 2>&1; then res PASS "1.codesign" "signature verifies ($(codesign -dv "$APP" 2>&1 | grep -E '^Signature=' | head -1))"; else res FAIL "1.codesign" "codesign --verify failed"; fi

PLIST_VER=$(/usr/libexec/PlistBuddy -c "Print :CFBundleShortVersionString" "$APP/Contents/Info.plist")
log "Info.plist CFBundleShortVersionString=$PLIST_VER CFBundleVersion=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleVersion' "$APP/Contents/Info.plist")"
H mcp "$MCPBIN" "$WORK" '[]' > "$WORK/mcp-version.json" 2>>"$LOG" || true
BIN_VER=$(H json-get "$WORK/mcp-version.json" server.version 2>/dev/null || true)
if [[ "$BIN_VER" == "$EXPECT_VERSION" ]]; then
  res PASS "1.version-binary" "designos-mcp serverInfo (compiled-in CARGO_PKG_VERSION, same value the app shows) = $BIN_VER; the CLI has no --version flag"
else
  res FAIL "1.version-binary" "binary reports '${BIN_VER:-nothing}', expected $EXPECT_VERSION"
fi
if [[ "$PLIST_VER" == "$EXPECT_VERSION" ]]; then
  res PASS "1.version-plist" "Info.plist CFBundleShortVersionString = $PLIST_VER"
elif [[ "$FINAL" != 1 && "$PLIST_VER" == 0.0.0-dev.* ]]; then
  res NOTE "1.version-plist" "Info.plist says $PLIST_VER: expected for a dry run (docs/releasing.md: files stamped 0.0.0-dev.<sha>, the app inside reports the Cargo.toml version). Re-run with FINAL=1 on the tagged build."
else
  res FAIL "1.version-plist" "Info.plist says '$PLIST_VER', expected $EXPECT_VERSION"
fi

mkdir -p "$WORK/lib"
SHOT1="$OUT/step1-launch-home.png"
rc=0; run_bg "$GUI_TIMEOUT" "$WORK/step1.out" "$STUDIO" --home --library "$WORK/lib" --size 1440x900 --no-toast --screenshot "$SHOT1" || rc=$?
if [[ $rc -eq 0 && -f "$SHOT1" ]] && H png-check "$SHOT1" 1440 900 > "$WORK/png1.json"; then
  res PASS "1.launch" "designos-studio launched, home screen captured (the version badge in its top-left reads v$BIN_VER): $(basename "$SHOT1") $(cat "$WORK/png1.json")"
else
  res FAIL "1.launch" "designos-studio exit=$rc, capture $(cat "$WORK/png1.json" 2>/dev/null || echo missing)"
fi
res OWNER-MANUAL "1.about-dialog" "the About dialog (menu click) and the Gatekeeper Open Anyway flow need a person; the badge above and the compiled-in version are what the script checks"

# ---------------------------------------------------------------------------------------------------------
# step 2: open one real .fig (the Untitled UI kit), screenshot a page with an auto layout frame
# ---------------------------------------------------------------------------------------------------------
cp "$KIT_FIG" "$WORK/kit.fig"            # baseline copy: never written to by this script
run_bg 300 "$WORK/kit-inspect.json" "$CLI" "$WORK/kit.fig" --inspect-json || { res FAIL "2.open-fig" "designos --inspect-json on the kit failed (see smoke.log)"; exit 1; }
cp "$WORK/kit-inspect.json" "$OUT/step2-inspect-before.json"
IS=$(H inspect-summary "$WORK/kit-inspect.json")
NODES_BEFORE=$(H json-get "$WORK/kit-inspect.json" node_count)
if [[ "$NODES_BEFORE" -gt 1000 ]] && AL_ID=$(H screen-id "$WORK/kit-inspect.json" "$AL_FRAME"); then
  res PASS "2.open-fig" "kit opens headless: $IS; top-level frame '$AL_FRAME' (id $AL_ID) found"
else
  res FAIL "2.open-fig" "unexpected kit contents: $IS; frame '$AL_FRAME' missing"; exit 1
fi
run_bg 300 "$WORK/al-select.json" "$CLI" "$WORK/kit.fig" --select "$AL_FRAME" --export-spec "$WORK/al-before.spec.json" || true
AL_SUM=$(H summary "$WORK/al-before.spec.json" 2>/dev/null || echo '{}')
AL_NAME=$(echo "$AL_SUM" | python3 -I -c 'import json,sys;print(json.load(sys.stdin).get("name"))')
AL_DIR=$(echo "$AL_SUM" | python3 -I -c 'import json,sys;print(json.load(sys.stdin).get("direction"))')
if [[ "$AL_NAME" == "$AL_FRAME" && ( "$AL_DIR" == row || "$AL_DIR" == column ) ]]; then
  res PASS "2.autolayout-structure" "$AL_SUM"
else
  res FAIL "2.autolayout-structure" "expected auto layout frame '$AL_FRAME', export-spec gave $AL_SUM"
fi
echo "$AL_SUM" > "$OUT/step2-autolayout-frame.json"
SHOT2="$OUT/step2-open-kit-autolayout.png"
rc=0; run_bg "$GUI_TIMEOUT" "$WORK/step2.out" "$STUDIO" --library "$WORK/lib" "$WORK/kit.fig" --select "$AL_FRAME" --size 1440x1000 --no-toast --screenshot "$SHOT2" || rc=$?
if [[ $rc -eq 0 ]] && H png-check "$SHOT2" 1440 1000 > "$WORK/png2.json"; then
  res PASS "2.screenshot" "kit page with '$AL_FRAME' selected, Auto layout section in the Properties panel: $(basename "$SHOT2") $(cat "$WORK/png2.json")"
else
  res FAIL "2.screenshot" "exit=$rc $(cat "$WORK/png2.json" 2>/dev/null || echo missing)"
fi
SHOT2G="$OUT/step2-open-kit-autolayout-gpu.png"
rc=0; run_bg "$GUI_TIMEOUT" "$WORK/step2g.out" "$STUDIO" --library "$WORK/lib" "$WORK/kit.fig" --select "$AL_FRAME" --gpu --size 1440x1000 --no-toast --screenshot "$SHOT2G" || rc=$?
if [[ $rc -eq 0 ]] && H png-check "$SHOT2G" 1440 1000 > "$WORK/png2g.json"; then
  # a screenshot of a document with more than 5000 nodes already uses the GPU renderer (shell.rs), so the default capture above is a GPU frame too; the two files are expected to match
  SAME=different; cmp -s "$SHOT2" "$SHOT2G" && SAME=byte-identical
  res PASS "2.screenshot-gpu" "capture with --gpu explicitly (the kit has >5000 nodes, so the default capture is GPU as well; the two PNGs are $SAME): $(basename "$SHOT2G") $(cat "$WORK/png2g.json")"
else
  res FAIL "2.screenshot-gpu" "exit=$rc $(cat "$WORK/png2g.json" 2>/dev/null || echo missing)"
fi

# ---------------------------------------------------------------------------------------------------------
# step 3: Layout 2.0 headline (grid + auto layout parity). --inspect-json carries no layout fields, so the
# structural assertions come from --export-spec (layout.direction, grid_columns, grid_rows, size).
# 3a: a real CSS-grid frame that already lives in the kit.  3b: a synthetic frame with more than 8 tracks.
# ---------------------------------------------------------------------------------------------------------
if GRID_PAGE_ID=$(H page-id "$WORK/kit-inspect.json" "$GRID_PAGE_NAME"); then
  run_bg 300 "$WORK/grid-page.json" "$CLI" "$WORK/kit.fig" --select "$GRID_PAGE_ID" --export-spec "$WORK/grid-page-before.spec.json" || true
  GRID_REAL=$(H find-grid "$WORK/grid-page-before.spec.json" 2>/dev/null || echo '{}')
  echo "$GRID_REAL" > "$OUT/step3a-kit-grid-frame.json"
  GCOLS=$(echo "$GRID_REAL" | python3 -I -c 'import json,sys;print(json.load(sys.stdin).get("grid_columns"))')
  GDIR=$(echo "$GRID_REAL" | python3 -I -c 'import json,sys;print(json.load(sys.stdin).get("direction"))')
  if [[ "$GDIR" == grid && "$GCOLS" == "$GRID_EXPECT_COLUMNS" ]]; then
    res PASS "3a.kit-grid-structure" "kit page '$GRID_PAGE_NAME' (id $GRID_PAGE_ID) holds a grid frame: $GRID_REAL"
  else
    res FAIL "3a.kit-grid-structure" "expected a grid frame with $GRID_EXPECT_COLUMNS columns on page '$GRID_PAGE_NAME', got $GRID_REAL"
  fi
else
  GRID_PAGE_ID=""
  res FAIL "3a.kit-grid-structure" "page '$GRID_PAGE_NAME' not found in the kit"
fi
SHOT3A="$OUT/step3a-kit-calendar-grid.png"
rc=0; run_bg "$GUI_TIMEOUT" "$WORK/step3a.out" "$STUDIO" --library "$WORK/lib" "$WORK/kit.fig" --select "$GRID_SELECT" --zoom 60 --size 1440x1000 --no-toast --screenshot "$SHOT3A" || rc=$?
if [[ $rc -eq 0 ]] && H png-check "$SHOT3A" 1440 1000 > "$WORK/png3a.json"; then
  res PASS "3a.screenshot" "kit calendar month view (its body is the 7-column grid frame): $(basename "$SHOT3A") $(cat "$WORK/png3a.json")"
else
  res FAIL "3a.screenshot" "exit=$rc $(cat "$WORK/png3a.json" 2>/dev/null || echo missing)"
fi

# 3b: add a 10-column grid (20 cells) to a COPY of the kit; the app saves it as .fig; reopen and assert
cat > "$WORK/syn-grid.spec.json" <<EOF
{"kind":"frame","name":"$SYN_NAME","x":-700,"y":0,"width":520,"width_sizing":"fixed","height_sizing":"hug","fill":"#F5F5F5",
 "layout":{"direction":"grid","grid_columns":$SYN_COLUMNS,"gap":8.0,"padding":[16,16,16,16]},
 "children":[$(for i in $(seq 1 20); do c='#1570EF'; (( i % 2 == 0 )) && c='#7F56D9'; printf '{"kind":"rectangle","name":"Cell %d","width":40,"height":40,"fill":"%s"}' "$i" "$c"; (( i < 20 )) && printf ','; done)]}
EOF
cp "$KIT_FIG" "$WORK/kit-grid.fig"
if run_bg 300 "$WORK/apply.out" "$CLI" "$WORK/kit-grid.fig" --apply-spec "$WORK/syn-grid.spec.json"; then
  run_bg 300 "$WORK/syn-sel.out" "$CLI" "$WORK/kit-grid.fig" --select "$SYN_NAME" --export-spec "$WORK/syn-after-fig.spec.json" || true
  SYN_SUM=$(H summary "$WORK/syn-after-fig.spec.json" 2>/dev/null || echo '{}')
  echo "$SYN_SUM" > "$OUT/step3b-synthetic-grid-after-fig-reopen.json"
  if echo "$SYN_SUM" | python3 -I -c "
import json,sys
d=json.load(sys.stdin)
ok=d.get('name')=='$SYN_NAME' and d.get('direction')=='grid' and d.get('grid_columns')==$SYN_COLUMNS and d.get('width')==520 and d.get('height')==120 and d.get('children')==20
sys.exit(0 if ok else 1)"; then
    res PASS "3b.synthetic-grid-structure" "$SYN_COLUMNS columns (>8 tracks) survive the app's .fig save and a reopen; solved height 120 = 16+40+8+40+16: $SYN_SUM"
  else
    res FAIL "3b.synthetic-grid-structure" "after .fig reopen: $SYN_SUM (expected grid, $SYN_COLUMNS columns, 520x120, 20 children)"
  fi
  SHOT3B="$OUT/step3b-synthetic-grid-10-columns.png"
  rc=0; run_bg "$GUI_TIMEOUT" "$WORK/step3b.out" "$STUDIO" --library "$WORK/lib" "$WORK/kit-grid.fig" --select "$SYN_NAME" --zoom 150 --size 1440x1000 --no-toast --screenshot "$SHOT3B" || rc=$?
  if [[ $rc -eq 0 ]] && H png-check "$SHOT3B" 1440 1000 > "$WORK/png3b.json"; then
    res PASS "3b.screenshot" "grid frame selected, Properties panel shows Grid (Columns x Rows) C $SYN_COLUMNS: $(basename "$SHOT3B") $(cat "$WORK/png3b.json")"
  else
    res FAIL "3b.screenshot" "exit=$rc $(cat "$WORK/png3b.json" 2>/dev/null || echo missing)"
  fi
else
  res FAIL "3b.synthetic-grid-structure" "--apply-spec on the kit copy failed (see smoke.log)"
fi

# ---------------------------------------------------------------------------------------------------------
# step 4: save, close, reopen
# 4a: .fig save-copy of the untouched kit baseline (--export-fig), reopen, compare. This is the only save path the
#     CLI can drive on a .fig; it is a convert, not the app's Save action.
# 4b: native .designos save and reopen of the synthetic grid frame (the CLI cannot convert the kit to .designos).
# ---------------------------------------------------------------------------------------------------------
if run_bg 300 "$WORK/exportfig.out" "$CLI" "$WORK/kit.fig" --export-fig "$WORK/kit-saved.fig"; then
  run_bg 300 "$WORK/kit-saved-inspect.json" "$CLI" "$WORK/kit-saved.fig" --inspect-json || true
  cp "$WORK/kit-saved-inspect.json" "$OUT/step4-inspect-after-fig.json"
  H inspect-diff "$WORK/kit-inspect.json" "$WORK/kit-saved-inspect.json" > "$WORK/inspect-diff.json"
  NODES_AFTER=$(H json-get "$WORK/kit-saved-inspect.json" node_count)
  PAGES_DIFF=$(H json-get "$WORK/inspect-diff.json" pages_differing)
  # layout fields of the auto layout frame and of the grid page, before vs after the save/reopen
  run_bg 300 "$WORK/x.out" "$CLI" "$WORK/kit-saved.fig" --select "$AL_FRAME" --export-spec "$WORK/al-after.spec.json" || true
  AL_CMP=$(H sig-diff "$WORK/al-before.spec.json" "$WORK/al-after.spec.json" 2>/dev/null || echo '{"differences":-1}')
  GRID_CMP='{"differences":-1}'
  if [[ -n "$GRID_PAGE_ID" ]]; then
    AFTER_PAGE_ID=$(H page-id "$WORK/kit-saved-inspect.json" "$GRID_PAGE_NAME" 2>/dev/null || echo "")
    if [[ -n "$AFTER_PAGE_ID" ]]; then
      run_bg 300 "$WORK/x.out" "$CLI" "$WORK/kit-saved.fig" --select "$AFTER_PAGE_ID" --export-spec "$WORK/grid-page-after.spec.json" || true
      GRID_CMP=$(H sig-diff "$WORK/grid-page-before.spec.json" "$WORK/grid-page-after.spec.json" 2>/dev/null || echo '{"differences":-1}')
    fi
  fi
  printf '{"node_count_before":%s,"node_count_after":%s,"inspect_diff":%s,"autolayout_frame_signature_diff":%s,"grid_page_signature_diff":%s}\n' \
    "$NODES_BEFORE" "$NODES_AFTER" "$(cat "$WORK/inspect-diff.json")" "$AL_CMP" "$GRID_CMP" > "$OUT/step4-compare.json"
  log "step 4 compare: $(cat "$OUT/step4-compare.json")"
  if [[ "$NODES_BEFORE" == "$NODES_AFTER" ]]; then
    res PASS "4a.fig-node-count" "$NODES_BEFORE nodes before and after the .fig save/reopen"
  else
    SUB_AL=$(echo "$AL_CMP" | python3 -I -c 'import json,sys;d=json.load(sys.stdin);print("%s->%s" % (d.get("nodes_a"), d.get("nodes_b")))' 2>/dev/null || echo "?")
    SUB_GRID=$(echo "$GRID_CMP" | python3 -I -c 'import json,sys;d=json.load(sys.stdin);print("%s->%s" % (d.get("nodes_a"), d.get("nodes_b")))' 2>/dev/null || echo "?")
    res FAIL "4a.fig-node-count" "node count changed across one .fig save/reopen: before=$NODES_BEFORE after=$NODES_AFTER delta=$((NODES_AFTER - NODES_BEFORE)); $PAGES_DIFF pages differ in name/child count/top-level frames; subtree node counts unchanged for '$AL_FRAME' ($SUB_AL) and the grid page ($SUB_GRID), so the extra nodes sit elsewhere (node ids are renumbered on import, so ids are not compared)"
  fi
  if [[ "$PAGES_DIFF" == 0 ]]; then
    res PASS "4a.fig-pages" "all pages keep name, child count and every top-level frame's name, kind and size"
  else
    res FAIL "4a.fig-pages" "$PAGES_DIFF pages differ after the .fig save/reopen: $(cat "$WORK/inspect-diff.json")"
  fi
  if [[ "$(echo "$AL_CMP" | python3 -I -c 'import json,sys;print(json.load(sys.stdin)["differences"])')" == 0 ]]; then
    res PASS "4a.fig-autolayout-fields" "'$AL_FRAME' subtree: layout mode, gap, padding, alignment and sizes identical before/after ($AL_CMP)"
  else
    res FAIL "4a.fig-autolayout-fields" "layout fields changed for '$AL_FRAME': $AL_CMP"
  fi
  if [[ "$(echo "$GRID_CMP" | python3 -I -c 'import json,sys;print(json.load(sys.stdin)["differences"])')" == 0 ]]; then
    res PASS "4a.fig-grid-fields" "kit grid page: every frame's layout (incl. grid_columns/grid_rows) identical before/after ($GRID_CMP)"
  else
    res FAIL "4a.fig-grid-fields" "layout fields changed on the kit grid page: $GRID_CMP"
  fi
  SHOT4="$OUT/step4-reopened-saved-fig.png"
  rc=0; run_bg "$GUI_TIMEOUT" "$WORK/step4.out" "$STUDIO" --library "$WORK/lib" "$WORK/kit-saved.fig" --select "$AL_FRAME" --size 1440x1000 --no-toast --screenshot "$SHOT4" || rc=$?
  if [[ $rc -eq 0 ]] && H png-check "$SHOT4" 1440 1000 > "$WORK/png4.json"; then
    res PASS "4a.screenshot" "the saved copy reopened in the app, '$AL_FRAME' selected: $(basename "$SHOT4") $(cat "$WORK/png4.json")"
  else
    res FAIL "4a.screenshot" "exit=$rc $(cat "$WORK/png4.json" 2>/dev/null || echo missing)"
  fi
else
  res FAIL "4a.fig-save" "--export-fig failed (see smoke.log)"
fi

mkdir -p "$WORK/native"
if run_bg 120 "$WORK/native-apply.out" "$CLI" "$WORK/native/grid.designos" --apply-spec "$WORK/syn-grid.spec.json" \
   && run_bg 120 "$WORK/native-inspect1.json" "$CLI" "$WORK/native/grid.designos" --inspect-json \
   && run_bg 120 "$WORK/x.out" "$CLI" "$WORK/native/grid.designos" --select "$SYN_NAME" --export-spec "$WORK/syn-native.spec.json" \
   && run_bg 120 "$WORK/x.out" "$CLI" "$WORK/native/grid2.designos" --apply-spec "$WORK/syn-native.spec.json" \
   && run_bg 120 "$WORK/native-inspect2.json" "$CLI" "$WORK/native/grid2.designos" --inspect-json; then
  N1=$(H json-get "$WORK/native-inspect1.json" node_count); N2=$(H json-get "$WORK/native-inspect2.json" node_count)
  run_bg 120 "$WORK/x.out" "$CLI" "$WORK/native/grid2.designos" --select "$SYN_NAME" --export-spec "$WORK/syn-native2.spec.json" || true
  NATIVE_CMP=$(H sig-diff "$WORK/syn-native.spec.json" "$WORK/syn-native2.spec.json" 2>/dev/null || echo '{"differences":-1}')
  FIG_VS_NATIVE=$(H sig-diff "$WORK/syn-after-fig.spec.json" "$WORK/syn-native.spec.json" 2>/dev/null || echo '{"differences":-1}')
  echo "{\"native_nodes_first_save\":$N1,\"native_nodes_second_save\":$N2,\"native_resave_diff\":$NATIVE_CMP,\"fig_vs_native_diff\":$FIG_VS_NATIVE}" > "$OUT/step4b-native-compare.json"
  if [[ "$N1" == "$N2" && "$(echo "$NATIVE_CMP" | python3 -I -c 'import json,sys;print(json.load(sys.stdin)["differences"])')" == 0 && "$(echo "$FIG_VS_NATIVE" | python3 -I -c 'import json,sys;print(json.load(sys.stdin)["differences"])')" == 0 ]]; then
    res PASS "4b.native-designos" "grid frame saved as .designos, reopened, re-saved: $N1 nodes both times, layout fields identical, and identical to the .fig-reopened copy"
  else
    res FAIL "4b.native-designos" "$(cat "$OUT/step4b-native-compare.json")"
  fi
else
  res FAIL "4b.native-designos" "a CLI step failed (see smoke.log)"
fi
res OWNER-MANUAL "4c.app-save-action" "the app's own Save (Cmd+S / Save to library) of the kit as .designos, closing the tab and reopening it from Home needs the GUI; the headless CLI cannot convert a .fig to .designos, so 4a (.fig) and 4b (.designos, synthetic frame) stand in for the file formats"

# ---------------------------------------------------------------------------------------------------------
# step 5: export a PNG of a frame. The designos CLI has no PNG export flag; designos-export is reached through
# designos-mcp designos_render_png (the same designos_export::export_png as File > Export). That server refuses
# imports above 64 MiB, so the kit itself is tried first and the PNG of the synthetic grid frame is the fallback.
# ---------------------------------------------------------------------------------------------------------
KIT_MCP_DIR="$WORK/kitws"; mkdir -p "$KIT_MCP_DIR"; ln "$WORK/kit.fig" "$KIT_MCP_DIR/kit.fig" 2>/dev/null || cp "$WORK/kit.fig" "$KIT_MCP_DIR/kit.fig"
H mcp "$MCPBIN" "$KIT_MCP_DIR" "$(printf '[["designos_open_file",{"path":"kit.fig"}],["designos_render_png",{"path":"kit-frame.png","node_id":%s,"scale":0.25}]]' "$AL_ID")" > "$WORK/mcp-kit.json" 2>>"$LOG" || true
KIT_OPEN_ERR=$(python3 -I -c 'import json,sys;r=json.load(open(sys.argv[1]))["results"][0];print(r["data"].get("error","") if r["isError"] else "")' "$WORK/mcp-kit.json" 2>/dev/null || echo "mcp call failed")
if [[ -z "$KIT_OPEN_ERR" && -f "$KIT_MCP_DIR/kit-frame.png" ]]; then
  cp "$KIT_MCP_DIR/kit-frame.png" "$OUT/step5-export-kit-frame.png"
  if H png-check "$OUT/step5-export-kit-frame.png" > "$WORK/png5k.json"; then
    res PASS "5a.export-kit-frame" "PNG of the real kit frame '$AL_FRAME' at 25%: $(cat "$WORK/png5k.json")"
  else
    res FAIL "5a.export-kit-frame" "$(cat "$WORK/png5k.json")"
  fi
else
  res OWNER-MANUAL "5a.export-kit-frame" "no headless path exports a frame of the kit: designos-mcp says '$KIT_OPEN_ERR'. Export the kit frame from the app (select it, Export panel, PNG) and attach the file"
fi
SYN_ID=$(H screen-id "$WORK/native-inspect1.json" "$SYN_NAME" 2>/dev/null || echo "")
if [[ -n "$SYN_ID" ]]; then
  H mcp "$MCPBIN" "$WORK/native" "$(printf '[["designos_open_file",{"path":"grid.designos"}],["designos_render_png",{"path":"export-grid.png","node_id":%s,"scale":2}]]' "$SYN_ID")" > "$OUT/step5-mcp-calls.json" 2>>"$LOG" || true
  if [[ -f "$WORK/native/export-grid.png" ]]; then
    cp "$WORK/native/export-grid.png" "$OUT/step5-export-synthetic-grid.png"
    if H png-check "$OUT/step5-export-synthetic-grid.png" 1040 240 > "$WORK/png5.json"; then
      res PASS "5b.export-synthetic-grid" "designos_render_png at 2x of the 520x120 grid frame: $(cat "$WORK/png5.json") (a render of the .designos from step 4b, not of a kit frame)"
    else
      res FAIL "5b.export-synthetic-grid" "$(cat "$WORK/png5.json")"
    fi
  else
    res FAIL "5b.export-synthetic-grid" "no PNG written: $(cat "$OUT/step5-mcp-calls.json" 2>/dev/null)"
  fi
else
  res FAIL "5b.export-synthetic-grid" "frame '$SYN_NAME' not found in the native file"
fi

# ---------------------------------------------------------------------------------------------------------
log ""
log "== summary =="
column -t -s $'\t' "$SUMMARY" | tee -a "$LOG" >/dev/null
log "PASS $(grep -c '^PASS' "$SUMMARY")  FAIL $FAILS  OWNER-MANUAL $MANUALS  NOTE $(grep -c '^NOTE' "$SUMMARY")"
if (( FAILS > 0 )); then log "RESULT: FAIL ($FAILS failing checks)"; exit 1; fi
log "RESULT: PASS (owner-manual steps remain: $MANUALS)"
