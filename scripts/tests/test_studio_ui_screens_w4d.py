"""Batch W4D of page `04 Screens` (screens/w4d_*.py): the agents-panel and AI image state screens.

What is pinned: that all 14 captures of the batch are built under `Screen/EditorDesign/<case>`, that every region sits at the rect its capture's layout.json gives, that every placed record
and variant (in slots too) is in the registry, that every override names a node that exists in the placed component's main in the committed file, and that `screen_geometry.check` matches the
named rects with 0 mismatches. The capture folder is untracked: the layout cross-checks run when it is present.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_screens_w4d
"""
import json
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
sys.path.insert(0, str(HERE.parent))
from evidence import screen_geometry  # noqa: E402
from evidence.capture_dirs import captures_dir  # noqa: E402
from screens import w4d_agents, w4d_ai_image, w4d_common  # noqa: E402

CAPTURES = captures_dir(json.loads((HERE.parent / "studio_ui_file/evidence/crops.json").read_text(encoding="utf-8"))["captures"], None)
REGISTRY = json.loads((REPO / "design/component-registry.json").read_text(encoding="utf-8"))
AGENTS = ["i-agents", "i-agents-chat", "i-agents-picker", "i-agents-start-failure", "i-agent-permissions", "i-agents-approval-command", "i-terminal", "agents-approval-open-tab",
          "agents-approval-queued", "agents-skills", "kit-agents"]
AI_IMAGE = ["ai-image-menu", "ai-image-erase", "ai-image-expand"]


def specs():
    return w4d_agents.screens() + w4d_ai_image.screens()


def kind(n):
    k = n["kind"]
    return next(iter(k)) if isinstance(k, dict) else k


class FileTree:
    """The committed file's component mains, found by registry name, with a path lookup by node names."""

    def __init__(self):
        self.doc = json.loads((REPO / "design/studio-ui.designos").read_text(encoding="utf-8"))

    def main(self, record, variant=None):
        """The main of `record`; of a set, the variant the placed `variant` names: the set's first variant with the given axes laid over it (a partial map keeps the others)."""
        nodes = self.doc["nodes"]
        for n in nodes.values():
            if n["name"] == record and kind(n) in ("ComponentSet", "Component"):
                if kind(n) == "Component":
                    return n
                first = [nodes[str(c)] for c in n["children"]]
                pairs = dict(part.split("=") for part in first[0]["name"].split(", "))
                pairs.update(variant or {})
                want = ", ".join(f"{axis}={value}" for axis, value in pairs.items())
                return next((c for c in first if c["name"] == want), first[0])
        raise KeyError(record)

    def resolve(self, root, path):
        node = root
        for part in path.split("/"):
            node = next((self.doc["nodes"][str(c)] for c in node.get("children", []) if self.doc["nodes"][str(c)]["name"] == part), None)
            if node is None:
                return None
        return node


class BuiltTest(unittest.TestCase):
    def test_every_capture_of_the_batch_is_built_once_under_its_registry_screen(self):
        self.assertEqual([s["name"] for s in specs()], [f"Screen/EditorDesign/{c}" for c in AGENTS + AI_IMAGE])
        self.assertTrue(all((s["w"], s["h"], s["state"]) == (1440, 900, s["name"].split("/")[-1]) for s in specs()))
        self.assertIn("Screen/EditorDesign", {r["name"] for r in REGISTRY["components"] if r.get("category") == "screen"})

    def test_every_placed_record_and_variant_including_slots_is_in_the_registry(self):
        known = {r["name"]: r.get("variants", []) for r in REGISTRY["components"]}
        for s in specs():
            entries = list(s["place"]) + [e for p in s["place"] for es in (p.get("slots") or {}).values() if isinstance(es, list) for e in es]
            for p in entries:
                self.assertIn(p["component"], known, s["name"])
                for axis, value in (p.get("variant") or {}).items():
                    self.assertIn(f"{axis}={value}", known[p["component"]], s["name"])

    def test_the_shells_place_the_regions_at_the_rects_of_the_captures(self):
        for s in specs():
            at = {p["name"]: (p["x"], p["y"]) for p in s["place"]}
            fixed = (at["Navigation/TopTabBar"], at["Navigation/LeftRail"], at["Panel/RightInspector"], at["Navigation/BottomCanvasToolbar"])
            self.assertEqual(fixed, ((0, 0), (0, 32), (1199, 32), (464, 840)), s["name"])
            agents = s["state"] in AGENTS
            self.assertEqual((at["Panel/AgentTerminal" if agents else "Panel/LeftLayers"], at["Navigation/Rulers"]), ((49, 32), s["canvas"][:2]), s["name"])
            self.assertEqual(s["canvas"][:4], w4d_common.AGENTS_CANVAS if agents else w4d_common.LAYERS_CANVAS)

    def test_the_rulers_are_painted_under_the_right_inspector_which_covers_their_overhang(self):
        order = [p["name"] for p in specs()[0]["place"]]
        self.assertLess(order.index("Navigation/Rulers"), order.index("Panel/RightInspector"))

    @unittest.skipUnless(CAPTURES.exists(), "the 0.7.5 captures are untracked")
    def test_the_regions_of_every_capture_are_the_rects_the_shells_use(self):
        for s in specs():
            regions = json.loads((CAPTURES / f"{s['state']}.layout.json").read_text())["regions"]
            agents = s["state"] in AGENTS
            self.assertEqual(regions["left_panel"], [49.0, 32.0, 401.0 if agents else 241.0, 868.0], s["state"])
            self.assertEqual(regions["canvas"], [float(v) for v in s["canvas"][:4]], s["state"])
            self.assertEqual((regions["rail"], regions["right_panel"], regions["tab_bar"]), ([0.0, 32.0, 49.0, 868.0], [1199.0, 32.0, 241.0, 868.0], [0.0, 0.0, 1440.0, 32.0]), s["state"])


class OverrideTest(unittest.TestCase):
    tree = None

    @classmethod
    def setUpClass(cls):
        cls.tree = FileTree()

    def test_every_override_names_a_node_of_the_placed_main_and_a_text_override_a_text_node(self):
        for s in specs():
            for p in s["place"]:
                for o in p.get("overrides", []):
                    node = self.tree.resolve(self.tree.main(p["component"], p.get("variant")), o["path"])
                    self.assertIsNotNone(node, f"{s['name']}: {p['component']} has no node at {o['path']!r}")
                    if "text" in o:
                        self.assertEqual(kind(node), "Text", f"{s['name']}: {o['path']!r} is not a text")

    def test_a_pty_screen_hides_the_picker_body_and_keeps_the_header_strip_and_composer(self):
        main = self.tree.main("Panel/AgentTerminal")
        kept = {self.tree.doc["nodes"][str(c)]["name"] for c in main["children"]} - set(w4d_agents.BODY)
        self.assertTrue({"Bar", "Sessions", "Skills", "Selection chip", "Mention", "Composer"} <= kept)
        term = next(p for p in next(s for s in specs() if s["state"] == "i-agent-permissions")["place"] if p["component"] == "Panel/AgentTerminal")
        self.assertEqual([o["path"] for o in term["overrides"]], w4d_agents.BODY)  # the modal sits over a PTY: the picker body is not shown
        self.assertTrue(all(o == {"path": o["path"], "visible": False} for o in term["overrides"]))
        states = {"i-agents-chat": "Chat", "i-agents-start-failure": "Failure", "i-agents-approval-command": "Approval", "agents-approval-open-tab": "Approval",
                  "agents-approval-queued": "Approval", "i-agents": "Picker", "i-agents-picker": "Picker", "i-terminal": "Picker", "kit-agents": "Picker"}
        for case, state in states.items():  # a session state is a state of the panel, so no picker part is hidden by hand
            term = next(p for p in next(s for s in specs() if s["state"] == case)["place"] if p["component"] == "Panel/AgentTerminal")
            self.assertEqual(term["variant"], {"State": state}, case)
            self.assertFalse({o["path"] for o in term.get("overrides", [])} & set(w4d_agents.BODY), case)
        term = next(p for p in next(s for s in specs() if s["state"] == "agents-skills")["place"] if p["component"] == "Panel/AgentTerminal")
        self.assertNotIn("overrides", term)

    def test_the_overlays_sit_at_the_rects_the_captures_give_and_are_the_kind_they_show(self):
        want = {"i-agent-permissions": ("Popover/Panel", (540, 326), "Permissions", (360, 248)), "agents-skills": ("Popover/Panel", (57, 608), "Skills", (320, 192)),
                "ai-image-menu": ("Menu/Panel", (1216, 668), None, (180, 161)), "ai-image-erase": ("Popover/Panel", (944, 640), "Erase", (248, 144)),
                "ai-image-expand": ("Popover/Panel", (956, 640), "Expand", (236, 156))}
        for case, (component, xy, kind_name, size) in want.items():
            place = next(s for s in specs() if s["state"] == case)["place"]
            overlay = next(p for p in place if p["component"] in ("Popover/Panel", "Menu/Panel"))
            self.assertEqual((overlay["component"], (overlay["x"], overlay["y"])), (component, xy), case)
            if kind_name:
                self.assertEqual(overlay["variant"]["Kind"], kind_name, case)
                main = self.tree.main("Popover/Panel", overlay["variant"])
                self.assertEqual((main["size"]["x"], main["size"]["y"]), size, f"{case}: the kind is the size of the capture's overlay")
            else:
                self.assertEqual((overlay["w"], overlay["h"]), size, case)
            self.assertEqual([p["component"] for p in place if p["component"] in ("Radio/Card", "Button/Close", "Divider/Hairline")], [], f"{case}: the kind draws its own rows and header")
        self.assertEqual([p["component"] for p in next(s for s in specs() if s["state"] == "i-agents")["place"]][-1], "Panel/RightInspector")

    def test_the_skills_popover_close_button_sits_where_production_centres_it(self):
        """`skills/mod.rs:841` centres the 24 px close at (right - 16, top + 16) of the 320 x 192 popover at (57, 608): its box starts at (349, 612) (the capture's `agents-skills-picker-close`)."""
        rects = screen_geometry.path_rects(self.tree.doc, "Screen/EditorDesign/agents-skills", "agents-skills-picker-popover > Close")
        self.assertEqual(rects, [(349.0, 612.0, 24.0, 24.0)])
        rule = screen_geometry.path_rects(self.tree.doc, "Screen/EditorDesign/agents-skills", "agents-skills-picker-popover > Header rule")
        self.assertEqual(rule, [(65.0, 636.0, 304.0, 1.0)], "`skills/mod.rs:871` draws the rule from left + SPACE_INLINE to right - SPACE_INLINE, 304 px at y 636")

    @unittest.skipUnless(CAPTURES.exists(), "the 0.7.5 captures are untracked")
    def test_every_screen_matches_its_layout_geometry_with_zero_mismatches(self):
        for s in specs():
            layout = json.loads((CAPTURES / f"{s['state']}.layout.json").read_text())
            rep = screen_geometry.check(self.tree.doc, s["name"], layout, aliases=s["aliases"])
            self.assertEqual(rep["mismatched"], [], s["name"])
            self.assertGreaterEqual(len(rep["matched"]), 30, s["name"])


class HelperTest(unittest.TestCase):
    def test_popover_carries_its_title_and_line_and_the_size_of_the_capture_rect(self):
        p = w4d_common.popover((10, 20, 300, 100), "A title", "A line")
        self.assertEqual((p["x"], p["y"], p["w"], p["h"], p["component"], p["props"]), (10, 20, 300, 100, "Popover/Panel", {"Title": "A title"}))
        self.assertEqual(p["overrides"], [{"path": "Header/Close", "visible": False}, {"path": "Body/Row/Label", "text": "A line"}])

    def test_hide_and_say_build_override_entries(self):
        self.assertEqual(w4d_common.hide("A", "B/C"), [{"path": "A", "visible": False}, {"path": "B/C", "visible": False}])
        self.assertEqual(w4d_common.say("A", "t"), {"path": "A", "text": "t"})


if __name__ == "__main__":
    unittest.main()
