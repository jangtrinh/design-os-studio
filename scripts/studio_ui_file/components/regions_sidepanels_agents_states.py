"""The session states of Panel/AgentTerminal (`State=Chat, Failure, Approval`): the panel with a session tab open, drawn from the v0.7.5 painters. Not a component module (no
`component_sets`).

`State=Picker` is the new-session picker (`regions_sidepanels_agents.py`, the capture `i-agents`). The three states below keep that panel's strip, composer and surface and change
what a session tab shows; every rect is relative to the panel's top-left (49, 32 in the captures) and read from the captures' `layout.json` (`agents-sessions-btn`,
`agent-approval-card`, `agents-composer-box`, ...):
* The header (`terminal.rs:5986-6054`, the tab loop of `show_left_with_doc` :5932, 36 high): a session tab (`Caption` Medium title :6037-6042, the status glyphs, a 9 px close, a 2
  px `BORDER_SELECTED` underline), then the `Sessions`, `+` and `Skills` controls 8 apart, so the controls follow the tab's width (126 for `Claude Code #1`, 114.5 for `Antigravity
  #1`, 101.25 for `Grok CLI #1`). The tab and the three controls are one auto-layout row: a screen's tab label moves them.
* The strip (24 high, `terminal.rs:6119-6188`): the selection chip shrinks to 183.75 beside the `Needs approval` status (a 6 px dot :6150 and `Caption` text, `STATUS_WARN_TEXT` =
  BORDER_SELECTED by value: 27, the nearest tone) that a pending approval takes over the right slot with (:6141-6143, `agents-needs-approval-pill` :6188, (203, 36, 103.25, 24)).
* The body is the PTY viewport (`TERM_BG_VIEWPORT` 0E0E12 (14, 14, 18) from the strip to the panel's bottom, drawn in HUD_POINTER_FILL, the nearest tone by value at distance 4; the
  Picker state keeps BG, 8 away; the strip above it is BG_PRIMARY): a block cursor (`Chat`: (12, 68, 8, 15) in the capture) or, for `Approval`, the card of Card/Approval pinned
  `SPACE_ROW` under the strip (`agent_approval_card.rs:51-78`) and the shell prompt `sh-3.2$` and cursor 12 below it. `Failure` is the start-failure card
  (`agent_start_failure.rs:233-308` `paint`: `BG_SECONDARY` :266, `RADIUS_FRAME`, `INSET_PANEL` :233 of padding, an 8 px `DANGER` dot (radius `SPACE_ROW`) :271 and the title in
  `Large` Semibold :247, the headline in `DANGER` :248, the explanation in `TEXT_SECONDARY`, `Last output` :250 over a `BG` mono block :286 and the fix button :299-307 in
  `BG_SELECTED` with `BORDER_SELECTED` and `TEXT_BRAND`), 377 wide 16 under the strip (:243, :264). Not the mid-session banner `agent_session_failure.rs:871-989 paint_banner` the
  plan names: that one is a 48 px bar over a live session; the capture `i-agents-start-failure` is the start-failure card.
* The composer (Input/Composer at (12, 754, 376, 80); (12, 776) with the failure card, `agent_composer.rs:847` `show_composer`, the notice line :794): the hint and the notice under
  it are the screen's text.
Gaps: the tab's status glyphs are one generic pair (a ring and a brand dot): the Rust picks them per CLI and status; the PTY's own text beyond the prompt.
"""
from components.regions_sidepanels_agents_parts import box, centred, glyph_box, line, skills_chip, text
from components.dialogs_text import text_at
from rest_ir import E, F, I, T, X, lit

W = 401.0
TAB_GAP = "the capture's tab is 126 wide (x 8-134): the glyph pair starts 3 in and the title and the close are 3 apart; terminal.rs lays the tab out from raw offsets"
TAB_NEXT = "terminal.rs: the control after the last tab starts 10 from its edge (the capture's `agents-sessions-btn` at x 144 for a tab ending at 134), 8 of the row gap and 2 more"
TAB_STATUS = {"Chat": "BORDER_SELECTED", "Approval": "BORDER_SELECTED", "Failure": "INFO"}


def flow(node):
    """A header control built for an absolute rect, placed in the row's flow instead (fixed size, no `abs`)."""
    node.pop("abs", None)
    return node


def tab(env, state):
    """The session tab: status glyphs, title, close, with the active underline below; hugs its title."""
    ring = E("Ring", 8.0, 8.0, stroke=env.c("TEXT_SECONDARY"), sw=env.c("BORDER_WIDTH"), sx="fixed", sy="fixed")
    dot = E("Dot", 6.0, 6.0, fill=env.c(TAB_STATUS[state]), sx="fixed", sy="fixed")
    glyphs = F("Status", [ring, dot], dir="H", sx="hug", sy="hug", align=("center", "center"))
    title = T("Title", {"Chat": "Claude Code #1", "Approval": "Claude Code #1", "Failure": "Antigravity #1"}[state], "Caption / Medium", env.c("TEXT"))
    close = I("Close", "Close", 9, env.c("ICON_TERTIARY"))
    row = F("Row", [glyphs, title, close], dir="H", gap=lit(3, TAB_GAP), pad=[None, env.c("SPACE_FACT"), None, lit(3, TAB_GAP)], h=22.0, sx="hug", sy="fixed", align=("start", "center"))
    underline = F("Underline", fill=env.c("BORDER_SELECTED"), h=2.0, sx="fill", sy="fixed")
    pill = F("Tab", [row, underline], dir="V", fill=env.c("BG_TERTIARY"), radius=env.c("RADIUS_CONTROL"), sx="hug", sy="hug", align=("start", "start"))
    return F("Tab slot", [pill], dir="H", pad=[None, lit(2, TAB_NEXT), None, None], sx="hug", sy="hug")


def header(env, state):
    sessions = flow(box("Sessions", (0, 0, 76.0, 24.0), env, [text("Label", (2.0, 3.0, 60.0, 18.0), "Sessions", "Caption / Medium", "TEXT_SECONDARY", env),
                                                               glyph_box(env, "Chevron", (64.0, 8.0, 8.0, 8.0), "ChevronDown", 8, "TEXT_SECONDARY")], radius=None))
    plus, chevron = glyph_box(env, "Plus", (4.5, 6.5, 11.0, 11.0), "Plus", 11, "ICON"), glyph_box(env, "Chevron", (17.0, 8.0, 8.0, 8.0), "ChevronDown", 8, "ICON")
    add = flow(box("Add session", (0, 0, 28.0, 24.0), env, [plus, chevron], fill="BG_SELECTED", stroke="BORDER"))
    skills = flow(skills_chip(env, "Skills", (0, 0, 46.75, 28.0)))
    bar = F("Bar", [tab(env, state), sessions, add, skills], dir="H", gap=env.c("SPACE_INLINE"), fill=env.c("BG_SECONDARY"), pad=[None, None, None, env.c("INSET_FRAME")],
            w=W, h=36.0, sx="fixed", sy="fixed", align=("start", "center"), abs=(0.0, 0.0, W, 36.0))
    return [bar, line(env, "Bar rule", (0.0, 35.0, W, 1.0))]


def strip(env, approval):
    chip = X("Selection chip", "Tag/Chip", "Size=Sm, State=Default", props={"Label": "Page 1 · 1 selected · …" if approval else "Page 1 · 1 selected · Frame 1"},
             abs=(8.0, 36.0, 183.75 if approval else 240.0, 24.0), w=183.75 if approval else 240.0)
    mention = box("Mention", (314.25, 38.0, 77.75, 20.0), env, [centred(env, "@ Mention", "Caption / Medium", "TEXT", 77.75, 20.0)], fill="BG_SECONDARY", stroke="BORDER")
    kids = [chip, mention, line(env, "Strip rule", (0.0, 59.0, W, 1.0))]
    if approval:
        kids += [E("Status dot", 6.0, 6.0, fill=env.c("BORDER_SELECTED"), sx="fixed", sy="fixed", abs=(203.0, 45.0, 6.0, 6.0)),
                 text_at("Needs approval", (217.0, 36.0, 90.0, 24.0), "Needs approval", "Caption / Regular", "BORDER_SELECTED", env)]
    return kids


def failure_card(env):
    """The start-failure card (agent_start_failure.rs:239-308) at the capture's rects: card (12, 76, 376, 217), dot and title, headline, explanation, `Last output` and the mono block,
    the fix button."""
    pad = env.num("INSET_PANEL")
    kids = [E("Dot", 8.0, 8.0, fill=env.c("DANGER"), sx="fixed", sy="fixed", abs=(pad, 21.0, 8.0, 8.0)),
            T("Title", "Antigravity failed to start", "Large / Semibold", env.c("TEXT"), abs=(pad + 16.0, 14.0, 300.0, 20.0)),
            T("Headline", "agy rejected a launch flag", "Body / Medium", env.c("DANGER"), abs=(pad, 40.0, 344.0, 18.0)),
            T("Explanation", "This `agy` version does not accept an option Studio passed. Run it by hand to see what it supports.", "Body / Regular", env.c("TEXT_SECONDARY"),
              wrap=True, w=344.0, abs=(pad, 59.0, 344.0, 36.0)),
            T("Caption", "Last output", "Caption / Medium", env.c("TEXT_SECONDARY"), abs=(pad, 106.0, 120.0, 16.0)),
            F("Output", [T("Line", "flags provided but not defined: -mcp-config", "Mono / Value", env.c("TEXT_SECONDARY"))], dir="H", fill=env.c("BG"), stroke=env.c("BORDER_SUBTLE"),
              sw=env.c("BORDER_WIDTH"), radius=env.c("RADIUS_CONTROL"), pad=[None, None, None, env.c("INSET_CONTROL")], w=344.0, h=30.0, sx="fixed", sy="fixed", align=("start", "center"),
              abs=(pad, 126.0, 344.0, 30.0)),
            F("Fix", [T("Label", "Open a shell", "Body / Medium", env.c("TEXT_BRAND"))], dir="H", fill=env.c("BG_SELECTED"), stroke=env.c("BORDER_SELECTED"), sw=env.c("BORDER_WIDTH"),
              radius=env.c("RADIUS_CONTROL"), w=103.0, h=28.0, sx="fixed", sy="fixed", align=("center", "center"), abs=(pad, 173.0, 103.0, 28.0))]
    return F("Failure card", kids, fill=env.c("BG_SECONDARY"), stroke=env.c("BORDER"), sw=env.c("BORDER_WIDTH"), radius=env.c("RADIUS_FRAME"), w=376.0, h=217.0, sx="fixed", sy="fixed",
             abs=(12.0, 76.0, 376.0, 217.0))


def body(env, state):
    if state == "Failure":
        return [failure_card(env)]
    if state == "Chat":
        return [F("Cursor", fill=env.c("ICON"), w=8.0, h=15.0, sx="fixed", sy="fixed", abs=(12.0, 68.0, 8.0, 15.0))]
    card = X("Approval card", "Card/Approval", "", sx="fixed", sy="hug")
    prompt = F("Prompt", [T("Text", "sh-3.2$", "Mono / Value", env.c("TEXT")), F("Cursor", fill=env.c("ICON"), w=8.0, h=15.0, sx="fixed", sy="fixed")], dir="H", gap=env.c("SPACE_FACT"),
               pad=[None, None, None, env.c("INSET_FRAME")], h=15.0, sx="fill", sy="fixed", align=("start", "center"))
    gap = lit(8.0, "agent_approval_card.rs:27-31: the card keeps `SPACE_ROW` below it as its outer margin, and the capture puts the PTY text 12 under the card: 4 and 8 more")
    return [F("Approval", [card, prompt], dir="V", gap=gap, w=393.0, sx="fixed", sy="hug", align=("start", "start"), abs=(4.0, 64.0, 393.0, 150.0))]


def session_panel(env, state):
    """A PTY state (Chat, Approval) draws the viewport in HUD_POINTER_FILL under a BG_PRIMARY strip; the start-failure painter fills its rect with BG (agent_start_failure.rs:241)."""
    pty = state != "Failure"
    composer_y = 776.0 if state == "Failure" else 754.0
    fill = [F("Strip fill", fill=env.c("BG_PRIMARY"), w=W, h=24.0, sx="fixed", sy="fixed", abs=(0.0, 36.0, W, 24.0))] if pty else []
    kids = [*header(env, state), *fill, *strip(env, state == "Approval"), *body(env, state), X("Composer", "Input/Composer", "", abs=(12.0, composer_y, 376.0, 80.0))]
    if state != "Failure":
        notice = "Starting Claude Code… your message will be sent" if state == "Chat" else "Select an agent tab"
        kids.append(text_at("Notice", (20.0, 836.0, 360.0, 18.0), notice, "Caption / Regular", "TEXT_SECONDARY", env))
    return F("Agents panel", kids, fill=env.c("HUD_POINTER_FILL" if pty else "BG"), w=W, h=868.0, sx="fixed", sy="fixed", clip=True)
