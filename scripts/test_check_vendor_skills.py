#!/usr/bin/env python3
"""Unit and contract tests for `scripts/check_vendor_skills.py` (E15-46 §10 / #636)."""

from __future__ import annotations

import hashlib
import json
import os
import shutil
import stat
import sys
import tempfile
import tomllib
import unittest
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import check_vendor_skills as cvs  # noqa: E402


class TestCheckVendorSkills(unittest.TestCase):
    def _copy_vendor_fixture(self) -> Path:
        tmp = Path(tempfile.mkdtemp(prefix="dos-636-test-"))
        self.addCleanup(lambda: shutil.rmtree(tmp, ignore_errors=True))
        shutil.copytree(
            REPO_ROOT / "skills" / "vendor",
            tmp / "skills" / "vendor",
        )
        # The licence gate reads the E15-23 register and the generated third-party notice.
        (tmp / "docs").mkdir()
        shutil.copy2(REPO_ROOT / "docs" / "eval-licences.md", tmp / "docs" / "eval-licences.md")
        shutil.copy2(REPO_ROOT / "THIRD_PARTY_LICENSES.md", tmp / "THIRD_PARTY_LICENSES.md")
        return tmp

    def _edit(self, path: Path, old: str, new: str) -> None:
        raw = path.read_text(encoding="utf-8")
        self.assertIn(old, raw)
        path.write_text(raw.replace(old, new, 1), encoding="utf-8")

    def test_licence_and_pin_match_upstream(self) -> None:
        """AC1: `verify` exits 0; LICENSE SHA-1 equals recorded upstream hash; commit equals 4c66b1f…"""
        code, errors = cvs.cmd_verify(REPO_ROOT, online=False)
        self.assertEqual(code, 0, f"Expected verify exit 0, got errors: {errors}")

        jakub_dir = REPO_ROOT / "skills" / "vendor" / "jakubkrehel-skills"
        upstream_meta = tomllib.loads((jakub_dir / "UPSTREAM.toml").read_text(encoding="utf-8"))
        self.assertTrue(upstream_meta["commit"].startswith("4c66b1f"))
        self.assertEqual(upstream_meta["commit"], cvs.PINNED_COMMIT)

        lic_sha1 = hashlib.sha1((jakub_dir / "LICENSE").read_bytes()).hexdigest()
        upstream_lic_sha1 = hashlib.sha1((jakub_dir / "upstream" / "LICENSE").read_bytes()).hexdigest()
        self.assertEqual(lic_sha1, upstream_meta["license_sha1"])
        self.assertEqual(upstream_lic_sha1, upstream_meta["license_sha1"])

    def test_every_rule_mapped(self) -> None:
        """AC2: `map-coverage` exits 0; deleting one entry -> `unmapped-rule`."""
        code, errors = cvs.cmd_map_coverage(REPO_ROOT)
        self.assertEqual(code, 0, f"Expected map-coverage exit 0, got errors: {errors}")

        tmp = self._copy_vendor_fixture()
        map_path = tmp / "skills" / "vendor" / "jakubkrehel-skills" / "rule-map.toml"
        raw = map_path.read_text(encoding="utf-8")
        # Remove the last [[rule]] block
        last_idx = raw.rfind("[[rule]]")
        self.assertGreater(last_idx, 0)
        map_path.write_text(raw[:last_idx], encoding="utf-8")

        code_del, errors_del = cvs.cmd_map_coverage(tmp)
        self.assertEqual(code_del, 1)
        self.assertTrue(
            any("unmapped-rule" in e for e in errors_del),
            f"Expected 'unmapped-rule' in {errors_del}",
        )

    def test_no_unlicensed_skill_bundled(self) -> None:
        """AC3: adding `skills/vendor/transitions-dev/x.md` -> `verify` exits 1 `unlicensed-file`."""
        tmp = self._copy_vendor_fixture()
        bad_dir = tmp / "skills" / "vendor" / "transitions-dev"
        bad_dir.mkdir(parents=True, exist_ok=True)
        (bad_dir / "x.md").write_text("# Unlicensed transition skill\n", encoding="utf-8")

        code, errors = cvs.cmd_verify(tmp, online=False)
        self.assertEqual(code, 1)
        self.assertTrue(
            any("unlicensed-file" in e and "transitions-dev/x.md" in e for e in errors),
            f"Expected 'unlicensed-file' for transitions-dev/x.md in {errors}",
        )

    def test_unknown_lint_id_fails(self) -> None:
        """Mapping a rule to `lint:not-a-rule` -> exit 1 `unknown-lint-id`."""
        tmp = self._copy_vendor_fixture()
        map_path = tmp / "skills" / "vendor" / "jakubkrehel-skills" / "rule-map.toml"
        raw = map_path.read_text(encoding="utf-8")
        mutated = raw.replace('map = "lint:contrast"', 'map = "lint:not-a-rule"', 1)
        self.assertNotEqual(raw, mutated)
        map_path.write_text(mutated, encoding="utf-8")

        code, errors = cvs.cmd_map_coverage(tmp)
        self.assertEqual(code, 1)
        self.assertTrue(
            any("unknown-lint-id" in e and "not-a-rule" in e for e in errors),
            f"Expected 'unknown-lint-id' in {errors}",
        )

    def test_canvas_rules_generated(self) -> None:
        """`canvas-rules.md` equals the generated text and begins with the attribution header."""
        jakub_dir = REPO_ROOT / "skills" / "vendor" / "jakubkrehel-skills"
        map_doc = tomllib.loads((jakub_dir / "rule-map.toml").read_text(encoding="utf-8"))
        expected = cvs.render_canvas_rules(map_doc["rule"])
        actual = (jakub_dir / "canvas-rules.md").read_text(encoding="utf-8")
        self.assertTrue(actual.startswith(f"<!-- {cvs.ATTRIBUTION_HEADER} -->"))
        self.assertEqual(actual, expected)

        code, msgs = cvs.cmd_extract(REPO_ROOT, write=False)
        self.assertEqual(code, 0, f"Expected extract exit 0, got: {msgs}")


    def test_missing_register_leaves_the_vendor_folder_unlicensed(self) -> None:
        tmp = self._copy_vendor_fixture()
        (tmp / "docs" / "eval-licences.md").unlink()
        code, errors = cvs.cmd_verify(tmp, online=False)
        self.assertEqual(code, 1, "no register, no licence on record: verify must not fall back to a built-in approval")
        self.assertTrue(any(e.startswith("unlicensed-file:") and "eval-licences.md" in e for e in errors), errors)

    def test_unparsable_register_leaves_the_vendor_folder_unlicensed(self) -> None:
        tmp = self._copy_vendor_fixture()
        (tmp / "docs" / "eval-licences.md").write_text("# Register\n\nno toml block here\n", encoding="utf-8")
        code, errors = cvs.cmd_verify(tmp, online=False)
        self.assertEqual(code, 1)
        self.assertTrue(any(e.startswith("unlicensed-file:") and "cannot be read" in e for e in errors), errors)

    def test_register_entry_that_is_not_approved_or_is_unlicensed_fails(self) -> None:
        for old, new in (
            ('review = "approved"\nreviewer = "owner"\nreviewed_on = "2026-10-18"\nnotes = "Owner ruling 2026-10-18: adopt', 'review = "pending"\nreviewer = "owner"\nreviewed_on = "2026-10-18"\nnotes = "Owner ruling 2026-10-18: adopt'),
            ('licence = "MIT"\nuse = "ship-in-skills"\nlicence_text_file = "docs/legal/licences/jakubkrehel-skills.txt"', 'licence = "NONE"\nuse = "ship-in-skills"\nlicence_text_file = "docs/legal/licences/jakubkrehel-skills.txt"'),
        ):
            with self.subTest(change=new[:24]):
                tmp = self._copy_vendor_fixture()
                self._edit(tmp / "docs" / "eval-licences.md", old, new)
                code, errors = cvs.cmd_verify(tmp, online=False)
                self.assertEqual(code, 1)
                self.assertTrue(any(e.startswith("unlicensed-file:") and "jakubkrehel-skills" in e for e in errors), errors)

    def test_register_pin_must_match_the_vendored_commit(self) -> None:
        tmp = self._copy_vendor_fixture()
        self._edit(tmp / "docs" / "eval-licences.md", f'pinned = "{cvs.PINNED_COMMIT}"', 'pinned = "0123456789abcdef0123456789abcdef01234567"')
        code, errors = cvs.cmd_verify(tmp, online=False)
        self.assertEqual(code, 1)
        self.assertTrue(any(e.startswith("licence-mismatch:") and "pins jakubkrehel-skills" in e for e in errors), errors)

    def test_third_party_notice_must_carry_the_attribution(self) -> None:
        tmp = self._copy_vendor_fixture()
        (tmp / "THIRD_PARTY_LICENSES.md").write_text("# Third-Party Licenses\n", encoding="utf-8")
        code, errors = cvs.cmd_verify(tmp, online=False)
        self.assertEqual(code, 1)
        self.assertTrue(any("THIRD_PARTY_LICENSES.md does not carry" in e for e in errors), errors)

    @unittest.skipIf(os.name == "nt", "the fake binary is a shell script")
    def test_built_binary_listing_replaces_the_canonical_lint_ids(self) -> None:
        tmp = Path(tempfile.mkdtemp(prefix="dos-636-bin-"))
        self.addCleanup(lambda: shutil.rmtree(tmp, ignore_errors=True))
        (tmp / "debug").mkdir()
        fake = tmp / "debug" / "designos-mcp"
        fake.write_text("#!/bin/sh\nprintf 'contrast\\noverflow-parent\\n'\n", encoding="utf-8")
        fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
        old = os.environ.get("CARGO_TARGET_DIR")
        os.environ["CARGO_TARGET_DIR"] = str(tmp)
        try:
            ids = cvs.load_lint_rule_ids(REPO_ROOT)
        finally:
            if old is None:
                os.environ.pop("CARGO_TARGET_DIR")
            else:
                os.environ["CARGO_TARGET_DIR"] = old
        self.assertEqual(ids, {"contrast", "overflow-parent"}, "a rule the binary no longer lists must stop being a valid target")

    def test_ci_runs_every_vendor_skill_check(self) -> None:
        ci = (REPO_ROOT / ".github" / "workflows" / "ci.yml").read_text(encoding="utf-8")
        for command in (
            "python3 -B scripts/check_vendor_skills.py verify",
            "python3 -B scripts/check_vendor_skills.py extract",
            "python3 -B scripts/check_vendor_skills.py map-coverage",
            "python3 -B -m unittest scripts/test_check_vendor_skills.py",
        ):
            self.assertIn(command, ci, f"CI must run `{command}` so deleting LICENSE or a rule block turns the build red")

    def _rule_entries(self) -> dict:
        jakub_dir = REPO_ROOT / "skills" / "vendor" / "jakubkrehel-skills"
        doc = tomllib.loads((jakub_dir / "rule-map.toml").read_text(encoding="utf-8"))
        return {r["id"]: r for r in doc["rule"]}

    def test_rows_that_carried_unrelated_guidance_now_restate_their_rule(self) -> None:
        """Review finding: `canvas_text` must restate the locked bullet of its own row."""
        rules = self._rule_entries()
        forms6 = rules["better-accessibility/forms#6"]
        self.assertNotEqual(forms6["map"], "lint:placeholder-text", "forms#6 is about a busy submit button, not placeholders")
        self.assertIn("spinner", forms6["canvas_text"].lower())
        hit1 = rules["better-accessibility/hit-areas#1"]
        self.assertNotEqual(hit1["map"], "lint:tap-target-size", "hit-areas#1 is about single-pointer drag alternatives")
        self.assertIn("single-pointer", hit1["canvas_text"])
        color_mix = rules["better-colors/color-formats#5"]["canvas_text"]
        self.assertNotIn("Variable Collections", color_mix, "color-formats#5 says keep derived colours OUT of the token layer")
        self.assertIn("color-mix", color_mix)
        contrast2 = rules["better-colors/contrast#2"]["canvas_text"]
        self.assertNotIn("4.5:1", contrast2, "contrast#2 is about gamut after a lightness push")
        self.assertIn("gamut", contrast2)
        self.assertEqual(rules["explain-interface/from-an-image#14"]["map"], "web-only", "keyboard reachability and focus are web-only")
        self.assertIn("vividness", rules["better-colors/palette-generation#3"]["canvas_text"].lower())
        writing1 = rules["better-writing/SKILL#1"]
        self.assertIn("translation", writing1.get("reason", "") + writing1.get("canvas_text", ""))

    def test_every_canvas_text_is_its_own(self) -> None:
        """No sentence is shared by two rules and none is a group summary tagged with the rule id."""
        rules = [r for r in self._rule_entries().values() if r["map"] != "web-only"]
        texts = [r["canvas_text"] for r in rules]
        self.assertEqual(len(texts), len(set(texts)))
        self.assertEqual([r["id"] for r in rules if r["canvas_text"].endswith(f"({r['id']})")], [])

    def test_repeated_canvas_text_fails_map_coverage(self) -> None:
        tmp = self._copy_vendor_fixture()
        map_path = tmp / "skills" / "vendor" / "jakubkrehel-skills" / "rule-map.toml"
        doc = tomllib.loads(map_path.read_text(encoding="utf-8"))
        first, second = [r for r in doc["rule"] if r["map"] != "web-only"][:2]
        self._edit(
            map_path,
            f"canvas_text = {json.dumps(second['canvas_text'], ensure_ascii=False)}",
            f"canvas_text = {json.dumps(first['canvas_text'], ensure_ascii=False)}",
        )
        code, errors = cvs.cmd_map_coverage(tmp)
        self.assertEqual(code, 1)
        self.assertTrue(any(e.startswith("duplicate-canvas-text:") for e in errors), errors)

    def test_check_mapping_is_unknown_without_the_surface_data(self) -> None:
        tmp = self._copy_vendor_fixture()
        map_path = tmp / "skills" / "vendor" / "jakubkrehel-skills" / "rule-map.toml"
        self._edit(map_path, 'map = "rubric:color"', 'map = "check:surface.web-app.required-states"')
        code, errors = cvs.cmd_map_coverage(tmp)
        self.assertEqual(code, 1)
        self.assertTrue(any(e.startswith("unknown-check-id:") for e in errors), errors)

    def test_fail_before_when_vendor_folder_missing(self) -> None:
        """When `skills/vendor` does not exist, `verify` exits 2."""
        tmp = Path(tempfile.mkdtemp(prefix="dos-636-empty-"))
        self.addCleanup(lambda: shutil.rmtree(tmp, ignore_errors=True))
        code, _ = cvs.cmd_verify(tmp, online=False)
        self.assertEqual(code, 2)


if __name__ == "__main__":
    unittest.main()
