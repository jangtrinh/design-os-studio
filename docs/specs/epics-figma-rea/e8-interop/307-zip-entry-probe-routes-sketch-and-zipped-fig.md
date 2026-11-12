# #307 One content probe routes `.sketch`, zipped `.fig` and every other import

| Field | Value |
|---|---|
| Issue | #307 (task T8.24, S / P2; epic #21; milestone v0.26 Sketch import) |
| Code baseline | `origin/main@d22dad970`. Re-anchor every line against HEAD and report drift before coding. |
| Verdict | **Still real.** Routing is by file extension only, in two copies; a `.sketch` falls to the native-JSON branch and fails as "Invalid designos file". |
| Size | S: 4–5 h builder |
| Tier | Grok build-fast (pure function + table tests; reversible) — review by Claude |
| Touches | new `crates/designos-library/src/probe.rs` (pure `probe(path, head_bytes) -> ImportKind`), `crates/designos-library/src/store.rs` (`load_external_counted` `:152-200`), `crates/designos-app/src/io.rs` (`load_path_importing_pen_with_mode` `:211-300`, `open_request` filter `:484-492`), `crates/designos-app/src/home/mod.rs` (`import_files_dialog` filter `:633-640`). |
| Depends on | none. Unblocks #308. |

## 1. Still real? (FACT)

- Two routers, both keyed on the lower-cased extension:
  - library import: `load_external_counted` (`store.rs:152-153`) — `"fig"`, `"svg"`, `"json" | "webcapture"`, else native JSON (`:157-193`);
  - app open: `load_path_importing_pen_with_mode` (`io.rs:211-219`) — `"fig"`, `"pen"`, `"webcapture"`, `"json"`, `"svg"`, else native (`:220-299`).
- They already differ: the library path has no `.pen` branch; the app path has no shared probe. `.sketch` → `_` branch in both → `Document::from_json_with_report` / `parse_native_with_warning` on ZIP bytes → error text "Invalid designos file: …".
- The `.fig` reader already accepts a ZIP that holds `canvas.fig` at any depth (`designos-fig/src/container.rs:111-117`, `:177`: `name == "canvas.fig" || name.ends_with("/canvas.fig")`), with entry-count and inflate budgets (`:165-170`). So a `.fig` that is itself a ZIP already opens; what is **not** handled is a `.zip` file (or a misnamed file) that wraps a `.fig`.
- File pickers: app Open lists `designos, legacy, fig, pen, json, webcapture, svg` (`io.rs:486`); Home Import lists only `designos, fig, json, svg` (`home/mod.rs:636`) — no `.pen`, no `.webcapture`.
- Precedent: the full-scope plan decided "The file probe inspects zip entries, because `.sketch` and zipped `.fig` share the zip magic" (`plans/261017-epics-full-scope/full-scope-e7-e8-e10.md:323`).

## 2. User-visible outcome

Dropping or picking a `.sketch` file, a `.zip` that holds a `.fig`, or a file whose extension is wrong, opens with the right importer, or says plainly what the file is and why it cannot open yet. Home Import and File > Open accept the same file types.

## 3. Behaviour rules (decided; technical)

- R1 One probe, `designos_library::probe(path, head)` returning `ImportKind::{Native, Fig, FigInZip, Sketch, Pen, WebCapture, FigmaRest, Svg, Unknown}`. Both routers call it; the per-format branches stay where they are.
- R2 Content first, extension second: read ≤ 8 KB head (same budget as `file_kind`, `store.rs:731-745`).
  - `fig-kiwi` / `fig-jam.` magic → `Fig` (`container.rs:75-76`).
  - `PK\x03\x04` → read the ZIP central directory only (no inflate): has `canvas.fig` (root or nested) → `Fig`; has `document.json` + `meta.json` (+ `pages/`) → `Sketch` (Sketch format: "ZIP archives containing JSON", entries `document.json`, `meta.json`, `user.json`, `pages/`, `images/`, `previews/` — developer.sketch.com/file-format); has exactly one entry ending `.fig` → `FigInZip`; else `Unknown("zip with no design file")`.
  - `<svg` / `<?xml` → `Svg`; `{` → JSON sub-probe (web capture marker `looks_like_web_capture_json`, Figma REST, `.pen`, native) as today.
  - Extension only breaks ties among JSON kinds, as today.
- R3 `FigInZip`: extract that one entry within the existing inflate budget (`container::open_with_budget`) and import it as `.fig`. More than one `.fig` entry → `Unknown("zip holds 3 Figma files; unzip it and open one")` (decided; not a picker, keeps the S size).
- R4 `Sketch` before #308 ships: error toast "Sketch import is coming in a later release" — no fake import (Zero Fake UI). After #308: route to the importer.
- R5 Pickers: Home Import and Open share one filter list built from the probe's kinds (add `pen`, `webcapture`, and `sketch`/`zip` only once #308 ships).

## 4. Acceptance criteria (red then green)

- **AC-1** Table test: one fixture per kind (native, fig-kiwi bare, fig zip, zip-wrapping-fig, sketch zip, pen, web capture json, REST json, svg, random zip, empty file, truncated zip) → expected `ImportKind`. Red: probe does not exist.
- **AC-2** A `.sketch` dropped on Home produces the R4 message, not "Invalid designos file". Red on main.
- **AC-3** `design.zip` containing `design.fig` opens as a document whose page count equals the inner `.fig`'s. Red on main.
- **AC-4** A `.fig` renamed `.json` opens as `.fig`; a native file renamed `.fig` opens as native (content wins).
- **AC-5** Library import and app open give the same kind for every AC-1 fixture (one test drives both routers).
- **AC-6** Probe reads ≤ 8 KB + the ZIP central directory; a 300 MB `.fig` probes in < 50 ms (measured on the kit copy, logged in the PR).

## 5. Test steps

`cargo test -p designos-library probe`; app test for AC-2/AC-3 through `load_path_importing_pen`; Home screenshot of the AC-2 toast (1440).

## 6. Edge cases

- ZIP64 / encrypted ZIP → `Unknown` with "encrypted or unsupported zip".
- `.sketch` from Sketch < 43 (binary, not ZIP) → `Unknown("This Sketch file is from Sketch 42 or older; open and save it in a newer Sketch")` — Sketch says the JSON format "was originally introduced in Sketch 43" (developer.sketch.com/file-format).
- Zip slip names (`../x.fig`) are never written to disk (in-memory only).

## 7. Non-goals

Sketch import itself (#308). `.jam` / `.deck` routing (#320; probe leaves `fig-jam.` as `Fig`). PPTX (#290).

## 8. User-visible details not settled by the issue

Error wording for R3/R4 and the edge cases — decided above, ≤ 8-word style where possible; reviewer checks against the PCP ux-writing rule.

## 9. Rollback

Revert; both routers fall back to their extension `match`.
