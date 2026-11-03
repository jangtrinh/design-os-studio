"""G3 structure: every component set on pages `02 Core` and `03 Studio` is a record of design/component-registry.json, on the page its category belongs to, and
carries exactly the registry's variant values, as a full matrix (one Component child per combination, named `Axis=Value, Axis=Value`).

Registry variants are flat `Axis=Value` entries; the axes keep their order of first appearance, and a variant name lists its
pairs in that order (`Size=Xs, State=Default`, as the property order produces it). A set that is not in the registry, an axis or
value that differs, a hole or an extra name in the matrix, a name whose pairs are permuted and a name used twice are failures.
Page: `element` and `component` records live on `02 Core`, `composite` and `region` records on `03 Studio` (plan S6); a set on the other page fails.
A record with no variants (10 composites today) is ONE plain Component named after the record, with no variant property: a plain Component for a record that
has variants fails, a set for a record that has none fails, and a plain Component that is not a record fails.
No set and no plain component on either page, or a registry without records, is an input error: the gate never passes on an empty selection.
"""
from __future__ import annotations

import itertools
from typing import Dict, List, Tuple

from studio_ui_gate_doc import GateInputError, walk

CORE = "02"
PAGES = ("02", "03")
PAGE_OF_CATEGORY = {"element": "02 Core", "component": "02 Core", "composite": "03 Studio", "region": "03 Studio", "screen": "04 Screens"}


def registry_axes(registry: dict) -> Dict[str, Dict[str, List[str]]]:
    records = registry.get("components") if isinstance(registry, dict) else None
    if not records:
        raise GateInputError("the component registry has no components")
    out: Dict[str, Dict[str, List[str]]] = {}
    for rec in records:
        axes: Dict[str, List[str]] = {}
        for v in rec.get("variants", []):
            axis, _, value = v.partition("=")
            if value not in axes.setdefault(axis, []):
                axes[axis].append(value)
        out[rec["name"]] = axes
    return out


def registry_pages(registry: dict) -> Dict[str, str]:
    """{record name: the page its category belongs to}; a record without a category is a component."""
    return {rec["name"]: PAGE_OF_CATEGORY[rec.get("category", "component")] for rec in (registry.get("components") or [])}


def _variant_options(node: dict) -> Dict[str, List[str]]:
    out = {}
    for p in (node.get("component") or {}).get("props", []):
        kind = p.get("kind")
        if isinstance(kind, dict) and "Variant" in kind:
            out[p["name"]] = list(kind["Variant"].get("options", []))
    return out


def _check_set(doc: dict, node: dict, expected: Dict[str, List[str]]) -> List[str]:
    name, fails = node["name"], []
    actual = _variant_options(node)
    for axis in sorted(set(expected) | set(actual)):
        want, have = set(expected.get(axis, [])), set(actual.get(axis, []))
        if axis not in actual:
            fails.append(f"{name}: axis {axis} is missing (registry: {sorted(want)})")
        elif axis not in expected:
            fails.append(f"{name}: axis {axis} is not in the registry")
        elif want != have:
            fails.append(f"{name}: axis {axis} values {sorted(have)} differ from the registry {sorted(want)}"
                         f" (missing {sorted(want - have)}, extra {sorted(have - want)})")
    names = [doc["nodes"][str(c)]["name"] for c in node.get("children", [])]
    wanted = {", ".join(f"{a}={v}" for a, v in zip(expected, combo)) for combo in itertools.product(*expected.values())}
    have = set(names)
    fails += [f"{name}: variant {label} is missing from the matrix" for label in sorted(wanted - have)]
    fails += [f"{name}: variant {label} is not in the registry matrix" for label in sorted(have - wanted)]
    if len(names) != len(have):
        fails.append(f"{name}: a variant name is used twice")
    return fails


def _page_fail(name: str, want: str, page: str) -> List[str]:
    return [] if want[:2] == page[:2] else [f"{name}: a record of this category belongs on {want}, it is on {page}"]


def _plain_fails(node: dict, want: Dict[str, List[str]]) -> List[str]:
    """A plain Component (not a variant of a set) is right only for a record with no variants, and has no variant property."""
    name = node["name"]
    if want[name]:
        return [f"{name}: the registry gives it variants {sorted(want[name])}, so it is a component set, not a plain component"]
    if _variant_options(node):
        return [f"{name}: a record with no variants has no variant property"]
    return []


def check_registry(doc: dict, registry: dict) -> Tuple[List[str], int]:
    """(failures, number of sets and plain components checked) under pages whose name starts with `02` or `03`."""
    known, page_of = registry_axes(registry), registry_pages(registry)
    fails, seen = [], set()
    for pid in doc["pages"]:
        page = doc["nodes"].get(str(pid))
        if page is None or not page["name"].startswith(PAGES):
            continue
        visited: set = {str(pid)}
        for child in page.get("children", []):
            for node, _ in walk(doc, child, (page["name"],), visited):
                kind = node["kind"] if isinstance(node["kind"], dict) else {}
                parent = doc["nodes"].get(str(node.get("parent"))) or {}
                plain = "Component" in kind and "ComponentSet" not in (parent.get("kind") if isinstance(parent.get("kind"), dict) else {})
                if "ComponentSet" not in kind and not plain:
                    continue
                if node["name"] in seen:
                    fails.append(f"{node['name']}: appears twice on pages 02 Core and 03 Studio")
                seen.add(node["name"])
                if node["name"] not in known:
                    fails.append(f"{node['name']}: {'plain component' if plain else 'component set'} is not in the registry")
                    continue
                fails += _page_fail(node["name"], page_of[node["name"]], page["name"])
                if plain:
                    fails += _plain_fails(node, known)
                elif not known[node["name"]]:
                    fails.append(f"{node['name']}: the registry gives it no variants, so it is one plain component, not a component set")
                else:
                    fails += _check_set(doc, node, known[node["name"]])
    if not seen:
        raise GateInputError("G3 found no component set on 02 Core or 03 Studio: refusing to pass an empty selection")
    return fails, len(seen)
