"""The default Body of Dialog/Modal Kind=Libraries: the publish list (`libraries/modal.rs` at v0.7.5, the capture `k-libraries`). Not a component module (no `component_sets`):
`discover` skips it.

Every rect is the modal's own coordinates (the modal at (0, 0), 560 wide); the Body slot starts under the 48 px header, so `_at` moves a modal y into the slot. Painter lines are
v0.7.5:
* `show` :147-171 (`HEADER_H` :14 = 48 plus 16 of space, `this_file`, a hairline :159, then `available`), `this_file` :173-279, `available` :300-400, `label` :143-145,
  `paint_change_chip` :134-141.
* `this_file` rows: the "This file" label (`label`, Body Semibold TEXT :176) and the status (Body TEXT_TERTIARY, right edge `x1` :182) are 10 below `y`; the file name (Body
  TEXT_SECONDARY) 28 below :184; the summary 24 further :187 (Body TEXT_SECONDARY); then `SPACE_BLOCK` :202. A change row is `CHANGE_PITCH` = `BADGE_MD` 24 high (:17, :203-226):
  the kind icon (14 px, ICON_SECONDARY) centred 16 in :217, the name (Body TEXT) at 30, the status chip (`status_tag`: Badge/StatusCount Md) ending 8 before `x1` :220, and the kind
  caption (Caption TEXT_TERTIARY) 8 before the chip :221. The caption "and N more" sits at x0 + 8 (:204-211, `MORE_ROW_H` 20 :20). The description field :227-243 is `field_chrome`
  at `CONTROL_SM` (Input/Field Sm, the hint in TEXT_PLACEHOLDER 10 in) and the Publish button :248-253 `CONTROL_SM` high at the right edge (Button/Primary Sm; its width is
  `max(label + 2 x INSET_BUTTON, 80)` :251).
* `available`: the "Libraries" label and the 220 x 36 search box (`search_box` :98-115, Input/Search Sm) :304-307, then the empty state: two centred lines 40 and 60 below the
  list's top (Body, then Caption, both TEXT_TERTIARY, :337-342).
Content: the capture's file name and asset names are a kit's, so neutral names of the same kind stand in (G4); the numbers are the shape of the capture's counts (four rows and "and
N more").
Gaps: the toggle rows of an available library (`available` rows, `ROW_H` 60 :16), "Unpublish", the confirm line and the scroll thumb: no capture shows them.
"""
from components.dialogs_text import text_at
from rest_ir import F, I, T, X, lit

X0, X1 = 24.0, 536.0  # PAD = INSET_MODAL and the modal's right edge less PAD (modal.rs:15, :175)
TOP = 48.0  # HEADER_H: the Body slot starts here
ROW_X = "libraries/modal.rs:217-221: the kind icon is centred at x0 + 16 and the name starts at x0 + 30, raw literals"
CHIP_END = "libraries/modal.rs:220: the chip ends 8 before x1, a raw literal (SPACE_INLINE holds 8 for other roles)"
CHANGES = [("Component 1", "Component"), ("Component 2", "Component"), ("Component 3", "Component"), ("Component set 1", "Component set")]
MORE = "and 8 more"


def _at(y):
    return y - TOP


def text(env, name, rect, content, style, colour, align=("start", "center")):
    x, y, w, h = rect
    return text_at(name, (x, _at(y), w, h), content, style, colour, env, align=align)


def change_row(env, n, name, kind, cy):
    """A change row (modal.rs:203-226): icon, name, caption and the New chip, 24 high, centred on `cy`."""
    icon = I("Icon", "Component", 14, env.c("ICON_SECONDARY"), abs=(9.0, 5.0, 14.0, 14.0))
    title = T("Name", name, "Body / Regular", env.c("TEXT"), sx="fill")
    caption = T("Kind", kind, "Caption / Regular", env.c("TEXT_TERTIARY"))
    chip = X("Status", "Badge/StatusCount", "Size=Md, Tone=Success, State=Default", props={"Label": "New"})
    pad = [None, lit(8, CHIP_END), None, lit(30, ROW_X)]
    return F(f"Change {n}", [icon, title, caption, chip], dir="H", gap=env.c("SPACE_INLINE"), pad=pad, w=X1 - X0, h=env.num("BADGE_MD"), sx="fixed", sy="fixed", align=("start", "center"),
             abs=(X0, _at(cy - 12.0), X1 - X0, env.num("BADGE_MD")))


def body(env):
    """The Body slot's default children: the publish list over the empty library list, placed at the capture's rects."""
    kids = [text(env, "This file", (X0, 65.0, 120.0, 18.0), "This file", "Body / Semibold", "TEXT"),
            text(env, "Status", (X1 - 160.0, 65.0, 160.0, 18.0), "Not published", "Body / Regular", "TEXT_TERTIARY", align=("end", "center")),
            text(env, "File name", (X0, 91.0, X1 - X0, 18.0), "Design file", "Body / Regular", "TEXT_SECONDARY"),
            text(env, "Summary", (X0, 115.0, X1 - X0, 18.0), "12 assets to publish", "Body / Regular", "TEXT_SECONDARY")]
    kids += [change_row(env, i + 1, name, kind, 144.0 + 24.0 * i) for i, (name, kind) in enumerate(CHANGES)]
    kids.append(text(env, "More", (X0 + 8.0, 228.0, 160.0, 20.0), MORE, "Caption / Regular", "TEXT_TERTIARY"))
    field = X("Description", "Input/Field", "Size=Sm, State=Default", props={"Placeholder": "Describe what changed (optional)", "Has Placeholder": True, "Has Value": False},
              abs=(X0, _at(254.0), X1 - X0, env.num("CONTROL_SM")), w=X1 - X0)
    publish = X("Publish", "Button/Primary", "Size=Sm, State=Default", props={"Label": "Publish"}, abs=(X1 - 80.0, _at(306.0), 80.0, env.num("CONTROL_SM")), w=80.0)
    rule = X("Rule", "Divider/Hairline", "State=Default", abs=(0.0, _at(352.0), 560.0, 1.0))
    search = X("Search", "Input/Search", "Size=Sm, State=Default", props={"Value": "Search libraries"}, abs=(X1 - 220.0, _at(367.0), 220.0, env.num("CONTROL_SM")), w=220.0)
    kids += [field, publish, rule, text(env, "Libraries", (X0, 376.0, 120.0, 18.0), "Libraries", "Body / Semibold", "TEXT"), search,
             text(env, "Empty title", (0.0, 430.0, 560.0, 18.0), "No libraries yet", "Body / Regular", "TEXT_TERTIARY", align=("center", "center")),
             text(env, "Empty caption", (0.0, 450.0, 560.0, 18.0), "Publish a file to share its components, styles and variables.", "Caption / Regular", "TEXT_TERTIARY", align=("center", "center"))]
    return kids
