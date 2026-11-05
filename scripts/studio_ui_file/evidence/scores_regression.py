"""The regression of one compare run against another, without Pillow or MCP: every row of a BASE `compare-scores.json` against the row of the same key in a HEAD one.

    python3 scripts/studio_ui_file evidence regress BASE.json HEAD.json

Rows are keyed by (set, variant, capture, label, icon). Every field is compared except `note` and the `finding` text (words, not numbers), so a `finding_pin` that moves while its score
holds is a changed row too: the pin is the measurement an accepted finding is held to, and a differ that skipped it counted a row as unchanged when its pin had moved (the Table/Data
`[322,204,449,41]` pin gained `mean_abs_rgb_diff 1.42`). Exit 1 when a BASE row is missing at HEAD or a row that passed at BASE fails at HEAD; changed rows are listed and do not fail the run.

    regress(base_rows, head_rows) -> {"base", "head", "missing", "identical", "changed": [(key, {field: (base, head)})], "pass_to_fail", "new"}
    lines(report)                 the text the command prints
"""
import argparse
import json
import pathlib

IGNORED = ("note", "finding")


def key(row):
    return (row["set"], row["variant"], row["capture"], row.get("label"), row.get("icon"))


def index(rows):
    out = {}
    for r in rows:
        if key(r) in out:
            raise ValueError(f"two rows have the key {key(r)}: a run holds one row per (set, variant, capture, label, icon)")
        out[key(r)] = r
    return out


def regress(base_rows, head_rows):
    base, head = index(base_rows), index(head_rows)
    changed, missing, pass_to_fail = [], [], []
    for k, b in base.items():
        if k not in head:
            missing.append(k)
            continue
        h = head[k]
        diff = {f: (b.get(f), h.get(f)) for f in sorted(set(b) | set(h)) if f not in IGNORED and b.get(f) != h.get(f)}
        if diff:
            changed.append((k, diff))
        if not b.get("bad") and h.get("bad"):
            pass_to_fail.append(k)
    return {"base": len(base), "head": len(head), "missing": missing, "changed": changed, "pass_to_fail": pass_to_fail,
            "identical": len(base) - len(missing) - len(changed), "new": sum(1 for k in head if k not in base)}


def lines(r):
    out = [f"base rows {r['base']}, head rows {r['head']}, missing at head {len(r['missing'])}, identical {r['identical']}, changed {len(r['changed'])}, new at head {r['new']}"]
    out += [f"MISSING {k}" for k in r["missing"]]
    for k, diff in r["changed"]:
        out.append(f"CHANGED {k[0]} | {k[1]} | {k[2]}" + (f" | {k[3]}" if k[3] else ""))
        out += [f"    {f} {a} -> {b}" for f, (a, b) in diff.items()]
    return out + [f"rows passing at base but failing at head: {len(r['pass_to_fail'])}"]


def main(argv=None):
    ap = argparse.ArgumentParser(prog="studio_ui_file evidence regress", description=__doc__.split("\n")[0])
    ap.add_argument("base", help="compare-scores.json of the earlier run")
    ap.add_argument("head", help="compare-scores.json of the later run")
    a = ap.parse_args(argv)
    r = regress(*(json.loads(pathlib.Path(p).read_text(encoding="utf-8")) for p in (a.base, a.head)))
    print("\n".join(lines(r)))
    return 1 if r["missing"] or r["pass_to_fail"] else 0
