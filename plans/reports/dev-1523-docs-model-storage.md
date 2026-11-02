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

Status: DONE_WITH_CONCERNS
Summary: `designos-docs` crate implemented; acceptance 1-6 pass with captured output after the last edit; 12 fixtures byte-identical, conflict/atomic/lock tests, watcher within 1 s, pulldown-cmark chosen with measured fidelity table.
Concerns/Blockers: no recorded red-first run (mutation-checked instead); Skill tool unavailable so skills not loaded; canonical rewrite is not stable on a few pathological inputs (1 in 2M random cases, documented); watcher idle CPU and non-macOS CI unverified.
