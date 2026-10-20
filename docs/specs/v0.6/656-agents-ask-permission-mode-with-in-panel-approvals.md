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

What the owner has ruled ships: skip stays the default, ask is optional (one switch for all agent CLIs, Q1 Option A), a waiting tab shows "Needs approval", the approval is offered in the panel (Approve/Deny for Claude, "Open tab" for Codex/Grok/Antigravity, Q2 Option A), and an OS notification is sent when Studio is unfocused (`<CLI> needs approval` with the file name and no command text, Q4 Option A). Under the 2026-10-19 owner ruling (`owner-rulings-v06.md` line 9 and line 17), Q1–Q6 all take their recommended Option A; rule 3 (pre-allowing `mcp__designos__*`) still requires the engineering live check that Claude honours `permissions.allow` from `--settings`.

1. **Setting (ruled, Q1 & Q5 Option A).** Preferences gets "Agent permissions..." with two choices: `Skip permissions` (default, today's behaviour) and `Ask` (one switch for all agent tabs).
   Stored as `agent_permission_mode` in `settings.json` (`comments/settings.rs` store, key preserved by its `read`/`save`
   helpers). It applies to tabs started after the change; running tabs keep their mode and show it in their tooltip.
2. **Launch commands (ruled).** In `Skip` mode nothing changes. In `Ask` mode each CLI's launch command loses its skip flag
   (the table above) and keeps everything else (MCP config, hooks, skill, cwd, resume flags from #653). A test pins both command
   strings against the captured `--help` flag sets.
3. **Pre-allowed design tools (Q3 Option A; gated on live check).** In Ask mode the session settings file adds
   `"permissions": {"allow": ["mcp__designos__*"]}` so Studio's own design tools do not prompt on every canvas edit; every other tool
   asks. Before enabling this rule in shipped `hooks.json`, a live check on the installed Claude must verify that `permissions.allow` inside `--settings` takes effect for the session (an MCP call to a `designos_*` tool runs without a prompt while a shell command still prompts). Until that live check passes, the settings file carries no allow list and every tool asks.
4. **Claude in-panel approval (ruled: approval offered in the panel; mechanism documented for Claude).** In Ask mode the session `PermissionRequest` hook (added to `hooks.json`) runs a small script that:
   1. reads the hook JSON from stdin and writes it to `<session folder>/approvals/<id>.request.json` (`id` = a counter plus the
      tool name; the file holds `tool_name`, a truncated `tool_input` summary, and a timestamp);
   2. waits until `<id>.answer` appears (polling every 200 ms) for at most the hook's `timeout` (initial value 300 s);
   3. on `allow` or `deny` prints the decision JSON above (deny carries `message`: "Denied in design:os Studio");
   4. on timeout, or when Studio writes `withdrawn`, prints nothing, so Claude falls back to its own prompt in the terminal.
   Studio's bridge (`poll_live_agent_bridge`) lists new request files each frame, shows the approval card (rule 6) and writes
   the answer file when the user clicks. Requests queue per tab in arrival order.
5. **Status (ruled, Q2 Option A).** A tab with a pending request shows `Needs approval` (a new label for `WaitingForInput` with a request attached). For
   Codex, Grok and Antigravity, which have no waiting hook, the label comes from their existing status path: Grok's passive
   `Notification` hook (`permission_prompt`) is added to its hook file; Codex and agy keep the heuristic. In those tabs the card has no
   Approve or Deny buttons (Studio cannot answer for them); it offers "Open tab" so the user answers in the terminal.
6. **The card (ruled, Q4 Option A).** For a Claude request the panel shows, above the terminal viewport: the tool name, a one-line summary of the input
   (the command, or the file path; at most 200 characters, ellipsised), and the buttons Approve and Deny. Esc does not
   answer; closing the card leaves the request pending. Only one card is visible; the rest wait in the queue with a count.
7. **OS notification (ruled, Q4 Option A).** When a request arrives and the window is not focused (`ViewportInfo.focused == Some(false)`), Studio sends one
   OS notification: "<CLI> needs approval" with the file name; the notification never contains the command or path text. Clicking it
   (where the platform supports it) focuses the window and the tab.
8. **Cleanup.** When the tab closes or exits, pending requests are written `withdrawn`; stale request files older than 24 h are
   deleted at start. The approvals folder lives in the session folder (#653).
9. **Codex revisit (the ruling asks to look at it here).** This task changes nothing in how Codex status works. It records, in the PR, a
   short evaluation of the two candidates, with a live check of each on the installed Codex: (a) hooks passed so that they run
   without forging trust, if the CLI offers a supported way; (b) `--dangerously-bypass-hook-trust`, which the help text calls
   dangerous. Studio does not use (b).
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

- **Settings (Q5 Option A):** Preferences > "Agent permissions..." opens a small dialog (`RADIUS_FRAME`, `INSET_MODAL`) with two radio rows, "Skip permissions (default)" and "Ask before agents act", each with one sentence of explanation (body, `TEXT_SECONDARY`) and a note that already-open tabs keep their mode. The selected radio row uses the neutral fill.
- **Tab state (Q6 Option A):** the tab's status dot shows the waiting state using a warning constant added to `theme.rs` and `design/design.tokens.json` (P1 single token authority), and the tab tooltip says "Needs approval".
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
| `ask_mode_session_settings_have_the_permission_request_hook_with_a_timeout` | `hooks.json` has the `PermissionRequest` entry with `timeout` and carries no `permissions.allow` list before the live check |
| `ask_mode_pre_allows_designos_tools` (gated on live check) | once the live run shows the rule works: `hooks.json` has `permissions.allow == ["mcp__designos__*"]` |
| `hook_script_allow_prints_the_documented_decision` | run the script with a PermissionRequest JSON on stdin, write `allow`: stdout equals the documented JSON |
| `hook_script_deny_carries_a_message` | `deny`: `behavior: "deny"` and `message` |
| `hook_script_timeout_prints_nothing` | no answer within a 1 s test timeout: empty stdout, exit 0 |
| `request_file_holds_only_a_summary` | a 5,000-character command: stored summary at most 200 characters |
| `bridge_surfaces_a_request_and_writes_the_answer` | write a request file; the poll shows the card; a click writes `answer` |
| `status_becomes_needs_approval` | pending request: tab label; cleared after the answer |
| `grok_and_agy_cards_have_no_buttons` | "Open tab" only |
| `unfocused_window_sends_one_notification_without_the_command_text` | fake notifier records one call; text lacks the tool input |
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
- Owner rulings 2026-10-19 (`plans/reports/release-060-evidence/owner-rulings-v06.md`):
  1. Granularity: **Option A** — one switch for all agent tabs.
  2. Questions 2–6 take the recommended **Option A** (Claude gets Approve/Deny, other CLIs get "Open tab"; pre-allow `mcp__designos__*` subject to the live check; notification shows `<CLI> needs approval` and file name without command text, card shows tool name and 200-char summary; Preferences > "Agent permissions..." dialog; warning constant added to `theme.rs` and `design/design.tokens.json`).

## Open questions for the owner

None. All six questions of the first draft were resolved by the owner rulings of 2026-10-19 and are reflected in §3, §7, §10 and §13 above.

## Could not ground

- Whether Claude Code shows its own terminal permission dialog while the `PermissionRequest` hook is still running (the permissions
  and hooks pages do not say). A live run settles it; rule 8.2 assumes it may.
- Grok's behaviour for `permission_prompt` notifications: the event exists in the user guide's table; its payload fields were not
  read for this spec.
- The notification crate and its per-platform behaviour (none in the repository; not chosen here).
- That a `permissions.allow` rule inside the `--settings` file takes effect for the session: the permissions page says rules live in
  settings files and `--settings` loads one, but no run was made for this spec; the pre-allow test in the plan is a live check too.
