# 0.6.5 smoke, run by Claude on Jang's Mac (2026-10-27, 19:48–20:10 UTC+7)

**Build:** Release dry run 40495181911 on `451e42ba`, artifact `designos-macos-universal`. `designos-0.0.0-dev.451e42b-macos.dmg`: `shasum -a 256 -c` OK. The app was copied out of the read-only mounted dmg. Its tree is identical to the `release/v0.6.5` head `67f45b74` (tree `a573b70c`).
**Isolation:** every binary ran with `env -i HOME=<scratch>/home TMPDIR=<scratch>/tmp`, so the library started empty and nothing touched Jang's library or config. A 150–300 s alarm guarded every run.
**Kit:** an APFS clone of the Untitled UI PRO v8.0 kit. The original's `stat -f '%z %m %l'` is identical before and after (`kit-stat-*.txt`).
**Not used:** no client file, no agent CLI, and no clipboard.

| # | Item | Result | Evidence |
|---|---|---|---|
| 9 | Versions | **0.6.5** from `designos`, `designos-studio`, `designos-mcp` and `designos-diff --version` | `09-versions.txt` |
| 9 | About | The in-app About prints `Version {CARGO_PKG_VERSION}` (`home/dialog.rs:216`), the same constant as `--version`. Home shows the badge **v0.6.5** (`01-home-fresh-library.png`, top left). I did not open the About dialog (it needs a menu click). The dry run stamps the bundle `Info.plist` as `0.0.0-dev.451e42b`, as `docs/releasing.md` describes; a tag build stamps 0.6.5. | `01-home-fresh-library.png` |
| 1 | Fresh library, starter (#1212) | `--home` on an empty HOME seeded `.starter-seeded` and one `.designos` file. Home shows **Getting started** and **DESIGN-OS Studio Starter · In Getting started**. *Open starter sample* is a click, not run in GUI; the automated test `ac10_help_open_starter_sample_creates_numbered_copies_in_current_project` covers it. | `01-home.txt`, `01-home-fresh-library.png` |
| 2 | No file argument opens Home (#1212) | A real launch was not run (it needs an interactive window). `--screenshot` without a file shows an Untitled canvas by design (`cli.rs:143` `wants_home` excludes screenshot runs), see `02-no-file-arg.png`. Automated tests: `ac02_no_arg_launch_opens_home_and_never_builtin_demo`, `first_launch_shows_the_home_screen_with_the_getting_started_project`. | `02-*` |
| 3 | Agents: zoom scrollback, Codex Working→Done, approval banner (#1171, #1194, #1195) | **NOT RUN in GUI.** It needs a live Codex, Grok or Antigravity CLI, and agent CLIs are not run on this Mac. The `--panel agents-scrollback` and `agents-approval-background-tab` previews show only the default agent picker in a release build (`03-agents-pair.png`), so they are not evidence. Automated tests: `ac01_fake_cli_mode_a_and_mode_b_scrollback_on_zoom`, `ac02_scrollbar_drag_click_and_thumb_across_5000_lines_and_min_size`, `codex_turn_end_pure_decay_and_precedence_rules`, `active_tab_approval_shows_show_prompt_and_focuses_terminal`, `background_tab_approval_shows_banner_and_open_tab_switches_to_waiting_tab`, `keyboard_tab_and_enter_reach_show_prompt_and_open_tab_with_accessible_labels`. | `03-*` |
| 4 | Canvas labels cut like Figma (#1197) | Partly: the kit canvas renders. A capture zoomed to show cut frame labels was not taken. | `08a-select-button.png` |
| 5 | Kit import: icons in instances (#1242), styles (#1165) | The kit `.fig` imports and renders: 864,170 layers on 113 pages, and the chevron icon inside the Button instance is crisp at 736%. A resized-instance icon and the style picker were not checked in GUI. | `08a-select-button.png`, `08-select.txt` |
| 6 | Command-palette auto-fix (#993) | **NOT RUN in GUI** (click flow). Automated tests: `fix_all_is_one_undo_step_and_a_contrast_fix_outranks_a_bind_it_made_stale`. Controller bench on the kit: the record loop takes 179 and 190 ms. | — |
| 7 | Enter a path with `~/` and `file://` (#1192) | **NOT RUN in GUI** (typing flow). Automated tests: `typed_import_path_expands_tilde_file`, `path_from_file_url`, `expand_tilde`. | — |
| 8 | `--select name:` / `--zoom fit` / miss (#1127) | `name:Button` matched 166 layers and selected id 27511, zoomed to fit (736%), exit 0, PNG written. `name:__no_such_layer__` gave "matched no node (searched 864170 layers on 113 pages…)", **exit 2, no PNG**. | `08-select.txt`, `08a-select-button.png` |
| 8 | DTCG one mode (#1140) | `--export-variables … --format dtcg --mode "Light mode"` exit 0. One file, 697 tokens, aliases inlined from `_Primitives` and `_Primitives (2)`. **All 160 `{…}` aliases resolve inside the file** (checked by script). | `08c-export.txt` |

Found during the smoke: none blocking. A `--screenshot` run that edits the document never exits on "Save changes?" (#1274). It has been present since 0.6.1 and was hit by the dev `--panel` presets only.

## Round 2 (2026-10-27 21:00 – 2026-10-28 07:35 UTC+7), after QA's review

Same build (`451e42ba`), same isolation, the kit as an APFS clone (`cp -c`, proof below). One Studio window at a time.

**Jang's rulings:**
- 21:13: items 3 and 4's agent rename are waived. Agent CLIs are not run against his real home.
- Later (via AskUserQuestion): the click-only checks are waived and 0.6.5 is tagged on QA PASS. The waived checks are About, *Open starter sample*, *Enter a path…* and auto-fix + Undo. Studio's egui window reports no accessibility focus, so the GUI driver refuses its clicks.

| # | Item | Result | Evidence |
|---|---|---|---|
| — | Kit is an APFS clone | `cp -c` exit 0 (`clonefile`, which fails instead of copying). Free space dropped by 8 KiB for a 336 MB file. The original's size, mtime, link count and inode are unchanged. | `B0-kit-apfs-clone.txt` |
| 2 | Launch with no file argument opens Home (#1212) | **PASS.** A real launch, with no file argument and no `--screenshot`, opened the Home window: Recents, *Getting started*, *DESIGN-OS Studio Starter*, and the badge **v0.6.5**. | `A2-real-launch-no-file-home.png` |
| 4 | Frame labels cut like Figma (#1197) | **PASS.** The kit's *Desktop*/*Mobile* row at 10 %: the narrow mobile frames' labels read `Mo…`, and the wider desktop frames show `Desktop` in full. | `B4-kit-frame-labels-cut.png` (`--select fig:1673:402973 --zoom 10`) |
| 5 | Icon in a resized instance (#1242) | **PASS.** `chevron-selector-vertical`, resized to 12 × 12 in a dropdown (`id:27753`, panel *Changed: size, layout*), keeps the main component's shape: two chevrons with round caps and the stroke scaled in proportion. The main is 24 × 24 (`id:6403`). | `B5a-resized-icon-instance-12px.png` (1600 %), `B5a-icon-main-component-24px.png` (800 %) |
| 5 | No blank-named style in the style picker (#1165) | **PASS.** The effect style picker lists 26 effect styles, all named: *Backdrop blurs/…*, *Focus rings/…*, *Shadows/…*. The raw id `1186:100350`, which #1165 reported, is not listed. | `B5b-effect-style-picker.png` (`--panel effect-styles`) |
| About · 1 *Open starter sample* · 6 auto-fix + Undo · 7 *Enter a path…* | **WAIVED** (click flows, Jang's ruling above). | Tests green on `67f45b74`: `ac10_help_open_starter_sample_creates_numbered_copies_in_current_project`; `fix_all_is_one_undo_step_and_a_contrast_fix_outranks_a_bind_it_made_stale`; `typed_import_path_expands_tilde_file`, `path_from_file_url`, `expand_tilde`. About prints `Version {CARGO_PKG_VERSION}` (`home/dialog.rs:216`); every binary's `--version` prints 0.6.5 (`09-versions.txt`), and Home shows v0.6.5 (`A2-…`). |
| 3 · 4's agent rename | **WAIVED** (Jang, 21:13; tests listed in the #1250 comment). | — |
