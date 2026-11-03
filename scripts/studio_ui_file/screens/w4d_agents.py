"""The agents-panel screens of batch W4D, one per 0.7.5 capture of the flow (`shell.rs` `draw_window` with the agents panel open; `agent/terminal.rs`).

`Panel/AgentTerminal` draws the state of the capture (`gap_agents_data`): `Picker` with the probe results of the five CLI rows, `Chat`, `Failure` or `Approval`; the rail shows the Agents tool.
`i-terminal` is the same new-session picker capture as `i-agents-picker`. `i-agent-permissions` keeps the region's header, strip and composer and hides the picker body, with
Popover/Panel `Kind=Permissions` over it; `agents-skills` draws the picker with Popover/Panel `Kind=Skills` over it (`gap_overlays_data.POPOVER`).
"""
from .gap_wiring import overlays
from .w4d_common import AGENTS_ALIASES, AGENTS_CANVAS, _CHIP, _COMP, _HDR, _MENTION, _PICKER, agents_shell, hide, screen

BODY = ["Picker", "Heading", "Prompt 1", "Prompt 2", "Prompt 3", "Prompt 4", "Prompt 5", "Attach hint"]
PTY = {"overrides": hide(*BODY)}  # the picker body the capture does not show: a PTY view sits there
SKILLS_ALIASES = {k: v for k, v in AGENTS_ALIASES.items() if not k.endswith("-skills")}
KIT_ALIASES = {**_MENTION, **_HDR, **_COMP, **_PICKER}
FAILURE_ALIASES = {**_MENTION, **_CHIP, **_COMP}
CHAT_ALIASES = {**_MENTION, **_CHIP}


def screens():
    return [screen("i-agents", AGENTS_CANVAS, agents_shell(), aliases=AGENTS_ALIASES),
            screen("i-agents-chat", AGENTS_CANVAS, agents_shell(), aliases=CHAT_ALIASES),
            screen("i-agents-picker", AGENTS_CANVAS, agents_shell(), aliases=AGENTS_ALIASES),
            screen("i-agents-start-failure", AGENTS_CANVAS, agents_shell(), aliases=FAILURE_ALIASES),
            screen("i-agent-permissions", AGENTS_CANVAS, agents_shell(overlays("i-agent-permissions"), terminal=PTY), aliases=CHAT_ALIASES),
            screen("i-agents-approval-command", AGENTS_CANVAS, agents_shell(), aliases=dict(_MENTION)),
            screen("i-terminal", AGENTS_CANVAS, agents_shell(), aliases=AGENTS_ALIASES),
            screen("agents-approval-open-tab", AGENTS_CANVAS, agents_shell(), aliases=dict(_MENTION)),
            screen("agents-approval-queued", AGENTS_CANVAS, agents_shell(), aliases=dict(_MENTION)),
            screen("agents-skills", AGENTS_CANVAS, agents_shell(overlays("agents-skills")), aliases=SKILLS_ALIASES),
            screen("kit-agents", AGENTS_CANVAS, agents_shell(), aliases=KIT_ALIASES)]
