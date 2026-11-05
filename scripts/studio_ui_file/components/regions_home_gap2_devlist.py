"""Panel/DevInspect `State=List`: the Dev mode left list under the Pages list (`panels/left.rs` :140-215, `dev/list.rs` `tabs` :53-120 and `show` :123-210; lines are the v0.7.5 tag).

The region is the 241 x 595 part of the left panel below the Pages list and its separator (a screen places it at (49, 305), over the Layers header and rows of Panel/LeftLayers, which it covers
with BG): the 52 px `Ready for development` section (a 13 px Dev glyph at 18, the label at 30 in `font_caption_strong`, the count badge `BADGE_SM` high, right edge `PANEL_PADDING_X` from the inner
edge, filled DEV_READY at 0.22 when something is ready; the 24 px `</> Mark selection ready for dev` button, DEV_READY at 0.16, label in DEV_READY), a
  BORDER hairline, the tab row replacing the `Layers`
title (the `Ready for dev` and `Layers` tabs, 24 high at `PANEL_PADDING_X`, the active one BG_SECONDARY, `font_body_strong`) and the list (`ROW_H` 32): a page row (name `font_body_strong` at 16,
count right-aligned, caption TEXT_TERTIARY), then one row per ready layer (name `font_body` at inner.left + 16, the status pill 6 px from inner.right,
  a selected row BG_SELECTED at RADIUS_CONTROL inset 8 x 1).
Fifteen item rows are drawn (the boxmodel capture's); a screen with fewer hides the rest with `visible` overrides on `Row <n>`, and shows `Selected row` when a layer is selected.
Gaps: the count in the tab, hover fills, the scrollbar thumb, the empty state (`Nothing is ready for dev yet`), and the Layers tab's own list.
Names are neutral (`Frame 1`, `Page 1`); the status text and pill are Studio's.
"""
from rest_ir import E, F, T, lit
from components.regions_home_dev_parts import PILL, TINT, button, glyph, text
from components.regions_home_parts import rule

W, H, ROWS, ITEM_TOP = 241, 595, 15, 126
MARK_TINT = "panels/left.rs:20 `DEV_TINT_REST`: DEV_READY at a raw alpha of 0.16, a paint opacity no variable holds"


def section(env):
    count = F("Ready count", [T("Count", "78", "Mono / Value", env.c("TEXT"))], dir="H", fill=env.c("DEV_READY"), alpha=lit(0.22, TINT), radius=lit(11, PILL), abs=(202, 3, 22, 22), w=22, h=22,
             sx="fixed", sy="fixed", align=("center", "center"))
    mark = F("left-dev-mark-ready", [T("Label", "</> Mark selection ready for dev", "Caption / Medium", env.c("DEV_READY"))], dir="H", fill=env.c("DEV_READY"), alpha=lit(0.16, MARK_TINT),
             radius=env.c("RADIUS_CONTROL"), abs=(10, 25, 220, 24), w=220, h=24, sx="fixed", sy="fixed", align=("center", "center"))
    head = F("left-dev-ready-section", [glyph(env, "Dev glyph", "Dev", 13, 18, 14, "DEV_READY"), text(env, "Ready label", "Ready for development", "Caption / Semibold", "TEXT", 30, 14, 150),
                                        count, mark], abs=(0, 0, 240, 52), w=240, h=52, sx="fixed", sy="fixed")
    return [head, rule("Rule dev section", 0, 53, 240)]


def tabs(env):
    active = F("dev-left-tab-ready", [T("Label", "Ready for…", "Body / Semibold", env.c("TEXT"), abs=(8, 3, 70, 18))], fill=env.c("BG_SECONDARY"), radius=env.c("RADIUS_CONTROL"),
               abs=(16, 62, 83, 24), w=83, h=24, sx="fixed", sy="fixed")
    other = F("dev-left-tab-layers", [T("Label", "Layers", "Body / Semibold", env.c("TEXT_SECONDARY"), abs=(8, 3, 40, 18))], radius=env.c("RADIUS_CONTROL"), abs=(103, 62, 55, 24), w=55, h=24,
              sx="fixed", sy="fixed")
    return [active, other]


def item(env, i):
    y = ITEM_TOP + 32 * i
    dot = E("Dot", 7, 7, fill=env.c("DEV_READY"), abs=(6.5, 6.5, 7, 7))
    pill = F("Status pill", [dot, text(env, "Label", "Ready for dev", "Caption / Medium", "TEXT", 19, 10, 80)], dir="H", fill=env.c("DEV_READY"), alpha=lit(0.22, TINT), radius=lit(10, PILL),
             abs=(120, 6, 106, 20), w=106, h=20, sx="fixed", sy="fixed")
    return F(f"Row {i + 1}", [text(env, "Name", f"Frame {i + 1}", "Body / Regular", "TEXT", 24, 16, 90), pill], abs=(0, y, 240, 32), w=240, h=32, sx="fixed", sy="fixed")


def list_body(env):
    page = F("Page row", [text(env, "Page name", "Page 1", "Body / Semibold", "TEXT", 16, 16, 140),
                          T("Page count", "78", "Caption / Regular", env.c("TEXT_TERTIARY"), align_h="right", wrap=True, w=40, abs=(184, 7, 40, 18))],
                abs=(0, 94, 240, 32), w=240, h=32, sx="fixed", sy="fixed")
    selected = F("Selected row", fill=env.c("BG_SELECTED"), radius=env.c("RADIUS_CONTROL"), abs=(8, ITEM_TOP + 1, 224, 30), w=224, h=30, sx="fixed", sy="fixed", hidden=True)
    kids = [*section(env), *tabs(env), page, selected, *[item(env, i) for i in range(ROWS)], rule("Right border", 240, 0, 1)]
    kids[-1]["abs"] = (240, 0, 1, H)
    root = F("List", kids, dir="V", fill=env.c("BG"), w=W, h=H, sx="fixed", sy="fixed", align=("start", "start"), clip=True)
    return root
