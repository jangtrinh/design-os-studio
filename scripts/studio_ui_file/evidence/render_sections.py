"""Render every top-level section of every page to PNG with designos_render_png (default scale 2).

    python3 scripts/studio_ui_file evidence render --workspace <dir with studio-ui.designos> --proof-dir <dir> [--scale 2]

Writes <proof-dir>/<page>__<section>.png and render-manifest.json (node ids, sizes, bytes). A section that
renders to an empty file is fatal.
"""
import argparse
import json
import pathlib
import re
import shutil

import mcp_client


def slug(s):
    return re.sub(r"[^a-z0-9]+", "-", s.lower()).strip("-")


def main(argv=None):
    ap = argparse.ArgumentParser(prog="studio_ui_file evidence render")
    ap.add_argument("--workspace", required=True)
    ap.add_argument("--proof-dir", required=True)
    ap.add_argument("--scale", type=float, default=2.0)
    ap.add_argument("--file", default="studio-ui.designos")
    a = ap.parse_args(argv)
    ws, proof = pathlib.Path(a.workspace), pathlib.Path(a.proof_dir)
    proof.mkdir(parents=True, exist_ok=True)
    (ws / "render").mkdir(exist_ok=True)
    mcp = mcp_client.Mcp(str(ws))
    mcp.tool_or_die("designos_open_file", {"path": a.file, "discard_changes": True})
    manifest = []
    for page in mcp.tool_or_die("designos_get_document", {})["pages"]:
        for n in mcp.tool_or_die("designos_list_nodes", {"parent_id": page["node_id"], "limit": 500})["nodes"]:
            if n["kind"] != "Section":
                continue
            rel = f"render/{slug(page['name'])}__{slug(n['name'])}.png"
            if (ws / rel).exists():
                (ws / rel).unlink()
            mcp.tool_or_die("designos_render_png", {"node_id": n["node_id"], "path": rel, "scale": a.scale})
            size = (ws / rel).stat().st_size
            if size == 0:
                raise SystemExit(f"{rel} is empty")
            shutil.copyfile(ws / rel, proof / pathlib.Path(rel).name)
            manifest.append({"page": page["name"], "section": n["name"], "node_id": n["node_id"], "width": n["width"], "height": n["height"],
                             "scale": a.scale, "png": pathlib.Path(rel).name, "bytes": size})
    mcp.close()
    (proof / "render-manifest.json").write_text(json.dumps(manifest, indent=1))
    print(json.dumps(manifest, indent=1))
    return 0
