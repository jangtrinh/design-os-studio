"""Table/Data of page `03 Studio` (row `Data & Code`). Code/Block is in `data_views.py`. The record has no registry variants, so it is a plain COMPONENT (rule (j)).

Table/Data
* Painter: `table` (variables/view.rs:810-953) with `header_row` (:984-1030, a group row), `variable_row` (:1031-1127, the name cell) and `value_cell` (variables/view.rs:1283-1415). Callers: 1,
  `main` (:644-750) at `variables/view.rs:747` (the Variables modal and its floating form share it). Capture `k-variables`: header (290, 81, 521, 40), group rows `vars-header:*`,
  rows `vars-row:*` (200 x 40) and `vars-cell:*` (280 x 40),
  `vars-add-mode` (772, 81, 39, 40). The capture's names (Colors, Base, white, Style, FFFFFF ...) are kit names: labels in the compare only (G4); the saved texts are neutral.
* Census of the cell painters (nest a W1 set only where its live painter is that set's):
  - Boolean value cell: `kit::toggle` (view.rs:1388-1393, a `SWITCH_W` x `SWITCH_H` = 36 x 20 track) is Toggle/Switch's painter (a boolean row would
    nest it `State=Selected` with `True` 8 after it); the default rows draw none, the capture shows no boolean variable.
  - Colour value cell: `vars-swatch` is `variables/view.rs:367`, its own painter (radius `round(w * 0.2)`, a `BORDER_TRANSLUCENT` stroke): not Swatch/Color, so drawn local. The hex is a text edit
    (`edit_or_show`), painted only as text at rest: not Input/PropertyField. `cell_chrome` (the hover and focus fill) is a gap.
  - Name cell: `kind_glyph` (variables/widgets.rs:35-60, strokes and discs of its own: a palette for Color, a pill and a dot for Boolean) and a `Body` name: no W1 set. Group row: a 12 px chevron
    and a `Body` Semibold label, not Tree/Row (no row look, no toggles). Add-mode: `Icon::Plus` 16 in `ICON`, not Button/Icon (no look, no hover fill at rest).
  - Number and String value cells (a text in an edit rect), the alias pill (`pill`, widgets.rs:117) and the Hidden / `Nsc` caption at the name cell's right are gaps.
* Nested sets: none (the boolean cell's Toggle/Switch is not in the default rows). The registry's `composes=List/Row,Input/PropertyField,Badge/StatusCount` are spec intent: none is painted.
* Local parts: the grid. `hline` / `vline` are Divider/Hairline's painter, but a childless variant nests only as an absolutely placed instance and a row must carry its own line (the rows are a Slot
  that grows), so a row and the header end in a 1 px `BORDER` frame and the cells are separated by 1 px `BORDER` frames (the same pixels). The columns are `VAR_NAME_COL` 200,
  `mode_w` 280 (`VAR_MODE_MAX`,
  for one mode: `clamp(VAR_MODE_MIN, VAR_MODE_MAX)`), `VAR_ADD_COL` 39, each followed by its 1 px line, so a row is 521 wide and `VAR_ROW_PITCH` 41 tall (`VAR_ROW_H` 40 plus the line).
  Header: `BG_SECONDARY`, `Name` and the mode name in `Body` Semibold `TEXT_TERTIARY` 16 in, the `Plus` in the add cell. Group row: `BG_SECONDARY` over the whole row, the chevron box starts 14 in
  (centred at x + 4), the label at 36. Variable row: `BG` (the name cell paints `BG`; the value cells paint nothing over the panel's `BG`), the glyph box at `VAR_ICON_X` and the name at
  `VAR_NAME_X` 44.
* The rows are the `Rows` Slot (rule (m)); the default is the capture's 18 rows (`BODY`): four group headers, three of them one level in, and 14 colour variables (swatch and hex), names neutral; the
  boolean cell (Toggle/Switch) is no longer drawn by default, the capture shows none. The mode header is the `Mode`
  text property (`Value`, what the painter shows for a single mode named `Mode 1`).
* Gaps: hover and selected row fills (`BG_HOVER`, `BG_SELECTED`), the group row's hover fill (`BG_SECONDARY_HOVER`), a folded group (chevron right), a depth > 0 group (16 per level), more than one
  mode column, the horizontal scroll with its sticky name column, the read-only library table, the opacity field and the bind hexagon that appear on hover, the number, string and alias cells, the
  scroll strip, and a Slot filled with rows (the lines belong to each row, so they follow).
"""
from rest_ir import E, F, I, T, lit

LINE_X = "variables/view.rs:894,914: `x0 + 16.0` and `cl + 16.0`, the header labels start 16 in: a literal of the Rust"
CHEVRON_X = "variables/view.rs:992-994: the chevron is painted centred at x + 4 with x = r.left + 16, 12 px wide, so its box starts at 14: a sum with no variable"
CHEVRON_TO_LABEL = "variables/view.rs:992,998: the label starts at x + 20 and the 12 px chevron box ends at 14 + 12 = 26: a difference with no variable"
GLYPH_TO_NAME = "variables/view.rs:1068,1095: VAR_NAME_X (44) - VAR_ICON_X (16) - the 16 px glyph box = 12: a difference of constants"
SWATCH_X = "variables/view.rs:1320: the swatch is at cell.left + 17: a literal of the Rust"
SWATCH_TO_HEX = "variables/view.rs:1320,1334: the swatch is at cell.left + 17, 16 wide, and the hex text at cell.left + 36: 36 - 17 - 16 = 3, a difference of literals"
SWATCH_RADIUS = "variables/view.rs:368 `(rect.width() * 0.2).round()` = round(16 * 0.2) = 3: computed from the swatch's width, no radius role holds it"
SWATCH_STROKE = "variables/view.rs:376 `Stroke::new(1.0, theme::BORDER_TRANSLUCENT)`: a literal of the Rust (BORDER_WIDTH is also 1, but the Rust does not name it)"
GLYPH_STROKE = "variables/widgets.rs:38,54 `Stroke::new(1.2, col)`: a literal of the Rust, no stroke-weight constant in theme.rs"


def cell(name, kids, w, env, fill=None, **kw):
    return F(name, kids, dir="H", align=kw.pop("align", ("start", "center")), fill=fill, w=w, sx="fixed", sy="fill", **kw)


def grid_line(env):
    return F("Grid line", fill=env.c("BORDER"), w=env.num("BORDER_WIDTH"), sx="fixed", sy="fill")


def row_line(env):
    return F("Row line", fill=env.c("BORDER"), h=env.num("BORDER_WIDTH"), sx="fill", sy="fixed")


def row(name, columns, env, fill):
    """One 41 px row: its 40 px cells (name column, the mode column, the add column, a grid line after each) and the 1 px line under it."""
    name_col, mode_col, add_col = columns
    cells = F("Cells", [name_col, grid_line(env), mode_col, grid_line(env), add_col], dir="H", align=("start", "center"), fill=fill, h=env.num("VAR_ROW_H"), sx="fill", sy="fixed")
    return F(name, [cells, row_line(env)], dir="V", align=("start", "start"), sx="fill", sy="hug")


def columns(env, name_kids, mode_kids, add_kids, name_pad=None, mode_pad=None, gap=None, mode_gap=None, mode_w=None):
    n, m, a = env.num("VAR_NAME_COL"), mode_w or env.num("VAR_MODE_MAX"), env.num("VAR_ADD_COL")
    return (cell("Name column", name_kids, n, env, pad=[None, None, None, name_pad], gap=gap), cell("Mode column", mode_kids, m, env, pad=[None, None, None, mode_pad], gap=mode_gap),
            cell("Add column", add_kids, a, env, align=("center", "center")) if add_kids else cell("Add column", [], a, env))


def glyph_colour(env):
    ink, st, c = env.c("ICON"), lit(1.2, GLYPH_STROKE), env.c("BG")
    ring = E("Ring", 12.4, 12.4, stroke=ink, sw=st, sa="center", sx="fixed", sy="fixed", abs=(1.8, 1.8, 12.4, 12.4))
    dots = [E("Dot", 1.8, 1.8, fill=ink, sx="fixed", sy="fixed", abs=(x, y, 1.8, 1.8)) for x, y in ((4.7, 4.9), (7.7, 4.1), (9.9, 6.5), (3.9, 8.1))]
    cut = E("Cutout", 3.8, 3.8, fill=c, stroke=ink, sw=st, sa="center", sx="fixed", sy="fixed", abs=(8.3, 9.3, 3.8, 3.8))
    return [ring, *dots, cut]



def variable(name, glyph, label, mode_kids, env, mode_pad, mode_gap=None, mode_w=None):
    box = env.num("LAYER_ICON_SIZE")
    kind = F("Kind glyph", glyph, w=box, h=box, sx="fixed", sy="fixed")
    cols = columns(env, [kind, T("Name", label, "Body / Regular", env.c("TEXT"))], mode_kids, [], name_pad=env.c("VAR_ICON_X"), gap=lit(12, GLYPH_TO_NAME), mode_pad=mode_pad, mode_gap=mode_gap,
                   mode_w=mode_w)
    return row(name, cols, env, env.c("BG"))


# The capture's rows (`k-variables`, vars-row / vars-header keys): four group headers (the last three one level in) and the 14 colour variables between them: 18 rows, the fourth header cut by the
# horizontal scrollbar as in the capture: (kind, label, hex, Semantic swatch).
# The colours are the capture's values mapped to the nearest theme tone (a swatch fill is bound; the ramp's F5F5F5, E5E5E5 and 525252 have no token of their own).
RAMP = (("FAFAFA", "TEXT"), ("F5F5F5", "TEXT"), ("E5E5E5", "TEXT_SECONDARY"), ("D4D4D4", "TEXT_SECONDARY"), ("A3A3A3", "TEXT_TERTIARY"), ("737373", "ICON_QUATERNARY"), ("525252", "BORDER_STRONG"),
        ("404040", "BORDER_STRONG"), ("262626", "BG_TERTIARY"), ("171717", "BG_SECONDARY"), ("0A0A0A", "BG"))
BODY = ((("group", "Group", 0), ("group", "Group B", 1)) + tuple(("color", f"Color {i}", h, c) for i, (h, c) in enumerate((("FFFFFF", "TEXT"), ("000000", "BG"), ("FFFFFF", "TEXT_SECONDARY")), 1))
        + (("group", "Group C", 1),) + tuple(("color", f"Color {i}", h, c) for i, (h, c) in enumerate(RAMP, 4)) + (("group", "Group D", 1),))


def body_rows(env, spec, mode_w=None):
    """The rows of `spec` ((kind, label, depth) / (kind, label, hex, swatch token)), each one `row` of the grid."""
    out = []
    for i, entry in enumerate(spec):
        if entry[0] == "group":
            chevron, label = I("Chevron", "ChevronDown", 12, env.c("TEXT_TERTIARY")), T("Label", entry[1], "Body / Semibold", env.c("TEXT_TERTIARY"))
            pad = lit(14 + 16 * entry[2], CHEVRON_X)
            out.append(row(f"Group row {i + 1}", columns(env, [chevron, label], [], [], name_pad=pad, gap=lit(10, CHEVRON_TO_LABEL), mode_w=mode_w), env, env.c("BG_SECONDARY")))
            continue
        swatch = F("Swatch", fill=env.c(entry[3]), stroke=env.c("BORDER_TRANSLUCENT"), sw=lit(1, SWATCH_STROKE), radius=lit(3, SWATCH_RADIUS), w=env.num("VAR_SWATCH"), h=env.num("VAR_SWATCH"),
                   sx="fixed", sy="fixed")
        out.append(variable(f"Colour row {i + 1}", glyph_colour(env), entry[1], [swatch, T("Hex", entry[2], "Body / Regular", env.c("TEXT"))], env, lit(17, SWATCH_X), lit(3, SWATCH_TO_HEX),
                            mode_w=mode_w))
    return out


def table_header(env, mode_w=None, prop="Mode"):
    head = columns(env, [T("Label", "Name", "Body / Semibold", env.c("TEXT_TERTIARY"))], [T("Label", "Value", "Body / Semibold", env.c("TEXT_TERTIARY"), prop=prop)],
                   [I("Icon", "Plus", int(env.num("LAYER_ICON_SIZE")), env.c("ICON"))], name_pad=lit(16, LINE_X), mode_pad=lit(16, LINE_X), mode_w=mode_w)
    cells = F("Cells", [head[0], grid_line(env), head[1], grid_line(env), head[2]], dir="H", align=("start", "center"), fill=env.c("BG_SECONDARY"), h=env.num("VAR_HEAD_H"), sx="fill", sy="fixed")
    return F("Header", [cells, row_line(env)], dir="V", align=("start", "start"), sx="fill", sy="hug")


def table(values, env):
    rows = F("Rows", body_rows(env, BODY), dir="V", align=("start", "start"), slot="Rows", sx="fill", sy="hug")
    width = env.num("VAR_NAME_COL") + env.num("VAR_MODE_MAX") + env.num("VAR_ADD_COL") + 2 * env.num("BORDER_WIDTH")
    return F("Table", [table_header(env), rows], dir="V", align=("start", "start"), w=width, sx="fixed", sy="hug", clip=True)


def component_sets():
    return [{"name": "Table/Data", "row": "Data & Code", "axes": {}, "build": table,
             "props": [{"name": "Mode", "type": "TEXT", "default": "Value"}, {"name": "Rows", "type": "SLOT"}]}]
