"""G5 size: a projection report, never a failure. The file is one file (owner ruling 2026-11-01: about 62 MB), so there is no limit to fail on; the report says where the size is going.

Page `04 Screens` holds one frame per screen. Each screen's bytes are what its nodes add to the saved file: the compact JSON of every node of the screen's subtree plus the `"<id>":` and `,` around it
(measured on `Screen/EditorDesign`: 792,389 bytes by this count, 792,407 by building the file with and without the screen). The base is the file without its screens, and the projection is the base
plus `PLANNED_SCREENS` screens of the mean weight (the plan's 74 states in user-flow order); a file with that many screens or more projects to itself.

    report = check_size(doc, file_bytes)      {"file", "screens": [(name, nodes, bytes)], "base", "mean", "projected"} (mean and projected are None with no screen)
    lines(report)                             the lines the gate prints: the file and its mean screen, one line per screen, the projection
"""
from __future__ import annotations

import json
from typing import List

from studio_ui_gate_doc import walk

PLANNED_SCREENS = 74
PAGE = "04"


def node_bytes(node: dict) -> int:
    """What one node adds to the file: its compact JSON, the quoted id key, the colon and the comma."""
    return len(json.dumps(node, separators=(",", ":"), ensure_ascii=False).encode("utf-8")) + len(str(node["id"])) + 4


def _is_kind(node: dict | None, kind_name: str) -> bool:
    if node is None:
        return False
    k = node.get("kind")
    return k == kind_name or (isinstance(k, dict) and kind_name in k)


def check_size(doc: dict, file_bytes: int) -> dict:
    screens: List[tuple] = []
    for pid in doc["pages"]:
        page = doc["nodes"].get(str(pid))
        if page is None or not page["name"].startswith(PAGE):
            continue
        seen: set = {str(pid)}
        candidates = []
        for child in page.get("children", []):
            cnode = doc["nodes"].get(str(child))
            if _is_kind(cnode, "Section"):
                seen.add(str(child))
                candidates.extend(cnode.get("children", []))
            else:
                candidates.append(child)
        for child in candidates:
            if _is_kind(doc["nodes"].get(str(child)), "Text"):
                continue
            nodes = [node for node, _ in walk(doc, child, (page["name"],), seen)]
            screens.append((nodes[0]["name"], len(nodes), sum(node_bytes(n) for n in nodes)))
    total = sum(b for _, _, b in screens)
    report = {"file": file_bytes, "screens": screens, "base": file_bytes - total, "mean": None, "projected": None}
    if screens:
        report["mean"] = total / len(screens)
        report["projected"] = file_bytes if len(screens) >= PLANNED_SCREENS else round(report["base"] + PLANNED_SCREENS * report["mean"])
    return report


def lines(r: dict) -> List[str]:
    head = f"G5 size   : the file is {r['file']:,} bytes ({r['file'] / 1e6:.1f} MB)"
    if not r["screens"]:
        return [head + f", no screen on page {PAGE} yet: nothing to project"]
    heaviest = max(r["screens"], key=lambda s: s[2])
    nodes = sum(n for _, n, _ in r["screens"])
    first = head + f"; {len(r['screens'])} screens weigh {r['mean']:,.0f} bytes each on average ({nodes / len(r['screens']):.0f} nodes), the heaviest is {heaviest[0]} at {heaviest[2]:,}"
    tail = f"{r['projected']:,} bytes ({r['projected'] / 1e6:.1f} MB), a report: the size never fails the gate"
    each = [f"G5 size   :   {name}: {n} node{'s' if n != 1 else ''}, {b:,} bytes" for name, n, b in r["screens"]]
    if len(r["screens"]) >= PLANNED_SCREENS:
        return [first, *each, f"G5 size   : the file already holds {len(r['screens'])} screens (planned {PLANNED_SCREENS}), so it is its own projection: {tail}"]
    return [first, *each, f"G5 size   : projection for {PLANNED_SCREENS} screens: {r['base']:,} bytes without screens + {PLANNED_SCREENS} x {r['mean']:,.0f} = {tail}"]
