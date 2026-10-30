#!/usr/bin/env python3
"""Regression tests for the shared Cargo target resolver."""

from __future__ import annotations

import os
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).parent))

from cargo_target import is_safe_cache_target, resolve


class CargoTargetTests(unittest.TestCase):
    def test_relative_override_matches_cargo_repo_working_directory(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp) / "checkout"
            caller_dir = Path(tmp) / "caller"
            repo_root.mkdir()
            caller_dir.mkdir()
            previous_cwd = Path.cwd()
            try:
                os.chdir(caller_dir)
                with patch.dict(os.environ, {"CARGO_TARGET_DIR": "../shared-target"}):
                    self.assertEqual(resolve(repo_root), (repo_root.parent / "shared-target").resolve())
            finally:
                os.chdir(previous_cwd)

    def test_absolute_override_is_preserved(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp) / "checkout"
            repo_root.mkdir()
            configured = (Path(tmp) / "target-cache").resolve()
            with patch.dict(os.environ, {"CARGO_TARGET_DIR": str(configured)}):
                self.assertEqual(resolve(repo_root), configured)

    def test_only_known_external_cache_paths_are_cleanable(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp) / "checkout"
            repo_root.mkdir()
            self.assertTrue(is_safe_cache_target(repo_root.resolve().parent / ".design-os-studio-target", repo_root))
            self.assertTrue(is_safe_cache_target(Path("../.design-os-studio-target"), repo_root))
            self.assertFalse(is_safe_cache_target(repo_root / "target", repo_root))
            self.assertFalse(is_safe_cache_target(Path("/"), repo_root))
            self.assertFalse(is_safe_cache_target(Path.home(), repo_root))
            self.assertFalse(is_safe_cache_target(Path(tmp) / "unrelated", repo_root))

    def test_cache_symlink_is_not_cleanable(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp) / "checkout"
            repo_root.mkdir()
            unrelated = Path(tmp) / "unrelated"
            unrelated.mkdir()
            cache_link = repo_root.parent / ".design-os-studio-target"
            try:
                cache_link.symlink_to(unrelated, target_is_directory=True)
            except OSError as exc:
                self.skipTest(f"symlinks unavailable: {exc}")
            self.assertFalse(is_safe_cache_target(cache_link, repo_root))


if __name__ == "__main__":
    unittest.main()
