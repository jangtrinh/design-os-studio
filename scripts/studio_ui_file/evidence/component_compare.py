"""Render every variant at scale 1 and 2; compare each one a 0.7.5 capture shows with its production crop; list the rest as UNVERIFIED-VISUAL.

    python3 scripts/studio_ui_file evidence compare [--file F] [--crops evidence/crops.json] [--out DIR] [--workspace DIR]

Pass 1 renders every variant of every set (default labels) at scale 1 and 2 into `renders/`. Pass 2: for each `crops.json` entry, on a
scratch copy the production text is set on the variant (`label`, in the text node `label_node`, default `Label`), an `icon` is swapped
in on a placed instance (its glyph repainted in the variant's own colour: the swap drops it, engine bug #1427), the render is composited
over the entry's `bg` and compared with the capture crop (layout.json named rect, or a hand `rect` with its derivation in `note`).
Outputs in --out: one `compare-<set>.png` per set (render | production | heat map, 4x; a variant with no capture gets its render and the
tag UNVERIFIED-VISUAL with the Rust it follows), `compare-scores.json` (mean and max absolute RGB difference, width and height difference
of the box where the rect is a layout rect, and the foreground box of both images with its difference: crop_metrics) and
`unverified-visual.json`. Entry keys added for the Forms & Inputs sets: `texts` {text node: production text} for a set with several texts, `labels` {path below the variant: production text} for a composite (see crops.json), `fills` {layer: #hex} for a content colour (set on the scratch copy), and `render_pad` (compare_images). Every entry is checked for its foreground box (width and height within 1 px), its surface colour and its
strongest-pixel colour (crop_metrics), and fails as "ambiguous background" when the colour most pixels have is not clearly the surface. A `colour_only` entry is exempt from the box; an `fg_pending` entry is exempt from the box and
both colours while #1427 is open (crop_metrics.issue_open: swap_check.SWAP_CHECK_FATAL is False) and the output names those rows. A `gradient` entry (a strip with no surface and no mark) is compared pixel by pixel instead (crop_gradient). An entry that fails on a known
difference carries a `finding` (the why) and a `finding_pin` (the measured score fields, crop_report): the run accepts it while the pin still equals the measurement. Every `render_rect` is checked against the node it names (`render_node`, `render_trim`; crop_geometry): a GEOMETRY line and exit 1, which no finding accepts. Exit 1 when an entry
fails without an accepted finding or a pin no longer holds, 2 when nothing was compared. Needs Pillow; nothing here runs in CI.
"""
import argparse
import json
import pathlib
import shutil

from PIL import Image

import mcp_client as mcpc
import scratch_workspace as scratch
from evidence import compare_images as ci
from evidence import crop_geometry
from evidence.capture_dirs import captures_dir, captures_for
from evidence import crop_metrics as cm
from evidence import node_paths
from evidence.follow_refs import follow_refs

HERE = pathlib.Path(__file__).resolve().parent
REPO = HERE.parents[2]
ZOOM = ci.ZOOM


def children(mcp, nid):
    r = mcp.tool_or_die("designos_list_nodes", {"parent_id": nid, "limit": 500})
    return {n["name"]: n["node_id"] for n in r["nodes"]}


def render(mcp, ws, out, vid, slug, bg=ci.BG):
    for scale in (1, 2):
        mcp.tool_or_die("designos_render_png", {"node_id": vid, "path": f"r-{slug}@{scale}x.png", "scale": scale})
        shutil.copyfile(ws / f"r-{slug}@{scale}x.png", out / "renders" / f"{slug}@{scale}x.png")
    return ci.over_bg(ws / f"r-{slug}@1x.png", bg), mcp.tool_or_die("designos_get_node", {"node_id": vid})["node"]["size"]


def glyph_of(mcp, nid):
    """The `Glyph` vector inside the icon slot (the first child that is not the focus ring) of a host node."""
    slot = next(i for n, i in children(mcp, nid).items() if n != "Focus ring")
    return children(mcp, slot)["Glyph"]


def render_with_icon(mcp, ws, out, set_name, variant, vid, icon, slug, bg):
    """Render the variant with the production icon: an instance on `99 Archive` with the icon swapped in, its glyph repainted in the
    variant's own colour (a swap drops it in 0.7.5, engine bug #1427; only the extent and the colour of the control are being compared)."""
    pages = {p["name"]: p["node_id"] for p in mcp.tool_or_die("designos_get_document", {})["pages"]}
    values = dict(part.split("=") for part in variant.split(", "))
    r = mcp.tool_or_die("designos_apply_spec", {"parent_id": pages["99 Archive"], "lint": "off", "spec": {
        "kind": "instance", "name": slug, "component": set_name, "variant": values, "props": {"Icon": f"Icon/{icon}"}}})
    solid = mcp.tool_or_die("designos_get_node", {"node_id": glyph_of(mcp, vid)})["node"]["fills"][0]["kind"]["Solid"]
    mcp.tool_or_die("designos_update_node", {"node_id": glyph_of(mcp, r["root_node_id"]), "fill": "#{:02X}{:02X}{:02X}".format(*(round(solid[k] * 255) for k in "rgb"))})
    return render(mcp, ws, out, r["root_node_id"], slug, bg)


def main(argv=None):
    ap = argparse.ArgumentParser(prog="studio_ui_file evidence compare", description=__doc__.split("\n")[0])
    ap.add_argument("--file", default=str(REPO / "design/studio-ui.designos"))
    ap.add_argument("--crops", default=str(HERE / "crops.json"))
    ap.add_argument("--out", required=True)
    ap.add_argument("--captures", help="folder of the 0.7.5 captures (default: from crops.json, in the repo or the main checkout)")
    ap.add_argument("--workspace", help="scratch directory for the MCP session, kept after the run (default: a temporary directory, removed after the run)")
    a = ap.parse_args(argv)
    with scratch.workspace(a.workspace, prefix="studio-ui-compare-") as ws:
        return run(a, ws)


def run(a, ws):
    out = pathlib.Path(a.out)
    (out / "renders").mkdir(parents=True, exist_ok=True)
    for old in ws.glob("r-*.png"):  # designos_render_png never overwrites
        old.unlink()
    shutil.copyfile(a.file, ws / "scratch.designos")
    crops = json.loads(pathlib.Path(a.crops).read_text())
    where = crop_geometry.check(json.loads(pathlib.Path(a.file).read_text(encoding="utf-8")), crops["entries"])  # each render_rect is where its node puts it (no MCP, no Pillow)
    captures = captures_dir(crops["captures"], a.captures)
    mcp = mcpc.Mcp(str(ws))
    mcp.tool_or_die("designos_open_file", {"path": "scratch.designos", "discard_changes": True})
    pages = {p["name"]: p["node_id"] for p in mcp.tool_or_die("designos_get_document", {})["pages"]}
    kids = node_paths.set_variants(mcp, pages)  # both component pages; a plain component is one variant named ""
    plain = {(n, v): render(mcp, ws, out, vid, f"{n.replace('/', '-')}__{v.replace('=', '-').replace(', ', '_')}") for n, vs in kids.items() for v, vid in vs.items()}
    rows, scores, covered = {n: {v: [] for v in vs} for n, vs in kids.items()}, [], set()
    for n, e in enumerate(crops["entries"]):
        vid, bg = kids[e["set"]][e["variant"]], tuple(e.get("bg", ci.BG))
        if e.get("label"):
            mcp.tool_or_die("designos_update_node", {"node_id": children(mcp, vid)[e.get("label_node", "Label")], "text": e["label"]})
        for node_name, hex_fill in (e.get("fills") or {}).items():  # content colour of a layer (Swatch/Color `Color`), set on the scratch copy only
            mcp.tool_or_die("designos_update_node", {"node_id": children(mcp, vid)[node_name], "fill": hex_fill})
        for node_name, text in (e.get("texts") or {}).items():  # a set with more than one text (Radio/Card: Title and Caption)
            mcp.tool_or_die("designos_update_node", {"node_id": children(mcp, vid)[node_name], "text": text})
        node_paths.apply_labels(mcp, vid, e.get("labels"))  # a composite: several texts, the texts of its nested instances by `/` path below the variant
        slug = f"{e['set'].replace('/', '-')}__{e['variant'].replace('=', '-').replace(', ', '_')}__{n:02d}-{e['case']}__{e.get('named') or 'rect'}"
        img, size = (render_with_icon(mcp, ws, out, e["set"], e["variant"], vid, e["icon"], slug, bg) if e.get("icon") else render(mcp, ws, out, vid, slug, bg))
        prod, (pw, ph) = ci.crop_of(e, captures_for(e, captures))
        m = ci.measure(img, prod, e)
        layout_rect = "named" in e and "render_rect" not in e
        dw, dh = (round(size["x"] - pw, 3), round(size["y"] - ph, 3)) if layout_rect else (None, None)
        geometry = "fg" if not layout_rect else "exact" if abs(dw) < 0.01 and abs(dh) < 0.01 else "off"
        scores.append({"set": e["set"], "variant": e["variant"], "capture": f"{e['case']}/{e.get('named') or e.get('rect')}", "label": e.get("label"),
                       "icon": e.get("icon"), "kit_painted": e["kit"], "render_px": [size["x"], size["y"]], "production_px": [pw, ph], "d_width": dw,
                       "d_height": dh, "geometry": geometry, "mean_abs_rgb_diff": round(m["mean"], 2), "max_abs_rgb_diff": m["max"],
                       "fg_render_px": m["fg_render"], "fg_production_px": m["fg_prod"], "d_fg_width": m["fg"]["d_width"], "d_fg_height": m["fg"]["d_height"],
                       "fg_ok": not m["fg_fails"], "box_exempt": bool(e.get("colour_only")), "pending": m["pending"], "finding": e.get("finding"), "finding_pin": e.get("finding_pin"),
                       "gradient": bool(e.get("gradient")), "fit_mean": m.get("fit_mean"), "fit_p99": m.get("fit_p99"), "fit_size_ok": m.get("fit_size_ok"), "fit_ok": (not m["fit_fails"]) if e.get("gradient") else None,
                       "surface_render": m["surface_render"], "surface_production": m["surface_prod"], "surface_diff": m["surface_diff"],
                       "surface_ok": not m["surface_fails"], "peak_colour_render": m["peak_render"], "peak_colour_production": m["peak_prod"],
                       "peak_diff": m["peak_diff"], "peak_ok": not m["peak_fails"], "background_ok": not m["ambiguous_fails"], "bad": m["bad"], "note": e.get("note", "")})
        rows[e["set"]][e["variant"]].append((f"{e['variant']}  {e['case']}/{e.get('named') or 'hand rect'}  render {size['x']}x{size['y']} prod {pw}x{ph}  "
                                             f"fg {m['fg_render']} vs {m['fg_prod']}  surface d{m['surface_diff']}  strongest d{m['peak_diff']}  mean {m['mean']:.2f} max {m['max']}"
                                             + (f"  PENDING {m['pending']}" if m["pending"] else ""), m["render"], prod, m["heat"]))
        covered.add((e["set"], e["variant"]))
    kit_rows = json.loads((REPO / "crates/designos-app/src/widgets/kit/testdata/kit-states.json").read_text())["rows"]
    unverified = []
    for name in kids:
        follows = follow_refs(name, crops["follows"][name], kit_rows)
        for v in kids[name]:
            if (name, v) not in covered:
                unverified.append({"set": name, "variant": v, "follows": follows})
                img = plain[(name, v)][0]
                rows[name][v].append((f"{v}  UNVERIFIED-VISUAL  no capture shows it  follows {'; '.join(follows[:3])}", img, Image.new("RGB", img.size, (36, 36, 36)),
                                      Image.new("RGB", (img.width * ZOOM, img.height * ZOOM), (36, 36, 36))))
    mcp.close()
    for name, by_variant in rows.items():
        ci.sheet([r for v in by_variant.values() for r in v], out / f"compare-{name.replace('/', '-')}.png")
    (out / "compare-scores.json").write_text(json.dumps(scores, indent=1))
    (out / "unverified-visual.json").write_text(json.dumps(unverified, indent=1))
    bad, exact = cm.bad_rows(scores), sum(s["geometry"] == "exact" for s in scores)
    print(f"{sum(len(v) for v in kids.values())} variants rendered at 1x and 2x, {len(scores)} compared with a capture (layout rect exact {exact}/"
          f"{sum(s['geometry'] != 'fg' for s in scores)}, box, surface and strongest pixel all within tolerance {len(scores) - len(bad)}/{len(scores)}), "
          f"{len(unverified)} UNVERIFIED-VISUAL, {len(rows)} sheets in {out}")
    if cm.pending_note(scores):
        print("PENDING " + cm.pending_note(scores))
    for line in cm.report_lines(scores):
        print(line)
    for line in where:
        print("GEOMETRY " + line)
    return cm.exit_code(scores, where)  # a GEOMETRY line fails the run whatever the rows say (crop_report)
