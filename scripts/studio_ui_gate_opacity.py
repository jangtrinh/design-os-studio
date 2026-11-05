"""G2 opacity: half alpha per paint. Every variant of a component set on `02 Core` or `03 Studio` named `State=Disabled` carries opacity 0.5 on
each visible paint (fill, stroke, text and glyph fills) and every other variant carries 1.0, because the kit disables a control
by halving each paint's alpha (`kit/mod.rs` half_color, `disabled`), never as a node or group opacity: no node inside a variant
carries an `opacity` (the engine applies it on top of the paints, `designos-doc` node.rs `Node::opacity`, so a 0.5 node would dim a
Disabled variant twice). There is no `opacity/disabled` variable: theme.rs has no constant for 0.5, so the rule lives here and on
page `00 Start Here`. A record with no variants is one plain Component: it counts as a variant that is not Disabled.

Disabled is sticky downward. A paint wants 0.5 when its variant, or any instance on the path from the variant root to the paint (the instance's own
paints included), is an instance of a `State=Disabled` variant; every other paint wants 1.0. A control draws itself at its own state wherever a composite
nests it: a Default composite may hold a Disabled control, and a Disabled composite still dims everything it holds, a nested Default control included.
Sticky never reaches a sibling or an ancestor of the nested instance.

A paint may carry another opacity only through an exact allow-list entry `{path, property: "fills/<i>/opacity" | "strokes/<i>/opacity", value, reason}` (the Ready-for-dev pill is
`DEV_READY.gamma_multiply(0.22)`: bound to its constant, opacity 0.22). The entry is compared like the literal entries of G2 bound: the stored opacity must equal the listed value (0.5 times it
where Disabled is sticky), a stale (no such paint), duplicate or valueless entry fails, and a listed value strictly between 0 and 1 is the only kind an entry can hold.
"""
from __future__ import annotations

from typing import List, Tuple

from studio_ui_gate_bound import paints_of
from studio_ui_gate_doc import is_opacity_entry, walk
from studio_ui_gate_registry import PAGES

DISABLED = "State=Disabled"
HALF = 0.5


def is_disabled(variant_name: str) -> bool:
    return DISABLED in [part.strip() for part in variant_name.split(",")]


def _disabled_instance(doc: dict, node: dict) -> bool:
    """True when `node` is an instance of a `State=Disabled` variant (a missing or unrelated main never is)."""
    inst = node["kind"].get("Instance") if isinstance(node["kind"], dict) else None
    main = doc["nodes"].get(str(inst["component"])) if inst and inst.get("component") is not None else None
    return main is not None and is_disabled(main["name"])


def _listed(allow: list) -> Tuple[dict, set, List[str]]:
    """({(path, property): opacity}, every (path, property) an opacity entry names, failures) for the paint-opacity entries of `allow`; a duplicate or valueless entry fails and pins nothing."""
    pinned, keys, fails = {}, set(), []
    for a in (a for a in allow if is_opacity_entry(a)):
        k, v = (a["path"], a["property"]), a.get("value")
        if k in keys:
            fails.append(f"allow-list entry {k[0]}: {k[1]} is listed twice")
        elif not isinstance(v, (int, float)) or isinstance(v, bool) or not 0 < v < 1:
            fails.append(f"allow-list entry {k[0]}: {k[1]} has no value strictly between 0 and 1: an allow-list entry pins the opacity it waives")
        else:
            pinned[k] = float(v)
        keys.add(k)
    return pinned, keys, fails


def check_opacity(doc: dict, allow: list = ()) -> Tuple[List[str], dict]:
    """(failures, {"variants", "disabled", "paints", "listed"}) over the variants of every component set, and every plain component, under pages named `02...` or `03...`;
    `allow` is the allow-list, of which the paint-opacity entries are read here ("listed" counts the paints one of them waives)."""
    pinned, keys, fails = _listed(allow)
    used: set = set()
    counts = {"variants": 0, "disabled": 0, "paints": 0, "listed": 0}
    for pid in doc["pages"]:
        page = doc["nodes"].get(str(pid))
        if page is None or not page["name"].startswith(PAGES):
            continue
        visited: set = {str(pid)}
        for child in page.get("children", []):
            for node, path in walk(doc, child, (page["name"],), visited):
                kind = node["kind"] if isinstance(node["kind"], dict) else {}
                parent = doc["nodes"].get(str(node.get("parent"))) or {}
                in_set = "ComponentSet" in (parent.get("kind") if isinstance(parent.get("kind"), dict) else {})
                if "ComponentSet" in kind:
                    roots = [(vid, path) for vid in node.get("children", [])]
                elif "Component" in kind and not in_set:
                    roots = [(node["id"], path[: -len(node["name"]) - 1])]  # a plain component is its own variant, under its parent's path
                else:
                    continue
                for vid, base in roots:
                    variant = doc["nodes"][str(vid)]
                    off = is_disabled(variant["name"])
                    counts["variants"] += 1
                    counts["disabled"] += off
                    dim: dict = {}  # node id -> True when the node, or an instance above it inside this variant, is Disabled
                    for sub, subpath in walk(doc, vid, (base,), set()):
                        here = dim[sub["id"]] = dim.get(sub.get("parent"), off) or _disabled_instance(doc, sub)
                        want = HALF if here else 1.0
                        who = "a Disabled variant" if off else "a paint inside an instance of a Disabled variant" if here else "a non-Disabled variant"
                        if abs(sub.get("opacity", 1.0) - 1.0) > 1e-3:
                            fails.append(f"{subpath}: has node opacity {sub['opacity']:g}, the kit never dims a node (Disabled is half alpha per paint)")
                        for prefix, i, paint in paints_of(sub):
                            counts["paints"] += 1
                            key, stored = (subpath, f"{prefix}/{i}/opacity"), paint.get("opacity", 1.0)
                            used.add(key)
                            counts["listed"] += key in pinned
                            if key in pinned:
                                listed = pinned[key] * (HALF if here else 1.0)
                                if abs(stored - listed) > 1e-3:
                                    fails.append(f"{subpath}: {prefix}/{i} has opacity {stored:g}, {who} carries {listed:g} here (the allow-list pins {pinned[key]:g})")
                            elif abs(stored - want) > 1e-3:
                                fails.append(f"{subpath}: {prefix}/{i} has opacity {stored:g}, {who} carries {want:g} on every visible paint")
    return fails + [f"allow-list entry {p}: {k} matches no paint" for p, k in sorted(keys - used)], counts
