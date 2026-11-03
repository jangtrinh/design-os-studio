"""Registry `region` Navigation/Rulers and Navigation/BottomCanvasToolbar (page `03 Studio`, row Regions). Lines are the 0.7.5 tag of crates/designos-app/src.

Navigation/Rulers (canvas/rulers.rs `strips`, `overlay`, `ticks`; a 909 x 868 canvas rect, layout.json `regions.canvas`): the top strip (the rect from x 20, 20 high), the left strip (from y 20,
20 wide) and the corner, all BG, a BORDER hairline on the strips' inner edges, then the ticks and labels in `RULER_TICK` / `RULER_TEXT`. The capture is at 2 %: `tick_step` is 5000 page units
(100 px), the minor step 500 (10 px), so a tick every 10 px is 3 px long, every 50 px 6 px and every 100 px 14 px (`RULER_SIZE` - 6) and carries its value (`fmt_value`: `0`, `5000`, ... `40000`
on the top ruler from x 359.5, the left ruler from y 400.5 with -15000 ... 25000 turned a quarter turn, `rotated_text`). The ticks of a strip are ONE vector path (a 1 px rectangle
  each), never a node per
tick. The page between the strips is the canvas, drawn by the screen, so the region is transparent there. The labels are `font_ruler` (`Ruler / Regular`). The selection band, guides and
the value badge only appear with a selection or a drag: not drawn. The values are the ones the 2 % view of the capture shows; a pan or a zoom is another set of ticks.

Navigation/BottomCanvasToolbar (panels/toolbar.rs `show`, `toolbar_width`; (464, 840, 512, 48): centred on the window, `TOOLBELT_BOTTOM_MARGIN` 12 above its bottom edge): BG_TOOLBAR at
  `RADIUS_FRAME`
with a 1 px `INNER_HIGHLIGHT` stroke inside (a (1, 1, 511, 47) frame: egui snaps the stroke of `bar.shrink(0.5)` inward at the top and left), `TOOLBELT_PADDING` 8, then the six tool
  groups (Move, Frame, Rectangle, Pen, Text, Comment: `GROUPS`) as Button/Tool at `TOOL_GROUP_GAP` 8, the first
one Selected (the Move tool is active), each followed by a 16 px chevron cell (`TOOL_CHEVRON_W`) when `has_chevron` (all but Comment), whose 11 px ChevronDown is centred 11 px right of
  the button edge (`TOOL_CHEVRON_CENTER_DX`),
then the Actions button (Sparkle), 13 px (`TOOLBAR_DIVIDER_GAP_BEFORE`) before a 1 px BORDER divider that runs the bar's height less its 1 px stroke, 8 px (`TOOLBAR_DIVIDER_GAP_AFTER`)
  before the mode
switcher: a `BG_TOOLBAR_MODE_SWITCHER` pill of `MODE_SEGMENTS` (4) * `MODE_SWITCH_W` (29.5) + 2 * `MODE_SWITCH_PAD` wide and three cells of the rest (`(width - 2 * pad) / 3`), each 16 px glyph
(Pencil, Design, Dev). The Design cell is Selected (`segmented_look(Selected)` with its border cleared: BG_SELECTED, ICON_SECONDARY); the others are Default. The cells at BG_SELECTED /
  the pill are not
Tabs/Segmented (a 216 x 24 strip of text cells): the painter draws icon cells of its own, so they are local frames. Gaps: the contextual bar above the toolbar needs a selection
  (`contextual_bar`),
the Dev-mode toolbar (`show_dev_mode_toolbar`) is `State=Dev` and `State=DevAnnotate` (regions_editor_gap2_devbar.py), `State=Comment` and `State=AI` are the Comment tool and
  the Actions button Selected, and the swapped 20 px tool glyphs render at 16 px in designos-mcp 0.7.5 (#1427; the file carries the Size override a
  fixed engine honours).
"""
from rest_ir import F, I, T, V, X
from components.regions_editor_gap2_devbar import dev_toolbar

TICK = 10  # px per minor tick at 2 % (500 page units)
LEN = (14, 6, 3)  # RULER_SIZE - 6, 6, 3: the length of a major, a half and a minor tick
TOP_VALUES = [(359 + 100 * k, 5000 * k) for k in range(9)]  # (page x of the tick, the value it reads)
LEFT_VALUES = [(100 + 100 * k, -15000 + 5000 * k) for k in range(9)]  # (page y of the tick, the value it reads)
RULER_W, RULER_H = 909.0, 868.0  # layout.json regions.canvas


def level(n):
    """0 for a major tick (every 10 minor steps), 1 for the half (the 5th), else 2: `ticks` with a 500 step in a 5000 major."""
    return 0 if n % 10 == 0 else 1 if n % 10 == 5 else 2


def top_ticks(env):
    s = env.num("RULER_SIZE")
    d = "".join(f"M{TICK * i} {s - LEN[level(i - 5)]:g}h1v{LEN[level(i - 5)]}h-1z" for i in range(89))  # x 309 ... 1189, the major at 359
    return V("Top ticks", d, 890.0, s, env.c("RULER_TICK"), abs=(s - 1, 0, 890.0, s))


def left_ticks(env):
    s = env.num("RULER_SIZE")
    d = "".join(f"M{s - LEN[level(j - 34)]:g} {8 + TICK * j}h{LEN[level(j - 34)]}v1h-{LEN[level(j - 34)]}z" for j in range(84))  # y 60 ... 890, the major at 100
    return V("Left ticks", d, s, RULER_H - s, env.c("RULER_TICK"), abs=(0, s, s, RULER_H - s))


def ruler_label(env, name, text, x, y, rot=False):
    kw = {"rot": -90} if rot else {}
    return T(name, text, "Ruler / Regular", env.c("RULER_TEXT"), abs=(x, y, 5.0 * len(text), 11.0), **kw)


def rulers(values, env):
    s, line = env.num("RULER_SIZE"), env.c("BG")
    strip = lambda name, x, y, w, h: F(name, fill=line, w=w, h=h, sx="fixed", sy="fixed", abs=(x, y, w, h))
    hair = lambda name, x, y, w, h: X(name, "Divider/Hairline", "State=Default", abs=(x, y, w, h))
    kids = [strip("Top strip", s, 0, RULER_W - s, s), strip("Left strip", 0, s, s, RULER_H - s), strip("Corner", 0, 0, s, s), hair("Top border", 0, s - 1, RULER_W, 1),
            hair("Left border", s - 1, 0, 1, RULER_H), top_ticks(env), left_ticks(env)]
    kids += [ruler_label(env, f"Top label {v}", str(v), x + 3 - 290, 2) for x, v in TOP_VALUES]
    kids += [ruler_label(env, f"Left label {v}", str(v), 2, y - 3 - 32, rot=True) for y, v in LEFT_VALUES]
    return F("Rulers", kids, w=RULER_W, h=RULER_H, sx="fixed", sy="fixed")


GROUP_ICONS = (("Move", True), ("Frame", True), ("Rectangle", True), ("Pen", True), ("Text", True), ("Comment", False))
MODES = (("toolbar-mode-draw", "Pencil", False), ("toolbar-mode-design", "Design", True), ("toolbar-mode-dev", "Dev", False))


def chevron(env, name, x, y):
    """The 16 x 32 cell right of a tool, in the Tool look at rest (no fill): the glyph is placed 11 px from the button's edge, so 3 px right of the cell's centre."""
    glyph = I("Chevron", "ChevronDown", 11, env.c("ICON_QUATERNARY_HOVER"), abs=(5.5, 10.5, 11, 11))
    return F(name, [glyph], radius=env.c("RADIUS_CONTROL"), w=env.num("TOOL_CHEVRON_W"), h=env.num("TOOL_BUTTON"), sx="fixed", sy="fixed",
             abs=(x, y, env.num("TOOL_CHEVRON_W"), env.num("TOOL_BUTTON")))


def mode_cell(env, name, icon, selected, x, y, w, h):
    look_fill, look_icon = (env.c("BG_SELECTED"), env.c("ICON_SECONDARY")) if selected else (None, env.c("ICON_QUATERNARY"))  # segmented_look(Selected, border cleared) / (Default)
    return F(name, [I("Glyph", icon, int(env.num("LAYER_ICON_SIZE")), look_icon)], dir="H", fill=look_fill, radius=env.c("RADIUS_SEGMENT"), w=w, h=h, sx="fixed", sy="fixed", abs=(x, y, w, h))


SELECTED_TOOL = {"Move": 0, "Comment": 5, "AI": 0}  # State -> the Selected group (j-comments: Comment; j-cmdk and j-palette: Move and the Actions button)


def toolbar(values, env):
    if values["State"] in ("Dev", "DevAnnotate"):
        return dev_toolbar(env, values["State"])
    chosen = SELECTED_TOOL[values["State"]]
    pad, tool, gap, y = env.num("TOOLBELT_PADDING"), env.num("TOOL_BUTTON"), env.num("TOOL_GROUP_GAP"), 8.0
    x, kids = pad, []
    for gi, (icon, has_chevron) in enumerate(GROUP_ICONS):
        x += gap if gi else 0
        kids.append(X(f"tool-{gi}", "Button/Tool", "State=Selected" if gi == chosen else "State=Default", props={"Icon": icon}, abs=(x, y, tool, tool)))
        x += tool
        if has_chevron:
            kids.append(chevron(env, f"tool-chevron-{gi}", x + env.num("TOOL_CHEVRON_GAP"), y))
            x += env.num("TOOL_CHEVRON_GAP") + env.num("TOOL_CHEVRON_W")
    x += gap
    kids.append(X("toolbar-actions-btn", "Button/Tool", "State=Selected" if values["State"] == "AI" else "State=Default", props={"Icon": "Sparkle"}, abs=(x, y, tool, tool)))
    x += tool + env.num("TOOLBAR_DIVIDER_GAP_BEFORE")
    kids.append(X("Divider", "Divider/Hairline", "State=Default", abs=(x, 1, 1, 46)))
    x += env.num("BORDER_WIDTH") + env.num("TOOLBAR_DIVIDER_GAP_AFTER")
    sw_pad, sw_w = env.num("MODE_SWITCH_PAD"), 4 * env.num("MODE_SWITCH_W") + 2 * env.num("MODE_SWITCH_PAD")
    seg = (sw_w - 2 * sw_pad) / 3
    cells = [mode_cell(env, n, ic, sel, sw_pad + i * seg, sw_pad, seg, tool - 2 * sw_pad) for i, (n, ic, sel) in enumerate(MODES)]
    kids.append(F("Mode switcher", cells, fill=env.c("BG_TOOLBAR_MODE_SWITCHER"), radius=env.c("RADIUS_CONTROL"), w=sw_w, h=tool, sx="fixed", sy="fixed", abs=(x, y, sw_w, tool)))
    # egui paints the stroke on `bar.shrink(0.5)` and snaps its top and left half pixel inward: the line is the bar's second row and column and its last, so it sits on a (1, 1, 511, 47) frame
    highlight = F("Highlight", stroke=env.c("INNER_HIGHLIGHT"), sw=env.c("BORDER_WIDTH"), radius=env.c("RADIUS_FRAME"), w=511.0, h=47.0, sx="fixed", sy="fixed", abs=(1.0, 1.0, 511.0, 47.0))
    return F("Toolbar", [highlight] + kids, fill=env.c("BG_TOOLBAR"), radius=env.c("RADIUS_FRAME"), w=512.0, h=48.0, sx="fixed", sy="fixed")


def component_sets():
    return [
        {"name": "Navigation/Rulers", "row": "Regions", "axes": {}, "uses": ["Divider/Hairline"], "build": rulers},
        {"name": "Navigation/BottomCanvasToolbar", "row": "Regions", "axes": {"State": ["Move", "Comment", "AI", "Dev", "DevAnnotate"]}, "uses": ["Button/Tool", "Divider/Hairline"],
         "build": toolbar},
    ]
