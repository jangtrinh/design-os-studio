"""Which glyphs the swap-paint pass (swap_paints) settles: the glyphs a swap changed (designos #1427) and the glyphs that copy one, in the order they can be settled.

An icon slot is an instance N of an `Icon/*` main; its glyph is its first child. N's `component.source` is S0, the slot it copies (an instance of an `Icon/*` main too), and the
glyph S0 holds is `pre`. Two kinds of glyph are targets:

    swapped glyph   N's main differs from S0's: the engine's swap rebuilt the slot from the new Icon main, so the glyph lost what `pre` carried (the paint, binding and flag).
    chain copy      N's main is S0's (a copy of a slot) and `pre` is itself a target. The REST import expands a composite's nested instances once, from the nested main as it
                    stands then, so a composite that nests a composite holding a swapped glyph keeps that glyph's paint from before the pass corrected it. Any depth: a copy
                    of a chain copy is a chain copy. Found by structure alone, never by what the glyph carries, so a copy the pass has already settled is still one.

A copy whose `pre` is no target (every slot a module draws directly, a plain copy of one) is left alone, so a file with no swap has no chain copy. A `pre` that is itself a
target is settled first, whatever kind either is: a re-swap inside a chain copy reads the copy's settled paint.

    targets(doc)       [(glyph, pre, swap)] in settle order, `swap` False for a chain copy; raises ValueError when a glyph is its own `pre`
    swapped(doc)       [(glyph, pre)] of the swapped glyphs;  chain_copies(doc)   [(glyph, pre)] of the chain copies
    instance, icon, first, paint0, solid, close, binding, path   the node readers the pass shares
"""
TOL = 1e-3
KEY = "fills/0"


def instance(n):
    kind = n.get("kind")
    inst = kind.get("Instance") if isinstance(kind, dict) else None
    return inst if inst and inst.get("component") is not None else None


def icon(doc, n):
    """The `Icon/*` main an instance draws, else None."""
    inst = instance(n)
    main = doc["nodes"].get(str(inst["component"])) if inst else None
    return main if main is not None and str(main.get("name", "")).startswith("Icon/") else None


def first(doc, n):
    kids = n.get("children") or []
    return doc["nodes"].get(str(kids[0])) if kids else None


def paint0(n):
    fills = n.get("fills") or []
    return fills[0] if fills else None


def solid(paint):
    return isinstance(paint, dict) and isinstance(paint.get("kind"), dict) and "Solid" in paint["kind"]


def close(a, b):
    if isinstance(a, dict) and isinstance(b, dict):
        return a.keys() == b.keys() and all(close(a[k], b[k]) for k in a)
    if isinstance(a, (int, float)) and isinstance(b, (int, float)) and not isinstance(a, bool):
        return abs(a - b) <= TOL
    return a == b


def binding(doc, n, seen=()):
    """The `fills/0` variable of `n`: its own, else the one its source has, else None."""
    own = ((n.get("variables") or {}).get("bindings") or {}).get(KEY)
    src = doc["nodes"].get(str((n.get("component") or {}).get("source")))
    if own or src is None or src["id"] in seen + (n["id"],):
        return own
    return binding(doc, src, seen + (n["id"],))


def path(doc, n):
    names = []
    while n is not None:
        names.append(n["name"])
        n = doc["nodes"].get(str(n.get("parent")))
    return "/".join(reversed(names))


def _slots(doc):
    """{glyph id: (glyph, pre, swap)} for every icon slot that copies another icon slot, before the chain filter."""
    found = {}
    for n in sorted(doc["nodes"].values(), key=lambda x: int(x["id"])):
        old = doc["nodes"].get(str((n.get("component") or {}).get("source")))
        if instance(n) is None or old is None or instance(old) is None:
            continue
        new_main, old_main, glyph, pre = icon(doc, n), icon(doc, old), first(doc, n), first(doc, old)
        if all(x is not None for x in (new_main, old_main, glyph, pre)):
            found[glyph["id"]] = (glyph, pre, new_main["id"] != old_main["id"])
    return found


def _reaches_swap(found, gid, trail=()):
    """True when the slot is a swap, or copies a slot whose glyph (through any number of copies) is one."""
    _, pre, swap = found[gid]
    return swap or (pre["id"] in found and pre["id"] not in trail + (gid,) and _reaches_swap(found, pre["id"], trail + (gid,)))


def targets(doc):
    slots = _slots(doc)
    found = {gid: v for gid, v in slots.items() if _reaches_swap(slots, gid)}
    order, done = [], set()

    def settle(gid, trail=()):
        if gid in done:
            return
        if gid in trail:
            raise ValueError(f"swapped glyph {gid} is its own pre-swap glyph")
        if found[gid][1]["id"] in found:
            settle(found[gid][1]["id"], trail + (gid,))
        done.add(gid)
        order.append(found[gid])

    for gid in found:
        settle(gid)
    return order


def swapped(doc):
    return [(g, p) for g, p, swap in targets(doc) if swap]


def chain_copies(doc):
    return [(g, p) for g, p, swap in targets(doc) if not swap]
