"""Small MCP helpers shared by the assemblers: batched writes and node lookups by name."""


def batch(mcp, ops):
    return mcp.tool_or_die("designos_apply_batch", {"ops": ops, "lint": "off"})


def ids_by_name(mcp, query, parent=None):
    out, off = {}, 0
    while True:
        r = mcp.tool_or_die("designos_list_nodes", {"query": query, "offset": off, "limit": 500, **({"parent_id": parent} if parent else {})})
        for n in r["nodes"]:
            out.setdefault(n["name"], []).append(n["node_id"])
        if r["next_offset"] is None:
            return out
        off = r["next_offset"]


def icon_glyphs(mcp, icons_section):
    """Node ids of the `Glyph` vector inside each `Icon/*` component of the Icons section. A `Glyph` inside an instance elsewhere
    (the component pages nest icons) is not one of them."""
    comps, off = set(), 0
    while True:
        r = mcp.tool_or_die("designos_list_nodes", {"parent_id": icons_section, "offset": off, "limit": 500})
        comps |= {n["node_id"] for n in r["nodes"] if n["name"].startswith("Icon/")}
        if r["next_offset"] is None:
            break
        off = r["next_offset"]
    out, off = [], 0
    while True:
        r = mcp.tool_or_die("designos_list_nodes", {"query": "Glyph", "offset": off, "limit": 500})
        out += [n["node_id"] for n in r["nodes"] if n["name"] == "Glyph" and n["parent_id"] in comps]
        if r["next_offset"] is None:
            return out
        off = r["next_offset"]
