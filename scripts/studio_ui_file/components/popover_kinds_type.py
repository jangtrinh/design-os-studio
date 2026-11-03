"""The two popovers with a layout of their own: the Type settings popover (Basic tab, 312 x 728) and the prototype Interaction details popover (240 x 306). Not a component module.
Lines are the v0.7.5 tag of crates/designos-app/src. Each part is placed with `abs` at the rect its painter's arithmetic gives, relative to the popover.

* TypeSettings: `panels/typography.rs:596` `details_popover` (`DETAILS_W` 312, :491): `popover::show` with a 40 px header, then rows from 8 below it. Label at x 16, control at
  `DETAIL_CONTROL_X` 152 (:493) 144 wide, 24 high (`detail_row`, :534-540), a 32 px pitch (`popover::ROW_H`). The three-tab strip and the Case / Decor / Direction / Wrap strips are `seg_rect`
  (:500-531), a second segmented painter (no inset, cells share the width, BG_SECONDARY and BG_TERTIARY, Body Semibold labels, a BG line between cells): drawn local, not Tabs/Segmented.
  The `Ag` preview is a 68 px box with two guide lines; its glyph is 24 px, which no text style holds: drawn in Title / Semibold (20 px), a recorded gap.
* Interaction: `panels/prototype.rs:518` `popover` (`PROTO_POPOVER_WIDTH` 240, theme.rs:843): a title row (`title`, panels/properties.rs:207) with the Minus action, `label_ctl` rows
  (panels/prototype.rs:458, label at
  x 17 - 1, a 112 px control at 72 after it, `FIELD_HEIGHT` 32, pitch `FIELD_HEIGHT` + 8), a BORDER_MENU line before and after the actions, the `Add action` row and the Animation row.
"""
from components.popover_kinds_controls import FIELD, PROPERTY, check, select, text_field
from components.popover_kinds_shell import hairline, header, label_text, place, surface
from rest_ir import F, I, T, X, lit

GUIDE = "panels/typography.rs:646 `from_rgba_unmultiplied(0x0D, 0x99, 0xFF, 110)`: INFO at alpha 110/255, a literal of the Rust"
CONTROL_X, CONTROL_W = 152.0, 144.0
# (label, y from the popover's top, control kind, its argument, label dimmed):
# panels/typography.rs:714-780 (the OpenType rows, the stylistic sets from :765) and :805-857 (the paragraph rows to Wrap style, :850), 32 px apart
DETAIL_ROWS = (("Number spacing", 160.0, "select", "Default", False), ("Number style", 192.0, "select", "Default", True), ("Fractions", 224.0, "check", False, False),
               ("Slashed zero", 256.0, "check", False, False), ("Case sensitive forms", 288.0, "check", False, False), ("Ligatures", 320.0, "check", True, True),
               ("Contextual alternates", 352.0, "check", True, False), ("Paragraph indent", 456.0, "field", "0", False), ("Paragraph spacing", 488.0, "field", "36", False),
               ("List spacing", 520.0, "field", "0", False), ("Hanging punctuation", 552.0, "check", False, False), ("Hanging lists", 584.0, "check", False, False),
               ("Vertical trim", 616.0, "select", "Standard", False), ("Text direction", 648.0, "strip", ("Auto", "LTR", "RTL"), False),
               ("Wrap style", 680.0, "strip", ("Auto", "Balance", "Pretty"), False))


def strip(env, name, labels, rect, selected=0):
    """`seg_rect` (panels/typography.rs:500): `n` cells sharing the width, the first and last rounded on their outer side, a BG line left of every cell but the first."""
    x, y, w, h = rect
    n, cw, r = len(labels), w / len(labels), env.c("RADIUS_CONTROL")
    cells = []
    for i, label in enumerate(labels):
        on = i == selected
        radius = r if n == 1 else [r, None, None, r] if i == 0 else [None, r, r, None] if i == n - 1 else None
        kids = [T("Label", label, "Body / Semibold", env.c("ICON" if on else "ICON_SECONDARY"))]
        if i > 0:
            kids.append(F("Divider", fill=env.c("BG"), w=1.0, h=h, sx="fixed", sy="fixed", abs=(0.0, 0.0, 1.0, h)))
        shape = {"radius": radius} if radius else {}
        cells.append(F(f"Cell {i + 1}", kids, dir="H", fill=env.c("BG_TERTIARY" if on else "BG_SECONDARY"), w=cw, h=h, sx="fixed", sy="fixed", abs=(i * cw, 0.0, cw, h), **shape))
    return F(name, cells, w=w, h=h, sx="fixed", sy="fixed", abs=(x, y, w, h))


def preview(env):
    """The `Ag` box (panels/typography.rs:642-654): 68 px, a BORDER edge, the cap-height and baseline guides 6 in from each side."""
    guide = lambda n, y: F(n, fill=env.c("INFO"), alpha=lit(110 / 255, GUIDE), w=56.0, h=1.0, sx="fixed", sy="fixed", abs=(6.0, y, 56.0, 1.0))
    ag = T("Glyph", "Ag", "Title / Semibold", env.c("TEXT"), abs=(18.0, 16.0, 32.0, 30.0))
    return F("Preview", [guide("Cap height", 21.0), guide("Baseline", 49.0), ag], fill=env.c("BG_SECONDARY"), stroke=env.c("BORDER"), sw=env.c("BORDER_WIDTH"),
             radius=env.c("RADIUS_CONTROL"), w=68.0, h=68.0, sx="fixed", sy="fixed", abs=(16.0, 80.0, 68.0, 68.0))


def control(env, kind, arg):
    """The nodes of a detail row's 144 px control column (row-relative, named `Control`)."""
    col = (CONTROL_X, 0.0, CONTROL_W, 24.0)
    if kind == "strip":
        return [strip(env, "Control", arg, col)]
    node = {"select": lambda: select(arg), "field": lambda: text_field(arg), "check": lambda: check(arg, CONTROL_W)}[kind]()
    node["abs"] = col
    return [node]


def mode_strip(env, label, labels, y):
    """The Case or Decor row beside the preview: the label at x 94 and the strip at 152, 24 high."""
    return [label_text(env, label, x=94.0, w=50.0, top=y), strip(env, label, labels, (CONTROL_X, y, CONTROL_W, 24.0))]


def type_settings(env):
    w = 312.0
    chip = lambda i: F(f"Set {i}", [T("Label", str(i), "Body / Regular", env.c("TEXT_SECONDARY"))], dir="H", fill=env.c("BG_SECONDARY"), radius=env.c("RADIUS_CONTROL"),
                       w=32.0, h=24.0, sx="fixed", sy="fixed", abs=(16.0 + (i - 1) * 34.0, 416.0, 32.0, 24.0))
    rows = [[label_text(env, label, w=130.0, dim=dim, top=y), *place(label, control(env, kind, arg), y)] for label, y, kind, arg, dim in DETAIL_ROWS]
    top = [*header(env, "Type settings", w), strip(env, "Tabs", ["Basic", "Details", "Variable"], (16.0, 48.0, 280.0, 24.0)), preview(env),
           *mode_strip(env, "Case", ["\u2014", "AG", "ag", "Ag"], 86.0), *mode_strip(env, "Decor", ["None", "U", "S"], 118.0), hairline(env, "Rule 1", 152.0, w)]
    sets = [label_text(env, "Stylistic sets", w=130.0, top=384.0), *(chip(i) for i in range(1, 9)), hairline(env, "Rule 2", 444.0, w)]
    return surface(env, [*top, *(n for r in rows[:7] for n in r), *sets, *(n for r in rows[7:] for n in r)], w, 728.0)


def interaction(env):
    w, row, ctl_x = 240.0, 7.0, 88.0
    label = lambda text, y, dy=row: T(f"{text} label", text, "Body / Regular", env.c("TEXT_SECONDARY"), abs=(16.0, y + dy, 72.0, 18))
    sel = lambda name, text, y, x=ctl_x, cw=112.0: X(name, "Select/Trigger", FIELD, props={"Label": text}, abs=(x, y, cw, 32.0))
    field = lambda name, prefix, x: X(name, "Input/PropertyField", PROPERTY, props={"Prefix": prefix, "Value": "0"}, abs=(x, 169.0, 52.0, 24.0))
    plus = I("Plus", "Plus", 12, env.c("ICON_SECONDARY"), abs=(24.0, 209.0 + 10.0, 12, 12))
    minus = X("Remove", "Button/Icon", "Size=Xs, State=Default", props={"Icon": "Minus"}, abs=(env.num("SECTION_TRAILING_X") - 1, 8.0, 24.0, 24.0))
    title = T("Title", "Interaction details", "Body / Semibold", env.c("TEXT"), abs=(16.0, 11.0, 160.0, 18))
    add = T("Add action", "Add action", "Body / Regular", env.c("TEXT_SECONDARY"), abs=(38.0, 209.0 + row, 120.0, 18))
    kids = [title, minus, label("Trigger", 40.0), sel("Trigger", "On click", 40.0), hairline(env, "Rule 1", 80.0, w), label("Action", 89.0), sel("Action", "Scroll to", 89.0, 68.0, 132.0),
            label("Scroll to", 129.0), sel("Scroll to", "Button", 129.0), label("Offset", 169.0, 3.0), field("Offset X", "X", ctl_x), field("Offset Y", "Y", 148.0), plus, add,
            hairline(env, "Rule 2", 249.0, w), label("Animation", 258.0), sel("Animation", "Instant", 258.0)]
    return surface(env, kids, w, 306.0)


TYPE = {"TypeSettings": type_settings, "Interaction": interaction}
