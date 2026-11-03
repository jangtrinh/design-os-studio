"""Edits MCP cannot make on a saved native `.designos` (verified to survive a reopen in 0.7.5):

* `code_syntax` of every Semantic variable = the Rust constant (platform key WEB, the only
  one a web/dev reader shows; recorded in the report). set_variables has no code-syntax field.
* `style_meta[style].bindings` = font size, weight, line height and family variables of each
  text style. set_style has no binding field.

* bind intents `{"sid", "key", "var"}` of the component modules: `bindings[key]` = the Semantic variable's id, or, for
  key `text_style`, the node's `style_refs.text`. The node is found by `source_id` (the REST id); a missing node, an unknown
  variable or style is fatal, and a node bound twice to different variables is too. Nothing is dropped silently.
* the editor's sync for nested instances: every node that mirrors a main node takes the main's bindings and text style it does not carry itself, and the main's
  allow-list entries (mirror_sources).
* the `Size` and `CornerRadii` overrides of every top instance inside a main that is resized or pinned against its main, and of the glyph of a resized icon slot,
  as the editor records them (size_overrides).
* the `fills/0` binding and the `Fills` flag a swapped icon glyph keeps from its pre-swap glyph, and the binding a chain copy of one takes from the glyph it copies; a chain copy
  gets no flag, as the editor clears every override flag on a copy (swap_paints; their paint 0 was written through MCP before the save). It runs after the bind intents (the
  pre-swap glyph is bound) and before the mirror, which only fills a binding the node lacks and would stamp the new Icon main's.

    post_pass(src_path, dst_path, plan, style_bindings, intents=(), allow=()) -> counts
    counts["allow"] is the allow-list entries given plus the ones mirrored: the caller writes and checks that list, not the one it passed in.
"""
import json
import pathlib

import mirror_sources
import size_overrides
import swap_paints


def post_pass(src, dst, plan, style_bindings, intents=(), allow=()):
    doc = json.loads(pathlib.Path(src).read_text(encoding="utf-8"))
    variables, meta = doc["variables"], doc["style_meta"]
    for s in plan["semantic"]:
        if s["id"] not in variables:
            raise SystemExit(f"post-pass: semantic variable {s['id']} missing from the saved file")
        variables[s["id"]]["code_syntax"] = {"WEB": s["code_syntax"]}
    bound = 0
    for style, props in style_bindings.items():
        if style not in meta:
            raise SystemExit(f"post-pass: text style {style!r} missing from the saved file")
        missing = [v for v in props.values() if v not in variables]
        if missing:
            raise SystemExit(f"post-pass: style {style!r} binds unknown variables {missing}")
        meta[style]["bindings"] = dict(props)
        bound += len(props)
    applied = apply_intents(doc, plan, intents)
    swaps = swap_paints.record(doc)
    mirrored, allow = mirror_sources.mirror(doc, allow)
    sizes = size_overrides.record(doc)
    pathlib.Path(dst).write_text(json.dumps(doc), encoding="utf-8")
    return {"code_syntax": len(plan["semantic"]), "style_bindings": bound, **applied, "mirrored": mirrored, "size_overrides": sizes, "swap_paints": swaps, "allow": allow}


def apply_intents(doc, plan, intents):
    by_sid = {n["source_id"]: n for n in doc["nodes"].values() if n.get("source_id")}
    sem = {s["name"]: s["id"] for s in plan["semantic"]}
    counts = {"bindings": 0, "text_styles": 0}
    for it in intents:
        node = by_sid.get(it["sid"])
        if node is None:
            raise SystemExit(f"post-pass: intent for {it['sid']} ({it['key']}) found no node with that source_id")
        if it["key"] == "text_style":
            if it["var"] not in doc["style_meta"]:
                raise SystemExit(f"post-pass: text style {it['var']!r} is not in the file ({it['sid']})")
            node["style_refs"] = {**(node.get("style_refs") or {}), "text": it["var"]}
            counts["text_styles"] += 1
            continue
        if it["var"] not in sem or sem[it["var"]] not in doc["variables"]:
            raise SystemExit(f"post-pass: Semantic variable {it['var']!r} is not in the file ({it['sid']} {it['key']})")
        bound = node.setdefault("variables", {"bindings": {}, "modes": {}}).setdefault("bindings", {})
        if bound.get(it["key"], sem[it["var"]]) != sem[it["var"]]:
            raise SystemExit(f"post-pass: {it['sid']} {it['key']} is bound to {bound[it['key']]} and to {sem[it['var']]}")
        bound[it["key"]] = sem[it["var"]]
        counts["bindings"] += 1
    return counts
