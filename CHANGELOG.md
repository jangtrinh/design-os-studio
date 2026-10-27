# Changelog (`design-os-studio`)

All notable changes to `design-os-studio` (**DESIGN:OS Studio**) are documented in this file.

## [0.6.3] - 2026-10-26

Patch for **AI design** ([milestone v0.6.3](https://github.com/jangtrinh/design-os-studio/milestone/49)): Studio keeps every agent hook output in a private folder no agent grant reaches, refuses broken `--screenshot` inputs and writes readable spec errors, and offers `Enter a path…` from Home › Import on Linux when no file dialog can open.

### Fixed
- **Agents panel.**
  - Hook commands that Studio installs for Claude, Grok, Antigravity and Gemini no longer write or read files in the session folder. Hook outputs (`agent-status.json`, `agent-events.jsonl`, `turn-marks.jsonl`, `claude-transcript.json`) and Claude's prompt-hook reply files (`page-context-reply.json`, `bootstrap-context-reply.json`, `bootstrap-compact-reply.json`) now live in a Studio-private `hook-out` folder inside the session's approval channel folder in Studio's data directory (mode 0700), outside the session folder, `$TMPDIR`, `/tmp`, `/var/tmp` and every folder an agent CLI is granted. `doc-patch.jsonl` stays in the session folder, and the turn-mark hook reads its byte size with `stat -L` instead of `wc -c <`, so a link or FIFO planted in the session folder is neither written through nor waited on. With no private data folder, Studio installs no file-writing or file-reading hook, and a hook that cannot write its folder (for example under Gemini's or Grok's own sandbox) exits 0 with nothing on stderr (#1072, #1050, #1157, PR #1085).
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
  - Windows was not built or run by hand for these changes.
- Not checked against a live model turn: the Ask-mode Claude launch (skill from a private plugin, Studio's hooks and MCP config from private copies), the Gemini approval state, and the Ask-mode Grok and Antigravity checks were tested with stubs and source reading, not with a real Claude, Gemini, Grok or Antigravity turn.
- Of the 38 GUI fixes from 0.6.1 listed in #1102, 16 were re-checked by tests. The rest are not re-tested yet, and U-17 (a new design file is named "Untitled") is still open (#1102).

## [0.6.2] - 2026-10-25

Patch for **AI design** ([milestone v0.6.2 AI design 2](https://github.com/jangtrinh/design-os-studio/milestone/47)): Studio keeps Ask-mode approvals and its own agent configs out of the agent's reach, Gemini CLI tabs show when they wait for approval, and a text style can be made from a text layer.

### Added
- **Styles.**
  - Create a text style from a text layer. The Typography section of the Properties panel has the style button that Effects and Layout guide already have. Its menu lists the file's text styles, checks the one the layer uses, and ends with `Create text style`, `Rename style`, `Detach style` and `Edit style`. The last three work while a style is linked. `Create text style` names the style `Text style {n}`, takes the layer's font family, size, weight, line height, letter spacing and the rest of its text style, links the layer, and is one undo step. It works in a file that has no styles yet (#875, PR #1024).
  - A text style belongs to the whole layer, not to a range. Text you are still typing is committed first, so the typing and the style are two undo steps (#875, PR #1024).
  - `Create text style` is off, with a tooltip that says why, when the layer's runs differ (`Text has mixed styles`), when several selected layers differ from each other (`Selected text has mixed styles`), and on a new text box that is still empty (`Type some text first`). Runs that differ only in color do not count as mixed. With several text layers that share the same type, it links all of them in one step (#875, PR #1024).
  - Picking a style on a new, empty text box keeps the box, so you can pick a style and then type. If other layers are selected with that box, only the box is styled, and a toast says `Only the new text box was styled` (#875, PR #1024).
- **Agents panel.**
  - Gemini CLI tabs in Ask mode show `Needs approval` and a card, `Gemini CLI needs approval in its terminal`, with an `Open tab` button, when Gemini asks to confirm a tool call. The card has no Approve or Deny, because Gemini keeps its own prompt in the terminal. `Open tab` moves focus there. Before, a Gemini tab showed no waiting state. Studio reads Gemini's confirmation dialog from the tab's own screen, because Gemini fires no hook when you answer No (#930, PR #1058).
  - While Gemini's dialog is on screen, the composer sends nothing into that tab and says `Answer the permission prompt in the terminal`. If Gemini opens a dialog after you send a prompt, Studio does not press the Enter that follows the paste: the prompt stays pasted for you to send (#930, PR #1058).
- **CLI and MCP.**
  - `designos-diff --version` and `-V` print `designos-diff {version}` (#1088, PR #1090).
  - When `designos-mcp` cannot read or append to a session file in the background (a setting, the idempotency ledger, a file it reads at start), the next tool reply carries `session_file_warnings`. When a reply's text is not its JSON (the metadata XML, a screenshot's images), the warnings come as one more text block (#1068, PR #1071).

### Changed
- **Agents panel.**
  - In Ask mode, Claude takes its settings from your own user settings only (`--setting-sources user`). The project and local settings and `.mcp.json` in the session folder are no longer loaded. Studio's skill loads from a private plugin, and Studio's hooks and MCP config come from private copies outside the session folder. Skip mode is unchanged. Claude's own "don't ask again" answer is expected to stop persisting in Ask tabs, because Claude saved it to the project settings. [unverified: expected from a probe, not seen in a live session] (#1044, #1046, PR #1045).
  - With no private data folder, an Ask-mode Claude tab starts without Studio's hooks, and Claude asks in its own terminal prompt (#1036, PR #1041).
  - Grok never evaluates a `.envrc`. Every Grok tab, Ask and Skip alike, starts with `GROK_ENVRC_TIMEOUT_SECS` set to `0`. A session folder never holds an `.envrc` you wrote (#1069, PR #1078).
  - An Ask-mode Grok or Antigravity tab starts only when what the CLI would load from the session folder is exactly what Studio wrote. Any other entry (hooks, MCP servers, plugins, agent definitions, `.envrc`) is moved, not deleted, into `.designos-set-aside/{stamp}/` in the session folder, and the status line says what moved. For Grok the move also takes `.claude/settings.json`, `.claude/settings.local.json`, `.claude/plugins` and `.claude/agents` away from a Claude tab of the same folder; they are kept in the same place. Skip mode starts as before. On a system without `openat`, such as Windows, the tab refuses to start instead of moving the entry (#1069, PR #1078).
  - An Ask-mode Gemini, Grok or Antigravity tab refuses to start when Studio cannot write its config in the session folder, or when what Studio wrote does not read back. The status line names the path and says what to do. Skip-mode tabs start as before (#1061, #1069, PR #1067, PR #1078).
  - Session folders for unsaved files, and the fallback folder for a file whose own folder cannot be used, are now in Studio's data folder: `<data dir>/designos/sessions` (macOS: `~/Library/Application Support/designos/sessions`), owner-only. They were in `$TMPDIR/designos-sessions`. Older folders there are left alone (#1057, PR #1060).
  - A tab that cannot start in a refused session folder says why and what to do, for example `Couldn't start {tab}: {path} is a link, not a folder. Delete the link, then try again.` A library file whose own agent folder is refused starts its agents in a temporary folder that is not kept, and says so: `Couldn't use this file's agent folder: {path} is a link, not a folder. This file's agents start in a temporary folder that is not kept. Delete the link and reopen this design file.` When Studio cannot update its session files, the status line starts `Couldn't update the agent files:` (#1057, #1061, PR #1060, PR #1067).
  - The skill Studio writes for agents, `AGENTS.md` and `GEMINI.md` no longer name the open document or page, and the MCP configs Studio writes for the CLIs no longer carry `DESIGNOS_DOC_NAME` or `DESIGNOS_TARGET_PAGE_NAME`. The agent reads the name from `$DESIGNOS_DOC_NAME` in its shell, or with `designos_get_app_state` (#1053, PR #1045).
  - The Stop button and Stop all do not count a Gemini CLI tab, and no Stop button shows on one. A second Ctrl+C exits Gemini, so Studio has no safe interrupt to send it (PR #1058).
- **Dev Mode.**
  - Code keeps one row per source line, so a token such as a CSS variable reference is no longer split and Android XML strings are no longer cut. A line wider than the box scrolls sideways: a trackpad swipe, Shift + wheel, dragging the code, or dragging the thumb under the last row. A mostly vertical swipe still scrolls the panel. Tabs show as four spaces, and Copy still puts the raw code on the clipboard. The MCP prompt box wraps to the panel width and grows to hold the whole prompt, link included (#887, PR #1033).
- **Properties panel.**
  - The Styles section of a selection also lists styles from enabled libraries when the file has no style of its own. It shows only the groups the selection can take: Text styles for a selection with a text layer, Grid styles for a frame. Library effect and grid styles are listed too. Picking a library style copies it into the file and applies it as one undo step (#1039, PR #1092).
  - The page panel's Export section is the working export section: `+` adds an export setting, and Export exports the page. A page exported as PDF writes its visible top-level frames (#1027, PR #1092).
  - The page panel's Styles header no longer has a `+`. It did nothing. A style is made from the layer it describes, through the style button of that layer's Typography, Effects or Layout guide section (#875, PR #1024).
- **Command palette.**
  - Rename layers semantically and Boost resolution take about one second per click on a very large page, where they took about four. The design fix from the palette takes about 1.7 s, where it took 2.3 s. What Studio records about your edits for an agent does not change. [unverified: timings come from PR #1025 and were not measured again] (#993, PR #1025).
- **CLI and MCP.**
  - `designos-mcp` no longer creates `DESIGNOS_SESSION_DIR`. A missing folder is an error: Studio makes the folder before it starts the tab. The session files `designos-mcp` writes are owner-only (mode 0600) (#1068, PR #1071).

### Fixed
- **Components.**
  - Editing a main component no longer renames its instances. A sync never writes the name of an instance, whether it is the root or sits inside another instance. Other layers inside an instance still follow the main's layer names unless you renamed them. Before, instances took the main's or the variant's name, also on pages that were not loaded yet, and the wrong name reached the saved `.designos` file (#983, PR #1091).
- **Export and CLI.**
  - SwiftUI code escapes `\`, `"`, line breaks, tabs and other control characters in text, component prop defaults and values, font names, token color names and image names. `\(` is escaped so Swift does not read it as an interpolation. `${y}` and emoji stay as they are. Before, the `$` of `${y}` came out with a backslash in front of it, which is not valid Swift and does not compile (#1003 RT-03, PR #1090).
  - Hook commands (status, prompt, skills reply and transcript capture) quote the session folder's path. A path with an apostrophe, such as a home folder named after a person, used to split the path or end the command in a syntax error (#1057, PR #1060).
- **CLI and MCP.**
  - MCP file writes (save, PNG render, `.fig` export) refuse a link at the file name, a name with control or bidi-override characters (#1003 RT-05), and a name over 255 bytes (#1003 RT-06). `designos_export_fig_file` takes an `overwrite` option that is off by default, so it refuses an existing file. An existing-file refusal says `already exists. Choose a new path, then try again.` (or `already exists. Pass overwrite: true to replace it.` when `overwrite` is supported). A path outside the workspace is no longer echoed in the error (#1008, #1087, PR #1103).
- **Opening a file from someone else could run commands or attach files through Studio's agent skill.** Studio wrote the document name and the page name into the skill it gives every agent CLI, and into `AGENTS.md` and `GEMINI.md`. A crafted name in a third-party `.fig` or `.designos` file, or set by an agent that switches the shown document, could add frontmatter keys to the skill, run a `!` command when the skill was invoked, or attach any readable file with `@path`. No agent write access was needed. In Skip mode a `!` command ran with no rule at all; in Ask mode it ran when one of your own allow rules covered it; an `@path` attached its file with no prompt in both modes. Studio now writes no document or page name into any file a CLI loads by itself (see Changed) (#1053, PR #1045).
- **Agents panel: what an agent can no longer do.** Each fix below needs an agent that can already write to its session folder or to `$TMPDIR`: for example a Codex tab under your own `workspace-write` setting, a command you approved, or another tab of the same file. They matter most in Ask mode.
  - Answer its own approval requests. In Ask mode, Claude's Approve and Deny waited for an answer file and ran a script that both sat in the session folder. They now live in a Studio-private folder outside the session folder, `/tmp`, `$TMPDIR` and every folder Studio hands the agent, and a request id is 128 random bits. An answer, a request or a script planted anywhere else is never read (#1023, PR #1035).
  - Replace Claude's hooks. In Ask mode the hooks file Claude reads is private too (#1036, PR #1041).
  - Add allow rules, hooks or MCP servers to an Ask-mode Claude through `.claude/settings.json`, `.claude/settings.local.json`, `.mcp.json` or `mcp-config.json` in the session folder (#1044, #1046, PR #1045).
  - Point another tab's session folder at a folder you own. Unsaved files' session folders sat in `$TMPDIR` under predictable names and were made with a call that accepts a link. Studio now makes every session folder itself, with an exclusive `mkdir` at 0700, and uses an existing path only when it is a real folder you own (#1057, PR #1060).
  - Make Studio write through a link the agent plants inside the session folder. Studio's own writes below a session folder (config for Gemini, Grok and Antigravity, usage cache, run record, session registry, lock, skill replies, snapshots) open each folder from its parent without following a link: a link at a folder name is refused, and a link at a file name is replaced, never written through. Reads of files an agent can plant a FIFO at no longer wait on it, so a FIFO at `live-spec.json` no longer freezes Studio (#1061, PR #1067). On Windows these calls check each entry and then act on its path, so they are not race-free there.
  - Make `designos-mcp` write or read through a link, or hang on a FIFO, at its session files (`canvas.designos`, `doc-sync.designos`, `doc-patch.jsonl`, `context.json`, `bootstrap.json`, `mcp-config.json`, `session-options.json`, `idempotency-ledger.jsonl`). It refuses a link, a second name for a file (a hard link), a FIFO, a socket and a folder. A refused edit fails the tool with an error that names the path and says what to do, for example `<path> is a link, not a file. Delete the link, then try again.` (#1068, PR #1071).
  - Add its own hooks, MCP servers, plugins, agent definitions or `.envrc` to an Ask-mode Grok or Antigravity tab (#1069, PR #1078).

### Known issues
- Hook commands that Studio gives Claude, Grok, Antigravity and Gemini write status files in the session folder, and follow a link an agent plants there (#1072).
- A broken file given to `--screenshot` exits 0 and writes a blank image, and a spec error with `children` as an array of arrays reads as a serde type error (#1003).
- Esc on the Other CLI menu or form also reaches Codex and puts it in rewind mode (#1097).
- A document with no path gives `designos-mcp` the whole disk as its workspace (#1074). In Ask mode each `designos-mcp` call asks first.
- A prompt that contains `No, suggest changes` can hold the automatic Enter, and refuse sends, in a Gemini Ask tab until the line scrolls off (#1066).
- Antigravity, Gemini CLI and Grok start MCP servers from copies of the session config that an agent can write, so a server command added there could run outside any sandbox (#1048). It needs an agent that can already write its session folder without asking, such as Codex under your own `workspace-write` setting. Studio checks and rewrites these copies when a tab starts. It does not stop an agent that writes later, and Grok may read an edited MCP config while a session runs [unverified: from Grok's source, not run]. Studio moves aside the sources that Grok's and Antigravity's docs and source name, and a later version of either CLI may read more. Workaround: in a window where an agent can write without asking, do not run a Grok or Antigravity tab in Ask mode. Planned for 0.6.3: run Grok and Antigravity from a folder only that tab can write.
- Gemini CLI in Ask mode reads `tools.allowed` and other settings from `.gemini/settings.json` in the session folder, which an agent can write (#1047). Some keys there skip Gemini's confirmation prompt, and `hooks` and `tools.discoveryCommand` run a command when the next Gemini tab starts. Gemini 0.35 has no switch that ignores workspace settings for one run. Studio rewrites the file when a tab starts, and an Ask-mode Gemini tab refuses to start when the rewrite fails, so a read-only `.gemini` no longer keeps an agent's file. Planned fix: #1059 (0.6.3).
- Ask covers agent tabs started from Studio. The `designos` CLI, which you run yourself, and a `designos-mcp` started by another host are not gated by Ask: that host approves its own MCP calls (#1101). A real MCP-level approval gate is planned for a later release.
- Codex, Grok, Antigravity and Gemini CLI ask for approval in their own terminal menu, and the panel shows `Open tab`; Approve and Deny in the panel exist for Claude only (#930). Clicking an option in Codex's own approval menu does nothing, so use the arrow keys and Enter (#1096).
- Gemini CLI and Grok in Ask mode: the status line may not show a waiting state for every approval prompt. For Gemini, the state comes from the confirmation dialog on the tab's screen or from Gemini's `ToolPermission` notification. The `ask_user` and `exit_plan_mode` dialogs draw no `No, suggest changes` option, and a panel narrower than 29 columns hides a cut label, so a wait there starts only from a hook record. The screen check is on for Gemini 0.35.x and 0.65.x, read from source only. For another Gemini version, a No answer leaves `Needs approval` standing until you type a prompt. Where Studio writes no Gemini hooks file (Windows, or Gemini already has system defaults), a wait starts only from the screen. For Grok, the state comes from its `permission_prompt` notification, and Studio never checked that payload against a real Grok [unverified]. No real Gemini or Grok approval prompt was run.
- Export Frame as `.spec.json` writes into `designos_specs` in the temporary folder when Studio finds no source tree, by path, so an agent that can write the temporary folder could plant a link at the file name. It needs you to run the export (#1073, 0.6.3).
- Windows (releases are verified on macOS only):
  - Stop on Windows is not checked by any test. The Stop tests fail there because their Python stub imports `tty`, which Python does not provide on Windows (#1066).
  - Palette auto-fix undo is unverified on Windows: eight tests show different auto-layout sizes after one undo, and the cause is not known (#1066).
  - The link checks for session files are not race-free on Windows (#1061, #1068), an Ask-mode Grok or Antigravity tab refuses an entry that is not Studio's instead of moving it (#1069), and no hooks file is written for Gemini, so its `Needs approval` comes from the screen only (#1058, #1066).
  - Linux and Windows were not built or run by hand for these changes.
- Not checked against a live model turn: the Ask-mode Claude launch (skill from a private plugin, Studio's hooks and MCP config from private copies), the Gemini approval state, and the Ask-mode Grok and Antigravity checks were tested with stubs and source reading, not with a real Claude, Gemini, Grok or Antigravity turn.
- Of the 38 GUI fixes from 0.6.1 listed in #1102, 16 were re-checked by tests. The rest are not re-tested yet, and U-17 (a new design file is named "Untitled") is still open (#1102).

## [0.6.1] - 2026-10-23

Patch for **AI design** ([milestone v0.6.1 patch](https://github.com/jangtrinh/design-os-studio/milestone/48)): the bottom terminal dock is gone, agents ask before acting by default, and Studio asks before it drops unsaved edits. Most other changes fix bugs found while testing 0.6.0.

### Added
- **Canvas and editing.**
  - Paste text and images from other apps. Cmd+V (Ctrl+V on Windows and Linux) and Edit > Paste turn plain text into a text layer, and a copied image into an image layer. Text wider than 400 px wraps at 400 px. An image is at most 400 px on its longest side, like the Image tool. Each paste is one undo step (#877).
  - Edit > Paste is never disabled. With nothing to paste, a toast says `Nothing to paste` (#877).
  - Drop PNG, JPEG or WebP files on the canvas. Each file becomes an image layer. A drop of several files is one undo step (#878).
  - A pasted or dropped layer lands in the frame under the point where it lands. Inside a frame that clips its content, as every frame imported from Figma does, it stays in the part that shows (#877, #878, #959).
- **Files and export.**
  - Closing a tab, closing the window, quitting, or opening another file over unsaved edits asks `Save changes to {name}?` with Save, Don't save and Cancel. This covers files outside the library. Library files autosave and never ask (#929).
  - On macOS, Cmd+Q and Quit in the app menu ask the same question (#974).
  - When another program changes a file that has unsaved edits in Studio, Studio asks `File changed on disk` with Use file and Keep mine. Enter, Esc and a click outside keep your edits (#933).
  - A save or export to `.fig` or `.pen` ends with `Comments left out: only .designos files keep them`, because those formats have no place for comments. Saves from the `designos` CLI print the same notice on stderr, and `--export-fig` and `designos_export_fig_file` report `omitted_comment_threads` (#851, PR #1006).
  - A `.fig` export toast shows the full path and a Show in folder action (#874).
  - On Linux, when no file dialog can open, Studio says so instead of doing nothing. Open, Save as, Import and image placement show `No file dialog. Install zenity or xdg-desktop-portal.` Export saves to `~/Downloads`, or to the home folder, under a name that is not taken, and shows `No save dialog. Saved to {path}`. The `.deb` recommends `zenity` or `xdg-desktop-portal` (#850).
- **CLI and MCP.**
  - `designos --version` and `-V` print the version. `designos-studio` and `designos-mcp` do the same (#845).
  - `designos_update_node`, `designos_create_node` and the batch `update` and `insert` operations accept `var:<collection>/<name>` for a fill or text color and bind the variable. An unknown name returns `UNKNOWN_VARIABLE` with the closest names. A variable of the wrong kind returns `VARIABLE_TYPE_MISMATCH` (#917).

### Changed
- **Agents panel.**
  - The bottom terminal dock is removed, and so is the `>_` button at the bottom of the left rail. The canvas keeps its full height. ``Cmd+` `` (``Ctrl+` `` on Windows and Linux) opens the Agents panel and closes it again. Opening it resumes the file's last session, or shows the new-session list. It never starts a plain shell (#838, PR #864).
  - Cmd+K agent presets, `Send selection to agent`, the Inspect `Send to agent` button and `--panel terminal` open the Agents panel. Sync Canvas and Apply Spec from Output are gone with the dock. The disk watcher still reloads a file that changes on disk, and a spec an agent writes to `live-spec.json` still applies (#838, PR #864).
  - **The default agent permission is Ask** (PR #861). A new or unset setting asks before acting, and so does a saved value that is unknown or cannot be read. A saved Skip permissions choice is kept, and Preferences > Agent permissions… still offers it. In 0.6.0 the default was Skip permissions (#841).
- **Canvas and editing.**
  - Paste, Ctrl+D and Alt-drag of a main component in the file it lives in make an instance, as in Figma. They used to add a second main. A paste into another file still pastes a real copy (#860).
  - A marquee over part of a main component or a component set selects it as a whole (#854).
  - The Arrow tool is hidden. It drew a plain line with no head. Shift+L selects Line. Arrows already in files open, render and save as before. The tool comes back when strokes have arrowheads (#858, #859).
  - On a machine whose GPU is a software renderer, such as Mesa llvmpipe in a VM, Studio starts on the CPU canvas. Preferences > Use GPU rendering says why. A saved choice, or `--gpu`, still wins (#840).
  - Home no longer shows the `+ Spec bundle` button or the `PRODUCT:OS Spec Bundle v2` card. Both opened a blank canvas (#867).
- **Files and export.**
  - One writer serves `.fig` export from the command palette, Home and the File menu. It never replaces an existing file, including the open one. A name already taken becomes `Name 2.fig`. A failed write shows `Export .fig failed: {reason}`. Files in the library export to the library `exports` folder, and files opened from disk export next to them (#874).
  - Menu labels: `Export .fig…` is now `Export .fig`, and `Save local .fig copy…` is now `Save .fig copy` (#874).
- **CLI and MCP.**
  - `designos FILE --export-fig PATH` refuses a path that does not end in `.fig`, an existing file unless you pass `--overwrite`, and a symbolic link. A refusal exits with status 2 and leaves the document alone. `--overwrite` replaces a regular, writable file only. Re-running an export onto the same path now needs `--overwrite` (#1000, PR #1005).
  - A FILE argument that is not on disk exits with status 2 and prints `No file named {arg}. Run designos --help.` It no longer opens an empty window. `--apply-spec`, `--import-capture` and `--import-variables` still create the file (#845).
  - Uncolored text in a `NodeSpec` takes `#000000` or `#F4F4F5`, whichever contrasts more with the nearest solid fill behind it. It was always `#F4F4F5`. An explicit color, a style or a `var:` binding still wins. Labels on `easeui-button-primary` are now black (#918).
  - `designos_open_file` checks the path before it checks unsaved changes. A path outside the workspace always returns `PATH_OUTSIDE_WORKSPACE` and is not echoed back. The unsaved-changes hint names `designos_save_file` and `discard_changes=true` (#919).
  - The live edit log is version 2. Studio refuses a log line from a newer build and says `Agent edit not applied. Update Studio and designos-mcp to the same version.` Builds up to 0.6.0 show their old message when they read a version 2 log. Use the `designos-mcp` that ships with this build (#824).

### Fixed
- **Undo.**
  - **Undo and redo of your own edits restore the document exactly.** Before, one edit and Cmd+Z could move or collapse frames on pages you were not looking at, and Cmd+Z could not take that back. On the Untitled UI kit, one edit and Cmd+Z left 80 layers changed, by up to 1,332 px (#973, PR #982).
  - After a font face arrives, and after an agent's edits, undo falls back to solving layout again. The re-solve stays for undo of agent edits and reverts, MCP edits, spec applies and web capture imports; for every step recorded before a font face arrived; for a step made while an earlier edit's layout was still queued; and for scrubbing a field on a document over 5,000 layers (#973, PR #982).
  - The command palette's design fix and its five Figma AI rows (Replace content, Boost resolution, Edit image with prompt, Make prototype flows and Rename layers semantically) are each one undo step. Before, they wrote with no undo step, and Cmd+Z undid the edit before them. Each toast names what it did, as in `Layers renamed · Cmd+Z to undo`. A run that changes nothing shows `Nothing changed`, adds no undo step and keeps the palette open (#984, PR #988).
- **Agents panel.**
  - After Revert, Keep or Dismiss on the review card, the typing caret returns to the prompt box. Before, the next letter picked a canvas tool (#922).
  - A dialog, the command palette or a menu gets Esc, Enter and the arrow keys before the agent terminal. The Esc that closes a dialog no longer interrupts a running turn, and Esc in the prompt box only leaves the box (#923). Popup menus and popovers do the same, including the color picker, the font dropdown and the Variables popovers (#960, #981). The Sessions popover does not yet (#1010).
  - Gemini CLI asks before each Studio canvas tool call in Ask mode. Studio no longer writes `trust` for its server in Gemini's settings (#924).
  - Return acts only on a row that has keyboard focus. Tab reaches the rows, and the arrow keys move between them. A bare Return no longer opens Other CLI (#885).
  - The `+` button opens and closes its menu in release builds (#925). The Other CLI caption no longer touches Cancel (#927). When an agent exits, its tab keeps its width and shows a hollow status ring, so the Sessions and `+` buttons stay under the pointer (#829).
  - A session rename takes the keyboard when it opens. The Other CLI form takes no keys while it is clipped out of view (#1001, PR #1007).
  - When an agent opens or creates another document, your unsaved edits stay in their own tab, and the agent's document never takes your file. Save on the agent's document asks where it goes (#966).
  - A rewritten `live-spec.json` with the same content changes nothing. A changed one replaces the layer its earlier content made. Before, each rewrite added another copy (#846).
  - With the Agents panel closed, an agent's edit shows up without a mouse move or key press first (PR #948).
- **Canvas and editing.**
  - On a slow frame, a drawn shape, a move, a resize and a rotate end where the pointer let go. Before, a quick drag made a 100 x 100 shape at the wrong spot, or left the layer short (#842, #862).
  - A quick drag in the Layers panel reorders the layer (#855). A toolbar menu picks the row under the pointer on release (#852).
  - Keys typed in the frame of a click on a field stay in the field, and Tab moves between fields in the order they are drawn (#843, #844).
  - Shift+D enters and leaves Dev Mode, as the toolbar tooltip says (#1012).
  - Ctrl+B draws bold in a downloaded font, and Bold follows a `wght` axis (#873).
  - Resizing a vector or a boolean group scales its shape, not only its box (#853).
  - Text draws with a font face as soon as it arrives. Text you sized while that face was missing is fitted to it. A text box that came from the file is not changed (#886).
  - Applying a fill style drops the fill's variable binding (#955).
  - The toast paints above the full Variables view (#830).
  - A property edit on an instance goes through when its stored variant would make a main hold itself (#989).
  - Studio no longer aborts when a main component holds an instance of itself through a pair or a ring of mains. Paste, Ctrl+D, Alt-drag, drops from Assets, variant switches and MCP batch `move` no longer put a main inside itself (#860).
- **Files and export.**
  - The unsaved dot clears when a save writes the document, returns with the next edit, and clears again when undo walks back to the saved state. A save as `.fig` or `.pen` leaves the dot lit, because those files drop comments (#847).
  - Switching tabs no longer replaces unsaved edits with the file on disk (#933).
  - The Comment tool no longer shows `Comments are not available in the offline editor`. Comments work, and the click opens the composer (#851, PR #1006).
- **CLI and MCP.**
  - A literal color written with `designos_update_node` or `designos_apply_batch` to a layer bound to a variable now drops the binding. Before, the call said `changed: true` and the canvas kept the variable's color. Linking a fill style with `set_style` drops the fill bindings too (#916).
  - `designos-diff compare` on images of different sizes exits with status 1 and prints `Images differ in size: {a} vs {b}. Render both at the same size.` It used to panic (#883).

### Known limits
- On very large pages (100,000 layers or more), Rename layers semantically and Boost resolution from the command palette can take several seconds. Recording them as one undo step costs time on a big page. The next release addresses this first (#993).
- On macOS, Quit from the Dock, logout and shutdown still skip the `Save changes` question (#977).
- A file where several main components hold instances of each other can still run Studio out of memory when you edit it. No Studio gesture or agent tool builds one. The guards cover pairs and rings of mains (#968).
- On Linux, paste from other apps and file drops work through X11 or XWayland only. A pure Wayland session sends no file drop event (#877, #878).
- A `var:` color that names a variable from an enabled library binds only through a direct `designos_apply_spec` call. `designos_create_node`, `designos_update_node` and the batch `update` and `insert` operations return an error that says so (#954).

## [0.6.0] - 2026-10-22

Pack **AI design** ([milestone v0.6 AI design](https://github.com/jangtrinh/design-os-studio/milestone/4)): agents can see a design, read its design system, write in one step and check their own work. The Agents panel gets sessions, review and stop.

### Added
- **Agents panel: sessions.**
  - Opening the panel resumes the file's last session. With none to resume, it shows the CLI picker. Studio no longer starts a shell by itself (#651).
  - A file keeps its sessions after a restart. A menu in the panel lists them, renames them and removes them from the list. Files opened from disk keep sessions too. Renaming or moving such a file starts a fresh history (#653).
  - A session that cannot resume says why and what to do next (PR #806).
  - The new-session picker lists installed CLIs first, with version and login state. The CLI you used last for the file comes first. A CLI that is not installed links to its install page. Rows keep their order while the checks finish, so a click lands on the row you aimed at (#652, #783).
  - Gemini CLI joins Claude Code, Codex, Grok and Antigravity in the new-session picker. OpenCode is not in the picker yet. A sample prompt starts it only when it is the only agent CLI installed, and it runs without Studio tools (#679).
- **Agents panel: talking to an agent.**
  - The composer sends into the tab's live CLI session. Each tab keeps its own draft and its last 100 prompts (#689).
  - A chip shows the current selection. Click it to hand the selection to the agent. It hides when nothing is selected (#654).
  - Type `@` to mention a layer, page, component, variable or style. The agent gets a reference it can resolve (#655).
  - The empty state offers sample prompts that fill the composer, and hints for attaching a selection (#688).
  - Attach bundled skills or your own `SKILL.md` files to a design run. Three skills ship with Studio (#573).
  - On macOS, Cmd+Z undoes and Shift+Cmd+Z redoes document edits while a terminal tab has focus. On Windows and Linux, Ctrl+Z stays with the CLI (#685).
  - AI image tools (remove background, isolate subject, expand, erase) run through the connected agent CLI. They turn off when no agent CLI is installed. Studio bundles no model weights (#177).
- **Agents panel: while an agent works.**
  - Each tab shows live status. Each turn ends with a receipt: how long it worked, how many tool calls, how many layers changed. A system notification tells you when an agent finishes in the background (#683).
  - Review a turn's edits, then keep them or revert them in one undo step. A revert leaves layers you edited yourself (#677).
  - Stop one agent, or all running agents, from the panel. Studio interrupts the CLI, closes the open edit batch as one undo step, and records that you stopped it (#684).
  - An optional ask mode (Preferences > Agent permissions… > Ask before acting) launches CLIs without their skip-permissions flags. Claude's requests show as Approve or Deny in the panel. Codex, Grok, Antigravity and Gemini CLI ask in their own tab. The default stays Skip permissions (#656, PR #817).
  - When a session fails, a banner names the cause and the next step: rate limit, expired login, billing or quota, provider error, full context, or no network (#686). A CLI that fails to start gets the same: not installed, not signed in, or a rejected flag. Any other early exit shows its exit code and the last lines it printed (#673).
  - Claude and Grok tabs show tokens and cost (Grok's cost is a list-price reference, marked ref.). Codex, Antigravity, Gemini CLI and OpenCode show none, because Studio has no usage source for them (#678).
  - Screen readers hear when an agent finishes, needs input or fails, and the panel regions have names. Cmd+Alt+J (Ctrl+Alt+J on Windows and Linux) moves focus between the composer and the canvas. When a text layer is selected or being edited, the chord still justifies text (#687).
- **Agents on the canvas.**
  - Each agent shows a named cursor, and a layer flashes when the agent writes it (#682).
  - When a turn ends with changes off screen, a chip at the canvas edge shows how many. The view does not move until you click it (#695).
  - When you edit inside a frame an agent is working on, a banner names the agent. You can duplicate the frame or edit anyway (#693).
- **MCP: see the design.**
  - `designos_get_screenshot` returns PNG or JPEG image content for a node, for up to 50 nodes on one page, or for a region (#238).
  - `designos_get_metadata` returns a sparse XML outline with bounds, layout, components, text previews and, with `lint: true`, issue counts (#239).
  - `designos_get_design_context` returns code, variable and style hints, and a screenshot for a node (#241). Dev Mode and the MCP server share the same data (#240).
  - Write calls can return before and after images of the region they changed, with `diff_image` (#593).
  - `designos_query_xpath` pages through large documents with a node budget, compact rows and cursors. It can select by lint problem, bound value, hard-coded value or in-progress state (#602).
- **MCP: know the design system.**
  - `designos_get_libraries` lists the local and enabled libraries (#247). `designos_search_design_system` searches components, styles and variables across them and returns instance-ready examples (#272).
  - `designos_get_guidelines` and the `create_design_system_rules` prompt write design-system rules from the open file (#248).
  - On its first turn, an agent gets the open file's outline, variables, components and style. `designos_get_app_state` returns the same on demand. The selection is sent only when the agent asks for it (#601).
  - A rules pack with a build, verify and fix workflow is generated from the tool schemas. It ships with the skill and with each session. `designos.enabled_tools` in the session config limits the tool list (#600).
- **MCP: write safely.**
  - `designos_apply_spec` places component instances and variants, and sets properties and overrides by path (#586). It fills slots (#587). It binds fills, strokes, numbers and styles to variables with `var:` references. The old `$easeui.*` tokens still resolve and warn as deprecated (#588). An agent undo keeps variables and collections you edited after the agent wrote them (PR #803).
  - `designos_apply_batch` applies up to 200 operations as one transaction. It supports `dry_run`, makes one undo step, and returns the created ids and a name-to-id map (#589).
  - A mutating call can carry an `idempotency_key`. A replay with the same key and arguments returns the first reply and writes nothing (#591).
  - `designos_set_in_progress` flags a frame while an agent works on it. The flag clears when the turn ends or you stop the agent. Flags are not undo steps (#592).
  - Every tool advertises an `outputSchema` and returns `structuredContent`. Errors carry a named code, a corrected example and a `safe_to_retry` flag (#590). See Breaking.
  - `designos_create_node` and `designos_update_node` take `fill` and `fills`. On a text layer the colour goes on its text style and runs (#593, PR #813).
  - On files above 25,000 nodes, agents keep the file's variables and styles, so `var:` and style references still resolve (#696).
- **MCP: check the work.**
  - `designos_lint` is one geometric linter with 27 rules, four presets (agent, check-designs, recommended and strict) and eight category presets. It finds overflow, clipping, sibling overlap, truncated and wrapped text, collapsed sizes and detached instances. Check designs shares its detached-instance and contrast checks with it (#594).
  - Contrast is measured against the real backdrop, at WCAG AA and AAA. A finding names the layer that covers the text (#595).
  - Off-scale and hard-coded value rules find spacing, radius and type sizes outside the file's scale, and raw values that match a token (#596).
  - Each mutating call returns a short `issues` block and marks problems that were already there. `designos_configure_session` sets the mode, the preset and the number of items (#597).
  - `designos_lint_fix` applies the deterministic fixes for auto-fixable findings on a page or on chosen layers, in one undo step. Its default preset is strict (#594).
- **MCP tool list.** `docs/mcp-parity.md` maps Figma's 41 MCP tools to Studio's. A test pins the tool list, and the tool tables in the README and the skill are generated from it (#243).
- **Web capture.**
  - A capture script in the source tree (`crates/designos-webcapture/js/capture.js`, run with Playwright) records a rendered page: computed styles, boxes, text runs and images (#276).
  - Studio imports a capture as editable auto layout frames with live text, in one undo step. Use the File menu, drop the file on the canvas, run `--import-capture`, or call `designos_import_web_capture`. A report lists substituted fonts and anything not captured (#277). If the import worker crashes, a toast says so (PR #806).
- **Styles hold variable aliases.** Colour, effect and text styles can point to variables (#131).
- **Variables as Figma JSON and DTCG.** Import and export variables in Figma's native JSON and in W3C Design Tokens (DTCG), from the Variables menu or with `--import-variables` and `--export-variables`. An import merges into the file. Studio reports how many variables it imported and lists any it skipped, and `--import-variables` prints the added, updated and skipped counts. The Figma REST format is unchanged (#138).
- **`.pen` 2.21.** Studio reads and writes JSON scene format 2.21 and migrates older files on import (#570). See Breaking.
- **Check designs.**
  - The rules match Figma's list. A rules popover turns each one on or off and sets contrast to AA or AAA (#158).
  - Ignore keeps a finding off and is saved in the file. Dismiss hides it for the session (#158).
  - Whole pages are scanned a little each frame (2 ms slices), so the canvas stays responsive, with a progress bar. The 20,000-layer cut-off is gone. Fix all waits until the scan ends (#157).
- **Slots.** Stretch on insert applies when you drag a layer into a slot, drop a library asset on it, or place an image, video or SVG in it. Drawn layers keep their size (#644).
- **Dev Mode codegen.**
  - HTML joins CSS, Tailwind, React, SwiftUI, UIKit, Compose and Android XML. It writes a full page or a fragment, from Dev Mode, `--dev html`, `--codegen html` and `designos_generate_code` (#250).
  - React components declare their variant and property props, with defaults and TypeScript unions for variants. Reserved words in prop and slot names are renamed the same way in React, Compose and SwiftUI (#605).
- **Branches in the library, engine only.** A file can keep branch copies and checkpoints (#343). A three-way diff compares nodes, styles, variables and components, and reports conflicts per layer (#344). A merge remaps ids and takes each conflict from main or from the branch (#345). Studio, the CLI and the MCP server do not use branches yet.
- **Vendored skills.** The MIT-licensed `jakubkrehel/skills` are vendored at a pinned commit, with attribution. A rule map ties each of its 273 rules to a lint rule (34), a rubric axis (70) or a skill section (58), or marks it web-only (111). `designos-mcp --list-lint-rules` prints the 27 lint rule ids (#636).
- **Evaluation harness.** `designos-eval` (in the source tree, not in the release packages) runs each design task three times by default. It reports bootstrap confidence intervals, keeps a held-out split, and writes JSONL. `DESIGNOS_MCP_TRANSCRIPT` logs every tool call (#613). A register and a CI check cover the licences of eval datasets, model weights and skills (#611).
- **UI copy standard.** `design/ux-writing.md` ports the VSF PCP writing rules. A gate fails new action labels over three words or with a count in parentheses (PR #804).

### Changed
- **DESIGN:OS Studio is source-available under PolyForm Shield 1.0.0** (PR #820). You can use and modify it, including for paid work. You cannot resell it or use it to build a competing product. Releases up to v0.5.0 were published under MIT OR Apache-2.0 and keep that license. Packages ship one `LICENSE` file instead of `LICENSE-MIT` and `LICENSE-APACHE`.
- **Large `.fig` imports have a global clone budget** of 8,000,000 nodes. Instances past it stay unexpanded, survive a `.fig` and `.designos` round trip, and a warning says how many (#5).
- **`designos_open_file` takes larger files:** up to 512 MiB and 1,200,000 layers. The limits were 64 MiB and 100,000 (#602).
- **UI copy follows the writing standard.** Labels and messages in the Agents panel, Check designs, the variable menus, AI image tools and the web capture report are shorter and verb-first. Tabs, the panel header, Sessions rows, screen-reader announcements and notifications use the same labels for the same agent states: Needs approval, Needs input, Working…, Done, Failed and Stopped. New action labels carry no count in parentheses, and failures never show raw codes (PR #814, PR #817).
- **The Agents panel shows only real output.** Send no longer inserts a template. The canned "Executed via" line and the microphone button are gone (#681). The empty-state category cards and the More button that collapsed them give way to sample prompts (#688, PR #807).

### Fixed
- **Code export.**
  - A child that fills the counter axis of a hugging parent exports its solved length, so a row of fill children no longer collapses to 0 px in CSS or Tailwind (#578).
  - A grid child with vertical fill exports the height the layout solver computed (#639).
- **Constraints.** `.fig` import and the editor use one constraint solver. Rotated children, rounding and sizes under 1 px match Figma (#691).
- **Components and slots.**
  - Migrate to slot on an unbound instance in one variant no longer adds a slot property to the whole set that the other variants never get (#645).
  - Slot content written through MCP, live patches or `.fig` import is marked, so a main-component sync keeps it (#582).
- **Auto layout.** A direction change made through MCP before the first solve no longer stretches a child twice (#583).
- **`.fig` export.** Component keys no longer collide, and half-null GUIDs survive a round trip (#669).
- **Check designs inside instances.** Layers that only mirror their main component are skipped, which cuts the work on large pages. A contrast problem that an override creates inside an instance is reported on the instance (#663).
- **Agents.**
  - Session files live in a per-session folder instead of shared paths under `/tmp`. The scratch folder is removed when the last tab or the window closes. Folders of dead sessions older than 7 days are swept at startup. The live document is written by a worker thread, not the UI thread (#690).
  - On macOS, a terminal tab no longer drops the output of a command that exits at once (#782).

### Breaking
These change what agent integrations and exported files see. No MCP tool is renamed or removed. The old names `export_fig_file`, `query_nodes` and `check_design` still work.
- **MCP errors are named errors (#590).**
  - `structuredContent.error` was a string. It is now an object with `code`, `message`, `path`, `safe_to_retry` and `next_step`, plus `expected`, `valid_options` and `corrected_example` when they apply. The same fields are copied to the top level of the reply and into `content[0].text`. `path` is a JSON Pointer.
  - If you matched on the error text, read `error.code`. Read `error.safe_to_retry` before you retry. Use `corrected_example` and `valid_options` to fix the call.
- **Unknown keys in a spec fail (#590).** `designos_apply_spec` and batch specs reject keys the schema does not list in `NodeSpec` and `LayoutSpec` (and in the new `OverrideSpec`). The error is `UNKNOWN_FIELD`. Remove the key. Documented aliases still work, such as `layout.dir`.
- **`designos_query_xpath` and `query_nodes` return pages (#602).** The reply was `{selector, page_id, nodes, total}` with every match. It is now `{columns, rows, count, scanned, scanned_total, scope_size, remaining, done, complete, cursor}`. A call returns at most 100 rows by default, and at most 500. Call again with `cursor` while `done` is false. Send `format: "objects"` to get objects instead of rows. Old selectors still work.
- **`designos_lint` and `check_design` replies changed (#594).**
  - The reply was `{page_id, findings, total, passed}`, and a finding was `{rule, severity, node_id, name, message}`.
  - It is now `{findings, total, by_severity, by_rule, skipped_rules, scanned, complete, elapsed_ms, cursor}`. A finding carries `node_name`, `xpath`, `bounds` and `fixable`, not `name`. There is no `passed`. A call returns 100 findings by default, so follow `cursor`.
  - Findings use canonical rule ids. `zero-bounds` is now `collapsed-size`. The old ids still work as input.
  - The default preset is `agent`. It no longer reports `default-layer-name`. Pass `preset: "recommended"` to get it.
- **The global live-session files are gone (#690).** Studio and `designos-mcp` no longer use `/tmp/designos-live-context.json`, `/tmp/designos-live-canvas.designos`, `/tmp/designos-live-agent-out.spec.json`, `/tmp/designos-live-doc-sync.designos` or `/tmp/designos-live-selection.spec.json`. Use the folder named by `DESIGNOS_SESSION_DIR` (or the `DESIGNOS_LIVE_*` path variables). Without them, `designos-mcp` leaves those paths unset.
- **Use the `designos-mcp` that ships with this build.** The live edit log has new operations for variables, styles, library state, images and in-progress flags, and new `label`, `session_id` and `agent_label` fields (#586, #589, #592, #696).
- **`.pen` export is version 2.21 (#570).** Studio wrote 1.0. A reader that accepts only 1.0 must read 2.21. Studio refuses `.pen` files newer than 2.21 and names both versions in the error.
- **`DESIGNOS_FORCE_LOCAL_SYNTH` is removed (#681).** Send no longer builds a template, so nothing replaces it.

## [0.5.0] - 2026-10-19

Pack **Components** ([milestone v0.5 Components](https://github.com/jangtrinh/design-os-studio/milestone/3)): slots and a stronger Check designs.

### Added
- **Slots.**
  - Create a slot on a component and set its rules in the slot settings panel: minimum and maximum children, preferred components, stretch on insert, and show empty by default (#32).
  - Edit slot content in an instance without detaching it. The limits hold on every edit: draw, paste, drag, drop, group, ungroup, and placing images or video (#34).
  - An empty slot shows a placeholder with a **+** that opens the preferred-instance picker. Inserting stretches the child to the slot when the slot asks for it (#150).
  - **Migrate to slot** turns an instance-swap or boolean region into a slot and keeps each instance's content, in one undo step (#151).
  - Dev Mode codegen emits slots: one parameter per slot with the slot's own layers as the fallback, in React, Tailwind, SwiftUI and Compose (#152).
- **Component playground** in Dev Mode: try a component's properties and variants without editing the file (#159).
- **Check designs.**
  - Flags hard-coded numbers that exactly match a number variable (#154).
  - Suggests variables and styles from enabled libraries (#155).
  - Audits text and overrides inside component instances. A finding inside a component is reported once, on the main component (#52).
  - Contrast checks use an opaque parent or sibling as the backdrop (#65).
- **Chrome design standard.** One spacing ladder, concentric radii (frame 14, control 6, inset 8), pills for chips and dialog buttons, a neutral selection fill, 10 px captions, and Roboto Mono for code (#580).

### Fixed
- **`.fig` files.**
  - Export uses the field numbering, types and enum values from Figma's own files (#110).
  - Slot preferred values are read and written as Figma's real object (#107).
  - Slot content ids are stored per property, and older files load unchanged (#108).
  - The slot-list parser rejects the raw-number fallback (#111). Unknown slot settings keys are kept (#112).
  - Nested instance swaps update the shell's fills, strokes and properties (#525).
  - Instance text overrides with several paragraphs or mixed styles are kept (#518).
  - Resized instances apply Stretch and Scale constraints to their children (#522).
- **Paste from Figma** remaps slot and instance-swap ids and drops unknown ones (#109).
- **Styles.** Applying a colour style recolours every text run, in one undo step (#68).
- **Agent tabs.**
  - Antigravity and Grok tabs start again: Studio registers its MCP server in the way each CLI accepts (#642, #646).
  - Claude, Grok and Antigravity tabs load the session hooks; Codex tabs keep the existing status detection (#647). The designos-studio skill and rules are reachable from every tab (#648).
  - Codex tabs and Agent Chat pass the session folder and workspace to designos-mcp (#649).
  - A tab that fails to start shows the cause and a way to continue (#650).
- Windows CI: terminal tests use the system temp folder (#581).

## [0.4.1] - 2026-10-19

Patch for **Layout 2.0** ([milestone v0.4.1 patch](https://github.com/jangtrinh/design-os-studio/milestone/46)): fixes found after 0.4.0, many of them from rendering the Untitled UI PRO v8.0 kit against Figma.

### Fixed
- **Grid and auto layout.**
  - Grid placement pins and spans in Dev Mode now match the solver (#531).
  - An instance that restates its grid track list keeps its children in the right cells and its track sizes in the right order (#539).
  - Older grid alignment data migrates on load. Unoverridden alignment axes stay in sync with the main component, and large margins centre correctly (#541).
  - Fill children are measured the same way in flow and grid layouts (#530).
  - Changing a frame's direction no longer stretches a child twice, and an all-stretch line shrinks back. CSS export writes `align-self`, and a grid child exports its solved size (#532).
  - A bound stroke weight is resolved during relayout (#507).
  - Undo and redo re-solve only the layouts they touch, so stroke colour, move and effect edits cost no relayout (#509).
  - The delete action is hidden on custom-sized implicit grid rows (#562).
  - Preset frames get the catalogue corner radius (#498).
- **Rendering.**
  - Frames with "First on top" canvas stacking paint and hit-test in Figma's order on the canvas, in exports and in prototypes. The Layers panel keeps layer order, as Figma does (#517, #527).
  - Strokes on per-side borders follow corner radii (#526). Outside-aligned strokes clip with imported stroke geometry (#524).
  - Radial, angular and diamond gradients use centred handles (#529).
  - Soft line breaks (U+2028) break the line without adding paragraph spacing (#523).
  - A bound colour variable's alpha is applied once (#516).
  - The mask stand-in no longer overdraws or disappears at sub-pixel sizes (#500).
  - Children of unknown layer types paint and can be selected (#511).
- **`.fig` import and export.**
  - Per-side stroke weights are imported only when they differ, and partial overrides merge (#515).
  - Style records no longer turn into canvas nodes in libraries, and instance layout alignment survives a round trip (#546).
  - An instance whose auto layout was removed writes `stackMode: NONE`, so the removal survives a round trip (#539).
- **Files and libraries.**
  - Non-finite numbers in imported files are cleaned up in memory, and a failed write rolls back (#56). The open document, hot reload and file switch stay consistent with the notice (#506).
  - Replacing a repaired original releases its cache (#505).

## [0.4.0] - 2026-10-18

Pack **Layout 2.0** ([milestone v0.4](https://github.com/jangtrinh/design-os-studio/milestone/2)): auto layout and grid behave the way Figma does.

### Added
- **Grid auto layout.**
  - More than 8 rows and columns (#24).
  - Auto-placed items and auto rows. `.fig` files keep their version 106 track maps (#25).
  - Children align horizontally and vertically inside their cell (#145).
  - Dev Mode prints CSS, Tailwind and React grid code from the real tracks and placement (#146).
  - Edit tracks on the canvas and in the Properties panel: insert, delete, resize, and drag a track to reorder it in one undo step (#144).
- **Auto layout.**
  - Child margins from `.fig` files (#29).
  - Per-child align-self in auto layout version 1 (#148).
  - Version 1 semantics, with borders inside the box. An **Update auto layout** action upgrades an older frame (#26).
- **Layout guides match Figma** (#147).
  - Alignment: Stretch, Left, Center and Right.
  - A fixed column or row width.
  - Auto count.
  - Show or hide each guide.
  - All of it round-trips through `.fig`.
- **Check designs** (#35).
  - A panel that audits the page for low contrast, unbound colours, detached styles and detached instances.
  - A one-click Fix per finding, and Fix all.
  - Contrast fixes skip text when something painted under it would make the check unreliable.
  - Text runs that have their own colour keep it when a variable or style is applied.
- **Vectors.**
  - Offset path (#195).
  - Scale several selected points with a bounding box (#186).
- **Export.**
  - PDF export draws shapes as vectors, and its text is selectable. In 0.3.0, PDF pages were images (#40).
  - Lossless WebP (#215).
- **Frame and device presets** match Figma's current list (#149).
- **Board timer** for facilitators in the board toolbar (#338).
- **Layers panel** shows a label for each layer type (#405).
- **Variables** from a library are grouped under *From library* and are read-only (#279).
- **Style editing.** Drag the label prefix to scrub number fields in the style popover (#69).
- **`.fig` round trip.**
  - Noise, texture and glass effects survive import and export (#106).
  - Fields and layer types Studio does not draw yet are kept and written back on export (#174).
  - Slots are read and written (#31).

### Fixed
- **An open library file could be emptied (#504).** Switching tabs or going Home could write an empty document over the open file and store an empty version. Unsaved edits are now saved before the tab is parked.
- **Crash on large pages at rest (#467).** Some large pages with many effects crashed about 1 s after opening. The full-quality refine is now split over several GPU submissions; Metal allows about 2,047 render passes per submission.
- **Security: copied or selected content is never written into a Shell tab (#513).**
- **Rendering.**
  - A mask frame without its own fill takes its alpha from its children (#448).
  - Hidden icons inside button components stay hidden (#450).
  - `--screenshot --gpu` captures the full-quality frame at rest (#449).
- **Layout correctness.**
  - Fill siblings share the space a clamped child cannot use (#60).
  - Grid tracks size from clamped children (#61).
  - The grid axis convention is the same in layout, export and the Properties panel (#63).
  - Variable-bound sizes resolve in headless relayout (#62).
  - Deep re-solves have a budget, and nothing past it goes stale silently (#59, #489).
- **Files and saves.**
  - Studio tells you when numbers that were not numbers (NaN or infinity) were replaced (#57).
  - Malformed native and library files open with a warning and keep a backup of the original (#7, #104). An existing `.bak` is never overwritten (#102).
  - Repaired original bytes are released when the tab closes (#103).
  - Zip bombs in `.fig` files are rejected before they are buffered (#9).
  - Atomic save edge cases are handled (#4).
  - NaN and infinity never reach grid track strings, variables or the clipboard (#71).
- **Components and styles.**
  - A per-instance style override survives component sync (#73).
  - A text in Height mode no longer records a Size override when a style changes (#72).
  - Library styles cannot be renamed, and the menu says why (#67, #100).
  - Effect and style popovers no longer show duplicate prefix letters (#101).
- **Agents and live sync.**
  - Each Studio window has its own agent session folder (#80).
  - A retried document switch does not log the same edit twice (#98).
  - Dropped edits are no longer counted twice (#97).
  - Comment edits update the canvas (#78).
  - Edits that would break builder-replaced layers are refused (#95, #96).
- **Idle CPU is back to 0%** (#12).

### Changed
- **One auto layout solver.** The editor, the exporters and the MCP server share it (#30).
- **The GPU refine has a program ceiling, and its VRAM budget counts the root target** (#83).
- **The homepage** is a dark landing page with the 0.3.0 film.

### Under the hood
- The document model has comment author identity (#351) and 15 easing types (#139). The editor does not write comment author identity yet (#472).

### Known issues
- Writing a large `.fig` and reading it back adds nodes on every cycle, and one nested instance can lose its centred alignment (#546). In 0.3.0 such files could not be written at all.
- The Windows test job is red (#487). Windows and Linux builds are tested by their users; releases are verified on macOS.
- Follow-ups from the Layout 2.0 reviews ship in [v0.4.1](https://github.com/jangtrinh/design-os-studio/milestone/46).

## [0.3.0] - 2026-10-17

Pack **Paste & Styles** ([milestone v0.3](https://github.com/jangtrinh/design-os-studio/milestone/1)).

### Added
- **Paste from Figma (#27).**
  - Copy a selection in Figma desktop and press Cmd+V on the canvas.
  - The layers land centered in the view as one undo step, images included.
- **Effect and grid styles (#27).**
  - Create, apply, detach and rename them from the Effects and Layout guide sections.
  - They also show under local styles.
  - `.fig` files bring their effect and grid styles with them.
- **Edit a style from the style picker (#44).** Changing a style's values updates every layer that uses it, as fill or as stroke, in one undo step. Library styles stay read-only.
- **PDF export (#27).** Selected frames export as one multi-page PDF from the Export panel. Pages are raster images for now.
- **Auto layout (#27).**
  - Space around and Space evenly.
  - Wrap in the vertical direction, with wrapped lines that can spread.
- **Text wrap Balance and Pretty (#27).**
- **Variables (#27).**
  - Paint and effect opacity can be bound to a number variable.
  - Variables keep their code syntax (Web, Android, iOS), which Dev Mode prints the way Figma does.
- **Dev Mode: UIKit and Android XML (#27).**
- **Clean captures:** `--no-toast` and `--no-caption` for `--screenshot` (#13).

### Fixed
- **Agent edits keep the open document intact (#28).**
  - `designos_apply_spec` and other edits made through the bundled `designos-mcp` server no longer replace the open document. On large files, the other frames used to become empty boxes and the Text styles list emptied.
  - Edits are replayed as node operations: one undo step per batch, the new layer is selected, and the view stays where it was.
  - Agent edits wait while you drag or edit text, then land, and they survive switching documents.
  - Undo through the MCP server is refused once the canvas has reloaded, so it can never remove your own layers. Undo in Studio works as before.
  - Update `designos-mcp` and Studio together; the bundled server matches the app.
- **Files with broken numbers open again (#42, #6).**
  - A value that was not a number (NaN or infinity) used to be saved as `null`. The file then refused to open with `invalid type: null, expected f32`.
  - Such values are now never written. Files that already contain them open, with each one replaced by a neutral value.
- **Large pages render at low zoom (#13).** The GPU canvas renders each frame in its own submission. Pages such as a 68,698-layer `.fig` at 3% zoom no longer show an empty canvas.
- **Untrusted files and saves (#1).**
  - `.fig`, kiwi and `.pen` decoding is bounded (total inflate budget, entry and depth caps).
  - Saves are atomic (temp file in the same folder, fsync, rename).
  - Documents with dangling or cyclic layer references are repaired on load instead of crashing; the MCP `open_file` tool refuses repaired documents.
- **The app reports its real version.**
  - The workspace version was still `0.1.0` in v0.2.0, so the About dialog and the updater misreported it.
  - The release workflow now fails if the tag and `Cargo.toml` disagree.

### Changed
- **Variable references in CSS, React and Tailwind (#27)** keep the variable's case and print a fallback, for example `var(--Color-Primary, #0D99FF)`.
- **Idle full-quality GPU refine (#1).**
  - After 250 ms without changes, the canvas renders one uncapped frame (masks, blur, blend modes, group opacity), bounded by a 512 MiB VRAM budget.
  - Any interaction returns to the capped interactive path.
- **Lower memory on `.fig` import.** Peak RSS on large files is about 6.4 GB lower, and Phase 2 engine optimizations were added.
- **Removed UI that did nothing (#1).**
- **The README and the site open with the 70-second promo film (#48).**
- **Releases are verified on macOS.** A failed Windows or Linux build no longer blocks the macOS release; those packages are attached when their builds pass.

## [0.2.0] - 2026-10-15

### Added
- **Cross-Platform Installers (`DIR-22`)**: Added one-command release builder (`scripts/build-release.sh`) producing macOS `.dmg` (`designos.app` + `install-cli.command`) + `.tar.gz`, Windows NSIS `-setup.exe` + `.zip` (`install.ps1`/`uninstall.ps1`), Linux Debian `.deb` + portable `.tar.gz` (`install.sh`), `dist/release-manifest.json`, and `dist/SHA256SUMS`.
- **Native Agentic PTY Chat UI Wrapper (`DIR-21`)**: Added structured User/Assistant chat turn bubbles (`AgentChatTurn` / `AgentChatRole`) inside the `Agents` Left Rail (`RailItem::Agents`) wrapping live `portable-pty` + `vt100` sessions for Claude Code (`claude`), Codex (`codex`), and Antigravity CLI (`agy`) with inline `NodeSpec` detection and 1-click Canvas apply.
- **100% Unified Phosphor Icons System (`DIR-19`)**: Unified all UI3 chrome, toolbars, left rail, inspector, and Spec Reader surfaces on official Phosphor Icons (`@phosphor-icons/core` `256x256` SVG rasterized via `resvg`).
- **11-Stage `PRODUCT:OS Spec Driven v2` Reader (`DIR-18`)**: Full in-app Spec Reader (`--panel specs` / `--rail specs`) rendering `US-01..US-15` User Stories, Gherkin `Given/When/Then` scenarios, and `g1..g5` verification gates.
- **1:1 Figma `126.9.13` UI3 & `.fig` v106 Parity (`387 / 387 — 100.00%`)**: Complete visual chrome, typography, Auto-Layout 5.0, Variables, Components, Prototype flows, Dev Mode codegen, and native `.fig` v106 binary read/write parity verified by `823` automated tests.
- **AI-Ready Repository Delivery & 9-Tier GitHub Pages (`DIR-23`)**: Added `.claude-plugin`, `.codex-plugin`, `.agents/plugins`, `.factory-plugin`, `skills/design-os-studio/SKILL.md`, `/doctor`, `scripts/self_check.py`, 12-section editorial `README.md`, and 9-tier `docs/index.html` with 15 visual proof captures.
