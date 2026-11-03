"""Synthetic native documents for the chain copies of the swap-paint pass: composites that nest the composite HOST of test_studio_ui_file_swap_paints (not a test module).

`HOST` holds the swapped glyph `NEW_GLYPH`. `chain(depth)` adds `depth` composites, each nesting the one before it (the first nests HOST), expanded the way the REST import does it:
one copy of every node of the nested main, with `component.source` on the node it copies, the paints the main had at that moment, no variable binding and no override flag.
The editor flags a copy never: `copy_rec` clears every override flag on a linked copy, so a settled copy (`settle`) carries paint 0 and its binding and no flag of its own.
"""
import copy
import itertools
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from test_studio_ui_file_swap_paints import GREY, HOST, NEW_GLYPH, PAGE, inst, node, paint, reset_glyph  # noqa: E402


def nest(d, main, parent, ids, name="Header"):
    """An instance of the composite `main` inside `parent`, with a copy of its whole subtree. Returns {id of a node of the main: id of its copy}."""
    made = {}

    def expand(src_id, parent_id, root=False):
        new = copy.deepcopy(d["nodes"][str(src_id)])
        new.update(id=next(ids), parent=parent_id, children=[], component={"source": src_id}, **({"name": name} if root else {}))
        new.pop("variables", None)
        if root:
            new["kind"] = inst(main)
        d["nodes"][str(new["id"])] = new
        d["nodes"][str(parent_id)]["children"].append(new["id"])
        made[src_id] = new["id"]
        for kid in d["nodes"][str(src_id)]["children"]:
            expand(kid, new["id"])

    expand(main, parent, root=True)
    return made


def correct_swap(d):
    """The swapped glyph as the pass leaves it: paint 0 grey at 0.5 (MCP), bound to `s.quat` and flagged `Fills` (post-pass)."""
    g = d["nodes"][str(NEW_GLYPH)]
    g["fills"] = [paint(GREY, 0.5)]
    g["variables"] = {"bindings": {"fills/0": "s.quat"}, "modes": {}}
    g["component"]["overrides"] = ["Fills"]


def chain(depth=1, corrected=True, base=None, descending=False):
    """(doc, [glyph id of the copy at each level, the innermost first]). The copies are made while the swapped glyph is still reset (white, opacity 1, no binding, no flag);
    the swapped glyph is then corrected unless `corrected` is False (the pass has not run), so every copy is stale. `base` replaces the document that holds HOST. Node ids grow
    with the level, or shrink with it when `descending`: then a copy has a lower id than the glyph it copies."""
    d, shared, main, glyph, copies = base or reset_glyph(), itertools.count(100), HOST, NEW_GLYPH, []
    for level in range(depth):
        ids = itertools.count(100 - 10 * level) if descending else shared
        comp = next(ids)
        d["nodes"][str(comp)] = node(comp, f"Level {level}", {"Component": {}}, PAGE)
        d["nodes"][str(PAGE)]["children"].append(comp)
        glyph = nest(d, main, comp, ids)[glyph]
        copies.append(glyph)
        main = comp
    if corrected:
        correct_swap(d)
    return d, copies


def settle(d, glyph_id, opacity=0.5, binding=None, flag=False):
    """Give one copy the state the pass leaves: paint 0 grey at `opacity`, and the binding when asked. A copy carries no `Fills` flag unless asked (`flag`): the pass never gives one."""
    g = d["nodes"][str(glyph_id)]
    g["fills"] = [paint(GREY, opacity)]
    if binding:
        g["variables"] = {"bindings": {"fills/0": binding}, "modes": {}}
    if flag:
        g["component"]["overrides"] = ["Fills"]
    else:
        g["component"].pop("overrides", None)
