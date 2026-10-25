# #246 Add create_new_file without replacing the open document

| Field | Value |
|---|---|
| Issue | #246 (task T7.36, epic #20; milestone v0.7 Effects & Export). Issue #43 is its listed dependency and has no milestone (see open question 3). |
| Size | M (the issue says S; the cross-process library lock and the board and deck constructors add the rest, see sections 2 and 3) |
| Depends on | issue: #43. This spec's reading: no code dependency on #43 (open question 3). Soft: v0.6's E15-5 (error envelope) and T7.6 (tool snapshot and parity matrix) |
| Unblocks | the parity-matrix row for Figma's `create_new_file`; a later "open a library file without replacing the open one" task (the follow-up in section 9) |
| Touches | new `crates/designos-mcp/src/tools/create_new_file.rs` (the `tools/` directory is created by the first v0.6 task that lands; if none has, this task creates it), `crates/designos-mcp/src/server.rs` (one dispatch arm and one `tools()` entry beside `designos_new_document`, `:429` and `:620-626`), `crates/designos-library/src/store.rs` (`write_lock` `:103-109` and its 8 call sites), new `crates/designos-library/src/blank.rs`, `crates/designos-doc/src/board.rs` (blank board and deck constructors), `crates/designos-app/src/board/create.rs` and `crates/designos-app/src/decks/mod.rs` (call the shared constructors), `crates/designos-app/src/terminal.rs` (hot spot: one `cmd.env("DESIGNOS_LIBRARY", ...)` line in `configure_pty_env_with_session`, `:2290-2301`), tests, and, if T7.6 has landed, its tool snapshot and parity matrix |

Code baseline: `origin/main@0e8a9a5` (`docs(specs): v0.6 task specs, part 4 (agent terminal UX and constraint solver) (#697)`). v0.7 opens after v0.6 is tagged. The worker re-anchors every cited line against HEAD before writing code and reports drift first.

## 1. Summary and user story

As an agent, I call `designos_create_new_file` and get a new file in the user's library without touching the document the user has open. As the user, I find that file in Home, and the layers, styles, variables and comments I have open stay exactly as they were.

## 2. Current state (FACT)

1. **Figma's tool** (developers.figma.com, "Figma MCP server" tools page, read 2026-10-19): `create_new_file`, category "Code to design", remote server only; it "creates a blank Figma Design, FigJam, or Figma Slides file in your drafts"; no parameters are documented; with more than one plan the agent asks which team to use. It returns nothing the page names.
2. **Only `designos_new_document` exists**, and it replaces the session document: it resets the engine, publishes the swap to the editor with `publish_live_doc_sync` and sets `dirty` (`crates/designos-mcp/src/server.rs:429-444`). Studio then assigns the new document over the open one (issue #43: "That drops unsaved layers, styles, variables and comments"). `designos_open_file` does the same (`server.rs:389-428`).
3. **Library files are created by `Library::create_file`** (`crates/designos-library/src/store.rs:623-662`): it needs a project id, writes `<root>/projects/<project>/files/<id>.designos` atomically, then appends to the project's manifest. `Library::open` creates the root if needed (`crates/designos-library/src/lib.rs:50-51`). `designos-mcp` already depends on `designos-library` (`crates/designos-mcp/Cargo.toml`) and uses `load_external` (`server.rs:410`).
4. **The manifest lock is in-process only.** `write_lock()` is a process-wide `static Mutex` (`crates/designos-library/src/store.rs:103-109`) taken by 8 mutating methods (`:630`, `:683`, `:733`, `:754`, `:783`, `:827`, `:901`, `:936`). `create_file_counted` reads the manifest, pushes an entry and writes it back (`:629-662`). Studio and the MCP server are separate processes, so their read-modify-write cycles can interleave and one entry is lost. A lost entry leaves a file on disk that Home never lists, silently. (A search for `flock`, `fs2`, `fd-lock` and `lock_exclusive` in `designos-library` finds none.) The standard library's `File::lock` and `File::try_lock` compile on the pinned toolchain (`rust-toolchain.toml`: 1.93.1) and a second handle on the same file is refused while the first holds the lock (probe built with `rustc 1.93.1` from the repository directory, 2026-10-19).
5. **The agent process does not know the library Studio uses.** Studio exports session paths but no `DESIGNOS_LIBRARY` (`terminal.rs:2290-2301`; the variable is read only in `crates/designos-library/src/store.rs:448-455` and set nowhere in the app). With `designos-studio --library DIR` (`crates/designos-app/src/cli.rs:45`) the MCP child would write to the default root, not the library on screen.
6. **Home refreshes when its `dirty` flag is set by an in-app action or when it is rebuilt**: `show` refreshes only if `h.dirty` (`crates/designos-app/src/home/mod.rs:431`); `go_home` builds a new `Home` and refreshes it (`home/session.rs:453-466`). Nothing watches the manifest. INFERENCE: a file created by the MCP process shows the next time the user goes Home. UNKNOWN: whether the agent dock can be on screen while Home is. Check: read `shell.rs` for where the terminal dock is drawn relative to `app.home`.
7. **How Home creates blank files** (`home/session.rs:543-571`): project = the open file's project, else the first project, else a new project named "My project"; a design is `Document::new("Untitled")` plus one page "Page 1"; a board is `crate::board::create::new_document` (`board/create.rs:13`: kind Board, page "Board", transparent background); a deck is `crate::decks::new_document` (`decks/mod.rs:33`: kind Deck, page "Slides", one 1280 x 720 slide frame). The last two live in `designos-app`, which `designos-mcp` cannot call. `DocumentKind` is `Design`, `Board`, `Deck` (`crates/designos-doc/src/board.rs:13-19`), the local equivalents of Figma's Design, FigJam and Slides.
8. **There is no tool to list projects or library files**, so an agent can only learn a project name from an error message.
9. Paths given to tools are confined to the workspace (`input_path`, `server.rs:319`); the library root is outside it by design and is never taken from the caller.

## 3. Behaviour rules

1. **Tool `designos_create_new_file`.** Arguments (all optional, `additionalProperties: false`):
   - `name`: 1 to 1024 bytes after trimming; default "Untitled".
   - `kind`: `design` (default), `board` or `deck`. Not allowed together with `source_path` (the copy keeps the source's kind).
   - `project`: a project id or an exact project name (case-insensitive). Default: the project named "Drafts", created on first use (open question 2).
   - `source_path`: a file inside the workspace (`.designos`, `.fig`, SVG or Figma REST JSON) to copy into the new library file; read with the same loader and limits as `designos_open_file` (64 MiB, `validate_document`).
2. **Reply:** `{ "file_id", "project_id", "project_name", "name", "kind" }`. No disk path. The new file is not opened anywhere.
3. **The open document is untouched.** The call never replaces `self.engine`, never calls `publish_live_doc_sync` or writes the edit log, never changes `dirty`, history or the session ledger. A session with unsaved changes returns the same `designos_get_document` before and after.
4. **Library choice.** The tool opens `Library::default_root()`. Studio exports `DESIGNOS_LIBRARY` (the root of the library it has open) to every agent process, next to the session variables, so the agent and Studio use one library.
5. **Cross-process lock.** `write_lock()` becomes a guard that holds the in-process mutex and an advisory file lock on `<root>/.lock` for the whole manifest read-modify-write (`std::fs::File::lock`). It waits up to 5 seconds (polling `try_lock`) and then fails with a named error `LIBRARY_BUSY`; it never waits forever. The lock file is created on first use and never deleted. Methods that today take no lock (`rename_project`, `delete_project`, `create_folder`, `rename_folder`, `move_folder`) are not changed by this task (follow-up, section 9).
6. **Blank documents** come from one shared constructor `designos_doc::Document::blank(kind, name)` (design: one page "Page 1"; board and deck as in fact 7). `designos-app`'s `board::create::new_document` and `decks::new_document` call it, so Home and the tool create byte-identical files (the existing app tests keep passing).
7. **Errors** (named, with the E15-5 envelope once it exists; plain messages before): `UNKNOWN_PROJECT` (lists up to 20 project names as `valid_options`), `PATH_OUTSIDE_WORKSPACE` and `FILE_TOO_LARGE` (the existing meanings), `DOCUMENT_INVALID`, `INVALID_ARGUMENTS` (a `kind` given with `source_path`, a blank name), `LIBRARY_BUSY`, `LIBRARY_UNAVAILABLE` (the root cannot be created or written).
8. **Atomic from the caller's side.** Either the file exists on disk and in the manifest and the reply names it, or nothing was added (`create_file_counted` already removes an unlisted file after a failed manifest write, `crates/designos-library/src/store.rs:655-670`).
9. The parity matrix row for `create_new_file` reads `implemented`, `studio_tool: designos_create_new_file`, with a note on the Figma differences: no plan choice (one local library), no tool-visible "drafts" except the "Drafts" project.

## 4. Data model / schema changes

None in documents. A new empty file `<root>/.lock` in the library root. A new project directory "Drafts" appears the first time the default is used.

## 5. Contracts

- Tool schema as in rule 1 and the reply of rule 2; `outputSchema` declared with E15-5.
- `Library` gains no public method; `write_lock` stays private.
- `designos_doc::Document::blank(kind: DocumentKind, name: &str) -> Document`.
- Environment: `DESIGNOS_LIBRARY` is exported by Studio to the agent process (the name `Library::default_root()` already reads).

## 6. File-format mapping

The created file is an ordinary `.designos` library file, loadable by every existing path. With `source_path`, a `.fig` or REST file is imported by `load_external` and stored as `.designos`, as Home's import does. Figma's three `create_new_file` types map: Design -> `design`, FigJam -> `board`, Slides -> `deck`.

## 7. UI

No new UI. The new file appears in Home like any file. Evidence is terminal output plus one Home screenshot: (a) a stdio session with unsaved edits calling `designos_create_new_file` and then `designos_get_document` (same node and comment counts, `dirty` unchanged); (b) `ls` of the library `projects/` tree before and after; (c) a desktop capture at 1440 of Home showing the new file in the "Drafts" project after going Home. Responsive: SKIPPED (owner default). `es-designer` is not needed (no UI code); the Home capture is evidence only.

## 8. Edge cases

1. The agent calls it ten times in a row: ten files, ten distinct ids, all listed (the lock serialises the writes).
2. Studio saves a thumbnail (a background thread) while the tool writes: both cycles are serialised by the lock; the manifest ends with both changes.
3. A second `designos-mcp` process (another Studio window) calls it at once: the file lock serialises it; neither entry is lost.
4. The library root is read-only or the disk is full: `LIBRARY_UNAVAILABLE`; no partial file stays.
5. A crashed process held the lock: the OS releases an advisory lock when the process ends, so the next caller proceeds.
6. `project` names a project that was moved to the trash: not found, so `UNKNOWN_PROJECT`.
7. `source_path` is a 60 MiB `.fig`: imported within the existing limit (this call blocks for the import time, as `designos_open_file` does); a 70 MiB one is refused with `FILE_TOO_LARGE`.
8. Home is on screen: the new file shows after the next refresh or return to Home (fact 6).
9. The agent wants to edit the new file in the same session: not possible yet (section 9); the reply says so in the tool description, not in the data.
10. `name` with slashes or control characters: stored as given in the manifest (file names on disk are ids).

## 9. Non-goals

- Opening the new file in Studio, or making it the session document. That needs the non-replacing open of issue #43; this tool never does it.
- A tool that lists projects or library files, and template galleries (Home's template documents live in `designos-app`).
- Taking the lock in the five manifest methods that skip it today: a follow-up, reported with this spec.
- Live refresh of a Home that is already on screen.
- Team or plan selection (one local library).

## 10. Test plan (red first)

Seams: `designos-mcp` session tests with a temporary library root and a live-bridge session directory; `designos-library` unit tests; one two-process lock test.

| Case | Expected | Fails before |
|---|---|---|
| `create_new_file_is_listed_and_not_read_only` | `tools()` contains it; stdio count follows the snapshot (23, or 24 with #244) | yes |
| `creates_a_library_file_and_returns_its_id` | reply ids resolve with `Library::file`; the file loads; the manifest lists it | yes |
| `the_open_document_is_untouched` | a dirty session with 3 edits and an undo step: `designos_get_document` identical before and after; `dirty`, undo depth and ledger unchanged | yes |
| `no_editor_publish_happens` | the session directory's `doc-sync.designos` and `doc-patch.jsonl` are byte-identical after the call | yes |
| `kind_board_and_deck_match_home` | `Document::blank` equals the app's `board::create::new_document` and `decks::new_document` outputs | yes |
| `source_path_copies_a_workspace_file` | a workspace `.designos` with 5 nodes: the new library file has the same nodes and the source is unchanged; a path outside the workspace is refused | yes |
| `kind_with_source_is_refused` | `INVALID_ARGUMENTS` | yes |
| `default_project_is_drafts_created_once` | first call creates "Drafts"; the second reuses it; `project` by id and by name work; unknown name gives `UNKNOWN_PROJECT` with `valid_options` | yes |
| `ten_concurrent_creates_lose_nothing` | 10 threads, 10 ids, manifest has 10 entries | guard (the in-process mutex already holds) |
| `two_processes_lose_nothing` | a test spawns two helper processes (the test binary re-invoked) each creating 20 files in one root: the manifest lists 40 | yes (an entry is lost without the file lock) |
| `lock_wait_is_bounded` | a handle holding the lock for 6 s: the call fails with `LIBRARY_BUSY` after about 5 s | yes |
| `no_disk_path_in_the_reply` | serialised reply has no `/` or the root path | yes |
| `studio_exports_the_library_root` | the agent environment built by `configure_pty_env_with_session` contains `DESIGNOS_LIBRARY` equal to the open library's root | yes |
| `existing_library_tests_pass` | `cargo test -p designos-library` and the app's home tests are green | guard |

Run: `cargo test -p designos-mcp`, `cargo test -p designos-library`, `cargo test -p designos-doc board`, `cargo test -p designos-app home`, `cargo test -p designos-app terminal`.

## 11. Performance budgets

- A design or board file: under 20 ms excluding disk sync (measured once and recorded).
- The file lock adds under 1 ms when uncontended. Lock hold time is the manifest read-modify-write: under 50 ms for a library of 1,000 files (UNKNOWN: measure on a synthetic library; the manifest is JSON, and `find_file` scans every project today).
- No UI-thread work is added. The UI thread's own manifest writes now wait on the same lock; the budget above is why a 5-second ceiling is safe and normally invisible.

## 12. Dependencies and build order

- Issue #43 is listed as the dependency. This spec finds no code reason to wait for it: the tool never goes through the replace path (rule 3). #43 stays a separate fix, and the "open without replacing" tool that would let an agent edit the new file needs it.
- `designos-mcp/src/server.rs` is shared with #43 and v0.6's E15-5 and #690: add one arm and one tool entry, then rebase. `terminal.rs` gets one line; keep it out of the hot-spot order by landing it with this task. Order in README: after #244 (MCP files) and before #242 (which also adds fields to the same `terminal.rs` function family).
- `crates/designos-library/src/store.rs` is not one of the named hot files, but the lock change touches 8 methods: keep that part of the PR free of other edits.

## 13. Owner rulings

- Only `new_document` exists and it replaces the open document; `create_new_file` must create a library file without replacing it; the open document is untouched; an optional template or source argument. (Issue text and the 2026-10-17 amendment on #43.)
- All AI goes through the user's own agent CLI; the tool is local and makes no network call (pack-wide ruling).
- Full scope, never MVP; the pack only orders the work.

## Open questions for the owner (recommended first)

1. **File types.**
   - A (recommended): `design`, `board` and `deck`, the three kinds Figma's tool creates (Design, FigJam, Slides). It needs the shared blank constructors of rule 6. Size M.
   - B: `design` only now; the matrix row says so and a follow-up issue covers board and deck. Size S.
2. **Where an unnamed file goes.**
   - A (recommended): a project named "Drafts", created on first use. It mirrors Figma's "in your drafts" and keeps agent files out of the user's real projects.
   - B: the same choice as Home's File > New file when nothing is selected: the first project, else a new "My project".
3. **Wait for #43?** #43 and #75 to #77 and #81 have no milestone. Recommended: do not wait; ship this tool on its own, and give #43 a milestone for the non-replacing open. Alternative: keep the issue's dependency, which holds this task until #43 is scheduled and done.
