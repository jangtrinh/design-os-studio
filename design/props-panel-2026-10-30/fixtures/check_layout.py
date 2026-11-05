#!/usr/bin/env python3
"""Layout acceptance for the instance props panel (spec AC2, AC4, AC6, AC9).
usage: check_layout.py layout.json "Name1,Name2,..."   (expected displayed order)
Make the JSON with: designos <fixture> --select fig:1:2 --size 1440x900 --screenshot out.png --layout-json layout.json
Exit 0 = pass."""
import json, sys
d = json.load(open(sys.argv[1]))["named"]
want = [w for w in sys.argv[2].split(",") if w]
rows = {k[5:]: v["rect"] for k, v in d.items() if k.startswith("prop:")}
fails = []
if not want:
    if rows: fails.append("expected no property rows, found %s" % sorted(rows))
else:
    got = [n for n, _ in sorted(rows.items(), key=lambda kv: kv[1][1])]
    if got != want: fails.append("order %s != %s" % (got, want))
    xs = {r[0] for r in rows.values()}; ws = {r[2] for r in rows.values()}; hs = {r[3] for r in rows.values()}
    if len(xs) != 1: fails.append("control x differs: %s" % sorted(xs))
    if ws != {88.0}: fails.append("control width != 88: %s" % sorted(ws))
    if hs != {32.0}: fails.append("row height != 32: %s" % sorted(hs))
    ys = sorted(r[1] for r in rows.values())
    if any(round(b - a, 1) != 36.0 for a, b in zip(ys, ys[1:])): fails.append("row pitch != 36: %s" % ys)
    sw = d.get("instance-swap")
    if sw and xs and round(next(iter(xs)) - sw["rect"][0], 1) != 96.0: fails.append("control col is not label x + 96")
for k in ("instance-swap", "go-to-main", "instance-more"):
    if k not in d: fails.append("missing header record " + k)
for f in fails: print("FAIL", f)
print("check_layout: %d FAIL" % len(fails)); sys.exit(1 if fails else 0)
