"""The editor's sync for a generated file: a node that mirrors a main node takes the main's bindings, style refs and waivers.

The importer expands a nested instance before any binding exists, so its copies hold the main's literals and none of its bindings or text style (probed on
designos-mcp 0.7.5: G2 flagged the unbound fills and the missing text style on every copied child). `mirror` fills that in after the fact; the override gate
then finds the instance equal to its main. Synthetic native documents.

Run: python3 -B -m unittest scripts.tests.test_studio_ui_file_mirror_sources
"""
import copy
import json
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
sys.path.insert(0, str(HERE.parent))
import mirror_sources as ms  # noqa: E402
import native_post_pass as post  # noqa: E402
from studio_ui_gate_overrides import check_overrides  # noqa: E402
from studio_ui_override_fixtures import BOX, HOST, IBOX, INST, ITEXT, MAIN, TEXT, base, mirror as copy_node  # noqa: E402




def expanded():
    """`base()` as the importer leaves it: the instance and its children hold the main's literals but no bindings and no style refs."""
    d = base()
    for nid in (INST, ITEXT, IBOX):
        n = d["nodes"][str(nid)]
        n.pop("variables", None)
        n.pop("style_refs", None)
    return d


def bindings(d, nid):
    return (d["nodes"][str(nid)].get("variables") or {}).get("bindings")


class MirrorTest(unittest.TestCase):
    def test_a_copy_takes_the_mains_bindings_and_text_style(self):
        d = expanded()
        counts, _ = ms.mirror(d)
        self.assertEqual(bindings(d, ITEXT), bindings(d, TEXT))
        self.assertEqual(bindings(d, IBOX), bindings(d, BOX))
        self.assertEqual(bindings(d, INST), bindings(d, MAIN))
        self.assertEqual(d["nodes"][str(ITEXT)]["style_refs"], {"text": "Text sm / Semibold"})
        self.assertEqual(counts["style_refs"], 1)
        self.assertEqual(counts["bindings"], len(bindings(d, TEXT)) + len(bindings(d, BOX)) + len(bindings(d, MAIN)))

    def test_the_override_gate_fails_before_and_passes_after(self):
        d = expanded()
        self.assertTrue(any("is bound to nothing" in f for f in check_overrides(d)[0]))
        ms.mirror(d)
        self.assertEqual(check_overrides(d)[0], [])

    def test_a_binding_the_node_carries_itself_wins(self):
        d = expanded()
        d["nodes"][str(IBOX)]["variables"] = {"modes": {}, "bindings": {"fills/0": "s.accent"}}
        ms.mirror(d)
        self.assertEqual(bindings(d, IBOX)["fills/0"], "s.accent")
        self.assertEqual(bindings(d, IBOX)["itemSpacing"] if "itemSpacing" in bindings(d, IBOX) else None, bindings(d, BOX).get("itemSpacing"))

    def test_a_style_ref_the_node_carries_itself_wins(self):
        d = expanded()
        d["nodes"][str(ITEXT)]["style_refs"] = {"text": "Text lg / Bold"}
        ms.mirror(d)
        self.assertEqual(d["nodes"][str(ITEXT)]["style_refs"], {"text": "Text lg / Bold"})

    def test_a_second_run_changes_nothing(self):
        d = expanded()
        ms.mirror(d)
        once = copy.deepcopy(d)
        counts, _ = ms.mirror(d)
        self.assertEqual((counts["bindings"], counts["style_refs"], counts["allow"]), (0, 0, 0))
        self.assertEqual(d, once)

    def test_a_node_with_no_source_and_a_source_without_bindings_stay_as_they_are(self):
        d = expanded()
        before = copy.deepcopy(d["nodes"][str(HOST)])
        d["nodes"][str(TEXT)].pop("variables")
        d["nodes"][str(TEXT)].pop("style_refs")
        ms.mirror(d)
        self.assertEqual(d["nodes"][str(HOST)], before)
        self.assertNotIn("variables", d["nodes"][str(ITEXT)])

    def test_a_copy_of_a_copy_passes_the_bindings_on_however_deep(self):
        d = expanded()
        deep = copy_node(d, INST, 70, HOST)
        deep["component"] = {"source": INST}
        deep.pop("variables", None)
        d["nodes"][str(HOST)]["children"].append(70)
        d["nodes"]["70"]["children"] = [71]
        copy_node(d, ITEXT, 71, 70)["component"] = {"source": ITEXT}
        d["nodes"]["71"].pop("variables", None)
        d["nodes"]["71"].pop("style_refs", None)
        ms.mirror(d)
        self.assertEqual(bindings(d, 71), bindings(d, TEXT))
        self.assertEqual(d["nodes"]["71"]["style_refs"], {"text": "Text sm / Semibold"})

    def test_two_nodes_that_mirror_each_other_fail(self):
        d = expanded()
        d["nodes"][str(MAIN)]["component"] = {"source": INST}
        with self.assertRaises(ValueError):
            ms.mirror(d)


class AllowListTest(unittest.TestCase):
    ENTRY = {"path": "02 Core/Card/Main/Box", "property": "paddingLeft", "reason": "r", "value": 6.0}

    def doc(self):
        d = expanded()
        d["nodes"][str(MAIN)]["name"] = "Main"
        d["nodes"][str(INST)]["name"] = "Instance"
        d["nodes"]["80"] = {"id": 80, "name": "Card", "kind": {"ComponentSet": {}}, "parent": 3, "children": [MAIN]}
        d["nodes"][str(MAIN)]["parent"] = 80
        d["nodes"][str(HOST)]["parent"] = 3
        d["nodes"]["3"]["children"] = [40, 42, 43, 80, HOST]
        return d

    def test_an_entry_on_the_main_is_repeated_on_each_copy(self):
        d = self.doc()
        _, allow = ms.mirror(d, [self.ENTRY])
        paths = [a["path"] for a in allow]
        self.assertEqual(paths, ["02 Core/Card/Main/Box", "02 Core/Host/Instance/Box"])
        self.assertEqual({k: v for k, v in allow[1].items() if k != "path"}, {k: v for k, v in self.ENTRY.items() if k != "path"})

    def test_an_entry_the_copy_already_has_is_not_repeated(self):
        d = self.doc()
        own = dict(self.ENTRY, path="02 Core/Host/Instance/Box")
        _, allow = ms.mirror(d, [self.ENTRY, own])
        self.assertEqual(len(allow), 2)

    def test_an_entry_on_a_node_with_no_copy_stays_alone(self):
        _, allow = ms.mirror(self.doc(), [dict(self.ENTRY, path="02 Core/Host")])
        self.assertEqual(len(allow), 1)

    def test_a_page_01_entry_is_not_walked(self):
        d = self.doc()
        d["nodes"]["4"]["component"] = {"source": TEXT}  # the Swatch on page 01 mirrors a node: it is not walked
        _, allow = ms.mirror(d, [dict(self.ENTRY, path="02 Core/Card/Main/Label")])
        self.assertEqual([a["path"] for a in allow], ["02 Core/Card/Main/Label", "02 Core/Host/Instance/Label"])

    def test_a_copy_of_a_copy_gets_the_entry_too(self):
        d = self.doc()
        deep = copy_node(d, INST, 70, HOST)
        deep["component"] = {"source": INST}
        deep["name"] = "Deep"
        deep["children"] = [72]
        d["nodes"][str(HOST)]["children"].append(70)
        copy_node(d, IBOX, 72, 70)["component"] = {"source": IBOX}
        _, allow = ms.mirror(d, [self.ENTRY])
        self.assertEqual([a["path"] for a in allow], ["02 Core/Card/Main/Box", "02 Core/Host/Instance/Box", "02 Core/Host/Deep/Box"])


class PostPassTest(unittest.TestCase):
    def test_the_post_pass_mirrors_and_returns_the_full_allow_list(self):
        d = {**expanded(), "variables": {**expanded()["variables"]}, "style_meta": {}}
        with tempfile.TemporaryDirectory() as tmp:
            src, dst = Path(tmp) / "in.designos", Path(tmp) / "out.designos"
            src.write_text(json.dumps(d), encoding="utf-8")
            counts = post.post_pass(src, dst, {"semantic": []}, {}, [], [{"path": "02 Core/Card/x", "property": "gap", "reason": "r", "value": 1.0}])
            written = json.loads(dst.read_text(encoding="utf-8"))
        self.assertGreater(counts["mirrored"]["bindings"], 0)
        self.assertEqual(len(counts["allow"]), 1)
        self.assertEqual(written["nodes"][str(ITEXT)]["variables"]["bindings"], d["nodes"][str(TEXT)]["variables"]["bindings"])


if __name__ == "__main__":
    unittest.main()
