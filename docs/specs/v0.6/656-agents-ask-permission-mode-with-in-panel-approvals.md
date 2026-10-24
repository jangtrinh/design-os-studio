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
- Studio writes `hooks.json` (a Claude-shaped settings file passed with `--settings`; in Ask mode it lives in the Studio-private folder, see rule 4) and equivalents for Grok and agy
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
   1. reads the hook JSON from stdin and writes it to `<private folder>/tab-<tab id>/<id>.request.json` (`id` = 128 random bits as 32 hex
      digits; the file holds `tool_name`, a truncated `tool_input` summary, and a timestamp);
   2. waits until `<id>.answer` appears (polling every 200 ms) for at most the hook's `timeout` (initial value 300 s);
   3. on `allow` or `deny` prints the decision JSON above (deny carries `message`: "Denied in design:os Studio");
   4. on timeout, or when Studio writes `withdrawn`, prints nothing, so Claude falls back to its own prompt in the terminal.
   Studio's bridge (`poll_live_agent_bridge`) lists new request files each frame, shows the approval card (rule 6) and writes
   the answer file when the user clicks. Requests queue per tab in arrival order.
   Whoever can write an answer, or the script that decides what to print, approves the agent's next action. So the script and the
   `<private folder>` (`<data dir>/designos/ap/<session id>/`, mode 0700; on macOS under `~/Library/Application Support`) sit outside
   the session folder (the agent's working folder), `/tmp`, `$TMPDIR` and every `--add-dir`, and the script takes no path or request
   id from its environment. A file in the session folder, however named, answers nothing and raises no card. The hooks file that
   names the script is just as decisive, and Claude's docs say edits to hooks in settings files are normally picked up as they
   happen. So in Ask mode Claude's `--settings` names `<private folder>/hooks.json` (the tab's `DESIGNOS_CLAUDE_SETTINGS`, mode
   0600, rewritten at every sync), and Studio writes no Claude hooks into the session folder (a leftover `hooks.json` that names the
   approval hook is removed). With no private folder, Claude gets the inline settings `{}`: none of Studio's hooks, its own terminal
   prompts. Skip mode keeps `<session>/hooks.json` and its launch strings byte for byte. On Unix the handler runs `/bin/sh` by path,
   and the hook script puts `/usr/bin:/bin` first on `PATH`.
   Claude also loads `<session>/.claude/settings.json`, `settings.local.json` and `.mcp.json` from its working folder, live (allow
   rules, hooks and MCP servers); Studio writes none of the three. Who guards them: Claude's own Write and Edit tools treat `.claude`
   as a protected path, so a write there is prompted in the `default` and `acceptEdits` modes and `permissions.allow` rules do not
   pre-approve it (Claude Code, Permission modes, "Protected paths": https://code.claude.com/docs/en/permission-modes). Another
   runtime's sandbox does not: a sandboxed Codex `workspace-write` command writes them without a prompt (checked with Codex 0.164.0).
   So the Ask launch also passes `--setting-sources user`, with which Claude 2.1.304 applied none of a forged allow rule, a forged
   project or local hook or a forged `.mcp.json` server (probed with the debug log and marker files), while `--settings`,
   `--mcp-config` and `--plugin-dir` still apply. That flag also drops the project skill folder, so Studio's session skill reaches the
   Ask launch as a plugin: `--plugin-dir "$DESIGNOS_CLAUDE_PLUGIN"`, a folder in the private folder that holds only the plugin manifest
   and `skills/designos-studio/SKILL.md` (the session copy under `.claude/skills` stays for Skip tabs). The three variables
   (`DESIGNOS_CLAUDE_MCP_CONFIG`, `DESIGNOS_CLAUDE_SETTINGS`, `DESIGNOS_CLAUDE_PLUGIN`) are never empty: Claude reads an empty
   `--plugin-dir` as its working folder and an empty `--settings` as no settings, so with no private plugin the value is an absolute
   path nothing exists at, and with no private hooks file it is the inline settings `{}`.
   Claude starts the servers of its `--mcp-config` file as commands, outside any sandbox, and Codex's `workspace-write` sandbox can write
   `<session>/mcp-config.json`, which Studio rewrites at every sync but which Claude may read before the next one (#1046). So in Ask
   mode `--mcp-config` names `<private folder>/mcp-config.json` (the tab's `DESIGNOS_CLAUDE_MCP_CONFIG`, mode 0600, rewritten at every
   sync): the session's own config, except that the `designos` server's `DESIGNOS_MCP_CONFIG` names this copy. With no private folder
   Claude gets the inline config `{"mcpServers":{}}`: no server and so no `designos` tools, the fail-closed side. Skip mode keeps
   `<session>/mcp-config.json`. No file in the session folder is named by anything Claude starts in Ask mode, and the private plugin
   skill and the private MCP config are generated by Studio from its own state, never copied from a file in the session folder. The
   only session data they carry is the user's `designos.enabled_tools` and `known_tools` list, rebuilt from known tool names (#1049).
   **Session folders live where no agent sandbox can write, and Studio makes each one itself** (#1057): unsaved-file and fallback sessions
   sit in `<data dir>/designos/sessions/<session id>` (0700), not in `$TMPDIR` (Codex's `workspace-write` sandbox writes it, so an agent could
   plant a link at the name a later tab gets). Each folder is made with an exclusive `mkdir`, an existing path is used only as a real folder
   of the user, and otherwise the tab says why it did not start.
   **Studio-written files that a CLI loads by itself carry no document-controlled text.** Those files are the session skill (the
   private plugin copy, `.claude/skills`, `.agents/skills`), `AGENTS.md`, `GEMINI.md` and the MCP and hook configs of every CLI.
   A document or page name comes from the file shown, which a third-party file or an agent that wrote the document can set, and
   a CLI acts on text it loads: Claude on frontmatter keys in a skill (`allowed-tools` runs tools without asking, `hooks`
   registers commands), on `` !`command` `` placeholders and ```` ```! ```` fences (they run when the skill is invoked, before
   any model call) and on `@path` (the file is attached, without asking); Gemini imports `@path` from `GEMINI.md`. Escaping one
   syntax at a time left the next one open, so the names are not written at all: the skill points at `$DESIGNOS_DOC_NAME` and
   `designos_get_app_state`, and the MCP server's environment holds ids and paths only (`DESIGNOS_FILE`, the document's path on
   disk, stays: `designos-mcp` needs it). The paths in the skill hold no name either: each is the session folder plus a fixed file
   name, and the folder is named by Studio (a library file id reduced to `[A-Za-z0-9_-]`, a hash of a disk path, or process and
   tab numbers). The skill's `description` is fixed text and is still written as a JSON string, as a second guard. The files that
   do carry the document's text (`context.json`, `bootstrap.md`, `bootstrap.json`, `canvas.designos`, the hook replies) are read
   by the agent on request or printed by a hook, and no CLI loads one as a skill or as memory. The tests compare the generated
   text and use no YAML or Markdown parser: every file a CLI loads by itself is the same bytes for any two document and page names,
   hostile ones included (frontmatter keys, `!` placeholders, `@` paths); the frontmatter is exactly two lines; and the template
   and the rules pack hold no `!` placeholder and no `@path`.
   **Claude's own "don't ask again" does not persist in Ask mode** (owner ruling, 2026-10-24, #1044). Claude saves "Yes, and don't
   ask again" for Bash and WebFetch to `.claude/settings.local.json` in its working folder, and `--setting-sources user` makes Ask
   tabs ignore project and local settings, so that choice is not kept there: the next such request is expected to ask again (in the
   panel, through the hook). That is an owner-manual check with a logged-in Claude and has not been run. Settings the user keeps in
   their own user settings still apply. Skip mode is unchanged.
   Known limits: Antigravity, Gemini CLI and Grok start MCP servers from their own copies of the session config in session
   subfolders, which an agent can write (#1048); and the user's `designos.enabled_tools` list lives in the agent-writable session
   file, so an agent can re-enable a tool the user switched off (#1049). See `crates/designos-app/src/agent_approval_channel.rs`.
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
   deleted at start. The private approvals folder is removed when the last tab closes or the session ends.
9. **Codex revisit (the ruling asks to look at it here).** This task changes nothing in how Codex status works. It records, in the PR, a
   short evaluation of the two candidates, with a live check of each on the installed Codex: (a) hooks passed so that they run
   without forging trust, if the CLI offers a supported way; (b) `--dangerously-bypass-hook-trust`, which the help text calls
   dangerous. Studio does not use (b).
10. Skip mode behaves exactly as today, including no allow list and no approvals folder.

## 4. Data model / schema changes

- Setting `agent_permission_mode: "skip" | "ask"` (string) in `settings.json`.
- Files: `<private folder>/tab-<tab id>/<id>.request.json`, `<id>.answer` (`allow`, `deny` or `withdrawn`). Both are Studio-internal.
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

- **Settings (Q5 Option A):** Preferences > "Agent permissions..." opens a small dialog (`RADIUS_FRAME`, `INSET_MODAL`) with two radio rows, "Skip permissions (default)" and "Ask before acting", each with one sentence of explanation (body, `TEXT_SECONDARY`) and a note that already-open tabs keep their mode. The selected radio row uses the neutral fill.
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

- A persistent allow-list ("always allow") of Studio's own. Codex, Grok, Antigravity and Gemini keep theirs. Claude in Ask mode does
  not: it ignores project and local settings (owner ruling 2026-10-24, #1044, #1046), so its own "Yes, and don't ask again" is
  expected not to be kept there (owner-manual check, not yet run; rule 4). Claude in Skip mode keeps it.
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
- Owner ruling 2026-10-24 (the owner chose "Ship the fix" when asked about #1044): Ask tabs of Claude ignore project and local
  settings (`--setting-sources user`), and Claude's own "don't ask again" is expected to no longer persist there (an
  owner-manual check with a logged-in Claude, not yet run). This supersedes §9's "the CLIs keep their own" for Claude in Ask
  mode. Claude's MCP config is a private copy too (#1044, #1046).
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
