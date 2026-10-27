"""Shipped resource, starter asset, legacy demo removal, and examples/ hygiene checks (#1212).

Covers:
  - AC-03: no `showcase::`, `board::demo`, `decks::demo`, or `demo::demo_document` in shipped
    (non-test, non-`fixtures`) Rust code.
  - AC-12: starter asset `crates/designos-app/assets/starter/designos-studio-starter.designos`
    exists, is <= 5 MB, decodes as valid JSON with an `About this file` page, and shipped
    resources (`crates/*/assets`, `resources/`, installer configs, and decoded starter asset)
    contain zero matches for `(?i)untitled ?ui|untitledui`.
  - `examples/` removal: `git ls-files examples/` is empty, `examples/` does not exist, and
    `docs/`, `scripts/`, `README.md`, `.codex-plugin/`, and test code (`crates/*/tests/`,
    `*_tests.rs`, `#[cfg(test)]` modules) contain no repo-relative references into `examples/`
    outside explicitly named exclusions.

Run: python3 -B -m unittest discover -s scripts/tests -p "test_*.py"
"""

from __future__ import annotations

import json
import re
import subprocess
import unittest
from pathlib import Path
from typing import Dict, List, Set, Tuple

REPO = Path(__file__).resolve().parent.parent.parent
STARTER_ASSET = (
    REPO / "crates" / "designos-app" / "assets" / "starter" / "designos-studio-starter.designos"
)
STARTER_MAX_BYTES = 5 * 1024 * 1024

LEGACY_DEMO_RE = re.compile(r"\b(?:showcase::|board::demo|decks::demo|demo::demo_document)\b")
UNTITLED_UI_RE = re.compile(r"untitled ?ui|untitledui", re.IGNORECASE)

# Explicit exclusions for `examples/` substring matches, each with its documented reason.
EXAMPLES_ALLOWED_HITS: List[Tuple[str, str, str]] = [
    (
        "crates/designos-app/src/specs.rs",
        "examples/specs",
        "Export Frame and workspace spec discovery look at examples/specs inside the user's opened workspace (#1073)",
    ),
    (
        "crates/designos-app/src/agent_session_files/fs_sites_allowlist.txt",
        "examples/specs",
        "Allowlist entry documenting specs.rs Export Frame output in the user's workspace (#1073)",
    ),
    (
        "crates/designos-gpu/examples/bench_gpu.rs",
        "designos-render/examples/bench_big.rs",
        "Refers to a Cargo crate example target under crates/designos-render/examples/, not the deleted root examples/ folder",
    ),
    (
        "crates/designos-gpu/tests/common/demo.rs",
        "examples/render_demo.rs",
        "Refers to the designos-render Cargo crate example target, not the deleted root examples/ folder",
    ),
    (
        "docs/specs/v0.6/E15-20-style-preset-gallery.md",
        "crates/designos-render/examples/style_thumbnails.rs",
        "Refers to a Cargo crate example target under crates/designos-render/examples/, not the deleted root examples/ folder",
    ),
    (
        "docs/specs/v0.6/E15-47-designos-skills-pack.md",
        "`examples/` (each a NodeSpec batch",
        "Describes the per-skill examples/ subfolder inside a skill pack directory layout",
    ),
    (
        "crates/designos-app/tests/fixtures/designos-studio-landing.designos",
        "--apply-spec examples/specs/designos-studio-landing.spec.json",
        "Test fixture moved from examples/landing/ per #1212; contains sample CLI text inside a serialized Text node",
    ),
]


def _is_test_or_fixtures_cfg(attr_text: str) -> bool:
    compact = re.sub(r"\s+", "", attr_text)
    return (
        "cfg(test)" in compact
        or 'feature="fixtures"' in compact
        or "cfg(any(test,feature=\"fixtures\"))" in compact
    )


def _strip_comments_and_gated_items(src: str) -> Tuple[str, Set[str], bool]:
    """Strips comments and items gated by `#[cfg(test)]` or `#[cfg(any(test, feature = "fixtures"))]`.

    Also returns the set of child module names (`mod foo;`) that were gated, and whether the file
    itself has a top-level `#![cfg(test)]` or `#![cfg(any(test, feature = "fixtures"))]`.
    """
    # Blank out line comments (outside strings) so braces inside comments do not skew depth.
    lines = []
    for line in src.splitlines():
        stripped = line.lstrip()
        if stripped.startswith("//"):
            lines.append("")
        else:
            lines.append(line)
    clean = "\n".join(lines)

    file_gated = bool(
        re.search(
            r"#!\s*\[\s*cfg\s*\(\s*(?:test|any\s*\(\s*test\s*,\s*feature\s*=\s*\"fixtures\"\s*\))\s*\)\s*\]",
            clean,
        )
    )
    gated_mods: Set[str] = set()
    for m in re.finditer(
        r"#\s*\[\s*cfg\s*\(([^]]*)\)\s*\]\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+([A-Za-z0-9_]+)\s*;",
        clean,
    ):
        if _is_test_or_fixtures_cfg(f"cfg({m.group(1)})"):
            gated_mods.add(m.group(2))

    # Strip gated blocks/items (`#[cfg(test)]` or `#[cfg(any(test, feature = "fixtures"))]`).
    out: List[str] = []
    i = 0
    n = len(clean)
    cfg_re = re.compile(r"#\s*\[\s*cfg\s*\(([^]]*)\)\s*\]")
    while i < n:
        m = cfg_re.match(clean, i)
        if m and _is_test_or_fixtures_cfg(f"cfg({m.group(1)})"):
            j = m.end()
            # Advance past any additional attributes before the item.
            while j < n:
                while j < n and clean[j].isspace():
                    j += 1
                if clean.startswith("#[", j):
                    close = clean.find("]", j + 2)
                    j = n if close == -1 else close + 1
                else:
                    break
            # Skip the item that follows: either a semicolon-terminated item or a brace-delimited item/arm.
            depth = 0
            saw_brace = False
            while j < n:
                ch = clean[j]
                if ch == '"':
                    j += 1
                    while j < n:
                        if clean[j] == "\\":
                            j += 2
                        elif clean[j] == '"':
                            j += 1
                            break
                        else:
                            j += 1
                    continue
                if ch == "{":
                    depth += 1
                    saw_brace = True
                elif ch == "}":
                    depth -= 1
                    if saw_brace and depth <= 0:
                        j += 1
                        if j < n and clean[j] == ",":
                            j += 1
                        break
                elif ch in (";", ",") and not saw_brace and depth == 0:
                    j += 1
                    break
                j += 1
            i = j
            continue
        out.append(clean[i])
        i += 1
    return "".join(out), gated_mods, file_gated


def _shipped_rust_files(crate_src: Path) -> Dict[str, str]:
    raw_by_path: Dict[Path, Tuple[str, Set[str], bool]] = {}
    for p in sorted(crate_src.rglob("*.rs")):
        rel = p.relative_to(crate_src).as_posix()
        if (
            rel == "tests.rs"
            or rel.endswith("/tests.rs")
            or rel.endswith("_tests.rs")
            or rel.startswith("test_")
            or "/tests/" in f"/{rel}"
        ):
            continue
        stripped, gated_mods, file_gated = _strip_comments_and_gated_items(
            p.read_text(encoding="utf-8")
        )
        raw_by_path[p] = (stripped, gated_mods, file_gated)

    excluded_dirs: Set[Path] = set()
    excluded_files: Set[Path] = set()
    for p, (_, gated_mods, file_gated) in raw_by_path.items():
        if file_gated:
            excluded_files.add(p)
            if p.name == "mod.rs":
                excluded_dirs.add(p.parent)
        base = p.parent if p.name in ("main.rs", "lib.rs", "mod.rs") else p.with_suffix("")
        for mod_name in gated_mods:
            flat = base / f"{mod_name}.rs"
            nested = base / mod_name / "mod.rs"
            excluded_files.add(flat)
            excluded_files.add(nested)
            excluded_dirs.add(base / mod_name)

    shipped: Dict[str, str] = {}
    for p, (stripped, _, _) in raw_by_path.items():
        if p in excluded_files or any(d in p.parents for d in excluded_dirs):
            continue
        shipped[p.relative_to(REPO).as_posix()] = stripped
    return shipped


class ShippedResourcesTest(unittest.TestCase):
    def test_ac03_no_legacy_demos_in_shipped_code(self) -> None:
        hits: List[str] = []
        for crate in ("designos-app", "designos-library"):
            shipped = _shipped_rust_files(REPO / "crates" / crate / "src")
            for rel, code in shipped.items():
                for lineno, line in enumerate(code.splitlines(), 1):
                    if LEGACY_DEMO_RE.search(line):
                        hits.append(f"{rel}:{lineno}: {line.strip()}")
        self.assertEqual(
            hits,
            [],
            "AC-03 violation — legacy demo builders referenced in shipped non-test, non-fixtures code:\n"
            + "\n".join(hits),
        )

    def test_ac12_no_untitled_ui_in_shipped_resources_or_starter(self) -> None:
        self.assertTrue(
            STARTER_ASSET.is_file(),
            f"Missing starter asset at {STARTER_ASSET.relative_to(REPO)}",
        )
        size = STARTER_ASSET.stat().st_size
        self.assertLessEqual(
            size,
            STARTER_MAX_BYTES,
            f"Starter asset {STARTER_ASSET.relative_to(REPO)} is {size} bytes (> 5 MB cap)",
        )
        starter_text = STARTER_ASSET.read_text(encoding="utf-8")
        starter_doc = json.loads(starter_text)
        pages = starter_doc.get("pages", [])
        nodes = starter_doc.get("nodes", {})
        page_names = [nodes.get(str(pid), {}).get("name") for pid in pages]
        self.assertIn(
            "About this file",
            page_names,
            f"Starter asset must include an 'About this file' page, found {page_names}",
        )

        scan_paths: List[Path] = []
        for assets_dir in sorted((REPO / "crates").glob("*/assets")):
            scan_paths.extend(p for p in sorted(assets_dir.rglob("*")) if p.is_file())
        for extra_dir in (REPO / "resources", REPO / "packaging"):
            if extra_dir.is_dir():
                scan_paths.extend(p for p in sorted(extra_dir.rglob("*")) if p.is_file())
        for pkg_script in sorted((REPO / "scripts").glob("package*")):
            if pkg_script.is_file():
                scan_paths.append(pkg_script)

        hits: List[str] = []
        for path in scan_paths:
            rel = path.relative_to(REPO).as_posix()
            text = path.read_text(encoding="utf-8", errors="ignore")
            for lineno, line in enumerate(text.splitlines(), 1):
                if UNTITLED_UI_RE.search(line):
                    hits.append(f"{rel}:{lineno}")
        self.assertEqual(
            hits,
            [],
            "AC-12 violation — 'Untitled UI' / 'untitledui' found in shipped resources:\n"
            + "\n".join(hits),
        )

    def test_examples_removed_and_unreferenced(self) -> None:
        proc = subprocess.run(
            ["git", "ls-files", "examples/"],
            cwd=REPO,
            capture_output=True,
            text=True,
            check=True,
        )
        tracked = [line for line in proc.stdout.splitlines() if line.strip()]
        self.assertEqual(
            tracked,
            [],
            f"git ls-files examples/ must be empty, found: {tracked}",
        )
        self.assertFalse(
            (REPO / "examples").exists(),
            "Top-level examples/ directory must not exist on disk",
        )

        # Scan docs/, scripts/ (except this test file), README.md, .codex-plugin/, and test files in crates/.
        candidates: List[Path] = []
        for root_file in (REPO / "README.md", REPO / ".codex-plugin" / "plugin.json"):
            if root_file.is_file():
                candidates.append(root_file)
        for folder in (REPO / "docs", REPO / "scripts"):
            if folder.is_dir():
                for p in sorted(folder.rglob("*")):
                    if p.is_file() and p.resolve() != Path(__file__).resolve() and "__pycache__" not in p.parts:
                        candidates.append(p)
        for p in sorted((REPO / "crates").rglob("*")):
            if not p.is_file():
                continue
            rel = p.relative_to(REPO).as_posix()
            if (
                "/tests/" in f"/{rel}"
                or rel.endswith("_tests.rs")
                or rel.endswith("/tests.rs")
                or p.suffix in (".rs", ".txt")
            ):
                candidates.append(p)

        unexpected: List[str] = []
        seen_seen: Set[str] = set()
        for p in candidates:
            rel = p.relative_to(REPO).as_posix()
            if rel in seen_seen:
                continue
            seen_seen.add(rel)
            text = p.read_text(encoding="utf-8", errors="ignore")
            for lineno, line in enumerate(text.splitlines(), 1):
                if "examples/" not in line:
                    continue
                allowed = any(
                    rel == allow_rel and allow_needle in line
                    for allow_rel, allow_needle, _ in EXAMPLES_ALLOWED_HITS
                )
                if not allowed:
                    unexpected.append(f"{rel}:{lineno}: {line.strip()}")

        self.assertEqual(
            unexpected,
            [],
            "Unexpected references to examples/ found:\n" + "\n".join(unexpected),
        )


if __name__ == "__main__":
    unittest.main()
