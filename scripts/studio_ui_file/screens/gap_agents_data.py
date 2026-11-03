"""Per-screen data of the agents panel states (Picker, Chat, Failure, Approval): plain dicts keyed by the capture's state, no `screens()` and no gate table.

`AGENTS[state]` is what a screen passes with `Panel/AgentTerminal` (`variant`, `props`, `overrides`). `Picker` states choose each probed row's result with the `Show <part>` properties
(`probes(...)`); the session states carry the capture's tab title and card texts as overrides on the nested parts (paths below the instance). Every text is a Rust string of the 0.7.5 painters.

    i-agents                   Picker, the default probe results (signed in, signed in, checking, unknown, checking)
    i-agents-picker            Picker, signed in, signed in, then three unknown; `i-terminal` is the same 0.7.5 capture, so the same entry
    kit-agents                 Picker, checking, checking, unknown, checking, unknown
    i-agents-chat              Chat (Claude Code #1, the PTY with its cursor)
    i-agents-start-failure     Failure (Antigravity #1, the start-failure card)
    i-agents-approval-command  Approval, Claude's card: the command, Deny and Approve
    agents-approval-queued     Approval, the same card with another command and the `1 of 3` queue count
    agents-approval-open-tab   Approval, Grok's card: `needs approval in its terminal` and `Show prompt`
"""
SIGNED, UNKNOWN, CHECKING = "signed", "unknown", "checking"
CARD = "Approval/Approval card"
HINT = {"path": "Composer/Hint", "text": "Select an agent tab"}  # agent_composer.rs:83 `Self::NoAgent => "Select an agent tab"`: the composer of a tab with no agent


def probes(*results):
    """The BOOLEAN `Show <part>` properties of the five probed rows for `results` (signed, unknown or checking each)."""
    out = {}
    for n, r in enumerate(results, 1):
        out.update({f"Show probed {n}": r != CHECKING, f"Show signed in {n}": r == SIGNED, f"Show unknown {n}": r == UNKNOWN, f"Show checking {n}": r == CHECKING})
    return out


def _hide(path):
    return {"path": f"{CARD}/{path}", "visible": False}


def _show(path):
    return {"path": f"{CARD}/{path}", "visible": True}


def _say(path, text):
    return {"path": f"{CARD}/{path}", "text": text}


def _entry(state, props=None, overrides=None):
    """One state's placement data: a key is present only when it is set, so a screen can pass the entry on verbatim (`at(.., **entry)`)."""
    entry = {"variant": {"State": state}}
    if props:
        entry["props"] = props
    if overrides:
        entry["overrides"] = overrides
    return entry


PICKER_PROBES = probes(SIGNED, SIGNED, UNKNOWN, UNKNOWN, UNKNOWN)
AGENTS = {
    "i-agents": _entry("Picker", probes(SIGNED, SIGNED, CHECKING, UNKNOWN, CHECKING)),
    "i-agents-picker": _entry("Picker", PICKER_PROBES),
    "i-terminal": _entry("Picker", PICKER_PROBES),
    "kit-agents": _entry("Picker", probes(CHECKING, CHECKING, UNKNOWN, CHECKING, UNKNOWN)),
    "i-agents-chat": _entry("Chat"),
    "i-agents-start-failure": _entry("Failure"),
    "i-agents-approval-command": _entry("Approval", overrides=[HINT]),
    "agents-approval-queued": _entry("Approval", overrides=[HINT, _say("Summary/Command", "git status --short"), _show("Header/Count")]),
    "agents-approval-open-tab": _entry("Approval", overrides=[
        HINT, {"path": "Bar/Tab slot/Tab/Row/Title", "text": "Grok CLI #1"}, _say("Header/Badge/Label", "GROK"), _say("Header/Title", "Grok Build needs approval in its terminal"),
        _hide("Summary"), _hide("Actions"), _show("Prompt row")]),
}
