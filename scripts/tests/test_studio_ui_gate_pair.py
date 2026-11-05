"""The pair guard: `design/studio-ui.gate-allowlist.json` records the sha256 of the `.designos` it was written for (`designos_sha256`), and
the gate refuses a file whose bytes differ. `library` publishes the file first and the allow-list last, so the states a publish can leave are
old + old, new + old (the gate rejects it) and new + new: a half pair can never pass as a whole one.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_gate_pair
"""
import hashlib
import json
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))
import studio_ui_gate_pair as pair  # noqa: E402
import test_check_studio_ui_file as cli_tests  # noqa: E402  (a module, not its TestCase classes, so they are not collected twice)
from studio_ui_gate_fixtures import make_doc  # noqa: E402


class PairTest(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.file = Path(self._tmp.name) / "f.designos"
        self.file.write_text("the drawing")

    def sha(self, text):
        return hashlib.sha256(text.encode()).hexdigest()

    def test_the_file_hash_is_the_sha256_of_its_bytes(self):
        self.assertEqual(pair.file_sha256(self.file), self.sha("the drawing"))

    def test_an_allow_list_written_for_the_file_passes(self):
        doc = pair.allowlist_document(self.file, [{"path": "p"}])
        self.assertEqual(doc, {"designos_sha256": self.sha("the drawing"), "allow": [{"path": "p"}]})
        self.assertEqual(pair.check_pair(self.file, doc)[0], [])

    def test_a_file_that_is_not_the_one_the_allow_list_was_written_for_fails(self):
        doc = pair.allowlist_document(self.file, [])
        self.file.write_text("a different drawing")
        fails, _ = pair.check_pair(self.file, doc)
        self.assertEqual(len(fails), 1, fails)
        self.assertIn("half-published pair", fails[0])
        self.assertIn(self.sha("a different drawing")[:12], fails[0])
        self.assertIn(self.sha("the drawing")[:12], fails[0])

    def test_an_allow_list_with_no_recorded_hash_fails_closed(self):
        for doc in ({"allow": []}, {"designos_sha256": None, "allow": []}, {"designos_sha256": 7, "allow": []}, {"designos_sha256": "", "allow": []}):
            fails, _ = pair.check_pair(self.file, doc)
            self.assertEqual(len(fails), 1, (doc, fails))
            self.assertIn("does not record the sha256", fails[0])

    def test_the_hash_is_of_the_raw_bytes_so_a_trailing_newline_or_non_text_bytes_count(self):
        doc = pair.allowlist_document(self.file, [])
        self.file.write_text("the drawing\n")
        self.assertEqual(len(pair.check_pair(self.file, doc)[0]), 1)
        self.file.write_bytes(b"\xff\xfe\x00 not utf-8")
        self.assertEqual(pair.file_sha256(self.file), hashlib.sha256(b"\xff\xfe\x00 not utf-8").hexdigest())

    def test_the_allow_list_is_written_beside_the_file_with_its_hash(self):
        target = pair.write_allowlist(self.file, [{"path": "p", "property": "paddingLeft", "value": 3.0}])
        self.assertEqual(target, self.file.parent / "studio-ui.gate-allowlist.json")
        written = json.loads(target.read_text())
        self.assertEqual(list(written), ["designos_sha256", "allow"])
        self.assertEqual(written["allow"], [{"path": "p", "property": "paddingLeft", "value": 3.0}])
        self.assertEqual(pair.check_pair(self.file, written)[0], [])
        self.assertTrue(target.read_text().endswith("}\n"))

    def test_one_changed_byte_is_enough(self):
        doc = pair.allowlist_document(self.file, [])
        self.file.write_text("the drawinG")
        self.assertEqual(len(pair.check_pair(self.file, doc)[0]), 1)

    def test_the_comparison_is_case_sensitive_hex_of_the_whole_digest(self):
        doc = {"designos_sha256": self.sha("the drawing")[:-1], "allow": []}
        self.assertEqual(len(pair.check_pair(self.file, doc)[0]), 1)
        doc = {"designos_sha256": self.sha("the drawing").upper(), "allow": []}
        self.assertEqual(len(pair.check_pair(self.file, doc)[0]), 1)


class PairCliTest(unittest.TestCase):
    """The gate command refuses a pair whose allow-list does not name the file: exit 1, not a pass."""
    setUp, run_main = cli_tests.CliTest.setUp, cli_tests.CliTest.run_main

    def test_a_clean_document_with_its_own_allow_list_exits_zero(self):
        self.assertEqual(self.run_main(json.dumps(make_doc())), 0)

    def test_an_allow_list_that_does_not_name_the_file_exits_one(self):
        """A publish cut between the file and the allow-list leaves a new file beside an old allow-list: the gate refuses the pair."""
        stale = {"designos_sha256": "0" * 64, "allow": []}
        self.assertEqual(self.run_main(json.dumps(make_doc()), allow=stale), 1)

    def test_an_allow_list_with_no_recorded_hash_exits_one(self):
        self.assertEqual(self.run_main(json.dumps(make_doc()), allow={"allow": []}), 1)


if __name__ == "__main__":
    unittest.main()
