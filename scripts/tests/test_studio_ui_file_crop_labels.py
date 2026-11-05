"""Crops gain `labels`: {"<node path below the variant>": "<text>"}, for a composite with several texts and for the texts of the instances nested in it.

A path is node names joined by `/`, resolved from the variant root; the compare sets each text on the scratch copy before rendering. An unknown name fails loudly with the names that
exist at that level, and so does a name two siblings share. `label` (with `label_node`) and `texts` keep working as they did. A fake MCP stands in for the engine.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_crop_labels
"""
import json
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
from evidence import node_paths  # noqa: E402

CROPS = HERE.parent / "studio_ui_file" / "evidence" / "crops.json"
# variant 1 -> Review button 2 (-> Leading icon 5, Label 6), Open item 3 (-> Label 7, Shortcut slot 8 -> Shortcut 9), Label 4, Label 10 (a twin of 4)
TREE = {1: [("Review button", 2), ("Open item", 3), ("Label", 4), ("Label", 10)], 2: [("Leading icon", 5), ("Label", 6)], 3: [("Label", 7), ("Shortcut slot", 8)], 8: [("Shortcut", 9)]}


class FakeMcp:
    def __init__(self):
        self.updates = []

    def tool_or_die(self, tool, args):
        if tool == "designos_list_nodes":
            return {"nodes": [{"name": n, "node_id": i} for n, i in TREE.get(args["parent_id"], [])], "next_offset": None}
        assert tool == "designos_update_node", tool
        self.updates.append((args["node_id"], args["text"]))
        return {}


class ResolveTest(unittest.TestCase):
    def test_a_direct_child_is_one_name(self):
        self.assertEqual(node_paths.resolve(FakeMcp(), 1, "Open item"), 3)

    def test_a_nested_text_is_a_path_below_the_variant(self):
        self.assertEqual(node_paths.resolve(FakeMcp(), 1, "Review button/Label"), 6)
        self.assertEqual(node_paths.resolve(FakeMcp(), 1, "Open item/Shortcut slot/Shortcut"), 9)

    def test_an_unknown_name_fails_and_lists_the_names_that_exist(self):
        with self.assertRaises(SystemExit) as e:
            node_paths.resolve(FakeMcp(), 1, "Review button/Title")
        self.assertIn("Title", str(e.exception))
        self.assertIn("Leading icon", str(e.exception))
        self.assertIn("Review button", str(e.exception))

    def test_a_name_two_siblings_share_is_ambiguous(self):
        with self.assertRaises(SystemExit) as e:
            node_paths.resolve(FakeMcp(), 1, "Label")
        self.assertIn("ambiguous", str(e.exception))

    def test_a_path_through_a_leaf_fails(self):
        with self.assertRaises(SystemExit):
            node_paths.resolve(FakeMcp(), 1, "Review button/Label/Deeper")

    def test_an_empty_path_or_segment_fails(self):
        for bad in ("", "Review button//Label", "/Label"):
            with self.assertRaises(SystemExit, msg=bad) as e:
                node_paths.resolve(FakeMcp(), 1, bad)
            self.assertIn("empty name", str(e.exception))

    def test_children_beyond_the_first_page_are_found(self):
        class Paged(FakeMcp):
            def tool_or_die(self, tool, args):
                if tool == "designos_list_nodes":
                    return ({"nodes": [{"name": "A", "node_id": 20}], "next_offset": 1} if args["offset"] == 0
                            else {"nodes": [{"name": "B", "node_id": 21}], "next_offset": None})
                return super().tool_or_die(tool, args)
        self.assertEqual(node_paths.resolve(Paged(), 1, "B"), 21)


class ApplyTest(unittest.TestCase):
    def test_every_label_is_set_on_the_node_its_path_names(self):
        mcp = FakeMcp()
        n = node_paths.apply_labels(mcp, 1, {"Review button/Label": "Review", "Open item/Label": "Open", "Open item/Shortcut slot/Shortcut": "Ctrl+O"})
        self.assertEqual((n, mcp.updates), (3, [(6, "Review"), (7, "Open"), (9, "Ctrl+O")]))

    def test_nothing_is_set_when_a_path_fails(self):
        mcp = FakeMcp()
        with self.assertRaises(SystemExit):
            node_paths.apply_labels(mcp, 1, {"Review button/Label": "Review", "Nope/Label": "x"})
        self.assertEqual(mcp.updates, [])

    def test_no_labels_sets_nothing(self):
        mcp = FakeMcp()
        self.assertEqual((node_paths.apply_labels(mcp, 1, None), node_paths.apply_labels(mcp, 1, {}), mcp.updates), (0, 0, []))

    def test_a_label_text_must_be_a_string(self):
        with self.assertRaises(SystemExit):
            node_paths.apply_labels(FakeMcp(), 1, {"Open item/Label": 3})


class DocTest(unittest.TestCase):
    def test_crops_json_documents_labels(self):
        doc = json.loads(CROPS.read_text())["_doc"]
        self.assertIn("`labels`", doc)
        self.assertIn("path", doc)

    def test_every_labels_entry_in_crops_json_maps_paths_to_strings(self):
        for e in json.loads(CROPS.read_text())["entries"]:
            if "labels" in e:
                self.assertIsInstance(e["labels"], dict, e)
                self.assertTrue(all(isinstance(k, str) and k and not k.startswith("/") and isinstance(v, str) for k, v in e["labels"].items()), e)

    def test_the_compare_applies_labels_before_it_renders(self):
        src = (HERE.parent / "studio_ui_file" / "evidence" / "component_compare.py").read_text()
        self.assertIn('node_paths.apply_labels(mcp, vid, e.get("labels"))', src)
        self.assertLess(src.index("node_paths.apply_labels"), src.index("render_with_icon(mcp, ws, out, e[\"set\"]"))


if __name__ == "__main__":
    unittest.main()
