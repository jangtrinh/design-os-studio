#!/bin/sh
# THROWAWAY. Error path + unsupported-type path for every engine; prints one line each.
set -eu
cd "$(dirname "$0")"
OUT="${1:-out-err}"
BIN=target/release/spike-mermaid
mkdir -p "$OUT"
for engine in mmdr merman; do
  for f in fixtures/err-*.mmd; do
    $BIN "$engine" "$f" "$OUT" 0 | python3 -c "
import sys, json
d = json.loads(sys.stdin.read())
print(f\"{d['engine']:7} {d['fixture']:28} ok={d['ok']!s:5} path={d.get('path','rendered'):12} line={d.get('line')} msg={d.get('message')}\")"
  done
done
