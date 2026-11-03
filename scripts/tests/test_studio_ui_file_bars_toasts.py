"""Bar/Contextual and Toast/Default against the painters 0.7.5 calls (synthetic env: names are the theme constants; looks from the real kit-states.json).

Bar: `panels/toolbar.rs:149-211` `contextual_bar` (`paint_frame` at `RADIUS_XL` in `BG_TOOLBAR` with an `INNER_HIGHLIGHT` edge, `CONTROL_MD` high, two kit Icon buttons, a `BORDER` vline, a
Tertiary-look "More" with a trailing chevron). Toast: `shell.rs:1228-1324` `toast` and `shell/toast_layout.rs` (kit `toast_surface`, `PAD_LEFT` 28, `PAD_RIGHT` 16, `ACTION_GAP` 12,
`ACTION_PAD` 8, `DOT_INSET` 16, `toast_action_look`).

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_bars_toasts
"""
import json
import re
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
sys.path.insert(0, str(HERE.parent))
import components  # noqa: E402
import rest_ir as ir  # noqa: E402
import text_styles_plan  # noqa: E402
from components.env import KIT_STATE, KIT_STATES_REL  # noqa: E402

ROWS = json.loads((HERE.parents[1] / KIT_STATES_REL).read_text(encoding="utf-8"))["rows"]
NUM = {"CONTROL_MD": 40.0, "CONTROL_XS": 32.0, "BORDER_WIDTH": 1.0, "LAYER_ICON_SIZE": 16.0, "TOAST_HEIGHT": 40.0, "SPACE_FACT": 4.0}


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
STYLES = {f"{role} / {label}" for role, label, *_ in text_styles_plan.ROLES}


def build(name, **values):
    return SETS[name]["build"](values, FakeEnv())


def named(node, name):
    return [k for k in ir.walk(node) if k["name"] == name]


def lits(node):
    """Every lit(value, reason) a node of the tree pins in a pad, gap or radius."""
    found = []
    for k in ir.walk(node):
        entries = (ir.pads(k) if k["t"] == "F" else []) + [k.get("gap"), k.get("radius")]
        found += [e for e in entries if isinstance(e, tuple) and e[0] == "lit"]
    return found


class ContextualBarTest(unittest.TestCase):
    def test_the_record_has_no_variants_so_it_is_one_plain_component_nesting_the_icon_button(self):
        s = SETS["Bar/Contextual"]
        self.assertEqual((s["row"], s["axes"], s["uses"]), ("Bars & Feedback", {}, ["Button/Icon"]))

    def test_the_surface_is_the_toolbar_fill_with_an_inner_highlight_edge_at_radius_xl_and_40_high(self):
        bar = build("Bar/Contextual")
        self.assertEqual((bar["fill"], bar["stroke"], bar["sw"], bar["radius"], bar["h"]), ("BG_TOOLBAR", "INNER_HIGHLIGHT", "BORDER_WIDTH", "RADIUS_XL", 40.0))
        self.assertEqual(ir.pads(bar), [None, "SPACE_FACT", None, "SPACE_FACT"], "toolbar.rs:180: 4 in from the left (`+ theme::SPACE_FACT`), ctx_w ends `+ SPACE_FACT` (:165)")
        self.assertEqual(bar["gap"][:2], ("lit", 4.0), "the 4 px frame gap around the line is the raw `cx += 3.0` and `cx += 5.0` of toolbar.rs:196-198, not a theme.rs constant")
        self.assertIn("toolbar.rs:196-198", bar["gap"][2])

    def test_two_xs_icon_buttons_a_step_apart_rectangle_then_pen(self):
        actions = named(build("Bar/Contextual"), "Actions")[0]
        buttons = [k for k in actions["kids"] if k["t"] == "X"]
        self.assertEqual([(b["set"], b["variant"], b["props"]) for b in buttons], [("Button/Icon", "Size=Xs, State=Default", {"Icon": "Rectangle"}),
                                                                                      ("Button/Icon", "Size=Xs, State=Default", {"Icon": "Pen"})])
        self.assertEqual(actions["gap"], "INSET_SEGMENT", "step = CONTROL_XS + INSET_SEGMENT (toolbar.rs:161)")

    def test_the_divider_is_a_local_vertical_border_line_6_inside_each_edge_and_5_after_the_buttons(self):
        bar = build("Bar/Contextual")
        line = named(bar, "Divider")[0]
        self.assertEqual((line["fill"], line["w"], line["h"], line["sx"], line["sy"]), ("BORDER", 1.0, 28.0, "fixed", "fixed"))
        actions = named(bar, "Actions")[0]
        self.assertEqual(ir.pads(actions)[1][1], 1.0, "buttons end at 70; the line is at 75 = 70 + 5 (the 2 px step gap, then `cx += 3`, then 4 + 1 around the line)")
        self.assertEqual([k["name"] for k in bar["kids"]], ["Actions", "Divider", "More"])

    def test_more_is_the_tertiary_look_with_the_label_at_inset_control_and_a_trailing_chevron_down(self):
        more = named(build("Bar/Contextual"), "More")[0]
        self.assertEqual((more["h"], more["radius"], ir.pads(more), more["gap"]), (32.0, "RADIUS_CONTROL", [None, "INSET_CONTROL", None, "INSET_CONTROL"], "SPACE_FACT"))
        label, chevron = named(more, "Label")[0], named(more, "Chevron")[0]
        self.assertEqual((label["style"], label["color"]), ("Text sm / Semibold", "TEXT_TERTIARY"))
        self.assertEqual((chevron["icon"], chevron["size"], chevron["color"]), ("ChevronDown", 16, "ICON_QUATERNARY"))


class ToastTest(unittest.TestCase):
    def test_the_set_is_one_default_variant_nesting_the_status_dot(self):
        s = SETS["Toast/Default"]
        self.assertEqual((s["row"], s["axes"], s["uses"]), ("Bars & Feedback", {"State": ["Default"]}, ["Status/Dot"]))
        self.assertEqual([p["name"] for p in s["props"]], ["Message", "Action", "Has Action"])

    def test_the_surface_is_the_toast_look_at_radius_xl_40_high(self):
        toast = build("Toast/Default", State="Default")
        self.assertEqual((toast["fill"], toast["stroke"], toast["sw"], toast["radius"], toast["h"]), ("BG_PRIMARY_ALT", "BORDER_MENU", "BORDER_WIDTH", "RADIUS_XL", 40.0))

    def test_the_dot_is_an_8_px_info_status_dot_16_in_from_the_left_and_centred_on_the_first_row(self):
        dot = named(build("Toast/Default", State="Default"), "Dot")[0]
        self.assertEqual((dot["set"], dot["variant"], dot["abs"]), ("Status/Dot", "Tone=Info", (12.0, 16.0, 8.0, 8.0)))

    def test_the_message_starts_28_in_and_the_action_sits_12_after_it_with_16_to_the_right_edge(self):
        toast = build("Toast/Default", State="Default")
        pad = ir.pads(toast)
        self.assertEqual((pad[3][1], pad[1][1], toast["gap"][1]), (28.0, 16.0, 12.0))
        self.assertIn("toast_layout.rs:14", pad[3][2])
        self.assertIn("toast_layout.rs:15", pad[1][2])
        self.assertIn("toast_layout.rs:18", toast["gap"][2], "ACTION_GAP is a local constant, SPACE_GROUP only holds the same 12")
        message = named(toast, "Message")[0]
        self.assertEqual((message["color"], message["prop"], message["style"]), ("TEXT", "Message", "Text sm / Medium"))

    def test_the_action_is_its_own_small_frame_in_the_toast_action_look_not_button_tertiary(self):
        toast = build("Toast/Default", State="Default")
        action = named(toast, "Action")[0]
        self.assertEqual((action["h"], action["radius"], action["visible_prop"]), (32.0, "RADIUS_CONTROL", "Has Action"))
        pads = ir.pads(action)
        self.assertEqual([p[:2] if p else None for p in pads], [None, ("lit", 8.0), None, ("lit", 8.0)], "ACTION_PAD is a local constant of toast_layout.rs:20, INSET_CONTROL only holds the same 8")
        self.assertIn("toast_layout.rs:20", pads[1][2])
        label = named(action, "Label")[0]
        self.assertEqual((label["style"], label["color"], label["prop"]), ("Text sm / Semibold", "TEXT_SECONDARY", "Action"))
        self.assertNotIn("fill", action, "toast_action_look: no fill at rest (button.rs:135)")
        self.assertEqual([k for k in ir.walk(toast) if k["t"] == "X" and k["set"] == "Button/Tertiary"], [])


def counter_axis(frame):
    """The vertical alignment an auto-layout H frame gives its in-flow children (the lowerer's default is centre)."""
    assert frame["dir"] == "H", frame["name"]
    return frame.get("align", ("center", "center"))[1]


class VerticalCentringTest(unittest.TestCase):
    """Both rows centre what they hold: the engine, not a padding literal, places the 32 px buttons and the text in the 40 px bars."""

    def test_the_toast_centres_its_message_and_action_on_the_row(self):
        toast = build("Toast/Default", State="Default")
        self.assertEqual(counter_axis(toast), "center", "toast_layout.rs:74 `rect.center().y - galley.size().y / 2` for the message, :78 `rect.center()` for the action")
        flow = [k for k in toast["kids"] if not k.get("abs")]
        self.assertEqual([k["name"] for k in flow], ["Message", "Action"])
        self.assertEqual([k.get("sy", "hug") for k in flow], ["hug", "fixed"], "a fill child would stretch to the row instead of being centred in it")

    def test_the_toast_action_centres_its_own_label(self):
        action = named(build("Toast/Default", State="Default"), "Action")[0]
        self.assertEqual(counter_axis(action), "center", "button.rs paints the label at the target's centre")

    def test_the_bar_centres_its_buttons_divider_and_more_on_the_row(self):
        bar = build("Bar/Contextual")
        self.assertEqual(counter_axis(bar), "center", "toolbar.rs:181 `cy = top + (ctx_h - CONTROL_XS) * 0.5` puts the 32 px buttons and More 4 px down")
        self.assertEqual([k["name"] for k in bar["kids"]], ["Actions", "Divider", "More"])
        self.assertEqual([k.get("abs") for k in bar["kids"]], [None, None, None], "every part is in flow, so the row's centring is what places it")
        self.assertEqual([k.get("sy", "hug") for k in bar["kids"]], ["hug", "fixed", "fixed"])

    def test_the_bar_divider_is_centred_by_the_row_6_px_from_each_edge(self):
        bar = build("Bar/Contextual")
        line = named(bar, "Divider")[0]
        self.assertEqual((bar["h"] - line["h"]) / 2, 6.0, "toolbar.rs:197 `ctx_bar.top() + 6.0 .. ctx_bar.bottom() - 6.0`")
        self.assertEqual((line["sy"], line.get("abs")), ("fixed", None))

    def test_the_bar_actions_group_and_more_centre_their_own_parts(self):
        bar = build("Bar/Contextual")
        for name in ("Actions", "More"):
            self.assertEqual(counter_axis(named(bar, name)[0]), "center", name)


class LiteralsTest(unittest.TestCase):
    def test_every_pinned_literal_names_the_rust_line_it_comes_from(self):
        for name, values in (("Bar/Contextual", {}), ("Toast/Default", {"State": "Default"})):
            found = lits(build(name, **values))
            self.assertTrue(found, name)
            for _, value, reason in found:
                self.assertRegex(reason, r"\w+\.rs:\d+", f"{name}: lit({value})")


class EveryStyleExistsTest(unittest.TestCase):
    def test_every_text_uses_a_defined_text_style(self):
        for name, values in (("Bar/Contextual", {}), ("Toast/Default", {"State": "Default"})):
            for t in (k for k in ir.walk(build(name, **values)) if k["t"] == "T"):
                self.assertIn(t["style"], STYLES, name)


if __name__ == "__main__":
    unittest.main()
