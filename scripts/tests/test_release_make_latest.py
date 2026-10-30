"""Tests for scripts/release-make-latest.py. Run: python3 -m unittest discover -s scripts/tests -v"""
import importlib.util
import json
import subprocess
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).parent
SCRIPT = HERE.parent / "release-make-latest.py"

spec = importlib.util.spec_from_file_location("release_make_latest", SCRIPT)
mod = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mod)


def release(tag, draft=False, prerelease=False):
    return {"tagName": tag, "isDraft": draft, "isPrerelease": prerelease}


# The published set when v0.7.5 went out before v0.7.1 to v0.7.4.
PUBLISHED = [release("v0.7.5"), release("v0.7.0"), release("v0.6.5"), release("v0.6.4", prerelease=True)]


class MakeLatestTests(unittest.TestCase):
    def test_older_tag_published_after_a_newer_one_is_not_latest(self):
        self.assertEqual(mod.make_latest("v0.7.1", PUBLISHED), "false")

    def test_newer_tag_is_latest(self):
        self.assertEqual(mod.make_latest("v0.7.6", PUBLISHED), "true")

    def test_rerun_of_the_highest_tag_stays_latest(self):
        # v0.7.5 is already in the list when the workflow runs again.
        self.assertEqual(mod.make_latest("v0.7.5", PUBLISHED), "true")

    def test_rerun_of_a_superseded_tag_is_not_latest(self):
        self.assertEqual(mod.make_latest("v0.7.5", PUBLISHED + [release("v0.7.6")]), "false")

    def test_no_releases_yet_is_latest(self):
        self.assertEqual(mod.make_latest("v0.1.0", []), "true")

    def test_compare_is_numeric_not_lexicographic(self):
        # "v0.10.0" sorts before "v0.9.0" as text; as versions it is newer.
        self.assertEqual(mod.make_latest("v0.10.0", [release("v0.9.0")]), "true")
        self.assertEqual(mod.make_latest("v0.9.0", [release("v0.10.0")]), "false")

    def test_higher_draft_or_prerelease_does_not_count(self):
        listed = PUBLISHED + [release("v0.8.0", draft=True), release("v0.9.0", prerelease=True)]
        self.assertEqual(mod.make_latest("v0.7.6", listed), "true")
        self.assertEqual(mod.make_latest("v0.7.1", listed), "false")

    def test_non_stable_tags_in_the_list_are_ignored(self):
        listed = PUBLISHED + [release("v1.0.0-rc.1"), release("nightly"), release("v2.0")]
        self.assertEqual(mod.make_latest("v0.7.6", listed), "true")

    def test_prerelease_tag_leaves_the_decision_to_github(self):
        # Empty means "do not pass make_latest"; GitHub never makes a pre-release Latest.
        self.assertEqual(mod.make_latest("v0.8.0-rc.1", PUBLISHED), "")
        self.assertEqual(mod.make_latest("v0.2.0-rc.1", PUBLISHED), "")

    def test_malformed_release_rows_are_rejected(self):
        with self.assertRaises(mod.LatestError):
            mod.make_latest("v0.7.6", [{"tagName": "v0.7.5"}])
        with self.assertRaises(mod.LatestError):
            mod.make_latest("v0.7.6", ["v0.7.5"])


class CliTests(unittest.TestCase):
    def run_cli(self, tag, stdin):
        return subprocess.run([sys.executable, str(SCRIPT), tag], input=stdin, capture_output=True, text=True)

    def test_prints_exactly_true_or_false(self):
        # The action turns anything but true/false/legacy into "unset", which means Latest.
        # Cases from the owner's ruling: 0.7.1 vs existing 0.7.5, 0.7.6, and a 0.7.5 rerun.
        payload = json.dumps(PUBLISHED)
        for tag, expected in (("v0.7.1", "false\n"), ("v0.7.6", "true\n"), ("v0.7.5", "true\n")):
            out = self.run_cli(tag, payload)
            self.assertEqual(out.returncode, 0, out.stderr)
            self.assertEqual(out.stdout, expected, tag)

    def test_prerelease_tag_prints_nothing(self):
        out = self.run_cli("v0.8.0-rc.1", json.dumps(PUBLISHED))
        self.assertEqual(out.returncode, 0, out.stderr)
        self.assertEqual(out.stdout, "")

    def test_unusable_list_exits_2_never_guesses(self):
        # An empty stdout (gh failed), non-JSON, or an API error body must not read as "no releases".
        for stdin in ("", "not json", '{"message": "Bad credentials"}', "null"):
            out = self.run_cli("v0.7.1", stdin)
            self.assertEqual(out.returncode, 2, stdin)
            self.assertEqual(out.stdout, "", stdin)
            self.assertNotIn("Traceback", out.stderr)
            self.assertIn("gh release list", out.stderr)

    def test_malformed_rows_exit_2(self):
        out = self.run_cli("v0.7.1", json.dumps([{"tagName": "v0.7.5"}]))
        self.assertEqual(out.returncode, 2)
        self.assertEqual(out.stdout, "")

    def test_missing_tag_argument_exits_2(self):
        out = subprocess.run([sys.executable, str(SCRIPT)], input="[]", capture_output=True, text=True)
        self.assertEqual(out.returncode, 2)
        self.assertEqual(out.stdout, "")


if __name__ == "__main__":
    unittest.main()
