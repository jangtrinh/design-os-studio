#!/bin/sh
# THROWAWAY. Binary-size delta per engine (stripped release), dependency counts,
# network-crate check, and an offline run under a macOS sandbox that denies all network.
set -eu
cd "$(dirname "$0")"
OUT="${1:-out-footprint}"
mkdir -p "$OUT"
size_of() { # label, cargo feature args...
  label="$1"; shift
  cargo build --release --no-default-features "$@" >/dev/null 2>&1
  cp target/release/spike-mermaid "$OUT/bin-$label"
  strip "$OUT/bin-$label"
  printf '%-10s stripped=%s bytes\n' "$label" "$(stat -f %z "$OUT/bin-$label")"
}
size_of none
size_of mmdr --features mmdr
size_of merman --features merman
size_of both --features mmdr,merman
echo "--- unique crates (cargo tree, normal deps, dedup) ---"
for feat in mmdr merman; do
  n=$(cargo tree --no-default-features --features $feat -e normal --prefix none | sort -u | wc -l)
  echo "$feat: $n crates (includes resvg/usvg stack shared by both)"
done
n=$(cargo tree --no-default-features -e normal --prefix none | sort -u | wc -l)
echo "none: $n crates (resvg + usvg + serde_json + libc only)"
echo "--- network-capable crates in the tree (expect none) ---"
cargo tree --features mmdr,merman -e normal --prefix none | sort -u | grep -E '^(reqwest|hyper|ureq|curl|tokio|isahc|surf|attohttpc|native-tls|openssl|rustls) ' || echo "none found"
echo "--- offline run: sandbox-exec denying all network ---"
for engine in mmdr merman; do
  sandbox-exec -p '(version 1)(allow default)(deny network*)' \
    target/release/spike-mermaid "$engine" fixtures/flow-1-feedback-review-flow-reconstructed.mmd "$OUT/offline" 1 \
    | python3 -c "import sys,json; d=json.loads(sys.stdin.read()); print('$engine under network-deny sandbox: ok=', d['ok'])"
done
