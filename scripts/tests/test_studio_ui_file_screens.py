"""Page `04 Screens` (screens/): the specs the modules export in user-flow order, their check against the registry, and the `apply_spec` frame built
from one (instances by registry name, the one background rectangle first). A fake MCP stands in for the engine.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_screens
"""
import json
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
sys.path.insert(0, str(HERE.parent))
import screens  # noqa: E402
from screens import gap_table_citations as gtc, same_drawing  # noqa: E402
from studio_ui_gate_screens import screen_record  # noqa: E402

REGISTRY = json.loads((REPO / "design/component-registry.json").read_text(encoding="utf-8"))
LAYOUT = json.loads((REPO / "scripts/studio_ui_file/evidence/captures/h-home-empty.layout.json").read_text(encoding="utf-8"))
SPEC = {"name": "Screen/EditorDesign", "w": 1440, "h": 900, "state": "d-editor-default", "canvas": (290, 32, 909, 868, "#F5F5F5", "Canvas background"),
        "place": [{"name": "Navigation/TopTabBar", "component": "Navigation/TopTabBar", "variant": {"Bar": "32px"}, "x": 0, "y": 0},
                  {"name": "Navigation/Rulers", "component": "Navigation/Rulers", "variant": None, "x": 290, "y": 32}]}
FLOW_GROUPS = (
    ("Home", ("h-home",)),
    ("Editor shell", ("d-editor-default", "d-window-chrome", "d-editor-selected", "kit-editor")),
    ("Left panel", ("e-no-selection", "e-layers-selected", "j-find", "k-assets", "i-agents", "i-agents-picker", "i-terminal", "i-agents-chat",
                    "i-agents-start-failure", "i-agent-permissions", "i-agents-approval-command", "agents-approval-open-tab", "agents-approval-queued",
                    "agents-skills", "kit-agents")),
    ("Right panel", ("picker-page", "fill-d9", "f-picker", "f-picker-gradient", "f-edit-style-color", "edit-style-effect", "edit-style-grid",
                     "edit-style-text", "f-stroke-settings", "f-effects-menu", "f-effect-styles", "effect-noise", "effect-glass", "effect-texture",
                     "grid-styles", "f-type-details", "f-prototype-tab", "ai-image-menu", "ai-image-erase", "ai-image-expand", "j-check-designs",
                     "j-comments", "j-version-history", "g-dev-react", "g-dev-annotate", "g-dev-compare")),
    ("Menus and dialogs", ("d-menu-main", "d-menu-main-file", "d-menu-shape", "d-menu-zoom", "j-cmdk", "j-palette", "j-plugins", "plugin-install",
                           "k-libraries")),
    ("Variables", ("k-variables", "k-variables-floating")),
    ("Present", ("x-present", "present-variables")),
    ("Canvas micro states", ("micro-layer-picker", "micro-smart-selection", "micro-selection-corner", "micro-scale-tool", "micro-vector-edit",
                             "micro-shape-controls", "micro-autolayout", "micro-css-grid", "micro-inspector-math", "micro-typography-multiedit",
                             "micro-components-variables", "micro-dev-boxmodel")),
)
EXPECTED_ORDER = [s for _, group in FLOW_GROUPS for s in group]


class FakeMcp:
    def __init__(self):
        self.calls = []

    def tool_or_die(self, tool, args):
        self.calls.append((tool, args))
        return {"root_node_id": 100 + len(self.calls)}


class DiscoverTest(unittest.TestCase):
    def test_the_editor_screen_is_found_and_is_a_registry_screen(self):
        specs = screens.check_specs(screens.discover(), REGISTRY)
        names = [s["name"] for s in specs]
        self.assertIn("Screen/EditorDesign", names)
        category = {r["name"]: r.get("category", "component") for r in REGISTRY["components"]}
        self.assertTrue(all(screen_record(n, category) for n in names), names)
        first = next(s for s in specs if s["name"] == "Screen/EditorDesign")
        self.assertEqual((first["w"], first["h"], first["state"]), (1440, 900, "d-editor-default"))

    def test_screens_discover_in_user_flow_order_and_stack_without_gaps(self):
        specs = screens.discover()
        self.assertEqual((screens.PAD, screens.GAP, screens.HEADER, screens.LABEL), (200, 200, 56, 28))
        self.assertEqual(screens.GROUPS, FLOW_GROUPS)
        self.assertEqual([s["state"] for s in specs], EXPECTED_ORDER)
        self.assertEqual(len(specs), 71)
        mcp = FakeMcp()
        screens.build(mcp, 7, specs)
        sections = [c[1]["spec"] for c in mcp.calls if c[1]["spec"]["kind"] == "section"]
        frames = [c[1]["spec"] for c in mcp.calls if c[1]["spec"]["kind"] == "frame"]
        labels = [c[1]["spec"] for c in mcp.calls if c[1]["spec"]["kind"] == "text"]
        self.assertEqual([s["name"] for s in sections], [g[0] for g in FLOW_GROUPS])
        self.assertEqual([s["x"] for s in sections], [i * 2040 for i in range(len(FLOW_GROUPS))])
        self.assertTrue(all(s["y"] == 0 and s["width"] == 1840 for s in sections))
        self.assertEqual([s["height"] for s in sections], [1384, 4768, 17176, 29584, 10408, 2512, 2512, 13792])
        self.assertTrue(all(s["children"] == [dict(screens.kit.text("Section title", s["name"], style="Title / Semibold"), x=200, y=200)] for s in sections))
        self.assertEqual([f["name"] for f in frames], [s["name"] for s in specs])
        self.assertEqual([l["name"] for l in labels], [f"Label {s['name']}" for s in specs])
        want_y = [284 + i * 1128 for _, grp in FLOW_GROUPS for i in range(len(grp))]
        self.assertEqual([f["y"] for f in frames], want_y)
        self.assertTrue(all(f["x"] == 200 for f in frames))

    def test_same_drawings_pins_the_current_count_and_labels_every_duplicate_frame(self):
        specs = screens.discover()
        same = screens.same_drawings(specs)
        info = screens.same_drawing_info_lines(specs)
        self.assertEqual(len(specs), 71)
        self.assertEqual(len(same), 3)
        self.assertEqual(len(specs) - len(same), 68)
        self.assertEqual(info, ["INFO same-drawing d-window-chrome = d-editor-default", "INFO same-drawing e-no-selection = d-editor-default", "INFO same-drawing i-terminal = i-agents-picker"])
        self.assertNotIn("plugin-install", same)
        self.assertNotIn("fill-d9", same)
        gap_titles = {e["title"] for e in gtc.GAP_TABLE}
        self.assertTrue(set(same_drawing.GAP_NAMES.values()).issubset(gap_titles))
        self.assertTrue({r["gap"] for r in same.values() if r["gap"]}.issubset(gap_titles))
        first_by_sig, oracle = {}, {}
        for idx, s in enumerate(specs, 1):
            sig = repr((s["w"], s["h"], s["canvas"], s["place"], s.get("scrim")))
            if sig not in first_by_sig:
                first_by_sig[sig] = (idx, s["state"])
            else:
                oracle[s["state"]] = (idx, *first_by_sig[sig])
        self.assertEqual({st: (r["pos"], r["of_pos"], r["of_state"]) for st, r in same.items()}, oracle)
        self.assertEqual({r["of_state"] for r in same.values()}, {"d-editor-default", "i-agents-picker"})
        self.assertEqual({r["of_pos"] for r in same.values()}, {2, 11})
        mcp = FakeMcp()
        screens.build(mcp, 7, specs)
        by_label = {c[1]["spec"]["name"]: c[1]["spec"]["text"] for c in mcp.calls if c[1]["spec"]["kind"] == "text"}
        for idx, s in enumerate(specs, 1):
            text = by_label[f"Label {s['name']}"]
            self.assertTrue(text.startswith(f"#{idx} "), text)
            if s["state"] in oracle:
                self.assertTrue(text.startswith(f"#{idx} same drawing as #{oracle[s['state']][1]}: "), text)
                self.assertEqual(text, same[s["state"]]["label"])  # the whole built text, the reason included
            else:
                self.assertEqual(text, f"#{idx} {s['name']}")

    def test_an_unranked_screen_state_fails_loudly(self):
        import types
        mod = types.SimpleNamespace(screens=lambda: [dict(SPEC, name="Screen/EditorDesign/unranked", state="unranked")])
        orig = screens.importlib.import_module, screens.pkgutil.iter_modules
        screens.importlib.import_module = lambda name: mod
        screens.pkgutil.iter_modules = lambda path: [types.SimpleNamespace(name="a")]
        try:
            with self.assertRaises(ValueError):
                screens.discover()
        finally:
            screens.importlib.import_module, screens.pkgutil.iter_modules = orig

    def test_the_editor_screen_places_the_regions_the_capture_shows_at_the_rects_of_its_layout(self):
        s = next(s for s in screens.discover() if s["name"] == "Screen/EditorDesign")
        at = {p["name"]: (p["x"], p["y"]) for p in s["place"]}
        self.assertEqual(at, {"Navigation/TopTabBar": (0, 0), "Navigation/LeftRail": (0, 32), "Panel/LeftLayers": (49, 32), "Navigation/Rulers": (290, 32),
                              "Navigation/BottomCanvasToolbar": (464, 840), "Panel/RightInspector": (1199, 32)})
        self.assertNotIn("Navigation/TitleBar", at, "no capture shows the title bar: the harness hides it")
        self.assertEqual(s["canvas"][:4], (290, 32, 909, 868))

    def test_every_variant_named_is_a_registry_value(self):
        known = {r["name"]: r.get("variants", []) for r in REGISTRY["components"]}
        for p in next(s for s in screens.discover() if s["name"] == "Screen/EditorDesign")["place"]:
            for axis, value in (p["variant"] or {}).items():
                self.assertIn(f"{axis}={value}", known[p["component"]], p["name"])

    def test_a_screen_that_is_not_a_screen_record_fails(self):
        with self.assertRaises(ValueError):
            screens.check_specs([dict(SPEC, name="Navigation/LeftRail")], REGISTRY)

    def test_a_component_the_registry_lacks_fails(self):
        with self.assertRaises(ValueError):
            screens.check_specs([dict(SPEC, place=[{"name": "x", "component": "Made/Up", "variant": None, "x": 0, "y": 0}])], REGISTRY)


class BuildTest(unittest.TestCase):
    def test_the_frame_is_the_window_with_the_background_first_then_the_instances(self):
        spec = screens.frame_spec(SPEC, 0)
        self.assertEqual((spec["name"], spec["width"], spec["height"], spec["fill"]), ("Screen/EditorDesign", 1440, 900, "var:Semantic/color/bg"))
        kids = spec["children"]
        self.assertEqual([k["kind"] for k in kids], ["rectangle", "instance", "instance"])
        self.assertEqual((kids[0]["name"], kids[0]["fill"], kids[0]["x"], kids[0]["y"], kids[0]["width"], kids[0]["height"]), ("Canvas background", "#F5F5F5", 290, 32, 909, 868))

    def test_an_instance_is_named_by_the_registry_and_carries_its_variant_only_when_it_has_one(self):
        kids = screens.frame_spec(SPEC, 0)["children"]
        self.assertEqual(kids[1], {"kind": "instance", "name": "Navigation/TopTabBar", "component": "Navigation/TopTabBar", "x": 0, "y": 0, "variant": {"Bar": "32px"}})
        self.assertNotIn("variant", kids[2])

    def test_a_screen_without_a_canvas_draws_instances_only(self):
        self.assertEqual({k["kind"] for k in screens.frame_spec(dict(SPEC, canvas=None), 0)["children"]}, {"instance"})

    def test_screens_stack_down_the_page_with_a_gap_and_the_ids_are_returned_by_name(self):
        mcp = FakeMcp()
        ids = screens.build(mcp, 7, [SPEC, dict(SPEC, name="Screen/EditorDev", state="d-window-chrome")])
        sections = [c for c in mcp.calls if c[1]["spec"]["kind"] == "section"]
        frames = [c[1]["spec"] for c in mcp.calls if c[1]["spec"]["kind"] == "frame"]
        labels = [c[1]["spec"] for c in mcp.calls if c[1]["spec"]["kind"] == "text"]
        self.assertEqual(len(sections), 1)
        self.assertEqual(sections[0][1]["parent_id"], 7)
        self.assertEqual(ids, {"Screen/EditorDesign": 103, "Screen/EditorDev": 105})
        self.assertEqual([l["y"] for l in labels], [256, 1384])
        self.assertEqual([f["y"] for f in frames], [284, 1412])
        self.assertTrue(all(c[0] == "designos_apply_spec" and c[1]["parent_id"] == 101 for c in mcp.calls[1:]))

    def test_two_screens_with_one_name_fail_loudly(self):
        import types
        mod = types.SimpleNamespace(screens=lambda: [SPEC])
        orig = screens.importlib.import_module, screens.pkgutil.iter_modules
        screens.importlib.import_module = lambda name: mod
        screens.pkgutil.iter_modules = lambda path: [types.SimpleNamespace(name="a"), types.SimpleNamespace(name="b")]
        try:
            with self.assertRaises(ValueError):
                screens.discover()
        finally:
            screens.importlib.import_module, screens.pkgutil.iter_modules = orig


if __name__ == "__main__":
    unittest.main()
