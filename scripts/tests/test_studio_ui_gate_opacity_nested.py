"""G2 opacity: Disabled is sticky downward. A paint wants 0.5 when its variant, or any instance on the path from the variant root to it, is an
instance of a `State=Disabled` variant; every other paint wants 1.0. A control draws itself at its own state wherever a composite nests it, so a
Default composite may hold a Disabled control and a Disabled composite still dims everything it holds.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_gate_opacity_nested
"""
import json
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))
from studio_ui_gate_doc import load_doc  # noqa: E402
from studio_ui_gate_fixtures import GREY, REPO, make_doc, node  # noqa: E402
from studio_ui_gate_opacity import check_opacity  # noqa: E402
from test_studio_ui_gate_registry import add_set  # noqa: E402

PANEL = "02 Core/Menu/Panel"
DISABLED_CONTROL, DEFAULT_CONTROL = 22, 21  # the Disabled and Default variants of Button/Icon (set 20)
DEFAULT_PANEL, DISABLED_PANEL = 31, 32  # the Default and Disabled variants of Menu/Panel (set 30)


def paint(opacity=1.0):
    return {"kind": {"Solid": GREY}, "opacity": opacity, "visible": True, "blend": "Normal"}


def text(nid, name, parent, opacity):
    return node(nid, name, {"Text": {"content": "x", "style": {"fills": [paint(opacity)]}}}, parent=parent)


class StickyDisabledTest(unittest.TestCase):
    def setUp(self):
        self.doc = make_doc()
        add_set(self.doc, 20, "Button/Icon", {"State": ["Default", "Disabled"]})
        add_set(self.doc, 30, "Menu/Panel", {"State": ["Default", "Disabled"]})

    def nest(self, panel, control, root_opacity, label_opacity, glyph_opacity, nid=100):
        """An instance of `control` named `Button` inside variant `panel`: its own frame, a Label and a Glyph below it carry the given paint opacities."""
        nodes = self.doc["nodes"]
        nodes[str(nid)] = node(nid, "Button", {"Instance": {"frame": {}, "component": control, "component_key": "k"}}, parent=panel,
                               children=[nid + 1, nid + 2], fills=[paint(root_opacity)])
        nodes[str(nid + 1)] = text(nid + 1, "Label", nid, label_opacity)
        nodes[str(nid + 2)] = node(nid + 2, "Glyph", "Vector", parent=nid, fills=[paint(glyph_opacity)])
        nodes[str(panel)]["kind"] = {"Component": {}}
        nodes[str(panel)].setdefault("children", []).append(nid)

    def check(self):
        return check_opacity(self.doc)[0]

    def test_a_default_composite_nesting_a_disabled_control_passes(self):
        """The instance's own frame, its label and its glyph are all dimmed (0.5); the composite's own paints and a sibling label are not (1.0)."""
        self.nest(DEFAULT_PANEL, DISABLED_CONTROL, 0.5, 0.5, 0.5)
        self.doc["nodes"][str(DEFAULT_PANEL)]["fills"] = [paint(1.0)]
        self.doc["nodes"]["150"] = text(150, "Title", DEFAULT_PANEL, 1.0)
        self.doc["nodes"][str(DEFAULT_PANEL)]["children"].append(150)
        self.assertEqual(self.check(), [])

    def test_the_nested_disabled_control_is_dimmed_only_below_its_own_root(self):
        """Sticky is downward: the composite's own paint and a sibling of the nested control stay at 1, so a 0.5 there still fails."""
        self.nest(DEFAULT_PANEL, DISABLED_CONTROL, 0.5, 0.5, 0.5)
        self.doc["nodes"][str(DEFAULT_PANEL)]["fills"] = [paint(0.5)]
        self.doc["nodes"]["150"] = text(150, "Title", DEFAULT_PANEL, 0.5)
        self.doc["nodes"][str(DEFAULT_PANEL)]["children"].append(150)
        self.assertEqual(sorted(f.split(": ")[0] for f in self.check()), [f"{PANEL}/State=Default", f"{PANEL}/State=Default/Title"])
        self.assertTrue(all("a non-Disabled variant carries 1" in f for f in self.check()))

    def test_a_glyph_at_full_opacity_inside_the_nested_disabled_control_fails(self):
        self.nest(DEFAULT_PANEL, DISABLED_CONTROL, 0.5, 0.5, 1.0)
        self.assertEqual(self.check(), [f"{PANEL}/State=Default/Button/Glyph: fills/0 has opacity 1, a paint inside an instance of a Disabled variant carries 0.5 on every visible paint"])

    def test_the_nested_disabled_instance_root_and_label_are_read_too(self):
        self.nest(DEFAULT_PANEL, DISABLED_CONTROL, 1.0, 1.0, 0.5)
        self.assertEqual(sorted(f.split(": ")[0] for f in self.check()), [f"{PANEL}/State=Default/Button", f"{PANEL}/State=Default/Button/Label"])

    def test_a_disabled_composite_nesting_a_default_control_still_fails(self):
        self.nest(DISABLED_PANEL, DEFAULT_CONTROL, 1.0, 1.0, 1.0)
        fails = self.check()
        self.assertEqual(sorted(f.split(": ")[0] for f in fails),
                         [f"{PANEL}/State=Disabled/Button", f"{PANEL}/State=Disabled/Button/Glyph", f"{PANEL}/State=Disabled/Button/Label"])
        self.assertTrue(all("a Disabled variant carries 0.5" in f for f in fails))

    def test_a_disabled_composite_nesting_a_disabled_control_passes(self):
        self.nest(DISABLED_PANEL, DISABLED_CONTROL, 0.5, 0.5, 0.5)
        self.assertEqual(self.check(), [])

    def test_a_default_composite_nesting_a_default_control_passes_at_full_opacity_and_fails_dimmed(self):
        self.nest(DEFAULT_PANEL, DEFAULT_CONTROL, 1.0, 1.0, 1.0)
        self.assertEqual(self.check(), [])
        self.doc["nodes"]["102"]["fills"] = [paint(0.5)]
        self.assertEqual(len(self.check()), 1)

    def test_disabled_reaches_through_two_levels_of_nesting(self):
        """A Default composite nests a Disabled composite, which nests a Default control: every paint under the Disabled composite wants 0.5."""
        add_set(self.doc, 60, "Menu/Sheet", {"State": ["Default"]})  # variant 61
        self.nest(DISABLED_PANEL, DEFAULT_CONTROL, 0.5, 0.5, 0.5)
        n = self.doc["nodes"]
        n["170"] = node(170, "Inner", {"Instance": {"frame": {}, "component": DISABLED_PANEL, "component_key": "k"}}, parent=61, children=[171], fills=[paint(0.5)])
        n["171"] = text(171, "Label", 170, 1.0)
        n["61"]["kind"], n["61"]["children"] = {"Component": {}}, [170]
        self.assertEqual(self.check(), [f"02 Core/Menu/Sheet/State=Default/Inner/Label: fills/0 has opacity 1, a paint inside an instance of a Disabled variant carries 0.5 on every visible paint"])

    def test_an_instance_of_a_node_that_is_not_a_variant_is_not_disabled(self):
        """A dangling or unrelated component id never makes a paint dim: the instance of the 03 Studio page node here reads as non-Disabled."""
        self.nest(DEFAULT_PANEL, 9, 1.0, 1.0, 1.0)
        self.assertEqual(self.check(), [])
        self.doc["nodes"]["100"]["kind"]["Instance"]["component"] = 999
        self.assertEqual(self.check(), [])

    def test_hidden_layers_inside_the_nested_disabled_control_are_read(self):
        self.nest(DEFAULT_PANEL, DISABLED_CONTROL, 0.5, 0.5, 1.0)
        self.doc["nodes"]["102"]["visible"] = False
        self.assertEqual(len(self.check()), 1)

    def nest_icon_slot(self, glyph_opacity):
        """The shape of a swapped icon glyph (designos #1427): `Button`, an instance of the Disabled Button/Icon, holds the `Icon` slot, an instance of the plain component Icon/Plus
        (not a variant, so not Disabled itself), which holds the `Glyph`. The Glyph sits two instance levels below the Disabled one, inside the Default variant of Menu/Panel."""
        n = self.doc["nodes"]
        n["80"] = node(80, "Icon/Plus", {"Component": {}}, parent=3, children=[81])
        n["81"] = node(81, "Glyph", "Vector", parent=80, fills=[paint(1.0)])  # the Icon main is a plain component and is read as a non-Disabled variant: its own glyph is 1
        n["3"]["children"].append(80)
        n["100"] = node(100, "Button", {"Instance": {"frame": {}, "component": DISABLED_CONTROL, "component_key": "k"}}, parent=DEFAULT_PANEL, children=[101], fills=[paint(0.5)])
        n["101"] = node(101, "Icon", {"Instance": {"frame": {}, "component": 80, "component_key": "k"}}, parent=100, children=[102])
        n["102"] = node(102, "Glyph", "Vector", parent=101, fills=[paint(glyph_opacity)])
        n[str(DEFAULT_PANEL)]["kind"], n[str(DEFAULT_PANEL)]["children"] = {"Component": {}}, [100]

    def test_a_glyph_two_instance_levels_below_a_disabled_instance_wants_half(self):
        """Sticky reaches the glyph through a slot instance that is not a Disabled variant itself: at 0.5 it passes, at 1.0 it fails and only it is named."""
        self.nest_icon_slot(0.5)
        self.assertEqual(self.check(), [])
        self.doc["nodes"]["102"]["fills"] = [paint(1.0)]
        self.assertEqual(self.check(), [f"{PANEL}/State=Default/Button/Icon/Glyph: fills/0 has opacity 1, a paint inside an instance of a Disabled variant carries 0.5 on every visible paint"])


class CommittedFileTest(unittest.TestCase):
    def test_the_committed_file_passes_with_disabled_sticky_and_the_listed_opacities(self):
        doc = load_doc(REPO / "design/studio-ui.designos")
        allow = json.loads((REPO / "design/studio-ui.gate-allowlist.json").read_text(encoding="utf-8"))["allow"]
        fails, counts = check_opacity(doc, allow)
        self.assertEqual(fails, [])
        self.assertGreater(counts["disabled"], 0)
        # the 0.22 / 0.16 dev-ready tints (the Status pill of DevInspect Inspect and Annotate, the dev list's count badge, mark button and 15 rows)
        # and the two Type settings guide lines
        self.assertEqual(counts["listed"], 21)

    def test_without_its_entries_the_only_opacities_the_committed_file_does_not_hold_are_the_dev_ready_tints_and_the_type_guides(self):
        fails, _ = check_opacity(load_doc(REPO / "design/studio-ui.designos"))
        dev = "03 Studio/Regions/Panel/DevInspect/"
        want = [dev + "State=Inspect/Status pill", dev + "State=Annotate/Status pill"]
        want += [dev + "State=List/left-dev-ready-section/Ready count", dev + "State=List/left-dev-ready-section/left-dev-mark-ready"]
        want += [dev + f"State=List/Row {i}/Status pill" for i in range(1, 16)]
        want += [f"03 Studio/Menus & Popovers/Popover/Panel/State=Default, Kind=TypeSettings/Preview/{guide}" for guide in ("Cap height", "Baseline")]
        self.assertEqual(sorted(f.split(": ")[0] for f in fails), sorted(want))
        self.assertTrue(all("a non-Disabled variant carries 1 on every visible paint" in f for f in fails))


if __name__ == "__main__":
    unittest.main()
