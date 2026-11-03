"""Pages `02 Core` and `03 Studio` as REST: every component module's sets lowered, grouped into one section per registry row.

    core = build_core(repo, plan, styles, icons)
    core["children"]   REST sections of page 02 (registry `element` and `component`), core["studio"] those of page 03 (`composite` and `region`);
                       core["components"], core["componentSets"]  the maps of the REST file (a plain component, one for a record with no variants, is no set)
    core["intents"]    bind intents for the native post-pass; core["allow"]  gate allow-list entries (path + property + reason + the literal value it pins)
    core["info"]       per set: REST ids of its variants, their text nodes (nested instances' too) and focus rings (each with the id of the node it sits in, the host
                       it rings), and the copied texts that keep their main's content ("mirrors"), for the layout pass (library_layout)

Canvas arrangement (sections in one row, 200 px padding and gap, a header per row) is canvas layout, not chrome: literal.
"""
import json
import pathlib

import components as comps_pkg
import rest_ir as ir
from components.env import Env
from rest_lower import Lowerer, bb

PAD, GAP, HEADER, SET_GAP, VAR_GAP, LABEL = 200, 200, 56, 96, 48, 28


def _grid(sizes, cols):
    """Row-major cell origins for `sizes` in `cols` columns, plus the grid's total size."""
    xs, ys, x, y, row_h = [], [], 0, 0, 0
    for i, (w, h) in enumerate(sizes):
        if i and i % cols == 0:
            x, y, row_h = 0, y + row_h + VAR_GAP, 0
        xs.append(x)
        ys.append(y)
        x, row_h = x + w + VAR_GAP, max(row_h, h)
    return xs, ys, max(a + s[0] for a, s in zip(xs, sizes)), max(b + s[1] for b, s in zip(ys, sizes))


def _defs(low, spec, set_id):
    """componentPropertyDefinitions of a set (one VARIANT per axis, then the module's props); registers the props' REST keys for the lowerer."""
    axes = spec["axes"]
    defs, low.prop_keys = {a: {"type": "VARIANT", "defaultValue": v[0], "variantOptions": list(v)} for a, v in axes.items()}, {}
    for i, p in enumerate(spec.get("props", [])):
        key = f"{p['name']}#{set_id}:{i}"
        low.prop_keys[p["name"]] = key
        d = {"type": p["type"], "defaultValue": p.get("default", "")}
        if p["type"] == "INSTANCE_SWAP":
            d["defaultValue"] = low.icons.entry(p["default"])["id"]
            d["preferredValues"] = [{"type": "COMPONENT", "key": k} for k in low.icons.keys()]
        defs[key] = d
    low.set_props[spec["name"]] = {p["name"]: (low.prop_keys[p["name"]], p["type"]) for p in spec.get("props", [])}
    return defs


def described(spec):
    """{"description": text} for a spec that has one, else {}: the importer reads a component's description from its node (the COMPONENT_SET, or the plain COMPONENT), never from the `components` map."""
    return {"description": spec["description"]} if spec.get("description") else {}


def lower_plain(low, env, spec, index, ox, oy):
    """A record with no variants: one plain COMPONENT named after the record (no set, no variant property), lowered under the variant name ""."""
    defs = _defs(low, spec, f"set{index}")
    rest = low.component(spec["name"], "", spec["build"]({}, env), ox, oy)
    rest["name"], rest["componentPropertyDefinitions"] = spec["name"], defs
    rest.update(described(spec))
    texts = [(sid, content) for sid, content, _ in low.main_texts[(spec["name"], "")]]
    vinfo = {"id": rest["id"], "name": "", "texts": texts, "rings": rings_of(rest), "mirrors": low.mirror_pairs[(spec["name"], "")],
             "fills": low.fill_pairs[(spec["name"], "")], "resized": low.resized[(spec["name"], "")]}
    return rest, {rest["id"]: {"key": f"studio-ui/{spec['name']}", "name": spec["name"]}}, {"id": rest["id"], "name": spec["name"], "row": spec["row"], "cols": 1, "plain": True,
                                                                                            "variants": [vinfo]}


def lower_set(low, env, spec, index, ox, oy):
    if not spec["axes"]:
        return lower_plain(low, env, spec, index, ox, oy)
    set_id, axes = f"set{index}", spec["axes"]
    defs = _defs(low, spec, set_id)
    combos = comps_pkg.matrix(axes)
    irs = [spec["build"](v, env) for v in combos]
    sizes = [ir.estimate(n, mains=low.main_sizes) for n in irs]
    xs, ys, w, h = _grid(sizes, len(list(axes.values())[-1]))
    variants, vinfo, comps = [], [], {}
    for values, node, x, y in zip(combos, irs, xs, ys):
        vname = comps_pkg.variant_name(values)
        rest = low.component(spec["name"], vname, node, ox + 32 + x, oy + 32 + y)
        variants.append(rest)
        comps[rest["id"]] = {"key": f"studio-ui/{spec['name']}/{vname}", "name": vname}
        texts = [(sid, content) for sid, content, _ in low.main_texts[(spec["name"], vname)]]  # own texts, then the nested instances' (rest_nested)
        vinfo.append({"id": rest["id"], "name": vname, "texts": texts, "rings": rings_of(rest), "mirrors": low.mirror_pairs[(spec["name"], vname)],
                      "fills": low.fill_pairs[(spec["name"], vname)], "resized": low.resized[(spec["name"], vname)]})
    node = {"id": set_id, "type": "COMPONENT_SET", "name": spec["name"], "absoluteBoundingBox": bb(ox, oy, w + 64, h + 64), "fills": [],
            "strokes": [], "componentPropertyDefinitions": defs, "children": variants, **described(spec)}
    return node, comps, {"id": set_id, "name": spec["name"], "row": spec["row"], "cols": len(list(axes.values())[-1]), "variants": vinfo}


def rings_of(rest):
    """[{"id": ring REST id, "parent": REST id of the node it sits in}] for every `Focus ring` instance under `rest`, in document order. The
    parent is the host: the layout pass sizes the ring to it (a ring is its host grown by SPACE_FACT on every side)."""
    out = []
    for c in rest.get("children", []):
        if c["type"] == "INSTANCE" and c["name"] == "Focus ring":
            out.append({"id": c["id"], "parent": rest["id"]})
        out += rings_of(c)
    return out


def paths(children, page="02 Core"):
    """REST id -> the gate's node path (page and node names joined by `/`)."""
    out = {}

    def visit(node, trail):
        out[node["id"]] = "/".join(trail + (node["name"],))
        for c in node.get("children", []):
            visit(c, trail + (node["name"],))

    for c in children:
        visit(c, (page,))
    return out


def build_core(repo, plan, styles, icons):
    registry = json.loads((pathlib.Path(repo) / "design/component-registry.json").read_text(encoding="utf-8"))
    specs = comps_pkg.check_specs(comps_pkg.discover(), registry)
    low = Lowerer(plan, styles, icons)
    env = Env(repo, plan, icons, mains=low.main_sizes)  # the dict `Lowerer.component` fills: a set sees every main lowered before it
    components, set_map, info, pages = {}, {}, [], {}
    for page, rows in comps_pkg.PAGE_ROWS.items():  # page 02 first, then 03: a set is lowered after the sets it nests
        sections, x = [], 0
        for row in rows:
            row_specs = [s for s in specs if s["row"] == row]
            if not row_specs:
                continue
            sx, y, kids, w = x, PAD + HEADER, [], 0
            header = low.lower(ir.T("Section title", row, "Title / Semibold", "color/text"), sx + PAD, PAD)
            for spec in row_specs:
                label = low.lower(ir.T(f"Label {spec['name']}", spec["name"], "Body / Medium", "color/text-tertiary"), sx + PAD, y)
                node, comps, sinfo = lower_set(low, env, spec, len(info), sx + PAD, y + LABEL)
                sinfo.update(label=label["id"], section=f"sec:{row}", page=page)
                kids += [label, node]
                components.update(comps)
                if node["type"] == "COMPONENT_SET":
                    set_map[node["id"]] = {"key": f"studio-ui/{spec['name']}", "name": spec["name"]}
                info.append(sinfo)
                y += LABEL + node["absoluteBoundingBox"]["height"] + SET_GAP
                w = max(w, node["absoluteBoundingBox"]["width"])
            sec = {"id": f"sec:{row}", "type": "SECTION", "name": row, "absoluteBoundingBox": bb(sx, 0, w + 2 * PAD, y - SET_GAP + PAD),
                   "fills": [low.paint("color/bg")], "children": [header, *kids]}
            low.bind(sec["id"], "fills/0", "color/bg")
            sections.append(sec)
            x += w + 2 * PAD + GAP
        pages[page] = sections
    where = {**paths(pages["02 Core"], "02 Core"), **paths(pages["03 Studio"], "03 Studio")}
    allow = [{"path": where[a["sid"]], "property": a["property"], "reason": a["reason"], "value": a["value"]} for a in low.allow]
    return {"children": pages["02 Core"], "studio": pages["03 Studio"], "components": components, "componentSets": set_map, "intents": low.intents, "allow": allow,
            "info": info}
