"""Page `04 Screens`: one frame per screen, built from instances of the regions and components (never from local drawing) with the app's own window background.

A frame is named after a registry `screen` record (`Screen/EditorDesign`) or, for one state of it, `Screen/<that record>/<capture case>` (`Screen/EditorDesign/d-menu-main`):
the record the capture shows, then the capture's file name, which is the spec's `state`. A module of this package exports `screens()` returning a list of specs (dicts); the
assembler imports every module, so adding a module and its state in `GROUPS` is the whole registration:

    name     the frame's name, as above; the gate fails a frame that is not one of the two forms, and `discover` a full name used twice
    w, h     the window in px (the capture's size, 1440 x 900)
    state    the 0.7.5 capture the screen draws (`d-editor-default`): its layout.json is what `evidence screens` compares
    place    [{"name", "component", "variant": {axis: value} | None, "x", "y"} + optional "props", "overrides", "slots", "w", "h"] the instances, by registry name, in paint order
             (`x`, `y` in the screen); a record with no variants has `variant` None. `props` {"Title": ...}, `overrides` [{"path": "Header/Title", "text": ...}],
             `slots` {"Body": [place entries without x, y] | False} and a size `w` / `h` (apply_spec's `width` / `height`) are what apply_spec honours on 0.7.5; a slot holds instances
             of registry records only. A resized instance's absolutely placed children do NOT follow (a 312 wide Popover/Panel keeps its close button at x = 240 - 32): size an instance
             only where its main places the children in flow.
    aliases  {layout key: path of names below the screen, joined by ` > `} for a widget whose node a region cannot name (evidence/screen_geometry); optional
    canvas   (x, y, w, h, "#RRGGBB", name) the one local rectangle a screen may draw: the page's own colour behind the canvas content, which is the document's, not chrome; None for none
    scrim    {"fill": "var:Semantic/..." | "#RRGGBBAA", "above": name of a placed instance} the app's scrim behind a modal dialog: one rectangle `Scrim` at window size, painted right
             above that instance; allowed only for a state whose gate data declares it (`SCRIMS`, studio_ui_gate_screen_data); None for none

    build(mcp, page_id, specs)   one `designos_apply_spec` per screen (instances by registry name; every key is a name, never an id); returns {name: node id}

A screen is drawn at scale 1 at its window size, one below the other with `GAP` between, in user-flow order (`GROUPS`: Home -> editor shell -> left panel -> right panel ->
menus and dialogs -> variables -> present -> canvas micro states). Texts inside an instance are the regions' defaults (neutral content, no kit, client or library name).
"""
import importlib
import json
import pkgutil

import spec_kit as kit
import studio_ui_gate_screen_data as gate_data
from screens.same_drawing import LABEL, row_label_spec, row_label_text, same_drawing_info_lines, same_drawings  # noqa: F401
from studio_ui_gate_screens import SCRIM, screen_record

PAD, GAP, HEADER = 200, 200, 56  # 200 px section padding and gap, 56 px header above the first row label
PASS_THROUGH = (("props", "props"), ("overrides", "overrides"), ("w", "width"), ("h", "height"))
GROUPS = (
    ("Home", ("h-home",)),
    ("Editor shell", ("d-editor-default", "d-window-chrome", "d-editor-selected", "kit-editor")),
    ("Left panel", ("e-no-selection", "e-layers-selected", "j-find", "k-assets", "i-agents", "i-agents-picker", "i-terminal", "i-agents-chat",
                    "i-agents-start-failure", "i-agent-permissions", "i-agents-approval-command", "agents-approval-open-tab", "agents-approval-queued",
                    "agents-skills", "kit-agents")),
    ("Right panel", ("picker-page", "fill-d9", "f-picker", "f-picker-gradient", "f-edit-style-color", "edit-style-effect", "edit-style-grid",
                     "edit-style-text", "f-stroke-settings", "f-effects-menu", "f-effect-styles", "effect-noise", "effect-glass", "effect-texture",
                     "grid-styles", "f-type-details", "f-prototype-tab", "ai-image-menu", "ai-image-erase", "ai-image-expand", "j-check-designs",
                     "j-comments", "j-version-history", "g-dev-react", "g-dev-annotate", "g-dev-compare")),
    ("Menus and dialogs", ("d-menu-main", "d-menu-main-file", "d-menu-shape", "d-menu-zoom", "j-cmdk", "j-palette", "j-plugins", "plugin-install",
                           "k-libraries")),
    ("Variables", ("k-variables", "k-variables-floating")),
    ("Present", ("x-present", "present-variables")),
    ("Canvas micro states", ("micro-layer-picker", "micro-smart-selection", "micro-selection-corner", "micro-scale-tool", "micro-vector-edit",
                             "micro-shape-controls", "micro-autolayout", "micro-css-grid", "micro-inspector-math", "micro-typography-multiedit",
                             "micro-components-variables", "micro-dev-boxmodel")),
)
ORDER = tuple(s for _, group in GROUPS for s in group)
RANK = {s: i for i, s in enumerate(ORDER)}


def discover():
    out = []
    for mod in sorted(m.name for m in pkgutil.iter_modules(__path__)):
        module = importlib.import_module(f"{__name__}.{mod}")
        if hasattr(module, "screens"):
            out += [dict(s, module=mod) for s in module.screens()]
    names = [s["name"] for s in out]
    if len(set(names)) != len(names):
        raise ValueError(f"a screen is defined twice: {sorted(n for n in names if names.count(n) > 1)}")
    unranked = [s["state"] for s in out if s.get("state") not in RANK]
    if unranked:
        raise ValueError(f"screen state(s) missing from screens.GROUPS user-flow order: {unranked}")
    return sorted(out, key=lambda s: RANK[s["state"]])


def _check_place(p, records, where):
    """A placed instance and everything in its slots is an instance of a registry record: a slot is never a way to draw locally."""
    if p.get("kind", "instance") != "instance" or p.get("component") not in records:
        raise ValueError(f"{where} places {p.get('component') or p.get('kind')!r}, which is not an instance of a record of the registry")
    for slot, entries in (p.get("slots") or {}).items():
        if entries is False:
            continue
        if not isinstance(entries, list):
            raise ValueError(f"{where}: the slot {slot!r} must be a list of instances or False")
        for e in entries:
            _check_place(e, records, f"{where} > {slot}")


def check_specs(specs, registry, scrims=None):
    """Every screen is a registry `screen` record or `Screen/<record>/<case>` with the spec's `state` the case; every placed component, in a slot too, is a record;
    a scrim is declared for its state."""
    category = {r["name"]: r.get("category", "component") for r in registry["components"]}
    scrims = gate_data.load()["SCRIMS"] if scrims is None else scrims
    for s in specs:
        record = screen_record(s["name"], category)
        if record is None:
            raise ValueError(f"{s['name']} is not a `screen` record of design/component-registry.json, nor `Screen/<record>/<capture case>`")
        if record[1] is not None and s.get("state") != record[1]:
            raise ValueError(f"{s['name']}: its state is {s.get('state')!r}, the name says the capture {record[1]!r}")
        for p in s["place"]:
            _check_place(p, category, s["name"])
        if s.get("scrim"):
            if s["state"] not in scrims:
                raise ValueError(f"{s['name']} draws a {SCRIM}, which the gate data of {s['state']!r} does not declare (SCRIMS)")
            if not s["scrim"].get("fill"):
                raise ValueError(f"{s['name']}: the {SCRIM} has no fill")
            if s["scrim"].get("above") not in [p["name"] for p in s["place"]]:
                raise ValueError(f"{s['name']}: the {SCRIM} sits above {s['scrim'].get('above')!r}, which the screen does not place")
    return specs


def instance(p):
    """The apply_spec instance of one place entry; a key the entry does not set is absent, a slot's entries are built the same way and, with no position, carry none."""
    spec = {"kind": "instance", "name": p["name"], "component": p["component"]}
    if "x" in p:
        spec["x"], spec["y"] = p["x"], p["y"]
    if p.get("variant"):
        spec["variant"] = dict(p["variant"])
    for key, out in PASS_THROUGH:
        if p.get(key) is not None:
            spec[out] = json.loads(json.dumps(p[key]))
    if p.get("slots") is not None:
        spec["slots"] = {k: (v if v is False else [instance(e) for e in v]) for k, v in p["slots"].items()}
    return spec


def frame_spec(s, y, x=0):
    kids = []
    if s.get("canvas"):
        cx, cy, w, h, colour, name = s["canvas"]
        kids.append({"kind": "rectangle", "name": name, "fill": colour, "x": cx, "y": cy, "width": w, "height": h})
    for p in s["place"]:
        kids.append(instance(p))
        if s.get("scrim") and p["name"] == s["scrim"]["above"]:
            kids.append({"kind": "rectangle", "name": SCRIM, "fill": s["scrim"]["fill"], "x": 0, "y": 0, "width": s["w"], "height": s["h"]})
    return kit.box(s["name"], fill="color/bg", width=s["w"], height=s["h"], x=x, y=y, children=kids)


def build(mcp, page_id, specs):
    ids, sx, same = {}, 0, same_drawings(specs)
    pos_of = {s["name"]: i for i, s in enumerate(specs, 1)}
    group_of = {state: row for row, states in GROUPS for state in states}
    by_group = {}
    for s in specs:
        by_group.setdefault(group_of.get(s.get("state"), "Screens"), []).append(s)
    ordered = [(r, by_group[r]) for r, _ in GROUPS if r in by_group] + [(r, v) for r, v in by_group.items() if r not in dict(GROUPS)]
    for row, group_specs in ordered:
        w = max(s["w"] for s in group_specs)
        sec_h = PAD + HEADER + sum(LABEL + s["h"] + GAP for s in group_specs) - GAP + PAD
        sec = kit.box(row, kind="section", fill="color/bg", width=w + 2 * PAD, height=sec_h, x=sx, y=0,
                      children=[dict(kit.text("Section title", row, style="Title / Semibold"), x=PAD, y=PAD)])
        sec_id = mcp.tool_or_die("designos_apply_spec", {"parent_id": page_id, "lint": "off", "spec": sec})["root_node_id"]
        y = PAD + HEADER
        for s in group_specs:
            mcp.tool_or_die("designos_apply_spec", {"parent_id": sec_id, "lint": "off", "spec": row_label_spec(s, same, x=PAD, y=y, pos=pos_of.get(s["name"]))})
            r = mcp.tool_or_die("designos_apply_spec", {"parent_id": sec_id, "lint": "off", "spec": frame_spec(s, y + LABEL, x=PAD)})
            ids[s["name"]] = r["root_node_id"]
            y += LABEL + s["h"] + GAP
        sx += w + 2 * PAD + GAP
    return ids
