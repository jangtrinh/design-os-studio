"""Registry `composite` Card/Approval (page `03 Studio`, row Agents), one plain COMPONENT (the registry gives the record no variants). Lines are the v0.7.5 tag
(`agent_approval_card.rs`).

Painter `paint_approval_card_for_tab` (:51-170), 1 production caller (the agents panel viewport, `terminal.rs`), pinned to the top of the viewport inset by `SPACE_ROW` (:68). The
card is `BG_TERTIARY` at `RADIUS_FRAME` with a 1 px `BORDER` inside stroke (:75-76); its inner rect is `INSET_FRAME` in (:78). Rows, top to bottom, `SPACE_ROW` apart:
* The header (`CONTROL_DENSE` 24, :79): the CLI badge pill (`cr_pill`, 24 high, text + 2 x `INSET_FRAME` wide, `Caption` Medium, :82-90), the `1 of N` queue count at the right edge
  (`Caption` TEXT_SECONDARY, :91-97, only when more than one request is queued) and the title (`Body` Medium TEXT, 8 after the badge, :103-110: `<CLI> wants to run <tool>` for
  Claude on its own tab, else `<CLI> needs approval in its terminal`).
* The summary (Claude only, :119-129): a `CONTROL_STANDARD` 28 box (`BG_SECONDARY`, `BORDER`, `RADIUS_CONTROL`) with the tool input in `Mono` TEXT_SECONDARY, `INSET_BUTTON` in.
* The actions (:131-170, right-aligned, `CONTROL_STANDARD` 28, `SPACE_ROW` apart): Deny (secondary) and Approve (primary :136) for Claude, or `Show prompt` (secondary :153) for the
  other CLIs on their own tab (`Open tab` elsewhere :162, a text swap). `paint_card_button` (:172-203): `Body` Medium TEXT (:182), at least `CONTROL_MD` wide, `INSET_BUTTON` of
  padding, `BG_BRAND` without an edge (primary) or `BG_SECONDARY` with a `BORDER` edge, `RADIUS_CONTROL`. These are not the kit buttons (28 high, not 32, the labels `Body` Medium),
  so they are local frames.
The card is 392 wide in the capture (the panel's 401 less 2 x `SPACE_ROW`), 104 high with the summary and the actions (`approval_card_outer_height` :24-31 less its two outer
margins) and 72 with `Show prompt` alone. The parts that depend on the CLI are BOOLEAN `Show <part>` properties (the summary, the Deny / Approve row, the `Show prompt` row and the
queue count), so one component draws the three captured cards (`i-agents-approval-command`, `agents-approval-queued`, `agents-approval-open-tab`); the texts are a screen's data
(`Badge`, `Title`, `Summary`, `Count`).
* Colours without a variable: `STATUS_WARN_BG` 35291A, `STATUS_WARN_BORDER` 5C4720 and `STATUS_WARN_TEXT` E0A53C (`terminal.rs:47-49`, the text first) are drawn as the nearest
  theme tones by value (per-channel maximum): BG_SELECTED (15), BORDER_STRONG (32) and BORDER_SELECTED (27). A deviation of the drawing from the painter, never a new literal.
Gaps: the focus stroke (:193 `BORDER_SELECTED` while focused), the hover fills (`BG_BRAND_HOVER`, `BG_HOVER`), a card on another tab (`Open tab` is a text swap of the same button).
"""
from rest_ir import F, T, lit

W = 392.0
PILL = "agent_approval_card.rs:86 `theme::cr_pill(theme::CONTROL_DENSE)` = 24 / 2: computed from the badge's height, no radius role holds it"


def button(env, name, label, primary):
    paint = {"fill": env.c("BG_BRAND")} if primary else {"fill": env.c("BG_SECONDARY"), "stroke": env.c("BORDER"), "sw": env.c("BORDER_WIDTH")}
    return F(name, [T("Label", label, "Body / Medium", env.c("TEXT"))], dir="H", radius=env.c("RADIUS_CONTROL"), pad=[None, env.c("INSET_BUTTON"), None, env.c("INSET_BUTTON")],
             h=env.num("CONTROL_STANDARD"), sx="hug", sy="fixed", align=("center", "center"), **paint)


def approval(values, env):
    badge = F("Badge", [T("Label", "CLAUDE", "Caption / Medium", env.c("BORDER_SELECTED"), prop="Badge")], dir="H", fill=env.c("BG_SELECTED"), stroke=env.c("BORDER_STRONG"),
              sw=env.c("BORDER_WIDTH"), radius=lit(12, PILL), pad=[None, env.c("INSET_FRAME"), None, env.c("INSET_FRAME")], h=env.num("CONTROL_DENSE"), sx="hug", sy="fixed",
              align=("center", "center"))
    count = T("Count", "1 of 3", "Caption / Regular", env.c("TEXT_SECONDARY"), prop="Count", visible_prop="Show count", hidden=True)
    header = F("Header", [badge, T("Title", "Claude Code wants to run Bash", "Body / Medium", env.c("TEXT"), prop="Title", sx="fill"), count], dir="H", gap=env.c("SPACE_INLINE"),
               h=env.num("CONTROL_DENSE"), sx="fill", sy="fixed", align=("start", "center"))
    summary = F("Summary", [T("Command", "cargo test -p designos-app --bin designos", "Mono / Value", env.c("TEXT_SECONDARY"), prop="Summary")], dir="H", fill=env.c("BG_SECONDARY"),
                stroke=env.c("BORDER"), sw=env.c("BORDER_WIDTH"), radius=env.c("RADIUS_CONTROL"), pad=[None, None, None, env.c("INSET_BUTTON")], h=env.num("CONTROL_STANDARD"), sx="fill", sy="fixed",
                align=("start", "center"), visible_prop="Show summary")
    actions = F("Actions", [button(env, "Deny", "Deny", False), button(env, "Approve", "Approve", True)], dir="H", gap=env.c("SPACE_ROW"), h=env.num("CONTROL_STANDARD"), sx="fill", sy="fixed",
                align=("end", "center"), visible_prop="Show actions")
    prompt = F("Prompt row", [button(env, "Show prompt", "Show prompt", False)], dir="H", h=env.num("CONTROL_STANDARD"), sx="fill", sy="fixed", align=("end", "center"),
               visible_prop="Show prompt", hidden=True)
    return F("Card", [header, summary, actions, prompt], dir="V", gap=env.c("SPACE_ROW"), fill=env.c("BG_TERTIARY"), stroke=env.c("BORDER"), sw=env.c("BORDER_WIDTH"), radius=env.c("RADIUS_FRAME"),
             pad=env.c("INSET_FRAME"), w=W, sx="fixed", sy="hug", align=("start", "start"))


def component_sets():
    return [{"name": "Card/Approval", "row": "Agents", "axes": {}, "build": approval,
             "props": [{"name": "Badge", "type": "TEXT", "default": "CLAUDE"}, {"name": "Title", "type": "TEXT", "default": "Claude Code wants to run Bash"},
                       {"name": "Summary", "type": "TEXT", "default": "cargo test -p designos-app --bin designos"}, {"name": "Count", "type": "TEXT", "default": "1 of 3"},
                       {"name": "Show summary", "type": "BOOLEAN", "default": True}, {"name": "Show actions", "type": "BOOLEAN", "default": True},
                       {"name": "Show prompt", "type": "BOOLEAN", "default": False}, {"name": "Show count", "type": "BOOLEAN", "default": False}]}]
