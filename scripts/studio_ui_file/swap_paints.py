"""The paint a swap keeps: what designos #1427 drops from a swapped icon glyph inside a nested instance, written back in a generated file, and the same paint for every copy of it.

Swapping the `Icon` of a nested control (`X(.., props={"Icon": "Plus"})`) makes the engine rebuild the slot from the new Icon main (0.7.5, #1427, open): the glyph takes
that main's own fill, white at opacity 1 and bound to `color/icon`, so a Disabled control's 0.5 glyph or a Default control's `color/icon-quaternary` is lost. Figma keeps
the overrides set on the old instance's layers across a swap (researcher-261031-1130-figma-instance-swap-rules.md) and #1427 will do the same in the engine. Until then
the generator writes what the swap dropped, in the two places 0.7.5 allows (a `fills` write clears the node's binding, and no MCP write records an override):

    counts = apply_in_session(mcp, workspace)   live session, after layout: for every target `designos_update_node fills` gives paint 0 the pre-swap glyph's solid
                                                colour and opacity (the editor's own write; the glyph's other paints, visibility and blend stay). {"glyphs", "copies", "paints"}
    counts = record(doc)                        native post-pass, after the bind intents and before mirror_sources: the pre-swap glyph's `fills/0` binding (every target) and
                                                the `Fills` flag (swapped glyphs only), as size_overrides records `Size`. {"glyphs", "copies", "bindings", "flags"}. Raises when a paint was not written.
    problems = missing(doc)                     what is still dropped ([] once both ran); the library build asserts this on the reopened, saved file

The targets are in swap_targets: a swapped glyph (a slot copied with another Icon main; its pre-swap glyph is the one it copied) and a chain copy (a copy, at any depth, of a
glyph that is itself a target: a composite that nests a composite holding a swap keeps the paint from before the pass corrected it, and takes it from the glyph it copies). They
are settled sources first. A glyph that already carries the paint, binding and (a swapped glyph) flag is untouched (an engine that keeps overrides across a swap), so nothing is written.
The flag of a swapped glyph is recorded only when the kept paint or binding differs from the glyph's own source. A chain copy never gets one: the editor clears every override flag on a
linked copy (`copy_rec`) and flags only the node a user edits, so a copy carries paint 0 and its binding and mirrors the glyph it copies, and a flag would freeze it against its main.
"""
import copy
import json
import pathlib

import swap_targets as tg
from swap_targets import chain_copies, swapped  # noqa: F401  (the pass's targets; the tests and the verify read them here)

OVERRIDE = "Fills"
KEY = tg.KEY
SAVED = "swap-paints.designos"
TOL = tg.TOL


def _show(kind, opacity):
    c = kind["Solid"]
    return "#{:02X}{:02X}{:02X} at opacity {:g}".format(*(round(c[k] * 255) for k in "rgb"), opacity)


def _flagged(n):
    return OVERRIDE in ((n.get("component") or {}).get("overrides") or [])


def _drops(doc, glyph, kind, opacity, binding):
    """True when what a swapped glyph keeps differs from what its own source main gives it: only then would a main sync reset it, so only then it carries the flag."""
    own = doc["nodes"].get(str((glyph.get("component") or {}).get("source")))
    own_paint = tg.paint0(own) if own is not None else None
    return own is not None and (not tg.solid(own_paint) or not tg.close(own_paint["kind"], kind) or abs(own_paint.get("opacity", 1.0) - opacity) > TOL
                                or (binding is not None and tg.binding(doc, own) != binding))


def _gaps_of(doc, glyph, pre, swap=True):
    """[(field, problem)] of one target: `paint` (paint 0's colour or opacity), `binding` (`fills/0`) and `flag` (`Fills`, a swapped glyph only), against `pre`, the pre-swap glyph of a swapped glyph
    or the glyph a chain copy copies."""
    want, mine, name = tg.paint0(pre), tg.paint0(glyph), tg.path(doc, glyph)
    if not tg.solid(want) or not tg.solid(mine):
        return [("paint", f"{name}: cannot keep the pre-swap paint, glyph {pre['id']} or this one has no solid first paint")]
    of = "the pre-swap glyph" if swap else "the glyph it copies"
    kind, opacity, binding, out = want["kind"], want.get("opacity", 1.0), tg.binding(doc, pre), []
    if not (tg.close(mine["kind"], kind) and abs(mine.get("opacity", 1.0) - opacity) <= TOL):
        out.append(("paint", f"{name}: paint 0 is {_show(mine['kind'], mine.get('opacity', 1.0))}, {of} {pre['id']} has {_show(kind, opacity)}"))
    if binding is not None and tg.binding(doc, glyph) != binding:
        out.append(("binding", f"{name}: {KEY} is bound to {tg.binding(doc, glyph)}, {of} {pre['id']} has {binding}"))
    if swap and not _flagged(glyph) and _drops(doc, glyph, kind, opacity, binding):
        out.append(("flag", f"{name}: keeps the pre-swap paint but has no {OVERRIDE} override, so the next main sync resets it to its main's"))
    return out


def _counts(plan):
    return {"glyphs": sum(1 for *_, swap in plan if swap), "copies": sum(1 for *_, swap in plan if not swap)}


def missing(doc):
    return [{"id": g["id"], "what": "swapped glyph" if swap else "chain copy", "field": f, "problem": p}
            for g, pre, swap in tg.targets(doc) for f, p in _gaps_of(doc, g, pre, swap)]


def write_paints(mcp, doc):
    """Give paint 0 of every target (a swapped glyph, then its copies) the pre-swap glyph's solid colour and opacity through `designos_update_node fills`; `doc` is a saved
    copy of the live session and is updated with what was written, so a glyph that copies a corrected one sees the corrected paint. Returns {"glyphs", "copies", "paints"}."""
    plan, written = tg.targets(doc), 0
    for glyph, pre, swap in plan:
        gaps = _gaps_of(doc, glyph, pre, swap)
        bad = [p for f, p in gaps if f == "paint" and "cannot keep" in p]
        if bad:
            raise SystemExit("swap paints: " + bad[0])
        if any(f == "paint" for f, _ in gaps):
            want, fills = tg.paint0(pre), copy.deepcopy(glyph["fills"])
            fills[0] = dict(fills[0], kind=copy.deepcopy(want["kind"]), opacity=want.get("opacity", 1.0))
            mcp.tool_or_die("designos_update_node", {"node_id": glyph["id"], "fills": fills, "lint": "off"})
            glyph["fills"] = fills
            written += 1
    return {**_counts(plan), "paints": written}


def apply_in_session(mcp, workspace):
    """Save the live session into `workspace`, read it back and run `write_paints` on it."""
    mcp.tool_or_die("designos_save_file", {"path": SAVED, "overwrite": True})
    return write_paints(mcp, json.loads((pathlib.Path(workspace) / SAVED).read_text(encoding="utf-8")))


def record(doc):
    """Write the pre-swap `fills/0` binding and the `Fills` flag on every target that lacks them. Paint 0 must already be written (`apply_in_session`): a glyph that still
    differs there stops the build, because the generator never patches a paint into the saved file."""
    plan, unwritten = tg.targets(doc), []
    counts = {**_counts(plan), "bindings": 0, "flags": 0}
    for glyph, pre, swap in plan:
        for field, problem in _gaps_of(doc, glyph, pre, swap):
            if field == "paint":
                unwritten.append(problem)
            elif field == "binding":
                var = glyph.setdefault("variables", {})
                var.setdefault("modes", {})
                var.setdefault("bindings", {})[KEY] = tg.binding(doc, pre)
                counts["bindings"] += 1
            elif field == "flag":
                glyph.setdefault("component", {}).setdefault("overrides", []).append(OVERRIDE)
                counts["flags"] += 1
    if unwritten:
        raise SystemExit(f"{len(unwritten)} swapped glyphs lost the pre-swap paint and it was not written through MCP:\n" + "\n".join(unwritten[:25]))
    return counts
