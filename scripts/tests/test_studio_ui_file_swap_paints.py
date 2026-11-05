"""Which glyphs a swap resets (designos #1427) and what `missing` names about them. Synthetic native documents.

A Disabled Button/Icon (`CTRL`) holds the slot `OLD_SLOT`: an instance of Icon/Search whose glyph is dimmed to 0.5, bound to `s.quat` and flagged `Fills`. A composite (`HOST`) nests it as
`ACTION` with the Icon swapped to Icon/Plus: the engine rebuilds the copied slot (`NEW_SLOT`, source `OLD_SLOT`) from Icon/Plus, whose glyph is white at opacity 1, bound to `s.icon`. That
copy, `NEW_GLYPH`, is the swapped glyph; `OLD_GLYPH` is its pre-swap glyph.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_swap_paints
"""
import copy
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
import swap_paints as sp  # noqa: E402

PAGE, PLUS, PLUS_GLYPH, SEARCH, SEARCH_GLYPH, CTRL, OLD_SLOT, OLD_GLYPH, HOST, ACTION, NEW_SLOT, NEW_GLYPH = 1, 10, 11, 12, 13, 20, 21, 22, 30, 31, 32, 33
GREY = {"Solid": {"r": 0.451, "g": 0.451, "b": 0.451, "a": 1.0}}
WHITE = {"Solid": {"r": 1.0, "g": 1.0, "b": 1.0, "a": 1.0}}


def paint(kind=WHITE, opacity=1.0):
    return {"kind": copy.deepcopy(kind), "opacity": opacity, "visible": True, "blend": "Normal"}


def node(nid, name, kind, parent, children=(), source=None, fills=None, binding=None, flags=()):
    n = {"id": nid, "name": name, "kind": kind, "parent": parent, "children": list(children)}
    if fills is not None:
        n["fills"] = fills
    if binding:
        n["variables"] = {"bindings": {"fills/0": binding}, "modes": {}}
    if source is not None or flags:
        n["component"] = {**({"source": source} if source is not None else {}), **({"overrides": list(flags)} if flags else {})}
    return n


def inst(main):
    return {"Instance": {"frame": {}, "component": main}}


def doc(kind=GREY, opacity=0.5, binding="s.quat", flag=True, old=None):
    """The staged state of the swap: the swapped glyph carries `kind`, `opacity`, `binding` and (with `flag`) `Fills`. The defaults are what the pass leaves; the pre-swap
    glyph is `old` (default: dimmed grey 0.5, `s.quat`, `Fills`)."""
    nodes = [
        node(PAGE, "03 Studio", "Page", None, [PLUS, SEARCH, CTRL, HOST]),
        node(PLUS, "Icon/Plus", {"Component": {}}, PAGE, [PLUS_GLYPH]),
        node(PLUS_GLYPH, "Glyph", "Vector", PLUS, fills=[paint(WHITE, 1.0)], binding="s.icon"),
        node(SEARCH, "Icon/Search", {"Component": {}}, PAGE, [SEARCH_GLYPH]),
        node(SEARCH_GLYPH, "Glyph", "Vector", SEARCH, fills=[paint(WHITE, 1.0)], binding="s.icon"),
        node(CTRL, "Size=Xs, State=Disabled", {"Component": {}}, PAGE, [OLD_SLOT]),
        node(OLD_SLOT, "Icon", inst(SEARCH), CTRL, [OLD_GLYPH], source=SEARCH),
        old or node(OLD_GLYPH, "Glyph", "Vector", OLD_SLOT, source=SEARCH_GLYPH, fills=[paint(GREY, 0.5)], binding="s.quat", flags=["Fills"]),
        node(HOST, "State=Default", {"Component": {}}, PAGE, [ACTION]),
        node(ACTION, "Action", inst(CTRL), HOST, [NEW_SLOT], source=CTRL),
        node(NEW_SLOT, "Icon", inst(PLUS), ACTION, [NEW_GLYPH], source=OLD_SLOT),
        node(NEW_GLYPH, "Glyph", "Vector", NEW_SLOT, source=PLUS_GLYPH, fills=[paint(kind, opacity)], binding=binding, flags=["Fills"] if flag else ()),
    ]
    return {"nodes": {str(n["id"]): n for n in nodes}, "pages": [PAGE], "variables": {}, "style_meta": {}}


def reset_glyph():
    """What 0.7.5 leaves after the swap: the Icon main's own paint, no binding of its own, no flag."""
    return doc(kind=WHITE, opacity=1.0, binding=None, flag=False)


def fields(d):
    return [m["field"] for m in sp.missing(d)]


class Targets(unittest.TestCase):
    def test_the_swapped_glyph_is_found_with_its_pre_swap_glyph(self):
        (glyph, pre), = sp.swapped(reset_glyph())
        self.assertEqual((glyph["id"], pre["id"]), (NEW_GLYPH, OLD_GLYPH))

    def test_a_slot_whose_source_is_the_icon_main_is_left_alone(self):
        d = reset_glyph()
        d["nodes"][str(NEW_SLOT)]["component"]["source"] = PLUS  # a slot a module draws directly: no pre-swap node exists
        self.assertEqual(sp.swapped(d), [])

    def test_a_copy_of_a_slot_with_the_same_main_is_not_a_swap(self):
        d = reset_glyph()
        d["nodes"][str(NEW_SLOT)]["kind"] = inst(SEARCH)
        self.assertEqual(sp.swapped(d), [])

    def test_a_swap_to_something_that_is_not_an_icon_is_ignored(self):
        d = reset_glyph()
        d["nodes"][str(PLUS)]["name"] = "Plus"
        self.assertEqual(sp.swapped(d), [])

    def test_a_pre_swap_glyph_that_is_itself_swapped_comes_first(self):
        d = reset_glyph()
        n = d["nodes"]
        n["40"] = node(40, "Icon", inst(SEARCH), ACTION, [41], source=NEW_SLOT)  # swapped again, back to Search
        n["41"] = node(41, "Glyph", "Vector", 40, source=SEARCH_GLYPH, fills=[paint(WHITE, 1.0)])
        n[str(ACTION)]["children"].append(40)
        self.assertEqual([(g["id"], p["id"]) for g, p in sp.swapped(d)], [(NEW_GLYPH, OLD_GLYPH), (41, NEW_GLYPH)])

    def test_a_swap_that_is_its_own_pre_swap_glyph_raises(self):
        d = reset_glyph()
        n = d["nodes"]
        n[str(OLD_SLOT)]["component"]["source"] = NEW_SLOT  # the two slots copy each other, with different mains
        with self.assertRaises(ValueError):
            sp.swapped(d)

    def test_a_slot_with_no_glyph_has_nothing_to_keep(self):
        d = reset_glyph()
        d["nodes"][str(NEW_SLOT)]["children"] = []
        self.assertEqual(sp.swapped(d), [])


class Missing(unittest.TestCase):
    def test_the_reset_glyph_names_the_dropped_paint_binding_and_flag(self):
        out = sp.missing(reset_glyph())
        self.assertEqual([m["field"] for m in out], ["paint", "binding", "flag"])
        self.assertEqual({m["id"] for m in out}, {NEW_GLYPH})
        paint_line, binding_line, flag_line = (m["problem"] for m in out)
        self.assertIn("03 Studio/State=Default/Action/Icon/Glyph", paint_line)
        self.assertIn("paint 0 is #FFFFFF at opacity 1, the pre-swap glyph 22 has #737373 at opacity 0.5", paint_line)
        self.assertIn("fills/0 is bound to s.icon, the pre-swap glyph 22 has s.quat", binding_line)
        self.assertIn("no Fills override", flag_line)

    def test_a_glyph_the_pass_has_fixed_is_complete(self):
        self.assertEqual(sp.missing(doc()), [])

    def test_a_wrong_colour_alone_is_the_paint(self):
        self.assertEqual(fields(doc(kind=WHITE)), ["paint"])

    def test_a_wrong_opacity_alone_is_the_paint(self):
        self.assertEqual(fields(doc(opacity=1.0)), ["paint"])

    def test_a_wrong_binding_alone_is_the_binding(self):
        self.assertEqual(fields(doc(binding="s.icon")), ["binding"])

    def test_a_missing_flag_alone_is_the_flag(self):
        self.assertEqual(fields(doc(flag=False)), ["flag"])

    def test_a_glyph_with_no_binding_of_its_own_reads_its_source_through_the_chain(self):
        """The glyph has no `fills/0` of its own, so its effective binding is its source's `s.icon`: different from `s.quat`, so the binding is dropped."""
        self.assertEqual(fields(doc(binding=None)), ["binding"])

    def test_a_pre_swap_glyph_with_no_binding_of_its_own_takes_its_source_s(self):
        old = node(OLD_GLYPH, "Glyph", "Vector", OLD_SLOT, source=SEARCH_GLYPH, fills=[paint(GREY, 0.5)], flags=["Fills"])  # its binding is the Icon main's, s.icon
        self.assertEqual(fields(doc(binding="s.icon", old=old)), [])
        self.assertEqual(fields(doc(binding="s.quat", old=old)), ["binding"])

    def test_a_swap_between_icons_that_paint_alike_needs_no_flag(self):
        """Pre-swap glyph = the new Icon main's glyph (white, s.icon): the glyph already equals its source, so there is nothing to keep and nothing to flag."""
        old = node(OLD_GLYPH, "Glyph", "Vector", OLD_SLOT, source=SEARCH_GLYPH, fills=[paint(WHITE, 1.0)], binding="s.icon")
        self.assertEqual(fields(doc(kind=WHITE, opacity=1.0, binding="s.icon", flag=False, old=old)), [])
        self.assertEqual(fields(doc(kind=WHITE, opacity=1.0, binding=None, flag=False, old=old)), [])

    def test_a_pre_swap_glyph_that_is_not_solid_cannot_be_kept(self):
        old = node(OLD_GLYPH, "Glyph", "Vector", OLD_SLOT, source=SEARCH_GLYPH, fills=[], binding="s.quat")
        out = sp.missing(doc(old=old))
        self.assertEqual([m["field"] for m in out], ["paint"])
        self.assertIn("cannot keep the pre-swap paint", out[0]["problem"])


if __name__ == "__main__":
    unittest.main()
