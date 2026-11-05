"""Input/Field, Input/Search, Input/PropertyField and Select/Trigger against the painters 0.7.5 calls (synthetic env: no theme.rs or MCP; the looks
come from the real kit-states.json).

Field chrome: `kit::paint_field` (widgets/kit/field.rs:43-50), look `field_look` / `search_look` / `select_look` (field.rs:7-40): BG fill, BORDER_STRONG 1 px,
Focus = BORDER_SELECTED 2 px, no hover stroke and no ring. Search: find/filter.rs:75-118 (Xs) and home/mod.rs:940-960 (Sm). Property field: `num_field`
(widgets/field.rs:563-644). Select: `dropdown_items` (widgets/menu.rs:598-627).

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_forms_fields
"""
import json
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
sys.path.insert(0, str(HERE.parent))
import components  # noqa: E402
import rest_ir as ir  # noqa: E402
from components.env import KIT_STATE, KIT_STATES_REL  # noqa: E402

ROWS = json.loads((HERE.parents[1] / KIT_STATES_REL).read_text(encoding="utf-8"))["rows"]
NUM = {"CONTROL_XS": 32.0, "CONTROL_SM": 36.0, "CONTROL_MD": 40.0, "FIELD_HEIGHT": 32.0, "FIELD_PREFIX_W": 24.0, "FIELD_PREFIX_X": 8.0, "TARGET_MIN": 24.0}


class FakeEnv:
    def c(self, const):
        return const

    def num(self, const):
        return NUM.get(const, 1.0)

    def look(self, row, state):
        cell = ROWS[row][KIT_STATE[state]]
        out = {k: cell.get(k) for k in ("fill", "border", "text", "icon", "ring")}
        out["alpha"] = cell["alpha"]
        return out


SETS = {s["name"]: s for s in components.discover()}
STATES = ("Default", "Hover", "Focus", "Disabled")


def build(name, **values):
    return SETS[name]["build"](values, FakeEnv())


def named(node, name):
    return [k for k in ir.walk(node) if k["name"] == name]


class ChromeTest(unittest.TestCase):
    """Every field set shares the kit chrome."""

    CASES = (("Input/Field", {"Size": "Xs"}), ("Input/Search", {"Size": "Sm"}), ("Input/PropertyField", {"Size": "Xs", "Axis": "Scrub"}), ("Select/Trigger", {"Size": "Xs"}))

    def test_the_sets_are_the_registry_matrices_in_the_forms_row(self):
        for name, n in (("Input/Field", 8), ("Input/Search", 12), ("Input/PropertyField", 4), ("Select/Trigger", 10)):
            self.assertEqual((SETS[name]["row"], len(components.matrix(SETS[name]["axes"]))), ("Forms & Inputs", n), name)

    def test_default_is_bg_with_a_1_px_strong_border_and_the_control_radius(self):
        for name, values in self.CASES:
            node = build(name, State="Default", **values)
            self.assertEqual((node["fill"], node["stroke"], node["sw"], node["radius"], node.get("alpha", 1.0)), ("BG", "BORDER_STRONG", "BORDER_WIDTH", "RADIUS_CONTROL", 1.0), name)

    def test_there_is_no_hover_stroke(self):
        for name, values in self.CASES:
            self.assertEqual(build(name, State="Hover", **values), build(name, State="Default", **values), name)

    def test_focus_is_a_2_px_selected_border_pinned_as_a_literal_and_no_ring(self):
        for name, values in self.CASES:
            state = "Open" if name == "Select/Trigger" else "Focus"
            node = build(name, State=state, **values)
            self.assertEqual((node["stroke"], node["sw"][:2]), ("BORDER_SELECTED", ("lit", 2.0)), name)
            self.assertEqual([k for k in ir.walk(node) if k["t"] == "X"], [], name)

    def test_disabled_is_the_default_at_half_alpha_on_every_paint(self):
        for name, values in self.CASES:
            node = build(name, State="Disabled", **values)
            self.assertEqual(node["alpha"], 0.5, name)
            self.assertEqual({k.get("alpha") for k in ir.walk(node) if k["t"] in ("T", "I")}, {0.5}, name)
            stripped = lambda n: {**n, "alpha": None, "kids": []}
            self.assertEqual(stripped(node), stripped(build(name, State="Default", **values)), name)


class InputFieldTest(unittest.TestCase):
    def test_xs_is_32_high_with_body_text_8_in_and_sm_is_36_with_text_sm_10_in(self):
        xs, sm = build("Input/Field", Size="Xs", State="Default"), build("Input/Field", Size="Sm", State="Default")
        self.assertEqual((xs["h"], sm["h"]), (32.0, 36.0))
        self.assertEqual((ir.pads(xs)[3], named(xs, "Value")[0]["style"]), ("FIELD_PLAIN_X", "Body / Regular"))
        self.assertEqual((ir.pads(sm)[3][:2], named(sm, "Value")[0]["style"]), (("lit", 10.0), "Text sm / Regular"))

    def test_value_is_text_and_the_placeholder_is_a_hidden_part(self):
        node = build("Input/Field", Size="Xs", State="Default")
        value, hint = named(node, "Value")[0], named(node, "Placeholder")[0]
        self.assertEqual((value["color"], value["prop"], value.get("visible_prop")), ("TEXT", "Value", "Has Value"))
        self.assertEqual((hint["prop"], hint["visible_prop"], hint.get("hidden")), ("Placeholder", "Has Placeholder", True))

    def test_the_xs_placeholder_is_text_tertiary_the_colour_text_input_ph_paints(self):
        """`text_input_ph` -> `edit_text_hint` paints the hint itself in TEXT_TERTIARY (widgets/field.rs:711-716, 98-101), not through egui's weak colour."""
        for state in STATES:
            self.assertEqual(named(build("Input/Field", Size="Xs", State=state), "Placeholder")[0]["color"], "TEXT_TERTIARY", state)

    def test_the_sm_placeholder_is_text_placeholder_the_colour_the_dialog_field_sets_as_the_weak_colour(self):
        """home/dialog.rs:308-311 sets `weak_text_color` to TEXT_PLACEHOLDER for its TextEdit, so egui draws the hint in it."""
        for state in STATES:
            self.assertEqual(named(build("Input/Field", Size="Sm", State=state), "Placeholder")[0]["color"], "TEXT_PLACEHOLDER", state)


class InputSearchTest(unittest.TestCase):
    def test_heights_are_the_three_control_steps(self):
        self.assertEqual([build("Input/Search", Size=s, State="Default")["h"] for s in ("Xs", "Sm", "Md")], [32.0, 36.0, 40.0])

    def test_the_glyph_is_14_px_centred_at_14_and_the_text_starts_at_26_xs_or_28_sm(self):
        """filter.rs:75-80 (SEARCH_ICON_DX 14, SEARCH_TEXT_DX 26) and home/mod.rs:943-945 (14, 28): 2 + a 24 px slot centres the glyph at 14."""
        for size, text_x in (("Xs", 26), ("Sm", 28)):
            node = build("Input/Search", Size=size, State="Default")
            slot = node["kids"][0]
            glyph = named(node, "Icon")[0]
            gap = 0 if node.get("gap") is None else 2
            self.assertEqual((ir.pads(node)[3], slot["w"], glyph["size"], glyph["color"]), ("INSET_SEGMENT", 24.0, 14, "ICON_QUATERNARY"), size)
            self.assertEqual((2 + slot["w"] / 2, 2 + slot["w"] + gap), (14, text_x), size)

    def test_text_is_the_placeholder_until_focus_and_the_style_follows_the_size(self):
        self.assertEqual(named(build("Input/Search", Size="Sm", State="Default"), "Value")[0]["color"], "TEXT_PLACEHOLDER")
        self.assertEqual(named(build("Input/Search", Size="Sm", State="Focus"), "Value")[0]["color"], "TEXT")
        self.assertEqual(named(build("Input/Search", Size="Xs", State="Default"), "Value")[0]["style"], "Body / Regular")
        self.assertEqual(named(build("Input/Search", Size="Sm", State="Default"), "Value")[0]["style"], "Text sm / Regular")

    def test_the_clear_control_is_a_24_box_with_a_12_px_glyph_and_off_by_default(self):
        box = named(build("Input/Search", Size="Xs", State="Default"), "Clear box")[0]
        self.assertEqual((box["w"], box["kids"][0]["size"], box["visible_prop"], box.get("hidden")), (24.0, 12, "Has Clear", True))


class PropertyFieldTest(unittest.TestCase):
    def test_prefix_at_8_and_value_at_24_inside_an_88_by_32_field(self):
        node = build("Input/PropertyField", Size="Xs", Axis="Scrub", State="Default")
        slot = named(node, "Prefix slot")[0]
        self.assertEqual((node["w"], node["h"], ir.pads(node)[3], slot["w"]), (88, 32.0, "FIELD_PREFIX_X", 16.0))
        self.assertEqual(8 + slot["w"], 24)
        prefix, value = named(node, "Prefix")[0], named(node, "Value")[0]
        self.assertEqual((prefix["color"], prefix["style"], value["color"], value["style"]), ("TEXT_SECONDARY", "Body / Regular", "TEXT", "Body / Regular"))

    def test_the_prefix_is_not_the_unused_property_field_look_colour(self):
        """`property_field_look` (TEXT_TERTIARY prefix) has 0 callers; `num_field` paints the prefix TEXT_SECONDARY (widgets/field.rs:578-602)."""
        node = build("Input/PropertyField", Size="Xs", Axis="Scrub", State="Default")
        self.assertNotIn("TEXT_TERTIARY", {k.get("color") for k in ir.walk(node)})

    def test_the_suffix_is_a_hidden_text_secondary_part(self):
        suffix = named(build("Input/PropertyField", Size="Xs", Axis="Scrub", State="Default"), "Suffix")[0]
        self.assertEqual((suffix["color"], suffix["visible_prop"], suffix.get("hidden")), ("TEXT_SECONDARY", "Has Suffix", True))


class SelectTriggerTest(unittest.TestCase):
    def test_label_is_body_medium_8_in_and_the_chevron_is_12_px_centred_12_from_the_right(self):
        node = build("Select/Trigger", Size="Xs", State="Default")
        label, chevron = named(node, "Label")[0], named(node, "Chevron")[0]
        self.assertEqual((label["style"], label["color"], label["sx"], ir.pads(node)[3]), ("Body / Medium", "TEXT", "fill", "INSET_CONTROL"))
        self.assertEqual((chevron["icon"], chevron["size"], chevron["color"], ir.pads(node)[1][:2]), ("ChevronDown", 12, "ICON_QUATERNARY", ("lit", 6.0)))
        self.assertEqual(6 + 12 / 2, 12)

    def test_open_focus_and_the_pointer_free_states_follow_focused_equals_open_or_focus(self):
        """menu.rs:603: `focused = open || has_focus`, so Open and Focus are one look; there is no hover arm and no disabled path."""
        self.assertEqual(build("Select/Trigger", Size="Xs", State="Open"), build("Select/Trigger", Size="Xs", State="Focus"))
        self.assertEqual(build("Select/Trigger", Size="Xs", State="Hover"), build("Select/Trigger", Size="Xs", State="Default"))

    def test_sizes_are_the_control_steps(self):
        self.assertEqual([build("Select/Trigger", Size=s, State="Default")["h"] for s in ("Xs", "Sm")], [32.0, 36.0])


class TwoModulesOneRowTest(unittest.TestCase):
    """The assembler groups specs by `row`, not by module, so a second Forms & Inputs module needs no assembler change: a section holds both modules' sets.
    This was meant to be a red test showing the need; it is green at the base commit, so the need is not present."""

    def test_sets_of_two_modules_in_one_row_share_one_section(self):
        import library_core
        import variables_plan
        import text_styles_plan
        import icon_lib
        from unittest import mock
        repo = HERE.parents[1]
        plan = variables_plan.build_plan(str(repo))
        styles, _ = text_styles_plan.build(plan)
        specs = [dict(SETS["Input/Field"], module="forms"), dict(SETS["Input/Search"], module="forms_fields")]
        with mock.patch.object(components, "discover", return_value=specs):
            core = library_core.build_core(str(repo), plan, styles, icon_lib.IconLib(str(repo)))
        self.assertEqual([s["name"] for s in core["info"]], ["Input/Field", "Input/Search"])
        self.assertEqual({s["section"] for s in core["info"]}, {"sec:Forms & Inputs"})
        self.assertEqual([c["name"] for c in core["children"]], ["Forms & Inputs"])


if __name__ == "__main__":
    unittest.main()
