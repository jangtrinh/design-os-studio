#!/bin/sh
# THROWAWAY. Licence mix of the normal-dependency tree per engine, minus the
# resvg/usvg stack Studio already ships. Flags anything outside permissive SPDX.
set -eu
BASE="$(mktemp)"
cd "$(dirname "$0")"
tree() { cargo tree --no-default-features "$@" -e normal --prefix none -f '{p}|{l}' | sort -u; }
tree > "$BASE" 2>/dev/null || tree > /dev/null
for feat in mmdr merman; do
  echo "== $feat: licences of crates NOT in the baseline tree"
  tree --features "$feat" | grep -v -F -f "$BASE" | awk -F'|' '{print $2}' | sort | uniq -c | sort -rn
  echo "-- non-permissive candidates (EPL/MPL/GPL/LGPL/OFL/CC/Unicode):"
  tree --features "$feat" | grep -v -F -f "$BASE" | grep -E 'EPL|MPL|GPL|OFL|CC-|Unicode|LicenseRef' || echo "(none)"
  echo "-- elk / manatee / ratex in tree?"
  tree --features "$feat" | grep -i -E 'elk|manatee|ratex|roughr' || echo "(none)"
done
head -3 ../../LICENSE
