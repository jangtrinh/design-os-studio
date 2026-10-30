# 0.7.5 smoke evidence (controller run, for QA's verdict)

**Build:** Release dry run [40936477664](https://github.com/jangtrinh/design-os-studio/actions/runs/40936477664) on PR #1384 head `6d6faed7` (`release/v0.7.5` `2b69fb31` plus the version bump and release notes). All three platforms built. Artifact `designos-0.0.0-dev.6d6faed-macos.dmg`, sha256 matches the artifact's `.sha256` (`00-artifact-sha256.txt`). The bundle holds `designos`, `designos-studio`, `designos-mcp` and `designos-diff`, and all four print 0.7.5 (`01-versions.txt`).

**PR head since the dry run:** `c4f89d40` adds one commit that rewords the release notes only (`git diff --stat 6d6faed7 c4f89d40`: `CHANGELOG.md`, `README.md`, `docs/index.html`). No code or build file changed. `Test (macos-14)`, `Clippy` and `Rustfmt` pass on `c4f89d40`.

**Isolation:** the layout harness (`scripts/layout_baseline_capture.py`) ran every window with `env -i HOME=<scratch> TMPDIR=<scratch> PATH=<stubs>:/usr/bin:/bin`, with agent CLIs replaced by logging stubs (`04-capture-summary.txt`: every spawn logged a stub line).

**Documents:**
- The Untitled UI kit through an APFS clone (`cp -c`). The original's size, mtime and inode are unchanged before, after the clone and after the run. The clone's sha256 is the same before and after the run (`02-kit-apfs-clone.txt`). The harness works on its own per-case clones.
- No client file. No `--fixture`.

**Windows:** one at a time inside the harness, started after Jang had been idle 9,143 s (`03-gui-run.txt`), 515 s wall time.

**Kit captures stay local** (paid kit, never uploaded): `/Users/jangtrinh/Products/design-os-studio/plans/261025-1247-untitled-ui-dark-revamp/proof/qa075-dmg-captures/` holds all 74 PNGs, their `.layout.json` files and `qa075-dmg-contact-sheet.png`. Per-surface before/after sheets for each PR are in the parent `proof/` folder (`pr1375-J`, `pr1376-D`, `pr1377-E`, `pr1378-H`, `pr1380-K`).

**Jang's ruling (2026-10-30 07:08 UTC+7, via AskUserQuestion in the controller session):** "Waive them (Recommended)" for the click and keyboard items listed under WAIVED below, the same rule as 0.6.5 and 0.7.0.

| # | Change (CHANGELOG 0.7.5) | Result | Evidence |
|---|---|---|---|
| 1 | Version 0.7.5 | PASS. Four binaries print 0.7.5; the Home badge reads `v0.7.5`. | `01-versions.txt`, `05-home-version-badge.png` |
| 2 | Layout unchanged on every surface | PASS. 74 captures from the dry-run app on the kit clone, compared with the blessed baseline `dd53ba7a` and `allow.json`: G6 0 mismatches. `x-board` and `x-deck` are Tier A only (blank board and deck show no kit content). | `04-capture-summary.txt`, local captures |
| 3 | Colours (#1142, PR #1343) | PASS. Dark neutral chrome, orange brand action in every capture. Tests: `theme::tests::chrome_contrast_pairs`, `widgets::kit::table_tests::every_table_row_matches_its_look`, `theme::tests::no_chrome_colour_is_purple`. | `d-editor-default`, `h-home` |
| 4 | Type, spacing and control sizes (#1143, PR #1349) | PASS. Tests: `theme::tests::type_roles_are_the_text_steps`, `only_three_ui_weights_exist`, `nested_radii_are_concentric`, `control_heights_are_the_button_and_badge_sizes`; Dev status badge `shell::tests::dev::a_top_level_frame_shows_its_status_once_in_the_label_badge_and_not_as_a_dev_pill`. | `d-editor-default` |
| 5 | Controls (#1139, PR #1369) | PASS. Kit checkbox, switch and fields visible. Tests: `widgets::kit::state_tests::*`, `focus_tests::focus_ring_is_two_px_outside_with_two_px_gap`, `painter_menu_tests::submenu_chevron_uses_the_item_icon_at_rest`. | `k-variables`, `k-libraries`, `j-check-designs`, `d-menu-main-file` |
| 6 | Window chrome (#1119, PR #1376) | PASS. Title bar, tabs, rail, toolbar, rulers, shell menus. Tests: `tabs::tests::tab_bar_paints_only_theme_roles`, `tab_close_hit_box_meets_the_target_floor`, `canvas::rulers::tests::a_press_and_release_in_the_press_band_drops_no_guide`. | `d-window-chrome`, `d-editor-default`, `d-menu-main`, `d-menu-zoom` |
| 7 | Popovers, dialogs and the command bar (#1146, PR #1375) | PASS. Tests: `shell::tests::overlays_chrome::cmdk_palette_uses_contract_roles` (36 px rows), `overlays_chrome_layout::command_bar_shows_hover_apart_from_the_selected_row`, `dialog_body_text_is_text_sm`, `context_menu_and_layer_picker_rows_are_menu_items`. | `j-cmdk`, `j-find`, `j-comments`, `j-version-history`, `j-palette`, `micro-layer-picker` |
| 8 | Pages and Layers (#1120, PR #1377) | PASS. 240 px panel, section order and control places held by G6. Tests: `shell::tests::left_panel_actions::left_panel_actions_are_32_and_keep_their_right_edge`, `panels::layers::tests::row_states_use_tree_row_roles`. | `e-no-selection`, `e-layers-selected` |
| 9 | Home (#1138, PRs #1350 and #1378) | PASS. `+ Design file` orange with a dark label; 264 px sidebar, header, filter bar, grid, Trash in place. Tests: `shell::tests::home::home_chrome_uses_studio_theme_colours`, `home_layout_regions_unchanged`. | `h-home`, `05-home-version-badge.png` |
| 10 | Assets, Variables, Libraries, Plugins and Check designs (#1155, PR #1380) | PASS. Assets search on a neutral border, `New` status chips, value cells clean at rest, `Create variable` centred, rule checkboxes. Tests: `shell::tests::assets_variables_libraries_chrome::*` (search outline and ring, value cells, boolean switch, alias pill size, popover cross and checkboxes, dialog sizes, `review_change_labels_are_status_chips`, rule checkboxes, segmented contrast). | `k-assets`, `k-variables`, `k-variables-floating`, `k-libraries`, `j-plugins`, `j-check-designs` |
| 11 | Fixed bullets | PASS by test: `overlays_chrome_layout::command_bar_stays_above_the_toolbar_at_the_minimum_window`, `comment_message_dots_stay_clear_of_the_message_text`, `import_path_prompt_paints_its_hint_as_a_placeholder`, `shell::tests::app_shell::the_import_toast_shows_details_once_as_a_button`, `left_panel_resize_hit_band_meets_the_target_floor`, `widgets::kit::painter_minor_tests::labelled_painters_dim_once_inside_a_disabled_ui`. | — |
| — | Focus ring 2 px; switch 24 px hit band; ruler press band; page-row click keeps the keyboard on the canvas; command bar small-window scroll and Enter runs the keyboard pick (code read: `cmdk.rs` moves the pick only on arrows, a new query or a tab click, and Enter runs the picked row; no test presses Enter); import toast `Details` once; left panel resize band; disabled controls dim once; inline auto-layout editor box; Dev status badge; export chevron target; Variables edit popover and alias pills; Libraries `Changed` chip | **WAIVED (click and keyboard flows)** under Jang's ruling. Covered by their tests, all green in the controller gate on the final code (`designos-app` 3775 passed, 0 failed, on `a4e28d9f`, whose tree equals `release/v0.7.5` `2b69fb31`). | — |

## Release notes checked against the code
A bullet-by-bullet scout against tests and source found claims the code does not back. Commit `c4f89d40` removed or reworded them before this smoke:
- The tab bar still shows the green Dev ready badge (`tabs.rs`), so "drops the green Dev badge" is gone.
- The Libraries dialog labels a deleted asset `Removed` and only the update review says `Missing from library`, so the notes name `New` and `Changed` only.
- Plugins has no rule checkboxes, so that bullet names Check designs only.
- The Assets search ring shows whenever the field has focus, not only on keyboard focus.
- No check exists for the dialog field heights, Home's 14 px body text or "orange only for live state", so those claims are gone.

## Not 0.7.5 regressions
None found in this run.
