"""Registry `region` Panel/AgentTerminal (page `03 Studio`, row Regions), a set over `State=Picker, Chat, Failure, Approval` (the registry's). Lines are the v0.7.5 tag.

`Picker` is the panel below, today's drawing and the first variant; `Chat`, `Failure` and `Approval` are in `regions_sidepanels_agents_states.py`. The picker's probe results are
`Show <part>` properties per row (`regions_sidepanels_agents_parts.py`): the default is the capture `i-agents` (two CLIs signed in, one unknown, two checking), a screen picks the
results of `i-agents-picker`, `i-terminal` and `kit-agents`.

WHICH PAINTER: the registry describes spec I (#1438, v0.7.6: one header row, an empty new-chat body, the agent picker held in the composer), and `terminal.rs` on `0c894123` draws that. The
brief's captures (`i-terminal`, `i-agents`,
`i-agents-chat`) and the plan's truth (D5, 0.7.5) are the old panel, so this region is the v0.7.5 empty state of `i-agents`: the new-session picker in the body. Spec I supersedes it; redraw
from the new painter once v0.7.6 ships.

Painter: `show_left_with_doc` (terminal.rs:5932-6300 at v0.7.5), the left sidebar's Agents tab; the capture is `i-agents`, the region is (49, 32, 401, 868) and every rect is relative to its
top-left. State drawn: no session tab, the
picker (the CLIs the capture probes) over the `frame` sample prompts, one selection. `i-terminal` is the same picture with two CLIs `Sign-in unknown`; `i-agents-chat` (a session tab, a black
PTY view, a start notice) is a
gap, as is every other tab state.
* Nested sets (live): Tag/Chip Sm Default (the selection chip, agent_selection_chip.rs:161-187, a 24 px pill in `BG_SELECTED`, width 240 here, `w=`) and Input/Composer (the composer box,
  agent_composer.rs:954-1160, drawn from the same painter in `regions_sidepanels_composer.py`).
* Local, from the painters: the header bar (36 high: the `Sessions` ghost button, agent_open_flow.rs:428-441, 76 x `CONTROL_DENSE`; the `+` runtime button, terminal.rs:6079-6096, 28 x 24
  with an 11 px plus and an 8 px
  chevron; the 28 px `Skills` chip, :6112-6117), the strip under it (24 high, a rule on its last pixel, the chip and the `@ Mention` button, agent_mention_popup.rs:444-472, 20 high), the
  picker card (agent_picker.rs:303-456: `BG_MENU`,
  a `BORDER_MENU` stroke, `RADIUS_FRAME`, `INSET_POPOVER` in, the title in Body Medium, 44 px row pitch, the footer caption in Caption `TEXT_TERTIARY`), the `What's next?` heading
  (agent_sample_prompts.rs:261, Large Semibold,
  centred), five sample prompts (:332-352) and the attach hint (:349-354).
* Colours without a variable (terminal.rs `TERM_*` and `STATUS_*`, the Skills chip's inline greys) are drawn as the nearest theme tone by value, the mapping and distances in
  `regions_sidepanels_agents_parts.py`. The panel
  fill, 0E0E12 in the capture, is BG (10): the largest gap.
* Kit content: the selection chip's text names the file's page and layer (`Dashboards 01 · 1 selected · Desktop` in the capture): neutral `Page 1 · 1 selected · Frame 1`. Everything else is
  the Studio's own: the CLI names,
  the sample prompts and the hints are Rust strings; the versions `1.0.0` and the CLI states are the capture's probe results.
* Gaps: `i-agents-chat` and the other tab states, the sessions and `+` popovers, the install button of a missing CLI (agent_picker.rs:386-406), a selected or hovered row, the keyboard focus,
  the `Other CLI…` inline form,
  the skills picker, the turn status and usage readout, the mention popup, the resize handle.
"""
from components.regions_sidepanels_agents_parts import box, centred, glyph_box, line, picker_row, prompt_row, skills_chip, text
from components.regions_sidepanels_agents_states import session_panel
from components.regions_sidepanels_parts import label
from rest_ir import F, X

ROW = "Regions"
W, H = 401.0, 868.0
CLIS = [("Claude Code", 74.05, "1.0.0", "signed", None), ("Codex", 36.95, "1.0.0", "signed", None), ("Grok", 27.33, "1.0.0", "checking", None),
        ("Antigravity", 62.64, "1.0.0", "unknown", None), ("Gemini CLI", 62.0, "1.0.0", "checking", None), ("Other CLI…", 65.17, None, None, "Plain command: Studio tools not connected"),
        ("Shell", 28.08, None, None, None)]  # name, name width, version, the probe result the default draws, caption
PROMPTS = [("Review this frame", "Review the selected frame for spacing, alignment and cont…"), ("Make frame responsive", "Make the selected frame resize well with auto layout."),
           ("Bind variables", "Bind the selected frame's colours and spacing to this file's…"), ("Shorten text", "Rewrite the text in the selected frame to be shorter and cle…"),
           ("Try another layout", "Make a second version of the selected frame with a differe…")]


def header(env):
    sessions = box("Sessions", (10.0, 6.0, 76.0, 24.0), env, [text("Label", (2.0, 3.0, 60.0, 18.0), "Sessions", "Caption / Medium", "TEXT_SECONDARY", env),
                                                               glyph_box(env, "Chevron", (64.0, 8.0, 8.0, 8.0), "ChevronDown", 8, "TEXT_SECONDARY")], radius=None)
    plus, chevron = glyph_box(env, "Plus", (4.5, 6.5, 11.0, 11.0), "Plus", 11, "ICON"), glyph_box(env, "Chevron", (17.0, 8.0, 8.0, 8.0), "ChevronDown", 8, "ICON")
    add = box("Add session", (94.0, 6.0, 28.0, 24.0), env, [plus, chevron], fill="BG_SELECTED", stroke="BORDER")
    return [box("Bar", (0.0, 0.0, W, 36.0), env, fill="BG_SECONDARY", radius=None), line(env, "Bar rule", (0.0, 35.0, W, 1.0)), sessions, add, skills_chip(env, "Skills", (130.0, 4.0, 46.75, 28.0))]


def strip(env):
    chip = X("Selection chip", "Tag/Chip", "Size=Sm, State=Default", props={"Label": "Page 1 · 1 selected · Frame 1"}, abs=(8.0, 36.0, 240.0, 24.0), w=240.0)
    mention = box("Mention", (314.25, 38.0, 77.75, 20.0), env, [centred(env, "@ Mention", "Caption / Medium", "TEXT", 77.75, 20.0)], fill="BG_SECONDARY", stroke="BORDER")
    return [chip, mention, line(env, "Strip rule", (0.0, 59.0, W, 1.0))]


def picker(env):
    rows = [picker_row(env, f"CLI {n}", 40.0 + 44.0 * (n - 1), name, name_w, version, probe, caption, n if probe else None) for n, (name, name_w, version, probe, caption) in enumerate(CLIS, 1)]
    title = text("Title", (16.0, 15.0, 200.0, 18.0), "New session", "Body / Medium", "TEXT", env)
    footer = text("Footer", (16.0, 365.0, 200.0, 18.0), "Ask before acting", "Caption / Regular", "TEXT_TERTIARY", env)
    kids = [title, *rows, footer]
    return box("Picker", (16.0, 76.0, 368.0, 386.0), env, kids, fill="BG_MENU", stroke="BORDER_MENU", radius="RADIUS_FRAME")


def agents_panel(values, env):
    if values["State"] != "Picker":
        return session_panel(env, values["State"])
    prompts = [prompt_row(env, n, title, body) for n, (title, body) in enumerate(PROMPTS, 1)]
    kids = [*header(env), *strip(env), picker(env), label("Heading", (0.0, 476.0, W, 20.0), "What's next?", "Large / Semibold", "TEXT", env, align=("center", "center")), *prompts,
            text("Attach hint", (16.0, 738.0, 368.0, 18.0), "Type @ to mention layers.", "Caption / Regular", "TEXT_SECONDARY", env),
            X("Composer", "Input/Composer", "", abs=(12.0, 776.0, 376.0, 80.0))]
    return F("Agents panel", kids, fill=env.c("BG"), w=W, h=H, sx="fixed", sy="fixed", clip=True)


def probe_props():
    """The BOOLEAN `Show <part>` properties of the five probed rows, defaulted to the capture `i-agents` (CLIS)."""
    out = []
    for n, (_, _, _, probe, _) in enumerate(CLIS, 1):
        if probe:
            out += [{"name": f"Show probed {n}", "type": "BOOLEAN", "default": probe != "checking"}, {"name": f"Show signed in {n}", "type": "BOOLEAN", "default": probe == "signed"},
                    {"name": f"Show unknown {n}", "type": "BOOLEAN", "default": probe == "unknown"}, {"name": f"Show checking {n}", "type": "BOOLEAN", "default": probe == "checking"}]
    return out


DESCRIPTION = ("Picker is the v0.7.5 empty state (capture i-agents): the new-session picker sits in the body; its probe results are Show properties. Chat, Failure and Approval are a "
               "session tab (i-agents-chat, i-agents-start-failure, i-agents-approval-command). Spec I (v0.7.6) moves the picker into the composer and shortens the header: redraw from "
               "its painter once it ships.")


def component_sets():
    return [{"name": "Panel/AgentTerminal", "row": ROW, "axes": {"State": ["Picker", "Chat", "Failure", "Approval"]}, "uses": ["Input/Composer", "Tag/Chip", "Card/Approval"],
             "build": agents_panel, "props": probe_props(), "description": DESCRIPTION}]
