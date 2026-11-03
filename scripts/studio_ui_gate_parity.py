"""G1: structure (S1/S2), Primitive colour names, value parity with theme.rs, and the token half (design/design.tokens.json).

Parity: every Semantic variable aliases (one hop) a Primitive whose value equals the theme.rs constant named by its code
syntax (`theme::NAME`); every drawable constant has exactly one variable. Structure: two collections, one mode named
`Dark` each, Primitives hidden with scopes `[]`, Semantic names `group/kebab`. Token half: a constant that G01 or
`non_colour_token_parity` maps to a token key names that key in its description (failure); tokens with no mapped constant
and token/theme value differences are reported as INFO only, because theme.rs is the truth (D5).
"""
from __future__ import annotations

import re
import sys
from pathlib import Path
from typing import Dict, List, Tuple

import studio_ui_theme_reader as theme
from studio_ui_gate_doc import GateInputError, canonical, collection_names, same

from studio_ui_theme_policy import MODE_NAME, PRIMITIVES, SEMANTIC, SEMANTIC_NAME as _SEMANTIC_PATTERN  # noqa: E402

SEMANTIC_NAME = re.compile(_SEMANTIC_PATTERN)
CODE = re.compile(r"theme::([A-Z0-9_]+(?:\[\d+\])?)")


def check_structure(doc: dict) -> List[str]:
    fails, names = [], collection_names(doc)
    for col in doc["variable_collections"].values():
        if col["name"] not in (PRIMITIVES, SEMANTIC):
            fails.append(f"collection {col['name']}: only {PRIMITIVES} and {SEMANTIC} are allowed")
        modes = col["modes"]
        if len(modes) != 1:
            fails.append(f"{col['name']}: exactly one mode expected, found {len(modes)}")
        elif modes[0]["name"] != MODE_NAME:
            fails.append(f"{col['name']}: the mode must be named {MODE_NAME}, not {modes[0]['name']!r}")
    for v in doc["variables"].values():
        if names.get(v["collection"]) == PRIMITIVES:
            if not v.get("hidden_from_publishing"):
                fails.append(f"{v['name']}: a Primitive must be hidden from publishing")
            if v.get("scopes"):
                fails.append(f"{v['name']}: a Primitive must have scopes [], found {v['scopes']}")
        elif names.get(v["collection"]) == SEMANTIC and not SEMANTIC_NAME.fullmatch(v["name"]):
            fails.append(f"{v['name']}: a Semantic name must be group/kebab")
    return fails


def check_primitive_names(doc: dict) -> List[str]:
    """Every Primitive colour carries the name the objective-step rule gives it (`colour_names.build_names`, the generator's own
    rule): its Tailwind v3.4.17 name when it equals a palette value, else `<hue>/<step>`, never a Tailwind name that holds another
    value, never hex. Number and string Primitives are not named by this rule."""
    sys.path.insert(0, str(Path(__file__).resolve().parent / "studio_ui_file"))
    import colour_names
    cols = collection_names(doc)
    colours = {}
    for v in doc["variables"].values():
        if cols.get(v["collection"]) == PRIMITIVES and v["kind"] == "Color":
            c = canonical((v.get("values") or {}).get(doc["variable_collections"][v["collection"]]["default_mode"]))
            if c:
                colours[v["id"]] = (c["r"], c["g"], c["b"], c["a"])
    expected = colour_names.build_names(colours.values())
    def show(c):
        return "#%02X%02X%02X" % c[:3] + ("" if c[3] == 255 else f" alpha {c[3]}/255")

    return [f"{v['name']}: Primitive colour {show(colours[v['id']])} should be named {expected[colours[v['id']]]}"
            for v in doc["variables"].values() if v["id"] in colours and v["name"] != expected[colours[v["id"]]]]


def check_parity(doc: dict, rows: list) -> Tuple[list, dict]:
    cols = collection_names(doc)
    sem = [v for v in doc["variables"].values() if cols.get(v["collection"]) == SEMANTIC]
    if not sem:
        raise GateInputError("no variable in a collection named Semantic: nothing to check")
    want = {r["name"]: r for r in theme.drawable_rows(rows)}
    seen: Dict[str, list] = {}
    fails = []
    for v in sem:
        mode = doc["variable_collections"][v["collection"]]["default_mode"]
        raw = (v.get("values") or {}).get(mode)
        m = CODE.fullmatch((v.get("code_syntax") or {}).get("WEB", ""))
        if not m:
            fails.append(f"{v['name']}: code syntax WEB is not `theme::NAME`")
            continue
        const = m.group(1)
        if const not in want:
            fails.append(f"{v['name']}: {const} is not a drawable theme.rs constant")
            continue
        seen.setdefault(const, []).append(v["name"])
        if not (isinstance(raw, dict) and "Alias" in raw):
            fails.append(f"{v['name']}: Semantic value is a literal, expected an alias of a Primitive")
            continue
        prim = doc["variables"].get(raw["Alias"])
        if not prim or cols.get(prim["collection"]) != PRIMITIVES:
            fails.append(f"{v['name']}: alias target {raw['Alias']} is not a Primitive")
            continue
        got, exp = canonical((prim.get("values") or {}).get(mode)), want[const]["value"]
        if got is None:
            fails.append(f"{v['name']}: Primitive {prim['name']} is itself an alias (more than one hop)")
        elif not same(got, exp, tol=0):
            fails.append(f"{v['name']}: {got} != theme::{const} {exp}")
    fails += [f"{c}: {len(n)} variables {n}, expected exactly one" for c, n in seen.items() if len(n) > 1]
    fails += [f"theme::{c} has no Semantic variable" for c in sorted(set(want) - set(seen))]
    return fails, {"semantic": len(sem), "drawable_constants": len(want)}


def _flat_tokens(tokens: dict) -> Dict[str, object]:
    return {f"{g}.{n}": leaf["$value"] for g, items in tokens.items() if isinstance(items, dict)
            for n, leaf in items.items() if isinstance(leaf, dict) and "$value" in leaf}


def _same_token_value(token, row) -> bool:
    if row["kind"] == "colour":
        return str(token).lstrip("#").lower() == theme.colour_hex(row["value"]).lstrip("#").lower()
    m = re.fullmatch(r"\s*(-?[\d.]+)\s*(?:px)?\s*", str(token))
    return bool(m) and same(float(m.group(1)), row["value"])


def _show_row(row) -> str:
    return theme.colour_hex(row["value"]) if row["kind"] == "colour" else f"{row['value']:g}"


def check_tokens(doc: dict, rows: list, tokens: dict, const_keys: Dict[str, str]) -> Tuple[List[str], List[str]]:
    """(failures, infos) for the token half. Only the description check can fail."""
    cols = collection_names(doc)
    var_of = {}
    for v in doc["variables"].values():
        m = CODE.fullmatch((v.get("code_syntax") or {}).get("WEB", "")) if cols.get(v["collection"]) == SEMANTIC else None
        if m:
            var_of[m.group(1)] = v
    row_of = {r["name"]: r for r in theme.drawable_rows(rows)}
    flat, fails, infos = _flat_tokens(tokens), [], []
    for const, key in sorted(const_keys.items()):
        v = var_of.get(const)
        if v is not None and f"token {key}" not in v.get("description", ""):
            fails.append(f"{v['name']}: description does not name `token {key}` (theme::{const})")
        if const in row_of and key in flat and not _same_token_value(flat[key], row_of[const]):
            infos.append(f"INFO token {key} {flat[key]} != theme::{const} {_show_row(row_of[const])}")
    unmapped = sorted(set(flat) - set(const_keys.values()))
    if unmapped:
        infos.append(f"INFO {len(unmapped)} token keys have no mapped constant: {', '.join(unmapped)}")
    for key in unmapped:
        same_name = [r for n, r in row_of.items() if n.lower().replace("_", "-") == key.split(".")[-1] and not r["compound"]]
        for r in same_name:
            if not _same_token_value(flat[key], r):
                infos.append(f"INFO token {key} {flat[key]} != theme::{r['name']} {_show_row(r)} (same name, different constant)")
    return fails, infos
