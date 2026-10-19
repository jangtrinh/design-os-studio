# #656 Agents settings: optional "ask" permission mode with in-panel approvals

| Field | Value |
|---|---|
| Issue | #656 (owner ruling 2026-10-19; audit proposal 12). Needs #647 (session hooks loaded), which is merged in 0.5.0. |
| Size / priority | L / P2 |
| Depends on | #647 (done), #653 (stable session folder for the approvals folder), #652 (permission caption in the picker) |
| Unblocks | the "revisit hook-based Codex status" ruling (section 3.9) |
| Touches | new `crates/designos-app/src/agent_permissions.rs` (mode, launch flags, approvals queue, hook scripts), new `crates/designos-app/src/agent_approval_card.rs` (UI), `crates/designos-app/src/terminal.rs` (hot spot: `cli_launch_command` `:155-180`, `build_real_cli_command` `:286-326`, `session_hooks_settings` `:2207-2221`, `poll_live_agent_bridge` `:1280-1364`), `crates/designos-app/src/comments/settings.rs` (setting store), `crates/designos-app/src/panels/menus.rs` (Preferences entry), tests |

Code baseline: `origin/main@01d48f2` (0.5.0). CLI facts: each CLI's `--help` captured 2026-10-19
(`plans/reports/release-060-evidence/cli-help-261019/`); Claude from its hooks and permissions documentation
(code.claude.com/docs/en/hooks, `/permissions`, fetched 2026-10-19); Grok from its bundled user guide
(`~/.grok/docs/user-guide/10-hooks.md`, `22-permissions-and-safety.md`, grok 1.0.62).

## 1. Summary and user story

As a designer who wants to see what an agent is about to do, I can switch the agent tabs to "ask" mode. When the agent waits for
approval, its tab says "Needs approval", the panel shows the request with Approve and Deny, and Studio notifies me if it is in the
background. The default stays as today: agents run with permissions skipped.

## 2. Current state (FACT)

- Every agent launch skips permission prompts (`AgentCliPreset::cli_launch_command`, `terminal.rs:155-180`):
  Claude `--dangerously-skip-permissions`, Codex `--dangerously-bypass-approvals-and-sandbox`, Grok `--always-approve`,
  Antigravity `--dangerously-skip-permissions`. The `+` menu header says "LAUNCH CLI (SKIP PERMISSIONS)" (`:3123`).
- There is no Settings window. App settings live in `settings.json` in the library root, read by `comments/settings.rs`
  (`load_flag`, `save_flag`); the menu has Preferences with "Your name..." and "Use GPU rendering" (`panels/menus.rs:511-521`).
- Studio writes `hooks.json` (a Claude-shaped settings file passed with `--settings`) and equivalents for Grok and agy
  (`session_hooks_settings`, `agy_session_hooks`, `terminal.rs:2194-2233`). Its hooks write `{"state": ...}` to
  `agent-status.json`: `UserPromptSubmit`, `PreToolUse`, `PostToolUse` -> working, `PermissionRequest` -> waiting, `Stop` -> done.
  `poll_live_agent_bridge` reads that file for the active tab (`terminal.rs:1328-1338`). Because every launch skips permissions,
  `PermissionRequest` never fires today (audit note on `:140-163`).
- `AgentTabStatus` has `Idle`, `Working`, `WaitingForInput`, `Done` (`terminal.rs:357-364`); the heuristic path sets it from
  OSC titles, spinner text and output timing (`parse_osc_agent_status_and_strip`, `:1611-1652`; `:1940-1955`).
- Codex tabs keep the heuristic status: no hooks are loaded; the comment at `terminal.rs:160-169` records why (Codex runs only
  hooks the user reviewed, Studio must not forge trust, `-c` hooks were not run on 0.163, `notify` would replace the user's own
  program and never reports `working`).
- No OS notification code or crate exists in `designos-app` (`Cargo.toml`). egui 0.35 (`Cargo.lock`) reports window focus:
  `egui::ViewportInfo::focused: Option<bool>`.
- What each CLI supports for asking (sources above):

| CLI | Ask mode | A hook that can wait for the user's answer | Status of "waiting" |
|---|---|---|---|
| Claude Code 2.1.302 | omit `--dangerously-skip-permissions`; `--permission-mode` choices `acceptEdits, auto, bypassPermissions, manual, dontAsk, plan` | **Yes.** The `PermissionRequest` hook receives `tool_name`, `tool_input`, optional `permission_suggestions`; stdout JSON `{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow"\|"deny","message":"..."}}}` decides; printing nothing leaves the normal prompt; the `timeout` field of the hook entry overrides the 600 s default. | the `PermissionRequest` hook itself |
| Codex 0.163.0 | omit the bypass flag; `-a/--ask-for-approval` accepts `on-request` and `never`; `-s/--sandbox` | no hooks loaded (see above); `--dangerously-bypass-hook-trust` exists ("DANGEROUS") | PTY heuristic only |
| Grok 1.0.62 | omit `--always-approve`; `--permission-mode default` is "ask" (user guide) | **No for approvals.** The hook events are `SessionStart`, `UserPromptSubmit`, `PreToolUse`, `PostToolUse`, `PostToolUseFailure`, `PermissionDenied`, `Stop`, `StopFailure`, `StopCancelled`, `Notification`, `SubagentStart/Stop`, `PreCompact`, `PostCompact`, `SessionEnd` (no `PermissionRequest`). A `PreToolUse` `allow` "does not auto-approve a call the user would otherwise be asked about"; `ask` and `defer` leave the call to the native prompt. | the passive `Notification` event with `permission_prompt` (user guide, hook events table) |
| Antigravity 1.4.2 | omit `--dangerously-skip-permissions`; `--mode` accepts `accept-edits, auto, plan` | Studio's agy hooks cover `PreInvocation` and `Stop` only (`agy_session_hooks`); no approval event found | PTY heuristic only |

- Existing evidence that a settings file carries MCP allow rules: Claude documents `mcp__<server>__*` as the rule that matches every
  tool of an MCP server (permissions page); Studio's server key in `mcp-config.json` is `designos` (`terminal.rs:861-880`), so the rule
  is `mcp__designos__*`.

## 3. Behaviour rules

What the owner has ruled ships: skip stays the default, ask is optional, a waiting tab shows "Needs approval", the approval is offered in the panel, and an OS notification is sent when Studio is unfocused. Rules tagged **(ruled)** are that behaviour; rules tagged **(pending Qn)** are written as the recommended answer so the shape is visible, are **not built and not covered by a required test until the owner answers question n**, and pre-allow (rule 3) additionally needs a live run before it can be Ready.

1. **Setting (ruled; surface pending Q5 and Q1).** The issue says "Settings -> Agents", but no Settings window exists (section 2); the proposal is a Preferences entry (open question 5). Preferences gets "Agent permissions..." with two choices: `Skip permissions` (default, today's behaviour) and `Ask`.
   Stored as `agent_permission_mode` in `settings.json` (`comments/settings.rs` store, key preserved by its `read`/`save`
   helpers). It applies to tabs started after the change; running tabs keep their mode and show it in their tooltip. (Granularity:
   open question 1.)
2. **Launch commands (ruled).** In `Skip` mode nothing changes. In `Ask` mode each CLI's launch command loses its skip flag
   (the table above) and keeps everything else (MCP config, hooks, skill, cwd, resume flags from #653). A test pins both command
   strings against the captured `--help` flag sets.
3. **Pre-allowed design tools (pending Q3; not built, no required test, live check first).** In Ask mode the session settings file adds
   `"permissions": {"allow": ["mcp__designos__*"]}` so Studio's own design tools do not prompt on every canvas edit; every other tool
   asks. Whether to pre-allow is the owner's call (open question 3), and no run has shown that a `permissions.allow` rule inside a `--settings` file takes effect for the session (section "Could not ground"): until the owner answers A and a live run shows Claude honouring the rule (an MCP call to a `designos_*` tool runs without a prompt while a shell command still prompts), the settings file carries no allow list and every tool asks. Auto-approving all Studio tools (including delete and apply-spec) before that answer is explicitly out.
4. **Claude in-panel approval (ruled: approval offered in the panel; mechanism documented for Claude).** In Ask mode the session `PermissionRequest` hook (added to `hooks.json`) runs a small script that:
   1. reads the hook JSON from stdin and writes it to `<session folder>/approvals/<id>.request.json` (`id` = a counter plus the
      tool name; the file holds `tool_name`, a truncated `tool_input` summary, and a timestamp);
   2. waits until `<id>.answer` appears (polling every 200 ms) for at most the hook's `timeout` (initial value 300 s);
   3. on `allow` or `deny` prints the decision JSON above (deny carries `message`: "Denied in design:os Studio");
   4. on timeout, or when Studio writes `withdrawn`, prints nothing, so Claude falls back to its own prompt in the terminal.
   Studio's bridge (`poll_live_agent_bridge`) lists new request files each frame, shows the approval card (rule 6) and writes
   the answer file when the user clicks. Requests queue per tab in arrival order.
5. **Status (ruled; buttons for other CLIs pending Q2).** A tab with a pending request shows `Needs approval` (a new label for `WaitingForInput` with a request attached). For
   Codex, Grok and Antigravity, which have no waiting hook, the label comes from their existing status path: Grok's passive
   `Notification` hook (`permission_prompt`) is added to its hook file; Codex and agy keep the heuristic. In those tabs the card has no
   Approve or Deny buttons (Studio cannot answer for them); it offers "Open tab" so the user answers in the terminal. (Scope:
   open question 2.)
6. **The card (ruled for Claude).** For a Claude request the panel shows, above the terminal viewport: the tool name, a one-line summary of the input
   (the command, or the file path; at most 200 characters, ellipsised), and the buttons Approve and Deny. Esc does not
   answer; closing the card leaves the request pending. Only one card is visible; the rest wait in the queue with a count.
7. **OS notification (ruled; wording pending Q4).** When a request arrives and the window is not focused (`ViewportInfo.focused == Some(false)`), Studio sends one
   OS notification: "<CLI> needs approval" with the file name; the notification never contains the command or path text. Clicking it
   (where the platform supports it) focuses the window and the tab. (Content: open question 4.)
8. **Cleanup.** When the tab closes or exits, pending requests are written `withdrawn`; stale request files older than 24 h are
   deleted at start. The approvals folder lives in the session folder (#653).
9. **Codex revisit (the ruling asks to look at it here).** This task changes nothing in how Codex status works. It records, in the PR, a
   short evaluation of the two candidates, with a live check of each on the installed Codex: (a) hooks passed so that they run
   without forging trust, if the CLI offers a supported way; (b) `--dangerously-bypass-hook-trust`, which the help text calls
   dangerous. Studio does not use (b). The outcome goes to the owner as a decision (open question 2), not into code.
10. Skip mode behaves exactly as today, including no allow list and no approvals folder.

## 4. Data model / schema changes

- Setting `agent_permission_mode: "skip" | "ask"` (string) in `settings.json`.
- Files: `<session>/approvals/<id>.request.json`, `<id>.answer` (`allow`, `deny` or `withdrawn`). Both are Studio-internal.
- `AgentTabStatus::WaitingForInput(Option<String>)` already carries an optional label (`terminal.rs:362`); the request summary
  fills it.

## 5. Contracts

- The hook script is a POSIX `sh` script written into the session folder; on Windows the unchanged "not verified on Windows" rule
  applies (releases are verified on macOS).
- The `timeout` field is set explicitly on the `PermissionRequest` entry (documented: the default is 600 s and the field overrides it).
- No MCP change.

## 6. File-format mapping

None.

## 7. UI

Described in words. Checked against `design/principles.md` (P1 to P10) and `design/soul.md`; the rules used, with the `theme.rs` constants, are recapped in README-part3 under "UI rules checked for part 3". In short: every colour, gap, inset, control height and radius below is a `crates/designos-app/src/theme.rs` constant (the panel's own `TERM_*` / `STATUS_*` constants at `terminal.rs:29-55` stay for the terminal viewport and the existing tab status dots and are not extended); selected and active rows use the neutral fill `BG_SELECTED`; the brand orange is the view's one primary action; counts sit in their own pill, not in a label.

The issue fixes: an Ask option in settings, a "Needs approval" tab state, the approval offered in the panel, an OS notification when unfocused. The look is a proposal for the owner's confirmation:

- **Settings:** Preferences > "Agent permissions..." opens a small dialog (`RADIUS_FRAME`, `INSET_MODAL`) with two radio rows, "Skip permissions (default)" and "Ask before agents act", each with one sentence of explanation (body, `TEXT_SECONDARY`) and a note that already-open tabs keep their mode. The selected radio row uses the neutral fill.
- **Tab state:** the tab's status dot shows the waiting state the panel already draws for `WaitingForInput` (`paint_orca_tab_status_and_provider`), and the tab tooltip says "Needs approval". A distinct warning colour would need a new token in `theme.rs` (P1): see open question 6.
- **Card:** a `RADIUS_FRAME` surface (`BG_SECONDARY`, `BORDER` hairline, inset `INSET_FRAME`) at the top of the viewport area, sized by its content: the tool name in body-medium and the input summary in `font_mono` caption (elided), `SPACE_FACT` apart; at the right a `Deny` secondary button and an `Approve` primary button (`BG_BRAND` with a dark label: the view's one primary action), both `CONTROL_STANDARD` high with `RADIUS_CONTROL`; a count pill (same geometry as above) with the caption "waiting" when requests are queued.
- Desktop captures at 1440: settings dialog, card with a command, queued card, "Open tab" card for a CLI without approvals. Responsive SKIPPED (owner default). Load es-designer before UI code; update the Figma frames from the captures first.

## 8. Edge cases

1. Studio is closed or frozen while a request waits: the hook times out and Claude shows its own prompt.
2. The user answers in the terminal while the card is up: the hook's answer arrives late or never; the card clears when the
   tab's status leaves "waiting" (a `PostToolUse` or `Stop` write), and Studio writes `withdrawn`. (Whether Claude shows its dialog
   while the hook is still running is not stated in the docs: see "Could not ground".)
3. Two tabs, two requests: separate cards per tab; each tab's approvals folder is its own.
4. A request for a tool with a huge input: only the 200-character summary is shown and stored.
5. The same request fires twice: two ids, two cards.
6. `Skip` mode while an `Ask` tab is open: the old tab keeps asking until closed.
7. Grok or Codex in Ask mode: no buttons; "Open tab".
8. Notification permission denied by the OS: no notification, the in-panel badge still shows.

## 9. Non-goals

- A persistent allow-list ("always allow"); the CLIs keep their own.
- Answering Codex, Grok or agy prompts by typing into their terminals.
- Per-tool rules editing in Studio.
- Changing the default.

## 10. Test plan (red first)

Seams: launch-string tests; the hook script run as a real `sh` process with synthetic stdin; bridge tests with a stub agent; a `Sim`
test for the card.

| Case | Expected (fails before) |
|---|---|
| `skip_mode_launch_commands_unchanged` | the four strings equal today's (`cli_launch_command`) |
| `ask_mode_drops_each_skip_flag` | no `--dangerously-skip-permissions`, no `--dangerously-bypass-approvals-and-sandbox`, no `--always-approve` in the four strings; flags checked against the captured `--help` sets |
| `ask_mode_session_settings_have_the_permission_request_hook_with_a_timeout` | `hooks.json` has the `PermissionRequest` entry with `timeout` and **no** `permissions.allow` |
| (pending Q3, live check first) `ask_mode_pre_allows_designos_tools` | only after the owner answers A and a live run shows the rule works: `hooks.json` has `permissions.allow == ["mcp__designos__*"]` |
| `hook_script_allow_prints_the_documented_decision` | run the script with a PermissionRequest JSON on stdin, write `allow`: stdout equals the documented JSON |
| `hook_script_deny_carries_a_message` | `deny`: `behavior: "deny"` and `message` |
| `hook_script_timeout_prints_nothing` | no answer within a 1 s test timeout: empty stdout, exit 0 |
| `request_file_holds_only_a_summary` | a 5,000-character command: stored summary at most 200 characters |
| `bridge_surfaces_a_request_and_writes_the_answer` | write a request file; the poll shows the card; a click writes `answer` |
| `status_becomes_needs_approval` | pending request: tab label; cleared after the answer |
| (pending Q2) `grok_and_agy_cards_have_no_buttons` | "Open tab" only |
| `unfocused_window_sends_one_notification` (ruled) and, pending Q4, `..._without_the_command_text` | fake notifier records one call; text lacks the tool input |
| `default_mode_is_skip` | fresh settings file: `skip` |
| `claude_real_run_in_ask_mode` (owner-manual) | a real Claude tab asks, the card appears, Approve lets the tool run, Deny blocks it (screenshots) |

Run: `cargo test -p designos-app`. Evidence: red/green capture, the four 1440 captures, and the owner-manual run recorded with
screenshots (owner-manual; an agent never claims it).

## 11. Performance budgets

- The approvals folder is read in the existing per-frame bridge poll only when it exists; a `read_dir` of a few entries, under 0.1 ms.
- The hook script polls every 200 ms inside the agent's process, not in Studio.
- No timers in Studio beyond the bridge's existing cadence; no periodic repaint when no request is pending.

## 12. Dependencies and build order

After #653 and with #652 (caption). `terminal.rs` is a hot spot: the permission logic is a new module; `terminal.rs` gets the
launch-string and hook-file call sites. A new notification dependency (crate choice and licence check, for example `notify-rust`)
is verified by the worker before adding; no dependency is added by this spec.

## 13. Owner rulings

- Agent tabs keep skip-permissions by default. Settings > Agents adds an "ask" mode. In ask mode, a tab whose agent waits shows
  "Needs approval" and offers the approval in the panel. An OS notification is sent when Studio is unfocused. (Issue text.)
- Codex tabs keep the OSC/PTY heuristic status in 0.5.0; hook-based Codex status is revisited here, together with the ask mode
  (comment of 2026-10-19).
- Studio never forges or writes Codex's hook trust (`terminal.rs:164-169`). `GROK_FOLDER_TRUST=0` stays.

## Open questions for the owner

1. **Granularity of the setting.**
   - A (recommended): one switch for all agent tabs, because the issue describes one "ask" mode in Settings > Agents.
   - B: a switch per CLI, since the CLIs differ in what they can show.
2. **Which CLIs get in-panel Approve and Deny, and what happens to Codex.** Only Claude Code documents a hook that can wait for an
   answer. Codex has `--dangerously-bypass-hook-trust`, which conflicts with the rule that Studio never forges trust.
   - A (recommended): Claude gets Approve and Deny in the panel; Codex, Grok and Antigravity get the "Needs approval" state and an
     "Open tab" button, and the user answers in the terminal. Do not use the bypass-trust flag.
   - B: ask mode is offered for Claude only; the other CLIs stay in skip mode until they gain a supported way.
   - C: type the approval keystroke into the other CLIs' terminals. Not verified for any CLI and breaks when a prompt changes; not
     recommended.
3. **Pre-allowing Studio's own design tools in ask mode.** Without it, every canvas edit by the agent asks.
   - A (recommended): pre-allow `mcp__designos__*` (the user's own file) and ask for everything else.
   - B: ask for those too; safest, but a design session becomes a stream of prompts.
4. **What the notification and the card show.** A command can contain secrets.
   - A (recommended): the notification shows only "<CLI> needs approval" and the file name; the card in the panel shows the tool
     name and a 200-character summary.
   - B: the notification also shows the summary.
   - C: the card shows only the tool name.
5. **Where the setting lives.** The app has a Preferences menu (Your name, GPU rendering) and no Settings window.
   - A (recommended): a Preferences entry "Agent permissions..." with a small dialog, the smallest change that gives the setting a home.
   - B: build a Settings window with an Agents page, as the issue title names it; larger, and it adds an empty shell for later pages.
6. **A colour for "Needs approval".** `theme.rs` has no warning or info colour (the panel's `STATUS_WARN_*` live in `terminal.rs:45-47`), and `design/principles.md` P1 requires colours to come from `theme.rs` and `design/design.tokens.json`.
   - A (recommended): add one warning constant to `theme.rs` for this state by the interim route the owner ruled for the design standard (R8, 2026-10-18: constants in `theme.rs` guarded by the ratchet and parity checks until `ui ds add-token` exists), so the new state follows the single token authority.
   - B: reuse the panel's existing waiting dot colour with no new token; no design-system change, but it adds to the P1 debt in `terminal.rs`.

## Could not ground

- Whether Claude Code shows its own terminal permission dialog while the `PermissionRequest` hook is still running (the permissions
  and hooks pages do not say). A live run settles it; rule 8.2 assumes it may.
- Grok's behaviour for `permission_prompt` notifications: the event exists in the user guide's table; its payload fields were not
  read for this spec.
- The notification crate and its per-platform behaviour (none in the repository; not chosen here).
- That a `permissions.allow` rule inside the `--settings` file takes effect for the session: the permissions page says rules live in
  settings files and `--settings` loads one, but no run was made for this spec; the pre-allow test in the plan is a live check too.
