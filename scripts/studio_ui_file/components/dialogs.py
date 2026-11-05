"""Registry `composite` Dialog/Modal (page `03 Studio`, row Dialogs). Lines are the v0.7.5 tag.

Dialog/Modal (Kind=Libraries, Plugins, InstallPlugin, Compare x State=Default): kit `modal_surface` (widgets/kit/surface.rs:64-69, `modal_look` :36-39): the frame at `RADIUS_FRAME` in `BG` with no
border. The scrim (`BG_OVERLAY` over the whole screen) is painted by the same call and is a screen layer (W4): it is not in the set. The registry's two first kinds are these two modals; the other
`modal_surface` callers (home/dialog.rs:273, history/modal.rs:32, updater.rs:667; origin/main adds agent_approval_card.rs:316) are not registry kinds and are recorded for W4.
* Libraries: `libraries/modal.rs:46-61` `frame`, `show` :147-170 at 560 x 640 (`modal_rect`, :41-43). `header` (:64-73): the title in Large Semibold (`font_large_strong`) at `PAD`
  = `INSET_MODAL` in a `HEADER_H` 48 row (a local const, modal.rs:14), a `BORDER` hairline on the row's last pixel (Divider/Hairline, resized to 560 and absolute at y 47: its main
  is the same 1 px line) and the `widgets::icon_button` close at `PAD` from the right, `CONTROL_SM` square = Button/Close Sm at its own 36 (the glyph stays 16). Nests Button/Close
  and Divider/Hairline.
* Plugins: `plugins/ui.rs:17-227` `show` at 560 x 440. The title is `font_title` (Title Semibold) centred 36 down; the close is a text button, `kit::button(.., Secondary, Sm)` labelled "Close" at
  (right - `PAD` - its width, top + 18) = Button/Secondary Sm; the footer button is `kit::button(.., Primary, Md)` ("Install plugin") with `PAD` below and to the right of it = Button/Primary Md.
  No header hairline. The header is the 72 px above the body (`rect.top() + 72.0`, ui.rs:57) and the footer the 76 below it (`rect.bottom() - 76.0`). Nests Button/Secondary and Button/Primary.
  The install form's own second footer button (Cancel) and its field are the InstallPlugin kind's.
Local: the header row (title filling, the close in flow: the row centres a 36 px button at the same y the Rust gives it), the footer row and the Slot container `Body` (fills the rest, `INSET_MODAL`
of side padding, so a caller's content sits inside the inset; its default content is the kind's own body). Gaps: `libraries-close` and `plugin-close` are the two registry kinds' only chrome
buttons; the title is plain text, not a TEXT property, because the kinds differ and a property default would be wrong for one of them (an instance overrides the text). Production body content a
caller adds (library rows with kit asset names, plugin cards) is the caller's: no capture text enters the file (G4).
Kinds and bodies drawn from the v0.7.5 painters, each in its own module: the Libraries default Body is the publish list (dialogs_publish.py), the Plugins default Body is the empty state
`plugins/ui.rs` draws (dialogs_plugins.py: the Tools glyph and three left-aligned paragraphs, not the Home EmptyState), InstallPlugin is the Install plugin form (dialogs_install.py: the Plugins
header with the title `Install plugin`, the URL field with its focus ring, two notes, Cancel and a disabled Install) and Compare is the 818 x 638 compare dialog's chrome (dialogs_compare.py:
`menu_surface`, not `modal_surface`).
"""
from components.dialogs_compare import modal as compare_modal
from components.dialogs_install import body as install_body, footer_buttons
from components.dialogs_plugins import body as plugins_body
from components.dialogs_publish import body as publish_body
from rest_ir import F, T, X

W = 560.0  # libraries/modal.rs:152 and plugins/ui.rs:24: `560.0`
# libraries/modal.rs:152 `640.0` with HEADER_H 48; ui.rs:24 `440.0` with the 72 px above the body
KINDS = {"Libraries": {"h": 640.0, "header_h": 48.0}, "Plugins": {"h": 440.0, "header_h": 72.0}, "InstallPlugin": {"h": 440.0, "header_h": 72.0}}
# ui.rs:47 `if install_form { "Install plugin" } else { "Plugins" }`
TITLES = {"Libraries": "Libraries", "Plugins": "Plugins", "InstallPlugin": "Install plugin"}
FOOTER_H = 76.0  # plugins/ui.rs:57 `rect.bottom() - 76.0`


def body_slot(env, kind, side):
    """The Body slot and its default content per kind: the publish list, the plugins empty state, the install form.
    The slot pads its sides by `INSET_MODAL`, so a caller's content sits inside them."""
    kids = {"Libraries": publish_body, "Plugins": plugins_body, "InstallPlugin": install_body}[kind](env)
    return F("Body", kids, dir="V", slot="Body", sx="fill", sy="fill", pad=side, align=("start", "start"))


def modal(values, env):
    kind = values["Kind"]
    if kind == "Compare":
        return compare_modal(env)
    cfg, libraries = KINDS[kind], kind == "Libraries"
    side = [None, env.c("INSET_MODAL"), None, env.c("INSET_MODAL")]
    title = T("Title", TITLES[kind], "Large / Semibold" if libraries else "Title / Semibold", env.c("TEXT"), sx="fill")
    close = X("Close", "Button/Close", "Size=Sm, State=Default") if libraries else X("Close", "Button/Secondary", "Size=Sm, State=Default", props={"Label": "Close"})
    kids = [title, close] + ([X("Hairline", "Divider/Hairline", "State=Default", abs=(0.0, cfg["header_h"] - 1, W, env.num("BORDER_WIDTH")))] if libraries else [])
    header = F("Header", kids, dir="H", pad=side, h=cfg["header_h"], sx="fill", sy="fixed", align=("start", "center"))
    parts = [header, body_slot(env, kind, side)]
    if not libraries:
        buttons = footer_buttons() if kind == "InstallPlugin" else [X("Primary action", "Button/Primary", "Size=Md, State=Default", props={"Label": "Install plugin"})]
        parts.append(F("Footer", buttons, dir="H", gap=env.c("SPACE_INLINE"), pad=[None, env.c("INSET_MODAL"), env.c("INSET_MODAL"), None], h=FOOTER_H, sx="fill", sy="fixed", align=("end", "end")))
    return F("Modal", parts, dir="V", fill=env.c("BG"), radius=env.c("RADIUS_FRAME"), w=W, h=cfg["h"], sx="fixed", sy="fixed", align=("start", "start"))


def component_sets():
    return [{"name": "Dialog/Modal", "row": "Dialogs", "axes": {"Kind": ["Libraries", "Plugins", "InstallPlugin", "Compare"], "State": ["Default"]},
             "uses": ["Button/Close", "Button/Secondary", "Button/Primary", "Divider/Hairline", "Badge/StatusCount", "Input/Field", "Input/Search", "Focus/Ring"],
             "props": [{"name": "Body", "type": "SLOT"}], "build": modal}]
