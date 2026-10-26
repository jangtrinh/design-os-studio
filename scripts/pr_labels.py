#!/usr/bin/env python3
"""Evaluate PR size and sensitive-path labels from a pr-gate-result JSON artifact."""

from __future__ import annotations
import json, re, sys
from pathlib import Path

MAX_LINES_WITHOUT_SIZE_L = 400
MAX_FILES_WITHOUT_SIZE_L = 10

SENSITIVE_PATH_PREFIXES: tuple[str, ...] = (
    ".github/", "packaging/", "scripts/build-release.sh", "scripts/package.sh",
    "scripts/package-macos.sh", "scripts/release-smoke-macos.sh", "scripts/release-sync-pages.py",
    "crates/designos-mcp/", "crates/designos-eval/src/sandbox.rs",
    "crates/designos-app/src/agent_permissions.rs", "crates/designos-app/src/agent_session",
    "crates/designos-app/src/skills/hook_delivery.rs", "crates/designos-app/src/terminal",
    "crates/designos-app/src/shell/tests/terminal_undo_chord.rs", "skills/vendor/",
)

_SHA_RE = re.compile(r"^[0-9a-fA-F]{40}$")
_BRACE_RENAME_RE = re.compile(r"^(.*?)\{(.*?) (?:=>|->) (.*?)\}(.*)$")


def _is_int(v: object) -> bool:
    return isinstance(v, int) and not isinstance(v, bool)


def _norm_path(path: str) -> str:
    cleaned = re.sub(r"/+", "/", path.strip())
    while cleaned.startswith("./"):
        cleaned = cleaned[2:]
    return cleaned.lstrip("/")


def expand_rename_paths(raw_path: str) -> list[str]:
    m = _BRACE_RENAME_RE.match(raw_path)
    if m is not None:
        pre, old_m, new_m, suf = m.groups()
        parts = [f"{pre}{old_m}{suf}", f"{pre}{new_m}{suf}"]
    elif " => " in raw_path:
        parts = list(raw_path.split(" => ", 1))
    elif " -> " in raw_path:
        parts = list(raw_path.split(" -> ", 1))
    else:
        parts = [raw_path]
    return [p for p in (_norm_path(c) for c in parts) if p]


def is_sensitive_path(raw_path: str) -> bool:
    return any(p.startswith(pfx) for p in expand_rename_paths(raw_path) for pfx in SENSITIVE_PATH_PREFIXES)


def validate_artifact(data: object) -> dict[str, object]:
    if not isinstance(data, dict):
        raise ValueError("artifact payload must be a JSON object")
    pr_number, head_sha = data.get("pr_number"), data.get("head_sha")
    if not _is_int(pr_number) or int(pr_number) <= 0:
        raise ValueError("pr_number must be a positive integer")
    if not isinstance(head_sha, str) or _SHA_RE.match(head_sha) is None:
        raise ValueError("head_sha must be a 40-character hex SHA")
    has_add_del, has_lines = ("additions" in data or "deletions" in data), ("lines_changed" in data)
    if not has_add_del and not has_lines:
        raise ValueError("missing line counts")
    additions = deletions = 0
    if has_add_del:
        ra, rd = data.get("additions"), data.get("deletions")
        if not (_is_int(ra) and int(ra) >= 0 and _is_int(rd) and int(rd) >= 0):
            raise ValueError("additions and deletions must be non-negative integers")
        additions, deletions = int(ra), int(rd)
    if has_lines:
        rl = data.get("lines_changed")
        if not _is_int(rl) or int(rl) < 0 or (has_add_del and int(rl) != additions + deletions):
            raise ValueError("invalid lines_changed")
        lines_changed = int(rl)
    else:
        lines_changed = additions + deletions
    rf, raw_paths = data.get("changed_files"), data.get("paths")
    if not _is_int(rf) or int(rf) < 0 or not isinstance(raw_paths, list) or int(rf) != len(raw_paths):
        raise ValueError("invalid changed_files or paths")
    for entry in raw_paths:
        if not isinstance(entry, str) or not entry.strip() or any(c in entry for c in "\x00\r\n") or not expand_rename_paths(entry):
            raise ValueError("invalid path entry")
    return {"pr_number": int(pr_number), "head_sha": head_sha, "additions": additions, "deletions": deletions, "lines_changed": lines_changed, "changed_files": int(rf), "paths": list(raw_paths)}


def labels_for_artifact(data: object) -> list[str]:
    v = validate_artifact(data)
    labels: list[str] = []
    if int(v["lines_changed"]) > MAX_LINES_WITHOUT_SIZE_L or int(v["changed_files"]) > MAX_FILES_WITHOUT_SIZE_L:
        labels.append("size/L")
    if any(is_sensitive_path(str(p)) for p in v["paths"]):  # type: ignore[union-attr]
        labels.append("sensitive-path")
    return labels


def labels_from_artifact_path(path: str | Path) -> list[str]:
    p = Path(path)
    if not p.exists():
        return []
    try:
        return labels_for_artifact(json.loads(p.read_text(encoding="utf-8")))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ValueError(f"invalid artifact JSON: {exc}") from exc


def main(argv: list[str] | None = None) -> int:
    args = sys.argv[1:] if argv is None else argv
    if len(args) != 1:
        print("usage: pr_labels.py <pr-gate-result.json>", file=sys.stderr)
        return 2
    try:
        for label in labels_from_artifact_path(args[0]):
            print(label)
    except ValueError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
