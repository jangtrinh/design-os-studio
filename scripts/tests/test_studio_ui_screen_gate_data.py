"""The per-batch gate data (studio_ui_gate_screen_data): a batch adds `screens/<batch>_gate.py` and nothing else, `screen_caps` and `screen_masks` read the merged tables, and `KEEP` leaves a rect
inside the canvas content measured (a dialog or popover over the canvas is chrome).

Run: python3 -B -m unittest scripts.tests.test_studio_ui_screen_gate_data
"""
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file" / "evidence"))
sys.path.insert(0, str(HERE.parent))
import screen_caps as caps  # noqa: E402
import screen_masks as sm  # noqa: E402
import studio_ui_gate_screen_data as gd  # noqa: E402

MODULE = '''CAPS = {"x-one": {"screen": 4.6}}
CAUSES = {"x-one": {}}
REGIONS = {"x-one": {"home": (0, 32, 264, 868)}}
'''


def folder(**files):
    tmp = tempfile.TemporaryDirectory()
    for name, text in files.items():
        (Path(tmp.name) / f"{name}.py").write_text(text, encoding="utf-8")
    return tmp


class LoadTest(unittest.TestCase):
    def test_every_gate_module_of_the_folder_is_found_by_its_file_name_and_merged(self):
        with folder(w9_gate="CAPS = {'x-a': {'screen': 1.0}}\nKEEP = {'x-a': [(1, 2, 3, 4)]}\n", w8_gate="CAPS = {'x-b': {'screen': 2.0}}\n", w8_screens="CAPS = {'x-z': {}}\n") as d:
            t = gd.load(d)
        self.assertEqual(t["CAPS"], {"x-a": {"screen": 1.0}, "x-b": {"screen": 2.0}})
        self.assertEqual(t["KEEP"], {"x-a": [(1, 2, 3, 4)]})
        self.assertEqual(set(t), set(gd.TABLES))

    def test_an_empty_folder_has_empty_tables(self):
        with folder() as d:
            self.assertEqual(gd.load(d), {t: {} for t in gd.TABLES})

    def test_a_state_in_two_modules_fails_naming_both(self):
        with folder(a_gate=MODULE, b_gate=MODULE) as d:
            with self.assertRaisesRegex(ValueError, r"a_gate.py and by b_gate.py"):
                gd.load(d)

    def test_a_table_that_is_not_a_known_one_fails_so_a_typo_does_not_pass_unread(self):
        with folder(a_gate="CAP = {'x-one': {}}\n") as d:
            with self.assertRaisesRegex(ValueError, "CAP"):
                gd.load(d)

    def test_a_table_that_is_not_a_dict_fails(self):
        with folder(a_gate="CAPS = [1]\n") as d:
            with self.assertRaisesRegex(ValueError, "dict"):
                gd.load(d)

    def test_merge_adds_the_states_and_refuses_one_the_framework_table_has(self):
        t = {"CAPS": {"x-new": {"screen": 1.0}}}
        self.assertEqual(gd.merge({"d-editor-default": {}}, t, "CAPS"), {"d-editor-default": {}, "x-new": {"screen": 1.0}})
        with self.assertRaisesRegex(ValueError, "d-editor-default"):
            gd.merge({"d-editor-default": {}}, {"CAPS": {"d-editor-default": {}}}, "CAPS")

    def test_the_real_batches_load_and_keep_the_framework_states(self):
        self.assertIn("d-editor-default", caps.CAPS)
        self.assertIn("d-editor-default", sm.TEXTS)
        gd.load()


STATE = "x-dialog"
CONTENT = ((310, 52, 889, 848), (464, 840, 512, 48))


def covered(ex, x, y):
    return any(e[0] <= x < e[0] + e[2] and e[1] <= y < e[1] + e[3] for e in ex)


class KeepTest(unittest.TestCase):
    def exclusions(self, keep):
        tables = {"TEXTS": {STATE: []}, "CONTENT": {STATE: CONTENT}, "KEEP": {STATE: keep}}
        with mock.patch.dict(sm.TEXTS, tables["TEXTS"]), mock.patch.dict(sm.CONTENT, tables["CONTENT"]), mock.patch.dict(sm.KEEP, tables["KEEP"]):
            return sm.exclusions(STATE)

    def test_without_a_keep_rect_the_content_is_excluded_as_before(self):
        ex = self.exclusions([])
        self.assertTrue(covered(ex, 700, 300) and covered(ex, 400, 600))
        self.assertFalse(covered(ex, 700, 860), "the toolbar is chrome")

    def test_a_kept_rect_is_measured_and_its_surroundings_are_still_excluded(self):
        keep = (500, 200, 300, 400)
        ex = self.exclusions([keep])
        for point in ((500, 200), (799, 599), (650, 400)):
            self.assertFalse(covered(ex, *point), point)
        for point in ((499, 300), (800, 300), (650, 199), (650, 600), (400, 300), (900, 700)):
            self.assertTrue(covered(ex, *point), point)

    def test_a_keep_rect_over_the_edge_of_the_content_area_leaves_the_rest_excluded(self):
        ex = self.exclusions([(1100, 600, 300, 300)])
        self.assertFalse(covered(ex, 1150, 650))
        self.assertTrue(covered(ex, 1050, 650) and covered(ex, 1150, 580))

    def test_two_kept_rects_are_both_measured(self):
        ex = self.exclusions([(500, 200, 100, 100), (800, 500, 100, 100)])
        self.assertFalse(covered(ex, 550, 250) or covered(ex, 850, 550))
        self.assertTrue(covered(ex, 700, 250))

    def test_keep_never_unmasks_a_replaced_text(self):
        text = ((520, 220, 50, 20), "label", "Name")
        with mock.patch.dict(sm.TEXTS, {STATE: [text]}), mock.patch.dict(sm.CONTENT, {STATE: CONTENT}), mock.patch.dict(sm.KEEP, {STATE: [(500, 200, 300, 400)]}):
            self.assertTrue(covered(sm.exclusions(STATE), 530, 230))

    def test_a_state_with_no_content_excludes_only_its_texts(self):
        with mock.patch.dict(sm.TEXTS, {STATE: []}), mock.patch.dict(sm.CONTENT, {STATE: None}):
            self.assertEqual(sm.exclusions(STATE), [])


if __name__ == "__main__":
    unittest.main()
