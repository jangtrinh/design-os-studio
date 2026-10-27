# DEV Report — `#1165` (P0): Orphan Style IDs (`1186:100350`) and Unnamed Style Fallbacks

- **Issue:** `#1165` (`US-01`, `v0.6.5` hardening batch)
- **Branch:** `jangtrinh/agy-1165-orphan-style-names` (based on `origin/release/v0.6.4` at `6f0c3bb2`)
- **Controller Ruling Applied:** `msg_803274bbbced` (`#1165 controller ruling: Figma parity (Jang, 2026-10-27)`, backed by `plans/reports/re-figma-261027-ui-facts.md` §Q1)

---

## 1. Epistemic Ledger (`FACT` / `INFERENCE` / `ASSUMPTION`)

- **`[FACT]`** In `crates/designos-fig/src/fig.rs` (`entry_from_json`, line 373), Kiwi node entries with `phase == "REMOVED"` or `isSoftDeleted == true` are dropped before `Lk::new` and `style_table`. When a consumer layer references such a style node (e.g. `1186:100350`), `Lk::style_id_of` returns `Some("1186:100350")`, `style_table` omits `1186:100350`, and `crates/designos-fig/src/rest_import.rs:1279-1298` seeds `doc.styles["1186:100350"]` from the consumer layer's own values without creating an entry in `doc.style_meta`.
- **`[FACT]`** Because `style_entries_of` (`crates/designos-app/src/styles.rs:55`) and `style_picker::linked_style` (`crates/designos-app/src/panels/style_picker.rs:53`) previously checked `doc.styles` without checking whether the style existed in the file's style table (`doc.style_meta`), consumer-seeded orphan IDs like `"1186:100350"` leaked into the style picker, Assets, MCP `designos_get_document` / `designos_search_design_system`, and the layer's Inspector style chip with raw GUID labels.
- **`[FACT]`** For `isSoftDeletedStyle: true` (where `phase != "REMOVED"` and `isSoftDeleted != true`), `Lk::style_id_of` previously stripped the style ID via `(!self.deleted_styles.contains(&id)).then_some(id)`, dropping `node.style_refs` and preventing `.fig` export round-trip of `style_refs`.
- **`[FACT]`** Per `plans/reports/re-figma-261027-ui-facts.md` §Q1 (Figma 126.10.11 RE) and controller ruling `msg_803274bbbced`:
  1. **R1 (List):** List only styles of the requested type that the file itself owns and that still exist. Exclude soft-deleted styles (`isSoftDeleted` / `isSoftDeletedStyle`), `phase == "REMOVED"` styles, and orphan IDs seeded only from layer references. Styles with `isPublishable: false` and names starting with `_` or `.` remain listed.
  2. **R2 (Deleted style on a layer):** Show **no** style chip on a layer whose referenced style is deleted/orphan; show the layer's raw fill/stroke/text/effect/grid values unchanged, and keep `style_refs` so `.fig` export round-trips unchanged. Styles from a library that is not enabled but is embedded in the file still show on the layer under their embedded name.
  3. **R3 (Empty name):** A listed table style whose name is empty or whitespace-only displays as `"Unnamed fill style"`, `"Unnamed text style"`, `"Unnamed effect style"`, or `"Unnamed grid style"`, with the raw ID shown only in the tooltip.
  4. **R4 (Order & folders):** Keep Studio's existing `#129` folder and sort logic unchanged.

---

## 2. Changes Made

### Allowed Scope
1. [`crates/designos-doc/src/lib.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1165-orphan-style-names/crates/designos-doc/src/lib.rs) & [`crates/designos-doc/src/library_ops.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1165-orphan-style-names/crates/designos-doc/src/library_ops.rs):
   - Added `Document::unnamed_style_label(prop: StyleProperty) -> &'static str`, `Document::is_unnamed_style(&self, key: &str) -> bool`, `Document::is_orphan_style(&self, key: &str) -> bool`, and `Document::is_table_style(&self, key: &str) -> bool`.
   - Updated `Document::style_name` to return the typed `"Unnamed ... style"` fallback when a table style's name is empty or whitespace-only.
   - Filtered `publishable_assets` by `doc.is_table_style(k)` so orphan references are never published as library styles.
2. [`crates/designos-fig/src/fig.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1165-orphan-style-names/crates/designos-fig/src/fig.rs) & [`crates/designos-fig/src/rest_import.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1165-orphan-style-names/crates/designos-fig/src/rest_import.rs):
   - Preserved `node.style_refs` in `Lk::style_id_of` for soft-deleted (`isSoftDeletedStyle`, `isSoftDeleted`), `phase == "REMOVED"`, and missing styles while keeping `Lk::style_node_of` returning `None` so consumer layers retain their raw values and round-trip `style_refs` on `.fig` export.
   - Updated `style_soft_deleted` in `fig.rs` and `style_meta` in `rest_import.rs` to exclude `isSoftDeleted`, `isSoftDeletedStyle`, `phase == "REMOVED"`, and un-keyed orphan style entries re-imported after `.fig` export.
   - Prevented consumer seeding in `rest_import.rs` from inserting unresolved `"key:..."` asset refs into `doc.styles`, allowing `fig_export.rs` to round-trip `assetRef` references unchanged.
3. [`crates/designos-app/src/styles.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1165-orphan-style-names/crates/designos-app/src/styles.rs), [`crates/designos-app/src/panels/style_picker.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1165-orphan-style-names/crates/designos-app/src/panels/style_picker.rs), & [`crates/designos-app/src/panels/properties.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1165-orphan-style-names/crates/designos-app/src/panels/properties.rs):
   - Filtered `style_entries_of` by `doc.is_table_style(k)` at listing time (reopened `.designos` files with legacy orphans hide them without migration).
   - Updated `style_picker::linked_style`, `properties::paint_title`, `properties::paint_rows`, and `properties::styles_list` to require `app.doc.is_table_style(k)`, so layers pointing at a deleted/orphan style render no style chip and display their raw values.
   - Attached the raw style ID as the menu-item / row tooltip only when `app.doc.is_unnamed_style(key)` is true.
4. [`crates/designos-mcp/src/engine.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1165-orphan-style-names/crates/designos-mcp/src/engine.rs) & [`crates/designos-mcp/src/tools/search_design_system.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1165-orphan-style-names/crates/designos-mcp/src/tools/search_design_system.rs):
   - Filtered `designos_get_document` (`styles` and `style_count`) and `designos_search_design_system` by `doc.is_table_style(k)`.

### Files Outside Scope
- None.

---

## 3. Red / Green Test Evidence (`US-01` AC-02, AC-03, AC-04)

- **Red log:** `plans/reports/agy-1165-orphan-style-names-evidence/red.log`
- **Green log:** `plans/reports/agy-1165-orphan-style-names-evidence/green.log`
- **Test names:**
  1. `a_soft_deleted_style_is_ignored` (`crates/designos-fig/tests/fig_effect_grid_styles.rs`)
  2. `us01_ac02_and_ac03_deleted_removed_and_orphan_styles_keep_layer_values_and_round_trip_style_refs` (`crates/designos-fig/tests/fig_effect_grid_styles.rs`)
  3. `us01_ac04_empty_and_whitespace_style_names_use_typed_unnamed_fallbacks` (`crates/designos-fig/tests/fig_effect_grid_styles.rs`)
  4. `panels::style_picker::tests::us01_ac02_orphan_styles_are_excluded_from_picker_and_reopened_designos_while_unpublished_stay_listed` (`crates/designos-app/src/panels/style_picker.rs`)
  5. `panels::style_picker::tests::us01_ac03_layer_with_deleted_or_orphan_style_shows_no_style_chip` (`crates/designos-app/src/panels/style_picker.rs`)
  6. `panels::style_picker::tests::us01_ac04_unnamed_style_shows_fallback_label_and_id_only_in_tooltip` (`crates/designos-app/src/panels/style_picker.rs`)
  7. `tools::search_design_system::tests::us01_ac02_mcp_excludes_orphan_styles_from_search_and_get_document` (`crates/designos-mcp/src/tools/search_design_system.rs`)

---

## 4. Visual Evidence (`1440x900`, `--no-toast`)

1. **Layer with deleted/orphan effect style (`1186:100350`) showing raw values and no style chip in the Inspector:**
   - `plans/reports/agy-1165-orphan-style-names-evidence/inspector-deleted-style-raw-values.png`
2. **Effect style picker excluding orphan `1186:100350` and showing `"Unnamed effect style"` for an empty-named table style:**
   - `plans/reports/agy-1165-orphan-style-names-evidence/picker-unnamed-effect-style.png`

---

## 5. Gate Verification (Under Isolated Harness)

### 5.1 `cargo fmt --all -- --check`
```text
$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo fmt --all -- --check
FMT_EXIT=0
```

### 5.2 `cargo clippy --workspace --all-targets --locked -- -D warnings`
```text
$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo clippy --workspace --all-targets --locked -- -D warnings
   Compiling designos-mcp v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1165-orphan-style-names/crates/designos-mcp)
    Checking designos-app v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1165-orphan-style-names/crates/designos-app)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 35.39s
CLIPPY_EXIT=0
```

### 5.3 `cargo test --workspace --locked --lib`
```text
$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test --workspace --locked --lib
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 633 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.40s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 136 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 1.01s
test result: ok. 121 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 81.31s
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.18s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.16s
test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.05s
test result: ok. 246 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.90s
test result: ok. 55 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 35.67s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.24s
WORKSPACE_LIB_EXIT=0
```

### 5.4 `cargo test -p designos-app --bin designos --locked` (+ 3x isolated rerun of known flake `#996`/`#1211`)
```text
$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked
test result: FAILED. 3337 passed; 1 failed; 11 ignored; 0 measured; 0 filtered out; finished in 1337.22s
failures:
    shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one

=== FLAKE RERUN 1 ===
running 1 test
test shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3348 filtered out; finished in 0.97s

=== FLAKE RERUN 2 ===
running 1 test
test shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3348 filtered out; finished in 0.64s

=== FLAKE RERUN 3 ===
running 1 test
test shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3348 filtered out; finished in 0.73s
```

### 5.5 `python3 scripts/zero_fake_ui_gate.py` & `python3 scripts/check_spacing_relationships.py`
```text
$ python3 scripts/zero_fake_ui_gate.py
 Gate Verdict: PASS (17/17 invariants satisfied)
ZERO_FAKE_EXIT=0

$ python3 scripts/check_spacing_relationships.py
literal geometry outside theme.rs: offset_literals=1668, inset_literals=45, radius_literals=257, font_literals=25, nonconcentric_pairs=2
note: 14 entries are below baseline; run --write-baseline to lock the gain in
PASS: no count rose above the baseline
SPACING_EXIT=0
```
