"""An entry of crops.json may carry its own `captures` folder (the empty Home capture is not in the folder the 87 older rows were taken into): `captures_for` resolves it like the run's default (repo,
then the main checkout) and ignores `--captures`, which only redirects the default; an entry without it uses the run's folder. No Pillow, no MCP.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_crop_capture_dirs
"""
import json
import subprocess
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file" / "evidence"))
import capture_dirs as cd  # noqa: E402

CROPS = HERE.parent / "studio_ui_file" / "evidence" / "crops.json"
DEFAULT = Path("/captures/default")


class CaptureDirsTest(unittest.TestCase):
    def test_an_override_wins_for_the_default_folder(self):
        self.assertEqual(cd.captures_dir("plans/x", "/elsewhere"), Path("/elsewhere"))

    def test_a_folder_in_the_repo_is_taken_as_it_is(self):
        self.assertEqual(cd.captures_dir("scripts/studio_ui_file/evidence", None), cd.REPO / "scripts/studio_ui_file/evidence")

    def test_a_folder_the_repo_lacks_is_looked_for_beside_the_git_common_dir_the_main_checkout_in_a_worktree(self):
        common = subprocess.check_output(["git", "-C", str(cd.REPO), "rev-parse", "--git-common-dir"], text=True).strip()
        main = (Path(common) if Path(common).is_absolute() else cd.REPO / common).resolve().parent
        self.assertEqual(cd.captures_dir("plans/never-in-the-repo-xyz", None), main / "plans/never-in-the-repo-xyz")

    def test_an_entry_without_its_own_folder_uses_the_runs(self):
        self.assertEqual(cd.captures_for({"case": "h-home"}, DEFAULT), DEFAULT)
        self.assertEqual(cd.captures_for({"case": "h-home", "captures": ""}, DEFAULT), DEFAULT)

    def test_an_entry_with_its_own_folder_resolves_it_like_the_default_and_ignores_the_override(self):
        got = cd.captures_for({"case": "h-home-empty", "captures": "scripts/studio_ui_file/evidence"}, DEFAULT)
        self.assertEqual(got, cd.REPO / "scripts/studio_ui_file/evidence")

    def test_every_own_folder_in_crops_json_is_a_relative_repo_path(self):
        for e in json.loads(CROPS.read_text())["entries"]:
            if "captures" in e:
                self.assertFalse(Path(e["captures"]).is_absolute(), e["captures"])

    def test_every_own_folder_is_tracked_in_the_repo_so_a_fresh_copy_needs_no_manual_copy(self):
        """The picture and its layout sit in the checkout itself, not in the main checkout the fallback finds (merge review LOW 6): a `git archive` or a scratch copy has them."""
        entries = [e for e in json.loads(CROPS.read_text())["entries"] if "captures" in e]
        self.assertTrue(entries)
        for e in entries:
            with self.subTest(e["case"]):
                folder = cd.REPO / e["captures"]
                self.assertEqual(cd.captures_dir(e["captures"], None), folder, "resolved inside the repo, not through the main-checkout fallback")
                for name in (f"{e['case']}.png", f"{e['case']}.layout.json"):
                    self.assertTrue((folder / name).is_file(), f"{folder / name} is missing")
                if (cd.REPO / ".git").exists():
                    tracked = subprocess.run(["git", "-C", str(cd.REPO), "ls-files", "--error-unmatch", str((folder / f"{e['case']}.png").relative_to(cd.REPO))], capture_output=True)
                    self.assertEqual(tracked.returncode, 0, f"{e['case']}.png is not tracked by git")


if __name__ == "__main__":
    unittest.main()
