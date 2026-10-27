# Completion Report — `release: 0.6.3`: release notes and workspace version bump on `release/v0.6.3`

- **Branch:** `jangtrinh/agy-r063-release-notes`
- **Base:** `origin/release/v0.6.3` (`eac72061a6dc2ef891a772fd00f184c4b2498353`)
- **Date:** 2026-10-26
- **Evidence directory:** `plans/reports/agy-r063-release-notes-evidence/`

---

## 1. What could NOT be verified

1. **PR #1201 (`#1192`, Home › Import on Linux) has not merged into `origin/release/v0.6.3` yet.**
   - At completion time (`git fetch origin && gh pr view 1201 --json state,mergedAt,isDraft`), PR #1201 (`jangtrinh/agy-1192-linux-import` → `release/v0.6.3`) is `OPEN` (`isDraft: true`, `mergedAt: null`), and `origin/release/v0.6.3` is at `eac72061a` (`v0.6.2` + `#1085` + `#1117`).
   - **Code comparison required by Ruling 1 & Ruling 4:**
     - On current `origin/release/v0.6.3` (`eac72061a`, before PR #1201 merges), `crates/designos-app/src/io/dialog_probe.rs` uses the 0.6.1 heuristic (`< 300 ms` elapsed check plus static filesystem checks for `.portal` files and `zenity` on `PATH`) and `crates/designos-app/src/io/file_dialog.rs` logs `designos: warn: file dialog failed or is unavailable ({reason})` and shows the toast `No file dialog. Install zenity or xdg-desktop-portal.` without an `Enter a path…` button.
     - In PR #1201 (`origin/jangtrinh/agy-1192-linux-import`, `356290bf6`), `crates/designos-app/src/io/dialog_probe.rs` checks on the session D-Bus (within 1.5 s, allowing D-Bus auto-start so an installed-but-stopped portal starts on first use) whether `org.freedesktop.portal.Desktop` provides `org.freedesktop.portal.FileChooser`, or whether an executable `zenity` is on `PATH` when the portal check fails without timing out. When no file dialog can open, `crates/designos-app/src/io/file_dialog.rs` logs `designos: warn: file dialog unavailable: {reason}` on stderr, shows the toast `No file dialog. Install zenity or xdg-desktop-portal.`, and on Home › `Import` (`crates/designos-app/src/ui/shell.rs`) offers `Enter a path…`, which opens the `Import file` prompt (`Path to a .fig, .designos, .json or .svg file`).
     - Per Ruling 1 and Ruling 4, `CHANGELOG.md` and `README.md` describe PR `#1201`'s exact behaviour, and the four `#1201`-specific backticked spans (`Enter a path…`, `designos: warn: file dialog unavailable:`, `org.freedesktop.portal.Desktop`, `org.freedesktop.portal.FileChooser`) are listed in the backtick-span table as composed on `eac72061a` with their exact `file:line` locations on `origin/jangtrinh/agy-1192-linux-import`. When PR #1201 merges into `release/v0.6.3`, the controller will notify this worker (or merge `origin/release/v0.6.3` into `jangtrinh/agy-r063-release-notes`).
2. **Live model turns and non-macOS interactive desktop sessions were not run.**
   - Live Claude, Gemini CLI, Grok, and Antigravity sessions, as well as interactive Linux and Windows desktop sessions, were not executed during this release-notes and version-bump task; those known limitations remain explicitly listed in `### Known issues` of `## [0.6.3]`.

---

## 2. Conclusion, Epistemic Ledger, Weakest Link & Skills Loaded

### Conclusion
`CHANGELOG.md` and `README.md` now document `0.6.3` (`#1085` for `#1072`/`#1050`, `#1117` for `#1003` RT-04/RT-07 and `#1101`, and `#1201` for `#1192`, plus updated `### Known issues` including the `#1158` Windows test suite advisory note and removal of fixed `#1072` and `#1003` rows). The workspace version in `Cargo.toml` and `Cargo.lock` is bumped to `0.6.3` in its own commit (`release: 0.6.3`), verified via `cargo metadata`, `cargo build -p designos-diff --locked`, `designos-diff --version`, and `cargo fmt --all -- --check`.

### Epistemic Ledger
- `FACT`: `git log --oneline v0.6.2..origin/release/v0.6.3` contains `e7d03e332` (`#1085`) and `eac72061a` (`#1117`). PR `#1201` (`#1192`) is open at `origin/jangtrinh/agy-1192-linux-import` (`356290bf6`).
- `FACT`: Every GitHub issue referenced in `0.6.2`'s `### Known issues` (`#1072`, `#1003`, `#1097`, `#1074`, `#1066`, `#1048`, `#1047`, `#1059`, `#1101`, `#930`, `#1096`, `#1073`, `#1102`) and `README.md`'s `### Known issues` (`#834`, `#977`, `#1158`) was checked with `gh issue view`; `#1072` and `#1003` RT-04/RT-07 are fixed on `release/v0.6.3` by `#1085` and `#1117` and dropped from `0.6.3`'s `### Known issues`, while all remaining open issues are retained and `#1158` is added.
- `FACT`: All 60 unique backticked spans in `## [0.6.3]` of `CHANGELOG.md` were checked with `git grep -n -F -e`: 52 are found verbatim on `HEAD` (`eac72061a`), 4 are found verbatim in PR `#1201` (`origin/jangtrinh/agy-1192-linux-import`), and 4 are composed CLI/error examples whose literal templates exist in `crates/`.
- `FACT`: `cargo metadata --format-version 1 --no-deps --locked` reports version `{'0.6.3'}` across all 13 workspace crates, and `designos-diff --version` built under the isolated harness prints `designos-diff 0.6.3`.
- `INFERENCE`: Once PR `#1201` merges into `release/v0.6.3` and `origin/release/v0.6.3` is merged into `jangtrinh/agy-r063-release-notes`, all 4 PR `#1201` backticked spans will also match directly on `HEAD`.
- `ASSUMPTION`: None.

### Weakest Link
PR `#1201` (`#1192`) is still open (`isDraft: true`) against `release/v0.6.3`; if `#1201`'s user-facing strings change before it merges into `release/v0.6.3`, the `#1192` bullet in `CHANGELOG.md` and `README.md` must be updated when merging `origin/release/v0.6.3` per Ruling 4.

### Skills Loaded
- `es:dev` (`/Users/jangtrinh/.gemini/config/skills/es-dev/SKILL.md`)
- `es:feature-dev` (`/Users/jangtrinh/.gemini/config/skills/es-feature-dev/SKILL.md`)
- `es:rust` (`/Users/jangtrinh/.gemini/config/skills/es-rust/SKILL.md`)
- `es:rust-gpu-canvas` (`/Users/jangtrinh/.gemini/config/skills/es-rust-gpu-canvas/SKILL.md`)
- `es-astra-work` (`/Users/jangtrinh/.gemini/config/skills/es-astra-work/SKILL.md`)
- `es:fable-thinking` (`/Users/jangtrinh/.gemini/config/skills/es-fable-thinking/SKILL.md`)

---

## 3. Acceptance Gates & Verbatim Outputs

| # | Gate | Command | Result |
|---|---|---|---|
| 1 | Allowed files only (`origin/release/v0.6.3..HEAD`) | `git diff --stat origin/release/v0.6.3..HEAD` | **PASS** — Only `CHANGELOG.md`, `README.md`, `Cargo.toml`, `Cargo.lock`, plus this report and evidence in Commit 3 |
| 1b | Bump commit isolation (`release: 0.6.3`) | `git diff --stat 0414e846..4c749e3b` (`git diff --stat HEAD~1` on Commit 2) | **PASS** — Only `Cargo.toml` and `Cargo.lock` |
| 2 | Backtick-span verification (`## [0.6.3]`) | `git grep -n -F -e <span> -- crates/ docs/specs/ docs/releasing.md skills/` | **PASS** — 60 unique spans: 52 found verbatim on `HEAD`, 4 found verbatim in PR `#1201` (composed on `eac72061a`), 4 composed examples |
| 3 | Workspace crate versions | `cargo metadata --format-version 1 --no-deps --locked \| python3 -c 'import json,sys;print({p["version"] for p in json.load(sys.stdin)["packages"]})'` | **PASS** — Prints `{'0.6.3'}` |
| 4a | Isolated harness build | `cargo build -p designos-diff --locked` (under isolated `HOME`/`TMPDIR`) | **PASS** — Exit `0` |
| 4b | Binary version check | `"$CARGO_TARGET_DIR/debug/designos-diff" --version` | **PASS** — Prints `designos-diff 0.6.3` |
| 4c | Rustfmt check | `cargo fmt --all -- --check` | **PASS** — Exit `0` |
| 4d | Temporary directory cleanup | `rm -rf "$PWD/out" "$S" && test ! -e "$PWD/out"` | **PASS** — `out/` and `$S` removed |

### 3.1 `git diff --stat` Outputs

Before Commit 3 (code + docs commits `0414e846` and `4c749e3b`):
```text
$ git diff --stat origin/release/v0.6.3..4c749e3b
 CHANGELOG.md | 34 ++++++++++++++++++++++++++++++++++
 Cargo.lock   | 26 +++++++++++++-------------
 Cargo.toml   |  2 +-
 README.md    | 10 +++++++++-
 4 files changed, 57 insertions(+), 15 deletions(-)
```

On the `release: 0.6.3` bump commit (`4c749e3b`):
```text
$ git diff --stat 0414e846..4c749e3b
 Cargo.lock | 26 +++++++++++++-------------
 Cargo.toml |  2 +-
 2 files changed, 14 insertions(+), 14 deletions(-)
```

### 3.2 Backtick-Span Table for `## [0.6.3]` (`CHANGELOG.md`)

All 60 unique backticked spans in `## [0.6.3]` of `CHANGELOG.md` (raw output saved in `plans/reports/agy-r063-release-notes-evidence/backtick-spans-grep.txt`):

| # | Backticked span | Classification | `git grep -n -F` match or composition source |
|---|---|---|---|
| 1 | `$TMPDIR` | Found | `crates/designos-app/src/agent_approval_channel.rs:6` |
| 2 | `--apply-spec` | Found | `crates/designos-app/src/cli.rs:310` |
| 3 | `--screenshot` | Found | `crates/designos-app/src/cli.rs:359` |
| 4 | `.designos` | Found | `crates/designos-app/src/cli.rs:362` |
| 5 | `.designos-set-aside` | Found | `crates/designos-app/src/agent_project_config.rs:7` |
| 6 | `.fig` | Found | `crates/designos-app/src/cli.rs:362` |
| 7 | `.gemini` | Found | `crates/designos-app/src/agent_composer.rs:365` |
| 8 | `.gemini/settings.json` | Found | `crates/designos-app/src/agent_session_files/fs_sites_scan_tests/patterns.rs:53` |
| 9 | `.json` | Found | `crates/designos-app/src/cli.rs:362` |
| 10 | `.spec.json` | Found | `crates/designos-app/src/agent_selection_chip.rs:5` |
| 11 | `.svg` | Found | `crates/designos-app/src/cli.rs:362` |
| 12 | `/tmp` | Found | `crates/designos-app/src/agent_approval_channel.rs:6` |
| 13 | `/var/tmp` | Found | `crates/designos-app/src/agent_approval_channel.rs:46` |
| 14 | `Enter a path…` | Composed on `eac72061a` (Found in PR `#1201`) | `origin/jangtrinh/agy-1192-linux-import:crates/designos-app/src/app.rs:146`, `crates/designos-app/src/ui/shell.rs:1025` (`Enter a path\u{2026}`) |
| 15 | `Import` | Found | `crates/designos-app/src/ui/home/mod.rs:312` |
| 16 | `Needs approval` | Found | `crates/designos-app/src/agent_a11y.rs:222` |
| 17 | `No file dialog. Install zenity or xdg-desktop-portal.` | Found | `crates/designos-app/src/io/file_dialog.rs:14` |
| 18 | `No, suggest changes` | Found | `crates/designos-app/src/gemini_dialog.rs:5` |
| 19 | `NodeSpec` | Found | `crates/designos-doc/src/spec.rs:261` |
| 20 | `Open tab` | Found | `crates/designos-app/src/agent_approval_card.rs:35` |
| 21 | `PATH` | Found | `crates/designos-app/src/io/dialog_probe.rs:95` |
| 22 | `README.md` | Found | `crates/designos-app/src/agent_sample_prompts.rs:488` |
| 23 | `ToolPermission` | Found | `crates/designos-app/src/agent_turn_status.rs:714` |
| 24 | `agent-events.jsonl` | Found | `crates/designos-app/src/agent_hook_outputs.rs:12` |
| 25 | `agent-status.json` | Found | `crates/designos-app/src/agent_hook_outputs.rs:11` |
| 26 | `ask_user` | Found | `crates/designos-app/src/gemini_dialog.rs:16` |
| 27 | `bootstrap-compact-reply.json` | Found | `crates/designos-app/src/agent_hook_outputs.rs:16` |
| 28 | `bootstrap-context-reply.json` | Found | `crates/designos-app/src/agent_hook_outputs.rs:16` |
| 29 | `children must be an array of NodeSpec objects. Pass each child as a JSON object.` | Composed | Composed from `crates/designos-mcp/src/tools/errors.rs:1340` (`"{field_path} must be an array of NodeSpec objects. Pass each child as a JSON object."`) and `crates/designos-app/src/main.rs:653` (`"children must be an array of NodeSpec objects"`) |
| 30 | `claude-transcript.json` | Found | `crates/designos-app/src/agent_hook_outputs.rs:14` |
| 31 | `crates/designos-mcp/rules/workflow.md` | Found | `crates/designos-mcp/src/rules.rs:4` |
| 32 | `designos` | Found | `crates/designos-app/Cargo.toml:2` |
| 33 | `designos FILE --screenshot OUT.png` | Composed | Composed CLI invocation from `crates/designos-app/src/cli.rs:359` (`usage: designos [FILE] [--screenshot OUT.png] ...`) |
| 34 | `designos-mcp` | Found | `crates/designos-app/Cargo.toml:37` |
| 35 | `designos-studio FILE --screenshot OUT.png` | Composed | Composed CLI invocation for the `designos-studio` binary (`crates/designos-app/Cargo.toml:14`) and `crates/designos-app/src/cli.rs:359` |
| 36 | `designos: warn: file dialog unavailable:` | Composed on `eac72061a` (Found in PR `#1201`) | `origin/jangtrinh/agy-1192-linux-import:crates/designos-app/src/io/file_dialog.rs:141` (`format!("designos: warn: file dialog unavailable: {reason}")`) |
| 37 | `designos_apply_batch` | Found | `crates/designos-mcp/generated/rules.md:10` |
| 38 | `designos_apply_spec` | Found | `crates/designos-mcp/generated/rules.md:10` |
| 39 | `designos_specs` | Found | `crates/designos-app/src/agent_session_files/fs_sites_allowlist.txt:21` |
| 40 | `doc-patch.jsonl` | Found | `crates/designos-app/src/agent_hook_outputs.rs:13` |
| 41 | `every name Studio tried was taken. Close this file's other agent tabs, then try again.` | Found | `crates/designos-app/src/agent_session_files/no_follow/set_aside.rs:235` |
| 42 | `exit_plan_mode` | Found | `crates/designos-app/src/gemini_dialog.rs:16` |
| 43 | `hook-out` | Found | `crates/designos-app/src/agent_hook_outputs.rs:10` |
| 44 | `hooks` | Found | `crates/designos-app/src/agent_approval_channel.rs:4` |
| 45 | `is locked, so Studio can't move it. Unlock it, then try again.` | Found | `crates/designos-app/src/agent_session_files/locked_entry_tests.rs:45` |
| 46 | `must be a finite number` | Found | `crates/designos-app/src/main.rs:772` |
| 47 | `org.freedesktop.portal.Desktop` | Composed on `eac72061a` (Found in PR `#1201`) | `origin/jangtrinh/agy-1192-linux-import:crates/designos-app/src/io/dialog_probe.rs:21` (`const PORTAL_BUS_NAME: &str = "org.freedesktop.portal.Desktop";`) |
| 48 | `org.freedesktop.portal.FileChooser` | Composed on `eac72061a` (Found in PR `#1201`) | `origin/jangtrinh/agy-1192-linux-import:crates/designos-app/src/io/dialog_probe.rs:22` (`const PORTAL_FILE_CHOOSER_INTERFACE: &str = "org.freedesktop.portal.FileChooser";`) |
| 49 | `page-context-reply.json` | Found | `crates/designos-app/src/agent_hook_outputs.rs:16` |
| 50 | `permission_prompt` | Found | `crates/designos-app/src/agent_turn_status.rs:132` |
| 51 | `skills/design-os-studio/rules.md` | Found | `crates/designos-mcp/src/bin/gen-tool-docs.rs:243` |
| 52 | `stat -L` | Found | `crates/designos-app/src/agent_turn_marks.rs:152` |
| 53 | `tools.allowed` | Found | `docs/specs/v0.6/656-agents-ask-permission-mode-with-in-panel-approvals.md:135` |
| 54 | `tools.discoveryCommand` | Found | `docs/specs/v0.6/656-agents-ask-permission-mode-with-in-panel-approvals.md:138` |
| 55 | `tty` | Found | `crates/designos-app/src/agent_permissions.rs:209` |
| 56 | `turn-marks.jsonl` | Found | `crates/designos-app/src/agent_hook_outputs.rs:13` |
| 57 | `wc -c <` | Found | `crates/designos-app/src/agent_turn_marks.rs:149` |
| 58 | `workspace-write` | Found | `crates/designos-app/src/agent_approval_channel.rs:6` |
| 59 | `zenity` | Found | `crates/designos-app/src/io/dialog_probe.rs:3` |
| 60 | `{file} could not be opened: {reason}. Check the file, then try again.` | Composed | Composed from `crates/designos-app/src/shell.rs:181` (`format!("{} could not be opened: {reason}. Check the file, then try again.", f.display())`) |

### 3.3 `cargo metadata`, Harness Build, `designos-diff --version` & `cargo fmt` Outputs

```text
$ cargo metadata --format-version 1 --no-deps --locked | python3 -c 'import json,sys;print({p["version"] for p in json.load(sys.stdin)["packages"]})'
{'0.6.3'}

$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo build -p designos-diff --locked
   Compiling designos-diff v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-r063-release-notes/crates/designos-diff)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 20.58s

$ "$CARGO_TARGET_DIR/debug/designos-diff" --version
designos-diff 0.6.3

$ cargo fmt --all -- --check
(exit 0, no output)
```

---

## 4. Changes per File

- `CHANGELOG.md`: Added `## [0.6.3] - 2026-10-26` above `## [0.6.2] - 2026-10-25` with `### Fixed` (`Agents panel` for `#1072`/`#1050` PR `#1085`; `CLI and MCP` for `#1003` RT-04/RT-07 and `#1101` PR `#1117`; `Linux` for `#1192` PR `#1201`) and `### Known issues` (dropped fixed `#1072` and `#1003` bullets, kept open issues, added the `#1158` Windows test suite advisory bullet).
- `README.md`: Updated `The newest is 0.6.2.` to `The newest is 0.6.3.` in `## What's new`, added `<a id="new-in-v063"></a>` and `### 0.6.3` (3 bullets drawn from `CHANGELOG.md`) above `<a id="new-in-v062"></a>`, and added the `#1158` Windows test suite advisory bullet in `### Known issues`.
- `Cargo.toml`: Bumped `[workspace.package]` `version` from `"0.6.2"` to `"0.6.3"`.
- `Cargo.lock`: Updated workspace crate versions from `0.6.2` to `0.6.3` via `cargo update -w --offline`.
- `plans/reports/agy-r063-release-notes.md` & `plans/reports/agy-r063-release-notes-evidence/`: Added this report and raw verification outputs.

---

## 5. Verbatim Release Notes Added

### 5.1 `CHANGELOG.md` — `## [0.6.3] - 2026-10-26`

````markdown
## [0.6.3] - 2026-10-26

Patch for **AI design** ([milestone v0.6.3](https://github.com/jangtrinh/design-os-studio/milestone/49)): Studio keeps every agent hook output in a private folder no agent grant reaches, refuses broken `--screenshot` inputs and writes readable spec errors, and offers `Enter a path…` from Home › Import on Linux when no file dialog can open.

### Fixed
- **Agents panel.**
  - Hook commands that Studio installs for Claude, Grok, Antigravity and Gemini no longer write or read files in the session folder. Hook outputs (`agent-status.json`, `agent-events.jsonl`, `turn-marks.jsonl`, `claude-transcript.json`) and Claude's prompt-hook reply files (`page-context-reply.json`, `bootstrap-context-reply.json`, `bootstrap-compact-reply.json`) now live in a Studio-private `hook-out` folder inside the session's approval channel folder in Studio's data directory (mode 0700), outside the session folder, `$TMPDIR`, `/tmp`, `/var/tmp` and every folder an agent CLI is granted. `doc-patch.jsonl` stays in the session folder, and the turn-mark hook reads its byte size with `stat -L` instead of `wc -c <`, so a link or FIFO planted in the session folder is neither written through nor waited on. With no private data folder, Studio installs no file-writing or file-reading hook, and a hook that cannot write its folder (for example under Gemini's or Grok's own sandbox) exits 0 with nothing on stderr (#1072, #1050, PR #1085).
  - When an Ask-mode Grok or Antigravity tab cannot move a non-Studio entry into `.designos-set-aside`, a locked entry says `is locked, so Studio can't move it. Unlock it, then try again.`, a permission or ACL refusal names both the entry and its folder, and a full `.designos-set-aside` says `every name Studio tried was taken. Close this file's other agent tabs, then try again.` (#1072, PR #1085).
- **CLI and MCP.**
  - `designos FILE --screenshot OUT.png` and `designos-studio FILE --screenshot OUT.png` on a missing, empty or corrupt file now exit with status 1, write no PNG, and print `{file} could not be opened: {reason}. Check the file, then try again.` on stderr instead of rendering a blank Untitled canvas and exiting 0. Valid files are loaded once (#1003 RT-04, PR #1117).
  - `--apply-spec`, `designos_apply_spec` and `designos_apply_batch` refuse invalid `NodeSpec` field values with a plain-words message that names the field path and what it must be (for example `children must be an array of NodeSpec objects. Pass each child as a JSON object.`) instead of serde's internal Rust type names. `--apply-spec` also refuses duplicate JSON keys and non-finite numbers (`must be a finite number`) and strips internal error-code prefixes (#1003 RT-07, PR #1117).
  - `README.md`, `crates/designos-mcp/rules/workflow.md` and `skills/design-os-studio/rules.md` note that Ask covers agent tabs started from Studio's Agents panel, while the `designos` CLI you run yourself and a `designos-mcp` started by another host are not gated by Studio; that host approves its own MCP calls (#1101, PR #1117).
- **Linux.**
  - Home › `Import` on Linux checks whether `org.freedesktop.portal.Desktop` provides `org.freedesktop.portal.FileChooser` on the session D-Bus (starting the portal on first use when installed) or whether an executable `zenity` is on `PATH`. When no file dialog can open, Studio logs `designos: warn: file dialog unavailable:` with the cause on stderr, shows `No file dialog. Install zenity or xdg-desktop-portal.`, and on Home › `Import` offers `Enter a path…` to open a `.fig`, `.designos`, `.json` or `.svg` file by path (#1192, PR #1201).

### Known issues
- Esc on the Other CLI menu or form also reaches Codex and puts it in rewind mode (#1097).
- A document with no path gives `designos-mcp` the whole disk as its workspace (#1074). In Ask mode each `designos-mcp` call asks first.
- A prompt that contains `No, suggest changes` can hold the automatic Enter, and refuse sends, in a Gemini Ask tab until the line scrolls off (#1066).
- Antigravity, Gemini CLI and Grok start MCP servers from copies of the session config that an agent can write, so a server command added there could run outside any sandbox (#1048). It needs an agent that can already write its session folder without asking, such as Codex under your own `workspace-write` setting. Studio checks and rewrites these copies when a tab starts. It does not stop an agent that writes later, and Grok may read an edited MCP config while a session runs [unverified: from Grok's source, not run]. Studio moves aside the sources that Grok's and Antigravity's docs and source name, and a later version of either CLI may read more. Workaround: in a window where an agent can write without asking, do not run a Grok or Antigravity tab in Ask mode. Planned for a later release: run Grok and Antigravity from a folder only that tab can write.
- Gemini CLI in Ask mode reads `tools.allowed` and other settings from `.gemini/settings.json` in the session folder, which an agent can write (#1047). Some keys there skip Gemini's confirmation prompt, and `hooks` and `tools.discoveryCommand` run a command when the next Gemini tab starts. Gemini 0.35 has no switch that ignores workspace settings for one run. Studio rewrites the file when a tab starts, and an Ask-mode Gemini tab refuses to start when the rewrite fails, so a read-only `.gemini` no longer keeps an agent's file. Planned fix: #1059.
- Ask covers agent tabs started from Studio. The `designos` CLI, which you run yourself, and a `designos-mcp` started by another host are not gated by Ask: that host approves its own MCP calls (#1101). A real MCP-level approval gate is planned for a later release.
- Codex, Grok, Antigravity and Gemini CLI ask for approval in their own terminal menu, and the panel shows `Open tab`; Approve and Deny in the panel exist for Claude only (#930). Clicking an option in Codex's own approval menu does nothing, so use the arrow keys and Enter (#1096).
- Gemini CLI and Grok in Ask mode: the status line may not show a waiting state for every approval prompt. For Gemini, the state comes from the confirmation dialog on the tab's screen or from Gemini's `ToolPermission` notification. The `ask_user` and `exit_plan_mode` dialogs draw no `No, suggest changes` option, and a panel narrower than 29 columns hides a cut label, so a wait there starts only from a hook record. The screen check is on for Gemini 0.35.x and 0.65.x, read from source only. For another Gemini version, a No answer leaves `Needs approval` standing until you type a prompt. Where Studio writes no Gemini hooks file (Windows, or Gemini already has system defaults), a wait starts only from the screen. For Grok, the state comes from its `permission_prompt` notification, and Studio never checked that payload against a real Grok [unverified]. No real Gemini or Grok approval prompt was run.
- Export Frame as `.spec.json` writes into `designos_specs` in the temporary folder when Studio finds no source tree, by path, so an agent that can write the temporary folder could plant a link at the file name. It needs you to run the export (#1073).
- The Windows test suite is advisory for 0.6.3. Releases are verified on macOS, and known Windows test failures are tracked in #1158.
- Windows (releases are verified on macOS only):
  - Stop on Windows is not checked by any test. The Stop tests fail there because their Python stub imports `tty`, which Python does not provide on Windows (#1066).
  - Palette auto-fix undo is unverified on Windows: eight tests show different auto-layout sizes after one undo, and the cause is not known (#1066).
  - The link checks for session files are not race-free on Windows (#1061, #1068), an Ask-mode Grok or Antigravity tab refuses an entry that is not Studio's instead of moving it (#1069), and no hooks file is written for Gemini, so its `Needs approval` comes from the screen only (#1058, #1066).
  - Linux and Windows were not built or run by hand for these changes.
- Not checked against a live model turn: the Ask-mode Claude launch (skill from a private plugin, Studio's hooks and MCP config from private copies), the Gemini approval state, and the Ask-mode Grok and Antigravity checks were tested with stubs and source reading, not with a real Claude, Gemini, Grok or Antigravity turn.
- Of the 38 GUI fixes from 0.6.1 listed in #1102, 16 were re-checked by tests. The rest are not re-tested yet, and U-17 (a new design file is named "Untitled") is still open (#1102).
````

### 5.2 `README.md` — `## What's new` (`0.6.3`) and `### Known issues`

````markdown
<a id="whats-new"></a>
## What's new

Notes for every release are in the [CHANGELOG](CHANGELOG.md). The newest is 0.6.3. [v0.6.0](https://github.com/jangtrinh/design-os-studio/releases/tag/v0.6.0) has captures from the release build, and older releases are short lists.

<a id="new-in-v063"></a>
### 0.6.3

- **Agents panel.** Hook outputs (`agent-status.json`, `agent-events.jsonl`, `turn-marks.jsonl`, `claude-transcript.json`) and Claude's prompt-hook replies live in a Studio-private `hook-out` folder outside the session folder, `$TMPDIR`, `/tmp`, `/var/tmp` and every folder an agent CLI is granted, and the turn-mark hook reads `doc-patch.jsonl` size with `stat -L` instead of opening it.
- **CLI and MCP.** `--screenshot` on a missing, empty or corrupt file exits 1 with an error message and writes no PNG. `--apply-spec`, `designos_apply_spec` and `designos_apply_batch` report invalid `NodeSpec` fields in plain words instead of serde type errors, and the docs note that Ask covers agent tabs started from Studio while your own `designos` CLI and external `designos-mcp` hosts approve their own calls.
- **Linux.** Home › `Import` opens the file chooser when `org.freedesktop.portal.Desktop` starts on first use or when `zenity` is installed, and when no file dialog can open it names the cause and offers `Enter a path…`.
````

````markdown
### Known issues

- Saving a `.fig` from Studio drops numbered-list formatting and writes inherited or variable-bound paints on instances as fixed values ([#834](https://github.com/jangtrinh/design-os-studio/issues/834)). Keep the original `.fig`, or save as `.designos`, until it is fixed. v0.5.0 behaves the same.
- On macOS, Quit from the Dock, logout and shutdown still skip the Save changes question ([#977](https://github.com/jangtrinh/design-os-studio/issues/977)). Cmd+Q and Quit in the app menu ask.
- The Windows test suite is advisory for 0.6.3. Releases are verified on macOS, and known Windows test failures are tracked in [#1158](https://github.com/jangtrinh/design-os-studio/issues/1158).
- The macOS app is ad-hoc signed, so the first launch asks for a one-time Open Anyway in System Settings > Privacy & Security.
````

---

## 6. Unresolved Questions

- PR #1201 (`#1192`, Home › Import on Linux) is still open (`isDraft: true`) against `release/v0.6.3`. Ready to merge `origin/release/v0.6.3` into `jangtrinh/agy-r063-release-notes` as soon as the controller notifies that `#1201` has landed.

Status: DONE
