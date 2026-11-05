"""The editor's override rule for every top instance inside a main, not only icon slots.

A top instance (an Instance whose `component.source` is its own component) inside a Component records what changed against its main, as the editor
does (`record_overrides`, crates/designos-app/src/comp_ops.rs:60-106): a top instance drops only Transform, Visible and Locked, so a size that differs
is `Size` and corners that differ are `CornerRadii` (`record_override`, designos-doc/src/components.rs:465). A node INSIDE an instance drops Size when it is
derived (a text that sizes itself, a child of an auto layout that hugs or fills). The 22 resized Focus/Ring instances and the hug Button that carries a longer
label are the two cases the generator meets. Synthetic native documents; `record` is the post-pass and `missing` its assertion.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_size_overrides_editor_rule
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
import size_overrides as so  # noqa: E402

PAGE, RING_MAIN, BTN_MAIN, BTN_TEXT_MAIN, HOST, RING, BTN, BTN_TEXT = 1, 10, 20, 21, 30, 31, 32, 33


def node(nid, name, kind, parent, children=(), size=(16.0, 16.0), **extra):
    return {"id": nid, "name": name, "kind": kind, "parent": parent, "children": list(children), "size": {"x": size[0], "y": size[1]}, **extra}


def doc(ring=(20.0, 20.0), ring_radii=12.0, ring_ov=("Layout",), btn=(80.0, 32.0), btn_ov=(), text=(50.0, 20.0), host="Component"):
    """A host with a Focus ring instance (16 px main, radius 12) and a Button instance that hugs a longer label than its main (60 px, 36.75 px text)."""
    def comp(source, ov):
        return {"source": source, **({"overrides": list(ov)} if ov else {})}
    nodes = {
        "1": node(PAGE, "02 Core", "Page", None, [RING_MAIN, BTN_MAIN, HOST]),
        "10": node(RING_MAIN, "Tone=Brand", {"Component": {}}, PAGE, corner_radii=[12.0] * 4),
        "20": node(BTN_MAIN, "Size=Sm, State=Default", {"Component": {}}, PAGE, [BTN_TEXT_MAIN], size=(60.0, 32.0)),
        "21": node(BTN_TEXT_MAIN, "Label", {"Text": {"auto_resize": "WidthAndHeight"}}, BTN_MAIN, size=(36.75, 20.0)),
        "30": node(HOST, "Field", {host: {}}, PAGE, [RING, BTN], size=(200.0, 40.0)),
        "31": node(RING, "Focus ring", {"Instance": {"component": RING_MAIN}}, HOST, size=ring, corner_radii=[ring_radii] * 4,
                   component=comp(RING_MAIN, ring_ov)),
        "32": node(BTN, "Action", {"Instance": {"component": BTN_MAIN}}, HOST, [BTN_TEXT], size=btn, component=comp(BTN_MAIN, btn_ov)),
        "33": node(BTN_TEXT, "Label", {"Text": {"auto_resize": "WidthAndHeight"}}, BTN, size=text,
                   component={"source": BTN_TEXT_MAIN, "overrides": ["Characters"], "bindings": {"Characters": "Label"}}),
    }
    return {"nodes": nodes, "pages": [PAGE]}


def flags(d, nid):
    return (d["nodes"][str(nid)].get("component") or {}).get("overrides") or []


class ResizedRing(unittest.TestCase):
    def test_a_resized_ring_without_size_is_reported(self):
        out = [m for m in so.missing(doc()) if m["id"] == RING]
        self.assertEqual([m["what"] for m in out], ["instance"])
        self.assertIn("Size", out[0]["problem"])
        self.assertIn("Focus ring", out[0]["name"])

    def test_record_adds_size_and_keeps_layout(self):
        d = doc()
        so.record(d)
        self.assertEqual(flags(d, RING), ["Layout", "Size"])
        self.assertEqual(so.missing(d), [])

    def test_a_ring_at_the_main_size_needs_none(self):
        d = doc(ring=(16.0, 16.0))
        self.assertNotIn(RING, [m["id"] for m in so.missing(d)])

    def test_a_ring_inside_a_frame_is_placed_not_defined(self):
        self.assertEqual(so.missing(doc(host="Frame")), [])


class PinnedCorners(unittest.TestCase):
    def test_pinned_corners_without_the_flag_are_reported(self):
        d = doc(ring=(16.0, 16.0), ring_radii=6.0)
        out = so.missing(d)
        self.assertEqual([(m["id"], m["what"]) for m in out if m["id"] == RING], [(RING, "instance")])
        self.assertIn("CornerRadii", [m for m in out if m["id"] == RING][0]["problem"])

    def test_record_adds_the_flag_and_keeps_the_others(self):
        d = doc(ring=(16.0, 16.0), ring_radii=6.0)
        counts = so.record(d)
        self.assertEqual(flags(d, RING), ["Layout", "CornerRadii"])
        self.assertEqual(counts["radii"], 1)

    def test_corners_equal_to_the_main_need_no_flag(self):
        d = doc(ring=(16.0, 16.0))
        self.assertEqual(so.record(d)["radii"], 0)
        self.assertEqual(flags(d, RING), ["Layout"])

    def test_resized_and_pinned_gets_both(self):
        d = doc(ring=(20.0, 20.0), ring_radii=6.0)
        so.record(d)
        self.assertEqual(sorted(flags(d, RING)), ["CornerRadii", "Layout", "Size"])

    def test_a_flag_already_there_is_not_added_twice(self):
        d = doc(ring=(16.0, 16.0), ring_radii=6.0, ring_ov=("Layout", "CornerRadii"))
        self.assertEqual(so.record(d)["radii"], 0)
        self.assertEqual(flags(d, RING), ["Layout", "CornerRadii"])


class HugInstanceWithANewLabel(unittest.TestCase):
    """What the editor records when a label changes inside a nested Button that hugs it: the text keeps Characters only (its size is derived, so Size is
    dropped), the layout pass that follows resizes the Button root, a top instance, and the root records Size."""

    def test_the_root_is_reported_and_the_derived_text_is_not(self):
        out = [m for m in so.missing(doc(ring=(16.0, 16.0))) if m["id"] in (BTN, BTN_TEXT)]
        self.assertEqual([(m["id"], m["what"]) for m in out], [(BTN, "instance")])

    def test_record_flags_the_root_and_leaves_the_text_alone(self):
        d = doc(ring=(16.0, 16.0))
        so.record(d)
        self.assertEqual(flags(d, BTN), ["Size"])
        self.assertEqual(flags(d, BTN_TEXT), ["Characters"])
        self.assertEqual(so.missing(d), [])

    def test_a_button_at_its_main_width_carries_no_flag(self):
        d = doc(ring=(16.0, 16.0), btn=(60.0, 32.0), text=(36.75, 20.0))
        so.record(d)
        self.assertEqual((flags(d, BTN), flags(d, BTN_TEXT)), ([], ["Characters"]))

    def test_record_twice_changes_nothing_the_second_time(self):
        d = doc()
        so.record(d)
        once = copy.deepcopy(d)
        self.assertEqual(so.record(d), {"slots": 0, "glyphs": 0, "instances": 0, "radii": 0, "layout": 0})
        self.assertEqual(d, once)


class Verify(unittest.TestCase):
    def test_the_verify_refuses_a_resized_ring_without_size_and_pinned_corners_without_the_flag(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "f.designos"
            path.write_text(json.dumps({**doc(ring=(20.0, 20.0), ring_radii=6.0), "variables": {}, "style_meta": {}}), encoding="utf-8")
            with self.assertRaises(SystemExit) as raised:
                library_verify.verify(None, path, {"semantic": []}, [])
        text = str(raised.exception)
        self.assertIn("Focus ring", text)
        self.assertIn("Size", text)
        self.assertIn("CornerRadii", text)


if __name__ == "__main__":
    unittest.main()
