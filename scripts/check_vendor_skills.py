#!/usr/bin/env python3
"""Verify, extract, and validate rule mappings for vendored MIT UI skills (E15-46 / #636).

Usage:
    python3 scripts/check_vendor_skills.py verify [--online] [--repo-root PATH]
    python3 scripts/check_vendor_skills.py extract [--write] [--repo-root PATH]
    python3 scripts/check_vendor_skills.py map-coverage [--write-canvas] [--repo-root PATH]
"""

from __future__ import annotations

import argparse
import hashlib
import os
import re
import subprocess
import sys
import tomllib
import urllib.request
from pathlib import Path
from typing import Dict, List, Optional, Set, Tuple

PINNED_COMMIT = "4c66b1fa86e9ab70187b43d7da9b6fdb29f2ef63"
PINNED_LICENSE_SHA1 = "7ec4838821b03d1c3b4388c5a50c94fd459ff557"
VENDOR_REL_DIR = Path("skills/vendor")
JAKUB_REL_DIR = VENDOR_REL_DIR / "jakubkrehel-skills"

ATTRIBUTION_HEADER = (
    "Portions of this folder are copied from or derived from jakubkrehel/skills "
    "(https://github.com/jakubkrehel/skills), commit 4c66b1fa86e9ab70187b43d7da9b6fdb29f2ef63, "
    "Copyright (c) 2026 Jakub Krehel, MIT License (see LICENSE)."
)

# Canonical E15-9 / E15-10 / E15-11 + Check-Designs lint rule registry IDs
CANONICAL_LINT_RULE_IDS: Set[str] = {
    "overflow-parent",
    "overflowing-children",
    "clipped-partial",
    "clipped-full",
    "sibling-overlap",
    "overlapping-siblings",
    "text-truncated",
    "text-unexpected-wrap",
    "collapsed-size",
    "zero-bounds",
    "sizing-conflict",
    "text-missing-fill",
    "absolute-in-auto-layout",
    "placeholder-text",
    "contrast",
    "low-contrast-text",
    "off-scale",
    "inconsistent-spacing",
    "non-scale-font-size",
    "inconsistent-radius",
    "hardcoded-value",
    "hardcoded-color",
    "hardcoded-number",
    "unstyled-text",
    "detached-instance",
    "missing-auto-layout",
    "tap-target-size",
}

# Canonical E15-31 / E15-34 critique rubric axes
CANONICAL_RUBRIC_AXES: Set[str] = {
    "layout",
    "typography",
    "spacing",
    "iconography",
    "depth",
    "consistency",
    "color",
    "motion",
}

# Canonical E15-47 skills
CANONICAL_SKILLS: Set[str] = {
    "designos-use",
    "generate-design",
    "generate-library",
    "critique",
    "design-os-studio",
}

# Third-party design tools forbidden in canvas_text (E15-46 §8.4: only Figma may be named)
FORBIDDEN_DESIGN_TOOLS_RE = re.compile(
    r"\b(?:sketch|penpot|lunacy|framer|adobe\s+xd|invision|zeplin|uxpin|canva)\b",
    re.IGNORECASE,
)

EXCLUDED_SOURCE_PATTERNS = ("transitions.dev", "transitions-dev")


def sha1_bytes(data: bytes) -> str:
    return hashlib.sha1(data).hexdigest()


def sha1_text(text: str) -> str:
    return hashlib.sha1(text.encode("utf-8")).hexdigest()


REGISTER_REL_PATH = Path("docs/eval-licences.md")


def load_vendor_register(repo_root: Path) -> Tuple[Dict[str, dict], Optional[str]]:
    """Read the E15-23 licence register (`docs/eval-licences.md`) and return `(approved, error)`.

    `approved` maps an id to its register entry for every entry that may ship in the skills pack: reviewed
    `approved`, a licence other than `NONE`, and use `ship-in-skills`. There is no built-in fallback: a missing or
    unparsable register means nothing is licensed, and `error` says why.
    """
    reg_path = repo_root / REGISTER_REL_PATH
    if not reg_path.is_file():
        return {}, f"{REGISTER_REL_PATH.as_posix()} does not exist, so no vendored folder has a licence on record"
    try:
        sys.path.insert(0, str(Path(__file__).resolve().parent))
        import check_eval_licences  # the register parser lives with the register checker

        parsed, _ = check_eval_licences.extract_register(reg_path.read_text(encoding="utf-8"))
        sources = parsed.get("source")
        if not isinstance(sources, list) or not sources:
            raise ValueError("no [[source]] entries")
    except Exception as exc:  # any parse failure must leave the vendor folders unlicensed
        return {}, f"{REGISTER_REL_PATH.as_posix()} cannot be read ({exc}), so no vendored folder has a licence on record"
    approved: Dict[str, dict] = {}
    for src in sources:
        licence = str(src.get("licence", "NONE")).strip()
        if (
            src.get("id")
            and src.get("review") == "approved"
            and licence.upper() not in ("", "NONE")
            and src.get("use") == "ship-in-skills"
        ):
            approved[str(src["id"])] = src
    return approved, None


def load_lint_rule_ids(repo_root: Path) -> Set[str]:
    """Lint rule IDs: the output of a built `designos-mcp --list-lint-rules` when there is one, else the canonical set."""
    ids = set(CANONICAL_LINT_RULE_IDS)
    target_dir = Path(os.environ.get("CARGO_TARGET_DIR", str(repo_root / "target")))
    for profile in ("debug", "release"):
        bin_path = target_dir / profile / "designos-mcp"
        if bin_path.is_file() and os.access(bin_path, os.X_OK):
            try:
                proc = subprocess.run(
                    [str(bin_path), "--list-lint-rules"],
                    capture_output=True,
                    text=True,
                    timeout=5,
                    check=True,
                )
                parsed = {line.strip() for line in proc.stdout.splitlines() if line.strip()}
                if parsed:
                    ids = parsed  # the built binary is the authority: an id it dropped is no longer valid
                    break
            except Exception:
                pass
    return ids


def load_surface_check_ids(repo_root: Path) -> Set[str]:
    """Surface check IDs read from the E15-21 data in assets/surfaces/*.json; none when that data is absent, so a
    `check:` mapping is `unknown-check-id` until E15-21 lands and cannot be validated against a built-in list."""
    ids: Set[str] = set()
    surfaces_dir = repo_root / "assets" / "surfaces"
    if surfaces_dir.is_dir():
        import json

        for p in sorted(surfaces_dir.glob("*.json")):
            try:
                doc = json.loads(p.read_text(encoding="utf-8"))
                surf_id = doc.get("id", p.stem)
                for r in doc.get("rules", []):
                    rid = r.get("id")
                    if rid:
                        ids.add(f"surface.{surf_id}.{rid}")
            except Exception:
                pass
    return ids


def extract_rules_from_markdown_file(md_path: Path) -> List[str]:
    """Extract top-level bullet or numbered items from a markdown body (skipping YAML frontmatter,
    headings, and fenced code blocks) per E15-46 §3.3.
    """
    raw = md_path.read_text(encoding="utf-8")
    lines = raw.splitlines()
    idx = 0
    if lines and lines[0].strip() == "---":
        idx = 1
        while idx < len(lines) and lines[idx].strip() != "---":
            idx += 1
        idx += 1

    in_code_block = False
    fence_marker: Optional[str] = None
    items: List[str] = []
    current_item_lines: Optional[List[str]] = None
    top_bullet_re = re.compile(r"^(?:[-*+]|\d+\.)\s+(.*\S.*)$")

    for line in lines[idx:]:
        stripped = line.lstrip()
        if stripped.startswith("```") or stripped.startswith("~~~"):
            marker = stripped[:3]
            if not in_code_block:
                in_code_block = True
                fence_marker = marker
            elif fence_marker and stripped.startswith(fence_marker):
                in_code_block = False
                fence_marker = None
            if current_item_lines is not None:
                items.append(" ".join(current_item_lines).strip())
                current_item_lines = None
            continue
        if in_code_block:
            continue

        m = top_bullet_re.match(line)
        if m:
            if current_item_lines is not None:
                items.append(" ".join(current_item_lines).strip())
            current_item_lines = [m.group(1).strip()]
        elif current_item_lines is not None:
            if (
                line.startswith("  ")
                and not re.match(r"^\s+(?:[-*+]|\d+\.)\s+", line)
                and line.strip()
            ):
                current_item_lines.append(line.strip())
            else:
                items.append(" ".join(current_item_lines).strip())
                current_item_lines = None

    if current_item_lines is not None:
        items.append(" ".join(current_item_lines).strip())
    return items


def extract_all_upstream_rules(upstream_skills_dir: Path) -> List[Tuple[str, str]]:
    """Return ordered list of `(rule_id, rule_text)` across all upstream skill `.md` files."""
    rules: List[Tuple[str, str]] = []
    for md_path in sorted(upstream_skills_dir.rglob("*.md")):
        rel = md_path.relative_to(upstream_skills_dir)
        skill = rel.parts[0]
        stem = md_path.stem
        extracted = extract_rules_from_markdown_file(md_path)
        for ordinal, text in enumerate(extracted, 1):
            rule_id = f"{skill}/{stem}#{ordinal}"
            rules.append((rule_id, text))
    return rules


def format_rules_lock(rules: List[Tuple[str, str]]) -> str:
    """Serialize extracted `(rule_id, rule_text)` pairs into deterministic `rules.lock` format."""
    lines = [
        f"# Generated by `python3 scripts/check_vendor_skills.py extract` at commit {PINNED_COMMIT}",
        "# Format: <rule_id>\\t<sha1>\\t<text>",
    ]
    for rule_id, text in rules:
        h = sha1_text(text)
        clean_text = text.replace("\t", " ").replace("\r", " ").replace("\n", " ")
        lines.append(f"{rule_id}\t{h}\t{clean_text}")
    return "\n".join(lines) + "\n"


def parse_rules_lock(lock_path: Path) -> List[Tuple[str, str, str]]:
    """Return list of `(rule_id, text_sha1, text)` from `rules.lock`."""
    entries: List[Tuple[str, str, str]] = []
    for raw_line in lock_path.read_text(encoding="utf-8").splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#"):
            continue
        parts = raw_line.split("\t", 2)
        if len(parts) != 3:
            raise ValueError(f"Malformed rules.lock line: {raw_line!r}")
        entries.append((parts[0], parts[1], parts[2]))
    return entries


def render_canvas_rules(rule_entries: List[dict]) -> str:
    """Render `canvas-rules.md` from non-`web-only` rules in `rule-map.toml`, grouped by skill."""
    lines: List[str] = [
        f"<!-- {ATTRIBUTION_HEADER} -->",
        "",
        "# jakubkrehel UI Rules (Canvas Edition)",
        "",
        f"> {ATTRIBUTION_HEADER}",
        "",
    ]

    by_skill: Dict[str, List[dict]] = {}
    for entry in rule_entries:
        if entry.get("map") == "web-only":
            continue
        rid = entry.get("id", "")
        skill = rid.split("/", 1)[0] if "/" in rid else "general"
        by_skill.setdefault(skill, []).append(entry)

    for skill in sorted(by_skill.keys()):
        lines.append(f"## {skill}")
        lines.append("")
        for entry in by_skill[skill]:
            rid = entry["id"]
            target = entry["map"]
            canvas_text = entry["canvas_text"].strip()
            lines.append(f"- **`{rid}`** (`{target}`): {canvas_text}")
        lines.append("")

    return "\n".join(lines).rstrip() + "\n"


def cmd_verify(repo_root: Path, online: bool = False) -> Tuple[int, List[str]]:
    vendor_dir = repo_root / VENDOR_REL_DIR
    if not vendor_dir.is_dir():
        return 2, [f"vendor-folder-missing: {vendor_dir} does not exist"]

    errors: List[str] = []
    approved, register_error = load_vendor_register(repo_root)
    if register_error:
        errors.append(f"unlicensed-file: {VENDOR_REL_DIR.as_posix()} ({register_error})")

    # Every folder under skills/vendor/ needs an approved, licensed, ship-in-skills register entry of the same id
    for child in sorted(vendor_dir.iterdir()):
        if child.name.startswith("."):
            continue
        if not child.is_dir():
            rel = child.relative_to(repo_root).as_posix()
            errors.append(f"unlicensed-file: {rel} (stray file in skills/vendor)")
            continue
        if child.name not in approved:
            reason = f"package '{child.name}' has no approved licensed ship-in-skills entry in {REGISTER_REL_PATH.as_posix()}"
            files = [f for f in sorted(child.rglob("*")) if f.is_file()]
            for f in files[:5]:
                errors.append(f"unlicensed-file: {f.relative_to(repo_root).as_posix()} ({reason})")
            if len(files) > 5:
                errors.append(f"unlicensed-file: {child.relative_to(repo_root).as_posix()} (and {len(files) - 5} more files; {reason})")
            if not files:
                errors.append(f"unlicensed-file: {child.relative_to(repo_root).as_posix()} ({reason})")

    # Scan all files under skills/vendor/ for excluded sources (e.g. transitions.dev)
    for f in sorted(vendor_dir.rglob("*")):
        if not f.is_file():
            continue
        rel = f.relative_to(repo_root).as_posix()
        rel_lower = rel.lower()
        if any(pat in rel_lower for pat in EXCLUDED_SOURCE_PATTERNS):
            msg = f"unlicensed-file: {rel} (excluded source path)"
            if msg not in errors:
                errors.append(msg)
            continue
        try:
            text_lower = f.read_text(encoding="utf-8", errors="ignore").lower()
            if any(pat in text_lower for pat in EXCLUDED_SOURCE_PATTERNS):
                errors.append(f"unlicensed-file: {rel} (references excluded source transitions.dev)")
        except Exception:
            pass

    jakub_dir = repo_root / JAKUB_REL_DIR
    upstream_toml_path = jakub_dir / "UPSTREAM.toml"
    if not upstream_toml_path.is_file():
        errors.append(f"licence-mismatch: missing {upstream_toml_path.relative_to(repo_root).as_posix()}")
        return 1, errors

    try:
        upstream_meta = tomllib.loads(upstream_toml_path.read_text(encoding="utf-8"))
    except Exception as exc:
        errors.append(f"licence-mismatch: invalid UPSTREAM.toml ({exc})")
        return 1, errors

    commit = upstream_meta.get("commit", "")
    if commit != PINNED_COMMIT:
        errors.append(f"licence-mismatch: UPSTREAM.toml commit {commit!r} != {PINNED_COMMIT!r}")

    recorded_lic_sha1 = upstream_meta.get("license_sha1", "")
    if recorded_lic_sha1 != PINNED_LICENSE_SHA1:
        errors.append(
            f"licence-mismatch: UPSTREAM.toml license_sha1 {recorded_lic_sha1!r} != {PINNED_LICENSE_SHA1!r}"
        )

    for lic_rel in ("LICENSE", "upstream/LICENSE"):
        lic_path = jakub_dir / lic_rel
        if not lic_path.is_file():
            errors.append(f"licence-mismatch: missing {lic_path.relative_to(repo_root).as_posix()}")
            continue
        lic_bytes = lic_path.read_bytes()
        actual_sha1 = sha1_bytes(lic_bytes)
        if actual_sha1 != recorded_lic_sha1:
            errors.append(
                f"licence-mismatch: {lic_path.relative_to(repo_root).as_posix()} sha1 {actual_sha1} != {recorded_lic_sha1}"
            )
        lic_text = lic_bytes.decode("utf-8", errors="replace")
        if "MIT License" not in lic_text or "Copyright (c) 2026 Jakub Krehel" not in lic_text:
            errors.append(
                f"licence-mismatch: {lic_path.relative_to(repo_root).as_posix()} missing MIT notice or copyright line"
            )

    attr_path = jakub_dir / "ATTRIBUTION.md"
    if not attr_path.is_file():
        errors.append("licence-mismatch: missing ATTRIBUTION.md")
    else:
        attr_text = attr_path.read_text(encoding="utf-8")
        for needle in (
            "jakubkrehel/skills",
            "https://github.com/jakubkrehel/skills",
            "4c66b1f",
            "Copyright (c) 2026 Jakub Krehel",
            "MIT License",
        ):
            if needle not in attr_text:
                errors.append(f"licence-mismatch: ATTRIBUTION.md missing {needle!r}")

    # The register entry and THIRD_PARTY_LICENSES.md must agree with the vendored copy (E15-46 §3.2, §3.5)
    entry = approved.get(JAKUB_REL_DIR.name)
    if entry is not None:
        if str(entry.get("pinned", "")) != PINNED_COMMIT:
            errors.append(
                f"licence-mismatch: {REGISTER_REL_PATH.as_posix()} pins {JAKUB_REL_DIR.name} at {entry.get('pinned')!r}, "
                f"the vendored copy is {PINNED_COMMIT!r}"
            )
        third_party = repo_root / "THIRD_PARTY_LICENSES.md"
        attribution = str(entry.get("attribution", "")).strip()
        if not third_party.is_file() or not attribution or attribution not in third_party.read_text(encoding="utf-8"):
            errors.append(
                "licence-mismatch: THIRD_PARTY_LICENSES.md does not carry the register attribution of "
                f"{JAKUB_REL_DIR.name} (run python3 scripts/check_eval_licences.py --write)"
            )

    if online:
        url = f"https://raw.githubusercontent.com/jakubkrehel/skills/{PINNED_COMMIT}/LICENSE"
        try:
            with urllib.request.urlopen(url, timeout=10) as resp:
                remote_bytes = resp.read()
            remote_sha1 = sha1_bytes(remote_bytes)
            if remote_sha1 != recorded_lic_sha1:
                errors.append(f"licence-mismatch: online GitHub LICENSE sha1 {remote_sha1} != {recorded_lic_sha1}")
        except Exception as exc:
            errors.append(f"licence-mismatch: failed to fetch {url}: {exc}")

    recorded_files: Dict[str, str] = upstream_meta.get("files", {})
    upstream_dir = jakub_dir / "upstream"
    if not upstream_dir.is_dir():
        errors.append("file-hash-mismatch: missing upstream/ directory")
    else:
        for rel_path, expected_sha1 in sorted(recorded_files.items()):
            fpath = upstream_dir / rel_path
            if not fpath.is_file():
                errors.append(f"file-hash-mismatch: missing upstream/{rel_path}")
                continue
            actual_sha1 = sha1_bytes(fpath.read_bytes())
            if actual_sha1 != expected_sha1:
                errors.append(
                    f"file-hash-mismatch: upstream/{rel_path} sha1 {actual_sha1} != {expected_sha1}"
                )
        for fpath in sorted(upstream_dir.rglob("*")):
            if fpath.is_file():
                rel_path = fpath.relative_to(upstream_dir).as_posix()
                if rel_path not in recorded_files:
                    errors.append(f"file-hash-mismatch: untracked file upstream/{rel_path}")

    return (1 if errors else 0), errors


def cmd_extract(repo_root: Path, write: bool = False) -> Tuple[int, List[str]]:
    jakub_dir = repo_root / JAKUB_REL_DIR
    upstream_skills_dir = jakub_dir / "upstream" / "skills"
    if not upstream_skills_dir.is_dir():
        return 2, [f"upstream-skills-missing: {upstream_skills_dir} does not exist"]

    rules = extract_all_upstream_rules(upstream_skills_dir)
    expected_lock = format_rules_lock(rules)
    lock_path = jakub_dir / "rules.lock"

    if write:
        lock_path.write_text(expected_lock, encoding="utf-8")
        return 0, [f"wrote {len(rules)} rules to {lock_path.relative_to(repo_root).as_posix()}"]

    if not lock_path.is_file():
        return 1, ["rules-lock-missing: rules.lock does not exist (run `extract --write`)"]

    committed = lock_path.read_text(encoding="utf-8")
    if committed != expected_lock:
        return 1, ["rules-lock-drift: committed rules.lock differs from extracted upstream rules"]

    return 0, [f"extract-ok: {len(rules)} rules match rules.lock"]


def cmd_map_coverage(repo_root: Path, write_canvas: bool = False) -> Tuple[int, List[str]]:
    jakub_dir = repo_root / JAKUB_REL_DIR
    lock_path = jakub_dir / "rules.lock"
    map_path = jakub_dir / "rule-map.toml"
    canvas_path = jakub_dir / "canvas-rules.md"

    if not lock_path.is_file() or not map_path.is_file():
        return 2, ["missing rules.lock or rule-map.toml"]

    errors: List[str] = []
    lock_entries = parse_rules_lock(lock_path)
    lock_by_id: Dict[str, Tuple[str, str]] = {rid: (sha1, txt) for rid, sha1, txt in lock_entries}

    try:
        map_doc = tomllib.loads(map_path.read_text(encoding="utf-8"))
    except Exception as exc:
        return 1, [f"invalid-rule-map: {exc}"]

    meta = map_doc.get("meta", {})
    if meta.get("upstream_commit") != PINNED_COMMIT:
        errors.append(
            f"commit-mismatch: rule-map.toml upstream_commit {meta.get('upstream_commit')!r} != {PINNED_COMMIT!r}"
        )

    rule_entries: List[dict] = map_doc.get("rule", [])
    lint_ids = load_lint_rule_ids(repo_root)
    check_ids = load_surface_check_ids(repo_root)

    seen_ids: Set[str] = set()
    canvas_text_owner: Dict[str, str] = {}
    for entry in rule_entries:
        rid = entry.get("id", "")
        if not rid:
            errors.append("unmapped-rule: rule entry missing 'id'")
            continue
        if rid in seen_ids:
            errors.append(f"duplicate-mapping: {rid}")
            continue
        seen_ids.add(rid)

        if rid not in lock_by_id:
            errors.append(f"unmapped-rule: {rid} (present in rule-map.toml but not in rules.lock)")
            continue

        expected_sha1, _ = lock_by_id[rid]
        rec_sha1 = entry.get("text_sha1")
        if not rec_sha1 or rec_sha1 != expected_sha1:
            errors.append(
                f"text-sha1-mismatch: {rid} has text_sha1={rec_sha1!r}, expected {expected_sha1!r}"
            )

        target = entry.get("map", "")
        if not target:
            errors.append(f"unmapped-rule: {rid} has empty 'map'")
            continue

        if target == "web-only":
            reason = entry.get("reason", "").strip()
            if not reason:
                errors.append(f"missing-reason: {rid} mapped to web-only without reason")
        else:
            if target.startswith("lint:"):
                lint_id = target[len("lint:") :]
                if lint_id not in lint_ids:
                    errors.append(f"unknown-lint-id: {lint_id} (in {rid})")
            elif target.startswith("check:"):
                check_id = target[len("check:") :]
                if check_id not in check_ids:
                    errors.append(f"unknown-check-id: {check_id} (in {rid})")
            elif target.startswith("rubric:"):
                axis = target[len("rubric:") :]
                if axis not in CANONICAL_RUBRIC_AXES:
                    errors.append(f"unknown-rubric-axis: {axis} (in {rid})")
            elif target.startswith("skill:"):
                rest = target[len("skill:") :]
                if "/" not in rest:
                    errors.append(f"invalid-skill-map: {target} (in {rid})")
                else:
                    sk, sec = rest.split("/", 1)
                    if sk not in CANONICAL_SKILLS or not sec.strip():
                        errors.append(f"invalid-skill-map: {target} (in {rid})")
            else:
                errors.append(f"invalid-map-target: {target} (in {rid})")

            canvas_text = entry.get("canvas_text", "").strip()
            if not canvas_text:
                errors.append(f"missing-canvas-text: {rid} ({target}) requires canvas_text")
            elif FORBIDDEN_DESIGN_TOOLS_RE.search(canvas_text):
                errors.append(
                    f"forbidden-tool-name: {rid} canvas_text names a third-party design tool other than Figma"
                )
            if canvas_text:
                # A canvas_text restates ONE locked rule. The same sentence on several rules is a group summary
                # standing in for the individual rules, which is how an unrelated rule ends up with wrong guidance.
                owner = canvas_text_owner.setdefault(canvas_text, rid)
                if canvas_text.endswith(f"({rid})"):
                    errors.append(f"generic-canvas-text: {rid} canvas_text only tags the group summary with the rule id; restate the rule")
                if owner != rid:
                    errors.append(f"duplicate-canvas-text: {rid} repeats the canvas_text of {owner}; restate its own locked rule")

    for rid in lock_by_id:
        if rid not in seen_ids:
            errors.append(f"unmapped-rule: {rid}")

    expected_canvas_md = render_canvas_rules(rule_entries)
    if write_canvas:
        canvas_path.write_text(expected_canvas_md, encoding="utf-8")
    else:
        if not canvas_path.is_file():
            errors.append("canvas-rules-missing: canvas-rules.md does not exist")
        else:
            actual_canvas_md = canvas_path.read_text(encoding="utf-8")
            if not actual_canvas_md.startswith(f"<!-- {ATTRIBUTION_HEADER} -->"):
                errors.append("canvas-rules-mismatch: canvas-rules.md missing attribution header")
            if actual_canvas_md != expected_canvas_md:
                errors.append(
                    "canvas-rules-mismatch: canvas-rules.md does not match generated output from rule-map.toml"
                )

    return (1 if errors else 0), errors


def main(argv: Optional[List[str]] = None) -> int:
    parser = argparse.ArgumentParser(description="Check vendored MIT UI skills (E15-46)")
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=Path(__file__).resolve().parent.parent,
        help="Repository root directory",
    )
    sub = parser.add_subparsers(dest="subcommand", required=True)

    p_verify = sub.add_parser("verify", help="Verify upstream pin, licence, and file SHA-1 hashes")
    p_verify.add_argument("--online", action="store_true", help="Refetch LICENSE from GitHub and compare")

    p_extract = sub.add_parser("extract", help="Extract rules from upstream markdown and check/write rules.lock")
    p_extract.add_argument("--write", action="store_true", help="Write extracted rules to rules.lock")

    p_map = sub.add_parser("map-coverage", help="Verify rule-map.toml coverage, targets, and canvas-rules.md")
    p_map.add_argument("--write-canvas", action="store_true", help="Write generated canvas-rules.md")

    args = parser.parse_args(argv)
    repo_root: Path = args.repo_root.resolve()

    if args.subcommand == "verify":
        code, msgs = cmd_verify(repo_root, online=args.online)
    elif args.subcommand == "extract":
        code, msgs = cmd_extract(repo_root, write=args.write)
    elif args.subcommand == "map-coverage":
        code, msgs = cmd_map_coverage(repo_root, write_canvas=args.write_canvas)
    else:
        return 2

    if code == 0:
        for m in msgs:
            print(m)
        if not msgs:
            print(f"{args.subcommand}: OK")
    else:
        for m in msgs:
            print(m, file=sys.stderr)
    return code


if __name__ == "__main__":
    sys.exit(main())
