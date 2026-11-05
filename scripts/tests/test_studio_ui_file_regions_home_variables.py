"""Window/Variables (components/regions_home_variables.py) read from the IR the builder returns against the real Semantic plan. The rects are the 0.7.5 capture `k-variables` (the full view,
`k-variables.layout.json` named rects of a 1440 x 900 window whose variables area starts at (49, 32)), so every rect here is the capture's minus (49, 32); the Rust is v0.7.5 `variables/view.rs`
(`sidebar` :444-641, `main` :644-749, `bottom_bar` :751-808). Structure only: pixels are compared in `evidence compare`.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_regions_home_variables
"""
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
sys.path.insert(0, str(HERE.parent))
import components  # noqa: E402
import icon_lib  # noqa: E402
import text_styles_plan  # noqa: E402
import variables_plan as plan_mod  # noqa: E402
from components.env import Env  # noqa: E402
from rest_ir import pads, walk  # noqa: E402

PLAN = plan_mod.build_plan(str(REPO))
ENV = Env(str(REPO), PLAN, icon_lib.IconLib(str(REPO)))
SPECS = {s["name"]: s for s in components.discover()}
STYLES = {f"{role} / {label}" for role, label, *_ in text_styles_plan.ROLES}


def part(node, name):
    found = [k for k in walk(node) if k["name"] == name]
    assert len(found) == 1, (name, len(found))
    return found[0]


class VariablesWindowTest(unittest.TestCase):
    def setUp(self):
        self.spec = SPECS["Window/Variables"]
        self.root = self.spec["build"]({"Layout": "Full"}, ENV)  # the first value is today's look

    def test_it_is_a_plain_region_record_nesting_table_data_and_input_search_as_instances(self):
        """No registry variants. The table is a Table/Data copy (its `Mode` text set on the copy, its default rows the component's) and the 174 x 24 search field an Input/Search Xs copy resized with
        `w=` / `h=`: nothing of either is drawn again here, and the region has no property of its own."""
        self.assertEqual(self.spec["axes"], {"Layout": ["Full", "Floating", "Present"]})
        self.assertNotIn("props", self.spec)
        self.assertEqual(sorted(self.spec["uses"]), ["Divider/Hairline", "Input/Search", "Table/Data"])
        self.assertEqual({k["set"] for k in walk(self.root) if k["t"] == "X"}, set(self.spec["uses"]))
        table, search = part(self.root, "Table"), part(self.root, "Search")
        self.assertEqual((table["set"], table["variant"], table["props"], table["abs"], table["h"]), ("Table/Data", "", {"Mode": "Style"}, (241, 49, 521, 778), 778))
        self.assertEqual((search["set"], search["variant"], search["abs"], (search["w"], search["h"])), ("Input/Search", "Size=Xs, State=Default", (1078, 12, 174, 24), (174, 24)))
        self.assertEqual([k["name"] for k in walk(self.root) if k["t"] != "X" and k.get("slot")], [], "no local copy of the table's Rows Slot")

    def test_the_region_is_the_full_view_1391_by_868_on_the_page_colour_and_clips(self):
        """`variables_full` (49, 32, 1391, 868); the sidebar is `VAR_SIDEBAR_W` 240 wide with a `BORDER` vline at x 240 (:449-450)."""
        self.assertEqual((self.root["w"], self.root["h"], self.root["fill"], self.root["clip"]), (1391, 868, "color/bg", True))
        edge = part(self.root, "Sidebar edge")
        self.assertEqual((edge["abs"], edge["fill"]), ((240, 0, 1, 868), "color/border"))

    def test_the_named_rects_of_the_capture_are_the_parts_of_the_same_name(self):
        """vars-sidebar-toggle 253, 44; vars-title 306, 44; vars-search 1127, 44 (174 x 24); vars-filter 1302, 44; vars-collapse 1338, 44; vars-settings 230, 89; vars-add-collection 257, 89;
        vars-create 298, 864 (128 x 32); vars-scope:All 442, 869 (30.5 x 22); the inlined table at its top-left (290, 81)."""
        want = {"Search": (1078, 12, 174, 24), "Filter": (1253, 12, 24, 24), "Create variable": (249, 832, 128, 32), "Scope All": (393, 837, 30.5, 22), "Table": (241, 49, 521, 778)}
        self.assertEqual({name: tuple(part(self.root, name)["abs"]) for name in want}, want)
        # the glyph buttons draw a 16 px glyph centred in their 24 px rect (`glyph_button`, :387-397): sidebar-toggle (204, 12), settings (181, 57), add-collection (208, 57), collapse (1289, 12)
        centres = {"Sidebar toggle": (216, 24), "Settings": (193, 69), "Add collection": (220, 69), "Collapse": (1301, 24)}
        self.assertEqual({n: (part(self.root, n)["abs"][0] + 8, part(self.root, n)["abs"][1] + 8) for n in centres}, centres)

    def test_the_collection_rows_are_224_by_24_at_a_32_pitch_from_93_and_the_first_is_selected(self):
        """`VAR_PILL_H` 24 rows at `VAR_SIDE_ROW` 32 from `base + 44` (:489-502): capture vars-coll rows 189, 221, 253, 285 -> 157, 189, 221, 253; the selected one is `BG_SECONDARY` (:510)."""
        rows = [part(self.root, f"Collection {i}") for i in range(1, 8)]
        self.assertEqual([r["abs"] for r in rows], [(8, 93 + 32 * i, 224, 24) for i in range(7)])
        self.assertEqual([r.get("fill") for r in rows], ["color/bg-secondary"] + [None] * 6)
        self.assertEqual((rows[0]["kids"][0]["text"], rows[0]["kids"][-1]["text"]), ("Primitives", "343"))
        self.assertEqual(pads(rows[0])[1], "inset/control")

    def test_the_library_collections_follow_a_heading_and_carry_a_lock(self):
        """vars-heading 'From library' at 349 -> 317; library rows below at 349, 381, 413 with `paint_lock` 10 px left of the count (:533)."""
        self.assertEqual(part(self.root, "Library heading")["abs"][1] + 9, 317 + 12)  # the heading row's centre line
        locks = [(r["abs"][1], [k["icon"] for k in r["kids"] if k["t"] == "I"]) for r in (part(self.root, f"Library {i}") for i in range(1, 4))]
        self.assertEqual(locks, [(349, ["Lock"]), (381, ["Lock"]), (413, ["Lock"])])

    def test_the_groups_start_below_a_rule_and_a_heading_and_nest_by_16(self):
        """`sep_y` = base + 48 + (collections + heading) * 32 = 449 (:557), `Groups` at +21, rows from +45 = 494 (:562): `All` selected on `BG_SELECTED` (:598), children at `INSET_CONTROL` + depth
        * 16."""
        self.assertEqual(part(self.root, "Rule above groups")["abs"], (0, 449, 239, 1))
        rows = [part(self.root, f"Group {i}") for i in range(1, 13)]
        self.assertEqual([r["abs"][1] for r in rows], [494 + 32 * i for i in range(12)])
        self.assertEqual((rows[0]["fill"], rows[1].get("fill")), ("color/bg-selected", None))
        self.assertEqual([pads(r)[3][1] if isinstance(pads(r)[3], tuple) else pads(r)[3] for r in rows[:3]], ["inset/control", 24.0, 40.0])

    def test_the_bottom_bar_holds_the_create_button_and_the_six_scope_chips(self):
        """:751-808: a hairline at `bottom_line`, `Create variable` (Plus 16 at left + 14, the label at left + 34), the scope chips 22 high pills in caption medium, `All` on `BG_TERTIARY`."""
        chips = [part(self.root, f"Scope {n}") for n in ("All", "Fills", "Strokes", "Text", "Radius", "Gap & Size")]
        self.assertEqual([(c["abs"][0], c["abs"][2]) for c in chips], [(393, 30.5), (427.75, 38.5), (470.25, 59.5), (533.75, 40.75), (578.25, 54.75), (637, 77.75)])
        self.assertEqual((chips[0]["fill"], chips[1].get("fill"), chips[0]["radius"][:2]), ("color/bg-tertiary", None, ("lit", 11.0)))
        self.assertEqual(part(self.root, "Rule above bottom bar")["abs"], (241, 827, 1150, 1))

    def test_kit_names_are_replaced_and_every_text_uses_an_existing_style_and_a_semantic_colour(self):
        names = {s["name"] for s in PLAN["semantic"]}
        texts = []
        for k in walk(self.root):
            if k["t"] == "T":
                self.assertIn(k["style"], STYLES)
                self.assertIn(k["color"], names)
                texts.append(k["text"])
        self.assertTrue(all(w not in t.lower() for t in texts for w in ("untitled", "vsf", "pcp", "olivia")))


if __name__ == "__main__":
    unittest.main()
