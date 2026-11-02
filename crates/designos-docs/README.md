# designos-docs

Project docs for DESIGN:OS Studio: a Markdown block model, the `docs-manifest.json` manifest, conflict-checked
atomic storage and a disk watcher. No UI. `designos-app` (phases 02, 04) and `designos-mcp` (phase 06) build on it.

```
<library>/projects/<project-id>/docs/
  docs-manifest.json        groups in fixed order, page order, pinned frames, linked spec folders
  about.md  spec/prd.md …   pages: Markdown with optional front matter (title, description, group)
  .docs.lock                advisory lock taken around every write (hidden; the watcher ignores it)
```

| Module | Public surface |
|---|---|
| `model`, `markdown`, `render` | `Page::parse` / `render` / `render_canonical`, `Block`, `BlockKind`, `Inline`, block edits |
| `outline` | `Page::headings()`, `plain_text(&[Inline])` for "On this page", search and titles |
| `manifest`, `group` | `Manifest::{new, parse, validate, to_json_pretty}`, `Group`, `Pin` |
| `store` | `DocsStore::{for_project, at, read_page, write_page, read_manifest, write_manifest, list_page_files}`, `Expect`, `Versioned`, `ContentHash` |
| `watch` | `DocsWatcher::start(dir, wake)`, `drain()`, `DocChange { rel, hash }` |

## Markdown library: pulldown-cmark as a span oracle, not a writer

**Choice: `pulldown-cmark` 0.13 (offset iterator) plus our own block model and writer. Not `comrak`.**

The requirement is that a file Studio wrote comes back byte for byte, and that Markdown we do not model survives
verbatim. A parse-then-print library cannot promise that, so the model never prints what the parser saw:

- pulldown-cmark finds where each top-level block starts and ends and what kind it is.
- Each parsed block keeps the exact source text it came from, and the whitespace before it (its *gap*). `render()`
  writes those back, so unedited content is byte-identical for **any** input, not only for text in Studio's own style.
- A block that is not exactly one of the v1 types (heading 1 to 3 in `#` form, paragraph, bullet / numbered list,
  checklist, `> [!kind]` callout, table, frame embed, fenced `mermaid`, fenced code) becomes `Raw` with its text kept.
  This covers setext and deeper headings, HTML, nested lists, footnote and link definitions, indented code, plain
  quotes, titled or multi-paragraph callouts, mixed task lists.
- Editing a block (`Block::kind_mut`) drops its kept text; only that block is then written canonically
  (`- ` bullets, `1.` numbers, `**bold**`, `*italic*`, `| a | b |` tables, longer fence when the code holds backticks).
  `render_canonical()` writes every block that way and is what proves the typed model carries the whole file.
- `Inline::Text` holds Markdown source (`2 \* 3` stays `2 \* 3`), so no escaping rules exist to get wrong.

### The 1-hour fidelity check (run 2026-11-02)

The 12 fixtures in `tests/fixtures/` (the v1 block types, an escapes page, an all-blocks page with front matter and
an unknown-syntax page) were parsed and printed back by each candidate, comparing bytes with the input:

| Candidate | Byte-identical | Passes | Where it differs |
|---|---|---|---|
| comrak 0.40 `format_commonmark` | 3 of 12 | checklist, frame embed, heading | escapes `[!note]` to `\[\!note\]`, inserts `<!-- end list -->` between lists, writes ``` ``` mermaid ``` with a space, rewrites table rules (`:---:` to `:-:`) |
| pulldown-cmark 0.13.6 + pulldown-cmark-to-cmark 21.1 | 3 of 12 | frame embed, heading, inline paragraph | `*` bullets, ```` ```` ```` fences, a blank `>` line before callouts, strips table padding (`\|Method\|Fee\|`) |
| this crate (`render`) | 12 of 12 | all | none |

Neither passes the lists, callout, table, code, mermaid, all-blocks or unknown-syntax fixtures, so using either as
the writer would have rewritten files on the first save. The harness was a throwaway binary outside
the repo; the fixtures it ran are committed, and `every_v1_fixture_round_trips_byte_identical` is the standing check.

### How the guarantee is tested

Public-API tests only (`tests/`): every fixture round-trips and also re-renders canonically; every prefix of every
fixture (about 2,400 cut points: unterminated fences, half tables, broken links) round-trips without panic; a
deterministic random Markdown soup (`DOCS_SOUP_CASES`, default 40,000) must round-trip with `render()`, and its
canonical form must be stable. At 2,000,000 cases `render()` was lossless every time; the canonical form was not
stable for 1 input, listed below.

## Known limits

- **Canonical output of pathological text.** `render_canonical()` / an edited block can read differently on a second
  pass when the source has stray underscore emphasis next to other delimiters, backticks mixed into running text, or
  indented list lines continuing a paragraph. Unedited blocks are never affected (they are written from source).
- **Looseness and delimiter style are not modelled.** A loose list (blank lines between items) is written tight, and
  `__bold__` / `_italic_` become `**` / `*` when that block is edited. Two adjacent lists are kept apart by
  alternating the bullet (`-` / `*`) or delimiter (`.` / `)`).
- **Front matter** is read only as one-line scalars for `title`, `description`, `group`; other keys are kept
  verbatim and ignored. A page that opens with `---` and has another `---` line later is front matter, as in every
  Markdown tool.
- **Write race with programs that do not use this crate.** Studio and `designos-mcp` serialise on `.docs.lock`.
  An editor that saves without the lock can win the few microseconds between the final hash check and the rename;
  the next read then sees its content and the watcher reports it.
- **Watcher** polls the folder on its own thread every 400 ms (stat only, hash only on change), like the library's
  `DiskWatcher`; an edit is reported within about half a second. It misses a rewrite that keeps both the size and
  the modification time. Studio's own writes are reported too: compare `DocChange::hash` with the hash `write_*` returned.
- Pages over 8 MiB are refused; symlinked pages are not listed or watched (an explicit path still follows a symlink).

## Layout drift

`DocsStore::for_project` mirrors `designos-library`'s project folder (`<root>/projects/<id>`, private
`Library::project_dir`). `docs_live_in_the_project_folder_of_the_library` creates a project through the library and
fails if the two ever disagree.
