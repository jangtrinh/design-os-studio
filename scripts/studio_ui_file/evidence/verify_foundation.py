"""Reopen design/studio-ui.designos with the installed designos-mcp and record the structural evidence.

    python3 scripts/studio_ui_file evidence verify --file <.designos> --workspace <dir> --proof-dir <dir>

Records into <proof-dir>/verify-foundation.json: node count, effective_bindings of one icon vector and one
swatch, code syntax and text-style bindings after the reopen, `has-hardcoded` XPath counts and the full
designos_lint findings of page 01 Foundations (every finding listed, not summarised away).
"""
import argparse
import json
import pathlib
import shutil

import mcp_client


def first(mcp, name, query_root=None):
    r = mcp.tool_or_die("designos_list_nodes", {"query": name, "limit": 5})
    hits = [n for n in r["nodes"] if n["name"] == name]
    if not hits:
        raise SystemExit(f"no node named {name!r}")
    return hits[0]["node_id"]


def main(argv=None):
    ap = argparse.ArgumentParser(prog="studio_ui_file evidence verify")
    ap.add_argument("--file", required=True)
    ap.add_argument("--workspace", required=True)
    ap.add_argument("--proof-dir", required=True)
    a = ap.parse_args(argv)
    ws, proof = pathlib.Path(a.workspace), pathlib.Path(a.proof_dir)
    proof.mkdir(parents=True, exist_ok=True)
    ws.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(a.file, ws / "verify.designos")
    mcp = mcp_client.Mcp(str(ws))
    out = {"server": mcp.server_info}
    mcp.tool_or_die("designos_open_file", {"path": "verify.designos", "discard_changes": True})
    doc = mcp.tool_or_die("designos_get_document", {})
    out["node_count"], out["variable_count"], out["style_count"] = doc["node_count"], doc["variable_count"], doc["style_count"]
    out["pages"] = [p["name"] for p in doc["pages"]]
    page1 = next(p["node_id"] for p in doc["pages"] if p["name"] == "01 Foundations")
    for label, node in (("icon_vector", first(mcp, "Glyph")), ("swatch", first(mcp, "Swatch"))):
        gv = mcp.tool_or_die("designos_get_variables", {"node_id": node})
        out[label] = {"node_id": node, "effective_bindings": [{k: b[k] for k in ("property", "variable_id")} for b in gv["effective_bindings"]]}
        if not out[label]["effective_bindings"]:
            raise SystemExit(f"{label}: no effective_bindings")
    gv = mcp.tool_or_die("designos_get_variables", {})
    out["code_syntax_sample"] = {v["name"]: v["code_syntax"] for v in list(gv["variables"].values())[-3:]}
    out["text_style_bindings"] = {s["name"]: s["bindings"] for s in doc.get("styles", [])[:3]}
    xp = mcp.tool_or_die("designos_query_xpath", {"page_id": page1, "selector": "//*[has-hardcoded('fills/0')]", "limit": 500})
    out["has_hardcoded_fills_page01"] = len(xp.get("rows", xp.get("nodes", xp.get("results", []))))
    findings, cursor = [], None
    while True:
        r = mcp.tool_or_die("designos_lint", {"page_id": page1, "limit": 500, **({"cursor": cursor} if cursor else {})})
        findings += r.get("issues", r.get("findings", []))
        cursor = r.get("next_cursor") or r.get("cursor")
        if not cursor:
            break
    out["lint_page01"] = {"total": len(findings), "by_rule": {}, "findings": findings}
    for f in findings:
        key = f"{f.get('rule')}/{f.get('severity')}"
        out["lint_page01"]["by_rule"][key] = out["lint_page01"]["by_rule"].get(key, 0) + 1
    mcp.close()
    (proof / "verify-foundation.json").write_text(json.dumps(out, indent=1))
    print(json.dumps({k: v for k, v in out.items() if k != "lint_page01"}, indent=1))
    print("lint page 01:", out["lint_page01"]["total"], out["lint_page01"]["by_rule"])
    return 0
