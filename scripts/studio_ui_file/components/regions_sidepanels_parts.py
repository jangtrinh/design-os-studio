"""Builders shared by the side-panel regions (`regions_sidepanels_*.py`). Not a component module (no `component_sets`): `discover` skips it. Every part is placed with `abs` at the rect
the Rust computes, relative to the region's top-left, as `color_picker_parts.py` does; a helper returns one node.

The Rust is the v0.7.5 tag (the five side panels are identical on `0c894123`, `git diff v0.7.5 HEAD` is empty for history/panel.rs, comments/panel.rs, find/panel.rs, panels/assets.rs and
panels/check_designs.rs). Nested sets are always `abs` instances of a Default state: an in-flow instance needs its main lowered first, and a Focus variant is never nested."""
from rest_ir import F, I, T, X, lit

SCROLL_THUMB = "panels/mod.rs:70-72 `scrollbar`: the thumb is `rect_filled(.., theme::cr(2), ..)`, a raw 2 (RADIUS_MARK is also 2, but the call does not name it)"


def hairline(y, w, x=0.0):
    """`hline(&p, x0, x1, y, BORDER)`: the Divider/Hairline instance across `w` at `y`."""
    return X(f"Rule {int(y)}", "Divider/Hairline", "State=Default", abs=(x, y, w, 1.0))


def close_button(rect, name="Close"):
    """`widgets::icon_button(.., Icon::Close, ..)` (widgets/mod.rs:144-150): the Button/Close Xs main resized to the caller's 24."""
    return X(name, "Button/Close", "Size=Xs, State=Default", abs=rect)


def icon_button(name, icon, rect, state="Default"):
    """`widgets::icon_button` / `kit::button_icon` at Xs: the Button/Icon Xs main resized to `rect`, its glyph swapped to `icon` (an engine swap, #1427)."""
    return X(name, "Button/Icon", f"Size=Xs, State={state}", props={"Icon": icon}, abs=rect)


def label(name, rect, text, style, colour, env, align=("start", "center"), pad=None, **kw):
    """A text in a fixed box at `rect` (a painter's `Pos2` + `Align2`): `align` places the line in the box, `pad` is [top, right, bottom, left]."""
    return F(name, [T("Text", text, style, env.c(colour))], dir="H", align=align, pad=pad, w=rect[2], h=rect[3], sx="fixed", sy="fixed", abs=rect, **kw)


def scrollbar_thumb(name, list_rect, top, height, env):
    """`panels::scrollbar` (panels/mod.rs:49-73): a 4 px wide thumb 6 px from the list's right edge, `TEXT_TERTIARY` at rest, `cr(2)` corners."""
    x, y, w, _ = list_rect
    return F(name, fill=env.c("TEXT_TERTIARY"), radius=lit(2, SCROLL_THUMB), w=4.0, h=height, sx="fixed", sy="fixed", abs=(x + w - 6.0, y + top, 4.0, height))


def glyph(name, icon, size, colour, env, rect):
    """A glyph centred in the fixed box `rect` (`icons::paint(.., centre, size, colour)`)."""
    return F(name, [I("Icon", icon, size, env.c(colour))], dir="H", w=rect[2], h=rect[3], sx="fixed", sy="fixed", abs=rect)
