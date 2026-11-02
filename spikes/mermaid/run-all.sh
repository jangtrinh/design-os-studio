#!/bin/sh
# THROWAWAY. Runs every fixture through every engine, one process each (so
# peak RSS is per diagram), writes SVG+PNG to $OUT and JSON lines to $OUT/results.jsonl.
#   ./run-all.sh [out-dir]                       build with the pinned 1.95 toolchain, run mmdr + merman
#   ENGINES=merman06 BIN=/path/to/bin ./run-all.sh out   reuse a binary built elsewhere (e.g. +1.93.1)
set -eu
cd "$(dirname "$0")"
OUT="${1:-out}"
ENGINES="${ENGINES:-mmdr merman}"
if [ -z "${BIN:-}" ]; then cargo build --release; BIN=target/release/spike-mermaid; fi
rm -rf "$OUT"; mkdir -p "$OUT"
$BIN baseline fixtures/flow-2-checkout-payment.mmd "$OUT" > "$OUT/results.jsonl"
for engine in $ENGINES; do
  for f in fixtures/*.mmd; do
    $BIN "$engine" "$f" "$OUT" 20 >> "$OUT/results.jsonl"
  done
done
python3 - "$OUT/results.jsonl" <<'EOF'
import sys, json
for line in open(sys.argv[1]):
    d = json.loads(line)
    if d["engine"] == "baseline":
        print(f"baseline process floor: {d['peak_rss_mb']:.1f} MB"); continue
    if d["ok"]:
        print(f"{d['engine']:9} {d['fixture']:44} cold={d['t_svg_cold_ms']:7.2f}ms warm={d['t_svg_warm_median_ms']:7.2f}ms png={d['t_png_ms']:6.2f}ms rss={d['peak_rss_mb']:5.1f}MB text_nodes={d['text_nodes']}")
    else:
        print(f"{d['engine']:9} {d['fixture']:44} {d['path']}: {d.get('message')} (line {d.get('line')})")
EOF
