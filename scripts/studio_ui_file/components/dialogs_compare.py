"""Dialog/Modal Kind=Compare: the chrome of the Compare changes dialog (`dev/compare.rs` at v0.7.5, the capture `g-dev-compare`). Not a component module (no `component_sets`).

Painter `show` (compare.rs:83-195), one caller (`open`, :34-45). The card is `widgets::menu_panel` = `kit::menu_surface` (kit/surface.rs:56-58 and `menu_panel_look` :23-25: `BG`
with a `BORDER_MENU` edge, the menu radius), not `modal_surface`: 818 x 638 in the capture (the Rust rect is `W` 820 x `h` 640 and its 1 px edge is not drawn inside it); every
child is placed at the Rust's own offsets from the 820 x 640 rect's top-left. The dim over the window is the black alpha 140 of :97, a screen layer (the Scrim rectangle of the
screen).
* Header: the title (`font_large_strong`, TEXT, :103-109) at `pad` 20 :102, 28 down, the 28 px close icon button at (right - 44, 14) (:110-112: Button/Close Xs resized to 28).
* Panes (`pane` :53-70, :115-120): two 382 x 324 boxes at y 56, x 20 and 418 (`pw` = (820 - 40 - 16) / 2 :116, `ph` = `PREVIEW` 300 :21 + 24 :117); the caption (Body Semibold TEXT)
  10 down, then the 300 high area (`BG_SECONDARY`, `RADIUS_CONTROL`) holding the rendered layer: document content, a gap here (the area is drawn empty; its pixels are masked by the
  screen gate). "No preview" :65.
* Summary (:122-132): `N changed, N added, N removed` (Body Semibold TEXT) 8 below `summary_y` = 56 + 324 + 14.
* Change list (:135-172): a clipped 780 x 160 rectangle under the summary (`summary_y + 22`), rows of 26 (a bullet in `DEV_CHANGED` :148, the name in Body Semibold TEXT at 22 :150,
  the word `changed` in Caption TEXT_SECONDARY 12 from the right :156) and, for a modified layer, a 20 px row per property (the name in Caption TEXT_SECONDARY at 22 :160, the old
  value in `DANGER` at 170 :163, an arrow in TEXT_TERTIARY at 330 :164 and the new value in `DEV_READY` at 350 :165) with 6 below the group; `scrollbar` (panels/mod.rs:49-73) is
  the 4 px thumb 6 from the list's right edge :172.
* Footer (`btn_h` 32, :174-182): `small_button` (panels/inspect.rs:217-223: `BG_SECONDARY` at `RADIUS_CONTROL`, `Body` Regular `TEXT`, 80 wide) `Close` at (right - 20 - 190 - 88,
  bottom - 52) :176-177, and the pill (`cr_pill` of its height, `BG_BRAND`, Body Semibold `TEXT`) `Mark as ready for dev again` 190 wide at (right - 20 - 190, bottom - 52) :175,
  :181-182.
Content: the capture's layer names are a kit's, so `Frame 1`... stand in (G4); the property names and values are document data (`Width`, `Fill`).
Gaps: the hover fills (`BG_TERTIARY`, `BG_BRAND_HOVER` :181), a list that scrolls, an `added` or `removed` row (`+` in DEV_READY, `-` in DANGER :144-145).
"""
from components.dialogs_text import text_at
from components.regions_sidepanels_parts import scrollbar_thumb
from rest_ir import F, T, X, lit

W, H = 818.0, 638.0
PAD = 20.0
LIST = (20.0, 416.0, 780.0, 160.0)  # `summary_y + 22` to `bottom - pad - btn_h - 12`
PILL = "dev/compare.rs:181 `theme::cr_pill(ok.height())` = 32 / 2: computed from the button's height, no radius role holds it"
CHANGES = [("Frame 1", [("Width", "1440", "1480"), ("Fill", "#FFFFFF", "#FFF1D6")]), ("Frame 2", [("Width", "68", "8")]), ("Frame 3", [("Width", "972", "1072")])]


def pane(env, name, x, caption):
    return [text_at(f"{name} title", (x, 56.0, 382.0, 20.0), caption, "Body / Semibold", "TEXT", env),
            F(f"{name} area", fill=env.c("BG_SECONDARY"), radius=env.c("RADIUS_CONTROL"), w=382.0, h=300.0, sx="fixed", sy="fixed", abs=(x, 80.0, 382.0, 300.0))]


def row(env, n, name, props, y):
    """One change (26 px) with its property rows, at y inside the list."""
    kids = [text_at("Mark", (6.0, y, 12.0, 26.0), "•", "Body / Semibold", "DEV_CHANGED", env), text_at("Name", (22.0, y, 600.0, 26.0), name, "Body / Semibold", "TEXT", env),
            text_at("State", (580.0, y, 188.0, 26.0), "changed", "Caption / Regular", "TEXT_SECONDARY", env, align=("end", "center"))]
    for i, (prop, before, after) in enumerate(props):
        py = y + 26.0 + 20.0 * i
        kids += [text_at(f"Property {i + 1}", (22.0, py, 140.0, 20.0), prop, "Caption / Regular", "TEXT_SECONDARY", env),
                 text_at(f"Before {i + 1}", (170.0, py, 150.0, 20.0), before, "Caption / Regular", "DANGER", env),
                 text_at(f"Arrow {i + 1}", (330.0, py, 16.0, 20.0), "→", "Caption / Regular", "TEXT_TERTIARY", env),
                 text_at(f"After {i + 1}", (350.0, py, 150.0, 20.0), after, "Caption / Regular", "DEV_READY", env)]
    return kids


def change_list(env):
    kids, y = [], 0.0
    for n, (name, props) in enumerate(CHANGES, 1):
        kids += row(env, n, name, props, y)
        y += 26.0 + 20.0 * len(props) + (6.0 if props else 0.0)
    return F("Change list", kids, clip=True, w=LIST[2], h=LIST[3], sx="fixed", sy="fixed", abs=LIST)


def modal(env):
    close = F("Close", [T("Text", "Close", "Body / Regular", env.c("TEXT"))], dir="H", fill=env.c("BG_SECONDARY"), radius=env.c("RADIUS_CONTROL"), w=80.0, h=32.0, sx="fixed", sy="fixed",
              align=("center", "center"), abs=(522.0, 588.0, 80.0, 32.0))
    mark = F("Mark as ready", [T("Text", "Mark as ready for dev again", "Body / Semibold", env.c("TEXT"))], dir="H", fill=env.c("BG_BRAND"), radius=lit(16, PILL), w=190.0, h=32.0,
             sx="fixed", sy="fixed", align=("center", "center"), abs=(610.0, 588.0, 190.0, 32.0))
    kids = [text_at("Title", (PAD, 18.0, 700.0, 20.0), "Compare changes • Frame 1", "Large / Semibold", "TEXT", env),
            X("Close button", "Button/Close", "Size=Xs, State=Default", abs=(776.0, 14.0, 28.0, 28.0), w=28.0, h=28.0),
            *pane(env, "Before", 20.0, "When marked Ready for dev"), *pane(env, "After", 418.0, "Now"),
            text_at("Summary", (PAD, 393.0, 400.0, 18.0), "21 changed, 0 added, 0 removed", "Body / Semibold", "TEXT", env), change_list(env),
            scrollbar_thumb("Scrollbar", LIST, 2.0, 20.0, env), close, mark]
    return F("Modal", kids, fill=env.c("BG"), stroke=env.c("BORDER_MENU"), sw=env.c("BORDER_WIDTH"), radius=env.c("RADIUS_FRAME"), w=W, h=H, sx="fixed", sy="fixed")
