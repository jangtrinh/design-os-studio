# #930 Codex approvals in the Agents panel: decision record

Status: **pending Jang** (not ratified).

Also takes #1195 AC-3 (Allow and Deny in the banner) for the one agent family that has a structured approval API.

## Decision

| Agent | Mechanism | Card |
|---|---|---|
| Claude Code | The #656 bridge (`PermissionRequest` hook and the private approvals folder) | Approve / Deny, unchanged |
| Codex 0.164.x, macOS, Ask mode | `codex app-server` on a Unix socket, JSON-RPC. Studio is a second client of the server the TUI runs against (`codex --remote unix://<socket>`) | Approve / Deny |
| Codex on another version, on Linux or Windows, or when the link cannot start | The version or platform gate fails | The #1195 banner (Show prompt / Open tab), with the reason as a caption |
| Codex, link lost while a request waits | The connection to the app-server ended | "Lost the link to Codex approvals", Show prompt / Open tab |
| Grok Build, Antigravity (agy), Gemini | No structured approval API | The #1195 banner |

The last row is the reason these three get no Approve or Deny: none of them exposes a request, an id, or a way to answer one
other than its own terminal menu. Reading their terminal text to build buttons, or sending keys to answer, is excluded below.

## Non-goals (binding)

- No keystroke injection. Nothing sends Enter, Esc, a digit or any other key into a CLI's PTY for the user.
- No PTY scraping to build cards or make terminal text clickable for Codex. (Gemini's detection-only read of its on-screen
  dialog, PR #1058, is an explicit exception that does not extend to Codex or any other agent.)

## How an answer travels

1. The app-server sends a request with an id. Studio maps it to a card only when every field the card needs is structured and
   present (`codex_remote/card_mapping.rs`). Anything else (a form, a network rule, another folder, a sub-agent) stays with the
   terminal's own menu and the tab shows the #1195 banner.
2. A click on Approve or Deny sends one JSON-RPC response to that request id (`CodexLink::answer`). The PTY gets nothing.
3. If the request is answered in Codex's own terminal, the server sends `serverRequest/resolved` and the card closes on the
   next frame. A click on a card that is gone sends nothing.
4. A file in the tab's approvals folder is never read as a Codex request or answer, so the agent cannot answer for itself.
5. When the tab closes or its process exits, its cards and its server are dropped. Other tabs keep theirs.

## Evidence this rests on

Per the #930 comment of 2026-10-23, a live spike passed every protocol and sandbox gate against a scripted provider on Codex
0.164.0. **No real-model turn has run.** That stays an owner-manual check on a Mac and is not claimed here.

## Gates

The card path starts only when all hold: macOS, Ask mode, a new Codex session (not a resume or a login), the exact `codex`
the tab will run reports `0.164.x`, a private channel folder exists, the rendezvous path fits a Unix socket, and the server
starts. When any row fails the tab starts exactly as before.
