"""`library` replaces the committed file and its allow-list only after every check passed: the staged copy lives in the workspace and
`publish` is the last step. A failing check never reaches it (proved end to end by the seeded-failure run in the fix-round report).

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_publish
"""
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
import library_publish as pub  # noqa: E402


class PublishTest(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)
        self.out = self.root / "design" / "studio-ui.designos"
        self.out.parent.mkdir()
        self.out.write_text("old file")
        (self.out.parent / pub.ALLOWLIST_NAME).write_text("old allow-list")
        self.staged = pub.stage_path(self.root / "ws", self.out)
        self.staged.parent.mkdir(parents=True)

    def tearDown(self):
        self._tmp.cleanup()

    def stage(self, file="new file", allow="new allow-list"):
        if file is not None:
            self.staged.write_text(file)
        if allow is not None:
            pub.staged_allowlist(self.staged).write_text(allow)

    def test_the_staged_copy_is_inside_the_workspace_not_at_the_output(self):
        self.assertEqual(self.staged, self.root / "ws" / "staged" / "studio-ui.designos")
        self.assertNotEqual(self.staged, self.out)
        self.assertEqual(pub.staged_allowlist(self.staged), self.root / "ws" / "staged" / "studio-ui.gate-allowlist.json")

    def test_nothing_changes_until_publish_runs(self):
        self.stage()
        self.assertEqual(self.out.read_text(), "old file")
        self.assertEqual((self.out.parent / pub.ALLOWLIST_NAME).read_text(), "old allow-list")

    def test_publish_replaces_the_file_and_its_allow_list_and_leaves_no_temporary_name(self):
        self.stage()
        pub.publish(self.staged, self.out)
        self.assertEqual(self.out.read_text(), "new file")
        self.assertEqual((self.out.parent / pub.ALLOWLIST_NAME).read_text(), "new allow-list")
        self.assertEqual(sorted(p.name for p in self.out.parent.iterdir()), ["studio-ui.designos", "studio-ui.gate-allowlist.json"])

    def test_a_missing_staged_allow_list_publishes_nothing_not_half(self):
        self.stage(allow=None)
        with self.assertRaises(SystemExit) as e:
            pub.publish(self.staged, self.out)
        self.assertIn("unchanged", str(e.exception))
        self.assertEqual(self.out.read_text(), "old file")

    def test_a_missing_staged_file_publishes_nothing(self):
        self.stage(file=None)
        with self.assertRaises(SystemExit):
            pub.publish(self.staged, self.out)
        self.assertEqual((self.out.parent / pub.ALLOWLIST_NAME).read_text(), "old allow-list")

    def test_the_output_folder_is_created_when_missing(self):
        self.stage()
        out = self.root / "fresh" / "studio-ui.designos"
        pub.publish(self.staged, out)
        self.assertEqual(out.read_text(), "new file")
        self.assertEqual((out.parent / pub.ALLOWLIST_NAME).read_text(), "new allow-list")


if __name__ == "__main__":
    unittest.main()
