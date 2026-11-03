"""Reading a native `.designos` document for the Studio UI gate: loading, variable values, a guarded tree walk."""
from __future__ import annotations

import json
import re
from pathlib import Path
from typing import Any, Optional


class GateInputError(Exception):
    """The file cannot be checked at all (exit 2)."""


def load_doc(path: Path) -> dict:
    try:
        raw = path.read_text(encoding="utf-8")
    except OSError as e:
        raise GateInputError(f"cannot read {path}: {e}")
    if not raw.strip():
        raise GateInputError(f"{path} is empty")
    try:
        doc = json.loads(raw)
    except json.JSONDecodeError as e:
        raise GateInputError(f"{path} is not JSON: {e}")
    if not isinstance(doc, dict) or not {"nodes", "pages", "variables", "variable_collections"} <= doc.keys():
        raise GateInputError(f"{path} is not a native .designos document")
    return doc


OPACITY_KEY = re.compile(r"^(fills|strokes)/\d+/opacity$")


def is_opacity_entry(entry: dict) -> bool:
    """True for an allow-list entry that pins a paint opacity (`fills/<i>/opacity`, `strokes/<i>/opacity`): G2 opacity reads it, G2 bound never does (it waives no unbound property)."""
    return bool(OPACITY_KEY.match(str(entry.get("property", ""))))


def collection_names(doc: dict) -> dict:
    return {c["id"]: c["name"] for c in doc["variable_collections"].values()}


def canonical(v: dict) -> Any:
    """Canonical value of one stored variable value: colour -> 0..255 ints, number -> float, string -> str; alias -> None."""
    if not isinstance(v, dict) or "Alias" in v:
        return None
    (kind, val), = v.items()
    if kind == "Color":
        return {k: round(val[k] * 255) for k in ("r", "g", "b", "a")}
    return float(val) if kind == "Number" else val


def variable_value(doc: dict, var: dict, hops: int = 3) -> Optional[Any]:
    """Value of `var` in its collection's default mode, following aliases (a Semantic variable is one hop)."""
    mode = doc["variable_collections"][var["collection"]]["default_mode"]
    v = (var.get("values") or {}).get(mode)
    if isinstance(v, dict) and "Alias" in v:
        target = doc["variables"].get(v["Alias"])
        return variable_value(doc, target, hops - 1) if target and hops > 0 else None
    return canonical(v)


def same(a: Any, b: Any, tol: int = 1) -> bool:
    """Equal values; colours compare per 0..255 channel within `tol` (parity uses 0), numbers within f32 noise."""
    if isinstance(a, dict) and isinstance(b, dict):
        return all(abs(a[k] - b[k]) <= tol for k in ("r", "g", "b", "a"))
    if isinstance(a, (int, float)) and isinstance(b, (int, float)):
        return abs(a - b) <= 1e-3 + 1e-6 * abs(b)
    return a == b


def walk(doc: dict, root_id: int, trail: tuple = (), seen: Optional[set] = None):
    """Yield (node, path) depth-first. A dangling child id, a cycle or a node listed twice is a GateInputError."""
    seen = set() if seen is None else seen
    node = doc["nodes"].get(str(root_id))
    if node is None:
        raise GateInputError(f"dangling node id {root_id} under {'/'.join(trail) or 'the document'}")
    if str(root_id) in seen:
        raise GateInputError(f"node {root_id} ({node['name']}) is reachable twice: a cycle or a shared child")
    seen.add(str(root_id))
    here = trail + (node["name"],)
    yield node, "/".join(here)
    for c in node.get("children", []):
        yield from walk(doc, c, here, seen)
