#!/usr/bin/env python3
"""Spacing / radius / type ratchet for `crates/designos-app/src`.

Counts literal geometry outside `theme.rs` (offset, inset, radius and font-size
literals) and non-concentric nested radius pairs, per file. It fails when any
file's count rises above `scripts/spacing_baseline.json`, or when a file that is
not in the baseline gains a literal. Counts may only go down.

  python3 scripts/check_spacing_relationships.py              # check (exit 0 / 1)
  python3 scripts/check_spacing_relationships.py --verbose    # list every site
  python3 scripts/check_spacing_relationships.py --write-baseline
                          # lower the baseline to today's counts; refuses a rise
  python3 scripts/check_spacing_relationships.py --src DIR --baseline FILE
                          # run against another tree (the test fixtures do)

Exit codes: 0 pass, 1 ratchet violated, 2 usage or I/O error.
Counting rules and their limits live in `rust_literal_scan.py`.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Dict, List

sys.path.insert(0, str(Path(__file__).resolve().parent))
from rust_literal_scan import CATEGORIES, parse_theme_consts, scan_file  # noqa: E402

REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_SRC = REPO_ROOT / "crates" / "designos-app" / "src"
DEFAULT_BASELINE = REPO_ROOT / "scripts" / "spacing_baseline.json"
# The only file allowed to hold literal geometry: it defines the constants.
THEME_FILE = "theme.rs"
# Own-palette readers, reported as a separate headline (the audit excluded them).
OWN_PALETTE = ("specs.rs", "terminal.rs")

Counts = Dict[str, Dict[str, int]]


def is_test_file(rel: str) -> bool:
    name = rel.rsplit("/", 1)[-1]
    return (
        name in ("tests.rs", "test_timing.rs")
        or name.endswith("_tests.rs")
        or name.startswith("test_")
        or "/tests/" in f"/{rel}"
        or "fixture" in rel
    )


def collect(src: Path) -> Dict[str, Dict[str, List[int]]]:
    """Per-file literal sites (line numbers by category), theme.rs excluded."""
    theme = src / THEME_FILE
    consts = parse_theme_consts(theme.read_text(encoding="utf-8")) if theme.exists() else {}
    result: Dict[str, Dict[str, List[int]]] = {}
    for path in sorted(src.rglob("*.rs")):
        rel = path.relative_to(src).as_posix()
        if rel == THEME_FILE or is_test_file(rel):
            continue
        sites = scan_file(path.read_text(encoding="utf-8", errors="replace"), consts)
        if any(sites[c] for c in CATEGORIES):
            result[rel] = sites
    return result


def to_counts(sites: Dict[str, Dict[str, List[int]]]) -> Counts:
    return {
        rel: {c: len(s[c]) for c in CATEGORIES if s[c]} for rel, s in sites.items()
    }


def totals(counts: Counts, skip: tuple = ()) -> Dict[str, int]:
    out = {c: 0 for c in CATEGORIES}
    for rel, per in counts.items():
        if rel in skip:
            continue
        for c, n in per.items():
            out[c] += n
    return out


def compare(current: Counts, baseline: Counts) -> Dict[str, List[tuple]]:
    """Rises (violations) and drops (baseline can be lowered), as (file, cat, old, new)."""
    rises: List[tuple] = []
    drops: List[tuple] = []
    for rel in sorted(set(current) | set(baseline)):
        for c in CATEGORIES:
            old = baseline.get(rel, {}).get(c, 0)
            new = current.get(rel, {}).get(c, 0)
            if new > old:
                rises.append((rel, c, old, new))
            elif new < old:
                drops.append((rel, c, old, new))
    return {"rises": rises, "drops": drops}


def load_baseline(path: Path) -> Counts:
    data = json.loads(path.read_text(encoding="utf-8"))
    files = data.get("files")
    if not isinstance(files, dict):
        raise ValueError(f"{path}: missing 'files' object")
    for rel, per in files.items():
        unknown = set(per) - set(CATEGORIES)
        if unknown:
            raise ValueError(f"{path}: {rel} has unknown categories {sorted(unknown)}")
    return files


def write_baseline(path: Path, counts: Counts) -> None:
    doc = {
        "version": 1,
        "note": (
            "Ratchet baseline for scripts/check_spacing_relationships.py. Per-file counts of "
            "literal geometry outside theme.rs. Counts may only fall: lower them with "
            "--write-baseline, which refuses to raise any entry."
        ),
        "totals": totals(counts),
        "totals_excluding_own_palette": totals(counts, OWN_PALETTE),
        "files": counts,
    }
    path.write_text(json.dumps(doc, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def main(argv: List[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    ap.add_argument("--src", type=Path, default=DEFAULT_SRC, help="Rust source root to scan")
    ap.add_argument("--baseline", type=Path, default=DEFAULT_BASELINE, help="baseline JSON file")
    ap.add_argument("--write-baseline", action="store_true", help="lower the baseline to today's counts")
    ap.add_argument("--verbose", action="store_true", help="list every counted site")
    ap.add_argument("--json", action="store_true", help="print a machine-readable report")
    args = ap.parse_args(argv)

    if not args.src.is_dir():
        print(f"error: source root not found: {args.src}", file=sys.stderr)
        return 2
    sites = collect(args.src)
    current = to_counts(sites)

    if args.write_baseline:
        if args.baseline.exists():
            try:
                baseline = load_baseline(args.baseline)
            except (OSError, ValueError, json.JSONDecodeError) as exc:
                print(f"error: {exc}", file=sys.stderr)
                return 2
            rises = compare(current, baseline)["rises"]
            if rises:
                print("refusing to raise the baseline:", file=sys.stderr)
                for rel, c, old, new in rises:
                    print(f"  {rel} {c}: {old} -> {new}", file=sys.stderr)
                return 1
        write_baseline(args.baseline, current)
        print(f"baseline written: {args.baseline}")
        return 0

    try:
        baseline = load_baseline(args.baseline)
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        print(f"error: cannot read baseline {args.baseline}: {exc}", file=sys.stderr)
        return 2
    diff = compare(current, baseline)
    ok = not diff["rises"]

    if args.json:
        print(
            json.dumps(
                {
                    "ok": ok,
                    "totals": totals(current),
                    "totals_excluding_own_palette": totals(current, OWN_PALETTE),
                    "violations": [
                        {"file": f, "category": c, "baseline": o, "current": n}
                        for f, c, o, n in diff["rises"]
                    ],
                    "below_baseline": len(diff["drops"]),
                },
                indent=2,
            )
        )
    else:
        t = totals(current)
        print("literal geometry outside theme.rs: " + ", ".join(f"{c}={t[c]}" for c in CATEGORIES))
        for rel, c, old, new in diff["rises"]:
            lines = ",".join(str(n) for n in sites[rel][c])
            print(f"FAIL {rel}: {c} rose {old} -> {new} (lines {lines})")
        if diff["drops"]:
            print(
                f"note: {len(diff['drops'])} entries are below baseline; "
                "run --write-baseline to lock the gain in"
            )
        if args.verbose:
            for rel, per in sorted(sites.items()):
                for c in CATEGORIES:
                    if per[c]:
                        print(f"  {rel} {c}: lines {','.join(str(n) for n in per[c])}")
        print("PASS: no count rose above the baseline" if ok else "FAIL: ratchet violated")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
