"""Side-by-side of 12 icons: the designos render of `Icon/<Name>` next to the app's own icon in a 0.7.5 capture.

    python3 scripts/studio_ui_file evidence icons --workspace <dir with studio-ui.designos> --captures <qa075 capture dir> --proof-dir <dir>

Centres are derived from the capture's `.layout.json` rects: the Layers header buttons, the mode switcher and
the actions button are read directly; the five tool buttons and the comment button are placed at the 56 px pitch
that the chevron rects imply. Only a 40x40 px crop around each icon is used. Mine is rendered at scale 4 and drawn
at the app's icon size (toolbar 20 px, layers 16 px), both magnified 5x onto the dark chrome fill.

Needs Pillow. This is the only module of the package that imports it, and only the `evidence icons` command loads it.
"""
import argparse
import pathlib

import mcp_client
from PIL import Image, ImageDraw

CAPTURE = "d-editor-default.png"
CHROME = (10, 10, 10)
# (enum name, centre x, centre y in the 1440x900 capture, app icon px)
ICONS = [("Move", 488, 864, 20), ("Frame", 544, 864, 20), ("Rectangle", 600, 864, 20), ("Pen", 656, 864, 20), ("Text", 712, 864, 20),
         ("Comment", 768, 864, 20), ("Sparkle", 808, 864, 20), ("Pencil", 867.6, 864, 20), ("Design", 906.9, 864, 20),
         ("Dev", 946.4, 864, 20), ("ChevronDown", 229, 325, 16), ("Search", 265, 325, 16)]
ZOOM, CELL = 5, 200


def main(argv=None):
    ap = argparse.ArgumentParser(prog="studio_ui_file evidence icons")
    ap.add_argument("--workspace", required=True)
    ap.add_argument("--captures", required=True)
    ap.add_argument("--proof-dir", required=True)
    ap.add_argument("--file", default="studio-ui.designos")
    a = ap.parse_args(argv)
    ws, proof = pathlib.Path(a.workspace), pathlib.Path(a.proof_dir)
    (ws / "render").mkdir(exist_ok=True)
    cap = Image.open(pathlib.Path(a.captures) / CAPTURE).convert("RGB")
    mcp = mcp_client.Mcp(str(ws))
    mcp.tool_or_die("designos_open_file", {"path": a.file, "discard_changes": True})
    cols = 3
    sheet = Image.new("RGB", (cols * 2 * CELL, 4 * (CELL + 28)), CHROME)
    draw = ImageDraw.Draw(sheet)
    for i, (name, cx, cy, px) in enumerate(ICONS):
        hits = [n for n in mcp.tool_or_die("designos_list_nodes", {"query": f"Icon/{name}", "limit": 20})["nodes"] if n["name"] == f"Icon/{name}"]
        if len(hits) != 1:
            raise SystemExit(f"Icon/{name}: expected one component, found {len(hits)}")
        rel = f"render/compare-{name}.png"
        if (ws / rel).exists():
            (ws / rel).unlink()
        mcp.tool_or_die("designos_render_png", {"node_id": hits[0]["node_id"], "path": rel, "scale": 4})
        mine = Image.open(ws / rel).convert("RGBA")
        bg = Image.new("RGBA", mine.size, CHROME + (255,))
        mine = Image.alpha_composite(bg, mine).convert("RGB").resize((px * ZOOM, px * ZOOM), Image.LANCZOS)
        app = cap.crop((round(cx - 20), round(cy - 20), round(cx + 20), round(cy + 20))).resize((40 * ZOOM, 40 * ZOOM), Image.LANCZOS)
        x0, y0 = (i % cols) * 2 * CELL, (i // cols) * (CELL + 28)
        sheet.paste(mine, (x0 + (CELL - mine.width) // 2, y0 + (CELL - mine.height) // 2))
        sheet.paste(app, (x0 + CELL, y0))
        draw.text((x0 + 8, y0 + CELL + 6), f"Icon/{name}  (left: designos render, right: 0.7.5 capture)", fill=(200, 200, 200))
    mcp.close()
    out = proof / "icon-compare-12.png"
    sheet.save(out)
    print(out, sheet.size)
    return 0
