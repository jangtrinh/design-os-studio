"""The per-screen data of Panel/RightInspector (screens/gap_right_panel_data.py): every state of the right panel is covered, every override path resolves in the IR of its variant (a
nested instance through its own set), the pairs the capture shows apart are apart, and `visible` flips what the variant draws the other way round.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_gap_b1_data
"""
import json
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from regions_editor_fixtures import DENY, ENV, REPO, SPECS, build  # noqa: E402

sys.path.insert(0, str(REPO / "scripts/studio_ui_file"))
from screens import gap_right_panel_data as data  # noqa: E402
from screens.gap_table_citations import RIGHT_46  # noqa: E402

DEV = ("g-dev-react", "g-dev-annotate", "g-dev-compare", "micro-dev-boxmodel")
INSPECTOR_12 = ("d-editor-selected", "kit-editor", "e-layers-selected", "fill-d9", "micro-smart-selection", "micro-selection-corner", "micro-scale-tool", "micro-shape-controls",
                "micro-autolayout", "micro-css-grid", "micro-inspector-math", "micro-typography-multiedit")


def children(node):
    """The kids of an IR node; a nested instance has the kids of its own set's variant."""
    if node["t"] == "X":
        return SPECS[node["set"]]["build"](dict(p.split("=") for p in node["variant"].split(", ")), ENV)["kids"]
    return node.get("kids", [])


def resolve(root, path):
    node = root
    for name in path.split("/"):
        found = [k for k in children(node) if k["name"] == name]
        if len(found) != 1:
            return None
        node = found[0]
    return node


def root_of(state):
    return build("Panel/RightInspector", **data.RIGHT_PANEL[state]["variant"])


def key(state):
    return json.dumps(data.RIGHT_PANEL[state], sort_keys=True)


class CoverageTest(unittest.TestCase):
    def test_every_state_of_the_right_panel_and_the_dev_states_is_covered_but_the_undrawn_one(self):
        self.assertEqual(set(RIGHT_46) | set(DEV), set(data.RIGHT_PANEL) | set(data.UNDRAWN))
        self.assertEqual(set(data.UNDRAWN), {"micro-vector-edit"})
        self.assertTrue(set(data.UNDRAWN) <= set(RIGHT_46))
        self.assertTrue(all(data.UNDRAWN.values()))

    def test_every_entry_names_a_state_of_the_registry_and_the_dev_states_the_dev_variant(self):
        states = SPECS["Panel/RightInspector"]["axes"]["State"]
        for name, entry in data.RIGHT_PANEL.items():
            self.assertEqual(list(entry["variant"]), ["State"], name)
            self.assertIn(entry["variant"]["State"], states, name)
        self.assertEqual({data.RIGHT_PANEL[s]["variant"]["State"] for s in DEV}, {"Dev"})

    def test_values_are_strings_flags_or_icons_and_name_no_kit_client_or_library(self):
        for name, entry in data.RIGHT_PANEL.items():
            for o in entry["overrides"]:
                self.assertEqual(sorted(k for k in o if k != "path") in (["text"], ["visible"], ["swap"]), True, f"{name}: {o}")
                if "text" in o:
                    self.assertIsInstance(o["text"], str)
                    self.assertIsNone(DENY.search(o["text"]), f"{name}: {o}")
                if "visible" in o:
                    self.assertIsInstance(o["visible"], bool)
                if "swap" in o:
                    self.assertTrue(o["swap"].startswith("Icon/"))


class PathTest(unittest.TestCase):
    def test_every_override_path_resolves_to_one_node(self):
        for name, entry in data.RIGHT_PANEL.items():
            root = root_of(name)
            for o in entry["overrides"]:
                self.assertIsNotNone(resolve(root, o["path"]), f"{name}: {o['path']}")

    def test_a_text_override_lands_on_text_and_a_swap_on_an_icon(self):
        for name, entry in data.RIGHT_PANEL.items():
            root = root_of(name)
            for o in entry["overrides"]:
                node = resolve(root, o["path"])
                if "text" in o:
                    self.assertEqual(node["t"], "T", f"{name}: {o['path']}")
                if "swap" in o:
                    self.assertEqual(node["t"], "I", f"{name}: {o['path']}")

    def test_visible_flips_the_default(self):
        """A section the capture does not show is visible by default and hidden by the data; a part it shows is hidden by default and shown by the data."""
        for name, entry in data.RIGHT_PANEL.items():
            root = root_of(name)
            for o in entry["overrides"]:
                if "visible" in o:
                    self.assertEqual(bool(resolve(root, o["path"]).get("hidden")), o["visible"], f"{name}: {o}")


class TierTest(unittest.TestCase):
    def test_the_wiring_tiers_partition_the_states(self):
        tiers = [set(states) for _, states in data.WIRING_TIERS]
        self.assertEqual(set.union(*tiers), set(data.RIGHT_PANEL))
        self.assertEqual(sum(len(t) for t in tiers), len(data.RIGHT_PANEL))

    def test_the_second_tier_is_the_inspector_duplicates_a_screen_can_free(self):
        """The twelve of gap table row 2 less fill-d9 (the same masked capture as d-editor-selected), plus micro-components-variables (row 15) and kit-agents (its panel differs from i-agents')."""
        want = (set(INSPECTOR_12) - {"fill-d9"}) | {"micro-components-variables", "kit-agents"}
        self.assertEqual(set(dict(data.WIRING_TIERS)["frees a duplicate"]), want)

    def test_the_first_tier_is_the_dev_and_prototype_states(self):
        self.assertEqual({data.RIGHT_PANEL[s]["variant"]["State"] for s in dict(data.WIRING_TIERS)["smaller file"]}, {"Dev", "Prototype"})


class PairTest(unittest.TestCase):
    def test_the_pairs_the_captures_draw_apart_differ_in_a_value(self):
        for a, b in (("micro-scale-tool", "micro-inspector-math"), ("micro-smart-selection", "micro-selection-corner"), ("micro-components-variables", "k-variables-floating")):
            self.assertNotEqual(key(a), key(b), (a, b))

    def test_the_twelve_inspector_duplicates_draw_apart_but_fill_d9_which_is_d_editor_selected(self):
        self.assertEqual(key("fill-d9"), key("d-editor-selected"))
        keys = [key(s) for s in INSPECTOR_12 if s != "fill-d9"]
        self.assertEqual(len(set(keys)), len(keys))

    def test_the_two_zoom_texts_and_names_are_what_the_captures_show(self):
        zoom = lambda s: [o["text"] for o in data.RIGHT_PANEL[s]["overrides"] if o["path"] == data.ZOOM][0]
        self.assertEqual([zoom(s) for s in ("micro-components-variables", "k-variables-floating", "f-prototype-tab", "micro-dev-boxmodel")], ["115%", "2%", "19%", "140%"])

    def test_the_family_of_states_one_selected_frame_shares_draws_the_same(self):
        for s in ("k-assets", "i-agents-chat", "f-picker", "j-cmdk", "k-variables-floating"):
            self.assertEqual(key(s), key("i-agents"), s)


if __name__ == "__main__":
    unittest.main()
