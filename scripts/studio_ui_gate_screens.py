"""G3 screens: every frame on page `04 Screens` is a registry `screen` record built from instances only.

A screen is the app's window drawn as instances of the regions and components (plan S6: "each screen is instances only"). The gate reads each child frame of a page whose name starts with `04`:

* the frame is named after a record of design/component-registry.json whose category is `screen`, or `Screen/<that record>/<capture case>` for one state of it
  (`screen_record`; a typo, another tier or a case that is not a capture's file name fails); two frames with one full name fail;
* every child is an Instance of a main whose record (the component set, or the plain component) is in the registry, or a local rectangle a screen may draw: the one named
  `Canvas background` (the page's own colour behind the canvas content: the document's, not chrome), and, for a state whose gate data declares it (`SCRIMS`,
  studio_ui_gate_screen_data), the one named `Scrim` at the window's size (the app's scrim behind a modal dialog; bound to its variable when the declaration names one); a
  frame, a text, a vector, a second background or a second scrim is local drawing and fails;
* a screen with no child fails (it draws nothing);
* every node inside an instance that mirrors a main node (`component.source`) carries the variable bindings and the text style its main has: the post-pass that gives a copy its main's bindings
  (`mirror_sources`) reads pages 02-03 only, so a copy on 04 that came out of the engine as a literal would pass the other gates unseen (G2 bound reads 01-03). A binding or style the copy holds
  beyond its main's is not read here (the override gate owns that on 02-03).

An empty page, or a file with no page `04`, is no failure: a foundation-only file has no screen. What is inside an instance is a copy of its main, which the override gate reads on pages 02-03.

    fails, counts = check_screens(doc, registry, scrims=None)    counts: {"screens", "instances", "backgrounds", "copies"}; `scrims` {state: declaration} (default: the batches' gate data)
"""
from __future__ import annotations

import re
from typing import List, Optional, Tuple

import studio_ui_gate_screen_data as gate_data
from studio_ui_gate_doc import GateInputError

PAGE = "04"
BACKGROUND = "Canvas background"
SCRIM = "Scrim"
CASE = re.compile(r"^[a-z0-9]+(-[a-z0-9]+)*$")
STATE_NAME = re.compile(r"^(Screen/[^/]+)/([^/]+)$")


def screen_record(name: str, category: dict) -> Optional[Tuple[str, Optional[str]]]:
    """(registry screen record, capture case) a frame name stands for, or None: a bare `screen` record (case None), or `Screen/<record>/<case>` with <record> a `screen` record and
      <case> a capture's file name."""
    if category.get(name) == "screen":
        return name, None
    m = STATE_NAME.match(name)
    if m and category.get(m.group(1)) == "screen" and CASE.match(m.group(2)):
        return m.group(1), m.group(2)
    return None


def _kind(n: dict) -> str:
    k = n["kind"]
    return next(iter(k)) if isinstance(k, dict) else k


def _record_of(doc: dict, inst: dict):
    """The registry name an instance places: its main's set, or the main itself for a plain component; None when the main is missing."""
    ref = inst["kind"]["Instance"].get("component")
    main = doc["nodes"].get(str(ref)) if ref is not None else None
    if main is None:
        return None
    parent = doc["nodes"].get(str(main.get("parent")))
    return parent["name"] if parent is not None and _kind(parent) == "ComponentSet" else main["name"]


def _copy_problems(doc: dict, node: dict, where: str):
    """(failures, copies checked) of the nodes below `node`: each copy has every binding key and text style its main has."""
    fails: List[str] = []
    copies = 0
    for cid in node.get("children", []):
        kid = doc["nodes"].get(str(cid))
        if kid is None:
            raise GateInputError(f"dangling node id {cid} under {where}")
        here = f"{where}/{kid['name']}"
        src = doc["nodes"].get(str((kid.get("component") or {}).get("source")))
        if src is not None:
            copies += 1
            have, want = (kid.get("variables") or {}).get("bindings") or {}, (src.get("variables") or {}).get("bindings") or {}
            fails += [f"{here}: `{key}` is not bound, its main ({src['name']!r}) binds it: a copy on a screen takes its main's variables" for key in sorted(want) if key not in have]
            styles = src.get("style_refs") or {}
            fails += [f"{here}: the text style {styles[key]!r} is missing, its main ({src['name']!r}) has it" for key in sorted(styles) if key not in (kid.get("style_refs") or {})]
        sub, n = _copy_problems(doc, kid, here)
        fails, copies = fails + sub, copies + n
    return fails, copies


def _scrim_problems(screen: dict, kid: dict, declared, where: str) -> List[str]:
    """Why a rectangle named `Scrim` is not the declared scrim of this screen's state (empty when it is)."""
    w, h = screen["size"]["x"], screen["size"]["y"]
    if declared is None:
        return [f"{where}/{kid['name']}: local drawing (Rectangle): a screen is instances only, plus one rectangle named {BACKGROUND!r} and, where its state declares it, one {SCRIM!r}"]
    out = []
    if (kid["transform"]["tx"], kid["transform"]["ty"], kid["size"]["x"], kid["size"]["y"]) != (0, 0, w, h):
        out.append(f"{where}/{kid['name']}: the {SCRIM} is the window's size at 0, 0 ({w} x {h}), it is {kid['size']['x']} x {kid['size']['y']} at {kid['transform']['tx']}, {kid['transform']['ty']}")
    bound = "fills/0" in ((kid.get("variables") or {}).get("bindings") or {})
    if declared.get("variable") and not bound:
        out.append(f"{where}/{kid['name']}: `fills/0` is not bound: the state declares the variable {declared['variable']!r}")
    if not declared.get("variable") and bound:
        out.append(f"{where}/{kid['name']}: `fills/0` is bound, but the state declares a literal scrim ({declared.get('reason')}): declare the variable")
    return out


def _page_screens(doc: dict, page: dict):
    for cid in page.get("children", []):
        node = doc["nodes"].get(str(cid))
        if node is None:
            raise GateInputError(f"dangling node id {cid} under {page['name']}")
        if _kind(node) == "Section":
            found = 0
            for scid in node.get("children", []):
                sub = doc["nodes"].get(str(scid))
                if sub is None:
                    raise GateInputError(f"dangling node id {scid} under {page['name']}/{node['name']}")
                if _kind(sub) == "Text" and (sub["name"] == "Section title" or sub["name"].startswith("Label ")):
                    continue
                found += 1
                yield sub, None
            if not found:
                yield None, f"{page['name']}/{node['name']}: the section has no screen"
        elif not (_kind(node) == "Text" and (node["name"] == "Section title" or node["name"].startswith("Label "))):
            yield node, None


def check_screens(doc: dict, registry: dict, scrims: Optional[dict] = None) -> Tuple[List[str], dict]:
    category = {r["name"]: r.get("category", "component") for r in (registry.get("components") or [])}
    scrims = gate_data.load()["SCRIMS"] if scrims is None else scrims
    fails: List[str] = []
    counts = {"screens": 0, "instances": 0, "backgrounds": 0, "copies": 0}
    for pid in doc["pages"]:
        page = doc["nodes"].get(str(pid))
        if page is None:
            raise GateInputError(f"dangling page id {pid}")
        if not page["name"].startswith(PAGE):
            continue
        names = []
        for screen, err in _page_screens(doc, page):
            if err:
                fails.append(err)
                continue
            where, counts["screens"] = f"{page['name']}/{screen['name']}", counts["screens"] + 1
            record = screen_record(screen["name"], category)
            if record is None:
                fails.append(f"{where}: not a `screen` record of the registry, nor `Screen/<record>/<capture case>` (a frame on {page['name']} is named after one)")
            if screen["name"] in names:
                fails.append(f"{where}: the name {screen['name']!r} is used twice on {page['name']}: a screen is named once")
            names.append(screen["name"])
            if not screen.get("children"):
                fails.append(f"{where}: the screen is empty (it draws nothing)")
            backgrounds = scrim_count = 0
            for kid_id in screen.get("children", []):
                kid = doc["nodes"].get(str(kid_id))
                if kid is None:
                    raise GateInputError(f"dangling node id {kid_id} under {where}")
                if _kind(kid) == "Instance":
                    name = _record_of(doc, kid)
                    counts["instances"] += 1
                    if name is None or name not in category:
                        fails.append(f"{where}/{kid['name']}: an instance of {name or 'a component the file lacks'}, which is not a record of the registry")
                    sub, n = _copy_problems(doc, kid, f"{where}/{kid['name']}")
                    fails, counts["copies"] = fails + sub, counts["copies"] + n
                elif _kind(kid) == "Rectangle" and kid["name"] == SCRIM:
                    scrim_count += 1
                    fails += _scrim_problems(screen, kid, scrims.get(record[1]) if record and record[1] else None, where)
                    if scrim_count > 1:
                        fails.append(f"{where}: more than one Scrim: a screen draws one Scrim and instances only")
                elif _kind(kid) == "Rectangle" and kid["name"] == BACKGROUND:
                    backgrounds += 1
                    counts["backgrounds"] += 1
                    if backgrounds > 1:
                        fails.append(f"{where}: more than one background: a screen draws one {BACKGROUND!r} and instances only")
                else:
                    fails.append(f"{where}/{kid['name']}: local drawing ({_kind(kid)}): a screen is instances only, plus one rectangle named {BACKGROUND!r}")
    return fails, counts
