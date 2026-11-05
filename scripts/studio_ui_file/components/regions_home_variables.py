"""Registry `region` Window/Variables (page `03 Studio`, row `REGION_ROW`). Lines are the v0.7.5 tag, `variables/view.rs` (`show` :247, `sidebar` :444-641, `main` :644-749, `bottom_bar` :751-808).

Window/Variables has the registry axis `Layout` (`Full`, `Floating`, `Present`). `Full` is 1391 x 868: the full view (`variables_full`, 49, 32, 1391, 868 in the capture `k-variables`), clipped to it.
The same painters draw the floating panel (`Floating`, `k-variables-floating`: 642 x 380, a 239 px sidebar, a 161 px mode column) from a smaller `Geom`; the scrolled, renamed, filtered and
menu states are gaps, because the registry gives the record no state for them. Every rect is the capture's named rect minus the area's origin (49, 32); the parts are in
`regions_home_variables_parts.py`. The table is a nested Table/Data copy at (241, 49), 521 x 779, its `Mode` text set on the copy (`Style`) and its default rows (the capture's 18: four group
headers, three of them one level in, and 14 colour variables) the component's; the header band to the right of x 762 is not drawn. A screen reaches the table's Slot on the copy (a nested copy's
Slot cannot be filled by a module on 0.7.5, see rest_ir.X). Nested (live): Table/Data, Input/Search `Xs` (the 174 x 24 search field, `field_chrome(.., Size::Xs)`, :679-692, resized from
224 x 32), Divider/Hairline (the bar, sidebar and bottom lines). The registry's
`composes=Popover/Panel,Menu/Item` are the popovers and the row context menu (`run_menu`, `run_alias_picker`): open states, not drawn.
Local: the sidebar (file name, toggle, `Collections` and the settings and add glyphs, 7 collection rows, the `From library` heading and 3 locked rows, `Groups` and 12 group rows: no set draws a
224 x 24 row with a name and a count; List/Row is 247 x 36 with a glyph), the filter button (an own painter: a ring with three lines, only its right corners rounded), the collapse arrows
(`arrows_glyph`, six stroked segments), `Create variable`, the six scope chips (22 high pills; Tag/Chip is the selection pill) and the scrollbar thumb.
Deviations: names are neutral (the capture's collection and group names are a design kit's: the file has `Primitives`, `Colors`, ... and colour ramp names, and the compare sets the capture's text
on a scratch copy); `sliders_glyph` (settings) is the Phosphor `Sliders` glyph. The search field is Input/Search: its glyph is 14 px at 14 and its text starts at 26, where this painter puts a 16
px glyph at 11 and the text at 24, and its hint colour follows Input/Search (#1389).
Gaps: the vertical scrollbar thumb and the end arrows of the horizontal one, hover and drop-target fills.
"""
from components.regions_home_gap2_floating import floating, present
from components.regions_home_parts import REGION_ROW
from components.regions_home_variables_parts import window


def variables(values, env):
    return {"Full": window, "Floating": floating, "Present": present}[values["Layout"]](env)


def component_sets():
    return [{"name": "Window/Variables", "row": REGION_ROW, "axes": {"Layout": ["Full", "Floating", "Present"]}, "build": variables, "uses": ["Divider/Hairline", "Input/Search", "Table/Data"]}]
