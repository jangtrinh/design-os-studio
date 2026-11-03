"""Builders of the Agents panel (`regions_sidepanels_agents.py`, `regions_sidepanels_composer.py`). Not a component module (no `component_sets`): `discover` skips it. Lines are the v0.7.5 tag.

Inline `Color32` literals and the terminal's own constants (`terminal.rs` `TERM_*`, `STATUS_*`, the Skills chip's `0x28282C` / `0x3C3C42` / `0xB3B3B3`, skills/mod.rs:772) have no variable:
G1 maps theme.rs
constants only. As in the colour picker, each is drawn as the nearest theme tone by value (the smallest per-channel difference against the opaque colour constants of theme.rs, a tie going to
the tone whose role fits),
a deviation of the drawing from the painter, never a new literal. The mapping, with the distance:
  | panel fill TERM_BG_VIEWPORT 0E0E12 (the viewport) | BG (10) | 8 |   header bar TERM_BG_BAR 16171B | BG_SECONDARY (23) | 4 |   bar rule and add-button border TERM_BORDER 2A2B31 | BORDER
  (38) | 11 |
  | add button fill TERM_BG_ELEVATED 1F2024 | BG_SELECTED (38) | 7 |   Skills chip fill 28282C | BG_SELECTED (38) | 6 |   Skills chip border 3C3C42 | BORDER_STRONG | 4 |   Skills chip text
  B3B3B3 | TEXT_TERTIARY (163) | 16 |
  | status pill text STATUS_OK_TEXT 4CC58B | TEXT_SUCCESS | 25 |   its fill 17302A | BG_SUCCESS_PRIMARY | 20 |   its border 2C5F4A | BADGE_SUCCESS_BORDER | 22 |
The panel fill is the one with the largest gap (8): the region sits on BG where the capture is 0E0E12."""
from rest_ir import F, I, T, lit

PILL = "agent_picker.rs:415 `cr_pill(pill_r.height())` = 18 / 2: computed from the pill's height, no radius role holds it"


def text(name, rect, content, style, colour, env, **kw):
    return T(name, content, style, env.c(colour), abs=rect, **kw)


def box(name, rect, env, kids=(), fill=None, stroke=None, radius="RADIUS_CONTROL", **kw):
    """A fixed frame at `rect` (a painter's `rect_filled` + `rect_stroke` over it) with its children placed absolutely."""
    paint = {"fill": env.c(fill)} if fill else {}
    if stroke:
        paint.update(stroke=env.c(stroke), sw=env.c("BORDER_WIDTH"))
    if radius:
        paint["radius"] = radius if isinstance(radius, tuple) else env.c(radius)
    return F(name, list(kids), w=rect[2], h=rect[3], sx="fixed", sy="fixed", abs=rect, **paint, **kw)


def centred(env, content, style, colour, w, h, name="Label"):
    """A text centred in a fixed box at the origin of its parent (`p.text(rect.center(), Align2::CENTER_CENTER, ..)`)."""
    return F(name, [T("Text", content, style, env.c(colour))], dir="H", w=w, h=h, sx="fixed", sy="fixed", abs=(0.0, 0.0, w, h))


def line(env, name, rect, colour="BORDER"):
    return F(name, fill=env.c(colour), w=rect[2], h=rect[3], sx="fixed", sy="fixed", abs=rect)


def skills_chip(env, name, rect):
    """`SkillsState::paint_chip` (skills/mod.rs:779-805): a `RADIUS_CONTROL` chip with a 1 px stroke and the `Skills` label (`font_caption_strong`) centred."""
    label = F("Label", [T("Text", "Skills", "Caption / Semibold", env.c("TEXT_TERTIARY"))], dir="H", w=rect[2], h=rect[3], sx="fixed", sy="fixed", abs=(0.0, 0.0, rect[2], rect[3]))
    return box(name, rect, env, [label], fill="BG_SELECTED", stroke="BORDER_STRONG")


def state_pill(env, label, width, x, ok, dim, name="State", **kw):
    """The CLI state pill (agent_picker.rs:408-420): 18 high, `cr_pill`, `text_width + SPACE_GROUP` wide, the label in Caption Regular centred in it; signed in is the success tone, the rest the
    neutral one. One frame with its text: a hidden alternate part costs its nodes in every instance of the panel, so the pill is two nodes, not three."""
    fill, edge, ink = ("BG_SUCCESS_PRIMARY", "BADGE_SUCCESS_BORDER", "TEXT_SUCCESS") if ok else ("BG_TERTIARY", "BORDER", "TEXT_DISABLED" if dim else "TEXT_SECONDARY")
    return F(name, [T("Text", label, "Caption / Regular", env.c(ink))], dir="H", fill=env.c(fill), stroke=env.c(edge), sw=env.c("BORDER_WIDTH"), radius=lit(9, PILL), w=width, h=18.0,
             sx="fixed", sy="fixed", align=("center", "center"), abs=(x, 11.0, width, 18.0), **kw)


PROBES = {"signed": ("Signed in", 64.78, True, False, "Pill signed in", "Show signed in"), "unknown": ("Sign-in unknown", 107.73, False, False, "Pill unknown", "Show unknown"),
          "checking": ("Checking…", 76.05, False, True, "Pill checking", "Show checking")}  # agent_picker.rs:408-420: the three probe results a row's pill draws


def picker_row(env, name, y, label, label_w, version, probe=None, caption=None, n=None):
    """One row of the new-session picker (agent_picker.rs:346-441): 336 x `CONTROL_MD` 40, `BG_SECONDARY` with a `BORDER` stroke at `RADIUS_CONTROL`, the name in Body Medium `INSET_CONTROL` in
    (centred, or 12 down above a caption), the version in `Mono / Value` after it, the pill 8 from the right and the caption in Caption `TEXT_TERTIARY` 8 above the bottom.
    `probe` is the probe result the default draws (`signed`, `unknown`, `checking`) or None for a row with no pill. A probed row (`n` its number 1..5) holds every result as a `Show <part>` part: the
    name and version (`Show probed n`), the dim name with the Checking pill (`Show checking n`), the Signed in pill and the Sign-in unknown pill, so a screen picks the capture's probe results."""
    top = 3.0 if caption else 11.0
    checking = probe == "checking"
    probed = {"visible_prop": f"Show probed {n}", "hidden": checking} if n else {}
    kids = [text("Name", (8.0, top, label_w + 2, 18.0), label, "Body / Medium", "TEXT", env, **probed)]
    if version:
        kids.append(text("Version", (round(8.0 + label_w + 8.0, 2), top, 60.0, 18.0), version, "Mono / Value", "TEXT_SECONDARY", env, **probed))
    if n:
        kids.append(text("Name dim", (8.0, top, label_w + 2, 18.0), label, "Body / Medium", "TEXT_DISABLED", env, visible_prop=f"Show checking {n}", hidden=not checking))
        for kind, (pill, width, ok, dim, part, prop) in PROBES.items():
            kids.append(state_pill(env, pill, width, round(328.0 - width, 2), ok, dim, name=part, visible_prop=f"{prop} {n}", hidden=probe != kind))
    if caption:
        kids.append(text("Caption", (8.0, 23.0, 320.0, 18.0), caption, "Caption / Regular", "TEXT_TERTIARY", env))
    return box(name, (16.0, y, 336.0, 40.0), env, kids, fill="BG_SECONDARY", stroke="BORDER")


def prompt_row(env, n, title, body):
    """A sample prompt (agent_sample_prompts.rs:332-352): 368 x `CONTROL_MD` 40, the label in Body Medium `TEXT` 12 down and the text in Caption `TEXT_SECONDARY` 28 down, both `INSET_BUTTON` in."""
    kids = [text("Title", (12.0, 3.0, 340.0, 18.0), title, "Body / Medium", "TEXT", env), text("Text", (12.0, 19.0, 340.0, 18.0), body, "Caption / Regular", "TEXT_SECONDARY", env)]
    return box(f"Prompt {n}", (16.0, 510.0 + 44.0 * (n - 1), 368.0, 40.0), env, kids, fill="BG_SECONDARY", stroke="BORDER")


def glyph_box(env, name, rect, icon, size, colour):
    return F(name, [I("Icon", icon, size, env.c(colour))], dir="H", w=rect[2], h=rect[3], sx="fixed", sy="fixed", abs=rect)
