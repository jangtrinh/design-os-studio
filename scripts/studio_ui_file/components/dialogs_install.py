"""The Install plugin form of Dialog/Modal Kind=InstallPlugin: its default Body and its footer (`plugins/ui.rs` at v0.7.5, `install_form` true; the capture `plugin-install`). Not a
component module.

Painter `show` (plugins/ui.rs:17-227), the same 560 x 440 modal as Kind=Plugins (the header is the Plugins one with the title `Install plugin`, :47):
* The body ScrollArea (:57-62) starts 72 down, `PAD` in, item spacing 12 (:62): `paragraph` "Paste a manifest URL from jangtrinh/design-os-plugins." (TEXT_TERTIARY :64), 4 more
  :65, the label `Plugin URL` (TEXT :66), the field (`CONTROL_SM` high, `field_chrome` :68), `INSET_SEGMENT` of space :90, then two TEXT_TERTIARY paragraphs (:97, :102). The
  capture's rects: field (464, 360, 512, 36), paragraph lines 15 px apart (the painter's `font_body` rows).
* The field is `variables::widgets::field_chrome` (variables/widgets.rs:192-197): `paint_field` (Input/Field Sm State=Focus: 2 px `BORDER_SELECTED`) and, when the field has
  keyboard focus, `focus_ring` (kit/focus.rs) around it, which the capture shows (`fresh` requests focus, :91-93). The field is a local host frame holding the Input/Field instance
  and the ring (the ring's parent is its host). The hint is the Rust string (:79-80), drawn the way the capture shows it: elided to the field's width with an ellipsis (a hug text
  cannot clip).
* The footer (:172-222): `Cancel` (Button/Secondary Md, 72 wide: `control_width.max(72)` :197) and `Install` (Button/Primary Md, 96 wide: `.max(96)` :180) at `SPACE_INLINE` from
  each other, disabled while the URL is empty (:183 `enabled`): Button/Primary State=Disabled.
Gaps: the pending line with its spinner (:105-111), the error line (:168-170) and the typed-URL state (the Value text).
"""
from rest_ir import F, T, X

BODY_W, X0, TOP = 512.0, 24.0, 72.0  # the field and paragraphs are 512 wide (560 less 2 x PAD); the Body slot starts at the modal's y 72, its abs children use the modal's x
# plugins/ui.rs:80, cut where it fits the field: Input/Field Sm draws its hint in `Text sm` (14) and the painter in `font_body` (12), so the string ends before the capture's `man...`
URL_HINT = "https://github.com/jangtrinh/design-os-plugins/blob/main/plugins/ico…"
PAGE_NOTE = "Paste a manifest URL from jangtrinh/design-os-plugins."
TOKEN_NOTE = "For private plugin repositories, set DESIGNOS_PLUGINS_TOKEN (or DESIGNOS_PLUGINS_TOKEN) to a GitHub token with read access."
DOWNLOAD_NOTE = "Install downloads the bundle. Open launches its browser panel with the permissions listed in your library."


def paragraph(env, name, y, content, colour):
    """A wrapping `paragraph` (ui.rs:229-231) whose first line's box starts at modal y: Body Regular, 512 wide, two lines."""
    return T(name, content, "Body / Regular", env.c(colour), wrap=True, w=BODY_W, abs=(X0, y - TOP, BODY_W, 36.0))


def field(env):
    """The URL field: Input/Field Sm Focus filling a 512 x 36 host, the keyboard ring around the host (Focus/Ring Brand)."""
    h = env.num("CONTROL_SM")
    inner = X("Field", "Input/Field", "Size=Sm, State=Focus", props={"Placeholder": URL_HINT, "Has Placeholder": True, "Has Value": False}, sx="fill", sy="fill")
    ring = X("Focus ring", "Focus/Ring", "Tone=Brand", abs=(-4.0, -4.0, BODY_W + 8.0, h + 8.0))
    return F("URL field", [inner, ring], dir="H", radius=env.c("RADIUS_CONTROL"), w=BODY_W, h=h, sx="fixed", sy="fixed", abs=(X0, 130.0 - TOP, BODY_W, h))


def body(env):
    """The Body slot's default children (the modal's rects, the slot's top at y 72): note, label, field and the two notes."""
    label = F("Label", [T("Text", "Plugin URL", "Body / Regular", env.c("TEXT"))], dir="H", w=BODY_W, h=18.0, sx="fixed", sy="fixed", align=("start", "center"), abs=(X0, 102.0 - TOP, BODY_W, 18.0))
    return [paragraph(env, "Note", 71.0, PAGE_NOTE, "TEXT_TERTIARY"), label, field(env), paragraph(env, "Token note", 190.0, TOKEN_NOTE, "TEXT_TERTIARY"),
            paragraph(env, "Download note", 232.0, DOWNLOAD_NOTE, "TEXT_TERTIARY")]


def footer_buttons():
    """Cancel and the disabled Install: the footer row lays them out right-aligned with `SPACE_INLINE` between."""
    return [X("Cancel", "Button/Secondary", "Size=Md, State=Default", props={"Label": "Cancel"}, w=72.0),
            X("Primary action", "Button/Primary", "Size=Md, State=Disabled", props={"Label": "Install"}, w=96.0)]
