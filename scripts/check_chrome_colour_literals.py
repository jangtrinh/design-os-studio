#!/usr/bin/env python3
"""
DESIGN:OS Studio — Chrome Colour Literal Gate

Counts inline `Color32` literal sites per Rust source file in
`crates/designos-app/src/` (excluding `theme.rs` and test files/blocks) using
the regex below over `rust_literal_scan.clean` output, ratcheted against
`scripts/chrome_colour_baseline.json`.

A chrome colour site is:
  - `Color32::from_rgb*` / `Color32::from_rgba*` with literal numeric first arg
  - `Color32::from_gray` with a literal numeric arg
  - `Color32::from_black_alpha` / `Color32::from_white_alpha`
  - Named `Color32` constants (`WHITE`, `BLACK`, `GRAY`, `LIGHT_GRAY`,
    `DARK_GRAY`, `RED`, `GREEN`, `BLUE`, `YELLOW`) — `TRANSPARENT` is excluded.

Counts may only fall. An optional per-file `allowlist` in the baseline JSON
may exempt specific line numbers or substrings for identity tints and canvas
exceptions.

Usage:
  python3 scripts/check_chrome_colour_literals.py
  python3 scripts/check_chrome_colour_literals.py --verbose
  python3 scripts/check_chrome_colour_literals.py --write-baseline
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

from rust_literal_scan import clean

REPO_ROOT = Path(__file__).resolve().parent.parent
APP_SRC = REPO_ROOT / "crates" / "designos-app" / "src"
BASELINE_PATH = Path(__file__).resolve().parent / "chrome_colour_baseline.json"

EXEMPT_FILES = {
    "theme.rs",
}

HEX_RE = re.compile(
    r"Color32::from_rgba?(?:_unmultiplied|_premultiplied)?\(\s*(?:0x[0-9A-Fa-f]+|\d+)\s*,"
    r"|Color32::from_gray\(\s*(?:0x[0-9A-Fa-f]+|\d+)\s*\)"
    r"|Color32::from_(?:black|white)_alpha\("
    r"|Color32::(?:WHITE|BLACK|GRAY|LIGHT_GRAY|DARK_GRAY|RED|GREEN|BLUE|YELLOW)\b"
)


def is_test_file(rel: str) -> bool:
    p = Path(rel)
    return p.stem.endswith("_tests") or "tests" in p.parts


def _is_allowlisted(lineno: int, raw_line: str, entries: list[int | str]) -> bool:
    for entry in entries:
        if isinstance(entry, int) and entry == lineno:
            return True
        if isinstance(entry, str):
            if entry.isdigit() and int(entry) == lineno:
                return True
            if entry in raw_line:
                return True
    return False


def scan_source(
    src: str, allowlist_entries: list[int | str] | None = None
) -> list[tuple[int, str]]:
    """Return `(lineno, matched_text)` for every chrome colour site in `src`."""
    cleaned_lines = clean(src).split("\n")
    raw_lines = src.split("\n")
    entries = allowlist_entries or []
    hits: list[tuple[int, str]] = []
    for idx, line in enumerate(cleaned_lines):
        lineno = idx + 1
        raw_line = raw_lines[idx] if idx < len(raw_lines) else line
        if entries and _is_allowlisted(lineno, raw_line, entries):
            continue
        for m in HEX_RE.finditer(line):
            hits.append((lineno, m.group(0)))
    return hits


def scan_tree(
    src_root: Path = APP_SRC,
    allowlist: dict[str, list[int | str]] | None = None,
) -> dict[str, list[tuple[int, str]]]:
    allow = allowlist or {}
    results: dict[str, list[tuple[int, str]]] = {}
    for path in sorted(src_root.rglob("*.rs")):
        rel = path.relative_to(src_root).as_posix()
        if rel in EXEMPT_FILES or is_test_file(rel):
            continue
        src = path.read_text(encoding="utf-8")
        hits = scan_source(src, allow.get(rel))
        if hits:
            results[rel] = hits
    return results


def counts_for_Baseline(
    results: dict[str, list[tuple[int, str]]],
) -> dict[str, dict[str, int]]:
    out: dict[str, dict[str, int]] = {}
    for rel, hits in sorted(results.items()):
        if hits:
            out[rel] = {"chrome_sites": len(hits)}
    return out


def _baseline_count(entry: int | dict[str, int] | None) -> int:
    if entry is None:
        return 0
    if isinstance(entry, int):
        return entry
    return int(entry.get("chrome_sites", 0))


def check_against_baseline(
    results: dict[str, list[tuple[int, str]]],
    baseline_files: dict[str, int | dict[str, int]],
) -> list[str]:
    violations: list[str] = []
    all_files = sorted(set(results.keys()) | set(baseline_files.keys()))
    for rel in all_files:
        hits = results.get(rel, [])
        actual = len(hits)
        allowed = _baseline_count(baseline_files.get(rel))
        if actual > allowed:
            sample = ", ".join(f"L{ln}:{txt}" for ln, txt in hits[:5])
            if len(hits) > 5:
                sample += f", … (+{len(hits) - 5} more)"
            violations.append(
                f"  {rel} [chrome_sites]: {actual} > baseline {allowed} ({sample})"
            )
    return violations


def write_baseline(
    baseline_path: Path,
    current_counts: dict[str, dict[str, int]],
    allowlist: dict[str, list[int | str]],
) -> tuple[bool, list[str]]:
    """Update baseline downwards only; refuse if any file's count rose."""
    if baseline_path.exists():
        old_data = json.loads(baseline_path.read_text(encoding="utf-8"))
        old_files = old_data.get("files", {})
        errors: list[str] = []
        for rel, cur in sorted(current_counts.items()):
            actual = cur["chrome_sites"]
            allowed = _baseline_count(old_files.get(rel))
            if actual > allowed:
                errors.append(
                    f"  {rel} [chrome_sites]: {actual} > existing baseline {allowed} (refusing to raise baseline)"
                )
        if errors:
            return False, errors

    payload = {
        "allowlist": allowlist,
        "files": current_counts,
    }
    baseline_path.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    return True, []


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--write-baseline",
        action="store_true",
        help="Write current counts to scripts/chrome_colour_baseline.json (only lowers counts)",
    )
    parser.add_argument(
        "--verbose",
        action="store_true",
        help="Print every matched chrome colour literal site",
    )
    parser.add_argument(
        "--src-root",
        type=Path,
        default=APP_SRC,
        help="Override crates/designos-app/src root directory",
    )
    parser.add_argument(
        "--baseline",
        type=Path,
        default=BASELINE_PATH,
        help="Override baseline JSON path",
    )
    args = parser.parse_args(argv)

    allowlist: dict[str, list[int | str]] = {}
    baseline_files: dict[str, int | dict[str, int]] = {}
    if args.baseline.exists():
        data = json.loads(args.baseline.read_text(encoding="utf-8"))
        allowlist = data.get("allowlist", {})
        baseline_files = data.get("files", {})

    results = scan_tree(args.src_root, allowlist)
    current = counts_for_Baseline(results)

    if args.write_baseline:
        ok, errs = write_baseline(args.baseline, current, allowlist)
        if not ok:
            print(
                "Chrome colour literal baseline update REFUSED (count increased):",
                file=sys.stderr,
            )
            for e in errs:
                print(e, file=sys.stderr)
            return 1
        total_sites = sum(v["chrome_sites"] for v in current.values())
        print(
            f"Wrote baseline to {args.baseline} ({len(current)} files, {total_sites} chrome colour sites)."
        )
        return 0

    if not args.baseline.exists():
        print(
            f"Missing baseline file {args.baseline}. Run with --write-baseline to initialize.",
            file=sys.stderr,
        )
        return 1

    if args.verbose:
        for rel, hits in sorted(results.items()):
            print(f"{rel}: chrome_sites={len(hits)}")
            for ln, txt in hits:
                print(f"  L{ln}: {txt}")

    violations = check_against_baseline(results, baseline_files)
    total_sites = sum(len(h) for h in results.values())
    baseline_total = sum(_baseline_count(v) for v in baseline_files.values())

    if violations:
        print(
            "Chrome colour literal gate FAILED (new inline Color32 literals introduced):"
        )
        for v in violations:
            print(v)
        print(
            "\nUse semantic colour constants from `crate::theme` instead of inline `Color32` literals."
        )
        return 1

    print(
        f"Chrome colour literal gate PASSED ({len(results)} files with findings, "
        f"{total_sites}/{baseline_total} baseline chrome sites)."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
