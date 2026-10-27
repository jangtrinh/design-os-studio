# Report — DEV · #1140 (P0): One Self-Contained, Alias-Resolved DTCG Export per Mode (`US-16`)

- **Issue:** `#1140` (`US-16`, spec key `P`)
- **Branch:** `jangtrinh/agy-1140-dtcg-export-bundle` (based on `origin/release/v0.6.4` at `6f0c3bb2`)
- **Target Batch:** `v0.6.5`
- **Evidence Directory:** `plans/reports/agy-1140-dtcg-export-bundle-evidence/`
  - `red.log` — Red-phase test failure output for Slice 1 (Tests 1–4) and Slice 2 (Tests 5–11)
  - `green.log` — Green-phase test pass output for all 11 spec tests
  - `synthetic-kit-1140.designos` — Self-made synthetic fixture reproducing the kit's 10-collection / 12-mode / duplicate-name / cross-collection-alias / alpha / alpha-0 shape
  - `dark.tokens.json` — Single-mode resolved hex DTCG export emitted from `synthetic-kit-1140.designos`
  - `cli-dogfood.log` — Full CLI directory export, `.zip` export, single-mode `dark.tokens.json` export, `ui ds import`, and `ui ds a11y` output

---

## 1. What Could NOT Be Verified

1. **Real Untitled UI PRO VARIABLES v8.0 `.fig` kit execution (`AC-04` and `AC-05` on the proprietary kit file):**
   - Per **Controller Ruling #2** (*"AC-04 and AC-05 run on the kit, so they belong to the controller. You must never open the Untitled UI kit or any client file"*), the real kit file at `~/Downloads/❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig` was never opened or read.
   - Instead, per Controller Ruling #2, we constructed [`synthetic-kit-1140.designos`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1140-dtcg-export-bundle/plans/reports/agy-1140-dtcg-export-bundle-evidence/synthetic-kit-1140.designos) with the exact collection/mode/token topology (`_Primitives` local + library, `1. Color modes` local + library with `Light mode` and `Dark mode` and 115 semantic colour tokens aliasing into `_Primitives (2)`, `2. Radius`, `3. Spacing`, `4. Widths`, `5. Containers`, and `6. Typography` local [32 tokens: 10 string + 22 number] + library [1 token], plus `#FFFFFF80` alpha and `#FFFFFF00` alpha-0 tokens) and verified the full CLI + `/opt/homebrew/bin/ui ds import` + `/opt/homebrew/bin/ui ds a11y` pipeline on it.
2. **Default `/Users/jangtrinh/.local/bin/ui` path:**
   - `/Users/jangtrinh/.local/bin/ui` is not present on this machine; the installed `ease-design` `ui` binary (`v0.6.0`) resides at `/opt/homebrew/bin/ui` and was used for the dogfood verification.
3. **2 of 3,348 `designos-app` binary tests under full-suite parallel CPU load:**
   - During the full 3,348-test `cargo test -p designos-app --bin designos --locked` run alongside concurrent worker builds, `shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one` and `terminal::tests::a_draft_on_an_exited_tab_follows_the_click_to_another_live_tab` timed out waiting for background PTY exit status; both passed 3/3 solo reruns in `0.36s–1.25s` (pasted in §4).

---

## 2. Drift Table (`35a65796` Spec Baseline vs. `HEAD` `6f0c3bb2`)

| Symbol / Site | Spec `file:line` (`35a65796`) | `HEAD` `file:line` (`6f0c3bb2`) | Drift & Notes |
|---|---|---|---|
| `zip_entries` (`FigError::InvalidMagic` on duplicate zip entry) | `crates/designos-fig/src/variables_dtcg.rs:109-121` | `crates/designos-fig/src/variables_dtcg.rs:109-121` | `0` lines drift |
| `color_to_dtcg_obj` | `crates/designos-fig/src/variables_dtcg.rs:491-506` | `crates/designos-fig/src/variables_dtcg.rs:491-506` | `0` lines drift |
| `resolve_literal_in_mode` | `crates/designos-fig/src/variables_dtcg.rs:541-568` | `crates/designos-fig/src/variables_dtcg.rs:541-568` | `0` lines drift |
| `export_variables` | `crates/designos-fig/src/variables_dtcg.rs:614-628` | `crates/designos-fig/src/variables_dtcg.rs:614-628` | `0` lines drift |
| `export_figma_native` duplicate stem/mode disambiguation | `crates/designos-fig/src/variables_dtcg.rs:751`, `:816-833` | `crates/designos-fig/src/variables_dtcg.rs:751`, `:816-833` | `0` lines drift |
| `export_dtcg` | `crates/designos-fig/src/variables_dtcg.rs:844-1014` | `crates/designos-fig/src/variables_dtcg.rs:844-1014` | `0` lines drift |
| `stage_dtcg_files` (manifest importer) | `crates/designos-fig/src/variables_dtcg.rs:1697-1754` | `crates/designos-fig/src/variables_dtcg.rs:1697-1754` | `0` lines drift |
| `assemble_staged_document` (`coll_by_group_name`) | `crates/designos-fig/src/variables_dtcg.rs:2267-2277` | `crates/designos-fig/src/variables_dtcg.rs:2267-2277` | `0` lines drift |
| `variables_dtcg::tests` | `crates/designos-fig/src/variables_dtcg.rs:2373+` | `crates/designos-fig/src/variables_dtcg.rs:2373+` | `0` lines drift |
| `cli::Options` (`import_variables`, `export_variables`, `format`) | `crates/designos-app/src/cli.rs:91-93` | `crates/designos-app/src/cli.rs:89-93` | `0` lines drift |
| `cli::parse` (`--export-variables`, `--format`) | `crates/designos-app/src/cli.rs:317-324` | `crates/designos-app/src/cli.rs:317-324` | `0` lines drift |
| `cli::USAGE` | `crates/designos-app/src/cli.rs:359-377` | `crates/designos-app/src/cli.rs:373-391` | `+14` lines drift |
| `run_agent_cli` `--export-variables` branch | `crates/designos-app/src/main.rs:426-461` | `crates/designos-app/src/main.rs:445-480` | `+19` lines drift (due to sibling `.tokens.json` collection in `--import-variables` at `:417-430`) |

---

## 3. Root Cause (`file:line`)

1. **Duplicate DTCG group and file names across same-named collections (`crates/designos-fig/src/variables_dtcg.rs:850-854`, `:864`, `:992-995`, `:1006` on `6f0c3bb2`):**
   - `export_dtcg` computed `coll_group_names` solely via `sanitize_dtcg_segment(&coll.name)` without deduplication across collections,unlike `export_figma_native` (`variables_dtcg.rs:751`, `:816-833`).
   - When a file had both a local collection (`source_library_key: None`) and a subscribed library copy (`source_library_key: Some(...)`) with the same `name` (e.g. `_Primitives`, `1. Color modes`, `6. Typography`), both produced identical `file_name` entries (`6_ Typography.Value.tokens.json`) and identical top-level group keys (`6_ Typography`).
   - In directory exports (`crates/designos-app/src/main.rs:461-471` on `6f0c3bb2`), `std::fs::write(&dest, bytes)` silently overwrote the local 32-token file with the 1-token library copy.
   - In `.zip` exports (`crates/designos-fig/src/variables_dtcg.rs:115`), `zip::ZipWriter::start_file` rejected the duplicate entry name and mapped the error to `FigError::InvalidMagic(format!("zip start_file {name}: {e}"))` (`"not a fig file: zip start_file ...: Duplicate filename"`).
2. **Manifest round-trip alias resolution by `group` (`crates/designos-fig/src/variables_dtcg.rs:1697-1754`, `:2267-2277` on `6f0c3bb2`):**
   - `stage_dtcg_files` read `c_obj.get("name")` from `tokens.manifest.json` (`:1712`) to preserve `VariableCollection.name`, but ignored `c_obj.get("group")`.
   - `assemble_staged_document` (`:2271-2277`) indexed `coll_by_group_name` only by `c.name` and `sanitize_dtcg_segment(&c.name)`. When two collections shared `name == "_Primitives"`, the second overwrote the first in `coll_by_group_name`, and aliases referencing `{_Primitives (2).…}` could not resolve to the second collection.
   - Propagating `collection_group: Option<String>` from `stage_dtcg_files` into `ParsedToken` and indexing `explicit_group_by_coll` in `assemble_staged_document` resolves `{_Primitives (2).…}` to the library collection while keeping `VariableCollection.name == "_Primitives"`.
3. **Lack of single-mode self-contained DTCG export, alias resolution, and hex colour formatting (`crates/designos-fig/src/variables_dtcg.rs:491-506`, `:844-1014` and `crates/designos-app/src/main.rs:445-480` on `6f0c3bb2`):**
   - `export_dtcg` always emitted a multi-file `ExportBundle` (`tokens.manifest.json` + one `.tokens.json` per `(collection, mode)`), wrote cross-collection aliases as unresolved `"{Group.path}"` strings, and formatted colours exclusively as DTCG sRGB objects (`color_to_dtcg_obj`).
   - Passing a `.json` path to `--export-variables x.json --format dtcg` fell into the directory branch (`main.rs:460`) because `bundle.files.len() > 1`, creating a directory named `x.json`.

---

## 4. Acceptance Table

| Criterion / Gate | Status | Command & Pasted Output |
|---|---|---|
| **AC-01 / Tests 1–4** (Unique DTCG groups & files, `.zip` export, round-trip import, CLI directory overwrite guard) | **PASS** | `cargo test -p designos-fig --locked --lib -- dtcg_export_gives_same_named_collections_unique_groups_and_files dtcg_zip_export_with_same_named_collections_succeeds dtcg_round_trip_keeps_every_collection` and `cargo test -p designos-app --bin designos --locked -- cli_directory_export_never_overwrites_within_one_run` (see `red.log` and `green.log` below) |
| **AC-02 / Tests 5–7, 11** (Single-mode export, `--resolve-aliases` with provenance, `--color-format hex` with `#RRGGBB` / `#RRGGBBAA` / `#FFFFFF00`, `--collection` local preference & `id:<id>`) | **PASS** | `cargo test -p designos-fig --locked --lib -- mode_export_is_one_self_contained_file resolve_aliases_writes_literals_and_keeps_provenance color_format_hex_writes_strings_with_alpha collection_filter_prefers_the_local_copy` (see `red.log` and `green.log` below) |
| **AC-03 / Tests 8–10** (Unknown mode lists available modes, unresolvable alias fails without writing, `.json` path without `--mode` fails without creating directory, wrong flag combinations fail with exit 2) | **PASS** | `cargo test -p designos-fig --locked --lib -- unknown_mode_is_an_error_listing_modes unresolvable_alias_stops_the_export_and_writes_nothing` and `cargo test -p designos-app --bin designos --locked -- dtcg_to_a_json_path_without_mode_is_an_error` (see `red.log` and `green.log` below) |
| **AC-04 & AC-05 Synthetic Fixture + `ui ds import` & `ui ds a11y`** (Per Controller Ruling #2) | **PASS** | `designos synthetic-kit-1140.designos --export-variables ...` + `/opt/homebrew/bin/ui ds import` + `/opt/homebrew/bin/ui ds a11y` (`13` unique files on disk, `32` vs `1` typography tokens, `115` dark mode colour tokens imported, `1307` text×surface pairs audited; pasted below) |
| **`cargo fmt --all -- --check`** | **PASS** (`exit 0`) | Pasted below |
| **`cargo clippy --workspace --all-targets --locked -- -D warnings`** | **PASS** (`exit 0`) | Pasted below |
| **`cargo test --workspace --locked --lib`** | **PASS** (`0 failed`) | Pasted below |
| **`cargo test -p designos-app --bin designos --locked`** | **PASS** (`3335 passed`, 2 PTY exit-timing flakes passed 3/3 solo reruns) | Pasted below |
| **`python3 scripts/zero_fake_ui_gate.py`** | **PASS** (`17/17`) | Pasted below |
| **`python3 scripts/check_spacing_relationships.py`** | **PASS** (`0 above baseline`) | Pasted below |

### 4.1 Red & Green Spec Tests (`red.log` / `green.log`)

**Green output (`plans/reports/agy-1140-dtcg-export-bundle-evidence/green.log`):**
```text
=== GREEN: All 11 spec tests for #1140 ===
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_fig-8c931ce94e001702)

running 9 tests
test variables_dtcg::tests::unknown_mode_is_an_error_listing_modes ... ok
test variables_dtcg::tests::unresolvable_alias_stops_the_export_and_writes_nothing ... ok
test variables_dtcg::tests::resolve_aliases_writes_literals_and_keeps_provenance ... ok
test variables_dtcg::tests::collection_filter_prefers_the_local_copy ... ok
test variables_dtcg::tests::mode_export_is_one_self_contained_file ... ok
test variables_dtcg::tests::color_format_hex_writes_strings_with_alpha ... ok
test variables_dtcg::tests::dtcg_export_gives_same_named_collections_unique_groups_and_files ... ok
test variables_dtcg::tests::dtcg_zip_export_with_same_named_collections_succeeds ... ok
test variables_dtcg::tests::dtcg_round_trip_keeps_every_collection ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 121 filtered out; finished in 0.01s

     Running unittests src/main.rs (out/cargo/debug/deps/designos-f103f8deb2404a79)

running 2 tests
test variables_cli_tests::dtcg_to_a_json_path_without_mode_is_an_error ... ok
test variables_cli_tests::cli_directory_export_never_overwrites_within_one_run ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 3346 filtered out; finished in 0.11s
```

### 4.2 Synthetic Kit CLI Flow + `ui ds import` & `ui ds a11y` (`cli-dogfood.log`)

```text
=== 1. Directory DTCG Export ===
{
  "files": [
    "tokens.manifest.json",
    "_Primitives.Style.tokens.json",
    "1_ Color modes.Light mode.tokens.json",
    "1_ Color modes.Dark mode.tokens.json",
    "1_ Color modes (2).Light mode.tokens.json",
    "1_ Color modes (2).Dark mode.tokens.json",
    "_Primitives (2).Style.tokens.json",
    "2_ Radius.Mode 1.tokens.json",
    "3_ Spacing.Value.tokens.json",
    "4_ Widths.Mode 1.tokens.json",
    "5_ Containers.Value.tokens.json",
    "6_ Typography.Value.tokens.json",
    "6_ Typography (2).Value.tokens.json"
  ],
  "format": "dtcg",
  "omitted_extensions": [],
  "path": "/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.SANnRd/dtcg-dir"
}
Count on disk: 13
6_ Typography.Value.tokens.json token count: 32
6_ Typography (2).Value.tokens.json token count: 1

=== 2. ZIP DTCG Export ===
Zip entry count: 13

=== 3. Single-Mode Resolved Hex DTCG Export (dark.tokens.json) ===
{
  "aliases_resolved": 115,
  "aliases_resolved_via_default_mode": 115,
  "collections": [
    {
      "group": "1_ Color modes",
      "id": "VariableCollectionId:1333:27347",
      "mode": "Dark mode",
      "name": "1. Color modes"
    }
  ],
  "color_format": "hex",
  "format": "dtcg",
  "inlined_for_aliases": [],
  "mode": "Dark mode",
  "omitted_extensions": [],
  "path": "/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.SANnRd/dark.tokens.json",
  "skipped_collections": [
    {
      "id": "VariableCollectionId:1333:99999",
      "modes": [
        "Light mode",
        "Dark mode"
      ],
      "name": "1. Color modes"
    }
  ],
  "tokens": 115
}

=== 4. ui ds import & ui ds a11y on dark.tokens.json ===
ds import: 115 token(s) [115 color] across 5 categories → /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.SANnRd/ds-scratch/design
  111 roles recognized, 12 gaps: card, popover, secondary, muted, accent, input, ring, destructive, success, warning, info, neutral
  soul: /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.SANnRd/ds-scratch/design/soul.md (draft — edit then set status: ratified)
  heartbeat: /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.SANnRd/ds-scratch/design/heartbeat.json
  next: ui ds a11y --dir /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.SANnRd/ds-scratch  ·  ui ds status --dir /var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.SANnRd/ds-scratch
ds a11y: 1307 text×surface pair(s) checked (roles inferred from token names — cartesian; prefer -foreground pairing or --pairs), 585 below AA (4.5:1).
```

### 4.3 Harness Gates Output

**`cargo fmt --all -- --check` & `cargo clippy --workspace --all-targets --locked -- -D warnings`:**
```text
fmt exit: 0
...
    Checking designos-fonts v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1140-dtcg-export-bundle/crates/designos-fonts)
    Checking designos-render v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1140-dtcg-export-bundle/crates/designos-render)
    Checking designos-svg v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1140-dtcg-export-bundle/crates/designos-svg)
    Checking designos-export v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1140-dtcg-export-bundle/crates/designos-export)
    Checking designos-gpu v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1140-dtcg-export-bundle/crates/designos-gpu)
    Checking designos-webcapture v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1140-dtcg-export-bundle/crates/designos-webcapture)
    Checking designos-fig v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1140-dtcg-export-bundle/crates/designos-fig)
    Checking designos-library v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1140-dtcg-export-bundle/crates/designos-library)
    Checking designos-eval v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1140-dtcg-export-bundle/crates/designos-eval)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2m 08s
clippy exit: 0
```

**`cargo test --workspace --locked --lib`:**
```text
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_diff-e9fb9a3e00ecdbf3)
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_doc-a0c6cf32749620e6)
test result: ok. 633 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 9.70s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_eval-029cd168a180af6a)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_export-f5a81d86319362e2)
test result: ok. 136 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 1.00s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_fig-648cb6f438830bb1)
test result: ok. 130 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 84.54s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_fonts-8afb0f60e41df5a3)
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.19s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_gpu-49227ce26b5d6686)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_library-d39027c82797cc1e)
test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 28.36s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_mcp-189215ed087510e0)
test result: ok. 245 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.17s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_render-39473ba3e638dab1)
test result: ok. 55 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 64.50s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_svg-125a628128d3698e)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_webcapture-a59ae055a8fa52f0)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.27s
```

**`cargo test -p designos-app --bin designos --locked` + 3× solo reruns of the 2 PTY exit-timing flakes:**
```text
running 3348 tests
...
test variables_cli_tests::cli_directory_export_never_overwrites_within_one_run ... ok
test variables_cli_tests::dtcg_to_a_json_path_without_mode_is_an_error ... ok
...
test result: FAILED. 3335 passed; 2 failed; 11 ignored; 0 measured; 0 filtered out; finished in 1308.81s

=== Solo rerun 1 ===
test shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3347 filtered out; finished in 1.25s
test terminal::tests::a_draft_on_an_exited_tab_follows_the_click_to_another_live_tab ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3347 filtered out; finished in 0.41s

=== Solo rerun 2 ===
test shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3347 filtered out; finished in 1.03s
test terminal::tests::a_draft_on_an_exited_tab_follows_the_click_to_another_live_tab ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3347 filtered out; finished in 0.38s

=== Solo rerun 3 ===
test shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3347 filtered out; finished in 0.98s
test terminal::tests::a_draft_on_an_exited_tab_follows_the_click_to_another_live_tab ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3347 filtered out; finished in 0.36s
```

**`python3 scripts/zero_fake_ui_gate.py` & `python3 scripts/check_spacing_relationships.py`:**
```text
------------------------------------------------------------------------------------------------------------
 Gate Verdict: PASS (17/17 invariants satisfied)
============================================================================================================
literal geometry outside theme.rs: offset_literals=1668, inset_literals=45, radius_literals=257, font_literals=25, nonconcentric_pairs=2
note: 14 entries are below baseline; run --write-baseline to lock the gain in
PASS: no count rose above the baseline
```

---

## 5. Skills Loaded

- `/Users/jangtrinh/.gemini/config/skills/es-dev/SKILL.md` (`es:dev`)
- `/Users/jangtrinh/.gemini/config/skills/es-feature-dev/SKILL.md` (`es:feature-dev`)
- `/Users/jangtrinh/.gemini/config/skills/es-rust/SKILL.md` (`es:rust`)
- `/Users/jangtrinh/.gemini/config/skills/es-rust-gpu-canvas/SKILL.md` (`es:rust-gpu-canvas`)
- `/Users/jangtrinh/.gemini/config/skills/es-fable-thinking/SKILL.md` (`es:fable-thinking`)
- `/Users/jangtrinh/.gemini/config/skills/es-astra-work/SKILL.md` (`es:astra-work`)

---

## 6. Files Outside Scope

- `crates/designos-fig/src/lib.rs` — Named in the detail spec's `Touches` table (`1140-dtcg-export-bundle-for-design-os-onboarding.md:14`); modified to add `FigError::Export(String)` (`#[error("{0}")]`) and re-export `ColorFormat`, `DtcgModeOptions`, `ModeExportCollection`, `ModeExportReport`, `SkippedModeCollection`, `export_dtcg_mode`, `export_dtcg_with_color_format`, and `export_variables_with_color_format`.
- `crates/designos-app/src/main.rs` — Named in the detail spec's `Touches` table (`1140-dtcg-export-bundle-for-design-os-onboarding.md:16`); modified only in `run_agent_cli`'s `--export-variables` branch and `mod variables_cli_tests` to enforce Rule 10 flag combinations, refuse duplicate export filenames in directory mode, and wire `export_dtcg_mode` / `export_variables_with_color_format`.

---

## 7. Epistemic Ledger & ASSUMPTIONS

- **FACT:** Duplicate-named `VariableCollection`s now receive unique DTCG group names (`_Primitives` for the local collection, `_Primitives (2)`, `_Primitives (3)` in collection id order for library/duplicate copies) and unique `.tokens.json` filenames (`variables_dtcg.rs:947-994`).
- **FACT:** Manifest-backed DTCG round-trip import (`stage_dtcg_files` + `assemble_staged_document`) indexes collections by their manifest `group` key so `{_Primitives (2).…}` aliases resolve to the library collection while preserving `VariableCollection.name == "_Primitives"`.
- **FACT:** `export_dtcg_mode` produces a single self-contained `.tokens.json` file for a named mode, supporting `--collection` (`<name>` with local-copy preference or `id:<collection_id>`), `--resolve-aliases` (recording `$extensions.org.designos.alias_of` and `alias_of_id` and reporting `aliases_resolved` + `aliases_resolved_via_default_mode`), and `--color-format hex|object` (`#RRGGBB` when `ai == 255`, `#RRGGBBAA` when `ai < 255` including `#FFFFFF00` for alpha 0).
- **FACT:** `/opt/homebrew/bin/ui ds import` and `/opt/homebrew/bin/ui ds a11y` succeed on the `dark.tokens.json` emitted from `synthetic-kit-1140.designos` (`115` colour tokens imported, `111` roles recognized, `1307` text×surface contrast pairs evaluated).
- **ASSUMPTION (for controller verification):** Running the same CLI commands on the real Untitled UI PRO VARIABLES v8.0 `.fig` kit (`AC-04` and `AC-05`, reserved for the controller per Ruling #2) will produce the same 13-file bundle and 115-token `dark.tokens.json` onboarding result.

Status: READY FOR CONTROLLER REVIEW


---

## Round 2 — One Value-Lookup Rule (`value_in_mode`), No Dangling Aliases

- **Review Source:** `plans/reports/grok-review-1140-dtcg.md` (Findings 1, 2, and 3)
- **Spec:** `plans/261027-0940-v06x-finish/spec-1140-r2-mode-value-lookup.md`
- **Base Commit:** `3e78d9dc`
- **Red Commit (Test First):** `770e1e59` (`test(fig): add round 2 failing tests and rule-5 helper (#1140)`)
- **Evidence Files:**
  - `plans/reports/agy-1140-dtcg-export-bundle-evidence/red-r2.log` — Red-phase test failure output (`0 passed; 4 failed`)
  - `plans/reports/agy-1140-dtcg-export-bundle-evidence/green-r2.log` — Green-phase test pass output (`4 passed; 0 failed`)
  - `plans/reports/agy-1140-dtcg-export-bundle-evidence/dark.tokens.json` — Re-generated single-mode resolved hex DTCG export
  - `plans/reports/agy-1140-dtcg-export-bundle-evidence/cli-dogfood.log` — Updated synthetic 10-collection dogfood (`ui ds import` + `ui ds a11y`)

### 1. What Could NOT Be Verified (Round 2)

1. **Real Untitled UI PRO VARIABLES v8.0 `.fig` kit execution (`AC-04` and `AC-05`):**
   - Per Controller Ruling #2, the proprietary kit file was never opened or read; verification was run against [`synthetic-kit-1140.designos`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1140-dtcg-export-bundle/plans/reports/agy-1140-dtcg-export-bundle-evidence/synthetic-kit-1140.designos) and unit tests in `crates/designos-fig/src/variables_dtcg.rs`.
2. **Known unrelated UI test flake `#1179` (`shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one`):**
   - Flaked once during the full 3,337-test `cargo test -p designos-app --bin designos --locked` run (`3336 passed; 1 failed`), then passed `3/3` consecutive isolated reruns per the task prompt rule (`only the known flakes #996, #1179 and #1211 may be rerun alone 3 times`).

### 2. Conclusion & Epistemic Ledger (Round 2)

**Conclusion:** Implemented a single `value_in_mode(var, coll, mode_id) -> Option<(&VariableValue, bool)>` lookup rule across the export loop, alias-inlining pass, and alias-resolution walk (`resolve_alias_chain` / `resolve_literal_in_mode`), added `values_from_default_mode` to `ModeExportReport` and the CLI JSON report, deduplicated `skipped_collections` against `selected_modes`, and enforced Detail Spec §3 Rule 5 across all export tests via `assert_all_aliases_resolve_in_dtcg_file` / `assert_all_aliases_resolve_in_bundle`.

- **`FACT`**: [`value_in_mode`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1140-dtcg-export-bundle/crates/designos-fig/src/variables_dtcg.rs#L639-L645) returns `Some((val, false))` when `var.values.get(mode_id)` is `Some`, else `var.values.get(&coll.default_mode).map(|val| (val, true))`, else `None`. Every `.values().next()` fallback was removed from `variables_dtcg.rs`.
- **`FACT`**: In [`export_dtcg_mode`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1140-dtcg-export-bundle/crates/designos-fig/src/variables_dtcg.rs#L943-L948), a token is written iff `value_in_mode(v, coll, mode_id)` is `Some`, incrementing `values_from_default_mode` whenever `used_default` is `true`. Because the export loop and [`resolve_alias_chain`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1140-dtcg-export-bundle/crates/designos-fig/src/variables_dtcg.rs#L662-L691) use the exact same `matching_mode_id` + `value_in_mode` lookup, no exported `{…}` alias can ever point to a variable omitted from the file (Finding 1 fixed).
- **`FACT`**: [`resolve_alias_chain`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1140-dtcg-export-bundle/crates/designos-fig/src/variables_dtcg.rs#L662-L691) counts every hop where `used_default == true` in `default_hops` (added to `aliases_resolved_via_default_mode`), whether the target collection lacked the requested mode or the target variable lacked a value in that collection's matched mode, and refuses (`Err(FigError::Export("unresolvable alias(es): ..."))`) when `value_in_mode` returns `None` (Finding 2 fixed).
- **`FACT`**: In [`export_dtcg_mode`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1140-dtcg-export-bundle/crates/designos-fig/src/variables_dtcg.rs#L803-L828), shadowed collections from name filters are only pushed to `skipped_collections` if `!selected_modes.contains_key(&shadowed.id)`, and selecting any collection removes it from `skipped_collections` via `skipped_collections.retain(|s| s.id != chosen.id)` (Finding 3 fixed).
- **`FACT`**: `assert_all_aliases_resolve_in_dtcg_file` and `assert_all_aliases_resolve_in_bundle` are called on every export in `variables_dtcg::tests`, verifying that every `{…}` reference resolves to a token written in the same output file.
- **`INFERENCE`**: None required for acceptance; all 4 Round 2 scenarios and all 11 Round 1 scenarios are directly covered by deterministic unit tests.
- **`ASSUMPTION`**: None.
- **Weakest link:** Only the controller can run `AC-04` and `AC-05` against the proprietary Untitled UI PRO v8.0 `.fig` kit on disk.

### 3. Skills Loaded & What Changed Because of Them (Round 2)

- `/es:dev` -> `es:feature-dev`, `es:astra-work`, `es:fable-thinking`, `es:rust`, `es:rust-gpu-canvas`:
  - Committed the 4 failing Round 2 unit tests and the Rule-5 test helper first in commit `770e1e59` (`red-r2.log`: `0 passed; 4 failed`) before touching production logic.
  - Replaced three divergent mode-lookup paths (export loop, inlining pass, resolution walk) with a single pure helper `value_in_mode` + `matching_mode_id`.

### 4. Red -> Green Evidence & Gate Outputs (Round 2)

#### 4.1 Red Phase (`plans/reports/agy-1140-dtcg-export-bundle-evidence/red-r2.log` at `770e1e59`)

```text
running 4 tests
test variables_dtcg::tests::mode_export_without_resolve_aliases_writes_default_mode_target_so_no_alias_dangles ... FAILED
test variables_dtcg::tests::resolve_aliases_counts_default_mode_fallback_and_never_uses_non_default_mode ... FAILED
test variables_dtcg::tests::collection_selected_by_both_name_and_id_is_not_in_skipped_collections ... FAILED
test variables_dtcg::tests::variable_with_no_value_in_any_mode_is_refused_when_aliased_and_omitted_when_unreferenced ... FAILED

test result: FAILED. 0 passed; 4 failed; 0 ignored; 0 measured; 130 filtered out; finished in 0.01s
```

#### 4.2 Green Phase (`plans/reports/agy-1140-dtcg-export-bundle-evidence/green-r2.log`)

```text
running 4 tests
test variables_dtcg::tests::mode_export_without_resolve_aliases_writes_default_mode_target_so_no_alias_dangles ... ok
test variables_dtcg::tests::resolve_aliases_counts_default_mode_fallback_and_never_uses_non_default_mode ... ok
test variables_dtcg::tests::collection_selected_by_both_name_and_id_is_not_in_skipped_collections ... ok
test variables_dtcg::tests::variable_with_no_value_in_any_mode_is_refused_when_aliased_and_omitted_when_unreferenced ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 130 filtered out; finished in 0.00s
```

#### 4.3 Gate 1: `cargo fmt --all -- --check`

```text
$ cargo fmt --all -- --check
(exited with code 0, zero output)
```

#### 4.4 Gate 2: `cargo clippy --workspace --all-targets --locked -- -D warnings`

```text
$ cargo clippy --workspace --all-targets --locked -- -D warnings
    Checking designos-fig v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1140-dtcg-export-bundle/crates/designos-fig)
    Checking designos-library v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1140-dtcg-export-bundle/crates/designos-library)
    Checking designos-render v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1140-dtcg-export-bundle/crates/designos-render)
    Checking designos-gpu v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1140-dtcg-export-bundle/crates/designos-gpu)
    Checking designos-eval v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1140-dtcg-export-bundle/crates/designos-eval)
    Checking designos-app v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1140-dtcg-export-bundle/crates/designos-app)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 49.68s
(exited with code 0)
```

#### 4.5 Gate 3: `cargo test -p designos-fig --locked`

```text
$ cargo test -p designos-fig --locked
     Running unittests src/lib.rs (target/debug/deps/designos_fig-8c931ce94e001702)
test result: ok. 134 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 45.28s
     Running tests/fig_Container.rs (target/debug/deps/fig_Container-cb01fa5bb90ed548)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/rest_import.rs (target/debug/deps/rest_import-d674f1f9bb34279c)
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/rest_import_grid_sizing.rs (target/debug/deps/rest_import_grid_sizing-ea51c962e3030958)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/rest_upgrade.rs (target/debug/deps/rest_upgrade-ca48d3ca04d75471)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/settings_import.rs (target/debug/deps/settings_import-8d451f6938f6406b)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/stack_wrap_and_text_wrap.rs (target/debug/deps/stack_wrap_and_text_wrap-7012fa8c00d51e5a)
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
     Running tests/variables.rs (target/debug/deps/variables-04fcdcded0842052)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/variables_code_syntax_opacity.rs (target/debug/deps/variables_code_syntax_opacity-c4eecbabc3d60f28)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/variables_library_collections.rs (target/debug/deps/variables_library_collections-a056a2624f7e82bb)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/vector_network.rs (target/debug/deps/vector_network-1b8b999d4ae5a024)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests designos_fig
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

#### 4.6 Gate 4: `cargo test --workspace --locked --lib`

```text
$ cargo test --workspace --locked --lib
     Running unittests src/lib.rs (target/debug/deps/designos_diff-e9fb9a3e00ecdbf3)
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running unittests src/lib.rs (target/debug/deps/designos_doc-a0c6cf32749620e6)
test result: ok. 633 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.17s
     Running unittests src/lib.rs (target/debug/deps/designos_eval-029cd168a180af6a)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (target/debug/deps/designos_export-f5a81d86319362e2)
test result: ok. 136 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.82s
     Running unittests src/lib.rs (target/debug/deps/designos_fig-648cb6f438830bb1)
test result: ok. 134 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 43.80s
     Running unittests src/lib.rs (target/debug/deps/designos_fonts-8afb0f60e41df5a3)
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.67s
     Running unittests src/lib.rs (target/debug/deps/designos_gpu-49227ce26b5d6686)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s
     Running unittests src/lib.rs (target/debug/deps/designos_library-d39027c82797cc1e)
test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 36.97s
     Running unittests src/lib.rs (target/debug/deps/designos_mcp-189215ed087510e0)
test result: ok. 245 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.66s
     Running unittests src/lib.rs (target/debug/deps/designos_render-39473ba3e638dab1)
test result: ok. 55 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 49.70s
     Running unittests src/lib.rs (target/debug/deps/designos_svg-125a628128d3698e)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (target/debug/deps/designos_webcapture-a59ae055a8fa52f0)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
```

#### 4.7 Gate 5: `cargo test -p designos-app --bin designos --locked`

```text
$ cargo test -p designos-app --bin designos --locked
test variables_cli_tests::dtcg_to_a_json_path_without_mode_is_an_error ... ok
test variables_cli_tests::cli_directory_export_never_overwrites_within_one_run ... ok
...
test result: FAILED. 3336 passed; 1 failed; 11 ignored; 0 measured; 0 filtered out; finished in 713.46s
(1 failure was known flake #1179: shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one)

=== Rerun 1 of #1179 ===
test shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3347 filtered out; finished in 0.69s

=== Rerun 2 of #1179 ===
test shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3347 filtered out; finished in 0.84s

=== Rerun 3 of #1179 ===
test shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3347 filtered out; finished in 0.89s
```

#### 4.8 Gate 6: `python3 scripts/zero_fake_ui_gate.py`

```text
$ python3 scripts/zero_fake_ui_gate.py
============================================================================================================
 design:os studio — Zero Fake UI Enforcement Gate (DIR-28 · BR-31 · REQ-25 · D25)
============================================================================================================
Result: 22/22 invariants PASSED — ZERO FAKE UI GATE GREEN
```

#### 4.9 Synthetic 10-Collection Dogfood Summary (`plans/reports/agy-1140-dtcg-export-bundle-evidence/cli-dogfood.log`)

```text
=== 1. Directory DTCG Export ===
Count on disk: 13

=== 2. ZIP DTCG Export ===
Zip entry count: 13

=== 3. Single-Mode Resolved Hex DTCG Export (dark.tokens.json) ===
{
  "aliases_resolved": 115,
  "aliases_resolved_via_default_mode": 115,
  "collections": [
    {
      "group": "1_ Color modes",
      "id": "VariableCollectionId:1333:27347",
      "mode": "Dark mode",
      "name": "1. Color modes"
    }
  ],
  "color_format": "hex",
  "format": "dtcg",
  "inlined_for_aliases": [],
  "mode": "Dark mode",
  "omitted_extensions": [],
  "path": "plans/reports/agy-1140-dtcg-export-bundle-evidence/dark.tokens.json",
  "skipped_collections": [
    {
      "id": "VariableCollectionId:1333:99999",
      "modes": [
        "Light mode",
        "Dark mode"
      ],
      "name": "1. Color modes"
    }
  ],
  "tokens": 115,
  "values_from_default_mode": 0
}

=== 4. ui ds import & ui ds a11y on dark.tokens.json ===
ds import: 115 token(s) [115 color] across 5 categories → .../ds-scratch/design
  111 roles recognized, 12 gaps: card, popover, secondary, muted, accent, input, ring, destructive, success, warning, info, neutral
ds a11y: 1307 text×surface pair(s) checked (roles inferred from token names — cartesian; prefer -foreground pairing or --pairs), 585 below AA (4.5:1).
```

Status: READY FOR CONTROLLER REVIEW
