#!/usr/bin/env python3
"""Resolve the Cargo target directory for this checkout."""

from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path


def resolve(repo_root: Path) -> Path:
    repo_root = repo_root.expanduser().resolve()
    configured = os.environ.get("CARGO_TARGET_DIR")
    if configured:
        target = Path(configured).expanduser()
        # Cargo interprets a relative CARGO_TARGET_DIR from the command's
        # working directory. All callers run Cargo from repo_root, even when
        # the resolver itself was imported by a script started elsewhere.
        return (repo_root / target if not target.is_absolute() else target).resolve()

    try:
        result = subprocess.run(
            ["cargo", "metadata", "--no-deps", "--format-version", "1"],
            cwd=repo_root,
            check=True,
            capture_output=True,
            text=True,
        )
        return Path(json.loads(result.stdout)["target_directory"]).resolve()
    except (FileNotFoundError, OSError, subprocess.CalledProcessError, KeyError, json.JSONDecodeError):
        return (repo_root.parent / ".design-os-studio-target").resolve()


def is_safe_cache_target(target_dir: Path, repo_root: Path) -> bool:
    """Allow cleanup only for the known local/shared DESIGN:OS target caches."""
    root = repo_root.expanduser().resolve()
    target_path = Path(target_dir).expanduser()
    target = (root / target_path if not target_path.is_absolute() else target_path)
    target = Path(os.path.abspath(target))
    home = Path(os.path.abspath(Path.home()))
    # Do not canonicalize a cache symlink: rm -rf on its resolved destination
    # would delete unrelated files. Reject symlinks in every existing component.
    current = Path(target.anchor)
    for part in target.parts[1:]:
        current /= part
        if current.is_symlink():
            return False
    try:
        target.relative_to(root)
        inside_checkout = True
    except ValueError:
        inside_checkout = False
    if target in {Path("/"), home, root} or inside_checkout:
        return False
    allowed = {
        Path(os.path.abspath(root.parent / ".design-os-studio-target")),
        Path(os.path.abspath(home / "Library/Caches/design-os-studio/target")),
    }
    return target in allowed


if __name__ == "__main__":
    if len(sys.argv) == 4 and sys.argv[1] == "--check-clean":
        root = Path(sys.argv[2]).resolve()
        target = Path(sys.argv[3])
        target = target.expanduser()
        candidate = Path(os.path.abspath(target if target.is_absolute() else root / target))
        if not is_safe_cache_target(candidate, root):
            print(f"cargo_target.py: refusing unsafe cache target: {candidate}", file=sys.stderr)
            raise SystemExit(1)
        print(candidate)
    else:
        root = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else Path.cwd()
        print(resolve(root))
