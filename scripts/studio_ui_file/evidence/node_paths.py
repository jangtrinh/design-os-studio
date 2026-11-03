"""Nodes of a variant by path, for the compare: `labels` in crops.json sets several texts, nested instances' texts included, and the sets of both component pages.

    set_variants(mcp, pages)                         {record name: {variant name: node id}} for every component set and plain component on pages 02 Core and 03 Studio;
                                                     a plain component (a record with no variants) is its own one variant, named ""
    resolve(mcp, root_id, "Review button/Label")     native id of the node, names joined by `/` below the root; an unknown, ambiguous or empty name fails loudly
    apply_labels(mcp, root_id, {path: text})         set each text on the scratch copy (every path is resolved first, so a bad one changes nothing); returns how many

A name two siblings share is ambiguous, not "the last one": a composite that nests the same set twice names its instances apart.
"""


COMPONENT_PAGES = ("02 Core", "03 Studio")
SET_KIND, PLAIN_KIND = "Component set", "Component"  # `kind` of designos_list_nodes (probed on 0.7.5: a set is spelled with a space)


def _listing(mcp, nid):
    out, off = [], 0
    while True:
        r = mcp.tool_or_die("designos_list_nodes", {"parent_id": nid, "offset": off, "limit": 500})
        out += r["nodes"]
        if r.get("next_offset") is None:
            return out
        off = r["next_offset"]


def _children(mcp, nid):
    return [(n["name"], n["node_id"]) for n in _listing(mcp, nid)]


def set_variants(mcp, pages):
    """The node kind decides, never the name: a ComponentSet's children are its variants, a plain Component is its own variant (its children are layers)."""
    out = {}
    for page in COMPONENT_PAGES:
        for sec in _listing(mcp, pages[page]) if page in pages else []:
            for n in _listing(mcp, sec["node_id"]):
                if n["kind"] == SET_KIND:
                    out[n["name"]] = {v["name"]: v["node_id"] for v in _listing(mcp, n["node_id"])}
                elif n["kind"] == PLAIN_KIND:
                    out[n["name"]] = {"": n["node_id"]}
    return out


def resolve(mcp, root_id, path):
    nid, trail = root_id, []
    for name in path.split("/"):
        if not name:
            raise SystemExit(f"labels: {path!r} has an empty name: write node names joined by single '/'")
        kids = _children(mcp, nid)
        found = [i for n, i in kids if n == name]
        where = "/".join(trail) or "the variant"
        if not found:
            raise SystemExit(f"labels: {path!r}: {name!r} is not a child of {where} (it has {[n for n, _ in kids]})")
        if len(found) > 1:
            raise SystemExit(f"labels: {path!r}: {name!r} is ambiguous, {len(found)} children of {where} have that name")
        nid, trail = found[0], trail + [name]
    return nid


def apply_labels(mcp, root_id, labels):
    todo = []
    for path, text in (labels or {}).items():
        if not isinstance(text, str):
            raise SystemExit(f"labels: {path!r} needs a string, got {text!r}")
        todo.append((resolve(mcp, root_id, path), text))
    for nid, text in todo:
        mcp.tool_or_die("designos_update_node", {"node_id": nid, "text": text})
    return len(todo)
