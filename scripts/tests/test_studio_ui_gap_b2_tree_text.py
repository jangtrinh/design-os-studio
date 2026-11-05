"""A wired layers row never draws its name over its type label, and no row is wider than its 224 px box. Production's rule (panels/layers.rs at v0.7.5: `split_row_text` :235-241,
the row text :646-668) drops the label where the column is too narrow for it plus a readable name and elides the name with `…` to the whole room it has;
`screens/gap_tree_text_fit.py` applies it to the data.

The widths are measured twice: with the table of Inter Regular 12 px advances the data module uses, and, when Pillow and the bundled Inter font are present, with the real font. The constants of
the data module are tied to the Tree/Row variant they describe: its width, chevron box, icon slot and toggles.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_gap_b2_tree_text
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
sys.path.insert(0, str(HERE.parent))
sys.path.insert(0, str(HERE))
from regions_editor_fixtures import ENV, build  # noqa: E402
from screens import gap_left_dev_data as data  # noqa: E402
from screens import gap_tree_text_fit as fit  # noqa: E402

GAP = 4.0  # a name's ink ends at least 4 px before the label's
FONT = REPO / "crates/designos-fonts/assets/inter/Inter[opsz,wght].ttf"
try:
    from PIL import ImageFont
    _FONT = ImageFont.truetype(str(FONT), 12)
    _FONT.set_variation_by_axes([14, 400])
except Exception:  # noqa: BLE001  (Pillow or the font missing: the real-font test is skipped)
    _FONT = None


def wired_rows():
    """(state, row index, depth, name, label) of every row of every wired tree."""
    out = []
    for state, entry in data.LEFT.items():
        for i, row in enumerate(entry["slots"]["Layers"]):
            out.append((state, i, int(row["variant"]["Depth"]), row["props"]["Name"], row["props"]["Type label"]))
    return out


def kid(node, name):
    return [k for k in node["kids"] if k["name"] == name][0]


def px(value):
    """A padding of the IR in px: a literal, the value of the variable it names, or 0."""
    if value is None:
        return 0.0
    if isinstance(value, tuple):
        return value[1]
    return float({v["name"]: v["value"] for v in ENV.by_const.values()}[value])


def content_width(depth, name, label):
    """What a Tree/Row of `depth` needs left to right, read off its IR: the indent, the chevron box, the icon slot and its padding, the name, the type slot (label and padding), the toggles."""
    row = build("Tree/Row", State="Default", Depth=str(depth))
    icon, slot = kid(row, "Icon slot"), kid(row, "Type slot")
    return (px(row["pad"][3]) + kid(row, "Chevron box")["w"] + kid(icon, "Icon")["size"] + px(icon["pad"][1]) + fit.width(name)
            + (fit.width(label) if label else 0) + px(slot["pad"][1]) + kid(row, "Toggles")["w"])


class TextFitRuleTest(unittest.TestCase):
    def test_the_label_is_dropped_where_production_drops_it(self):
        self.assertEqual([fit.shows_label("Auto layout", d) for d in range(6)], [True, True, True, False, False, False])
        self.assertEqual([fit.shows_label("Instance", d) for d in range(6)], [True, True, True, True, False, False])
        self.assertEqual([fit.shows_label("Rect", d) for d in range(6)], [True] * 6)
        self.assertFalse(fit.shows_label("", 0))

    def test_elide_keeps_the_longest_prefix_that_fits_and_ends_with_an_ellipsis(self):
        self.assertEqual(fit.elide("Frame 8", 100), "Frame 8")
        cut = fit.elide("Component 1", 40)
        self.assertTrue(cut.endswith("…") and fit.width(cut) <= 40 and fit.width(cut) > 40 - 12)

    def test_a_name_gets_the_whole_column_where_the_label_is_dropped_and_what_the_label_leaves_where_it_is_drawn(self):
        """`widgets::elide(.., split.name_right - text_left)`: no margin is taken off the room."""
        self.assertEqual(fit.fit("Frame 13", "Frame", 5), ("Frame…", ""))
        self.assertEqual(fit.fit("Component 15", "Instance", 4), ("Compone…", ""))
        self.assertEqual(fit.fit("State 5", "Component", 1), ("State 5", "Component"))
        self.assertEqual(fit.name_right("", 5), fit.TEXT_RIGHT)

    def test_the_constants_of_the_data_module_are_the_variants_geometry(self):
        row = build("Tree/Row", State="Default", Depth="0")
        self.assertEqual(fit.TEXT_RIGHT, row["w"] - kid(row, "Toggles")["w"])
        self.assertEqual(fit.NAME_LEFT, kid(row, "Chevron box")["w"] + kid(kid(row, "Icon slot"), "Icon")["size"] + px(kid(row, "Icon slot")["pad"][1]))
        self.assertEqual(fit.INDENT, ENV.num("LAYER_INDENT"))
        self.assertEqual(fit.LABEL_RIGHT, fit.TEXT_RIGHT - px(kid(row, "Type slot")["pad"][1]))

    def test_the_variants_default_label_follows_the_same_rule_so_depth_five_fits_its_box(self):
        labels = [kid(kid(build("Tree/Row", State="Default", Depth=str(d)), "Type slot"), "Type label")["text"] for d in range(6)]
        self.assertEqual(labels, ["Auto layout"] * 3 + [""] * 3)
        self.assertEqual(labels, [fit.fit("Desktop", "Auto layout", d)[1] for d in range(6)])
        self.assertLessEqual(content_width(5, "Desktop", labels[5]), build("Tree/Row", State="Default", Depth="5")["w"])
        self.assertGreater(content_width(5, "Desktop", "Auto layout"), 224)  # the rule is what keeps the deepest row inside: with the label it would not fit


class WiredRowsTest(unittest.TestCase):
    def test_without_the_rule_many_rows_would_overprint_so_the_check_below_can_fail(self):
        raw = 0
        for state, entry in data.LEFT.items():
            for i, code in enumerate(data.TREES[data.STATE_TREE[state]].split()):
                key, depth, _c, _s = data.parse(code)
                name, label = f"{data.NAMES[key]} {i + 1}", data.ICONS[key][1]
                if fit.name_left(depth) + fit.width(name) + GAP > fit.LABEL_RIGHT - fit.width(label):
                    raw += 1
        self.assertGreater(raw, 100)

    def test_every_wired_row_keeps_its_name_left_of_its_label_and_inside_its_box(self):
        """The name ends before the label, or, where none is drawn, at the column's right edge: the row's width less the toggles (see the constants test above)."""
        bad = []
        for state, i, depth, name, label in wired_rows():
            end = fit.name_left(depth) + fit.width(name)
            limit = fit.LABEL_RIGHT - fit.width(label) - GAP if label else fit.TEXT_RIGHT
            if end > limit:
                bad.append((state, i, name, label, round(end, 1), round(limit, 1)))
        self.assertEqual(bad, [])

    def test_a_label_is_set_exactly_where_the_rule_shows_it(self):
        for state, entry in data.LEFT.items():
            for i, (code, row) in enumerate(zip(data.TREES[data.STATE_TREE[state]].split(), entry["slots"]["Layers"])):
                key, depth, _c, _s = data.parse(code)
                kind_label = data.ICONS[key][1]
                self.assertEqual(row["props"]["Type label"], kind_label if fit.shows_label(kind_label, depth) else "", (state, i, code))

    @unittest.skipUnless(_FONT is not None and FONT.exists(), "Pillow or the bundled Inter font is missing")
    def test_with_the_real_font_no_name_reaches_within_4_px_of_its_label(self):
        bad = []
        for state, i, depth, name, label in wired_rows():
            end = fit.name_left(depth) + _FONT.getlength(name)
            limit = fit.LABEL_RIGHT - _FONT.getlength(label) - GAP if label else fit.TEXT_RIGHT
            if end > limit:
                bad.append((state, i, name, label, round(end, 1), round(limit, 1)))
        self.assertEqual(bad, [])


if __name__ == "__main__":
    unittest.main()
