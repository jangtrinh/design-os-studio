"""Shared builders of the selection states of Panel/RightInspector (`regions_editor_right_*`): the rows and strips `panels/properties.rs` draws with `pane.y` arithmetic. Lines are the 0.7.5 tag.

A section is a vertical auto-layout frame, so a section hidden by an instance override lets the ones below move up. Its parts, in the order `selection_ui` advances `pane.y`:
* the title row (`title` :207-226): 40 high (`SECTION_TITLE_HEIGHT`), 239 wide, the title in Body Semibold at `SECTION_CONTENT_X`; its actions are 24 px icon buttons at `SECTION_TRAILING_X`
  (Plus, Minus) and 28 px left of it (the Fill / Stroke / Effects styles button, `paint_title` :230-236);
* a group (`sublabel` :300-307 and the row under it): a caption box `LABEL_BOX_H` high, `SPACE_FACT` above its row, the group `SPACE_GROUP` below the group above (a row ends with
  `SPACE_ROW`, the next label adds the difference, :301-303), so the groups of a section sit `SPACE_GROUP` apart and the first one follows its title directly;
* the section's bottom: a row's `SPACE_ROW` and the `pane.y += 4` every section ends with, `SPACE_INLINE` in all (:956, :995, :1003, :1039);
* the hairline between sections (`section_sep` :199-204), the full 240 px of the panel.
A row is a fixed frame of 32 (`FIELD_HEIGHT`) whose children sit where `pane.col` / `segmented` put them (x from the group's left edge, `SECTION_CONTENT_X` in the panel): two fields at 0 and
96 (`FIELD_COL_X`), a strip of 3 x 29 (`SEGMENT_W`) or 4 x 22 (:322).

Size decides how these are drawn: a nested registry instance is stored expanded in every screen that places the region (a nested Section/Header carries its hidden Opacity action too, and each icon
glyph its own vector), and the file's size budget is smaller than a panel of nested instances weighs. So the title, the field, the icon button and the filled button are local frames and icons bound
to the variables of the components they follow (Section/Header, Input/PropertyField, Input/Field, Button/Icon: the looks come from `env.look`); the dropdown (Select/Trigger) and the checkbox stay
nested instances. A strip is local anyway (the painter's `segmented`, :314-351, is no Tabs/Segmented: icons, `BG_TERTIARY` where lit, a 1 px `BG` line between cells).
"""
from rest_ir import F, I, T, X

SEC_W, ROW_W = 239.0, 222.0  # RIGHT_PANEL_WIDTH - 1 (title row, :208); the group's room right of its 17 px inset
SELECT = ("Select/Trigger", "Size=Xs, State=Default")
SMALL = "Caption / Regular"


def rule(env, name):
    """`section_sep`: a 1 px BORDER line across the 240 px of the panel (a childless Divider/Hairline nests only absolutely placed, so a 240 x 1 frame holds it in the column)."""
    line = X(f"{name} line", "Divider/Hairline", "State=Default", abs=(0.0, 0.0, 240.0, env.num("BORDER_WIDTH")))
    return F(name, [line], w=240.0, h=env.num("BORDER_WIDTH"), sx="fixed", sy="fixed")


def icon_button(env, name, icon, x, y=0.0):
    """`widgets::icon_button` at the panel's 24 px action size (widgets/mod.rs:144-150): the 16 px glyph in the kit's resting icon colour, centred in the 24 px box at (x, y)."""
    side = env.num("PANEL_ACTION_BUTTON")
    return I(name, icon, 16, env.c("ICON_QUATERNARY"), abs=(x + (side - 16.0) / 2, y + (side - 16.0) / 2, 16, 16))


def title(env, name, text, plus=False, extra=(), opacity=False):
    """The title row of a section: the title and its action buttons, `plus` at `SECTION_TRAILING_X`, `extra` [(icon, x)] elsewhere on the row, `opacity` the Opacity glyph there instead."""
    h, edge = env.num("SECTION_TITLE_HEIGHT"), env.num("SECTION_TRAILING_X")
    kids = [T(f"{name} heading", text, "Body / Semibold", env.c("TEXT"), abs=(env.num("SECTION_CONTENT_X"), (h - 18.0) / 2, 160.0, 18.0))]
    acts = ([("Plus", edge)] if plus else []) + ([("Opacity", edge)] if opacity else []) + list(extra)
    return F(f"{name} title", kids + [icon_button(env, f"{name} {icon}", icon, x, (h - 24.0) / 2) for icon, x in acts], w=SEC_W, h=h, sx="fixed", sy="fixed")


def button(env, name, label, x, y, w, h, radius="RADIUS_SEGMENT", style=SMALL, tone="TEXT"):
    """A filled text button of the painters: `rect_filled` in 0x2C2C2C and a 0x383838 edge (properties.rs:875-877, 1337-1339, prototype.rs:58-59) at the nearest theme tones (BG_TERTIARY, 6 off, and
    BG_PRESSED, 8 off), the label centred."""
    return F(name, [T("Label", label, style, env.c(tone))], dir="H", fill=env.c("BG_TERTIARY"), stroke=env.c("BG_PRESSED"), sw=env.c("BORDER_WIDTH"), radius=env.c(radius),
             align=("center", "center"), w=w, h=h, sx="fixed", sy="fixed", abs=(x, y, w, h))


def caption(env, text, name=None):
    return T(name or f"{text} label", text, "Caption / Medium", env.c("TEXT_SECONDARY"))


def inset(env):
    return [None, None, None, env.c("SECTION_CONTENT_X")]


def group(env, name, label, *rows):
    """A caption and the rows under it (`SPACE_FACT` apart), 17 px in from the panel's left edge."""
    return F(f"{name} group", ([caption(env, label)] if label else []) + list(rows), dir="V", gap=env.c("SPACE_FACT"), pad=inset(env), w=SEC_W, sx="fixed", sy="hug", align=("start", "start"))


def row(env, name, kids, h=None):
    return F(name, kids, w=ROW_W, h=h or env.num("FIELD_HEIGHT"), sx="fixed", sy="fixed")


def section(env, name, head, groups, bottom="SPACE_INLINE", top=None, gap="SPACE_GROUP"):
    """Title row over a column of groups `gap` apart (`SPACE_GROUP`); `bottom` the variable of the room under the last row (a row's 4 and the section's 4), `top` the room under the title;
    a section without groups is its title row alone."""
    if not groups:
        return F(f"{name} section", [head], dir="V", w=SEC_W, sx="fixed", sy="hug")
    pad = [env.c(top) if top else None, None, env.c(bottom) if bottom else None, None]
    body = F(f"{name} body", groups, dir="V", gap=env.c(gap) if gap else None, pad=pad, w=SEC_W, sx="fixed", sy="hug")
    return F(f"{name} section", [head, body], dir="V", w=SEC_W, sx="fixed", sy="hug")


def box(env, name, x, w, kids, look="Input/PropertyField"):
    """The frame of a field (kit `paint_field`, widgets/kit/field.rs:43-50): the kit's resting fill, a 1 px `BORDER_STRONG` edge, `RADIUS_CONTROL`."""
    look = env.look(look, "Default")
    return F(name, kids, fill=look["fill"], stroke=look["border"], sw=env.c("BORDER_WIDTH"), radius=env.c("RADIUS_CONTROL"), w=w, h=env.num("FIELD_HEIGHT"), sx="fixed", sy="fixed",
             abs=(x, 0.0, w, env.num("FIELD_HEIGHT")))


def field(env, name, x, prefix, value, suffix=None, glyph=None):
    """A 88 x 32 `num_field` (widgets/field.rs:563-644): the prefix letter at `FIELD_PREFIX_X` in TEXT_SECONDARY (or a 12 px glyph there) and the value at `FIELD_PREFIX_W` in TEXT; `suffix` is
    part of the value text (the degree sign and the percent sign follow it directly)."""
    h, px = env.num("FIELD_HEIGHT"), env.num("FIELD_PREFIX_X")
    lead = (I("Glyph", glyph, int(env.num("FIELD_PREFIX_GLYPH")), env.c("TEXT_SECONDARY"), abs=(px, (h - 12.0) / 2, 12, 12)) if glyph else
            T("Prefix", prefix, "Body / Regular", env.c("TEXT_SECONDARY"), abs=(px, 7.0, 16.0, 18.0)))
    return box(env, name, x, env.num("FIELD_WIDTH_HALF"), [lead, T("Value", value + (suffix or ""), "Body / Regular", env.c("TEXT"), abs=(env.num("FIELD_PREFIX_W"), 7.0, 60.0, 18.0))])


def plain_field(env, name, x, value, w):
    """A text input (`kit::paint_field` with the plain look): the value at `FIELD_PLAIN_X`."""
    return box(env, name, x, w, [T("Value", value, "Body / Regular", env.c("TEXT"), abs=(env.num("FIELD_PLAIN_X"), 7.0, w - 16.0, 18.0))], look="Input/Field")


def select(env, name, x, label):
    return X(name, *SELECT, props={"Label": label}, abs=(x, 0.0, env.num("FIELD_WIDTH_HALF"), env.num("FIELD_HEIGHT")))


def cols(env):
    return [0.0, env.num("FIELD_COL_X[1]") - env.num("FIELD_COL_X[0]")]


def strip(env, name, icons, x, lit_cells=(), seg_w=None):
    """The painter's `segmented`: cells of `seg_w` (29, or 22 for four), the lit ones `BG_TERTIARY` with the full icon colour, the rest `BG_SECONDARY`; the end cells round the outer corners."""
    n, seg_w, h = len(icons), seg_w or env.num("SEGMENT_W"), env.num("FIELD_HEIGHT")
    cells = []
    for i, icon in enumerate(icons):
        on = i in lit_cells
        corners = [env.c("RADIUS_CONTROL") if i == 0 else None, env.c("RADIUS_CONTROL") if i == n - 1 else None, env.c("RADIUS_CONTROL") if i == n - 1 else None,
                   env.c("RADIUS_CONTROL") if i == 0 else None]
        paint = {"radius": corners} if any(corners) else {}
        if i:
            paint.update(stroke=env.c("BG"), sw=[None, None, None, env.c("BORDER_WIDTH")])
        tone = env.c("ICON" if on else "ICON_SECONDARY")
        glyph = T("Label", icon[4:], "Body / Regular", tone) if icon.startswith("txt:") else I("Icon", icon, 16, tone)
        cells.append(F(f"{name} cell {i + 1}", [glyph], dir="H", fill=env.c("BG_TERTIARY" if on else "BG_SECONDARY"), align=("center", "center"), w=seg_w, h=h, sx="fixed",
                       sy="fixed", abs=(i * seg_w, 0.0, seg_w, h), **paint))
    return F(name, cells, w=n * seg_w, h=h, sx="fixed", sy="fixed", abs=(x, 0.0, n * seg_w, h))
