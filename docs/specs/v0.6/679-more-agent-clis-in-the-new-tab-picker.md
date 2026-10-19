# #679 Agents panel: offer more agent CLIs in the new-tab picker

| Field | Value |
|---|---|
| Issue | #679 (owner request 2026-10-18; reference: how Orca ADE lists agent CLIs; ruled for v0.6 on 2026-10-19) |
| Size / priority | L / P2 |
| Depends on | #652 (catalog and picker), #673 (probe folders and output-first failure classes), #653 (resume), #656 and #678 (they add per-CLI rows that new CLIs fill in) |
| Unblocks | none |
| Touches | `crates/designos-app/src/agent_cli_catalog.rs` (new descriptors), `crates/designos-app/src/terminal.rs` (hot spot: `AgentCliPreset` and its `match` sites, `provision_session_agent_files` `:857-948`, `configure_pty_env_with_session`), `crates/designos-app/src/agent_start_failure.rs` (per-CLI URLs, login, needles), tests, `docs/agent-cli-usage-sources.md` (from #678) |

Code baseline: `origin/main@01d48f2` (0.5.0). Facts below come from running each CLI on the dev machine on 2026-10-19 (`--help`, `--version`; captures in
`plans/reports/release-060-evidence/cli-help-261019/`); anything not run is marked.

## 1. Summary and user story

As a designer who has other agent CLIs installed (Gemini CLI, OpenCode), I find them in the new-tab picker, detected from my machine, and each
starts connected to Studio: its design tools registered, its status hooks loaded where it supports them, and the `designos-studio` skill reachable.
A CLI I do not have shows an install hint, never a broken tab.

## 2. Current state (FACT)

- Four CLIs are wired: `AgentCliPreset::{Claude, Codex, Antigravity, Grok}` plus `Shell` and `DesignOsHelp` (`terminal.rs:77-85`). Each has a launch
  command (`cli_launch_command`, `:155-180`), a one-shot command (`build_real_cli_command`, `:286-326`), a title, badge and binary name
  (`:101-151`), an install URL and login command (`agent_start_failure.rs:46-65`), and session files provisioned for it:
  `mcp-config.json` (Claude `--mcp-config`), a Grok project config `.grok/config.toml` (`write_grok_session_config`, `:2157-2182`), an agy plugin
  `.agents/plugins/designos/` (`write_agy_session_plugin`, `:2133-2138`), Codex `-c mcp_servers.designos...` overrides (`codex_mcp_overrides!`,
  `:63-73`), hook files per CLI (`:906-915`), and the skill copies plus `AGENTS.md` in the session folder (`:917-947`).
- The contract those fixes established (#642 agy, #646 Grok, #647 hooks, #648 skill reachability, #649 Codex env, #650 failure panel): the CLI starts
  with Studio's `designos-mcp` registered per session without touching the user's global config, its hooks are loaded where supported, the skill is
  found from the session cwd, and a test pins each launch string to the flags the CLI's own `--help` lists.
- CLIs on this machine that are not wired: **Gemini CLI 0.35.3** (`/opt/homebrew/bin/gemini`, an npm link into `@google/gemini-cli`) and **OpenCode 1.0.122**
  (`~/.opencode/bin/opencode`). Not found by `which`: `cursor-agent`, `copilot`. Orca's list (audit §6, third-party summaries) names Claude Code, Codex, Cursor CLI,
  Gemini, Copilot, OpenCode, Pi and custom CLIs; those not installed here cannot be verified on this machine.
- Verified locally for **Gemini CLI**: `gemini [query..]` starts the TUI; `-i/--prompt-interactive`; `-y/--yolo`; `--approval-mode default|auto_edit|yolo|plan`
  (`default` prompts); `-r/--resume latest|<index>`; `--list-sessions`; `--include-directories`; `-m`; `gemini mcp add [-s user|project] [-t stdio|sse|http]
  [-e KEY=value] [--trust] <name> <commandOrUrl> [args...]` with **`project` the default scope**; `gemini hooks migrate` ("Migrate hooks from Claude Code
  to Gemini CLI"); `gemini skills`; `gemini extensions`. Third-party mirrors (not geminicli.com, which returned 404) describe a project file
  `.gemini/settings.json` with an `mcpServers` object and a default context file `GEMINI.md`.
- Verified locally for **OpenCode**: `opencode [project]` starts the TUI in a path; `-c/--continue`, `-s/--session <id>`, `-m provider/model`, `--agent`, `-p/--prompt`;
  commands `run`, `serve`, `acp`, `export [sessionID]`, `import`, `stats` ("show token usage and cost statistics"), `auth`, `agent`. Its MCP configuration path,
  hooks and skills were **not** read.
- The picker and catalog from #652 list the four CLIs only; install state is probed per descriptor.

## 3. Behaviour rules

1. **Catalog entries, not code forks.** Each additional CLI is one descriptor (#652) with: binary and install folders, version command, optional sign-in
   command, install URL, launch arguments for `Skip` and `Ask` permission modes (#656), MCP registration method, hooks support, skill discovery path, resume
   arguments (#653), usage source (#678), and failure-text needles (#673). A descriptor without a verified row for a field leaves that feature off for the CLI (rule 5),
   and the document says so.
2. **Detected, not assumed.** A CLI appears as available only when the probe finds its binary; otherwise it is listed greyed with its install hint, like the four
   existing ones. No CLI is shown as available because it is in the catalog.
3. **Admission checklist.** A CLI is added to the catalog only after each item below is shown on a machine that has it, with the evidence in the PR:
   1. it is installed and `--version` runs;
   2. every flag Studio passes appears in its own `--help` (a pin test, as for #642 and #646: the test fails if a rejected flag comes back);
   3. Studio's MCP server is registered for that session only: the CLI lists the `designos_*` tools in a live run (recorded);
   4. the status path: either its hooks load and `agent-status.json` goes working then done in a recorded run, or the PR states "hooks not supported: heuristic
      status" (as for Codex);
   5. the `designos-studio` skill (or `AGENTS.md`-style context) is found from the session folder in a fresh tab;
   6. resume works in the stable session folder (#653);
   7. its start failures classify (#673): unknown flag and signed-out outputs are recognised from the CLI's real messages.
4. **Gemini CLI (first candidate).** Plan, to be proven by the checklist; nothing below is stated as fact about Gemini's behaviour until its live check passes. Start `gemini` with cwd = the session folder. Registering the
   `designos` server uses only flags listed by `gemini mcp add --help` (captured in `cli-help-261019/gemini-mcp-add-help.txt`: `-s, --scope <user|project>` with `project` the default, `-t/--transport`, `-e/--env`,
   `--trust`, positionals `<name> <commandOrUrl> [args...]`), for example `gemini mcp add -s project -e KEY=value designos <designos-mcp> <args...>`; the `--workspace <dir>` argument belongs to `designos-mcp`, not to
   `gemini`, and goes in the trailing `[args...]`; how Gemini parses a dash-prefixed argument there (a `--` separator may be needed) is **unverified** and is settled by running the command in a scratch folder. Whether the
   result is the project file `<session folder>/.gemini/settings.json` with an `mcpServers` object is known only from third-party mirrors; Studio writes that file itself only after a live run shows Gemini reads that path, otherwise
   it runs the `gemini mcp add` command in the session folder before the tab starts. Permissions: `Skip` -> `--approval-mode yolo` (or `-y`), `Ask` -> `--approval-mode default` (both listed in `gemini --help`). Resume
   `--resume latest` (cwd scoping not verified: a live check, as for Codex and agy in #653). Sign-in command and usage source: none found. Hooks: `gemini hooks` exists but its event names and file format were not read, so status is
   heuristic until checklist item 4 is shown.
5. **OpenCode (second candidate).** Plan: start `opencode <session folder>`; resume `--continue` or `--session <id>`; usage from `opencode stats` (verify per #678's rules
   before showing anything); MCP registration, hooks and skills are unknown: no feature is claimed until the checklist shows it, and until then OpenCode appears as
   "Plain CLI: Studio tools not connected" if the owner wants it listed at all (open question 1).
6. **Names and badges.** Tab title, short badge (`GEMINI`, `OPENCODE`) and a text mark in the picker; no third-party logos are added.
7. **No Shell fallback.** A CLI that is not installed never produces a tab; choosing it opens its install page (existing `FixAction::OpenInstallPage`).
8. **Custom CLIs** (#652 "Other CLI...") remain the way to run anything not in the catalog; they get no MCP registration unless promoted to a descriptor through the checklist.

## 4. Data model / schema changes

- `AgentCliPreset` gains `Gemini` and `OpenCode` (the enum is matched exhaustively, so every `match` site is found by the compiler); `CliDescriptor` fields as in rule 1.
- Session folder files per CLI as the checklist proves (for example `.gemini/settings.json` in the session folder for Gemini).

## 5. Contracts

- Per-CLI launch strings are pinned by tests against the captured `--help` flag sets (extended with `gemini-help.txt` and `opencode-help.txt`, already in the
  evidence folder).
- The `designos-mcp` command line and environment are the same as for the other tabs (`DESIGNOS_*` variables, `--workspace`).

## 6. File-format mapping

None (document formats are untouched). Agent-side config files are written only inside the session folder.

## 7. UI

No new components: rows in the #652 picker and entries in the `+` menu, tab titles and badges. Uninstalled rows are greyed with the install link. Desktop captures
at 1440 for the picker with Gemini installed and one missing; responsive SKIPPED (owner default). Load es-designer before UI code; update the Figma frames from the captures first.

## 8. Edge cases

1. Gemini installed through a different package manager, with the binary outside the known folders: shows "Not installed" (known limit of the probe, #673).
2. Gemini prompts to trust the session folder before it loads project settings (a Grok-like trust prompt): unverified; if it does, the launch sets whatever the CLI documents
   (an environment variable or flag) after it is found in its own docs, as `GROK_FOLDER_TRUST=0` is for Grok; otherwise the checklist item 3 fails and the CLI stays unlisted.
3. Two CLIs with the same binary name on `PATH`: the first one found wins; the version shown tells the user.
4. A CLI update changes a flag: the pin test fails on the next run against fresh `--help`, not silently in the field.
5. A descriptor missing a verified field (no hooks): status falls back to the heuristic path, like Codex.

## 9. Non-goals

- Cursor CLI, Copilot CLI and Pi until a machine with them verifies the checklist.
- Bundling or installing CLIs.
- Third-party logos.
- A generic plugin system for unknown CLIs.

## 10. Test plan (red first)

Seams: launch-string pin tests against the captured help files; catalog unit tests with the fake command runner of #652; stub CLIs for tab launches.

| Case | Expected (fails before) |
|---|---|
| `gemini_launch_uses_only_flags_its_help_lists` | every flag in the generated string appears in `gemini-help.txt` (and `gemini mcp add` flags in its own help capture) |
| `opencode_launch_uses_only_flags_its_help_lists` | same against `opencode-help.txt` |
| `available_only_when_the_probe_finds_the_binary` | catalog with no binaries: all rows greyed, none selectable |
| `uninstalled_cli_opens_its_install_page_not_a_tab` | click: install action, `tabs` unchanged |
| `gemini_project_settings_written_in_the_session_folder_only` | after provisioning, the user's home settings path is untouched (temp `HOME`) |
| `failure_classification_for_the_new_clis` | the real unknown-flag and signed-out messages captured from each CLI classify (#673 needles extended) |
| `permission_flags_per_mode` | `Skip` and `Ask` strings for each CLI match the table in rule 4 / 5 |
| `gemini_live_check` (needs Gemini) | a recorded run: the tab lists the `designos_*` tools, an edit reaches `doc-patch.jsonl` |
| `opencode_live_check` (needs OpenCode) | whatever the checklist proves; unproven items are absent from the descriptor |

Run: `cargo test -p designos-app`. Evidence: red/green capture, the live-run recordings per admitted CLI, the picker capture.

## 11. Performance budgets

- Probing adds at most 3 subprocess calls per CLI per picker open (install, version, sign-in where it exists), off the UI thread, with the #652 timeouts.
- No per-frame work; no timers.

## 12. Dependencies and build order

After #652 and #673; per CLI, the descriptor PR can ship independently once its checklist is complete. `terminal.rs` is a hot spot: adding enum variants touches its
`match` sites in one mechanical commit; descriptors live in the catalog file.

## 13. Owner rulings

- The picker lists the installed agent CLIs beyond Claude, Codex, Grok and Antigravity (for example Gemini CLI and other CLIs with an MCP config path), detected on the machine,
  not hard-coded as available. Each starts with Studio's MCP server, session hooks where it supports them and the designos-studio skill, the same contract as #642 and #646-#650,
  with a launch test that fails if a flag it rejects comes back. A CLI that is not installed shows an install hint instead of a broken tab (#650, #673). (Issue text.)
- Skip-permissions stays the default; ask mode is optional (#656).

## Open questions for the owner

1. **Which CLIs join in v0.6.**
   - A (recommended): Gemini CLI and OpenCode, the two installed on the dev machine, each only as far as the checklist proves (OpenCode possibly as "plain CLI" at first).
   - B: Gemini CLI only; the second one waits until its MCP path is proven.
   - C: all of Orca's list, which cannot be verified here for Cursor CLI, Copilot CLI and Pi.
2. **A CLI whose MCP registration is not proven.**
   - A (recommended): do not list it as a Studio agent; the user can still run it through "Other CLI...".
   - B: list it with the note "Studio tools not connected" so it is discoverable.
3. **Brand marks.**
   - A (recommended): text badges only (as `GROK` and `AGY` today).
   - B: each CLI's logo, which needs the owner's decision on trademark use.

## Could not ground

- Gemini CLI: the official hooks, skills and project-settings documentation (geminicli.com returned 404; only third-party mirrors were found), its trust behaviour for a new
  folder, whether `--resume latest` is scoped to the working directory, and any sign-in status command.
- OpenCode: its MCP configuration path, hooks and skills; its resume scoping; install page URL.
- Cursor CLI, Copilot CLI and Pi: not installed on the dev machine.
