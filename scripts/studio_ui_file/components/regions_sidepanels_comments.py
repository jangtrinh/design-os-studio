"""Registry `region` Panel/Comments (page `03 Studio`), one plain COMPONENT (the registry gives the record no variants). Lines are the v0.7.5 tag (comments/panel.rs, comments/ui.rs).

Painter: `show` (comments/panel.rs:19-201), one caller; the capture is `j-comments`, the region is the right panel (1199, 32, 241, 868) and every rect is relative to its top-left. State
drawn: the capture's, `This page` and `Open` selected, no search text, one open thread, `Show resolved` off.
* Nested sets (live): Button/Close (`icon_button` Close, :34-35, 24 px at right - 8 - 24), Tabs/Segmented State=Selected twice (`ui::segmented`, :46 and :53: `kit::segmented` Xs, 216 x
  `COMMENT_FILTER_H` 24 at x `SIDE` + 1 = 13, 8 apart), Input/Field Xs (the search field's frame, `kit::paint_field` Xs, :60-63, 216 x `CONTROL_XS` 32, empty: `Has Value` off) and
  Divider/Hairline (`hline(rect.left() + 1, rect.right(), y, BORDER)` under the header :40, under the search row :88 and above the footer :93).
* The search field is not `search_look` (Input/Search, the registry's `composes`): the painter draws the field frame, then its own 16 px `ICON_SECONDARY` glyph centred at left + 14 and a
  TextEdit from
  left + 28 whose hint is `TEXT_PLACEHOLDER` (:64-86; it sets `weak_text_color`). Input/Search Xs draws a 14 px glyph in the tertiary tone centred at 14 and the text from 26, so it would differ by
  2 px and by the glyph's colour: the frame is the Input/Field Xs instance and the glyph and hint are the caller's own local parts, as in production.
* Local: the left edge (`vline`, :22), the title (`font_large_strong`, TEXT, 16 in: a raw 16.0, :27-33), the search glyph and hint, the thread list and the footer. The list is the SLOT `Threads`: a
  clipped column from y 162 (8 above the first card, :152; cards inset `SIDE` 12, :13) holding the capture's one card. A card (:154-195) is `BG_SECONDARY`, `BG_SELECTED` when its thread is
  open (:160-161,
  the capture opens it), with a `BORDER` inside stroke at `RADIUS_CONTROL`, padded `COMMENT_CARD_PAD` 12: a 20 px avatar circle (`ui::avatar`, `AVATAR_COLORS`, the initial in Body Semibold,
  ui.rs:14-18),
  the author in Body Semibold 8 after it and the age in Caption `TEXT_TERTIARY` 8 after that (both on one baseline, 14 below the avatar's top), the first message in Body `TEXT` 6 under the
  avatar. Its height
  is computed (24 + 20 + 6 + the galley's 15 = 65). The footer (:91-107) is a hairline, an unchecked 14 px box (`Stroke::new(1.0, TEXT_TERTIARY)` at `RADIUS_MARK`) and `Show resolved` in
  Body `TEXT` 8 after it.
  The author and message are document data: neutral (`Agent`, `Align icon spacing to 8px grid`, the same width as the capture's text).
* Gaps: the checked box (:97-99), a closed card's fill (`BG_SECONDARY`, `BG_SECONDARY_HOVER` on hover), the reply count and page line (:181-191), the `No comments yet` text (:113-117), the avatar
  colours of other authors (`avatar_index`), the scrollbar (the one card fits).
"""
from components.regions_sidepanels_parts import close_button, glyph, hairline, label
from rest_ir import F, T, X, lit

ROW = "Regions"
W, H = 241.0, 868.0
SIDE = "comments/panel.rs:13 `const SIDE: f32 = 12.0`: a local constant, the list's inset on each side (INSET_BUTTON, SPACE_GROUP and COMMENT_PAD also hold 12)"
LIST_TOP = "comments/panel.rs:152 `list.top() + 8.0 - scroll`: a raw 8.0 above the first card"
TITLE_X = "comments/panel.rs:28 `rect.left() + 16.0`: a raw 16.0 (PANEL_PADDING_X and INSET_PANEL hold 16 for other roles)"
AVATAR_R = "comments/ui.rs:15 `circle_filled(center, diameter / 2.0, ..)`: radius half of the 20 px avatar, computed from its size"
BOX_STROKE = "comments/panel.rs:101 `Stroke::new(1.0, TEXT_TERTIARY)`: a literal of the Rust (BORDER_WIDTH is also 1, but the call does not name it)"


def text(name, rect, content, style, colour, env):
    return T(name, content, style, env.c(colour), abs=rect)


def thread_card(env):
    avatar = F("Avatar", [T("Initial", "A", "Body / Semibold", env.c("TEXT"))], dir="H", fill=env.c("AVATAR_COLORS[0]"), radius=lit(10, AVATAR_R), w=20.0, h=20.0, sx="fixed", sy="fixed",
               abs=(12.0, 12.0, 20.0, 20.0))
    kids = [avatar, text("Author", (40.0, 13.0, 36.0, 18.0), "Agent", "Body / Semibold", "TEXT", env), text("Age", (84.0, 13.0, 50.0, 18.0), "just now", "Caption / Regular", "TEXT_TERTIARY", env),
            text("Message", (12.0, 37.0, 192.0, 18.0), "Align icon spacing to 8px grid", "Body / Regular", "TEXT", env)]
    return F("Thread 1", kids, fill=env.c("BG_SELECTED"), stroke=env.c("BORDER"), sw=env.c("BORDER_WIDTH"), radius=env.c("RADIUS_CONTROL"), w=216.0, h=65.0, sx="fixed", sy="fixed")


def comments_panel(values, env):
    side = lit(12, SIDE)
    segmented = lambda name, y, first, second: X(name, "Tabs/Segmented", "State=Selected", props={"First label": first, "Second label": second}, abs=(13.0, y, 216.0, 24.0))
    threads = F("Threads", [thread_card(env)], dir="V", slot="Threads", pad=[lit(8, LIST_TOP), side, None, side], w=240.0, h=666.0, sx="fixed", sy="fixed", clip=True, align=("start", "start"),
                abs=(1.0, 162.0, 240.0, 666.0))
    box = F("Show resolved box", stroke=env.c("TEXT_TERTIARY"), sw=lit(1, BOX_STROKE), radius=env.c("RADIUS_MARK"), w=14.0, h=14.0, sx="fixed", sy="fixed", abs=(13.0, 841.0, 14.0, 14.0))
    kids = [F("Left edge", fill=env.c("BORDER"), w=1.0, h=H, sx="fixed", sy="fixed", abs=(0.0, 0.0, 1.0, H)),
            label("Title", (0.0, 0.0, W, 48.0), "Comments", "Large / Semibold", "TEXT", env, pad=[None, None, None, lit(16, TITLE_X)]), close_button((209.0, 12.0, 24.0, 24.0)),
            hairline(48.0, 240.0, 1.0), segmented("Page filter", 57.0, "This page", "All pages"), segmented("Status filter", 89.0, "Open", "Resolved"),
            X("Search field", "Input/Field", "Size=Xs, State=Default", props={"Has Value": False}, abs=(13.0, 121.0, 216.0, 32.0)),
            glyph("Search icon", "Search", 16, "ICON_SECONDARY", env, (19.0, 129.0, 16.0, 16.0)),
            label("Search hint", (41.0, 121.0, 180.0, 32.0), "Search comments", "Body / Regular", "TEXT_PLACEHOLDER", env), hairline(161.0, 240.0, 1.0), threads,
            hairline(828.0, 240.0, 1.0), box, label("Show resolved", (35.0, 836.0, 120.0, 24.0), "Show resolved", "Body / Regular", "TEXT", env)]
    return F("Comments", kids, fill=env.c("BG"), w=W, h=H, sx="fixed", sy="fixed", clip=True)


def component_sets():
    return [{"name": "Panel/Comments", "row": ROW, "axes": {}, "uses": ["Button/Close", "Divider/Hairline", "Input/Field", "Tabs/Segmented"],
             "props": [{"name": "Threads", "type": "SLOT"}], "build": comments_panel}]
