# Release notes report: `release: 0.7.5`

Branch `jangtrinh/r075-release-notes`, checked out from `origin/release/v0.7.5` (2049158d) and rebased onto `ff2bd684` (#1378 merged while this was in progress). Pattern: PR #1283 (0.7.0).

## What changed

- `Cargo.toml`: `[workspace.package]` version 0.7.0 -> 0.7.5. `Cargo.lock`: 13 workspace crate version lines only (`cargo update -w`), nothing else.
- `CHANGELOG.md`: `## [0.7.5] - 2026-10-30` above 0.7.0. Summary paragraph with the milestone link (milestone 57, "v0.7.5 UI Revamp slice 1"), `### Changed` by area, a short `### Fixed`, and `### Known issues` copied from 0.7.0 with one edit: the Windows advisory line now says 0.7.5.
- `README.md`: lead says the newest is 0.7.5, new `### 0.7.5` entry (anchor `new-in-v075`) above 0.7.0, Windows advisory line says 0.7.5. The other Known issues lines are unchanged.
- `docs/index.html`: lead says the newest is 0.7.5, new `#new-in-v075` article above 0.7.0 (same six bullets as the README).
- `docs/assets/v2-new.css`: grid placement only. 0.7.5 takes the full first row (3 bullet columns); every older release moves down one row, columns and bullet layouts unchanged.
- Download links and `<small>` sizes: not touched.

## PR coverage

| PR | Issue | Where it is described |
| --- | --- | --- |
| #1343 | #1142 | Changed: Colours (palette, inline auto-layout editor field box) |
| #1349 | #1143 | Changed: Type, spacing and control sizes (scale, Dev Mode status badge, export scale chevron target) |
| #1350 | #1138 | Changed: Home |
| #1369 | #1139 | Changed: Controls; Fixed: disabled controls dim once |
| #1375 | #1146 | Changed: Popovers, dialogs and the command bar; Fixed: command bar vs toolbar, comment menu button, placeholder grey |
| #1376 | #1119 | Changed: Window chrome; Fixed: import toast `Details`, left panel resize band |
| #1377 | #1120 | Changed: Pages and Layers |
| #1378 | #1138 | Changed: Home (merged as `ff2bd684`; `+ Design file` and the Home layout claims re-checked on the merged tree) |
| #1380 | #1155 | Changed: Assets, Variables, Libraries and Check designs; switch click band under Controls (open at report time) |

No bullet: #1355, #1358, #1372, #1322 (test and gate only), #1286, #1305, #1309 (website badges), #1314 (merge).

## Acceptance

1. Rebased onto `ff2bd684` after #1378 merged. `git diff --stat origin/release/v0.7.5..HEAD` lists only: `CHANGELOG.md`, `Cargo.lock`, `Cargo.toml`, `README.md`, `docs/assets/v2-new.css`, `docs/index.html`, `plans/reports/r075-release-notes.md`, `plans/reports/r075-release-notes-evidence/*` (11 files; the exact output is in the hand-back message).
2. `cargo metadata --format-version 1 --no-deps --locked` prints `{'0.7.5'}`.
3. `cargo fmt --all -- --check` exits 0.
4. `python3 -B -m unittest discover -s scripts/tests -p "test_*.py"`, rerun on the rebased tree: `Ran 158 tests in 73.986s`, `OK`. The lines `Chrome colour literal gate FAILED ...` in that output come from the gate's own self-tests, which expect a failure.
5. Visual proof, 1440 wide, headless Chrome driven over CDP (the page reveals sections on scroll, so a plain `--screenshot` of the page shows empty sections): `plans/reports/r075-release-notes-evidence/whats-new-before-1440.png`, `whats-new-after-1440.png` (whole What's new section), `new-in-v075-1440.png` (the 0.7.5 article only). In the after image 0.7.5 sits in a full-width first row with six bullets in three columns, and 0.7.0 down to the folded 0.5.0 / 0.4.0 / 0.3.0 rows keep their order with no overlap.
6. One commit `release: 0.7.5`, no AI trailer.

Copy checks: 13 backticked spans, each a string on screen (list with source hits in `plans/reports/r075-release-notes-evidence/backtick-spans-grep.txt`). A grep of the three new sections for `Untitled`, leg or spec letters, `R1`/`R14`, `egui`, `x_band`, `layout_unchanged`, `widget kit`, file paths and PR numbers of gate-only PRs finds nothing.

## Left out, and why

- #1349 label fixes (Sessions button, resume notice, agent picker card, export format labels, text prefixes of fields, effects menu above the inspector footer, Dev status buttons): they correct cut-offs the new type size itself would cause, not 0.7.0 bugs.
- #1349 "canvas frame labels keep 11 px / 450": it says nothing changed.
- #1343 "no window or popup shadow" in the theme mapping: the PR gives no user-visible description.
- #1369 "screenshot hover tips use the same frame as a live hover tooltip": unclear whether this means the `--screenshot` flag, so left out.
- #1375 "presentation chrome" and "hidden updater": no user-visible description or on-screen name. "Decks and Plugins buttons look as before" is left out because #1380 restyles Plugins.
- #1376 "the Actions palette": no confirmed on-screen label (the Cmd+K bar says "Search actions"), so only "shell menus" is named.
- #1378 "category icon tiles and the empty state keep today's look until the kit lands" (from #1350): unknown whether #1378 changed them.
- All "Known follow-ups" in the PR bodies, gate and allow-entry changes, baselines, review rounds.

## Concerns

- #1380 is still open (#1378 merged). When #1380 merges, `origin/release/v0.7.5` moves and this branch needs another rebase; it should be clean because #1380 touches none of these files. Compare the Assets / Variables / Libraries / Check designs bullets and the switch-band bullet against #1380's merged body. The copy was written from the PR bodies and diffs (`+ Design file`, `Create variable`, `New`, `Changed`, `Missing from library` found in the #1380 / #1378 diffs and on HEAD). If either changes before merge, re-check the Home bullets and the Assets / Variables / Libraries / Check designs bullets.
- The 0.7.0 Known issues line "Not in 0.7.0: Codex approvals ... reverted before release (#1284)" is carried forward unchanged as instructed. It reads oddly in 0.7.5 notes. `agent_codex_cards` is not on this branch, so #930 is still not shipped. Controller may drop that line.
- Other 0.7.0 Known issues are still open on GitHub (checked with `gh issue view`: #930, #1173 (v0.7.1), #1277, #1278, #1279, #1097, #1074, #1066, #1048, #1047, #1059, #1101, #1096, #1073, #1158, #1102, #834, #977); none of the 0.7.5 PRs touches them.
- Pixel values (12 px text, 6/8/12/16 px radii, 32 px rows, 36/40 px buttons, 240 px and 264 px widths) come from the PR bodies, not from a render here.

## Unresolved questions

None blocking.
