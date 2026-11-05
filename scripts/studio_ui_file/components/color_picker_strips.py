"""The picker's three colour strips as real gradients (`picker.rs` v0.7.5), at the capture's state: hue h = 0, saturation 0, value 1, alpha 1 (the `f-picker` capture: the SV thumb in the
top-left corner, the hue thumb at the left end, the alpha thumb at the right end, a white colour). Not a component module (no `component_sets`): `discover` skips it.

* SV square (`sv_area`, :637-656): a 24 x 24 mesh whose corner colours are `Hsva(hue, s, v)`. The colour at (s, v) is `v * lerp(white, hue colour, s)`, the product of two linear terms, so the
  mesh reproduces it exactly with two layers: a left-to-right gradient from white to the hue colour, and over it a black gradient from alpha 0 at the top to alpha 1 at the bottom (`v = 1 - y`).
* Hue strip (`hue_bar`, :658-671): seven stops, `Hsva(i / 6, 1, 1)` for i = 0..6, left to right.
* Alpha strip (`checker` :612-623 then `quad_mesh` :625-635, called at :1045-1053): a `CHECKER_A` base with `CHECKER_B` cells of 4 px where `(i + j) % 2 == 1`, then the colour at alpha 0 on
the left to alpha 1 on the
  right over it. The base colour is `new_hsv` at the capture's state: white.
Each strip is the rect `picker::show` computes, with `round_corners(.., 6.0, BG)` (the SV square `RADIUS_CONTROL`) as the frame's own radius. The thumbs and the 24-dot AA contrast curve of the
SV square are separate parts (`thumb`, a gap).
"""
from components.color_picker_parts import ALPHA_RADIUS, HUE_RADIUS
from rest_ir import F, grad, lit

SV_RECT, HUE_RECT, ALPHA_RECT = (16, 122, 208, 208), (52, 343, 172, 12), (52, 371, 172, 12)
SV_REASON = "widgets/picker.rs:637-656 `sv_area`: the mesh colour at (s, v) is `Hsva(hue, s, v)`; the capture's hue is h = 0, so the right edge is the colour {}"
HUE_REASON = "widgets/picker.rs:658-671 `hue_bar`: stop {i} is `Hsva({i} / 6, 1, 1)` = {rgb}, a computed colour no Semantic variable holds"
ALPHA_REASON = ("widgets/picker.rs:1045-1053 `checker` then `quad_mesh`: the base colour (`new_hsv` at the capture's state, white) at alpha {a}: "
                "an alpha of a computed colour no Semantic variable holds")
CELL = 4


def shifted(rect, dy):
    """The strip's rect `dy` lower (a picker without the paint-type row is 40 higher, one with the stops row 14 lower than the solid one)."""
    return (rect[0], rect[1] + dy, rect[2], rect[3])


def hue_colour(i):
    """`Hsva(i / 6, 1, 1)` as integers: the six corners of the RGB cube's hue ring, red at both ends."""
    return [(255, 0, 0), (255, 255, 0), (0, 255, 0), (0, 255, 255), (0, 0, 255), (255, 0, 255), (255, 0, 0)][i]


def sv_square(env, dy=0):
    red = (255, 0, 0, 255)
    across = grad([(0, (255, 255, 255, 255), "widgets/picker.rs:637-656 `sv_area`: s = 0 is white at any hue (`Hsva(hue, 0, v)` is grey `v`, white at v = 1)"),
                   (1, red, SV_REASON.format("FF0000"))])
    down = grad([(0, (0, 0, 0, 0), "widgets/picker.rs:637-656 `sv_area`: v = 1 at the top is the colour itself, so the black layer is fully transparent"),
                 (1, (0, 0, 0, 255), "widgets/picker.rs:637-656 `sv_area`: v = 0 at the bottom is black at any hue and saturation")], start=(0.5, 0.0), end=(0.5, 1.0))
    return F("SV square", fills=[across, down], radius=env.c("RADIUS_CONTROL"), w=SV_RECT[2], h=SV_RECT[3], sx="fixed", sy="fixed", abs=shifted(SV_RECT, dy))


def hue_strip(env, dy=0):
    stops = [(i / 6, (*hue_colour(i), 255), HUE_REASON.format(i=i, rgb="#%02X%02X%02X" % hue_colour(i))) for i in range(7)]
    return F("Hue strip", fills=[grad(stops)], radius=lit(6, HUE_RADIUS),
             w=HUE_RECT[2], h=HUE_RECT[3], sx="fixed", sy="fixed", abs=shifted(HUE_RECT, dy))


def alpha_strip(env, dy=0):
    w, h = ALPHA_RECT[2], ALPHA_RECT[3]
    cells = [F(f"Checker {i},{j}", fill=env.c("CHECKER_B"), w=CELL, h=CELL, sx="fixed", sy="fixed", abs=(CELL * i, CELL * j, CELL, CELL))
             for j in range(-(-h // CELL)) for i in range(-(-w // CELL)) if (i + j) % 2 == 1]
    ramp = grad([(0, (255, 255, 255, 0), ALPHA_REASON.format(a=0)), (1, (255, 255, 255, 255), ALPHA_REASON.format(a=255))])
    cells.append(F("Alpha ramp", fills=[ramp], w=w, h=h, sx="fixed", sy="fixed", abs=(0, 0, w, h)))
    return F("Alpha strip", cells, fill=env.c("CHECKER_A"), clip=True, radius=lit(6, ALPHA_RADIUS),
             w=w, h=h, sx="fixed", sy="fixed", abs=shifted(ALPHA_RECT, dy))
