#!/usr/bin/env python3
"""One-off prune of the layout baselines after the capture dedupe by unclipped rect.

Run from anywhere inside the repo, with one command:

    python3 plans/reports/prune-clipped-twin-baselines.py

Re-run it unchanged on a rebased tree. It needs `cargo` on PATH, rewrites only the unnamed lists of
`crates/designos-app/tests/layout-baseline/*.layout.json`, and leaves allow.json and cases.json alone.

What it does, at the checked-out code:
  1. Capture every case with the previous rule (env DESIGNOS_LAYOUT_LEGACY_CAPTURE, read by test builds only).
  2. Capture every case with the current rule.
  3. Per case and region, twins = old-capture unnamed minus new-capture unnamed (the difference picks them, no
     geometry heuristic). The gate pairs unnamed widgets by index and main is G4-green, so a baseline index equals
     the old-capture index; those indices are deleted from the baseline.
  4. A case/region whose committed unnamed count differs from the old capture is NOT pruned and is listed.
  5. Prints the proof: per-file removals, a structural diff (only `unnamed` deletions), and for every removed rect
     whether it lies inside a named rect of the same baseline with the same x extent.
The committed baselines are restored after each capture run, so a failure leaves the tree as it was.
"""
import json
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
BDIR = ROOT / "crates/designos-app/tests/layout-baseline"
TOL = 0.5


def read_all():
    return {p.name: p.read_text() for p in sorted(BDIR.glob("*.layout.json"))}


def restore(saved):
    for name, text in saved.items():
        (BDIR / name).write_text(text)


def bless(env_extra):
    env = dict(os.environ, DESIGNOS_LAYOUT_BLESS="1", **env_extra)
    cmd = ["cargo", "test", "-p", "designos-app", "--bin", "designos", "layout_unchanged_"]
    r = subprocess.run(cmd, cwd=ROOT, env=env, capture_output=True, text=True)
    if r.returncode != 0:
        sys.exit("bless run failed:\n" + r.stdout[-2000:] + r.stderr[-2000:])
    return {n: json.loads(t) for n, t in read_all().items()}


def by_region(unnamed):
    out = {}
    for u in unnamed:
        out.setdefault(u["region"], []).append(u)
    return out


def twin_indices(old_list, new_list):
    """Indices of old_list not matched by new_list, which must be an in-order subsequence of old_list."""
    gone, j = [], 0
    for i, u in enumerate(old_list):
        if j < len(new_list) and new_list[j] == u:
            j += 1
        else:
            gone.append(i)
    if j != len(new_list):
        raise ValueError("new capture is not a subsequence of the old capture")
    return gone


def near_named(rect, named):
    x, y, w, h = rect
    for n in named.values():
        nx, ny, nw, nh = n["rect"]
        if abs(x - nx) <= TOL and abs(w - nw) <= TOL and y >= ny - TOL and y + h <= ny + nh + TOL:
            return True
    return False


def main():
    original = read_all()
    try:
        old_cap = bless({"DESIGNOS_LAYOUT_LEGACY_CAPTURE": "1"})
        restore(original)
        new_cap = bless({})
    finally:
        restore(original)

    committed = {n: json.loads(t) for n, t in original.items()}
    report = {"skipped": [], "drift_exact": [], "removed_total": 0, "per_file": {}, "not_near_named": []}
    pruned = {}
    for name, base in committed.items():
        if name not in old_cap or name not in new_cap:
            report["skipped"].append((name, "case not produced by the capture run"))
            continue
        old_regions, new_regions = by_region(old_cap[name]["unnamed"]), by_region(new_cap[name]["unnamed"])
        base_regions = by_region(base["unnamed"])
        drop = {}  # region -> set(index)
        ok = True
        for reg in sorted(set(old_regions) | set(new_regions) | set(base_regions)):
            old_l, new_l, base_l = old_regions.get(reg, []), new_regions.get(reg, []), base_regions.get(reg, [])
            if len(old_l) != len(base_l):
                report["skipped"].append((name, f"region {reg}: committed {len(base_l)} unnamed, old capture {len(old_l)}"))
                ok = False
                continue
            if old_l != base_l:
                report["drift_exact"].append((name, reg))
            try:
                gone = twin_indices(old_l, new_l)
            except ValueError as e:
                report["skipped"].append((name, f"region {reg}: {e}"))
                ok = False
                continue
            if gone:
                drop[reg] = set(gone)
        if not drop:
            continue
        seen, kept, removed = {}, [], []
        for u in base["unnamed"]:
            k = seen.get(u["region"], 0)
            seen[u["region"]] = k + 1
            (removed if k in drop.get(u["region"], ()) else kept).append(u)
        if not ok:
            # some region could not be matched: still prune the regions that could, they are independent lists
            pass
        pruned[name] = dict(base, unnamed=kept)
        report["per_file"][name] = len(removed)
        report["removed_total"] += len(removed)
        for u in removed:
            if not near_named(u["rect"], base["named"]):
                report["not_near_named"].append((name, u["region"], u["rect"]))

    for name, doc in pruned.items():
        (BDIR / name).write_text(json.dumps(doc, indent=2) + "\n")

    # Structural diff against the committed text: only `unnamed` deletions.
    violations = []
    for name, doc in pruned.items():
        before = committed[name]
        for k in before:
            if k != "unnamed" and before[k] != doc[k]:
                violations.append((name, f"field {k} changed"))
        it = iter(before["unnamed"])
        if not all(any(u == v for v in it) for u in doc["unnamed"]):
            violations.append((name, "unnamed is not an in-order subset of the committed list"))
    print(f"files pruned: {len(report['per_file'])}   rects removed: {report['removed_total']}")
    for n, c in sorted(report["per_file"].items()):
        print(f"  {n}: {c}")
    print(f"structural violations (anything but unnamed deletions): {len(violations)}")
    for v in violations:
        print("  ", v)
    print(f"removed rects not inside a same-x-extent named rect of their baseline: {len(report['not_near_named'])}")
    for v in report["not_near_named"]:
        print("  ", v)
    print(f"case/region pairs whose old capture differs from the committed baseline in rects (index pairing only): {len(report['drift_exact'])}")
    print(f"case/regions NOT pruned (count mismatch or non-subsequence): {len(report['skipped'])}")
    for s in report["skipped"]:
        print("  ", s)
    sys.exit(1 if violations or report["skipped"] else 0)


if __name__ == "__main__":
    main()
