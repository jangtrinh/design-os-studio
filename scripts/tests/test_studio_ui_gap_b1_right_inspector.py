"""The seven states of Panel/RightInspector (components/regions_editor_right*.py), read from the IR the builder returns against the real Semantic plan: the axes of the registry, the shell per
state, the section order, the places of the rows (the numbers are `pane.y` of properties.rs checked against the named rects of the 0.7.5 captures, quoted per test), bound gaps and paddings.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_gap_b1_right_inspector
"""
import json
import re
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from regions_editor_fixtures import DENY, ENV, REPO, SPECS, STYLES, build, by_name, rect  # noqa: E402
from rest_ir import walk  # noqa: E402

STATES = ["Page", "Frame", "Layers", "Text", "Instance", "Prototype", "Dev"]
VALUES = {v["name"]: v["value"] for v in ENV.by_const.values() if isinstance(v["value"], (int, float))}
MODULES = ("regions_editor_right", "regions_editor_right_parts", "regions_editor_right_sections", "regions_editor_right_paints", "regions_editor_right_states",
           "regions_editor_right_text_instance", "regions_editor_right_proto")


def state(name):
    return build("Panel/RightInspector", State=name)


def content(name):
    return by_name(state(name))["Content"]


def height(node):
    """The height of a node in a column: its own `h` when fixed, else its visible in-flow children plus gaps and pads (a caption text is one 18 px line)."""
    if node["t"] == "T":
        return 18.0
    if node.get("sy") == "fixed":
        return node["h"]
    kids = [k for k in node["kids"] if not k.get("hidden") and not k.get("abs")]
    gap = VALUES[node["gap"]] if node.get("gap") else 0.0
    pad = [VALUES[p] if p else 0.0 for p in (node.get("pad") or [None] * 4)]
    return sum(height(k) for k in kids) + gap * max(len(kids) - 1, 0) + pad[0] + pad[2]


def top(root, path):
    """The y of a node below `root`, by names joined with `/`, counting the flow of every column on the way (children of a fixed frame sit at their `abs`)."""
    y, node = 0.0, root
    for name in path.split("/"):
        gap = VALUES[node["gap"]] if node.get("gap") else 0.0
        pad = [VALUES[p] if p else 0.0 for p in (node.get("pad") or [None] * 4)]
        flowing, at = node.get("dir") == "V", pad[0]
        for k in node["kids"]:
            if k["name"] == name:
                y += k["abs"][1] if k.get("abs") else (at if flowing else 0.0)
                node = k
                break
            if not k.get("hidden") and not k.get("abs"):
                at += height(k) + gap
        else:
            raise KeyError(f"{name!r} not below {node['name']!r}")
    return y


class AxesTest(unittest.TestCase):
    def test_the_axes_are_the_registrys_with_page_first(self):
        record = next(c for c in json.loads((REPO / "design/component-registry.json").read_text())["components"] if c["name"] == "Panel/RightInspector")
        want = [v.partition("=")[2] for v in record["variants"]]
        self.assertEqual(SPECS["Panel/RightInspector"]["axes"], {"State": want})
        self.assertEqual(want, STATES)
        self.assertEqual(want[0], "Page")

    def test_the_page_state_is_the_default_drawing(self):
        self.assertEqual(json.dumps(build("Panel/RightInspector"), sort_keys=True), json.dumps(state("Page"), sort_keys=True))

    def test_every_state_is_241_by_868_with_the_shell_and_unique_sibling_names(self):
        for name in STATES:
            root = state(name)
            self.assertEqual((root["w"], root["h"]), (241.0, 868.0), name)
            for node in walk(root):
                names = [k["name"] for k in node.get("kids", [])]
                self.assertEqual(len(names), len(set(names)), f"{name}: {node['name']} has two children of one name")
            shell = by_name(root)
            for part in ("right-top-avatar", "present-button", "share-button", "Zoom", "Tabs rule", "Content", "help-button", "Left border"):
                self.assertIn(part, shell, f"{name}: {part}")

    def test_no_default_text_names_a_kit_client_or_library_and_every_style_exists(self):
        for name in STATES:
            nodes = list(walk(state(name)))
            self.assertLessEqual({n["style"] for n in nodes if n["t"] == "T"}, STYLES, name)
            texts = [n["text"] for n in nodes if n["t"] == "T"] + [str(v) for n in nodes if n["t"] == "X" for v in n["props"].values()]
            self.assertEqual([t for t in texts if DENY.search(t)], [], name)


class ShellTest(unittest.TestCase):
    def test_tabs_follow_the_selected_pill_with_the_widths_of_the_captures(self):
        """layout.json right-tab-<label>: Design 56.25 / Prototype 71.5 (d-editor-selected), 55.25 / 73.25 (f-prototype-tab), Inspect 59 / Plugins 57.25 (g-dev-react)."""
        want = {"Frame": (("Design", "Selected", 9.0, 56.25), ("Prototype", "Default", 69.25, 71.5)), "Prototype": (("Design", "Default", 9.0, 55.25), ("Prototype", "Selected", 68.25, 73.25)),
                "Dev": (("Inspect", "Selected", 9.0, 59.0), ("Plugins", "Default", 72.0, 57.25))}
        for name, tabs in want.items():
            shell = by_name(state(name))
            for label, selected, x, w in tabs:
                tab = shell[f"right-tab-{label}"]
                self.assertEqual((tab["variant"], rect(tab)[0], rect(tab)[2]), (f"State={selected}", x, w), f"{name} {label}")

    def test_share_is_green_in_dev_mode_and_blue_otherwise_and_the_prototype_tab_has_no_scrollbar(self):
        self.assertEqual(by_name(state("Dev"))["share-button"]["fill"], "color/success")
        self.assertEqual(by_name(state("Frame"))["share-button"]["fill"], "color/info")
        self.assertNotIn("scrollbar-track", by_name(state("Prototype")))
        self.assertIn("scrollbar-track", by_name(state("Dev")))

    def test_the_content_frame_starts_at_the_panels_outer_left_edge_in_every_selection_state(self):
        """Every x of properties.rs is relative to the panel's outer left edge (`Pane::x0` = region.left - BORDER_WIDTH, :376-379): the Content frame is 241 wide at x 0, below the hairline."""
        for name in ("Frame", "Layers", "Text", "Instance", "Prototype", "Dev"):
            self.assertEqual(content(name)["abs"], (0.0, 81.0, 241.0, 751.0), name)

    def test_the_dev_state_leaves_its_content_to_the_dev_inspector(self):
        self.assertEqual(content("Dev")["kids"], [])


class SectionsTest(unittest.TestCase):
    def test_each_state_lists_the_sections_of_its_painter_in_order(self):
        sections = lambda n: [k["name"] for k in content(n)["kids"] if not k["name"].startswith("Rule")]
        self.assertEqual(sections("Frame"), ["Name row", "Copy row", "Position section", "Auto layout section", "Appearance section"])
        self.assertEqual(sections("Layers"), ["Name row", "Position section", "Layout section", "Appearance section", "Fill section", "Stroke section", "Effects section",
                                              "Selection colors section"])
        self.assertEqual(sections("Text"), ["Name row", "Position section", "Layout section", "Typography section", "Appearance section"])
        self.assertEqual(sections("Instance"), ["Name row", "Copy row", "Properties section", "Position section"])
        self.assertEqual(sections("Prototype"), ["Interactions section", "Scroll behavior section"])

    def test_no_part_of_any_state_is_hidden_by_default(self):
        """A part a screen never shows would be stored in every instance of the region (size budget): the sections are drawn or absent, never hidden alternates."""
        for name in STATES:
            self.assertEqual([n["name"] for n in walk(state(name)) if n.get("hidden")], [], name)

    def test_frame_rows_sit_where_properties_rs_puts_them(self):
        """d-editor-selected layout.json, content top 113: copy-to-slides y 165, distribute 260, xform:0 392, al-upgrade-v1 799 (184 x 24)."""
        c = content("Frame")
        self.assertEqual(top(c, "Copy row"), 165 - 113)
        self.assertEqual(top(c, "Position section/Position body/Alignment group/Alignment row"), 260 - 113)
        self.assertEqual(top(c, "Position section/Position body/Rotation group/Rotation row"), 392 - 113)
        row = top(c, "Auto layout section/Auto layout body/Alignment group/Update row")
        self.assertEqual(row + 4.0, 799 - 113)

    def test_layers_rows_sit_where_the_capture_puts_them(self):
        """micro-smart-selection layout.json: distribute 228, xform:0 360, the fill swatch 637 (8 under its row, centred), fx-add 727 (8 under its title), selsw:0 804 (4 under its row)."""
        c = content("Layers")
        self.assertEqual(top(c, "Position section/Position body/Alignment group/Alignment row"), 228 - 113)
        self.assertEqual(top(c, "Position section/Position body/Rotation group/Rotation row"), 360 - 113)
        self.assertEqual(top(c, "Fill section/Fill body/Fill rows/Fill 1 row"), 637 - 8 - 113)
        self.assertEqual(top(c, "Effects section/Effects title") + 8.0, 727 - 113)
        self.assertEqual(top(c, "Selection colors section/Selection colors body/Colour rows/Colour 1 row"), 804 - 4 - 113)

    def test_instance_and_prototype_rows_sit_where_the_capture_puts_them(self):
        """kit-editor: copy 165, prop-add 206 (8 under the title), propdef:Actions 238, Breadcrumbs 314, distribute 833. f-prototype-tab: proto-show-settings 153, proto-row-0 187, proto-
        overflow 300, proto-position 370."""
        c = content("Instance")
        self.assertEqual(top(c, "Copy row"), 165 - 113)
        self.assertEqual(top(c, "Properties section/Properties title") + 8.0, 206 - 113)
        self.assertEqual([top(c, f"Properties section/Properties body/Property rows/Property {n}") for n in (1, 2)], [238 - 113, 314 - 113])
        self.assertEqual(top(c, "Position section/Position body/Alignment group/Alignment row"), 833 - 113)
        p = content("Prototype")
        self.assertEqual(top(p, "Interactions section/Interactions body/Settings row"), 153 - 113)
        self.assertEqual(top(p, "Interactions section/Interactions body/Interaction row frame"), 187 - 113)
        self.assertEqual(top(p, "Scroll behavior section/Scroll behavior body/Overflow group/Overflow row"), 300 - 113)
        self.assertEqual(top(p, "Scroll behavior section/Scroll behavior body/Position group/Position row"), 370 - 113)

    def test_the_text_state_places_typography_like_the_capture(self):
        """edit-style-text: text-style-row y 610 (the row follows the title, 40 high), the weight and size row and the line height row 36 apart."""
        c = content("Text")
        base = "Typography section/Typography body/Typography rows/"
        self.assertEqual(top(c, base + "Style row"), 610 - 113)
        self.assertEqual(top(c, base + "Weight row") - top(c, base + "Family row"), 36.0)
        self.assertEqual(top(c, base + "Spacing row") - top(c, base + "Weight row"), 36.0)


class BindingTest(unittest.TestCase):
    def test_every_gap_and_padding_is_a_semantic_variable_or_a_pinned_literal(self):
        for name in STATES:
            for node in walk(state(name)):
                for key in ("gap", "pad"):
                    for entry in ([node.get(key)] if key == "gap" else (node.get(key) or [None] * 4)):
                        ok = entry is None or (isinstance(entry, str) and entry in VALUES) or (isinstance(entry, tuple) and entry[0] == "lit")
                        self.assertTrue(ok, f"{name}: {node['name']} {key} {entry!r} is neither a Semantic variable nor lit(...)")

    def test_the_section_columns_gap_is_the_group_gap_and_the_groups_the_fact_gap(self):
        c = by_name(content("Frame"))
        self.assertEqual((c["Position body"]["gap"], c["Alignment group"]["gap"]), ("space/group", "space/fact"))

    def test_modules_stay_under_200_lines_and_200_characters(self):
        for module in MODULES:
            lines = (REPO / "scripts/studio_ui_file/components" / f"{module}.py").read_text(encoding="utf-8").splitlines()
            self.assertLess(len(lines), 200, module)
            self.assertEqual([n for n, line in enumerate(lines, 1) if len(line) >= 200], [], module)

    def test_no_module_mentions_a_review_label_or_plan_code(self):
        for module in MODULES:
            source = (REPO / "scripts/studio_ui_file/components" / f"{module}.py").read_text(encoding="utf-8")
            self.assertIsNone(re.search(r"\b(?:B[1-5]|R0|F\d{1,2}|merge-\d)\b", source), module)


if __name__ == "__main__":
    unittest.main()
