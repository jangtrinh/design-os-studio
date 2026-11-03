"""A fault inside `publish` never leaves a pair that looks whole. The committed file and its allow-list are two files, so a disk-full error, an
interrupt or a kill can land between them. Both temporaries are written first (the slow, fault-prone step: nothing is replaced yet), the two
renames follow back to back with the file first and the allow-list last, and leftover temporaries are removed in a `finally`. The states a
fault leaves are therefore old + old, or new file + old allow-list, which the gate's pair guard rejects.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_publish_fault
"""
import json
import os
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
import library_publish as pub  # noqa: E402
import studio_ui_gate_pair as pair  # noqa: E402


class FaultTest(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        root = Path(self._tmp.name)
        self.out = root / "design" / "studio-ui.designos"
        self.out.parent.mkdir()
        self.out.write_text("old drawing")
        self.allow_path = self.out.parent / pub.ALLOWLIST_NAME
        self.allow_path.write_text(json.dumps(pair.allowlist_document(self.out, [{"path": "old"}])))
        self.staged = pub.stage_path(root / "ws", self.out)
        self.staged.parent.mkdir(parents=True)
        self.staged.write_text("new drawing")
        pub.staged_allowlist(self.staged).write_text(json.dumps(pair.allowlist_document(self.staged, [{"path": "new"}])))

    def state(self):
        return self.out.read_text(), json.loads(self.allow_path.read_text())["allow"][0]["path"]

    def gate_failures(self):
        return pair.check_pair(self.out, json.loads(self.allow_path.read_text()))[0]

    def names(self):
        return sorted(p.name for p in self.out.parent.iterdir())

    def raising_on(self, real, nth, error):
        calls = []

        def wrapper(*args, **kwargs):
            calls.append(args)
            if len(calls) == nth:
                raise error
            return real(*args, **kwargs)
        return wrapper, calls

    def test_a_clean_publish_is_a_pair_the_gate_accepts(self):
        pub.publish(self.staged, self.out)
        self.assertEqual(self.state(), ("new drawing", "new"))
        self.assertEqual(self.gate_failures(), [])
        self.assertEqual(self.names(), ["studio-ui.designos", "studio-ui.gate-allowlist.json"])

    def test_the_old_pair_is_a_pair_the_gate_accepts(self):
        self.assertEqual(self.gate_failures(), [])

    def test_a_fault_between_the_two_replacements_leaves_a_pair_the_gate_rejects(self):
        """The file is new and the allow-list is the old one: the recorded hash is the old file's, so the gate refuses it."""
        flaky, calls = self.raising_on(os.replace, 2, KeyboardInterrupt("injected between the two replacements"))
        with mock.patch.object(pub.os, "replace", flaky), self.assertRaises(KeyboardInterrupt):
            pub.publish(self.staged, self.out)
        self.assertEqual(len(calls), 2)
        self.assertEqual(self.state(), ("new drawing", "old"))
        failures = self.gate_failures()
        self.assertEqual(len(failures), 1, failures)
        self.assertIn("half-published pair", failures[0])

    def test_no_temporary_file_is_left_after_a_fault_between_the_replacements(self):
        flaky, _ = self.raising_on(os.replace, 2, OSError("injected"))
        with mock.patch.object(pub.os, "replace", flaky), self.assertRaises(OSError):
            pub.publish(self.staged, self.out)
        self.assertEqual(self.names(), ["studio-ui.designos", "studio-ui.gate-allowlist.json"])

    def test_a_fault_while_copying_the_second_file_leaves_the_old_pair_whole(self):
        """Disk full on the allow-list copy: nothing has been replaced yet, so the old file and the old allow-list are both still there."""
        flaky, calls = self.raising_on(pub.shutil.copyfile, 2, OSError("disk full"))
        with mock.patch.object(pub.shutil, "copyfile", flaky), self.assertRaises(OSError):
            pub.publish(self.staged, self.out)
        self.assertEqual(len(calls), 2)
        self.assertEqual(self.state(), ("old drawing", "old"))
        self.assertEqual(self.gate_failures(), [])
        self.assertEqual(self.names(), ["studio-ui.designos", "studio-ui.gate-allowlist.json"])

    def test_a_fault_on_the_first_replacement_leaves_the_old_pair_whole(self):
        flaky, _ = self.raising_on(os.replace, 1, OSError("injected"))
        with mock.patch.object(pub.os, "replace", flaky), self.assertRaises(OSError):
            pub.publish(self.staged, self.out)
        self.assertEqual(self.state(), ("old drawing", "old"))
        self.assertEqual(self.gate_failures(), [])
        self.assertEqual(self.names(), ["studio-ui.designos", "studio-ui.gate-allowlist.json"])

    def test_both_files_are_copied_to_temporary_names_before_either_is_replaced(self):
        events = []
        real_copy, real_replace = pub.shutil.copyfile, os.replace
        with mock.patch.object(pub.shutil, "copyfile", lambda s, d: (events.append(("copy", Path(d).name)), real_copy(s, d))[1]), \
                mock.patch.object(pub.os, "replace", lambda s, d: (events.append(("replace", Path(d).name)), real_replace(s, d))[1]):
            pub.publish(self.staged, self.out)
        self.assertEqual(events, [("copy", "studio-ui.designos.publishing"), ("copy", "studio-ui.gate-allowlist.json.publishing"),
                                  ("replace", "studio-ui.designos"), ("replace", "studio-ui.gate-allowlist.json")])

    def test_a_leftover_temporary_of_an_earlier_crash_is_replaced_and_removed(self):
        (self.out.parent / "studio-ui.designos.publishing").write_text("junk from a crash")
        pub.publish(self.staged, self.out)
        self.assertEqual(self.state(), ("new drawing", "new"))
        self.assertEqual(self.names(), ["studio-ui.designos", "studio-ui.gate-allowlist.json"])


if __name__ == "__main__":
    unittest.main()
