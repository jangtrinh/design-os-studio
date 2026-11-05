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

    def test_tag_that_is_neither_stable_nor_hyphenated_is_rejected(self):
        # `v1.0.0+b1` is not a pre-release (no hyphen), so GitHub would make it Latest; the helper
        # cannot rank it, so it must refuse rather than print nothing.
        for tag in ("v1.0.0+b1", "v1.0", "nightly", "0.7.1", ""):
            with self.assertRaises(mod.LatestError, msg=tag):
                mod.make_latest(tag, PUBLISHED)
        self.assertEqual(mod.make_latest("v1.0.0-rc.1+b1", PUBLISHED), "")


def listed(tag, latest=False, **kw):
    return {**release(tag, **kw), "isLatest": latest}


# v0.7.1 was published without the guard and took the Latest badge from v0.7.5.
STOLEN = [listed("v0.7.1", latest=True), listed("v0.7.5"), listed("v0.7.0"), listed("v0.6.4", prerelease=True)]
# The badge is where it should be.
CORRECT = [listed("v0.7.1"), listed("v0.7.5", latest=True), listed("v0.7.0")]


class ReassertLatestTests(unittest.TestCase):
    """After an older number is published, the sync workflow puts Latest back on the highest release."""

    def test_older_tag_that_took_latest_hands_it_back_to_the_highest(self):
        self.assertEqual(mod.reassert_latest("v0.7.1", STOLEN), "v0.7.5")

    def test_nothing_to_do_when_the_highest_is_already_latest(self):
        self.assertEqual(mod.reassert_latest("v0.7.1", CORRECT), "")

    def test_nothing_to_do_for_the_highest_tag_when_it_holds_latest(self):
        self.assertEqual(mod.reassert_latest("v0.7.5", CORRECT), "")

    def test_the_highest_tag_is_repaired_when_the_badge_is_elsewhere(self):
        # A manual re-run for the highest version (v0.7.1 holds Latest) must repair, not report "all right".
        self.assertEqual(mod.reassert_latest("v0.7.5", STOLEN), "v0.7.5")
        self.assertEqual(mod.reassert_latest("v0.7.6", STOLEN + [listed("v0.7.6")]), "v0.7.6")

    def test_tag_newer_than_every_listed_release_is_left_alone(self):
        # The list is stale (the tag's release is not in it yet); promoting a lower release would be wrong.
        self.assertEqual(mod.reassert_latest("v0.7.6", STOLEN), "")

    def test_a_tag_below_a_newer_one_is_reasserted_even_if_it_is_not_the_oldest(self):
        rows = [listed("v0.7.3", latest=True), listed("v0.7.6"), listed("v0.7.5")]
        self.assertEqual(mod.reassert_latest("v0.7.3", rows), "v0.7.6")

    def test_drafts_and_prereleases_are_never_promoted(self):
        rows = [listed("v0.7.1", latest=True), listed("v0.7.5"),
                listed("v0.9.0", draft=True), listed("v0.8.0", prerelease=True), listed("v1.0.0-rc.1")]
        self.assertEqual(mod.reassert_latest("v0.7.1", rows), "v0.7.5")

    def test_compare_is_numeric(self):
        rows = [listed("v0.9.0", latest=True), listed("v0.10.0")]
        self.assertEqual(mod.reassert_latest("v0.9.0", rows), "v0.10.0")

    def test_no_published_stable_release_means_nothing_to_promote(self):
        self.assertEqual(mod.reassert_latest("v0.7.1", [listed("v0.7.1", draft=True)]), "")

    def test_prerelease_tag_is_left_alone(self):
        self.assertEqual(mod.reassert_latest("v0.8.0-rc.1", STOLEN), "")

    def test_rows_without_islatest_are_rejected(self):
        with self.assertRaises(mod.LatestError):
            mod.reassert_latest("v0.7.1", PUBLISHED)

    def test_tag_that_cannot_be_ranked_is_rejected(self):
        with self.assertRaises(mod.LatestError):
            mod.reassert_latest("v1.0.0+b1", STOLEN)

    def test_make_latest_and_reassert_agree_on_the_highest_release(self):
        # One choice, one implementation: the tag make_latest says "true" for is what reassert promotes.
        rows = [listed("v0.7.1", latest=True), listed("v0.7.6"), listed("v0.7.5")]
        self.assertEqual(mod.make_latest("v0.7.6", rows), "true")
        self.assertEqual(mod.make_latest("v0.7.1", rows), "false")
        self.assertEqual(mod.reassert_latest("v0.7.1", rows), "v0.7.6")


class CliTests(unittest.TestCase):
    def run_cli(self, tag, stdin):
        return subprocess.run([sys.executable, str(SCRIPT), tag], input=stdin, capture_output=True, text=True)

    def run_cli_with(self, flag, tag, stdin):
        args = [flag] + ([tag] if tag else [])
        return subprocess.run([sys.executable, str(SCRIPT), *args], input=stdin, capture_output=True, text=True)

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

    def test_unrankable_tag_exits_2_with_empty_stdout(self):
        out = self.run_cli("v1.0.0+b1", json.dumps(PUBLISHED))
        self.assertEqual(out.returncode, 2)
        self.assertEqual(out.stdout, "")
        self.assertIn("v1.0.0+b1", out.stderr)

    def test_reassert_prints_the_tag_to_promote_or_nothing(self):
        out = self.run_cli_with("--reassert", "v0.7.1", json.dumps(STOLEN))
        self.assertEqual((out.returncode, out.stdout), (0, "v0.7.5\n"), out.stderr)
        out = self.run_cli_with("--reassert", "v0.7.1", json.dumps(CORRECT))
        self.assertEqual((out.returncode, out.stdout), (0, ""), out.stderr)

    def test_reassert_unusable_input_exits_2(self):
        for stdin in ("", "not json", '{"message": "Bad credentials"}', json.dumps(PUBLISHED)):
            out = self.run_cli_with("--reassert", "v0.7.1", stdin)
            self.assertEqual(out.returncode, 2, stdin)
            self.assertEqual(out.stdout, "", stdin)
            self.assertNotIn("Traceback", out.stderr)

    def test_reassert_without_a_tag_exits_2(self):
        out = self.run_cli_with("--reassert", None, "[]")
        self.assertEqual(out.returncode, 2)
        self.assertEqual(out.stdout, "")


if __name__ == "__main__":
    unittest.main()
