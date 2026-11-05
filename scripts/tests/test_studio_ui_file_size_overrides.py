"""Swapped icon slots and their glyphs record a Size override, as the editor does when a slot inside a component is resized.

A control's icon slot is a nested instance of an `Icon/*` main (16 px). A 20 px tool, rail or icon-button glyph is that slot resized, so the editor
records a `Size` override on the slot and on its glyph. Without it a swap (the `Icon` instance-swap property) treats the 20 px as the main's default and
resets the slot to 16, or needs the engine to promote Min/Min vector constraints to Scale to keep the box (#1427, review round 3 MAJ-1). A synthetic
native document stands in for the saved file; `record` is what the generator's post-pass runs and `missing` is what it asserts afterwards.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_size_overrides
"""
import copy
import json
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
import library_verify  # noqa: E402
import native_post_pass as post  # noqa: E402
import size_overrides as so  # noqa: E402

MAIN, MAIN_GLYPH, BUTTON, SLOT, GLYPH = "22", "23", "30", "31", "32"


def node(nid, name, kind, parent, children=(), size=16.0, component=None):
    n = {"id": int(nid), "name": name, "kind": kind, "parent": int(parent) if parent else None, "children": [int(c) for c in children], "size": {"x": size, "y": size}}
    if component is not None:
        n["component"] = component
    return n


def doc(slot=20.0, glyph=20.0, slot_ov=(), glyph_ov=("Fills",), host="Component", main_name="Icon/Plus", source=MAIN):
    """A Button main (or another host kind) with one icon slot of `slot` px holding a `glyph` px glyph, resized from the 16 px `Icon/Plus` main."""
    nodes = {
        "1": node(1, "02 Core", "Page", None, [BUTTON, MAIN]),
        MAIN: node(MAIN, main_name, {"Component": {}}, 1, [MAIN_GLYPH]),
        MAIN_GLYPH: node(MAIN_GLYPH, "Glyph", {"Vector": {}}, MAIN),
        BUTTON: node(BUTTON, "Size=Sm, State=Default", {host: {}}, 1, [SLOT], size=36.0),
        SLOT: node(SLOT, "Leading icon", {"Instance": {"component": int(MAIN)}}, BUTTON, [GLYPH], size=slot,
                   component={"source": int(source), "bindings": {"MainComponent": "Icon"}, **({"overrides": list(slot_ov)} if slot_ov else {})}),
        GLYPH: node(GLYPH, "Glyph", {"Vector": {}}, SLOT, size=glyph, component={"source": int(MAIN_GLYPH), "overrides": list(glyph_ov)}),
    }
    return {"nodes": nodes, "pages": [1]}


def overrides(d, nid):
    return (d["nodes"][nid].get("component") or {}).get("overrides") or []


class MissingTest(unittest.TestCase):
    def test_a_resized_slot_and_glyph_without_the_override_are_both_reported(self):
        out = so.missing(doc())
        self.assertEqual([m["id"] for m in out], [int(SLOT), int(GLYPH)])
        self.assertEqual([m["what"] for m in out], ["slot", "glyph"])
        self.assertTrue(all("Size" in m["problem"] for m in out), out)

    def test_a_slot_without_the_override_is_reported_by_itself(self):
        self.assertEqual([m["id"] for m in so.missing(doc(glyph_ov=("Fills", "Size")))], [int(SLOT)])

    def test_a_glyph_without_the_override_is_reported_by_itself(self):
        self.assertEqual([m["id"] for m in so.missing(doc(slot_ov=("Size",)))], [int(GLYPH)])

    def test_both_overrides_present_is_clean(self):
        self.assertEqual(so.missing(doc(slot_ov=("Size",), glyph_ov=("Fills", "Size"))), [])

    def test_a_slot_at_the_main_size_needs_no_override(self):
        self.assertEqual(so.missing(doc(slot=16.0, glyph=16.0)), [])

    def test_only_the_axis_that_differs_counts(self):
        d = doc(slot=16.0, glyph=16.0)
        d["nodes"][SLOT]["size"] = {"x": 16.0, "y": 20.0}
        self.assertEqual([m["id"] for m in so.missing(d)], [int(SLOT)])

    def test_a_slot_outside_a_component_is_not_a_swap_target(self):
        """A specimen instance on the foundation page is placed, not defined: the editor's override is for a slot inside a component."""
        self.assertEqual(so.missing(doc(host="Frame")), [])

    def test_a_resized_instance_of_something_that_is_not_an_icon_main_is_an_instance_and_its_child_is_not_a_glyph(self):
        """The editor's rule covers every top instance: a resized ring is reported as an instance, and only an Icon main pairs the glyph."""
        d = doc(main_name="Focus/Ring")
        self.assertEqual([(m["id"], m["what"]) for m in so.missing(d)], [(int(SLOT), "instance")])

    def test_an_instance_whose_component_field_names_another_source_is_left_alone(self):
        self.assertEqual(so.missing(doc(source=BUTTON)), [])


class RecordTest(unittest.TestCase):
    def test_record_adds_size_to_the_slot_and_the_glyph_and_keeps_the_other_overrides(self):
        d = doc()
        counts = so.record(d)
        self.assertEqual(counts, {"slots": 1, "glyphs": 1, "instances": 0, "radii": 0, "layout": 0})
        self.assertEqual(overrides(d, SLOT), ["Size"])
        self.assertEqual(overrides(d, GLYPH), ["Fills", "Size"])
        self.assertEqual(so.missing(d), [])

    def test_record_keeps_the_slot_bindings(self):
        d = doc()
        so.record(d)
        self.assertEqual(d["nodes"][SLOT]["component"]["bindings"], {"MainComponent": "Icon"})
        self.assertEqual(d["nodes"][SLOT]["component"]["source"], int(MAIN))

    def test_record_twice_adds_nothing_the_second_time(self):
        d = doc()
        so.record(d)
        once = copy.deepcopy(d)
        self.assertEqual(so.record(d), {"slots": 0, "glyphs": 0, "instances": 0, "radii": 0, "layout": 0})
        self.assertEqual(d, once)

    def test_record_touches_nothing_that_needs_no_override(self):
        d = doc(slot=16.0, glyph=16.0)
        before = copy.deepcopy(d)
        self.assertEqual(so.record(d), {"slots": 0, "glyphs": 0, "instances": 0, "radii": 0, "layout": 0})
        self.assertEqual(d, before)

    def test_a_slot_that_already_carries_size_is_not_counted_again(self):
        d = doc(slot_ov=("Size",))
        self.assertEqual(so.record(d), {"slots": 0, "glyphs": 1, "instances": 0, "radii": 0, "layout": 0})

    def test_a_slot_with_no_glyph_child_is_recorded_without_one(self):
        d = doc()
        d["nodes"][SLOT]["children"] = []
        self.assertEqual(so.record(d), {"slots": 1, "glyphs": 0, "instances": 0, "radii": 0, "layout": 0})
        self.assertEqual(so.missing(d), [])


class PipelineTest(unittest.TestCase):
    """The post-pass records the overrides in the file it writes; the verify after the reopen refuses a file that lacks one."""

    def write(self, d, name="in.designos"):
        d = {**d, "variables": {}, "style_meta": {}}
        path = Path(self.tmp.name) / name
        path.write_text(json.dumps(d), encoding="utf-8")
        return path

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)

    def test_the_post_pass_writes_the_overrides_and_reports_the_counts(self):
        src, dst = self.write(doc()), Path(self.tmp.name) / "out.designos"
        counts = post.post_pass(src, dst, {"semantic": []}, {})
        self.assertEqual(counts["size_overrides"], {"slots": 1, "glyphs": 1, "instances": 0, "radii": 0, "layout": 0})
        written = json.loads(dst.read_text(encoding="utf-8"))
        self.assertEqual((overrides(written, SLOT), overrides(written, GLYPH)), (["Size"], ["Fills", "Size"]))

    def test_the_verify_refuses_a_file_whose_swapped_slot_has_no_size_override(self):
        with self.assertRaises(SystemExit) as raised:
            library_verify.verify(None, self.write(doc()), {"semantic": []}, [])
        self.assertIn("Size override", str(raised.exception))
        self.assertIn("Leading icon", str(raised.exception))

    def test_the_verify_counts_the_nodes_that_carry_the_override(self):
        out = library_verify.verify(None, self.write(doc(slot_ov=("Size",), glyph_ov=("Fills", "Size"))), {"semantic": []}, [])
        self.assertEqual(out["size_overrides_verified"], 2)


if __name__ == "__main__":
    unittest.main()
