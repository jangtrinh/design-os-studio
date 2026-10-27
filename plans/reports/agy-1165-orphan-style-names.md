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

---

## Round 2

### 1. What Could Not Be Verified, Conclusion, Epistemic Ledger & Weakest Link (Fable Loop v1.3)

- **What could NOT be verified:** None — every Round 2 requirement, test assertion, and verification gate was executed and verified directly on the 200,000-node fixture and full workspace test suites.
- **Conclusion:** `Document::is_orphan_style` and `Document::is_table_style` are now strictly $O(1)$ per key (`O(log S)` on `BTreeMap<String, Style>` / `BTreeMap<String, StyleMeta>` where `S = styles.len()`) and never access `Document::nodes`. No field was added to the saved `.designos` format.
- **Epistemic Ledger:**
  - **`[FACT]`** In Round 1 (`d432defa`), `Document::is_orphan_style` contained `let is_rest_consumer_seed = key.starts_with("S:") && self.nodes.values().any(|n| n.style_refs.values().any(|v| v == key));`, which scanned `self.nodes` in $O(N)$ on every `"S:..."` key lacking `style_meta`. Because `Document::is_table_style` calls `is_orphan_style`, UI inspector methods (`properties::paint_rows`, `paint_title`, `styles_list`) and `style_entries_of` incurred an $O(N)$ or $O(S \times N)$ node walk (taking `1.810s` for `style_entries_of` on a 200,000-node / 50-style document in debug).
  - **`[FACT]`** Extracting `is_orphan_style_in(styles: &BTreeMap<String, Style>, style_meta: &BTreeMap<String, StyleMeta>, key: &str) -> bool` in [`crates/designos-doc/src/lib.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1165-orphan-style-names/crates/designos-doc/src/lib.rs) proves by construction (and via Rust's borrow checker while holding `let _nodes_exclusive = &mut doc.nodes;`) that `Document::nodes` is never read.
  - **`[FACT]`** On the 200,000-node / 50-style benchmark test (`us01_ac02_round2`), `style_entries_of` dropped from `1.810319125s` (RED on `d432defa`) to `< 1 ms` in debug (total test binary time `0.82s` including 200k-node allocation) and `< 1 ms` in release (`0.12s` total test binary time).
  - **`[INFERENCE]`** Any `"S:..."` key present in `doc.styles` without an entry in `doc.style_meta` is an un-tabled style (either consumer-seeded by `rest_import.rs` when a layer references a missing/soft-deleted/`REMOVED` REST style, or a synthetic test entry that bypassed `style_meta`), regardless of whether any node currently references it.
  - **`[ASSUMPTION]`** None required beyond the verified repository callsites.
- **Weakest link (one line):** Synthetic unit tests that manually insert `"S:..."` keys directly into `doc.styles` without inserting into `doc.style_meta` will have `doc.is_table_style("S:...") == false` if they ever query `is_table_style` (audited across all workspace crates; no test expects a bare `"S:..."` without `style_meta` to be a listed table style).

---

### 2. Skills Loaded & Changes Driven by Them

1. [`/es:dev`](file:///Users/jangtrinh/.gemini/config/skills/es-dev/SKILL.md) & [`es:feature-dev`](file:///Users/jangtrinh/.gemini/config/skills/es-feature-dev/SKILL.md) — Operated directly in the DEV execution phase against the accepted Round 2 spec, committing the failing RED test (`79c0a498f`) on top of `d432defa` before applying the fix commit.
2. [`/es:rust`](file:///Users/jangtrinh/.gemini/config/skills/es-rust/SKILL.md) — Used split field borrows (`is_orphan_style_in(&self.styles, &self.style_meta, key)`) so the compiler statically enforces disjointness from `self.nodes`, allowing the unit test to hold an exclusive `&mut doc.nodes` borrow across the orphan check on all 50 keys.
3. [`/es:rust-gpu-canvas`](file:///Users/jangtrinh/.gemini/config/skills/es-rust-gpu-canvas/SKILL.md) — Enforced the `CLAUDE.md` §4 invariant that per-frame UI/inspector lookups (`properties::paint_rows`, `paint_title`, `styles_list`, `style_entries_of`) remain $O(1)$ per lookup and never traverse the scene graph (`doc.nodes`).
4. [`es:fable-thinking`](file:///Users/jangtrinh/.gemini/config/skills/es-fable-thinking/SKILL.md) & [`es:astra-work`](file:///Users/jangtrinh/.gemini/config/skills/es-astra-work/SKILL.md) — Exhaustively audited every `"S:..."` producer in the workspace before changing classification logic, predicted test outcomes before running commands, and verified all 8 gates with raw output. (No UI layout/token change was made in Round 2, so `es-designer` was not triggered.)

---

### 3. Audit: Which `S:` Keys Reach `doc.styles` Without `style_meta`, Where Each Comes From, and How Each Is Classified Now

| Source of `"S:..."` Key in `doc.styles` | Callsite | Why `style_meta` Is Absent (or Present) | Classification in Round 1 (`d432defa`) | Classification Now (Round 2) |
|---|---|---|---|---|
| **1. Figma REST consumer layer seeding for deleted / `REMOVED` / missing styles** | [`crates/designos-fig/src/rest_import.rs:1279-1298`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1165-orphan-style-names/crates/designos-fig/src/rest_import.rs#L1279-L1298) (`Ctx::import_node`) | When a layer in Figma REST JSON has `"styles": { slot: "S:..." }`, `import_node` seeds `doc.styles.entry("S:...").or_insert_with(...)` from the layer's raw fill/text/effect/grid values. Later, `style_meta(v.get("styles"), &cx.doc)` ([`rest_import.rs:660, 794, 3325-3374`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1165-orphan-style-names/crates/designos-fig/src/rest_import.rs#L3325-L3374)) populates `doc.style_meta` only for keys present in the top-level REST `"styles"` map that are not `isSoftDeleted`, `isSoftDeletedStyle`, or `phase == "REMOVED"`. | Orphan **only while** at least one node in `doc.nodes` still had `style_refs` pointing to `"S:..."` (via $O(N)$ `self.nodes.values().any(...)`); flipped to non-orphan if the last referencing layer was unlinked/deleted. | **Orphan (`is_orphan_style == true`, `is_table_style == false`) unconditionally in $O(1)$** via `key.starts_with("S:") && !style_meta.contains_key(key)`. Excluded from pickers, Assets, MCP listings, `publishable_assets`, and layer chips even if all referencing nodes are later unlinked or deleted. |
| **2. Reopened `.designos` files containing legacy REST consumer-seeded `"S:..."` entries** | [`crates/designos-doc/src/lib.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1165-orphan-style-names/crates/designos-doc/src/lib.rs) (`Document::from_json`) | Saved `.designos` documents persist both `styles` and `style_meta`. Legacy files saved after REST import carry consumer-seeded `"S:..."` keys in `styles` with no entry in `style_meta`. | Orphan only if referenced by a node (requiring an $O(N)$ node scan on every lookup). | **Orphan (`is_orphan_style == true`, `is_table_style == false`) in $O(1)$** without any `.designos` schema change or migration. |
| **3. Live Figma REST table styles (`S:fill1`, `S:text1`, `S:fx1`, `S:1`..`S:5`, `S:heading`, `S:body`, `S:surface`)** | [`crates/designos-fig/src/rest_import.rs:3325-3374`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1165-orphan-style-names/crates/designos-fig/src/rest_import.rs#L3325-L3374), [`crates/designos-fig/tests/rest_upgrade.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1165-orphan-style-names/crates/designos-fig/tests/rest_upgrade.rs), [`crates/designos-doc/src/live_sync_tests.rs:307-360`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1165-orphan-style-names/crates/designos-doc/src/live_sync_tests.rs#L307-L360) | Top-level REST `"styles"` entries that are not soft-deleted or `REMOVED` are inserted into `doc.style_meta` by `style_meta(...)`. | Table style (`is_orphan_style == false`, `is_table_style == true`). | **Table style (`is_orphan_style == false`, `is_table_style == true`) in $O(1)$** because `style_meta.contains_key(key)` is `true`. |
| **4. Synthetic test fixtures inserting bare `"S:..."` into `doc.styles` without `style_meta`** | <ul><li>[`crates/designos-app/src/panels/style_picker.rs:282, 377`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1165-orphan-style-names/crates/designos-app/src/panels/style_picker.rs#L282) (`"S:orphan-fx"`, `"S:orphan-00".."S:orphan-19"`)</li><li>[`crates/designos-fig/tests/fig_roundtrip.rs:3006`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1165-orphan-style-names/crates/designos-fig/tests/fig_roundtrip.rs#L3006) (`"S:grid-auto-max"`)</li><li>[`crates/designos-doc/src/branch_diff_tests.rs:273-274`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1165-orphan-style-names/crates/designos-doc/src/branch_diff_tests.rs#L273-L274) (`"S:primary"`)</li><li>[`crates/designos-doc/src/live_patch/tests.rs:18, 532, 594, 655, 966`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1165-orphan-style-names/crates/designos-doc/src/live_patch/tests.rs#L18) (`"S:1"`, `"S:2"`, `"S:new"`)</li></ul> | Unit tests constructing minimal `Document` structs directly in memory without populating `style_meta`. (`export_fig` in `fig_roundtrip.rs:3006` iterates `&doc.styles` directly and assigns a `"2:N"` GUID + `StyleMeta` on `.fig` re-import; `branch_diff_tests` and `live_patch/tests` test raw `BTreeMap` diff/patch operations and never call `is_table_style`.) | Orphan if a node had `style_refs` pointing to the key (`style_picker.rs:282`); non-orphan if no node referenced it (`S:orphan-19` in Round 2 test). | **Orphan (`is_orphan_style == true`, `is_table_style == false`) in $O(1)$** whether or not a node references it. All unit and integration tests across the workspace pass. |

---

### 4. Red-First Test & Fix Summary

1. **Commit 1 (RED test on top of `d432defa` — `79c0a498f`):**
   - Added `tests::us01_ac02_round2_is_table_style_and_is_orphan_style_do_not_walk_200k_nodes` in [`crates/designos-doc/src/lib.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1165-orphan-style-names/crates/designos-doc/src/lib.rs) and `panels::style_picker::tests::us01_ac02_round2_style_entries_of_on_200k_nodes_and_50_styles_is_o1_per_key_and_under_budget` in [`crates/designos-app/src/panels/style_picker.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1165-orphan-style-names/crates/designos-app/src/panels/style_picker.rs).
   - Builds a `Document` with **200,000 nodes** (`NodeId(1)..=NodeId(200_000)`) and **50 styles** (30 valid table styles with `StyleMeta`, including 10 `"S:table-..."` REST styles, plus 20 `"S:orphan-00".."S:orphan-19"` styles without `StyleMeta`, including `"S:orphan-19"` with zero referencing nodes).
   - **Observed RED failure on `d432defa`** (saved in [`plans/reports/agy-1165-orphan-style-names-evidence/round2-red.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1165-orphan-style-names/plans/reports/agy-1165-orphan-style-names-evidence/round2-red.log)):
     ```text
     $ cargo test -p designos-doc --lib --locked us01_ac02_round2
     test tests::us01_ac02_round2_is_table_style_and_is_orphan_style_do_not_walk_200k_nodes ... FAILED
     assertion `left == right` failed: is_table_style("S:orphan-19") mismatch
       left: true
      right: false
     test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 633 filtered out; finished in 3.61s

     $ cargo test -p designos-app --bin designos --locked us01_ac02_round2
     test panels::style_picker::tests::us01_ac02_round2_style_entries_of_on_200k_nodes_and_50_styles_is_o1_per_key_and_under_budget ... FAILED
     style_entries_of on 200,000 nodes and 50 styles took 1.810319125s (budget 50ms); must not walk nodes
     test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 3349 filtered out; finished in 2.39s
     ```
2. **Commit 2 (GREEN fix in [`crates/designos-doc/src/lib.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1165-orphan-style-names/crates/designos-doc/src/lib.rs)):**
   - Replaced the `self.nodes.values().any(...)` scan with pure table lookup helper `is_orphan_style_in(styles: &BTreeMap<String, Style>, style_meta: &BTreeMap<String, StyleMeta>, key: &str) -> bool`:
     ```rust
     fn is_orphan_style_in(
         styles: &BTreeMap<String, Style>,
         style_meta: &BTreeMap<String, StyleMeta>,
         key: &str,
     ) -> bool {
         if !styles.contains_key(key) || style_meta.contains_key(key) {
             return false;
         }
         let is_guid = key
             .split_once(':')
             .is_some_and(|(a, b)| !a.is_empty() && !b.is_empty() && a.bytes().all(|c| c.is_ascii_digit()));
         is_guid || key.starts_with("key:") || key.starts_with("S:")
     }
     ```
   - Added a compile-time borrow-checker proof in `us01_ac02_round2_is_table_style_and_is_orphan_style_do_not_walk_200k_nodes` holding `let nodes_exclusive = &mut doc.nodes;` across `is_orphan_style_in(&doc.styles, &doc.style_meta, &k)` for all 50 keys.
   - **Observed GREEN result** (saved in [`plans/reports/agy-1165-orphan-style-names-evidence/round2-green.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1165-orphan-style-names/plans/reports/agy-1165-orphan-style-names-evidence/round2-green.log)):
     ```text
     $ cargo test -p designos-doc --lib --locked us01_ac02_round2
     running 1 test
     test tests::us01_ac02_round2_is_table_style_and_is_orphan_style_do_not_walk_200k_nodes ... ok
     test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 633 filtered out; finished in 1.11s

     $ cargo test -p designos-doc --lib --release --locked us01_ac02_round2
     running 1 test
     test tests::us01_ac02_round2_is_table_style_and_is_orphan_style_do_not_walk_200k_nodes ... ok
     test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 633 filtered out; finished in 0.12s

     $ cargo test -p designos-app --bin designos --locked us01_ac02_round2
     running 1 test
     test panels::style_picker::tests::us01_ac02_round2_style_entries_of_on_200k_nodes_and_50_styles_is_o1_per_key_and_under_budget ... ok
     test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3349 filtered out; finished in 0.82s
     ```

---

### 5. Exact Gate Commands and Outputs (Round 2)

#### 5.1 `cargo fmt --all -- --check`
```text
$ cargo fmt --all -- --check
FMT_EXIT=0
```

#### 5.2 `cargo clippy --workspace --all-targets --locked -- -D warnings`
```text
$ cargo clippy --workspace --all-targets --locked -- -D warnings
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1165-orphan-style-names/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1165-orphan-style-names/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
    Checking designos-doc v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1165-orphan-style-names/crates/designos-doc)
    Checking designos-svg v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1165-orphan-style-names/crates/designos-svg)
    Checking designos-fig v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1165-orphan-style-names/crates/designos-fig)
    Checking designos-render v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1165-orphan-style-names/crates/designos-render)
    Checking designos-library v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1165-orphan-style-names/crates/designos-library)
    Checking designos-gpu v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1165-orphan-style-names/crates/designos-gpu)
    Checking designos-export v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1165-orphan-style-names/crates/designos-export)
    Checking designos-diff v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1165-orphan-style-names/crates/designos-diff)
    Checking designos-mcp v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1165-orphan-style-names/crates/designos-mcp)
    Checking designos-app v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1165-orphan-style-names/crates/designos-app)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2m 53s
CLIPPY_EXIT=0
```

#### 5.3 `cargo test --workspace --locked --lib`
```text
$ cargo test --workspace --locked --lib
Running unittests src/lib.rs (designos-app)
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
Running unittests src/lib.rs (designos-doc)
test result: ok. 634 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.12s
Running unittests src/lib.rs (designos-DS-bridge)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
Running unittests src/lib.rs (designos-export)
test result: ok. 136 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.21s
Running unittests src/lib.rs (designos-fig)
test result: ok. 121 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 77.16s
Running unittests src/lib.rs (designos-fonts)
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
Running unittests src/lib.rs (designos-grid-Import)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s
Running unittests src/lib.rs (designos-gpu)
test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.64s
Running unittests src/lib.rs (designos-library)
test result: ok. 246 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 28.13s
Running unittests src/lib.rs (designos-mcp)
test result: ok. 55 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 67.00s
Running unittests src/lib.rs (designos-render)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running unittests src/lib.rs (designos-svg)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.26s
WORKSPACE_LIB_EXIT=0
```

#### 5.4 `cargo test -p designos-fig --locked`
```text
$ cargo test -p designos-fig --locked
Running unittests src/lib.rs (designos-fig)
test result: ok. 121 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 102.57s
Running tests/All_Pages_Load.rs
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.20s
Running tests/Component_Variant_Props.rs
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/ Derived_Symbol_Override.rs
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/Export_Variables.rs
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
Running tests/Fig_Export.rs
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
Running tests/Instance_Overrides.rs
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
Running tests/Nested_Instance_Overrides.rs
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
Running tests/Override_Table.rs
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
Running tests/Page_Divider.rs
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.65s
Running tests/Preserve_Unsupported.rs
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
Running tests/Prototype_Interactions.rs
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.16s
Running tests/Rest_Import.rs
test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
Running tests/Rest_Upgrade.rs
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.13s
Running tests/Section_Import.rs
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
Running tests/Slot_Children.rs
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
Running tests/Style_Roundtrip.rs
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.78s
Running tests/Symbol_Swap.rs
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
Running tests/fig_roundtrip.rs
test result: ok. 38 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.88s
Running tests/fig_v2_gaps.rs
test result: ok. 32 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.16s
Running tests/fractional_position_sort.rs
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
Running tests/kiwi_probe.rs
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s
Running tests/layout_axes.rs
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
Running tests/mask_band.rs
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
Running tests/multi_style_text.rs
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.73s
Running tests/pcp_text_line_height.rs
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.77s
Running tests/rest_import.rs
test result: ok. 34 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
Running tests/symbol_auto_layout.rs
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
Running tests/text_auto_resize.rs
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
Running tests/text_baseline.rs
test result: ok. 0 passed; 0 failed; 6 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/text_decoration.rs
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
Running tests/text_list.rs
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
Running tests/text_truncation.rs
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/variable_alias.rs
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/variable_modes.rs
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/vector_network.rs
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
Running tests/vector_path_commands.rs
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/vector_stroke_cap.rs
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
Running tests/vector_winding.rs
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
Running tests/version_history.rs
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
FIG_EXIT=0
```

#### 5.5 `cargo test -p designos-mcp --locked`
```text
$ cargo test -p designos-mcp --locked
Running unittests src/lib.rs (target/debug/deps/designos_mcp-f66050606b2fb386)
test result: ok. 246 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 9.10s
Running unittests src/bin/designos-mcp.rs (target/debug/deps/designos_mcp-8d24468186349746)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running unittests src/main.rs (target/debug/deps/designos_mcp-c7df563bd92543d8)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
Running tests/apply_spec_auto_layout.rs (target/debug/deps/apply_spec_auto_layout-500816fb3892363e)
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.16s
Running tests/batch_mutate.rs (target/debug/deps/batch_mutate-c298f6180eb26c88)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.18s
Running tests/comments_storage.rs (target/debug/deps/comments_storage-4bd7bf09f89c52b2)
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
Running tests/fig_export_roundtrip.rs (target/debug/deps/fig_export_roundtrip-30b58ee27a728447)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.52s
Running tests/font_coverage.rs (target/debug/deps/font_coverage-bc69ba426085d3c6)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.60s
Running tests/headless_png_render.rs (target/debug/deps/headless_png_render-7eb1713bb828bc69)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.65s
Running tests/live_bridge.rs (target/debug/deps/live_bridge-1ce952946f18bd17)
test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.38s
Running tests/mcp_create_node_bounds.rs (target/debug/deps/mcp_create_node_bounds-24df9de7bc5b708b)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s
Running tests/mcp_dev_mode.rs (target/debug/deps/mcp_dev_mode-de56f6c47093efde)
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.23s
Running tests/mcp_prototype_tools.rs (target/debug/deps/mcp_prototype_tools-f0ab1280ec9a49c0)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
Running tests/session_files.rs (target/debug/deps/session_files-a7d5bc308ef70d5c)
test result: ok. 42 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.80s
Running tests/stdio_framing.rs (target/debug/deps/stdio_framing-578028f7eb16686b)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
Running tests/style_crud.rs (target/debug/deps/style_crud-209062479b659348)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
Running tests/svg_import.rs (target/debug/deps/svg_import-c192bb442194ca83)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
Running tests/token_sync.rs (target/debug/deps/token_sync-69bd025a3734955e)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
Running tests/variable_crud.rs (target/debug/deps/variable_crud-1fc8da7de7e8c92e)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
Running tests/version_history.rs (target/debug/deps/version_history-b3cb19ab43c32e8d)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
MCP_EXIT=0
```

#### 5.6 `cargo test -p designos-app --bin designos --locked` (+ 3x isolated rerun of known flake `#996`/`#1211`)
```text
$ env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" GEMINI_CLI_HOME="$S/home" CODEX_HOME="$S/home/.codex" CLAUDE_CONFIG_DIR="$S/home/.claude" cargo test -p designos-app --bin designos --locked
test result: FAILED. 3338 passed; 1 failed; 11 ignored; 0 measured; 0 filtered out; finished in 1369.12s
failures:
    shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one

=== FLAKE RERUN 1 ===
running 1 test
test shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3349 filtered out; finished in 0.63s

=== FLAKE RERUN 2 ===
running 1 test
test shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3349 filtered out; finished in 0.72s

=== FLAKE RERUN 3 ===
running 1 test
test shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3349 filtered out; finished in 0.65s
```

#### 5.7 `python3 scripts/zero_fake_ui_gate.py` & `python3 scripts/check_spacing_relationships.py`
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
