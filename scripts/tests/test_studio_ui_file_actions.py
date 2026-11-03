"""The W1a component builders against the painters 0.7.5 calls (synthetic env: no theme.rs, kit-states.json or MCP).

Close glyph: `widgets::icon_button` (widgets/mod.rs:144-150). Selection chip: `paint_selection_chip` (agent_selection_chip.rs:161-187).
Loading chip: `layer_row_progress_chip` (image_jobs.rs:1248-1283). Progress row: `progress_row` (panels/check_designs.rs:558-593).

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_actions
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
sys.path.insert(0, str(HERE.parent))
import components  # noqa: E402
import rest_ir as ir  # noqa: E402
import text_styles_plan  # noqa: E402

NUM = {"CONTROL_XS": 32.0, "CONTROL_SM": 36.0, "CONTROL_MD": 40.0, "LAYER_ICON_SIZE": 16.0, "SPACE_FACT": 4.0, "TOOL_BUTTON": 32.0,
       "TOOL_ICON_SIZE": 20.0, "CONTROL_DENSE": 24.0, "BORDER_WIDTH": 1.0, "BADGE_MD": 24.0}


class FakeEnv:
    """Semantic names are the theme constants themselves; numbers come from NUM (1.0 when a test does not care)."""

    def c(self, const):
        return const

    def num(self, const):
        return NUM.get(const, 1.0)

    def look(self, row, state):
        return {"fill": "FILL", "border": "BORDER", "text": "TEXT", "icon": "ICON", "ring": "FOCUS_RING" if state == "Focus" else None,
                "alpha": 0.5 if state == "Disabled" else 1.0}

    def estimate(self, node):
        return ir.estimate(node, mains={})


SETS = {s["name"]: s for s in components.discover()}
STYLES = {f"{role} / {label}" for role, label, *_ in text_styles_plan.ROLES}


def build(name, **values):
    return SETS[name]["build"](values, FakeEnv())


def kids(node, t=None):
    return [k for k in ir.walk(node) if k is not node and (t is None or k["t"] == t)]


class ButtonTest(unittest.TestCase):
    def test_a_close_glyph_is_16_at_every_size_like_icon_button(self):
        for size in ("Xs", "Sm"):
            self.assertEqual([i["size"] for i in kids(build("Button/Close", Size=size, State="Default"), "I")], [16], size)

    def test_an_icon_button_glyph_still_follows_icon_px(self):
        self.assertEqual([i["size"] for i in kids(build("Button/Icon", Size="Sm", State="Default"), "I")], [20])

    def test_every_text_button_label_is_text_sm_semibold(self):
        for kind in ("Primary", "Secondary", "Tertiary", "Destructive", "SecondaryDestructive"):
            label = [t for t in kids(build(f"Button/{kind}", Size="Md", State="Default"), "T")]
            self.assertEqual({t["style"] for t in label}, {"Text sm / Semibold"}, kind)


class SelectionChipTest(unittest.TestCase):
    def test_the_chip_is_a_24_px_pill_without_border_close_glyph_or_ring(self):
        for state in ("Default", "Hover", "Focus"):
            chip = build("Tag/Chip", Size="Sm", State=state)
            self.assertEqual((chip["h"], chip["radius"][1]), (24.0, 12.0), state)
            self.assertNotIn("stroke", chip)
            self.assertEqual([k["name"] for k in kids(chip)], ["Label"], state)

    def test_fill_label_style_and_padding_follow_paint_selection_chip(self):
        default, hover = build("Tag/Chip", Size="Sm", State="Default"), build("Tag/Chip", Size="Sm", State="Hover")
        self.assertEqual((default["fill"], hover["fill"]), ("BG_SELECTED", "BG_SELECTED_HOVER"))
        label = kids(default, "T")[0]
        self.assertEqual((label["style"], label["color"]), ("Caption / Regular", "TEXT"))
        self.assertEqual(ir.pads(default), [None, "INSET_CONTROL", None, "INSET_CONTROL"])

    def test_focus_has_the_default_look_because_the_chip_paints_no_ring(self):
        self.assertEqual(build("Tag/Chip", Size="Sm", State="Focus"), build("Tag/Chip", Size="Sm", State="Default"))
        self.assertNotIn("Focus/Ring", SETS["Tag/Chip"].get("uses", []))


class LoadingChipTest(unittest.TestCase):
    def test_the_chip_is_a_20_px_pill_with_a_label_and_a_cancel_action_and_no_ring(self):
        chip = build("Loading/Inline", State="Default")
        self.assertEqual((chip["h"], chip["radius"][1], chip["fill"], chip["stroke"]), (20.0, 10.0, "BG_SECONDARY", "BORDER"))
        self.assertEqual([k["name"] for k in chip["kids"]], ["Label", "Cancel"])
        self.assertEqual([k["t"] for k in kids(chip)], ["T", "F", "T"], "a ring would add an ellipse or a vector")
        self.assertEqual({t["color"] for t in kids(chip, "T")}, {"TEXT_SECONDARY"})

    def test_the_label_starts_at_6_and_the_cancel_pill_ends_inset_segment_from_the_edge(self):
        chip = build("Loading/Inline", State="Default")
        self.assertEqual((ir.pads(chip)[3][1], ir.pads(chip)[1]), (6.0, "INSET_SEGMENT"))
        self.assertEqual(chip["gap"][1], 12.0)


class ProgressRowTest(unittest.TestCase):
    def test_the_label_sits_below_the_bar_in_a_296_px_row(self):
        row = build("Progress/Bar", State="Default")
        self.assertEqual((row["w"], row["h"]), (296, 32))
        parts = {k["name"]: k for k in row["kids"]}
        self.assertEqual(parts["Track"]["abs"], (0, 4, 296, 8))
        self.assertEqual(parts["Label"]["abs"][1], 15)
        self.assertGreaterEqual(parts["Label"]["abs"][1], parts["Track"]["abs"][1] + parts["Track"]["abs"][3])

    def test_the_counts_are_monospace_text_and_the_words_caption_secondary(self):
        texts = {t["name"]: t for t in kids(build("Progress/Bar", State="Default"), "T")}
        self.assertEqual({texts[n]["style"] for n in ("Scanned", "Total")}, {"Mono / Caption"},
                         "the Rust sets the counts in FontId::new(FONT_SIZE_CAPTION, Monospace) (panels/check_designs.rs:569), not font_mono (BODY)")
        self.assertEqual({texts[n]["color"] for n in ("Scanned", "Total")}, {"TEXT"})
        self.assertEqual({texts[n]["style"] for n in ("Prefix", "Of", "Unit")}, {"Caption / Regular"})
        self.assertEqual({texts[n]["color"] for n in ("Prefix", "Of", "Unit")}, {"TEXT_SECONDARY"})


class TileTest(unittest.TestCase):
    def glyph(self, kind, size):
        return kids(build("Icon/Tile", Kind=kind, Size=size), "I")[0]["size"]

    def test_glyph_sizes_follow_the_painters(self):
        self.assertEqual([self.glyph("Design", "Sm"), self.glyph("Design", "Md"), self.glyph("Neutral", "Md")], [12, 17, 16])


class StartHereTest(unittest.TestCase):
    """The component rules the gate enforces are written on page `00 Start Here`."""

    def test_the_opacity_glyph_size_and_ring_radius_rules_are_on_the_page(self):
        import start_here
        plan = {"semantic": [], "primitives": []}
        words = " ".join(n["text"] for n in start_here.start_here(plan, 117, 18)["children"])
        for rule in ("half alpha per paint", "opacity 0.5", "no opacity/disabled variable", "Icon/Glyph Size Sm = 16 = LAYER_ICON_SIZE",
                     "Button/Icon Size Sm carries a 20 px glyph", "a radius binding on a ring instance is forbidden", "ignores a uniform cornerRadius binding",
                     "Focus/Ring radius on a non-12 host is a pinned literal (2026-10-31): the engine cannot bind a sum.",
                     "A ring rings the node it sits in, its host", "grown by SPACE_FACT on every side, at (-4, -4)", "round(parent radius + SPACE_FACT)",
                     "the gate reads the host from the ring's parent, never from the allow-list",
                     "A checkbox rings its 16 px box, not its control rect (kit/choice.rs:104-111): a checkbox variant makes the box the ring's parent."):
            self.assertIn(rule.lower(), words.lower(), rule)
        self.assertNotIn("variable + SPACE_FACT", words, "the superseded rule: a ring radius bound to the host's variable")
        self.assertNotIn("names the host constant", words, "the superseded rule: an allow-list entry that names its host")


class EveryStyleExistsTest(unittest.TestCase):
    def test_every_text_a_component_draws_uses_a_defined_text_style(self):
        for name, spec in SETS.items():
            for values in components.matrix(spec["axes"]):
                for t in kids(spec["build"](values, FakeEnv()), "T"):
                    self.assertIn(t["style"], STYLES, f"{name} {values}")


if __name__ == "__main__":
    unittest.main()
