"""Where a crop's `render_rect` sits in the built component, checked on the saved file without Pillow or MCP.

The compare cuts a window out of a render (`render_rect`) and sets it against a production crop. The compare is position-blind inside that window (a 2 px shift of the window still passes),
so the window is tied to the component's own geometry: every entry with a `render_rect` names the node it is cut from (`render_node`, node names joined by `/` below the variant, "" for the
variant itself) and, when the window is only a part of that node, how far it is trimmed from the node's edges (`render_trim`, [left, top, right, bottom] in px).
The rect must equal the node's bounds (in the variant's coordinates) trimmed by those amounts, within `TOL`. A window is pinned at its top-left corner, so `left` and `top` are always numbers;
a `right` or `bottom` that is `null` leaves that far edge free (the window is a production-sized part of the node), but never outside it: the far edge must lie within the node.
A rect that is an exact node has no `render_trim`.

    problems = check(doc, entries)     [] when every rect is where its node puts it; one line per entry that is not, names no node, or names one that is missing or ambiguous
    bounds(doc, variant_node, path)    (x, y, w, h) of the node at `path` below the variant, in the variant's coordinates
"""
TOL = 0.5


def _kind(n):
    k = n["kind"]
    return next(iter(k)) if isinstance(k, dict) else k


def variants(doc):
    """{(set name, variant name): node} of every component set (its children) and every plain component (named "") that sits in a section."""
    nodes, out = doc["nodes"], {}
    for n in nodes.values():
        if _kind(n) == "ComponentSet":
            for c in n.get("children", []):
                out[(n["name"], nodes[str(c)]["name"])] = nodes[str(c)]
        elif _kind(n) == "Component" and n.get("parent") is not None and _kind(nodes[str(n["parent"])]) == "Section":
            out[(n["name"], "")] = n
    return out


def bounds(doc, root, path):
    """(x, y, w, h) of the node at `path` below `root` in the root's coordinates. A missing or ambiguous name raises LookupError naming the children."""
    nodes, node, x, y = doc["nodes"], root, 0.0, 0.0
    for name in [p for p in path.split("/") if p] if path else []:
        kids = [nodes[str(c)] for c in node.get("children", [])]
        found = [k for k in kids if k["name"] == name]
        if len(found) != 1:
            raise LookupError(f"{name!r} is {'ambiguous' if found else 'not a child'} of {node['name']!r} (it has {[k['name'] for k in kids]})")
        node = found[0]
        x, y = x + node["transform"]["tx"], y + node["transform"]["ty"]
    return x, y, node["size"]["x"], node["size"]["y"]


def _is_number(t):
    return isinstance(t, (int, float)) and not isinstance(t, bool)


def _trim_problem(trim):
    ok = isinstance(trim, list) and len(trim) == 4 and _is_number(trim[0]) and _is_number(trim[1]) and all(t is None or _is_number(t) for t in trim[2:])
    return None if ok else f"render_trim must be [left, top, right, bottom]: left and top numbers (a window is pinned at its top-left corner), right and bottom numbers or null, got {trim!r}"


def check_entry(by_variant, doc, e):
    """The problems (a list of text) of one crops.json entry."""
    where = f"{e['set']} {e['variant']} {e['case']}/{e.get('named') or e.get('rect')}"
    if "render_rect" not in e:
        return [f"{where}: names a render_node or render_trim but has no render_rect"] if ("render_node" in e or "render_trim" in e) else []
    if not isinstance(e.get("render_node"), str):
        return [f"{where}: has a render_rect but names no render_node (the node, by path below the variant, the window is cut from)"]
    trim = e.get("render_trim", [0, 0, 0, 0])
    if _trim_problem(trim):
        return [f"{where}: {_trim_problem(trim)}"]
    root = by_variant.get((e["set"], e["variant"]))
    if root is None:
        return [f"{where}: the file has no {e['set']} / {e['variant']!r}"]
    try:
        nx, ny, nw, nh = bounds(doc, root, e["render_node"])
    except LookupError as err:
        return [f"{where}: render_node {e['render_node']!r}: {err}"]
    x, y, w, h = e["render_rect"]
    box, name = [nx, ny, nw, nh], e["render_node"] or "the variant"
    head = f"{where}: render_rect {e['render_rect']} has its"
    off = lambda side, got, want: f"{head} {side} edge at {got:g}, but {name!r} trimmed by {trim} puts it at {want:g} (the node is {box})"
    out = [off(side, got, want) for side, got, want in (("left", x, nx + trim[0]), ("top", y, ny + trim[1])) if abs(got - want) > TOL]
    for side, got, edge, cut in (("right", x + w, nx + nw, trim[2]), ("bottom", y + h, ny + nh, trim[3])):
        if cut is not None and abs(got - (edge - cut)) > TOL:
            out.append(off(side, got, edge - cut))
        elif cut is None and got > edge + TOL:
            out.append(f"{head} free {side} edge at {got:g}, past the {side} edge of {name!r} ({edge:g}): a free edge stays inside the node (the node is {box})")
    return out


def check(doc, entries):
    by_variant = variants(doc)
    return [p for e in entries for p in check_entry(by_variant, doc, e)]
