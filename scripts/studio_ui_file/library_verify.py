"""Reopen the saved file through MCP and prove every bind intent landed.

    counts = verify(mcp, native_file, plan, intents, recorded)

Variable bindings: for each node, `designos_get_variables` must list every intent key in `effective_bindings` with the intent's
Semantic variable id. Text styles are not variable bindings and never show there: they are checked on the node's `style_refs`
in the saved file and counted on their own line. A resized icon slot or glyph inside a main must still carry its `Size` override (size_overrides), and a swapped
glyph its pre-swap paint, binding and `Fills` flag, and every chain copy of one its paint and binding, never a flag (swap_paints): the reopen and save must not drop them. Any miss is fatal and names the node
and property; nothing is dropped. `recorded` is the swap pass's own count ({"glyphs": ..., "copies": ...}, as written to the document): the reopened file must hold exactly that
many swapped glyphs and chain copies, because a reopen that lost the `component.source` links leaves nothing for `swap_paints.missing` to check. None skips that
cross-check (the other passes' tests).
"""
import collections
import json
import pathlib

import size_overrides
import swap_paints


def verify(mcp, path, plan, intents, recorded=None):
    doc = json.loads(pathlib.Path(path).read_text(encoding="utf-8"))
    ids = {n["source_id"]: (int(i), n) for i, n in doc["nodes"].items() if n.get("source_id")}
    sem = {s["name"]: s["id"] for s in plan["semantic"]}
    want, styles, misses = collections.defaultdict(dict), [], []
    for it in intents:
        if it["sid"] not in ids:
            misses.append(f"{it['sid']}: no node with this source_id in the saved file ({it['key']})")
        elif it["key"] == "text_style":
            styles.append(it)
        else:
            want[it["sid"]][it["key"]] = sem[it["var"]]
    checked = 0
    for sid, keys in want.items():
        nid, node = ids[sid]
        r = mcp.tool_or_die("designos_get_variables", {"node_id": nid})
        have = {b["property"]: b["variable_id"] for b in r.get("effective_bindings", [])}
        for key, var in keys.items():
            checked += 1
            if have.get(key) != var:
                misses.append(f"{sid} ({node['name']}): {key} should be {var}, effective_bindings has {have.get(key)}")
    for it in styles:
        nid, node = ids[it["sid"]]
        if (node.get("style_refs") or {}).get("text") != it["var"]:
            misses.append(f"{it['sid']} ({node['name']}): text style should be {it['var']!r}, node has {node.get('style_refs')}")
    misses += [f"{m['name']} ({m['what']} {m['id']}) {m['problem']}" for m in size_overrides.missing(doc)]
    misses += [f"{m['what']} {m['id']} ({m['field']}) {m['problem']}" for m in swap_paints.missing(doc)]
    if misses:
        raise SystemExit(f"{len(misses)} bind intents, Size overrides or swap paints did not survive the reopen:\n" + "\n".join(misses[:25]))
    swaps, copies = swap_paints.swapped(doc), swap_paints.chain_copies(doc)
    if recorded is not None and (len(swaps), len(copies)) != (recorded["glyphs"], recorded["copies"]):
        raise SystemExit(f"the reopened file has {len(swaps)} swapped glyphs and {len(copies)} chain copies, the build recorded {recorded['glyphs']} and {recorded['copies']}: "
                         "the reopen lost the `component.source` links the swap paints are read through")
    carrying = sum(1 for n in doc["nodes"].values() if size_overrides.OVERRIDE in ((n.get("component") or {}).get("overrides") or []))
    return {"bindings_verified": checked, "text_styles_verified": len(styles), "nodes_checked": len(want), "size_overrides_verified": carrying,
            "swap_paints_verified": len(swaps), "chain_copies_verified": len(copies)}
