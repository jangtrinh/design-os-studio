# Report — DEV · #1212 (v0.6.5): Replace Legacy Bundled Samples with `"DESIGN-OS Studio Starter"`

- **Issue:** `#1212` (P1, v0.6.5 batch)
- **Branch:** `jangtrinh/agy-1212-starter-seed` (based on `origin/release/v0.6.4` at `6f0c3bb2`)
- **Evidence directory:** [`plans/reports/agy-1212-starter-seed-evidence/`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1212-starter-seed/plans/reports/agy-1212-starter-seed-evidence/)

---

## 1. Executive Summary

Replaced all legacy bundled sample documents (`demo_document()`, the 6 legacy Home template cards, `--seed-library` in release builds, and the tracked `examples/` directory) with a single offline-bundled starter file (`crates/designos-app/assets/starter/designos-studio-starter.designos`, loaded via `include_bytes!`). Fresh libraries seed a `"Getting started"` project containing `"DESIGN-OS Studio Starter"`, while existing `v0.4.0–v0.6.4` libraries (identified by `.seeded` without `.starter-seeded`) receive `"DESIGN-OS Studio Starter"` once without modifying or overwriting any user files. Ship-only `--fixture` documents are gated behind `cfg(any(test, feature = "fixtures"))`, and CI resource/attribution/examples checks run automatically in `scripts/tests/test_shipped_resources.py`.

---

## 2. Committed Starter Asset (`msg_2a6b005259a8`)

- **Asset Path:** [`crates/designos-app/assets/starter/designos-studio-starter.designos`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1212-starter-seed/crates/designos-app/assets/starter/designos-studio-starter.designos)
- **Commit:** `7706427e` (`feat(app): ship the DESIGN:OS Studio starter file`)
- **Size:** `1,860,182` bytes (`1.77 MB`, ≤ `5 MB` limit enforced by `starter_asset_exists_and_is_at_most_5_mb` and `scripts/tests/test_shipped_resources.py`)
- **SHA-256:** `753a6d48a5a6478e09e2826e83777dd2daec176055e06bd65c7900bfc99e0e0b`
- **Structure:**
  - **Pages (2):**
    1. `"Starter screens"` — 3 top-level frames: `"01 Onboarding"`, `"02 Bio Age"`, `"03 Console - All studies"`
    2. `"About this file"` — 1 top-level frame: `"About"`
  - **Total scene nodes:** `646`

---

## 3. Seed Rules & Pre-Seed Library Behavior (Ruling 3)

Implemented in [`crates/designos-library/src/store.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1212-starter-seed/crates/designos-library/src/store.rs) and [`crates/designos-app/src/home/session.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1212-starter-seed/crates/designos-app/src/home/session.rs):

1. **New install (`!has_seeded && list_projects().is_empty()`):**
   - `Library::needs_first_run_seed()` returns `true`.
   - Creates project `"Getting started"` and file `"DESIGN-OS Studio Starter"` (with pre-rendered thumbnail), then writes both `.seeded` and `.starter-seeded`.
2. **Upgrade from v0.4.0–v0.6.4 (`has_seeded && !has_starter_seeded`):**
   - `Library::needs_starter_upgrade_seed()` returns `true`.
   - Reuses existing `"Getting started"` project (or creates `"Getting started"` if the user deleted it), seeds `"DESIGN-OS Studio Starter"` (or `"DESIGN-OS Studio Starter (n)"` via `next_starter_name_in_project` if a file with that name already exists in the project — never overwriting user files), and writes `.starter-seeded`.
   - Existing user files (`Welcome`, `Marlow — Mobile Banking Kit`, etc.) are never deleted, renamed, or reseeded.
   - "Already present" is determined solely by the `.starter-seeded` marker file; user files are never scanned.
3. **Pre-seed library (`!has_seeded && !list_projects().is_empty()`):**
   - Follows the existing pre-seed rule: `needs_first_run_seed()` is `false` (because `list_projects()` is non-empty) and `needs_starter_upgrade_seed()` is `false` (because `.seeded` is absent). Neither seed runs, leaving pre-seed libraries untouched.
4. **Corrupt or missing starter asset:**
   - Displays an error toast reading `"Could not create the starter sample"`.
   - Keeps the library open and usable.
   - Writes neither `.seeded` nor `.starter-seeded`, so the next launch retries cleanly.

---

## 4. Removal & Feature-Gating Summary (R1–R3 & `examples/`)

- **R1 (`demo_document()` removed from release builds):**
  - `demo_document()` in [`crates/designos-app/src/app.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1212-starter-seed/crates/designos-app/src/app.rs) is gated with `#[cfg(any(test, feature = "fixtures"))]`.
  - Launching with no file or flags opens Home with a blank `"Untitled"` document (`"Page 1"` only) underneath and never constructs `demo_document()`.
- **R2 (Legacy Home templates removed):**
  - Removed the 6 legacy template cards (`Launch Brainstorm`, `Product Exploration`, `Design Token Sheet`, `Marlow — Mobile Banking Kit`, `Product UI Kit`, `Welcome`) and `Act::CreateFromTemplate` / `TemplateKind`.
  - Preserved `Blank design`, `Blank board`, `Blank deck`, and added `Open starter sample` in Home (`Templates & Specs` / `Resources & Libraries`) and `Help and account ▸ Open starter sample`.
- **R3 (`--fixture` gated behind `cfg(any(test, feature = "fixtures"))`):**
  - In release/default builds without `--features fixtures`, `--fixture <name>` returns `unknown fixture: <name>` and exits `≠ 0`, and `--seed-library` is rejected as an unexpected argument (`≠ 0`).
  - With `--features fixtures` (or inside `#[cfg(test)]`), all fixture builders (`showcase`, `board`, `slides`, `tokens`, `marlow`, `ui-kit`, `welcome`, `comments`, `proto-vars`) and `--seed-library` continue to work unchanged.
- **`examples/` deleted:**
  - Removed `examples/README.md`, `examples/designos-studio-landing.designos`, and `examples/marlow-banking.designos`.
  - Migrated unit/integration tests that referenced `examples/designos-studio-landing.designos` to `crates/designos-app/tests/fixtures/designos-studio-landing.designos`.
  - Updated `README.md`, `docs/user-guide.md`, `docs/architecture.md`, `docs/mcp-guide.md`, and `scripts/render-readme-shots.sh`.

---

## 5. CI Workflow Audit (`.github/workflows/*`)

Audited `.github/workflows/ci.yml` and `.github/workflows/release.yml`:
- **No workflow edits required.** Neither `ci.yml` nor `release.yml` runs `--fixture` or `--seed-library`.
- `scripts/tests/test_shipped_resources.py` is automatically discovered and executed by the existing **Design gates** job in `.github/workflows/ci.yml`:
  ```yaml
  python3 -B -m unittest discover -s scripts/tests -p "test_*.py"
  ```
- **Note for local helper scripts:** Local developer scripts that invoke `--fixture` on a compiled binary (`scripts/measure_ui.py`, `scripts/record-launch-marketing.py`, `scripts/capture-48-real-micro-steps.py`) must build `designos-app` with `--features fixtures` (e.g. `cargo build --release -p designos-app --features fixtures`).

---

## 6. Acceptance Criteria & Test Verification

| Requirement | Test / Verification | Status |
|---|---|---|
| **Asset ≤ 5 MB, 2 pages, 3 frames, `About this file`** | `shell::tests::home::starter_asset_exists_and_is_at_most_5_mb` + `test_shipped_resources.py` | **PASS** |
| **AC-01:** Fresh library seeds `"Getting started"` + `"DESIGN-OS Studio Starter"` | `shell::tests::home::ac01_fresh_library_seeds_getting_started_and_starter_file` | **PASS** |
| **AC-02:** No-arg launch opens Home and never builds `demo_document()` | `shell::tests::home::ac02_no_arg_launch_opens_home_and_never_builtin_demo` | **PASS** |
| **AC-03:** Release binary rejects `--fixture showcase` and `--seed-library`, and has no `Marlow — Mobile Banking Kit` / `Launch Brainstorm` / `Design Token Sheet` strings | `crates/designos-app/tests/cli_version.rs` (`ac03_*`) + `scripts/tests/test_shipped_resources.py` | **PASS** |
| **AC-04:** Home views paint none of the legacy sample names | `shell::tests::home::ac04_home_screen_contains_none_of_the_legacy_names` | **PASS** |
| **AC-05:** Upgrade from v0.4.0–v0.6.4 seeds starter once, preserves existing files, suffixes ` (1)` on clash, and never reseeds after deletion | `shell::tests::home::ac05_upgrade_from_v062_library_seeds_starter_once_and_handles_name_clash` | **PASS** |
| **AC-06:** Missing recent file shows `"File not found"` + `"Remove"` and removes cleanly | `shell::tests::home::ac06_missing_recent_file_shows_file_not_found_and_remove` | **PASS** |
| **AC-10:** `Help and account ▸ Open starter sample` creates `"DESIGN-OS Studio Starter (n)"` in current project | `shell::tests::home::ac10_help_open_starter_sample_creates_numbered_copies_in_current_project` | **PASS** |
| **AC-12:** Shipped resources & decoded starter asset contain no `untitled ?ui\|untitledui` | `scripts/tests/test_shipped_resources.py::test_ac12_no_untitledui_in_shipped_resources_or_starter_asset` | **PASS** |
| **Corrupt asset recovery:** Toasts `"Could not create the starter sample"`, keeps library usable, writes no marker | `shell::tests::home::corrupt_starter_asset_toasts_and_leaves_library_usable_without_marker` | **PASS** |
| **`examples/` removed:** Repo `examples/` directory stays deleted and unreferenced | `scripts/tests/test_shipped_resources.py::test_examples_directory_ stays_removed` | **PASS** |

### Test Execution Output (on Real Starter Asset `7706427e`)

Full Red and Green logs are stored in:
- [`plans/reports/agy-1212-starter-seed-evidence/red.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1212-starter-seed/plans/reports/agy-1212-starter-seed-evidence/red.log)
- [`plans/reports/agy-1212-starter-seed-evidence/green.log`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1212-starter-seed/plans/reports/agy-1212-starter-seed-evidence/green.log)

```text
$ python3 -B -m unittest -v scripts/tests/test_shipped_resources.py
test_ac03_starter_asset_exists_and_within_5mb (scripts.tests.test_shipped_resources.TestShippedResources.test_ac03_starter_asset_exists_and_within_5mb) ... ok
test_ac12_no_untitledui_in_shipped_resources_or_starter_asset (scripts.tests.test_shipped_resources.TestShippedResources.test_ac12_no_untitledui_in_shipped_resources_or_starter_asset) ... ok
test_examples_directory_stays_removed (scripts.tests.test_shipped_resources.TestShippedResources.test_examples_directory_stays_removed) ... ok
----------------------------------------------------------------------
Ran 3 tests in 0.032s
OK

$ cargo test -p designos-app --test cli_version --locked
running 8 tests
test ac03_default_binary_contains_no_removed_sample_titles ... ok
test ac03_default_build_rejects_fixture_showcase_and_seed_library ... ok
test help_flag_exits_0_and_prints_usage ... ok
test open_id_unknown_exits_2_with_no_library_file_with_id ... ok
test select_unknown_exits_2_with_no_node_matching ... ok
test unknown_flag_exits_2_with_unexpected_argument ... ok
test version_long_exits_0_and_prints_name_and_semver ... ok
test version_short_matches_version_long ... ok
test result: ok. 8 passed; 0 failed

$ cargo test -p designos-app --bin designos --locked shell::tests::home -- --test-threads=1
running 36 tests
...
test shell::tests::home::ac01_fresh_library_seeds_getting_started_and_starter_file ... ok
test shell::tests::home::ac02_no_arg_launch_opens_home_and_never_builtin_demo ... ok
test shell::tests::home::ac04_home_screen_contains_none_of_the_legacy_names ... ok
test shell::tests::home::ac05_upgrade_from_v062_library_seeds_starter_once_and_handles_name_clash ... ok
test shell::tests::home::ac06_missing_recent_file_shows_file_not_found_and_remove ... ok
test shell::tests::home::ac10_help_open_starter_sample_creates_numbered_copies_in_current_project ... ok
test shell::tests::home::corrupt_starter_asset_toasts_and_leaves_library_usable_without_marker ... ok
test shell::tests::home::starter_asset_exists_and_is_at_most_5_mb ... ok
...
test result: ok. 36 passed; 0 failed
```

All workspace gates also passed:
- `cargo fmt --all -- --check` (0 diff)
- `cargo clippy --workspace --all-targets --locked -- -D warnings` (0 warnings)
- `cargo test --workspace --locked --lib` (all passed)
- `python3 -B -m unittest discover -s scripts/tests -p "test_*.py"` (40/40 passed)
- `python3 scripts/zero_fake_ui_gate.py` (17/17 PASS)
- `python3 scripts/check_spacing_relationships.py` (PASS)

---

## 7. Visual Evidence (`1440x900`, `--no-toast`)

| Capture | Path | Description |
|---|---|---|
| **Before (v0.6.4 Home)** | [`before-home.png`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1212-starter-seed/plans/reports/agy-1212-starter-seed-evidence/before-home.png) | Legacy `Welcome` card seeded in `Getting started` |
| **After (Fresh first-run Home)** | [`after-home-fresh.png`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1212-starter-seed/plans/reports/agy-1212-starter-seed-evidence/after-home-fresh.png) | Fresh library showing `DESIGN-OS Studio Starter` card (`In Getting started`) with real starter thumbnail |
| **After (Upgrade Home)** | [`after-home-upgrade.png`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1212-starter-seed/plans/reports/agy-1212-starter-seed-evidence/after-home-upgrade.png) | Upgraded v0.6.4 library showing newly seeded `DESIGN-OS Studio Starter` alongside preserved user files (`Marlow — Mobile Banking Kit`, `Welcome`) |
| **After (Starter Canvas)** | [`after-starter-canvas.png`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1212-starter-seed/plans/reports/agy-1212-starter-seed-evidence/after-starter-canvas.png) | Real `DESIGN-OS Studio Starter` opened on canvas (`Starter screens` page with `01 Onboarding`, `02 Bio Age`, `03 Console - All studies`, plus `About this file` page) |
| **After (Help Menu)** | [`after-help-menu.png`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1212-starter-seed/plans/reports/agy-1212-starter-seed-evidence/after-help-menu.png) | `Main menu ▸ Help and account ▸ Open starter sample` |
