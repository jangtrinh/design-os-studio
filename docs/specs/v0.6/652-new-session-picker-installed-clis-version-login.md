# #652 New-session picker: installed CLIs first, with version and login state

| Field | Value |
|---|---|
| Issue | #652 (agent terminal UX list, 2026-10-19; audit experience 2) |
| Size / priority | M / P1 |
| Depends on | #673 (one install-folder list and output-first classification), #653 (per-file last choice) |
| Unblocks | #651 (the empty state shows this picker), #679 (more CLIs appear as rows), #656 (permission caption) |
| Touches | new `crates/designos-app/src/agent_cli_catalog.rs` (descriptors, probes), new `crates/designos-app/src/agent_picker.rs` (UI), `crates/designos-app/src/terminal.rs` (hot spot: `render_runtime_dropdown_menu` `:3106-3169`, `AgentCliPreset`, `is_installed`), tests |

Code baseline: `origin/main@01d48f2` (0.5.0). CLI facts below come from running each CLI on the dev machine on
2026-10-19 (`--help`, `--version`; captures in `plans/reports/release-060-evidence/cli-help-261019/`).

## 1. Summary and user story

As a designer starting an agent for a file, I pick from the agent CLIs that are really installed on this
machine, each showing its version and whether I am signed in. CLIs I do not have are greyed out with a way
to install them. The picker remembers my last choice for this file, and "Other CLI..." lets me add a command.

## 2. Current state (FACT)

- The `+` menu lists Claude, Codex, Grok, Antigravity and Shell with fixed flag captions under a header
  "LAUNCH CLI (SKIP PERMISSIONS)" (`render_runtime_dropdown_menu`, `terminal.rs:3106-3169`;
  `dropdown_entry`, `:183-191`). It never checks install state, so a missing CLI becomes a tab that exits
  (the failure panel then explains, `agent_start_failure.rs`).
- Install detection exists but is only used by `dispatch_figma_ai_prompt` and the failure panel:
  `AgentCliPreset::is_installed` (`terminal.rs:193-198`) over `runtime_binary_available` (`:202-240`), which does
  not see `~/.grok/bin` (see #673).
- Version and sign-in commands, as run on the dev machine (versions and timings measured with a plain
  subprocess call):

| CLI | Version command (output) | Time | Sign-in command | Time |
|---|---|---|---|---|
| Claude Code | `claude --version` -> `2.1.302 (Claude Code)` | 10 ms | `claude auth status` -> JSON with a boolean `loggedIn` (it also prints account details, which Studio must not store or show) | 118 ms |
| Codex | `codex --version` -> `codex-cli 0.163.0` | 32 ms | `codex login status` -> text "Logged in using ChatGPT", exit 0 when signed in (exit code when signed out: not verified) | 44 ms |
| Grok | `grok --version` -> `grok 1.0.62 (1474f9cbdd91) [stable]` | 8 ms | none: `grok --help` lists `login` and `logout` only | n/a |
| Antigravity | `agy --version` -> `1.4.2` | 47 ms | none: `agy --help` lists no auth subcommand (the failure panel already notes "`agy` has no login subcommand", `agent_start_failure.rs:61`) | n/a |

- The failure panel's install links and login commands exist per CLI (`install_url`, `login_command`,
  `agent_start_failure.rs:46-65`).
- Per-file state for "last choice" has no home yet; #653 adds the registry.

## 3. Behaviour rules

1. **Catalog.** `agent_cli_catalog.rs` holds one descriptor per CLI: preset, binary name, install folders (#673),
   version command, sign-in command (optional), install URL, login command. #679 adds descriptors; this
   task ships the four existing CLIs plus the Shell entry.
2. **Probe.** Opening the picker (or the `+` menu) starts one background probe per descriptor on a worker thread,
   never on the UI thread: install check (binary found in `install_folders`), then, if found, the version
   command and the sign-in command, each with a 3 s timeout. Results are cached in memory for the app run
   and refreshed on each open (stale values shown at once, replaced when the probe returns). Version =
   the first dotted number in the output (`2.1.302`, `0.163.0`, `1.0.62`, `1.4.2`).
3. **Row states.** Each CLI row shows: name, version, and one sign-in state:
   - Claude: `Signed in` / `Not signed in` from the boolean `loggedIn` of `claude auth status` (a field of its documented JSON output, seen with `true`; the `false` case has not been captured, so a missing or non-boolean
     field is `Sign-in not checked`, never a guess);
   - Codex: `Signed in` only when `codex login status` prints a line starting `Logged in` (captured: "Logged in using ChatGPT") and exits 0. The signed-out output and exit code have **not been captured**, so every
     other result is `Sign-in not checked` and the Codex `Not signed in` label is added only after a live check records the signed-out text; that live check gates the Codex sign-in part of this task (a signed-out run must be captured
     on a test account; an agent never logs out the owner's account);
   - `Sign-in not checked` for Grok and Antigravity, which have no status command (owner ruling 2026-10-19);
   - `Checking...` while a probe runs;
   - `Not installed` for a missing binary: the row is greyed (text `TEXT_DISABLED`), not selectable, and shows
     an "Install" link that opens the CLI's install page (`FixAction::OpenInstallPage`).
4. **Order.** Installed CLIs first, in the fixed order Claude, Codex, Grok, Antigravity, with the file's last
   choice first when there is one; uninstalled CLIs after them, greyed; then saved custom entries, "Other CLI...", and the Shell row last.
5. **Choosing a CLI** starts a new session tab with that CLI's launch command (#653), records the choice as
   the file's last choice, and registers the session. A `Not signed in` row is still selectable; the CLI then
   shows its own sign-in screen, and the existing failure panel offers "Run login" if it exits.
6. **Remembers the last choice per file**: `last_cli` in the file's registry (#653), used only for the order in
   rule 4 and for highlighting; it never starts anything by itself.
7. **Other CLI...** opens an inline form: a command line (one text field) and an optional label. On confirm the
   command is saved as a custom entry **for all files** (a Studio setting, `custom_clis` in `settings.json`, the store of `comments/settings.rs`) and started in a tab. Custom entries run in the file's session folder with the
   `DESIGNOS_*` environment, like every tab, and are listed in the picker afterwards. Studio does not know a custom CLI's MCP or hook configuration, so its `designos_*` tools are not registered for it, and its row
   says "Plain command: Studio tools not connected" (owner ruling 2026-10-19). A custom entry is never registered as a resumable session (#653 rule 4).
8. **Shell** is a row at the end of the list (owner ruling) and is never the default.
9. **Permissions caption (proposal).** The picker's footer states the current permission mode (`Skip permissions` by default;
   `Ask` when #656's setting is on) so the old header text "LAUNCH CLI (SKIP PERMISSIONS)" is not lost. The rows
   no longer show raw flags.
10. Probes never run a CLI's login flow and never send a prompt; they cost no tokens.
11. The same list is used by the empty-state of #651 and by the `+` menu, which shows it in compact form so versions, sign-in state and greyed rows are visible everywhere (owner ruling).

## 4. Data model / schema changes

- `CliDescriptor` and `CliProbe { installed, version: Option<String>, signed_in: SignIn }` in memory;
  `SignIn::{Yes, No, Unknown, Checking}`.
- Per file: `last_cli: Option<String>` in `sessions.json` (#653). Custom entries: `custom_clis: Vec<{label, command}>`
  in Studio's `settings.json` (all files).

## 5. Contracts

No MCP change. The Claude and Codex status calls are read-only; their output is parsed for the sign-in boolean
only and never logged or persisted (no account names or emails).

## 6. File-format mapping

None.

## 7. UI

Described in words. Checked against `design/principles.md` (P1 to P10) and `design/soul.md`; the rules used, with the `theme.rs` constants, are recapped in README-part3 under "UI rules checked for part 3". In short: every colour, gap, inset, control height and radius below is a `crates/designos-app/src/theme.rs` constant (the panel's own `TERM_*` / `STATUS_*` constants at `terminal.rs:29-55` stay for the terminal viewport and the existing tab status dots and are not extended); selected and active rows use the neutral fill `BG_SELECTED`; the brand orange is the view's one primary action; counts sit in their own pill, not in a label.

The issue fixes the content (rules 3 to 8); the layout below is a proposal for the owner's confirmation:

- A list of rows `CONTROL_ROW` high inside the panel's viewport area (or a `RADIUS_FRAME` popover with `INSET_POPOVER` when opened from `+`). Each row: a vector mark of the CLI (no emoji, no logo), the name in body-medium, the version in `font_mono` caption `TEXT_SECONDARY` after it (`SPACE_INLINE` apart), and the sign-in state as a status pill at the right.
- The sign-in state is a static status pill with the geometry of the existing count pill (`panels/check_designs.rs:186-196`: `BG_TERTIARY` fill, `radius` from the pill height, `font_caption_medium`, `TEXT`); "Signed in" and "Not signed in" differ by label and a leading dot, not by a new colour. If the owner wants colour for them, a token must first be added to `theme.rs` and `design/design.tokens.json` (P1); this spec adds none.
- Uninstalled rows use `TEXT_DISABLED` and show a text button "Install" (`CONTROL_DENSE` high, `TARGET_MIN` hit area) aligned right; the row itself is not a pointer target.
- The file's last chosen CLI shows the neutral `BG_SELECTED` fill. A `BORDER` hairline separates installed from uninstalled and "Other CLI..." and the footer line; groups are at least twice their inner gap apart (P9).
- Keyboard: arrow keys move, Enter starts; every control meets `TARGET_MIN`.
- Desktop captures at 1440 for: all installed, one missing, probing, and the "Other CLI..." form. Responsive SKIPPED (owner default). Load es-designer before UI code; update the Figma frames from the captures first.

## 8. Edge cases

1. A CLI installed in a folder outside the known list (a custom `npm` prefix): shows `Not installed` though it
   runs in the user's shell; "Other CLI..." with its path is the way in. Known limit (#673 non-goal).
2. A probe hangs: the 3 s timeout ends it; the row shows `Sign-in not checked` and the version (if it came back).
3. A CLI updated while the app runs: the next open refreshes.
4. Sign-in command prints a non-JSON line (Claude format change): `Sign-in not checked`, never a guess.
5. Two windows probing at once: independent; no shared state beyond the cache per process.
6. Studio offline: Claude and Codex status commands are local; if a CLI needs the network for status, a timeout
   gives the row `Sign-in not checked`.
7. The registry is absent (file without a key, #653): last choice is held in memory for the run.

## 9. Non-goals

- Installing CLIs for the user, or running login flows from the picker.
- Model selection per CLI (#627 owns it inside the variant explorer).
- Showing usage (#678) or permissions (#656) per row.

## 10. Test plan (red first)

Seams: `agent_cli_catalog` unit tests with a fake command runner (trait `RunCommand`), and one UI test through
the `Sim` harness.

| Case | Expected (fails before) |
|---|---|
| `version_parsing_for_the_four_real_outputs` | the four captured version strings parse to `2.1.302`, `0.163.0`, `1.0.62`, `1.4.2` |
| `installed_cli_row_shows_version_and_state` | fake runner: Claude status JSON `loggedIn: true` -> `Signed in` |
| `claude_signed_out_json` | Claude `{"loggedIn": false}` -> `Not signed in`; a missing field -> `Sign-in not checked` |
| `codex_only_the_captured_signed_in_phrase_counts` | `Logged in using ChatGPT` exit 0 -> `Signed in`; any other output or exit -> `Sign-in not checked` (the signed-out phrase is added with its live capture) |
| `cli_without_status_command_is_not_checked` | Grok and agy rows are `Unknown` (shown as "Sign-in not checked") |
| `shell_row_is_last_and_other_cli_row_says_studio_tools_not_connected` | order and wording |
| `plus_menu_shows_the_same_list_compactly` | the `+` menu rows equal the picker's rows (name, version, state) |
| `custom_cli_is_saved_for_all_files` | add in file A; listed in file B; not in any registry |
| `missing_binary_is_greyed_with_an_install_link` | `Not installed`, not selectable, link action present |
| `probe_runs_off_the_ui_thread` | the picker's first frame returns before the fake runner's blocking call finishes |
| `probe_timeout_degrades_to_unknown` | fake runner sleeps past 3 s -> `Unknown` |
| `last_choice_orders_first_and_is_per_file` | file A chose Codex, file B none: order differs |
| `other_cli_form_saves_and_lists_the_entry` | confirm -> entry listed and a tab started in the session folder with `DESIGNOS_*` set |
| `picker_reflects_this_machine` (owner-manual) | on the dev machine the rows equal `command -v` and `--version` of each CLI |

Run: `cargo test -p designos-app agent_cli_catalog` and `agent_picker`. Evidence: red/green capture plus the four
1440 captures.

## 11. Performance budgets

- UI frame: the picker only reads cached results; no process spawn or file read per frame.
- Probe: at most 8 short subprocess calls per open (about 0.3 s total on the dev machine from the measured times
  above), all off-thread, never concurrent with a tab spawn.
- Idle: nothing runs while the picker is closed.

## 12. Dependencies and build order

After #673 (install folders) and with #651. #679 extends the catalog. `terminal.rs` is a hot spot: the picker is a
new file; `terminal.rs` loses `render_runtime_dropdown_menu` to it (a move, not a rewrite).

## 13. Owner rulings

- Orca-style picker with installed CLIs first, version and login state, uninstalled greyed with an install hint,
  last choice per file, "Other CLI..." for a custom command (issue text).
- Skip-permissions stays the default; `GROK_FOLDER_TRUST=0` stays.
- The picker reflects real install and login state; no guessing.
- Owner rulings 2026-10-19 (`plans/reports/release-060-evidence/owner-rulings-v06.md`): Grok and Antigravity show "Sign-in not checked"; "Other CLI..." is saved for all files with the "Studio tools not connected" warning; the `+` menu shows the same list in compact form; a Shell row comes at the end.

## Open questions for the owner

None. The four questions of the first draft were answered by the owner on 2026-10-19 and are rules 3, 7, 8 and 11. One live check remains before the Codex sign-in label is Ready (rule 3).
