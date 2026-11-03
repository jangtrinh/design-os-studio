"""Consolidated gap table (the rows still open), the screens that show each, and v0.7.5 Rust source citations.

Run: python3 -B scripts/studio_ui_file/screens/gap_table_citations.py
"""
import json
import pathlib
import subprocess

REPO = pathlib.Path(__file__).resolve().parents[3]
SNAPSHOT_PATH = pathlib.Path(__file__).with_name("v075_rust_snapshot.json")
SNAPSHOT = json.loads(SNAPSHOT_PATH.read_text(encoding="utf-8"))

DEV = ("g-dev-react", "g-dev-annotate", "g-dev-compare", "micro-dev-boxmodel")
AGENTS = ("i-agents", "i-agents-picker", "i-terminal", "i-agents-chat", "i-agents-start-failure", "i-agent-permissions",
          "i-agents-approval-command", "agents-approval-open-tab", "agents-approval-queued", "agents-skills", "kit-agents")
POPOVERS = ("f-edit-style-color", "edit-style-effect", "edit-style-grid", "edit-style-text", "f-stroke-settings",
            "effect-noise", "effect-glass", "effect-texture", "f-type-details", "f-prototype-tab")
MENUS = ("d-menu-main", "d-menu-main-file", "d-menu-shape", "d-menu-zoom", "f-effects-menu", "grid-styles", "f-effect-styles", "ai-image-menu")
MICRO_SEL = ("micro-layer-picker", "micro-smart-selection", "micro-selection-corner", "micro-scale-tool", "micro-vector-edit",
             "micro-shape-controls", "micro-autolayout", "micro-css-grid", "micro-inspector-math", "micro-typography-multiedit", "micro-components-variables")
RULERS_51 = ("d-editor-selected", "kit-editor", "e-layers-selected", "k-assets", *AGENTS, "fill-d9", "f-picker", "f-picker-gradient", *POPOVERS,
             "ai-image-menu", "ai-image-erase", "ai-image-expand", "j-check-designs", "j-comments", "j-version-history", "g-dev-react", "g-dev-annotate",
             "g-dev-compare", "j-cmdk", "k-variables-floating", *MICRO_SEL, "micro-dev-boxmodel")
RIGHT_46 = ("d-editor-selected", "kit-editor", "e-layers-selected", "k-assets", *AGENTS, "fill-d9", "f-picker", "f-picker-gradient", *POPOVERS,
            "ai-image-menu", "ai-image-erase", "ai-image-expand", "j-check-designs", "d-menu-zoom", "j-cmdk", "k-variables-floating", *MICRO_SEL)
LEFT_39 = ("d-editor-selected", "kit-editor", "e-layers-selected", "j-find", "k-assets", "fill-d9", "f-picker", "f-picker-gradient", *POPOVERS,
           "ai-image-menu", "ai-image-erase", "ai-image-expand", "j-check-designs", "j-comments", "j-version-history", "d-menu-main-file", "j-cmdk",
           "k-libraries", "k-variables-floating", *MICRO_SEL)
RAIL_13 = ("k-assets", *AGENTS, "k-variables")
STYLE_MENUS = ("f-effects-menu", "f-effect-styles", "grid-styles")  # the menu captures no inspector or layer-tree data covers
UNSAVED_DOT = ("fill-d9", "f-picker-gradient", "f-edit-style-color", "edit-style-effect", "edit-style-grid", "edit-style-text", "effect-noise", "effect-glass", "effect-texture",
               "grid-styles", "ai-image-menu", "ai-image-erase", "ai-image-expand")  # the captures that show a dirty tab and are not Dev mode ones (`DEV` show it too)

SCROLLED = ("e-layers-selected", "f-picker", "j-comments", "j-cmdk", "k-variables-floating", "micro-components-variables")  # a slot cannot express the capture's 7 px scroll offset
FRAME_WITHOUT_AUTO_LAYOUT = ("micro-scale-tool", "micro-inspector-math", "micro-css-grid")  # a frame the Layers state lacks parts for
APPROVAL = ("i-agents-approval-command", "agents-approval-open-tab", "agents-approval-queued")
SCRIMMED = ("j-plugins", "plugin-install", "k-libraries")

# The rows that are still open after the screens draw their per-screen data, each with the screens that still show its gap. A row a screen no longer shows is gone: rows 4, 10, 14, 17
# and 23 are closed (the rail and toolbar tools, the palette placeholder, the Home tab bar and the presentation Variables panel are drawn), and a row narrows to what is left of it.
GAP_TABLE = (
    {"row": 1, "title": "Rulers at non-2% zoom / pan offsets and selection band (Navigation/Rulers)", "screens": RULERS_51,
     "citations": (("crates/designos-app/src/canvas/rulers.rs", 100, 106, "pub fn strips"),)},
    {"row": 2, "title": ("Selection inspector parts no state draws: the Vertex section, a frame without auto layout's copy, clip and variable-mode rows, a strip's lit cell, and the selected "
                         "frame of the three menu captures no data covers (Panel/RightInspector)"),
     "screens": ("micro-vector-edit", *FRAME_WITHOUT_AUTO_LAYOUT, "e-layers-selected", "kit-agents", *STYLE_MENUS),
     "citations": (("crates/designos-app/src/panels/right.rs", 11, 181, "pub fn show"),
                   ("crates/designos-app/src/shell.rs", 1660, 1946, "fn draw_window"))},
    # edit-style-text and f-type-details: their Layers list is scrolled about 7 px like the SCROLLED captures' (best vertical shift of the capture against the render: mean 14.956 -> 5.966)
    {"row": 3, "title": ("Left-panel layer tree scroll offset (about 7 px, in the scrolled captures and in edit-style-text and f-type-details), the layer trees of the three menu captures no data "
                         "covers, and the Tree/Row looks of component rows and children of a selected row (Panel/LeftLayers)"),
     "screens": (*SCROLLED, "f-prototype-tab", "micro-css-grid", *STYLE_MENUS, "edit-style-text", "f-type-details"),
     "citations": (("crates/designos-app/src/panels/left.rs", 61, 243, "pub fn show"),
                   ("crates/designos-app/src/panels/layers.rs", 74, 95, "pub fn flatten"))},
    {"row": 5, "title": "Agents approval card status pill and text tone (Panel/AgentTerminal, nearest theme tone of STATUS_WARN_TEXT)", "screens": APPROVAL,
     "citations": (("crates/designos-app/src/agent_approval_card.rs", 39, 47, "pub fn paint_approval_card"),
                   ("crates/designos-app/src/terminal.rs", 5872, 5875, "pub fn show_left"))},
    {"row": 6, "title": "Settings popover parts: bound-variable pill, swatch tones and the type settings preview glyph (Popover/Panel)",
     "screens": ("f-edit-style-color", "edit-style-grid", "f-type-details"),
     "citations": (("crates/designos-app/src/widgets/popover.rs", 17, 50, "pub fn show"),
                   ("crates/designos-app/src/styles.rs", 50, 52, "pub fn style_entries"))},
    {"row": 7, "title": "Zoom menu input row: Input/Field is 32 px high where production's is 26 (Menu/Panel)", "screens": ("d-menu-zoom",),
     "citations": (("crates/designos-app/src/panels/menus.rs", 704, 801, "fn show_zoom_menu"),)},
    {"row": 8, "title": ("Dev mode inspector: the orange Changed status of g-dev-compare (Compare changes, Mark ready) and the Component playground preview of micro-dev-boxmodel, "
                         "which is the document's own render (Panel/DevInspect)"),
     "screens": ("g-dev-compare", "micro-dev-boxmodel"),
     "citations": (("crates/designos-app/src/dev/sections.rs", 51, 114, "pub fn status_header"),
                   ("crates/designos-app/src/dev/playground.rs", 451, 576, "pub fn playground_section"))},
    {"row": 9, "title": "Skills picker popover surface and count pill tones (agents-skills)", "screens": ("agents-skills",),
     "citations": (("crates/designos-app/src/skills/mod.rs", 816, 1025, "pub fn show_picker_popover"),)},
    {"row": 11, "title": "Modal scrim blending: BG_OVERLAY at level 30 where the capture has 29 (j-plugins, plugin-install, k-libraries)", "screens": SCRIMMED,
     "citations": (("crates/designos-app/src/plugins/ui.rs", 17, 227, "pub fn show"),
                   ("crates/designos-app/src/libraries/modal.rs", 147, 170, "pub fn show"))},
    {"row": 12, "title": "Gradient colour picker Linear button strokes (f-picker-gradient)", "screens": ("f-picker-gradient",),
     "citations": (("crates/designos-app/src/widgets/picker.rs", 737, 1217, "pub fn show"),)},
    {"row": 13, "title": "Check Designs popover & Find panel 1-2 px text offsets (j-check-designs, j-find)", "screens": ("j-check-designs", "j-find"),
     "citations": (("crates/designos-app/src/panels/check_designs.rs", 135, 322, "pub fn show"),
                   ("crates/designos-app/src/find/panel.rs", 182, 317, "pub fn show"))},
    {"row": 15, "title": "Floating Variables window ink box 613 x 352 against 622 x 364 (k-variables-floating, micro-components-variables)",
     "screens": ("k-variables-floating", "micro-components-variables"),
     "citations": (("crates/designos-app/src/variables/view.rs", 88, 94, "pub fn floating_rect"),
                   ("crates/designos-app/src/variables/popover.rs", 88, 219, "pub fn show"))},
    {"row": 16, "title": "Dev mode Compare changes dialog preview panes (g-dev-compare)", "screens": ("g-dev-compare",),
     "citations": (("crates/designos-app/src/dev/compare.rs", 83, 195, "pub fn show"),)},
    {"row": 18, "title": "36 px orange comment pin (j-comments)", "screens": ("j-comments",),
     "citations": (("crates/designos-app/src/comments/thread.rs", 40, 46, "pub fn show"),
                   ("crates/designos-app/src/comments/pins.rs", 44, 142, "pub fn overlay"))},
    # evidence: the `left-header-subtitle` GEOMETRY pin of j-version-history (w4c_gate.py): the capture's subtitle is 80 px wide, the region's neutral subtitle 124.5 px
    {"row": 19, "title": "Starter-file left panel header subtitle (j-version-history)", "screens": ("j-version-history",),
     "citations": (("crates/designos-app/src/history/banner.rs", 9, 40, "pub fn show"),
                   ("crates/designos-app/src/history/session.rs", 37, 60, "pub fn view_version"))},
    # evidence: the capture layout of k-variables names `vars-hscrollbar` (300, 851, 1130 x 7) and `vars-add-mode` (772, 81, 39 x 40), so its rows run across the 1150 px window past
    # the add column; Window/Variables draws Table/Data's default rows 521 px wide (the `variables main` pin cause in w4c_gate.py)
    {"row": 20, "title": "Full-window Variables table: rows and hairlines across the 1150 px with the add column (k-variables)", "screens": ("k-variables",),
     "citations": (("crates/designos-app/src/variables/view.rs", 247, 314, "pub fn show"),
                   ("crates/designos-app/src/variables/widgets.rs", 117, 150, "pub fn pill"))},
    {"row": 21, "title": "Canvas layer picker caption, row offsets and separator (micro-layer-picker)", "screens": ("micro-layer-picker",),
     "citations": (("crates/designos-app/src/panels/context.rs", 169, 241, "fn show_layer_picker"),
                   ("crates/designos-app/src/shell.rs", 1660, 1946, "fn draw_window"))},
    {"row": 22, "title": "Vector edit bar painter: transparent 84 x 28 cells on a BG_TOOLBAR band (micro-vector-edit)", "screens": ("micro-vector-edit",),
     "citations": (("crates/designos-app/src/canvas/vector.rs", 1110, 1142, "pub fn toolbar"),)},
    {"row": 24, "title": ("Dirty tab title room: production reserves 10 px more for the title of a tab with unsaved changes, so a long title truncates one glyph earlier; the part's neutral "
                          "title is never truncated (Navigation/TopTabBar)"),
     "screens": (*DEV, *UNSAVED_DOT),
     "citations": (("crates/designos-app/src/tabs.rs", 936, 1279, "pub fn show_top_tab_bar_ui"),)},
    {"row": 25, "title": "Effect style list: the highlighted row's (23, 23, 23) fill and the list's scroll thumb (Menu/Panel)", "screens": ("f-effect-styles",),
     "citations": (("crates/designos-app/src/panels/style_picker.rs", 217, 292, "pub(crate) fn style_button"),)},
)


def has_tag(tag: str = "v0.7.5") -> bool:
    proc = subprocess.run(["git", "-C", str(REPO), "rev-parse", "-q", "--verify", f"refs/tags/{tag}"], capture_output=True, text=True, check=False)
    return proc.returncode == 0


def find_item_end(lines: list[str], start: int) -> int:
    depth, started = 0, False
    for idx in range(start - 1, len(lines)):
        for ch in lines[idx].split("//")[0]:
            if ch == "{":
                depth, started = depth + 1, True
            elif ch == "}":
                depth -= 1
                if started and depth == 0:
                    return idx + 1
    return -1


def verify_snapshot_against_tag(snapshot: dict = SNAPSHOT, tag: str = "v0.7.5") -> tuple[list[str], list[str]]:
    fails, out = [], []
    for path, info in snapshot.get("files", {}).items():
        proc = subprocess.run(["git", "-C", str(REPO), "show", f"{tag}:{path}"], capture_output=True, text=True, check=False)
        if proc.returncode != 0:
            fails.append(f"snapshot {path}: missing at tag {tag}")
            continue
        lines = proc.stdout.splitlines()
        if len(lines) != info["lines"]:
            fails.append(f"snapshot {path}: line count {info['lines']} != {len(lines)} at {tag}")
        for start_s, item in info.get("items", {}).items():
            s, e = int(start_s), item["item_end"]
            if lines[s - 1] != item["start_line"]:
                fails.append(f"snapshot {path}:{s}: start line mismatch at {tag}")
            if find_item_end(lines, s) != e or lines[e - 1] != item["closing_line"]:
                fails.append(f"snapshot {path}:{s}-{e}: closing line mismatch at {tag}")
            if any(ln.strip().startswith("#[cfg(test)]") for ln in lines[s - 1 : e]):
                fails.append(f"snapshot {path}:{s}-{e}: range contains #[cfg(test)] at {tag}")
        out.append(f"OK snapshot {path} ({info['lines']} lines, {len(info.get('items', {}))} items)")
    return fails, out


def check_citations(table=GAP_TABLE, tag="v0.7.5", snapshot: dict = SNAPSHOT):
    files = snapshot.get("files", {})
    fails, out, checked, truncated = [], [], 0, 0
    for entry in table:
        row = int(entry["row"])
        if any("…" in s or "..." in s for s in (entry["title"], *entry.get("screens", ()))):
            truncated += 1
            fails.append(f"row {row}: contains ellipsis truncation in title or screen list")
        for path, start, end, symbol in entry["citations"]:
            checked += 1
            finfo = files.get(path)
            if finfo is None:
                fails.append(f"row {row}: {path} does not exist at tag {tag}")
                continue
            total = finfo["lines"]
            if not (1 <= start <= end <= total):
                fails.append(f"row {row}: {path}:{start}-{end} is outside 1..{total} at {tag}")
                continue
            item = finfo.get("items", {}).get(str(start))
            if item is None:
                fails.append(f"row {row}: {path}:{start}-{end} start line {start} is not a snapshotted item start")
                continue
            if symbol not in item["start_line"]:
                fails.append(f"row {row}: symbol {symbol!r} not found on start line {path}:{start} at {tag}")
                continue
            if end != item["item_end"]:
                fails.append(f"row {row}: {path}:{start}-{end} does not end on item closing line {item['item_end']} at {tag}")
                continue
            out.append(f"OK row {row:02d}: {path}:{start}-{end} (file has {total} lines at {tag}; symbol {symbol!r} verified)")
    out.append(f"{len(table)} gap rows, {checked} {tag} Rust citations checked, {len(fails)} failures, {truncated} truncated screen lists")
    return fails, out


if __name__ == "__main__":
    failures, report = check_citations()
    print("\n".join(report))
    raise SystemExit(1 if failures else 0)
