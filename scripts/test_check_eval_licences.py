#!/usr/bin/env python3
"""Unit tests for scripts/check_eval_licences.py (Issue #611 / Task E15-23)."""

from __future__ import annotations

import pathlib
import shutil
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import check_eval_licences as cel  # noqa: E402

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent


class TestCheckEvalLicences(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.root = pathlib.Path(self.tmp.name)
        (self.root / "docs").mkdir(parents=True, exist_ok=True)
        shutil.copy2(REPO_ROOT / "docs" / "eval-licences.md", self.root / "docs" / "eval-licences.md")
        parsed, _ = cel.extract_register((REPO_ROOT / "docs" / "eval-licences.md").read_text(encoding="utf-8"))
        for src in parsed["source"]:
            text_file = src.get("licence_text_file")
            if text_file:
                (self.root / text_file).parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(REPO_ROOT / text_file, self.root / text_file)
        cel.check_repo(self.root, write_third_party=True)

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def test_repo_register_is_valid(self) -> None:
        code, findings = cel.check_repo(REPO_ROOT)
        self.assertEqual(code, 0, f"Unexpected findings on repo root: {findings}")

    def test_missing_register_exits_two(self) -> None:
        (self.root / "docs" / "eval-licences.md").unlink()
        code, findings = cel.check_repo(self.root)
        self.assertEqual(code, 2)
        self.assertTrue(any(f.startswith("parse:") for f in findings))

    def test_every_eval_source_registered(self) -> None:
        s1 = self.root / "crates" / "designos-eval" / "suites" / "s1"
        s1.mkdir(parents=True, exist_ok=True)
        (s1 / "tasks.toml").write_text(
            '[[task]]\nid = "t1"\nsource = "design2code"\n[[task]]\nid = "t2"\nsource = "uicrit"\n',
            encoding="utf-8",
        )
        code, findings = cel.check_repo(self.root)
        self.assertEqual(code, 0, findings)

        (s1 / "tasks.toml").write_text(
            '[[task]]\nid = "t1"\nsource = "unknown-dataset"\n',
            encoding="utf-8",
        )
        code, findings = cel.check_repo(self.root)
        self.assertEqual(code, 1)
        self.assertTrue(any(f.startswith("task-source-registered:") for f in findings), findings)

    def test_unlicensed_marked_excluded(self) -> None:
        reg = self.root / "docs" / "eval-licences.md"
        text = reg.read_text(encoding="utf-8")
        # Change transitions-dev (which has licence = "NONE") to use = "eval-only-cache"
        bad = text.replace(
            "| `transitions-dev` | `NONE` | `excluded` |",
            "| `transitions-dev` | `NONE` | `eval-only-cache` |",
        ).replace(
            'id = "transitions-dev"\nname = "transitions.dev skills (no licence published)"\nkind = "skills"\nurl = "https://transitions.dev"\npinned = ""\nlicence = "NONE"\nuse = "excluded"',
            'id = "transitions-dev"\nname = "transitions.dev skills (no licence published)"\nkind = "skills"\nurl = "https://transitions.dev"\npinned = ""\nlicence = "NONE"\nuse = "eval-only-cache"',
        )
        reg.write_text(bad, encoding="utf-8")
        code, findings = cel.check_repo(self.root)
        self.assertEqual(code, 1)
        self.assertTrue(any(f.startswith("none-means-excluded:") for f in findings), findings)

    def test_client_fixture_path_fails(self) -> None:
        s3 = self.root / "crates" / "designos-eval" / "suites" / "s3"
        s3.mkdir(parents=True, exist_ok=True)
        (s3 / "tasks.toml").write_text(
            '[[task]]\nid = "s3-1"\nsource = "shadcn-ui"\npath = "fixtures/VSF - PCP.fig"\n',
            encoding="utf-8",
        )
        code, findings = cel.check_repo(self.root)
        self.assertEqual(code, 1)
        self.assertTrue(any(f.startswith("client-file-ban:") for f in findings), findings)

    def test_home_path_fails(self) -> None:
        s3 = self.root / "crates" / "designos-eval" / "suites" / "s3"
        s3.mkdir(parents=True, exist_ok=True)
        (s3 / "tasks.toml").write_text(
            '[[task]]\nid = "s3-1"\nsource = "shadcn-ui"\npath = "/Users/x/Downloads/kit.fig"\n',
            encoding="utf-8",
        )
        code, findings = cel.check_repo(self.root)
        self.assertEqual(code, 1)
        self.assertTrue(any(f.startswith("client-file-ban:") for f in findings), findings)

        (s3 / "tasks.toml").write_text(
            '[[task]]\nid = "s3-1"\nsource = "shadcn-ui"\npath = "C:\\\\Users\\\\x\\\\Downloads\\\\gen-kit.fig"\n',
            encoding="utf-8",
        )
        code_win, findings_win = cel.check_repo(self.root)
        self.assertEqual(code_win, 1)
        self.assertTrue(any(f.startswith("client-file-ban:") for f in findings_win), findings_win)

    def test_word_containing_pcp_passes(self) -> None:
        s1 = self.root / "crates" / "designos-eval" / "suites" / "s1" / "specs"
        s1.mkdir(parents=True, exist_ok=True)
        (s1 / "pcpx.md").write_text("# Clean spec\nPath: specs/pcpx.md\n", encoding="utf-8")
        code, findings = cel.check_repo(self.root)
        self.assertEqual(code, 0, findings)

    def test_hyphenated_pcp_token_fails(self) -> None:
        s1 = self.root / "crates" / "designos-eval" / "suites" / "s1" / "specs"
        s1.mkdir(parents=True, exist_ok=True)
        (s1 / "pcp-free.md").write_text("# Spec\n", encoding="utf-8")
        code, findings = cel.check_repo(self.root)
        self.assertEqual(code, 1)
        self.assertTrue(any(f.startswith("client-file-ban:") for f in findings), findings)

    def _write_task(self, body: str, name: str = "tasks.toml") -> pathlib.Path:
        s3 = self.root / "crates" / "designos-eval" / "suites" / "s3"
        s3.mkdir(parents=True, exist_ok=True)
        (s3 / name).write_text(body, encoding="utf-8")
        return s3

    def test_windows_forward_slash_home_path_fails(self) -> None:
        self._write_task('[[task]]\nid = "s3-1"\nsource = "shadcn-ui"\npath = "C:/Users/x/Downloads/gen-client.fig"\n')
        code, findings = cel.check_repo(self.root)
        self.assertEqual(code, 1)
        self.assertTrue(any(f.startswith("client-file-ban:") for f in findings), findings)

    def test_generated_name_without_sidecar_fails(self) -> None:
        s3 = self._write_task('[[task]]\nid = "s3-1"\nsource = "shadcn-ui"\n')
        (s3 / "gen-client.fig").write_bytes(b"x")
        code, findings = cel.check_repo(self.root)
        self.assertEqual(code, 1)
        self.assertTrue(any(f.startswith("client-file-ban:") and "gen-client.fig" in f for f in findings), findings)

    def test_generated_file_with_sidecar_passes(self) -> None:
        s3 = self._write_task('[[task]]\nid = "s3-1"\nsource = "shadcn-ui"\n')
        (s3 / "gen-shadcn-1000-1.designos").write_bytes(b"x")
        (s3 / "gen-shadcn-1000-1.profile.json").write_text('{"generator": "designos-eval gen 0"}', encoding="utf-8")
        code, findings = cel.check_repo(self.root)
        self.assertEqual(code, 0, findings)

    def test_referenced_generated_name_without_sidecar_fails(self) -> None:
        self._write_task('[[task]]\nid = "s3-1"\nsource = "shadcn-ui"\nstart = "suites/s3/gen-client.fig"\n')
        code, findings = cel.check_repo(self.root)
        self.assertEqual(code, 1)
        self.assertTrue(any(f.startswith("client-file-ban:") for f in findings), findings)

    def test_harness_sources_may_name_their_run_files(self) -> None:
        src = self.root / "crates" / "designos-eval" / "src"
        src.mkdir(parents=True, exist_ok=True)
        (src / "runner.rs").write_text('const OUT: &str = "output.designos";\n', encoding="utf-8")
        (src / "preamble.md").write_text("Save to output.designos when done.\n", encoding="utf-8")
        code, findings = cel.check_repo(self.root)
        self.assertEqual(code, 0, findings)
        (src / "runner.rs").write_text('const KIT: &str = "Untitled UI kit";\n', encoding="utf-8")
        code, findings = cel.check_repo(self.root)
        self.assertEqual(code, 1, "client names are banned in sources too")

    def test_unquoted_client_path_in_toml_and_markdown_fails(self) -> None:
        for name, body in (
            ("tasks.toml", '[[task]]\nid = "s3-1"\nsource = "shadcn-ui"\n# path: fixtures/VSF - PCP.fig\n'),
            ("notes.md", "Open fixtures/VSF - PCP.fig and compare.\n"),
            ("brief.md", "Use the Untitled UI kit file.\n"),
        ):
            with self.subTest(name=name):
                s3 = self._write_task(body, name)
                code, findings = cel.check_repo(self.root)
                self.assertEqual(code, 1, (name, findings))
                self.assertTrue(any(f.startswith("client-file-ban:") for f in findings), findings)
                (s3 / name).unlink()

    def _edit_register(self, old: str, new: str) -> None:
        reg = self.root / "docs" / "eval-licences.md"
        text = reg.read_text(encoding="utf-8")
        self.assertIn(old, text)
        reg.write_text(text.replace(old, new, 1), encoding="utf-8")

    def test_approved_entry_with_non_revision_pin_fails(self) -> None:
        pristine = (REPO_ROOT / "docs" / "eval-licences.md").read_text(encoding="utf-8")
        for bad in ("v4.0", "rico-v1-2017", "", "9c3d5e7f"):
            with self.subTest(pin=bad):
                (self.root / "docs" / "eval-licences.md").write_text(pristine, encoding="utf-8")
                self._edit_register('pinned = "e2f61f6e9d9b5ee201b81d8e0ae4d37aa13f8f25"', f'pinned = "{bad}"')
                code, findings = cel.check_repo(self.root)
                self.assertEqual(code, 1, findings)
                self.assertTrue(any(f.startswith("schema:") and "design2code" in f for f in findings), findings)

    def test_pin_without_source_url_fails(self) -> None:
        self._edit_register(
            'pin_source = "https://huggingface.co/api/datasets/SALT-NLP/Design2Code/revision/e2f61f6e9d9b5ee201b81d8e0ae4d37aa13f8f25"',
            'pin_source = ""',
        )
        code, findings = cel.check_repo(self.root)
        self.assertEqual(code, 1)
        self.assertTrue(any(f.startswith("schema:") and "pin_source" in f for f in findings), findings)

    def test_non_spdx_licence_fails(self) -> None:
        self._edit_register('licence = "LicenseRef-RICO-Terms"', 'licence = "Custom-Academic-Eval"')
        code, findings = cel.check_repo(self.root)
        self.assertEqual(code, 1)
        self.assertTrue(any(f.startswith("schema:") and "licence" in f for f in findings), findings)

    def test_unreviewed_sources_stay_pending(self) -> None:
        parsed, _ = cel.extract_register((REPO_ROOT / "docs" / "eval-licences.md").read_text(encoding="utf-8"))
        review = {s["id"]: s["review"] for s in parsed["source"]}
        self.assertEqual(review["clip-vit-b32"], "approved")
        for sid in ("websight", "design2code-code", "font-inter"):
            self.assertEqual(review[sid], "pending", sid)
        for s in parsed["source"]:
            if s["review"] == "pending":
                self.assertEqual(s["reviewer"], "", s["id"])

    def test_third_party_file_embeds_full_licence_text(self) -> None:
        tp = (self.root / "THIRD_PARTY_LICENSES.md").read_text(encoding="utf-8")
        self.assertIn("Permission is hereby granted, free of charge", tp)
        self.assertIn("SIL OPEN FONT LICENSE Version 1.1", tp)

    def test_ship_entry_without_licence_text_file_fails(self) -> None:
        self._edit_register('licence_text_file = "docs/legal/licences/jakubkrehel-skills.txt"', 'licence_text_file = ""')
        code, findings = cel.check_repo(self.root)
        self.assertEqual(code, 1)
        self.assertTrue(any("licence_text_file" in f for f in findings), findings)

    def test_ship_entry_with_missing_licence_text_file_fails(self) -> None:
        (self.root / "docs" / "legal" / "licences" / "jakubkrehel-skills.txt").unlink()
        code, findings = cel.check_repo(self.root)
        self.assertEqual(code, 1)
        self.assertTrue(any("licence_text_file" in f for f in findings), findings)

    def test_third_party_file_stale(self) -> None:
        tp = self.root / "THIRD_PARTY_LICENSES.md"
        tp.write_text(tp.read_text(encoding="utf-8") + "\n<!-- stale edit -->\n", encoding="utf-8")
        code, findings = cel.check_repo(self.root)
        self.assertEqual(code, 1)
        self.assertTrue(any(f.startswith("third-party-current:") for f in findings), findings)

    def test_approved_without_review_date_fails(self) -> None:
        reg = self.root / "docs" / "eval-licences.md"
        text = reg.read_text(encoding="utf-8").replace('reviewed_on = "2026-10-19"', 'reviewed_on = ""', 1)
        reg.write_text(text, encoding="utf-8")
        code, findings = cel.check_repo(self.root)
        self.assertEqual(code, 1)
        self.assertTrue(any(f.startswith("schema:") for f in findings), findings)

    def test_table_mismatch_fails(self) -> None:
        reg = self.root / "docs" / "eval-licences.md"
        text = reg.read_text(encoding="utf-8").replace("| `ODC-By-1.0` |", "| `MIT` |", 1)
        reg.write_text(text, encoding="utf-8")
        code, findings = cel.check_repo(self.root)
        self.assertEqual(code, 1)
        self.assertTrue(any(f.startswith("table-matches-toml:") for f in findings), findings)


if __name__ == "__main__":
    unittest.main()
