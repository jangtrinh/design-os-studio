"""Assemble design/studio-ui.designos (W0: pages, variables, text styles, icons, Foundations specimens).

    python3 scripts/studio_ui_file foundation [--repo R] [--workspace DIR] --out FILE

A foundation-only file has no component set on page 02, so the gate refuses it (G3): --out is required, and the committed
design/studio-ui.designos is built by `python3 scripts/studio_ui_file library`.

Drives the INSTALLED designos-mcp 0.7.5. Everything is keyed on names and name_to_id, never on literal ids.
Order: REST import -> variables (Primitives, Semantic aliases) -> text styles -> icon fill bindings ->
specimens (apply_spec, `var:` refs) -> save -> native post-pass (code syntax, style bindings) -> reopen -> save.
Also writes the gate allow-list next to the file. Rebuilding on an unchanged theme.rs gives the same bytes.
"""
import argparse
import json
import pathlib
import shutil

import icons_rest as icons
import mcp_client as mcpc
from mcp_nodes import batch, icon_glyphs, ids_by_name
import native_post_pass as post
import scratch_workspace as scratch
import specimens_colour as colour
import specimens_contrast as contrast
import specimens_scales as scales
import spec_kit as kit
import start_here
import studio_ui_gate_pair as pair
import studio_ui_theme_policy as policy
import text_styles_plan as styles_mod
import variables_plan as plan_mod

PAGES = ["00 Start Here", "01 Foundations", "02 Core", "03 Studio", "04 Screens", "99 Archive"]
GAP, DARK = 200, {"r": 10 / 255, "g": 10 / 255, "b": 10 / 255, "a": 1}


def color_value(c):
    return {"Color": {"r": c["r"] / 255, "g": c["g"] / 255, "b": c["b"] / 255, "a": c["a"] / 255}}


def prim_value(p):
    return color_value(p["value"]) if p["kind"] == "Color" else {"Number": p["value"]} if p["kind"] == "Number" else {"String": p["value"]}


def rest_document(nodes, comps, core=None):
    """The REST file: page 01 holds the icon section; `core` (optional) = {"children", "studio", "components", "componentSets"} fills pages 02 and 03."""
    core = core or {"children": [], "components": {}, "componentSets": {}}
    w, h = icons.section_size(len(nodes))
    sec = {"id": "sec:icons", "type": "SECTION", "name": "Icons", "absoluteBoundingBox": {"x": 0, "y": 0, "width": w, "height": h},
           "fills": [{"type": "SOLID", "color": DARK}], "children": nodes}
    pages = {"01 Foundations": [sec], "02 Core": core["children"], "03 Studio": core.get("studio", [])}
    canvases = [{"id": f"page:{i}", "type": "CANVAS", "name": n, "backgroundColor": DARK, "children": pages.get(n, [])} for i, n in enumerate(PAGES)]
    return {"name": "DESIGN-OS Studio UI", "document": {"id": "0:0", "type": "DOCUMENT", "children": canvases},
            "components": {**comps, **core["components"]}, "componentSets": core["componentSets"]}


def variable_batches(plan):
    mode = [{"id": plan_mod.MODE, "name": policy.MODE_NAME}]
    cols = [{"id": plan_mod.COL_PRIM, "name": policy.PRIMITIVES, "modes": mode, "default_mode": plan_mod.MODE},
            {"id": plan_mod.COL_SEM, "name": policy.SEMANTIC, "modes": mode, "default_mode": plan_mod.MODE}]
    prims = [{"id": p["id"], "name": p["name"], "collection": plan_mod.COL_PRIM, "kind": p["kind"], "values": {plan_mod.MODE: prim_value(p)},
              "scopes": [], "hidden_from_publishing": True} for p in plan["primitives"]]
    sems = [{"id": s["id"], "name": s["name"], "collection": plan_mod.COL_SEM, "kind": s["kind"], "values": {plan_mod.MODE: {"alias": s["alias"]}},
             "scopes": s["scopes"], "description": s["description"]} for s in plan["semantic"]]
    return [{"op": "set_variables", "upsert_collections": cols, "upsert_variables": prims}, {"op": "set_variables", "upsert_variables": sems}]


def sized_section(mcp, page, name, content, x):
    """Create a section around `content`, then fit it to the content plus 200 px padding on every side."""
    r = mcp.tool_or_die("designos_apply_spec", {"parent_id": page, "lint": "off", "spec": kit.section(name, content, x, 0, 4000, 4000)})
    cid = r["name_to_id"][f"{name}/{content['name']}"]
    size = mcp.tool_or_die("designos_get_node", {"node_id": cid})["node"]["size"]
    w, h = size["x"] + 2 * 200, size["y"] + 2 * 200
    batch(mcp, [{"op": "update", "node": r["root_node_id"], "width": w, "height": h}])
    return r["root_node_id"], w, h, len(r.get("issues", []))


def open_and_define(mcp, plan, styles, rest_name):
    """Open the REST file, create the variables and text styles; returns {page name: node id}."""
    mcp.tool_or_die("designos_open_file", {"path": rest_name, "discard_changes": True})
    for op in variable_batches(plan):
        batch(mcp, [op])
    batch(mcp, [{"op": "set_style", "upsert": styles}])
    pages = {p["name"]: p["node_id"] for p in mcp.tool_or_die("designos_get_document", {})["pages"]}
    assert list(pages) == PAGES, pages
    return pages


def draw_foundation(mcp, plan, styles, nodes, pages):
    """Icon fills, icon labels, the Foundations specimens and Start Here. Returns the stats dict."""
    stats = {}
    found = ids_by_name(mcp, "Icons", pages["01 Foundations"])["Icons"]
    assert len(found) == 1, found
    icons_section = found[0]
    glyphs = icon_glyphs(mcp, icons_section)
    assert len(glyphs) == len(nodes), (len(glyphs), len(nodes))
    ops = [{"op": "update", "node": g, "fill": kit.V("color/icon")} for g in glyphs] + [{"op": "update", "node": icons_section, "fill": kit.V("color/bg")}]
    for i in range(0, len(ops), 200):
        batch(mcp, ops[i:i + 200])
    stats["icon_fills_bound"] = len(glyphs)
    labels = [{"op": "apply_spec", "parent": icons_section, "spec": dict(kit.text(f"Label {n['name'][5:]}", n["name"][5:], color="color/text-tertiary"),
                                                                           x=icons.grid_xy(i)[0], y=icons.grid_xy(i)[1] + 24)} for i, n in enumerate(nodes)]
    labels += [{"op": "apply_spec", "parent": icons_section, "spec": dict(kit.text("Section title", "Icons", style="Title / Semibold"), x=icons.PAD, y=icons.PAD)}]
    for i in range(0, len(labels), 100):
        batch(mcp, labels[i:i + 100])
    x, issues = 0, {}
    contrast_content, stats["contrast_pairs"] = contrast.contrast_content(plan, kit)
    for name, content in (("Colour", colour.colour_content(plan, kit)), ("Contrast", contrast_content),
                          ("Type", scales.type_content(kit, plan, styles)), ("Spacing", scales.spacing_content(kit, plan)),
                          ("Inset and control", scales.inset_control_content(kit, plan)), ("Radius nesting", scales.radius_content(kit, plan))):
        sid, w, h, n_issues = sized_section(mcp, pages["01 Foundations"], name, content, x)
        stats[f"section {name}"] = {"node": sid, "width": w, "height": h}
        issues[name] = n_issues
        x += w + GAP
    batch(mcp, [{"op": "update", "node": icons_section, "x": x}])
    sized_section(mcp, pages["00 Start Here"], "Start Here", start_here.start_here(plan, len(nodes), len(styles)), 0)
    stats["apply_spec_lint_issues"] = issues
    return stats


def finish(mcp, ws, out, plan, style_bindings, intents=(), allow=()):
    """Save, run the native post-pass (code syntax, style bindings, bind intents, mirrored bindings), reopen, save, copy to `out`. Returns the stats;
    `stats["allow"]` is the allow-list written next to `out`: `allow` plus the entries the post-pass mirrored onto nested instances."""
    mcp.tool_or_die("designos_save_file", {"path": "stage1.designos", "overwrite": True})
    mcp.close()
    counts = post.post_pass(ws / "stage1.designos", ws / "stage2.designos", plan, style_bindings, intents, allow)
    allow = counts.pop("allow")
    stats = {"post_pass": counts}
    mcp = mcpc.Mcp(str(ws))
    mcp.tool_or_die("designos_open_file", {"path": "stage2.designos", "discard_changes": True})
    mcp.tool_or_die("designos_save_file", {"path": "studio-ui.designos", "overwrite": True})
    mcp.close()
    out = pathlib.Path(out)
    out.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(ws / "studio-ui.designos", out)
    pair.write_allowlist(out, scales.ALLOW + list(allow))  # records the sha256 of `out`: the gate refuses a half-published pair
    return dict(stats, allow=list(allow))


def parse_args(prog, argv, default_out=True):
    repo_default = pathlib.Path(__file__).resolve().parents[2]
    ap = argparse.ArgumentParser(prog=prog, description="rebuild design/studio-ui.designos with the installed designos-mcp")
    ap.add_argument("--repo", default=str(repo_default))
    ap.add_argument("--workspace", help="scratch directory for the MCP session, kept after the run (default: a new temporary directory, removed after the run)")
    ap.add_argument("--keep-workspace", action="store_true", help="keep the temporary workspace and print its path (to inspect the staged file after a failed check)")
    ap.add_argument("--out", required=not default_out, help=f"where to write the file (default: <repo>/{policy.FILE_REL})" if default_out
                    else "where to write the file (required: a foundation-only file must never replace design/studio-ui.designos)")
    a = ap.parse_args(argv)
    a.out = a.out or str(pathlib.Path(a.repo) / policy.FILE_REL)
    return a


def main(argv=None):
    a = parse_args("studio_ui_file foundation", argv, default_out=False)
    with scratch.workspace(a.workspace, keep=a.keep_workspace) as ws:
        a.workspace = ws
        return build(a)


def build(a):
    plan = plan_mod.build_plan(a.repo)
    styles, style_bindings = styles_mod.build(plan)
    nodes, comps, skipped = icons.build_components(a.repo)
    (a.workspace / "foundation.json").write_text(json.dumps(rest_document(nodes, comps)))
    mcp = mcpc.Mcp(str(a.workspace))
    stats = {"server": mcp.server_info}
    pages = open_and_define(mcp, plan, styles, "foundation.json")
    stats.update(draw_foundation(mcp, plan, styles, nodes, pages))
    stats.update(finish(mcp, a.workspace, a.out, plan, style_bindings))
    stats.pop("allow")
    stats.update(bytes=pathlib.Path(a.out).stat().st_size, icons=len(nodes), skipped_icons=skipped, text_styles=len(styles),
                 variables={"primitives": len(plan["primitives"]), "semantic": len(plan["semantic"])})
    (a.workspace / "assemble-stats.json").write_text(json.dumps(stats, indent=1))
    print(json.dumps(stats, indent=1))
    return 0
