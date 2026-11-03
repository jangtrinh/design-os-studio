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
| `model`, `markdown`, `render`, `page_edit` | `Page::parse` (never fails) / `render` / `render_canonical`, `Block` (`hash()`), `BlockKind`, `ListItem`, `Inline`, `push_block` / `insert_block` / `remove_block` (an error for a bad index), `block_hashes()` for block-level compare-and-swap |
| `escape` | `escape_plain(text, TextContext)`, `plain_inlines`, `paragraphs_from_plain`: plain text typed by a person or an agent to Markdown that reads back as exactly that text (`- not a list` stays a paragraph) |
| `front_matter` | `Page::front_matter()` with `title()`, `description()`, `group()` and `problems()`: a bad key is left unset and reported next to the page, the body still reads |
| `outline` | `Page::headings()`, `plain_text(&[Inline])` for "On this page", search and titles |
| `manifest`, `group` | `Manifest::{new, parse, validate, to_json_pretty}`, `Group`, `Pin` |
| `store` | `DocsStore::{for_project, at, read_page, write_page, read_manifest, write_manifest, list_page_files}`, `Expect`, `Versioned`, `ContentHash` |
| `watch` | `DocsWatcher::start(dir, wake)` / `start_polling`, `drain()`, `mode()`, `DocChange { rel, hash }` |

## Markdown library: pulldown-cmark as a span oracle, not a writer

**Choice: `pulldown-cmark` 0.13 (offset iterator) plus our own block model and writer. Not `comrak`.**

The requirement is that a file Studio wrote comes back byte for byte, and that Markdown we do not model survives
verbatim. A parse-then-print library cannot promise that, so the model never prints what the parser saw:

- pulldown-cmark finds where each top-level block starts and ends and what kind it is.
- Each parsed block keeps the exact source text it came from, and the whitespace before it (its *gap*). `render()`
  writes those back, so unedited content is byte-identical for **any** input, not only for text in Studio's own style.
- A block that is not exactly one of the v1 types (heading 1 to 3 in `#` form, paragraph, bullet / numbered list,
  checklist, `> [!kind]` callout, table, frame embed, fenced `mermaid`, fenced code) becomes `Raw` with its text kept.
  This covers setext and deeper headings, HTML, footnote and link definitions, indented code, plain quotes, titled or
  multi-paragraph callouts, mixed task lists, lists whose items hold code, quotes or tables, and a table whose row has
  text after its last cell.
- List items hold their text plus child blocks (`ListItem::children`): nested bullet, numbered and task lists, and
  further paragraphs. Indentation of continuation lines carries no meaning and is not kept in the model.
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
canonical form must be stable; a second property (`DOCS_TOUCH_CASES`, default 30,000) takes block-shaped pieces, touches
each typed block without changing it and requires the page to keep the same blocks. Run at 1,000,000 soup cases and
200,000 touch cases, both hold.

## Known limits

- **Canonical output of pathological text.** `render_canonical()` / an edited block can read differently on a second
  pass when the source has stray underscore emphasis next to other delimiters or backticks mixed into running text.
  Unedited blocks are never affected (they are written from source). A rewritten list that is followed by a block kept
  with its indentation writes that block without the indentation, so the two cannot merge; a list in front of an
  indented raw block is kept raw.
- **Looseness and delimiter style are not modelled.** A loose list (blank lines between items) is written tight, and
  `__bold__` / `_italic_` become `**` / `*` when that block is edited. A rewritten list takes a bullet (`-` / `*` /
  `+`) or delimiter (`.` / `)`) that neither the list before it nor the list after it uses, so neighbours never merge.
  A numbered list inserted between a `.` list and a `)` list has no free delimiter: an `<!-- -->` comment is written
  next to the neighbour it would otherwise merge with (it reads back as one extra raw block).
- **Block hashes.** `Block::hash` of an edited block is final only after the page is saved and read again (the page
  may write it with the file's line ending or another list marker). A hash from memory can differ from the one read
  back: a false conflict, never a missed one.
- **Linked specs.** `linked_specs` may hold absolute paths (picked with the system folder picker). Whoever reads them
  must confine every read to inside the linked folder.
- **Line endings.** A page keeps its first line ending (`\n` or `\r\n`) for edited blocks; a file that mixes both is
  written back unchanged until a block is edited. A byte order mark is kept.
- **Front matter** is read as one-line scalars (plain, single or double quoted) for `title`, `description`, `group`;
  other keys are kept verbatim and ignored. A bad value leaves its key unset and shows up in `problems()`. A page that
  opens with a `---` rule and has no `key: value` lines before the next `---` is a page, not front matter.
- **Text typed into a block** must go through `escape_plain` (or `plain_inlines`): `Inline::Text` is Markdown source.
  Newlines in a heading or a table cell become spaces, and blank lines in a paragraph split it (`paragraphs_from_plain`).
- **Write race with programs that do not use this crate.** Studio and `designos-mcp` serialise on `.docs.lock`.
  An editor that saves without the lock can win the few microseconds between the final hash check and the rename;
  the next read then sees its content and the watcher reports it.
- **Watcher** sleeps until the system reports a change (FSEvents on macOS, inotify on Linux) via the `notify` crate, so
  an idle project uses no CPU: 0.00 s over 20 s with 200 pages (`tests/watch_idle_cpu.rs`, `ps -o time=`). Events are
  batched and the folder is compared by size and time, hashing only what changed. Only when the system watcher cannot
  start or reports a failure does it look every 400 ms (`mode()` says which; `start_polling` forces it for network
  shares). The first look at the folder runs on the watcher thread, so `start` does no disk work; a file written right
  after `start` is still reported. The 8 MiB cap is applied to the read itself, so a file that is still growing is never read whole. The watch follows
  the docs folder when it is created level by level, deleted or replaced (inotify drops a deleted folder's watch
  without any error, so the folder's identity is checked after every batch of events). Studio's own writes are reported too: compare
  `DocChange::hash` with the hash `write_*` returned. If the wake callback panics the thread ends and
  `stopped_flag()` turns true.
- **Locking.** A write waits up to 2 s for `.docs.lock` (`DocsStore::with_lock_timeout`), then fails with `Busy`, an
  error that says to close the other window or agent and retry (the lock frees itself when its holder exits).
- Pages over 8 MiB are refused (`DocsError::TooLarge`). Symlinked pages are not listed or watched. An explicit path follows a symlink
  only when the target is inside the docs folder; a link to anywhere else, for a file or a folder, is refused
  (`DocsError::OutsideDocs`) for reads and writes of pages and of the manifest. The file that is read is the one that was
  checked: it is opened by its real path, and the open handle's device and inode must match that path (a link swapped in
  between is refused). A regular file with more than one hard link is refused (`DocsError::HardLinked`), because a hard
  link cannot be told apart from the outside file it names; replace it with a copy. A save by rename during a read
  (the inode under the name changes, the path stays inside the folder) is not an escape: the read starts over, up to
  three more times, and then fails with `DocsError::ChangedDuringRead`, which is safe to retry.

## Layout drift

`DocsStore::for_project` mirrors `designos-library`'s project folder (`<root>/projects/<id>`, private
`Library::project_dir`). `docs_live_in_the_project_folder_of_the_library` creates a project through the library and
fails if the two ever disagree.
