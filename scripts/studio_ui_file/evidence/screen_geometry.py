"""G3 geometry of a screen, read from the saved native file (no Pillow, no MCP, so it runs anywhere): every named widget rect of the capture's layout.json is found on the screen's node
  of that name.

The capture's layout.json (`named`: key -> rect in window px) records the rect of each widget the 0.7.5 painter named. A screen is instances of regions whose children carry those keys as
names, so the check is a lookup by name below the screen frame: the node's rect (the sum of its ancestors' offsets, the screen's own origin at 0, 0) equals the layout rect within `TOL` px on x,
y, width and height. A name that several nodes share matches when one of them does (`scrollbar-track` is one per scrolling list). A widget a region cannot name by its own node (a rect inside a
nested instance whose main names it otherwise) is given an alias: the path of names below the screen, joined by ` > `. The keys a screen cannot name are listed with the reason, never dropped:

    OFF_WINDOW   the rect lies outside the window (a list row scrolled out)        ZERO_SIZE   a recorded anchor with no size
    HIT_ONLY     a press target or a band that paints nothing                      KIT_NAMED   the key is a kit's own style or folder id; the row is drawn with neutral names
    SAME_RECT    another key already names this rect                               NO_NODE     no node of that name

    rects = node_rects(doc, "Screen/EditorDesign")        {node name: [(x, y, w, h), ...]} of every node below the screen, in the screen's coordinates
    report = check(doc, screen, layout, aliases=None)     {"keys", "matched": [key], "mismatched": [{"key", "layout", "node"}], "not_named": [{"key", "reason"}]}
"""
import re

TOL = 1.0
PATH_SEP = " > "
OFF_WINDOW = "the rect lies outside the window (a row scrolled out of the list)"
ZERO_SIZE = "the recorded rect has no size (an anchor the painter keeps for tests)"
HIT_ONLY = "a press target or band that paints nothing: a screen draws no hit areas"
KIT_NAMED = "the key is a kit's own style or folder id: the row is drawn with a neutral name"
SAME_RECT = "the same rect is already named by "
NO_NODE = "no node of this name below the screen"
HIT = re.compile(r"(-hit(-|$)|-band$|-handle$|separator)")


def _kind(n):
    k = n["kind"]
    return next(iter(k)) if isinstance(k, dict) else k


def _screen(doc, name):
    for pid in doc["pages"]:
        for c in doc["nodes"][str(pid)].get("children", []):
            n = doc["nodes"][str(c)]
            for cand in ([n] + [doc["nodes"][str(sc)] for sc in n.get("children", [])] if _kind(n) == "Section" else [n]):
                if cand["name"] == name and _kind(cand) != "Page":
                    return cand
    raise LookupError(f"no screen named {name!r} on any page of the file")


def _walk(doc, node, x, y, path):
    for c in node.get("children", []):
        n = doc["nodes"][str(c)]
        nx, ny = x + n["transform"]["tx"], y + n["transform"]["ty"]
        here = path + (n["name"],)
        yield here, (nx, ny, n["size"]["x"], n["size"]["y"])
        yield from _walk(doc, n, nx, ny, here)


def node_rects(doc, screen):
    """{node name: [(x, y, w, h)]} of every node below `screen`, in the screen's coordinates."""
    out = {}
    for path, rect in _walk(doc, _screen(doc, screen), 0.0, 0.0, ()):
        out.setdefault(path[-1], []).append(rect)
    return out


def path_rects(doc, screen, path):
    """The rects of the nodes at `path` (names joined by `PATH_SEP`: a record's own name holds `/`) below `screen`; an unknown path raises LookupError."""
    want = tuple(path.split(PATH_SEP))
    found = [rect for p, rect in _walk(doc, _screen(doc, screen), 0.0, 0.0, ()) if p == want]
    if not found:
        raise LookupError(f"no node at {path!r} below {screen!r}")
    return found


def _close(a, b):
    return all(abs(p - q) <= TOL for p, q in zip(a, b))


def _off_window(rect, size):
    x, y, w, h = rect
    return x >= size[0] or y >= size[1] or x + w <= 0 or y + h <= 0


def _unnamable(key, rect, size):
    if rect[2] <= 0 or rect[3] <= 0:
        return ZERO_SIZE
    if _off_window(rect, size):
        return OFF_WINDOW
    if HIT.search(key):
        return HIT_ONLY
    if key.startswith(("folder:", "style:")):
        return KIT_NAMED
    return None


def check(doc, screen, layout, aliases=None):
    aliases, rects = aliases or {}, node_rects(doc, screen)
    matched, mismatched, not_named, seen, unmatched = [], [], [], {}, []
    for key, entry in layout["named"].items():
        rect = list(entry["rect"])
        why = _unnamable(key, rect, layout["size"])
        if why:
            not_named.append({"key": key, "reason": why})
            continue
        found = path_rects(doc, screen, aliases[key]) if key in aliases else rects.get(key)
        if not found:
            unmatched.append((key, rect))
            continue
        seen[key] = rect
        if any(_close(rect, r) for r in found):
            matched.append(key)
        else:
            nearest = min(found, key=lambda r: sum(abs(p - q) for p, q in zip(rect, r)))
            mismatched.append({"key": key, "layout": rect, "node": [round(v, 3) for v in nearest]})
    for key, rect in unmatched:  # a key with no node of its name: another key on the same rect (before or after it) names the widget, else nothing does
        twin = next((k for k, r in seen.items() if r == rect), None)
        not_named.append({"key": key, "reason": SAME_RECT + twin if twin else NO_NODE})
    return {"keys": len(layout["named"]), "matched": matched, "mismatched": mismatched, "not_named": not_named}
