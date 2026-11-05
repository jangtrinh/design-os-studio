"""Render every screen of page `04 Screens` at scale 1 and set it against the 0.7.5 capture it draws: widget rects by name, then pixels.

    python3 scripts/studio_ui_file evidence screens [--file F] [--captures DIR] --out DIR [--workspace DIR]

For each screen spec (screens/): the saved file is read for G3 geometry (`screen_geometry.check`: every named widget rect of the capture's layout.json is the rect of the screen's node
of that name within 1 px; the keys no screen can name are listed with the reason), `designos_render_png` renders the frame in a scratch copy, and the render is diffed with the capture
over every pixel the masks leave in (`screen_masks`: the canvas content and the replaced texts are out; `screen_diff`: mean, 99th percentile, share above 24, worst 40 px tile; the same
per region). Outputs in --out: `<state>-render.png`, `<state>-heat.png` (the difference x4 in red, excluded pixels in blue-grey), `<state>-sheet.png` (render | capture | heat at half
size) and `screens-scores.json`. The exit code is 1 when any named widget rect is off by more than 1 px or a region's mean difference is over its cap or the worst tile is over 34
(`screen_caps`: measured on the first screen + about 8 %), 2 when no screen was compared; the 99th percentile is reported, never capped. Needs Pillow and the engine to run (Pillow is
imported where an image is read or written, so the module and its exit decision import without it and `test_studio_ui_screen_compare_main` drives `main`); no CI path runs it.
"""
import argparse
import json
import pathlib
import shutil

import mcp_client as mcpc
import scratch_workspace as scratch
import screens as screens_pkg
from evidence import screen_caps as caps
from evidence import screen_diff as sd
from evidence import screen_geometry as sg
from evidence import screen_masks as masks
from evidence.capture_dirs import captures_dir

HERE = pathlib.Path(__file__).resolve().parent
REPO = HERE.parents[2]
BG = (10, 10, 10)


def pixels(im):
    return list(im.getdata()), im.width, im.height


def heat_map(diff, w, h, mask):
    from PIL import Image
    img = Image.new("RGB", (w, h))
    img.putdata([(70, 80, 110) if mask[i] else (min(255, diff[i] * 4), 0, 0) for i in range(w * h)])
    return img


def sheet(render, capture, heat, path):
    from PIL import Image
    half = (render.width // 2, render.height // 2)
    out = Image.new("RGB", (half[0] * 3 + 8, half[1]), (60, 0, 60))
    for i, im in enumerate((render, capture, heat)):
        out.paste(im.resize(half, Image.LANCZOS), (i * (half[0] + 4), 0))
    out.save(path)


def compare(mcp, ws, out, doc, spec, nid, captures):
    from PIL import Image
    state = spec["state"]
    mcp.tool_or_die("designos_render_png", {"node_id": nid, "path": f"{state}-render.png", "scale": 1})
    shutil.copyfile(ws / f"{state}-render.png", out / f"{state}-render.png")
    rgba = Image.open(out / f"{state}-render.png").convert("RGBA")
    render = Image.alpha_composite(Image.new("RGBA", rgba.size, BG + (255,)), rgba).convert("RGB")
    capture = Image.open(captures / f"{state}.png").convert("RGB")
    layout = json.loads((captures / f"{state}.layout.json").read_text())
    geometry = sg.check(doc, spec["name"], layout, spec.get("aliases"))
    r, c = pixels(render), pixels(capture)
    diffs, mask = sd.pixel_diffs(r, c), sd.exclusion_mask(r[1], r[2], masks.exclusions(state))
    everything = sd.region_stats(diffs, r[1], None, mask)
    by_region = {n: sd.region_stats(diffs, r[1], rect, mask) for n, rect in masks.regions(state).items()}
    heat = heat_map(diffs[0], r[1], r[2], mask)
    heat.save(out / f"{state}-heat.png")
    sheet(render, capture, heat, out / f"{state}-sheet.png")
    return {"screen": spec["name"], "state": state, "geometry": geometry, "pixels": everything, "regions": by_region, "worst_tile": sd.worst_tile(diffs, r[1], r[2], mask),
            "excluded_pixels": sum(mask)}


def lines(s):
    g, p = s["geometry"], s["pixels"]
    out = [f"{s['screen']} vs {s['state']}: widget rects by name {len(g['matched'])} matched, {len(g['mismatched'])} off by more than 1 px, {len(g['not_named'])} not named of {g['keys']} keys",
           f"  pixels counted {p['pixels']} (excluded {s['excluded_pixels']}): mean {p['mean']}, p99 {p['p99']}, share above {sd.NOTICE} {p['share']}; worst {sd.TILE} px tile {s['worst_tile']}"]
    out += [f"  {n}: {r['pixels']} px, mean {r['mean']}, p99 {r['p99']}, share {r['share']}" for n, r in s["regions"].items()]
    out += [f"  OFF {m['key']}: layout {m['layout']}, node {m['node']}" for m in g["mismatched"]]
    return out + [f"  CAP {line}" for line in caps.judge(s)]


def main(argv=None):
    ap = argparse.ArgumentParser(prog="studio_ui_file evidence screens", description=__doc__.split("\n")[0])
    ap.add_argument("--file", default=str(REPO / "design/studio-ui.designos"))
    ap.add_argument("--captures", help="folder of the 0.7.5 captures (default: from crops.json, in the repo or the main checkout)")
    ap.add_argument("--out", required=True)
    ap.add_argument("--workspace", help="scratch directory for the MCP session, kept after the run (default: a temporary directory, removed after the run)")
    a = ap.parse_args(argv)
    out = pathlib.Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    captures = captures_dir(json.loads((HERE / "crops.json").read_text())["captures"], a.captures)
    specs = screens_pkg.discover()
    if not specs:
        print("no screen to compare")
        return 2
    doc = json.loads(pathlib.Path(a.file).read_text(encoding="utf-8"))
    with scratch.workspace(a.workspace, prefix="studio-ui-screens-") as ws:
        shutil.copyfile(a.file, ws / "scratch.designos")
        mcp = mcpc.Mcp(str(ws))
        mcp.tool_or_die("designos_open_file", {"path": "scratch.designos", "discard_changes": True})
        pages = {p["name"]: p["node_id"] for p in mcp.tool_or_die("designos_get_document", {})["pages"]}
        top = mcp.tool_or_die("designos_list_nodes", {"parent_id": pages["04 Screens"], "limit": 500})["nodes"]
        secs = [n for s in top if s.get("kind") == "Section" for n in mcp.tool_or_die("designos_list_nodes", {"parent_id": s["node_id"], "limit": 500})["nodes"]]
        found = {n["name"]: n["node_id"] for n in top + secs}
        scores = [compare(mcp, ws, out, doc, s, found[s["name"]], captures) for s in specs]
        mcp.close()
    (out / "screens-scores.json").write_text(json.dumps(scores, indent=1))
    for s in scores:
        print("\n".join(lines(s)))
    for line in screens_pkg.same_drawing_info_lines(specs):
        print(line)
    return caps.exit_code(scores)
