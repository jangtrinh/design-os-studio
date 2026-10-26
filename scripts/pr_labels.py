#!/usr/bin/env python3
"""Resolve a PR from GitHub API payloads and compute size/sensitive-path labels."""

from __future__ import annotations
import argparse, json, re, sys
from pathlib import Path

MAX_LINES_WITHOUT_SIZE_L = 400
MAX_FILES_WITHOUT_SIZE_L = 10
MAX_ARTIFACT_BYTES = 65536
MAX_MATCHING_PRS = 20

SENSITIVE_PATH_PREFIXES: tuple[str, ...] = (
    ".github/", "packaging/", "scripts/build-release.sh", "scripts/package.sh",
    "scripts/package-macos.sh", "scripts/release-smoke-macos.sh", "scripts/release-sync-pages.py",
    "crates/designos-mcp/", "crates/designos-eval/src/sandbox.rs",
    "crates/designos-app/src/agent_permissions.rs", "crates/designos-app/src/agent_session",
    "crates/designos-app/src/skills/hook_delivery.rs", "crates/designos-app/src/terminal",
    "crates/designos-app/src/shell/tests/terminal_undo_chord.rs", "skills/vendor/",
)

_SHA_RE = re.compile(r"^[0-9a-fA-F]{40}$")
_REPO_RE = re.compile(r"^[^/\s]+/[^/\s]+$")
_BRACE_RENAME_RE = re.compile(r"^(.*?)\{(.*?) (?:=>|->) (.*?)\}(.*)$")


def _is_int(v: object) -> bool:
    return isinstance(v, int) and not isinstance(v, bool)


def _validate_sha(sha: object, field: str = "head_sha") -> str:
    if not isinstance(sha, str) or _SHA_RE.match(sha) is None:
        raise ValueError(f"{field} must be a 40-character hex SHA")
    return sha


def _validate_repo(repo: object, field: str = "head_repo") -> str:
    if not isinstance(repo, str) or _REPO_RE.match(repo) is None:
        raise ValueError(f"{field} must be in owner/repo format")
    return repo


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


def resolve_prs_from_commits_pulls(
    pulls: object, expected_head_sha: str, expected_head_repo: str,
) -> list[int]:
    sha = _validate_sha(expected_head_sha)
    repo = _validate_repo(expected_head_repo)
    if not isinstance(pulls, list):
        raise ValueError("commits/pulls payload must be a JSON array")
    matched: list[int] = []
    for entry in pulls:
        if not isinstance(entry, dict):
            raise ValueError("commits/pulls entry must be a JSON object")
        head = entry.get("head")
        if not isinstance(head, dict):
            continue
        head_repo = head.get("repo")
        full_name = head_repo.get("full_name") if isinstance(head_repo, dict) else None
        if head.get("sha") == sha and full_name == repo:
            num = entry.get("number")
            if not _is_int(num) or int(num) <= 0:
                raise ValueError("matching PR has invalid number")
            pr_num = int(num)
            if pr_num not in matched:
                matched.append(pr_num)
    return matched


def resolve_pr_from_commits_pulls(
    pulls: object, expected_head_sha: str, expected_head_repo: str,
) -> int | None:
    matched = resolve_prs_from_commits_pulls(pulls, expected_head_sha, expected_head_repo)
    return matched[0] if matched else None


def candidate_pr_from_artifact(artifact: object) -> int:
    if not isinstance(artifact, dict):
        raise ValueError("artifact payload must be a JSON object")
    pr_number = artifact.get("pr_number")
    if not _is_int(pr_number) or int(pr_number) <= 0:
        raise ValueError("artifact pr_number must be a positive integer")
    return int(pr_number)


def resolve_pr_numbers(
    *,
    head_pulls: object | None = None,
    pulls: object | None = None,
    head_sha: str,
    head_repo: str,
    artifact: object | None = None,
) -> list[int]:
    if head_pulls is not None:
        resolved = resolve_prs_from_commits_pulls(head_pulls, head_sha, head_repo)
        if resolved:
            return resolved
    if pulls is not None:
        resolved = resolve_prs_from_commits_pulls(pulls, head_sha, head_repo)
        if resolved:
            return resolved
    if head_pulls is None and pulls is None:
        _validate_sha(head_sha)
        _validate_repo(head_repo)
    if artifact is not None:
        return [candidate_pr_from_artifact(artifact)]
    return []


def resolve_pr_number(
    *,
    head_pulls: object | None = None,
    pulls: object | None = None,
    head_sha: str,
    head_repo: str,
    artifact: object | None = None,
) -> int | None:
    resolved = resolve_pr_numbers(
        head_pulls=head_pulls,
        pulls=pulls,
        head_sha=head_sha,
        head_repo=head_repo,
        artifact=artifact,
    )
    return resolved[0] if resolved else None


def validate_pull(
    pull: object, *, expected_pr_number: int, expected_head_sha: str, expected_head_repo: str,
) -> dict[str, int]:
    if not isinstance(pull, dict):
        raise ValueError("pull payload must be a JSON object")
    num = pull.get("number")
    if not _is_int(num) or int(num) != expected_pr_number:
        raise ValueError(f"pull number {num!r} does not match expected {expected_pr_number}")
    head = pull.get("head")
    if not isinstance(head, dict):
        raise ValueError("pull payload missing head object")
    actual_sha = head.get("sha")
    if not isinstance(actual_sha, str) or actual_sha != expected_head_sha:
        raise ValueError(f"pull head.sha {actual_sha!r} != expected {expected_head_sha!r}")
    head_repo = head.get("repo")
    actual_repo = head_repo.get("full_name") if isinstance(head_repo, dict) else None
    if not isinstance(actual_repo, str) or actual_repo != expected_head_repo:
        raise ValueError(f"pull head.repo.full_name {actual_repo!r} != expected {expected_head_repo!r}")
    ra, rd, rf = pull.get("additions"), pull.get("deletions"), pull.get("changed_files")
    if not (_is_int(ra) and int(ra) >= 0 and _is_int(rd) and int(rd) >= 0 and _is_int(rf) and int(rf) >= 0):
        raise ValueError("pull additions, deletions, and changed_files must be non-negative integers")
    return {"pr_number": int(num), "additions": int(ra), "deletions": int(rd), "changed_files": int(rf)}


def extract_file_paths(files: object) -> list[str]:
    if not isinstance(files, list):
        raise ValueError("files payload must be a JSON array")
    paths: list[str] = []
    for entry in files:
        if isinstance(entry, dict):
            fn = entry.get("filename")
            if not isinstance(fn, str) or not fn.strip() or any(c in fn for c in "\x00\r\n"):
                raise ValueError("invalid filename in files payload")
            if entry.get("status") == "renamed":
                prev = entry.get("previous_filename")
                if not isinstance(prev, str) or not prev.strip() or any(c in prev for c in "\x00\r\n"):
                    raise ValueError("renamed file missing valid previous_filename")
                raw = f"{prev} => {fn}"
            else:
                raw = fn
        elif isinstance(entry, str):
            if not entry.strip() or any(c in entry for c in "\x00\r\n"):
                raise ValueError("invalid path entry")
            raw = entry
        else:
            raise ValueError("file entry must be a JSON object or string")
        if not expand_rename_paths(raw):
            raise ValueError("empty normalized path")
        paths.append(raw)
    return paths


def compute_labels(
    *, additions: int, deletions: int, changed_files: int, paths: list[str],
) -> list[str]:
    if len(paths) > changed_files:
        raise ValueError("files list length exceeds changed_files")
    if len(paths) < changed_files:
        return ["size/L", "sensitive-path"]
    labels: list[str] = []
    if (additions + deletions) > MAX_LINES_WITHOUT_SIZE_L or changed_files > MAX_FILES_WITHOUT_SIZE_L:
        labels.append("size/L")
    if any(is_sensitive_path(p) for p in paths):
        labels.append("sensitive-path")
    return labels


def evaluate_pr_labels(
    *,
    head_pulls: object | None = None,
    pulls: object | None = None,
    pull: object | None = None,
    files: object | None = None,
    head_sha: str,
    head_repo: str,
    artifact: object | None = None,
    expected_pr_number: int | None = None,
) -> tuple[int | None, list[str]]:
    expected_sha = _validate_sha(head_sha)
    expected_repo = _validate_repo(head_repo)
    pr_numbers = resolve_pr_numbers(
        head_pulls=head_pulls,
        pulls=pulls,
        head_sha=expected_sha,
        head_repo=expected_repo,
        artifact=artifact,
    )
    if expected_pr_number is not None:
        if not _is_int(expected_pr_number) or int(expected_pr_number) <= 0:
            raise ValueError("expected_pr_number must be a positive integer")
        target_pr = int(expected_pr_number)
        if target_pr not in pr_numbers[:MAX_MATCHING_PRS]:
            raise ValueError(f"PR #{target_pr} is not in resolved matching PRs")
        pr_number = target_pr
    elif not pr_numbers:
        return None, []
    else:
        cand = pull.get("number") if isinstance(pull, dict) else None
        if _is_int(cand) and int(cand) in pr_numbers[:MAX_MATCHING_PRS]:
            pr_number = int(cand)
        else:
            pr_number = pr_numbers[0]
    if pull is None or files is None:
        raise ValueError("pull and files payloads are required when a PR is resolved")
    v = validate_pull(
        pull, expected_pr_number=pr_number, expected_head_sha=expected_sha, expected_head_repo=expected_repo,
    )
    paths = extract_file_paths(files)
    labels = compute_labels(
        additions=v["additions"], deletions=v["deletions"], changed_files=v["changed_files"], paths=paths,
    )
    return pr_number, labels


def evaluate_all_pr_labels(
    *,
    head_pulls: object | None = None,
    pulls: object | None = None,
    pulls_by_number: dict[int, object] | None = None,
    files_by_number: dict[int, object] | None = None,
    head_sha: str,
    head_repo: str,
    artifact: object | None = None,
    max_prs: int = MAX_MATCHING_PRS,
) -> tuple[list[tuple[int, list[str]]], list[str]]:
    expected_sha = _validate_sha(head_sha)
    expected_repo = _validate_repo(head_repo)
    pr_numbers = resolve_pr_numbers(
        head_pulls=head_pulls,
        pulls=pulls,
        head_sha=expected_sha,
        head_repo=expected_repo,
        artifact=artifact,
    )
    if not pr_numbers:
        return [], []
    total_matches = len(pr_numbers)
    capped_numbers = pr_numbers[:max_prs]
    results: list[tuple[int, list[str]]] = []
    errors: list[str] = []
    p_map = pulls_by_number or {}
    f_map = files_by_number or {}
    for pr_num in capped_numbers:
        try:
            if pr_num not in p_map or pr_num not in f_map:
                raise ValueError(f"missing pull or files payload for PR #{pr_num}")
            p_val, f_val = p_map[pr_num], f_map[pr_num]
            if isinstance(p_val, Exception):
                raise ValueError(str(p_val))
            if isinstance(f_val, Exception):
                raise ValueError(str(f_val))
            v = validate_pull(
                p_val,
                expected_pr_number=pr_num,
                expected_head_sha=expected_sha,
                expected_head_repo=expected_repo,
            )
            paths = extract_file_paths(f_val)
            labels = compute_labels(
                additions=v["additions"],
                deletions=v["deletions"],
                changed_files=v["changed_files"],
                paths=paths,
            )
            results.append((pr_num, labels))
        except ValueError as exc:
            errors.append(f"PR #{pr_num}: {exc}")
    if total_matches > max_prs:
        errors.append(
            f"matched {total_matches} PRs, exceeding cap of {max_prs} (labelled first {max_prs})"
        )
    return results, errors


def parse_paginated_arrays(raw: str, *, source: str = "<memory>") -> list[object]:
    try:
        dec = json.JSONDecoder()
        idx, n, merged, saw_array = 0, len(raw), [], False
        while idx < n:
            while idx < n and raw[idx].isspace():
                idx += 1
            if idx >= n:
                break
            val, idx = dec.raw_decode(raw, idx)
            if not isinstance(val, list):
                raise ValueError(f"expected JSON array in {source}")
            saw_array = True
            merged.extend(val)
        if not saw_array:
            raise ValueError(f"expected JSON array in {source}")
        return merged
    except json.JSONDecodeError as exc:
        raise ValueError(f"invalid JSON in {source}: {exc}") from exc


def _load_json_file(
    path: str | Path, *, allow_paginated_arrays: bool = False, max_bytes: int | None = None,
) -> object:
    p = Path(path)
    if not p.exists():
        raise ValueError(f"missing JSON file: {p}")
    size = p.stat().st_size
    if max_bytes is not None and size > max_bytes:
        raise ValueError(f"JSON file {p} ({size} bytes) exceeds limit of {max_bytes} bytes")
    try:
        raw = p.read_text(encoding="utf-8")
        if not allow_paginated_arrays:
            return json.loads(raw)
        return parse_paginated_arrays(raw, source=str(p))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ValueError(f"invalid JSON in {p}: {exc}") from exc


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Resolve PR and compute labels from GitHub API JSON.")
    parser.add_argument("--head-pulls", help="Path to pulls?state=open&head=<owner>:<branch> JSON")
    parser.add_argument("--pulls", help="Path to commits/<sha>/pulls JSON")
    parser.add_argument("--pr-number", type=int, help="Specific resolved PR number to validate")
    parser.add_argument("--pull", help="Path to pulls/<n> JSON")
    parser.add_argument("--files", help="Path to pulls/<n>/files JSON")
    parser.add_argument("--pulls-dir", help="Directory with pull-<n>.json and files-<n>.json for all matched PRs")
    parser.add_argument("--head-sha", required=True, help="Expected workflow_run.head_sha")
    parser.add_argument("--head-repo", required=True, help="Expected workflow_run.head_repository.full_name")
    parser.add_argument("--artifact", help="Optional path to candidate pr-gate-result.json")
    parser.add_argument("--resolve-pr", action="store_true", help="Print resolved/candidate PR number(s), one per line")
    args = parser.parse_args(argv)
    if not args.head_pulls and not args.pulls:
        parser.error("at least one of --head-pulls or --pulls is required")
    try:
        head_pulls_data = (
            _load_json_file(args.head_pulls, allow_paginated_arrays=True) if args.head_pulls else None
        )
        pulls_data = (
            _load_json_file(args.pulls, allow_paginated_arrays=True) if args.pulls else None
        )
        artifact_data = (
            _load_json_file(args.artifact, max_bytes=MAX_ARTIFACT_BYTES) if args.artifact else None
        )
        if args.resolve_pr:
            pr_nums = resolve_pr_numbers(
                head_pulls=head_pulls_data,
                pulls=pulls_data,
                head_sha=args.head_sha,
                head_repo=args.head_repo,
                artifact=artifact_data,
            )
            for pr_num in pr_nums:
                print(pr_num)
            return 0
        if args.pulls_dir:
            pr_nums = resolve_pr_numbers(
                head_pulls=head_pulls_data,
                pulls=pulls_data,
                head_sha=args.head_sha,
                head_repo=args.head_repo,
                artifact=artifact_data,
            )
            pulls_dir = Path(args.pulls_dir)
            pulls_by_num: dict[int, object] = {}
            files_by_num: dict[int, object] = {}
            for pr_num in pr_nums[:MAX_MATCHING_PRS]:
                p_file = pulls_dir / f"pull-{pr_num}.json"
                f_file = pulls_dir / f"files-{pr_num}.json"
                try:
                    pulls_by_num[pr_num] = _load_json_file(p_file)
                except ValueError as exc:
                    pulls_by_num[pr_num] = exc
                try:
                    files_by_num[pr_num] = _load_json_file(f_file, allow_paginated_arrays=True)
                except ValueError as exc:
                    files_by_num[pr_num] = exc
            results, errors = evaluate_all_pr_labels(
                head_pulls=head_pulls_data,
                pulls=pulls_data,
                pulls_by_number=pulls_by_num,
                files_by_number=files_by_num,
                head_sha=args.head_sha,
                head_repo=args.head_repo,
                artifact=artifact_data,
            )
            print(
                json.dumps(
                    {
                        "results": [{"pr_number": n, "labels": l} for n, l in results],
                        "errors": errors,
                    }
                )
            )
            for err in errors:
                print(f"error: {err}", file=sys.stderr)
            return 1 if errors else 0
        pull_data = _load_json_file(args.pull) if args.pull else None
        files_data = _load_json_file(args.files, allow_paginated_arrays=True) if args.files else None
        pr_num, labels = evaluate_pr_labels(
            head_pulls=head_pulls_data,
            pulls=pulls_data,
            pull=pull_data,
            files=files_data,
            head_sha=args.head_sha,
            head_repo=args.head_repo,
            artifact=artifact_data,
            expected_pr_number=args.pr_number,
        )
        print(json.dumps({"pr_number": pr_num, "labels": labels}))
    except ValueError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
