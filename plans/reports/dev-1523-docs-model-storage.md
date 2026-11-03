# Dev report: #1523 Docs model + storage crate (phase 01, epic #1522)

Branch `feat/1523-docs-model-storage` off `origin/main` 53860a64. Not pushed, no PR (controller does that after review).
Files touched: new `crates/designos-docs/**`, workspace `Cargo.toml` (member + path dependency), `Cargo.lock` (additive:
pulldown-cmark, sha2 and their tiny transitive deps). Plus this report and `plans/reports/evidence-1523/`.

## NOT verified / deviations (read first)

- **No red-first run.** Tests were authored before the implementation, but I wrote the implementation before running
  them, so there is no recorded red run. Substituted: mutation checks on the finished code (below). All three
  mutations were caught. This is a process deviation, not a result I can claim as test-first.
- **Skills not loaded.** The `Skill` tool does not exist in this runtime, so `es-feature-dev`, `es-rust`,
  `es-fable-thinking` were not loaded. I followed the phase file, the Zero-Bug Loop (attack, fuzz, mutate) and the
  Fable ledger by hand. The Phase 3 `/ak:cook` command was not run for the same reason.
- **Idle CPU of the watcher not measured.** It wakes every 400 ms and stats the folder. Phase 02 owns the 20 s
  `ps -o time=` gate with the tab open.
- **macOS only.** Ubuntu/Windows CI not run. `File::lock` (advisory lock, Rust 1.89+) is used; filesystems without
  locks fall back to the hash check only.
- **Canonical output is not perfectly stable** on pathological text (see Known limits). `render()` (what is written to
  disk for unedited blocks) was lossless in all 2,000,000 random cases.
- Latency of the watcher is asserted `< 1 s` in tests (10 consecutive full-crate runs green), not benchmarked.
- `cargo test --workspace --lib` skips `designos-app` (bin-only crate, no lib target; FACT: `ls` shows no `src/lib.rs`).
- Skills tool absent also means no `es-rust-gpu-canvas` question arose (no rendering here).

## Re-anchor against HEAD (before code)

No drift on the lines the phase cites: `store.rs:475` `fn project_dir` -> `<root>/projects/<id>` (matches);
`session.rs:38-44` `DiskWatcherInner` / `DiskWatcher` (matches). `DiskWatcher` is a background thread that sleeps on a
condvar and compares file mtime every 500 ms; the new watcher mirrors it (400 ms, directory scan). `Library::path` and
`Library::open` are public, so a test can create a project through the library and check the docs path (done).
Plan base says `origin/main 8f267b3`; actual HEAD is 53860a64; nothing relevant moved.

## Acceptance (commands run after the last edit, output in `plans/reports/evidence-1523/`)

| # | Result | Evidence |
|---|---|---|
| 1 Round-trip fixtures, byte-identical, unknown syntax kept | PASS. 12 fixtures (heading, inline paragraph, lists, checklist, callout, table, frame embed, mermaid, code, escapes, all-blocks with front matter, unknown syntax). `render()` equals input; `render_canonical()` also equals input for the 11 canonical fixtures. Every prefix of every fixture (~2,400 cuts) round-trips | `01`, `06` |
| 2 Manifest validation | PASS. Missing `groups` -> field `groups`, fix lists the five groups; bad pin group -> `pins[0].group`; also order, duplicates, parents, paths, unknown fields, schema version, bad JSON | `01` (`tests/manifest.rs`, 9 tests) |
| 3 Write conflict | PASS. Outside edit between read and write -> `Conflict { expected, found }`, file bytes unchanged; also deleted/appeared files, same-length edits, 8 racing threads -> exactly 1 winner. Writes via `designos_fonts::write_atomic` (tmp file + rename) under `docs/.docs.lock` | `01` (`tests/store.rs`, 14 tests) |
| 4 Watcher | PASS. Edit reported with its new hash within 1 s; wake callback runs on the watcher thread; caller only `drain()`s a queue; temp/hidden/other files ignored; own atomic write reported once; idle folder silent; thread stops on drop | `01` (`tests/watch.rs`, 7 tests) |
| 5 Gates | PASS. `cargo test -p designos-docs`: 0 + 9 + 19 + 14 + 7 tests, 0 failed. `cargo clippy -p designos-docs --all-targets -- -D warnings`: clean. `cargo fmt --check`: clean (workspace). `cargo test --workspace --lib`: every crate `ok`, 0 failed (720 designos-doc, 326 designos-mcp, 158 export, 143 fig, 56 render, 37 library, 30 fonts, 13 diff, 8 webcapture, 5 gpu, 1 eval, 1 svg) | `01`-`04` |
| 6 Markdown crate choice + fidelity check | `pulldown-cmark` 0.13 as span oracle with our own model/writer; comrak and pulldown-cmark-to-cmark each reproduce only 3 of 12 fixtures. Recorded with the table in `crates/designos-docs/README.md` | README |

Mutation checks (Zero-Bug Loop): (a) hash comparison disabled -> 5 store tests fail; (b) advisory lock removed -> the
concurrent-writer test fails 6 of 6 runs, passes 15 of 15 with the lock; (c) block gaps ignored on render -> CRLF and
prefix tests fail; (d) poll interval 2000 ms -> 5 watcher tests fail. Originals restored; final runs above.

## What the fuzzing found (all fixed, each now has a test or a soup piece)

1. Indented raw blocks lost their leading spaces (indented code read back as a paragraph in canonical output): block
   source now includes indentation.
2. Loose checklists (blank lines between items) were typed as bullet lists with literal `[ ]`: the task marker sits
   inside the paragraph of a loose item.
3. A heading whose text ends in `#` lost it on a second read (closing-sequence rule): trailing `#` is escaped on write.
4. Fence info strings were taken from the parser's unescaped copy: now read from the opening line as written.
5. Two adjacent lists merged into one when a rewritten list used the same bullet/delimiter as its neighbour: the
   canonical writer alternates `-`/`*` and `.`/`)`.
6. Generated text that starts with `---` would be read as front matter: a leading blank line is emitted.
7. Empty `Bold`/`Italic` (`****`) is kept as text; `*` vs `_` chosen so emphasis does not run into a literal `*`.

## Public API for phases 02, 04, 06

`Page::{parse, render, render_canonical, front_matter, headings, set_title/description/group, push/insert/remove_block}`,
`Block::{new, kind, kind_mut, source}` (`kind_mut` drops the kept text), `BlockKind`/`Inline` (11 kinds, `name()`),
`plain_text`, `Manifest::{new, parse, validate, to_json_pretty, group, group_mut}` + `Pin` + `Group` (`ALL`, `index`),
`DocsStore::{for_project, at, read_page, write_page, read_manifest, write_manifest, list_page_files}`,
`Expect::{Absent, Hash}`, `Versioned<T> { value, hash }`, `ContentHash` (SHA-256, hex `Display`/`from_hex` for MCP),
`DocsWatcher::{start(dir, wake), drain, recv_timeout}` + `DocChange { rel, hash }`, `DocsError` (every message names the
next action). Block ids for MCP (phase 06) are `Page::blocks` indices; not added here.

Additions beyond the five requirements, needed by 02/06 and small: `headings()` + `plain_text()` (On this page, search),
`list_page_files()` (pages on disk not in the manifest), `DocsWatcher::stopped_flag()` (test/diagnostic hook).

## Known limits (also in the crate README)

Canonical rewrite of pathological text can read differently on a second pass (stray `_` emphasis, backticks inside
running text, indented list lines continuing a paragraph); unedited blocks are never rewritten. Loose lists are written
tight. `__bold__` becomes `**bold**` only when that block is edited. Symlinked pages are not listed/watched. Pages over
8 MiB refused. Watcher misses a rewrite that keeps both size and mtime. Editors that save without `.docs.lock` can win
the microseconds between the last hash check and the rename.

## Decisions I made (technical, invisible to the user)

- Watcher polls on its own thread (like `DiskWatcher`) instead of adding `notify`: no new native dependency; swap later
  behind the same API if the 0% idle gate needs it.
- Manifest `groups` is an ordered array of `{id, pages}` (object keys would be written alphabetically by serde_json).
  Pins live in a top-level `pins` list; states require `parent_screen`, screens forbid it. Unknown fields are refused.
- No delete/rename/trash/conflict-copy helpers: trash lives in the library (`trash_dir`), phase 02 decides the wiring;
  a conflict copy is `write_page(<page>.conflict-<time>.md, .., Expect::Absent)`.

Assumption (confidence): phases 02/04 are fine with `Inline::Text` holding Markdown source rather than unescaped text
(high; `plain_text` covers display). Flips if the editor wants a rich-text model with escaping handled in this crate.

## Files

`crates/designos-docs/`: `Cargo.toml`, `README.md`, `src/{lib,error,hash,group,paths,model,front_matter,inline,markdown,markdown_structures,render,outline,manifest,manifest_parse,store,watch}.rs`, `tests/{roundtrip,manifest,store,watch}.rs`, `tests/fixtures/*.md` (12).
Evidence: `plans/reports/evidence-1523/` (text captures of test, clippy, fmt, workspace lib summary, 2M-case soup run, fixture list).

## Unresolved questions

1. Should the Project tab (02) treat `Raw` blocks as read-only source cards, or should 04 make them editable as source? Affects nothing in this crate.
2. Is a `.conflict-<time>.md` page expected to be listed in the manifest automatically (UI spec says "listed under Discovery")? That is a phase 02 manifest write; no helper here.

Round 0 status (superseded by the fix round below, which replaces the watcher, the idle CPU line, the "polls instead of
notify" decision and the "1 in 2M" claim):
Status: DONE_WITH_CONCERNS

---

# Fix round 1 (review of PR #1571: REQUEST_CHANGES)

Same branch `feat/1523-docs-model-storage`, started from 92e6714a (verified equal to origin). Files touched: `crates/designos-docs/**`,
`Cargo.lock` (notify and its platform crates), this report, `plans/reports/evidence-1523/r1-*`. No file outside that list.

## NOT verified / deviations (read first)

- **Red-first is recorded for most, not all.** Red runs before the fix are in `r1-01` (10 tests), `r1-02-*` (nested lists,
  `escape_plain`, `page_api`) and `r1-03` (3 watcher tests). Not recorded red: the 200k "touch a block, count unchanged"
  fuzz property and the extra tests added at the end (watch mode, start race, missing folder, linked specs, lock timeout).
  The fuzz property was written together with the fixes it drove; it found 9 distinct bugs while the fixes were being made
  (listed below) and each was fixed before the green 200k run, but no clean red-then-green transcript of the final property exists.
- **Skill tool still unavailable**, so the skills were again not loaded and Orca was not used (as instructed).
- **macOS only** for the native watcher (FSEvents). The inotify and Windows paths compile through `notify` but were not run.
  The polling fallback is tested on macOS only by forcing it (`start_polling`); the automatic switch when the native
  watcher breaks at run time (`Err` event) has no test (no portable way to make FSEvents fail).
- **Idle CPU is measured with `ps -o time=`** (10 ms resolution): "0.00 s" means under 10 ms in 20 s, the same method the
  CLAUDE.md invariant uses.
- **Files over 200 lines**: `model.rs` is now under 200 as asked, but `render.rs` (248), `front_matter.rs` (256), `markdown.rs`
  (239), `markdown_structures.rs` (235), `watch.rs` (235), `store.rs` (206), `manifest.rs` (202) are over the guideline.
  Not split in this round (review only asked for `model.rs`); say so if you want it.
- Known residue of the canonical writer (documented in the README): stray `_` emphasis next to other delimiters and
  stray backticks in running text can read differently on a second pass. Not seen in 1,000,000 soup cases, but the soup
  deliberately excludes those two shapes from the stability property.

## Findings and where they are fixed

| Finding | Fix | Test (file) |
|---|---|---|
| B1 table text after last cell lost; `\|` in cells | Table stays `Raw` when a row has source text after its last parsed cell; `\|` written as `\\|` in cells and code spans | `edit_blocks.rs` pipes, `soup.rs` |
| M1 adjacent lists merge | marker chosen to avoid both the previous and the next list; also: a rewritten list drops the indentation of a following block kept with indentation, a rewritten block under a table row gets a blank line, a list in front of an indented raw block stays raw | `edit_blocks.rs`, `soup.rs` touch property (200k cases green, 1M soup green: `r1-05`) |
| M2 nested lists | `ListItem { content, children }`, `ChecklistItem` likewise; children are nested lists and further paragraphs; continuation lines are re-indented under the item; unedited files stay byte-identical (`fixtures/nested-lists.md`) | `nested_lists.rs` (6) |
| M3 front matter failure | `Page::parse` is infallible; a bad value leaves the key unset and is listed in `FrontMatter::problems()`; body still parsed | `front_matter.rs` (7) |
| M4 watcher polls | `notify` 8.2 (FSEvents), poll fallback only if it cannot start or fails | `watch_robustness.rs`, `watch_idle_cpu.rs` |
| M5 escaping | `escape_plain`, `plain_inlines`, `paragraphs_from_plain`, `TextContext` | `escape_plain.rs` (5) |
| Phase 06 note | `Block::hash()`, `Page::block_hashes()` | `page_api.rs` |
| BOM before front matter | BOM kept, front matter found after it, not duplicated | `front_matter.rs` |
| Leading `---` rule | a page opening with a rule and no `key:` lines is a page | `front_matter.rs` |
| CRLF for edited blocks | `Page::crlf` from the first line ending; edited/inserted blocks use it | `edit_blocks.rs` |
| `remove_block` panic | returns `DocsError::BlockIndex { index, len }` with a fix | `page_api.rs` |
| `linked_specs` validation | empty, `..`, control characters, duplicates refused with field `linked_specs[i]` and a fix; absolute paths allowed (Studio's folder picker gives absolute paths; decision below) | `manifest.rs` |
| Case-insensitive duplicate pages | `Notes.md` vs `notes.md` refused (macOS/Windows are one file) | `manifest.rs` |
| Watcher read cap | files over 8 MiB (the store's limit) are never read or reported | `watch_robustness.rs` |
| First scan on the caller's thread | baseline scan runs on the watcher thread; files written at/after `start` are reported (start-time rule) | `watch_robustness.rs` (3 tests) |
| `stopped` after callback panic | drop guard on the thread sets the flag on unwind | `watch_robustness.rs` |
| Lock wait | 2 s `try_lock` loop, `DocsError::Busy` with a fix, `with_lock_timeout` | `store.rs` |
| `model.rs` > 200 lines | split into `model.rs` (types) and `page_edit.rs` (page operations) | n/a |

Bugs the new fuzz property found while fixing (each fixed, each has a soup piece or a test): indentation hiding a block
marker on a paragraph's first line; lazy-continuation indentation growing with each nesting level; `===` lazy
underline; HTML-start lines; a list range swallowing a link definition; a nested item outside the list's range; a
continuation line that is a bare bullet; a lazy table row that becomes a table once indented under an item; an item
range that starts with the marker's indentation (`  - b` was kept raw although it is a plain list).

## M4 numbers (200 pages, 20 s idle, `ps -o time=`, test `watch_idle_cpu`)

- BEFORE (400 ms polling thread, `r1-04-idle-cpu-BEFORE-polling.txt`): delta 0.05 s, test failed its gate.
- AFTER (`notify`, FSEvents, `mode() == Native`, `r1-04-idle-cpu-AFTER-native.txt`): before 0.02 s, after 0.02 s, delta 0.00 s.
  A second run printed 0.03 s / 0.03 s, delta 0.00 s. The gate asserts `< 0.01 s`; the phase 02 gate is not loosened.

Licence: `notify` is CC0-1.0 (its `Cargo.toml` says `license = "CC0-1.0"`); the other additions (`inotify`, `kqueue`, `mio`,
`objc2-core-*`, `walkdir`, `bitflags`, `libc`) are the usual permissive crates. The repo's licence register
(`docs/eval-licences.md` -> `THIRD_PARTY_LICENSES.md`, checked by `scripts/check_eval_licences.py`) lists datasets, weights,
kits, skills and fonts, not cargo crates (pulldown-cmark, sha2 and every other dependency are absent from it too), so no
entry was added. If the controller wants crates registered, that is a new convention, not something to do inside this PR.

## Acceptance (after the last edit; output in `plans/reports/evidence-1523/`)

| Command | Result | Evidence |
|---|---|---|
| `cargo test -p designos-docs --locked` | 82 passed, 0 failed, 1 ignored (the 20 s idle CPU test, run separately above) | `r1-06` |
| `cargo clippy -p designos-docs --all-targets --locked -- -D warnings` | clean | `r1-07` |
| `cargo fmt --check` | exit 0 | `r1-07` |
| `cargo test --workspace --lib --locked` | every crate ok, 0 failed | `r1-08` |
| `cargo metadata --locked` | exit 0 | `r1-08` |
| Fuzz, release: `DOCS_TOUCH_CASES=200000 DOCS_SOUP_CASES=1000000` | both properties pass | `r1-05` |
| Idle CPU, 200 pages, 20 s | delta 0.00 s, mode Native | `r1-04-*-AFTER-native` |

## Decisions I made (technical, invisible to the user)

- Absolute `linked_specs` stay valid: phase 07 reads spec folders chosen with the system folder picker, which are absolute.
  Refused: empty, `..` segments, control characters, duplicates.
- `Page::parse` never fails (it was `Result`): any text is a page; problems are data (`FrontMatter::problems()`).
- Watcher modes: `Native` by default, `start_polling` for network shares; `mode()` reports which. A baseline file whose
  modification time is within 10 ms of `start` is reported as a change (spurious at worst, never a miss).
- A missing docs folder is handled by watching its nearest existing parent and switching to the folder when it appears.
- Child blocks of a list item are limited to nested lists and paragraphs; code, quotes and tables inside an item keep the
  whole list `Raw` (lossless, just not typed).

## Corrected PR #1571 body text (for the controller to paste)

Replace the two claims the review found untrue and add the new surface:

> **What changed since review.** Lists nest (`ListItem::children`). Text typed into a block goes through
> `escape_plain` / `plain_inlines`. `Page::parse` never fails: front matter problems are data. Every block has a hash
> (`Block::hash`, `Page::block_hashes`) for block-level checks. The watcher uses the operating system's change
> notifications (FSEvents on macOS) and costs 0.00 s of CPU over 20 s idle with 200 pages; it falls back to a 400 ms timer
> only when the system watcher cannot start.
>
> **Verification.** 82 tests in the crate. Random Markdown soup: 1,000,000 cases, `render()` lossless and the canonical form
> stable. A second property (touching a block without changing it keeps the block count and kinds) ran 200,000 cases. The
> canonical writer is not claimed stable for text with stray `_` emphasis or stray backticks (README, Known limits).
> `linked_specs` entries are validated (not empty, no `..`, no control characters, each listed once); absolute paths are allowed.
>
> Remove: "about 1 in 2M canonical instability" and "linked specs validated" as they were worded before.

## Unresolved questions (round 1)

1. Should `render.rs`, `front_matter.rs`, `markdown.rs`, `markdown_structures.rs` and `watch.rs` be split under 200 lines in this PR or a follow-up?
2. Should cargo crates ever be listed in the licence register? Today they are not.

Round 1 status: DONE_WITH_CONCERNS (superseded by round 2: the re-review found a regression from this round).

---

# Fix round 2 (re-review of PR #1571: REQUEST_CHANGES, regression from round 1)

Started from 519ebeea (HEAD verified equal to origin). Same branch. Files: `crates/designos-docs/**`, this report,
`plans/reports/evidence-1523/r2-*`. Nothing else.

## NOT verified / deviations (read first)

- **N3 red could not be reproduced here.** The reviewer saw `an_oversized_file_is_not_read_into_memory_and_not_reported` fail
  10 of 10 alone; on this machine it passed 5 of 5 on the OLD code (`r2-02-oversized-alone-RED-old-code.txt`) and in the
  full red run. The race is timing-dependent (stat small, read returns the whole file). Replaced by what is deterministic:
  a unit test (`store::tests`) that the cap is applied to the read itself (file of cap and cap + 1 bytes), plus the loop
  of 10 alone-runs on the new code (10/10 green, `r2-02-oversized-alone-10x.txt`). I did not prove the old failure.
- **Linux (inotify) not run.** The delete / recreate and two-missing-levels tests pass on macOS; on Linux they run in the Ubuntu
  CI job. I will report that job's result once CI has run (not yet available at the time of this report).
- Red-first for N1, N2, N4, N5, N7, N8 is recorded in `r2-01-red.txt` (failing before the fix). The delete / recreate test
  passed on macOS before the fix (FSEvents keeps working); it exists for Linux. The N4 test failed (`recv_timeout` -> `None`).
- The deep comparison in the touch property skips pages that contain a carriage return (mixed line endings keep stray carriage returns in
  text); those are still compared by block name.
- The literal command `cargo test --test watch_idle_cpu -- --ignored --nocapture` needs `-p designos-docs` from the workspace
  root; I ran `cargo test -p designos-docs --locked --test watch_idle_cpu -- --ignored --nocapture`.
- Files over 200 lines stay as they are (controller ruling). `insert_blocks` / `move_block` are phase 06.

## Findings

| id | cause and fix | tests |
|---|---|---|
| N1 BLOCKER | The parser gives a list the blank line and indentation in front of the next block, so that block's kept gap has no newline; once the list is rewritten those characters are gone and the text glued on. `render_page` now uses the canonical gap after a rewritten block whenever the kept gap holds no newline (adopted from the suggested patch; it is the cause, not a workaround, and parsing is unchanged because moving the indentation into the gap broke canonical stability). Also an item whose text starts on the line after an empty marker stays raw (found by the new fuzz). | `touch_deep.rs::touching_an_indented_list_keeps_the_paragraph_after_it`; soup pieces for an indented paragraph, an indented `3.` list and an empty nested item; touch property now compares block kinds deeply (items and children) |
| N2 MAJOR | A child list that cannot interrupt a paragraph (starts at a number other than 1, or its first item is empty) is written after a blank line when paragraph text is directly above it (adopted from the patch with its condition on what is above). | `touch_deep.rs` (3 tests incl. 1,000,000-case nested soup) |
| N3 MAJOR | `store::read_capped` reads through `take(MAX + 1)` and returns `None` over the cap; the watcher uses it, and `read_limited` too. The watcher also skips a file that changed while it was read (stat, length and mtime must match after the read) and waits for the next event. | `store::tests`, `watch_robustness.rs` |
| N4 | `Native::follow_folder` re-arms on the nearest existing folder whenever it differs from what is armed, and when the armed folder's identity (device, inode) changed, so a folder deleted and created again is armed again. | `folders_created_one_level_at_a_time_with_pauses_are_followed`, `a_docs_folder_that_is_deleted_and_created_again_is_watched_again` |
| N5 | A numbered list between a `.` list and a `)` list takes `.` and an `<!-- -->` comment is written next to the neighbour it would merge with (reads back as one extra raw block; no merge). | `edit_blocks.rs::a_numbered_list_between_a_dot_list_and_a_paren_list_is_kept_apart_by_a_comment` |
| N6 | Documented on `Block::hash` and in the README (false conflict, never a missed one). Not normalised. | n/a |
| N7 | `Busy` now says to close the other Studio window or agent that is writing this project and retry; no advice to delete the lock. | `store.rs` lock test |
| N8 | Replacing or removing a key also drops the indented continuation lines of its old value. | `front_matter.rs::replacing_a_block_scalar_title_removes_its_continuation_lines` |
| ruling | `linked_specs` doc comment says absolute paths are allowed and the reader must confine every read to inside the linked folder. | n/a |

## Acceptance (after the last edit; output in `plans/reports/evidence-1523/`)

| Command | Result | Evidence |
|---|---|---|
| `cargo test -p designos-docs --locked` | 91 passed, 0 failed, 1 ignored (idle CPU, run below) | `r2-06` |
| `cargo clippy -p designos-docs --all-targets --locked -- -D warnings` | clean | `r2-05` |
| `cargo fmt --check` | exit 0 | `r2-05` |
| `cargo test --workspace --lib --locked` | every crate ok, 0 failed; `cargo metadata --locked` exit 0 | `r2-07` |
| Release fuzz `DOCS_TOUCH_CASES=200000 DOCS_SOUP_CASES=1000000` plus `DOCS_NESTED_CASES=1000000` | all pass. The reviewer's own `r_deep_touch_soup` (100,000 cases) printed `lossy=0 name_drift=0 deep_drift=0`; all 22 of the reviewer's probes pass | `r2-03` |
| `cargo test -p designos-docs --locked --test watch_idle_cpu -- --ignored --nocapture` | `mode=Some(Native)`, before 0.03 s, after 0.03 s, delta 0.00 s | `r2-04` |
| `an_oversized_file...` alone, 10 runs in a row | 10/10 ok | `r2-02-oversized-alone-10x.txt` |

## Corrected PR body additions

> **Review round 2.** Editing an indented list no longer glues the following paragraph onto it, and nested lists that cannot
> interrupt a paragraph keep their structure when edited; the touch property now compares blocks deeply (1,000,000 nested
> cases, 200,000 touch cases, 1,000,000 soup cases). The watcher reads files through the 8 MiB cap, follows a docs folder
> created level by level, deleted or replaced, and the lock error no longer suggests deleting the lock file.

Unresolved questions: none that block; the Ubuntu CI result for the new watcher tests is pending.

Status: DONE_WITH_CONCERNS
Summary: N1 to N8 fixed test-first (red in `r2-01`); the regression is gone (touch and nested fuzz at 200k/1M/1M cases, reviewer probes clean); idle CPU delta 0.00 s.
Concerns/Blockers: N3's reviewer-reported red not reproducible here (deterministic cap unit test added instead); Linux watcher paths not run locally; the `<!-- -->` separator reads back as one extra raw block.
