"""The stops row of Popover/ColorPicker Kind=Gradient (`widgets/picker.rs` at v0.7.5, the capture `f-picker-gradient`). Not a component module (no `component_sets`).

`show` (:912-942) draws it in the `STOPS_H` 40 band (:18) under the paint-type row (the mode is a gradient, so the contrast row :949 is not drawn): `gradient_bar` (:1444-1544) at
(`PAD`, y + 12, `AREA` - 56 = 152, 16) :913-914, then the flip (`Icon::FlipH`, :915, :917) and rotate (`Icon::Rotate90`, :916, :927) icon buttons, 24 px at x PAD + 152 + 4 = 172
and 200, y + 8.
* The bar (:1448-1479): `checker` (4 px cells, CHECKER_A / CHECKER_B), the stops' colours as a mesh left to right, `round_corners` at `RADIUS_CONTROL` and a 1 px
  `BORDER_TRANSLUCENT` inside stroke on top (:1478). The capture's paint is white at alpha 1 to white at alpha 0, so the mesh is a white ramp from 255 to 0 over the checker.
* A handle (:1497-1533) is a 12 x 16 white tile (`cr(4)` :1526) centred on the stop at the bar's mid height, with an inner `shrink(3.0)` (selected) or `shrink(2.5)` rect of `cr(2)`
  (:1529) over a 3 px checker holding the stop's colour; the selected one has a 1 px `BORDER_SELECTED` ring (`cr(6)`, :1531) 2 px outside the tile. The 1 px black-alpha 0x4D halo
  (:1525) is omitted (a raw alpha of black: a gap, as in the thumbs). Stop 0 (white, selected) and stop 1 (white at alpha 0) are the capture's.
* Gaps: a third stop, the hover and drag states, the second paint-type button rows (radial, angular and diamond draw the same row), the Image mode.
"""
from components.color_picker_parts import button
from components.color_picker_strips import CELL
from rest_ir import F, grad, lit

BAR = (16, 92, 152, 16)
EDGE = "widgets/picker.rs:1478 `Stroke::new(1.0, BORDER_TRANSLUCENT)`: a literal of the Rust, no stroke-weight constant (BORDER_WIDTH is also 1, but `gradient_bar` does not name it)"
TILE_RADIUS = "widgets/picker.rs:1526 `theme::cr(4)`: a literal of the Rust, 4 is no radius role (RADIUS_MARK 2, RADIUS_SEGMENT 6)"
INNER_RADIUS = "widgets/picker.rs:1529 `theme::cr(2)`: a literal of the Rust (RADIUS_MARK is also 2, but `gradient_bar` does not name it)"
RING_RADIUS = "widgets/picker.rs:1531 `theme::cr(6)`: a literal of the Rust (RADIUS_SEGMENT is also 6, but `gradient_bar` does not name it)"
HANDLE_RING = "widgets/picker.rs:1531 `Stroke::new(1.0, BORDER_SELECTED)`: a literal of the Rust, no stroke-weight constant"
RAMP_REASON = "widgets/picker.rs:1454-1475 `gradient_bar`: the stop's colour (white) at alpha {a}: an alpha of a computed colour no Semantic variable holds"


def bar(env):
    w, h = BAR[2], BAR[3]
    cells = [F(f"Checker {i},{j}", fill=env.c("CHECKER_B"), w=CELL, h=CELL, sx="fixed", sy="fixed", abs=(CELL * i, CELL * j, CELL, CELL))
             for j in range(h // CELL) for i in range(w // CELL) if (i + j) % 2 == 1]
    ramp = grad([(0, (255, 255, 255, 255), RAMP_REASON.format(a=255)), (1, (255, 255, 255, 0), RAMP_REASON.format(a=0))])
    cells.append(F("Ramp", fills=[ramp], w=w, h=h, sx="fixed", sy="fixed", abs=(0, 0, w, h)))
    cells.append(F("Edge", stroke=env.c("BORDER_TRANSLUCENT"), sw=lit(1, EDGE), radius=env.c("RADIUS_CONTROL"), w=w, h=h, sx="fixed", sy="fixed", abs=(0, 0, w, h)))
    return F("Stops bar", cells, fill=env.c("CHECKER_A"), clip=True, radius=env.c("RADIUS_CONTROL"), w=w, h=h, sx="fixed", sy="fixed", abs=BAR)


def handle(env, name, centre_x, selected):
    """A stop handle: the 12 x 16 white tile and its inner swatch; the selected one has the 1 px ring outside."""
    inset = 3.0 if selected else 2.5
    iw, ih = 12 - 2 * inset, 16 - 2 * inset
    kids = [F("Inner", [F(f"Checker {i},{j}", fill=env.c("CHECKER_B"), w=3.0, h=3.0, sx="fixed", sy="fixed", abs=(3.0 * i, 3.0 * j, 3.0, 3.0))
                        for j in range(4) for i in range(3) if (i + j) % 2 == 1] if not selected else [], fill=env.c("ICON") if selected else env.c("CHECKER_A"), clip=True,
              radius=lit(2, INNER_RADIUS), w=iw, h=ih, sx="fixed", sy="fixed", abs=(inset, inset, iw, ih))]
    tile = F(name, kids, fill=env.c("ICON"), radius=lit(4, TILE_RADIUS), w=12.0, h=16.0, sx="fixed", sy="fixed", abs=(centre_x - 6.0, BAR[1] + 8.0 - 8.0, 12.0, 16.0))
    if not selected:
        return [tile]
    ring = F(f"{name} ring", stroke=env.c("BORDER_SELECTED"), sw=lit(1, HANDLE_RING), radius=lit(6, RING_RADIUS), w=18.0, h=22.0, sx="fixed", sy="fixed",
             abs=(centre_x - 9.0, BAR[1] + 8.0 - 11.0, 18.0, 22.0))
    return [tile, ring]


def stops_row(env):
    """The stops bar, its two handles (stop 0 selected at the left end, stop 1 at the right end) and the flip and rotate buttons."""
    return [bar(env), *handle(env, "Stop 1", BAR[0] + 0.0, True), *handle(env, "Stop 2", BAR[0] + float(BAR[2]), False),
            button("Flip", "Button/Icon", "Size=Xs, State=Default", (172, 88, 24, 24), "FlipH"), button("Rotate", "Button/Icon", "Size=Xs, State=Default", (200, 88, 24, 24), "Rotate90")]
