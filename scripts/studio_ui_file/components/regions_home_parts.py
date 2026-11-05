"""Shared pieces of the Home, window, present and dev region modules (`regions_home_*.py`): the page row they sit in and the small nested-instance helpers they all place.

Page row: every region sits in the `Regions` row of `STUDIO_ROWS` (components/__init__.py), which the assembler lowers last, so every set a region nests is lowered before it. The row name lives here
only (`REGION_ROW`).

Regions have no registry variants (design/DESIGN.md gives every `region` record `Variants: -`), so each is ONE plain COMPONENT (`axes: {}`) of the size the 0.7.5 capture shows it. A region is a
fixed frame: its parts are placed with `abs` at the capture's own rects (a named rect of `h-home.layout.json` etc. minus the region's origin), because the Rust lays them out in egui rects and not
in a stack. Every part is a nested instance of a W1/W2 set where one exists; what no set draws is a local frame, named in the module's docstring.
"""
from rest_ir import F, T, X

REGION_ROW = "Regions"
RULE = ("Divider/Hairline", "State=Default")


def rule(name, x, y, w):
    """A 1 px `BORDER` line: Divider/Hairline resized to `w` and placed (the Popover/ColorPicker precedent)."""
    return X(name, RULE[0], RULE[1], abs=(x, y, w, 1))


def icon_button(name, icon, x, y, side=24, size="Xs", state="Default"):
    """`widgets::icon_button` / `kit::button_icon`: a Button/Icon copy resized to `side` and placed (the Section/Header precedent), glyph swapped to `icon`."""
    return X(name, "Button/Icon", f"Size={size}, State={state}", props={"Icon": icon}, abs=(x, y, side, side))


def region(name, kids, env, w, h, fill="BG"):
    """The region's root: a fixed `w` x `h` frame in the page colour (`fill=None`: no fill, for a region drawn over the caller's page) whose children are all placed."""
    paint = {"fill": env.c(fill)} if fill else {}
    return F(name, kids, dir="V", w=w, h=h, sx="fixed", sy="fixed", align=("start", "start"), **paint)


def label(name, content, style, color, x, y, w, h):
    """A text placed by its line box: (x, y) is the box's top-left, `h` the style's line height."""
    return T(name, content, style, color, abs=(x, y, w, h))
