"""Colour specimen: one swatch row per Semantic colour (fill bound to the variable), name and hex in Roboto Mono.

    content = colour_content(plan, kit)     # one column spec for the `Colour` section
"""
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))  # scripts/: the committed reader
from studio_ui_theme_reader import colour_hex as hex_of  # noqa: E402  (one copy of the #RRGGBB[AA] format)

COLUMNS = 5
LABELS = {"bg": "Background", "text": "Text", "icon": "Icon", "border": "Border", "badge": "Badge", "focus": "Focus", "canvas": "Canvas",
          "syntax": "Syntax", "reader": "Reader light", "tile": "Tile", "provider": "Provider", "agent": "Agent", "board": "Board",
          "content": "Content handles", "dev": "Dev mode", "box": "Box model", "annotation": "Annotation", "proto": "Prototype"}


def groups(plan):
    out = {}
    for s in plan["semantic"]:
        if s["group"] == "color":
            out.setdefault(s["name"].split("/")[1].split("-")[0], []).append(s)
    merged = {k: v for k, v in out.items() if len(v) >= 3}
    merged["other"] = [s for k, v in out.items() if len(v) < 3 for s in v]
    return merged


def swatch_row(kit, s):
    return kit.row(f"Swatch {s['name']}", [
        kit.box("Swatch", kind="rectangle", fill=s["name"], radius="radius/mark", stroke="color/border-strong", width=32, height=24),
        kit.text("Name", s["name"], width=232),
        kit.text("Hex", hex_of(s["value"]), color="color/text-tertiary", width=96)],
        gap="space/inline", pad=[None, "inset/control", None, "inset/control"], align="center")


def colour_content(plan, kit):
    cols = [{"children": [], "rows": 0} for _ in range(COLUMNS)]
    for key, items in sorted(groups(plan).items(), key=lambda kv: -len(kv[1])):
        col = min(cols, key=lambda c: c["rows"])
        title = LABELS.get(key, key.title())
        col["children"].append(kit.column(f"Group {title}", [kit.text("Group title", title, style="Large / Semibold")] +
                                          [swatch_row(kit, s) for s in items], gap="space/row"))
        col["rows"] += len(items) + 2
    columns = [kit.column(f"Column {i + 1}", c["children"], gap="space/section") for i, c in enumerate(cols) if c["children"]]
    return kit.column("Colour specimen", [kit.text("Section title", "Colour", style="Title / Semibold"),
                                          kit.row("Colour columns", columns, gap="space/region")], gap="space/section")
