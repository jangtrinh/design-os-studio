# #1127 carry: the CLI addresses a node by id or Figma id

Branch `jangtrinh/carry-1127-cli-node-address`, based on `origin/release/v0.6.4` (`6f0c3bb2`).
Spec: `plans/specs/ui-revamp/detail/1127-cli-addresses-nodes-by-id-and-figma-id.md` on `origin/pm/spec-ui-revamp`.
Carried: draft PR #1145 by NamAIEngineer (11 commits, +313/-47), replayed with its author kept and a
`Co-authored-by` line on each commit. Our `fix(cli)` commits close the gaps listed below.
Code tip the gates ran on: `24b44ecd`. The report commit on top of it changes no code.

## Commits

| # | Commit | What it is |
|---|---|---|
| 1 | `3c014c97` test(cli): acceptance tests | The spec cases, written first. Red on the base |
| 2-12 | NamAIEngineer's 11 commits | #1145 as written, replayed onto the release base (4 trivial conflicts: two `mod` lines, the `USAGE` constant, the end of the test module) |
| 13 | `22eaab06` fix(cli): resolve a --select name before a bare number, ... | Rules 4, 5, 6, 7, `SelectError`, one entry point. Keeps two of #1145's resolver tests |
| 14 | `863d6717` fix(cli): reject an empty --select and --select with --home ... | Parse-time rules, usage text |
| 15 | `138f6452` fix(cli): a --select miss records its message, ... | Miss record, no PNG, `exit_code`, `--edit-text` miss |
| 16 | `9b1ae1d1` fix(cli): --inspect-json reports the selection in page coordinates ... | Page coordinates, key only with `--select`, pure `inspect_json` |
| 17 | `24b44ecd` test(cli): cover the --edit-text capture miss ... | One more test, and the zoom-fit measure worded as the spec words it |

## Review of #1145 against the spec

| Spec item | #1145 | Closed in |
|---|---|---|
| One resolver for GUI and agent CLI; `resolve_select_node` and the inline GUI lookup deleted | Partly. One resolver and both call sites, but a `resolve_select_node` wrapper stayed in `main.rs` and errors were plain `String` | 13: `SelectError` (Display), `select_target::resolve_flag`, wrapper deleted |
| Grammar `id:` `fig:` `name:`, bare, `none` | Met | |
| Name order: page, screen, any layer in page-walk order, then case-insensitive | Met | |
| Rule 4: a bare number is a layer name first, then a node id, with a stderr note | **Not met, inverted.** Node id first, with a test asserting that (`bare_numeric_id_precedes_layer_name`); the PR body says it was chosen on purpose ("preserve the existing bare numeric ID precedence") | 13. The spec wins; the contributor's test is dropped |
| Rule 5: a miss is an error that names the selector and what was searched | Partly. An error, but `No node matches --select 'X'`: no counts, no hint | 13 |
| Rule 5: agent CLI returns `Err` before any action (apply-spec, add-comment, export-spec, codegen, inspect-json) | Met | |
| Rule 5: `--screenshot` records the miss, exits 2, writes no PNG, `main` prints the message | Partly. A bare `AtomicBool` with no message, `Close` sent from `build`, the text printed by the shell | 15 (reuses the closed-before-drawing path that a file that fails to open already takes) |
| Rule 5: interactive launch keeps the window, prints the message, shows the short toast | Partly. The toast held the long message | 13, 15 |
| Rule 6: expand lazy pages once, only after a miss; message says `(all pages expanded)` | Partly. Expanded every page on a unique `fig:` hit and retried by recursion for as long as the hydrator reported a change | 13 |
| Rule 7: several same-named layers: first wins, note names the chosen id; duplicate `fig:` lists up to 10 ids | Partly. The note lacked the id and only the agent CLI printed it. The duplicate error was met | 13 |
| Rule 8: a page selector switches page and selects nothing | Met | |
| Rule 9: `--inspect-json` `selection` in page coordinates; screens carry `source_id` | Partly. `source_id` met. `x`/`y` came from the parent-relative transform. `"selection": null` printed on every `--inspect-json` | 16 |
| Rule 10: `--zoom fit` frames the selection; default framing unchanged | Met, as `Options.zoom_fit: bool` plus `App.startup_fit_selection` rather than the spec's `StartupZoom` enum | Kept, see Assumptions |
| `--select ""` is a parse error | Missing (failed later, at run time) | 14 |
| `--select` with `--home` is a parse error | Missing | 14 |
| `--edit-text` miss exits 2 in a capture run | Missing | 15 |
| Pure `exit_code(select_miss, shot_ok)` | Missing | 15 |
| `USAGE` and option docs carry `fig:<figma id>` and `--zoom PERCENT|fit` | Partly (`fig:<id>`, no option lines) | 14 |
| `select_target.rs` under 200 lines | Met (160) | Still met: 185, `SelectError` in `select_target/error.rs` |
| Changes the spec does not ask for | `--select` is resolved before `--import-capture`, so a selector cannot name a layer the same command imports. `"selection": null` on a plain `--inspect-json` | Second fixed in 16. First kept: spec rule 5 says "before any action runs" |

## Acceptance table

All 15 spec section 10 names pass at `24b44ecd`:
`cargo test -p designos-app --bin designos --locked select` ran 205 tests, 204 passed. The one failure is
`terminal::tests::prompt_hooks_print_only_what_each_cli_can_read_and_never_the_selection`, which has nothing to do with
this change: it also fails under the same filter on the tests-only commit `3c014c97` and on #1145 alone, passes alone, and
passes in the full bin run. It is not one of #996, #1179, #1211.

| AC | Names | Red | Green |
|---|---|---|---|
| AC-01 | `resolve_by_id_fig_and_name`, `bare_number_is_a_name_first_then_an_id`, `fig_id_duplicates_are_an_error`, `miss_hydrates_lazy_pages_once` (+ `name_order_prefers_page_then_screen_then_walk`) | Base: `error[E0432]: unresolved import crate::select_target`. On the carried tip (#1145 only): `bare_number_is_a_name_first_then_an_id` FAILED, `miss_hydrates_lazy_pages_once` FAILED, `a_miss_says_what_was_searched` FAILED, both hint tests FAILED; `resolve_by_id_fig_and_name`, `name_order_...`, `fig_id_duplicates_are_an_error` ok | all ok |
| AC-02 | `export_spec_with_unknown_select_fails_and_writes_nothing`, `apply_spec_with_unknown_select_leaves_the_file_unchanged`, `codegen_and_add_comment_with_unknown_select_fail` | Base: all FAILED (the export command exits 0 and exports node 2, the first screen). On the carried tip: ok. The end-to-end `an_unknown_select_exits_2_says_what_was_searched_and_leaves_the_file_alone` FAILED on the message text | all ok |
| AC-03 | `inspect_json_reports_the_selection_and_screen_source_ids` (in-process) and `inspect_json_with_select_reports_the_selection_in_page_coordinates` (binary) | Base: FAILED. Carried tip: `x: 380 y: 140` (parent-relative) instead of `480, 200` | ok |
| AC-04 | `gui_select_by_id_selects_the_node`, `zoom_fit_frames_the_selection`, `screenshot_exit_code_for_a_select_miss` (+ `gui_select_miss_is_recorded_for_exit`) | Base: first two FAILED, the other two do not compile (`cannot find function exit_code`, `no field select_miss`). Carried tip: first two ok | all ok |
| AC-05 | fmt, clippy, tests, zero-fake gate | | see Gates. Partial: `--lib` plus the app bin and integration tests, not `cargo test --workspace` in full |

Red logs (scratchpad): `red-base-compile.log` (compile errors on the base, 9), `red-base-bin.log` and `red-base-e2e.log`
(buildable tests on the base: 15 and 8 failures), `red-carried-bin.log` and `red-carried-e2e.log` (#1145 alone: 8 and 5 failures).
All under `/private/tmp/claude-501/-Users-jangtrinh--claude-profiles-work-spares-spare-eMjL4g/4a19d7c5-37f7-497b-bf78-6cbf99e83d54/scratchpad/`.

## Gates (on `24b44ecd`)

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | exit 0 |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | exit 0 |
| `cargo test --workspace --locked --lib` | exit 0, 12 test binaries ok (633, 245, 136, 121, 55, ... passed) |
| `cargo test -p designos-app --bin designos --locked` | 3366 passed, **1 failed**, 11 ignored: `shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one`, a known load flake listed in #996. Rerun alone 3 of 3 ok. The full run took 878 s next to other builds |
| `cargo test -p designos-app --test cli_select --test cli_screenshot --test cli_version --locked` | 8 + 1 + 7 passed |
| `python3 scripts/zero_fake_ui_gate.py` | PASS 17/17 |

## Visual evidence

Document built from `two-cards.spec.json` with `--apply-spec`, exported with `--export-fig` and reopened as a `.fig`
(a Studio-built `.designos` has no Figma ids). `--inspect-json` on the `.fig`: `Card B` is Studio id 5, Figma id `1:5`, page position 424, 40, 360 x 220.

- `--select id:5 --zoom fit --size 1440x900 --no-toast --screenshot`: `.../scratchpad/evidence/sel-id-5-fit.png` (exit 0). Card B selected, framed, inspector shows Card B at X 424, Y 40, 360 x 220.
- `--select fig:1:5 --zoom fit ...`: `.../scratchpad/evidence/sel-fig-1-5-fit.png` (exit 0), identical frame.
- Unknown id, `--select id:99999 ... --screenshot sel-miss.png`:
  `designos: --select "id:99999" matched no node (searched 6 layers on 1 page by id; use --inspect-json to list screens, or id:<n> / fig:<id>)`, exit 2, no PNG written (`ls` shows none).
- `--select "Card Z" --apply-spec two-cards.spec.json` on the `.fig`: `designos: --select "Card Z" matched no node (searched 6 layers on 1 page by name; ...)`, exit 2.

## Not verified

- Spec section 11 timings (`--select fig:<id> --inspect-json`, and the hydrate-on-miss cost) on the kit: the kit is off limits here.
- US-15 manual step 2 on a real kit `.fig` (`fig:<real id>`, `id:99999`, `sha256sum` before and after): same reason. The synthetic round trip stands in.
- The interactive-launch miss toast as an image. The simulated-window test checks its text and the error flag; no screenshot shows it.
- `cargo test --workspace --locked` in full (AC-05): only `--lib`, the app bin and the app integration tests ran.

## Assumptions

- `StartupZoom` enum not introduced (spec rule 10 names it): it would touch five preset `startup_zoom = Some(...)` sites in `shell.rs`, outside the node-addressing paths and in a file #1141 is changing. `zoom_fit: bool` behaves the same; `--zoom 200 --zoom fit` and the reverse resolve last-wins.
- `--select` with `--home` is the explicit flag only. Rejecting an implicit home (no file, no fixture) broke the existing test `parses_menu_and_select_none`.
- `Target.node` stays `Option<NodeId>` and `Target.page` `Option<NodeId>` (#1145's shape); a page selector yields the page as `node`. The spec's `page: NodeId` would be a no-op rename.
- `none` stays case-insensitive, as in #1145 and the old agent CLI.
- The resolver tests moved to `select_target/tests.rs`; the two kept from #1145 were adapted (`char` literals, `to_string()`).
- Expected conflict point with #1141: the screenshot exit block at the end of `main()` and the `Shot` creation in `Shell::build`.
