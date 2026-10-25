# #155 Check designs: suggest variables and styles from enabled libraries

- Issue: https://github.com/jangtrinh/design-os-studio/issues/155 (task E3-11, epic #16, M / P2)
- Spec base: `main` at `767acf9`.
- Open PR: none.
- Files: `crates/designos-app/src/check_designs/hardcoded.rs`, the number rule from #154, `fix.rs`, `mod.rs`, `state.rs`, `tests/libraries.rs` (new); read-only use of `crates/designos-app/src/libraries/mod.rs` and `crates/designos-doc/src/library_ops.rs`.

## 1. Summary and user story

As a designer in a file that consumes a design-system library, I want Check designs to suggest the library's colour and number variables, and its styles, when a hard-coded value matches them exactly. The suggestion shows the library name. One click imports the asset into my file and binds it, as one undo step.

## 2. Current state

- FACT: the colour index reads only local `doc.variables` (`hardcoded.rs:39-66`). The style matching reads only local `doc.styles`.
- FACT: libraries enabled in a file are `doc.library.enabled: Vec<LibraryRef { id, name, version }>` (`designos-doc/src/library.rs:23-27`, `:57`). Imported copies are tracked in `doc.library.external: BTreeMap<asset key, ExternalAsset>` (`library.rs:59`).
- FACT: a published library version is a `LibrarySnapshot` (`library_ops.rs:45-56`) with these fields: `assets`, `nodes`, `styles`, `style_meta`, `variables`, `variable_collections`, `images`.
- FACT: the app loads snapshots with `App::snapshot_of(id, version) -> Option<Arc<LibrarySnapshot>>` (`libraries/mod.rs:243`), which caches them.
- FACT: importing an asset goes through `App::import_asset_in(&mut self, tx: &mut Tx, library_id, key) -> Option<String>` (`libraries/mod.rs:684`), which wraps `library_ops::import_asset` (`library_ops.rs:1034`). It returns the local key of the copy.
- FACT: library styles are read-only in a consumer file (brainstorm A13).

## 3. Behaviour rules

1. **Sources**, in this order:
   - (1) local variables and styles, as today;
   - (2) each library in `doc.library.enabled`, at its **pinned** `version`, loaded with `snapshot_of(id, version)`.

   Disabled or absent libraries are never consulted (AC3). Only enabled libraries count. A library that is published but not enabled in this file is not a source.
2. **Already imported.** A library asset that already has a local copy (`doc.library.external` contains its key) counts as a local candidate (its local key), not a library one. This avoids duplicate suggestions.
3. **Matching** is the same as the local rules:
   - colours: the 8-bit RGB key plus alpha within `ALPHA_EPS` (`hardcoded.rs:16`), mode-aware;
   - numbers: exact within 0.0005 (#154);
   - styles: an exact list or text-style match.

   For library variables, "holds" means the value in the library collection's **default mode**, the first entry of `VariableCollection.modes`, because the consumer node has no mode for a collection it has not imported. Constant-in-all-modes is preferred, as in the local rules.
4. **Priority.** If any local candidate exists, library candidates are not offered for that property. Local wins. If there are only library candidates:
   - exactly one: finding plus fix;
   - several: finding listing them, with no fix (R2.7).
5. **Message.** `Fill #0D99FF matches variable "color/primary" from library "Brand DS"`. For styles: `… style "Body/Regular" from library "Brand DS"`.
6. **Fix = import and bind, one undo step.** In one `Tx`:
   - (a) `import_asset_in(tx, library_id, key)` gets `local_key`;
   - (b) bind `key` to `local_key` (variable), or apply the style `local_key`.

   If (a) returns `None`, cancel the transaction, refuse the fix, and show the toast `"Couldn't import \"{name}\" from {library}"`.
7. **Snapshots load off the audit hot path.** The audit uses only snapshots already in the cache. A missing snapshot is requested through `snapshot_of` before the audit, from `refresh_check_designs`. If a snapshot fails to load (I/O error), that library is skipped. The panel shows a one-line note: `Library "{name}" couldn't be read; its suggestions are missing`.

## 4. Data model / schema changes

In memory only:
- `Fix::BindVariable` and `Fix::ApplyStyle` gain an optional `library: Option<String>` (the library id). When it is `Some`, the fix imports first.
- `Report` gains `unreadable_libraries: Vec<String>`.

## 5. Contracts

```rust
pub(super) struct LibrarySources { libs: Vec<(LibraryRef, Arc<LibrarySnapshot>)> }
impl LibrarySources { pub(super) fn collect(app: &mut App) -> (Self, Vec<String> /* unreadable */); }
pub fn audit_with_libraries(doc: &Document, scope: &Scope, libs: &LibrarySources) -> Report;
```

`audit(doc, scope)` stays as a wrapper with no libraries, for tests and headless use.

## 6. File-format mapping

None. Library snapshots are Studio's own format. RESOLVED.

REA (E3-d), from the Figma 126.9.13 editor bundle:
- Figma's linter has `REQUIRE_ASSETS_FROM_SELECTED_LIBRARIES` ("Detects assets that are not from the selected libraries"), plus the contexts `LIBRARY_VARIABLES_CONTEXT`, `VARIABLE_THUMBNAILS` and `TEXT_STYLES_CONTEXT`;
- the unified colour rule `RECOMMEND_VARIABLES` "detects hardcoded colors, wrong-library variables, and contrast issues".

So Figma's linter also works against a set of selected libraries. A "wrong library" rule (an asset from a library that is not selected) is not part of this task; see #158.

## 7. UI

- **Rows.** Findings are rows in Hard-coded values. The library name appears in the message and as a small muted badge `Library` (P5: a separate badge, not concatenated into the label).
- **Fix button.** Same as for local findings.
- **Unreadable library.** The note appears under the scope label in muted text.
- **Evidence.** A 1440 capture with one library suggestion.

## 8. Edge cases

1. An enabled library `Brand DS` v3 has `color/primary = #0D99FF`. A layer fill `#0D99FF`, with no local variable: 1 finding with fix. After the fix, `doc.library.external` contains the key, the fill is bound to the local copy, and one undo removes both.
2. The same library disabled: 0 findings (AC3).
3. Local `blue/500 = #0D99FF` and library `color/primary = #0D99FF`: suggest local only.
4. Two enabled libraries both with `#0D99FF`: listed with no fix.
5. A library variable already imported earlier: suggested as local (its local key), with no import step.
6. A library pinned at v2, latest v3 with a changed value: v2 is used, the pinned version.
7. A snapshot file missing on disk: the library is skipped, and the note is shown.
8. A library text style matching a layer: the `Apply style` fix imports the style. The style stays read-only (A13).

## 9. Non-goals

- Flagging assets from non-selected libraries (Figma's `REQUIRE_ASSETS_FROM_SELECTED_LIBRARIES`); see #158.
- Fetching libraries over the network (no-go).
- Updating library versions.

## 10. Test plan

Seam: `crates/designos-app/src/check_designs/tests/libraries.rs`, with a fixture library published into a temp library root (`set_root`, `libraries/mod.rs:150`) through `designos_library::publish`.

| Test | Fails before |
|---|---|
| `library_variable_is_suggested_with_its_library_name` | yes |
| `library_fix_imports_and_binds_in_one_undo_step` | yes |
| `disabled_library_is_never_suggested` | guard |
| `local_candidate_beats_library_candidate` | yes |
| `two_libraries_with_same_value_are_ambiguous` | yes |
| `pinned_version_is_used_not_latest` | yes |
| `unreadable_library_is_reported_and_skipped` | yes |

## 11. Performance budgets

- `CLAUDE.md` §4.1: snapshot I/O never happens inside the audit loop or per frame. It happens once per open and version change, through the existing cache.
- The library indexes are built once per audit.

## 12. Dependencies and build order

- Depends on #35 (merged) and #154 (number rule).
- Before #157, so the off-thread scan takes `LibrarySources` as an immutable `Arc` input.

## 13. Owner rulings that apply

- **R2.7** (round 2, item 7): exact matches only. Ambiguous matches have no fix.
- **A13** (brainstorm, coordinator 2026-10-17): library styles are read-only in a consumer file.
- No network (brainstorm no-gos).
- Owner default: 1440 capture only.
