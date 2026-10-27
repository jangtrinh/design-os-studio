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

---

## Round 2

### 1. What Could NOT Be Verified First
- **Known `#996` PTY exit timing flake (`shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one`):** Unrelated to `#1212` and explicitly excluded by controller instructions ("Ignore it"). On isolated 3x rerun, rerun 1 passed (`1 passed; 0 failed`) while reruns 2–3 timed out waiting for the child shell exit status under parallel build load.
- **ByteRover `.brv/` curation (`brv curate`):** Skipped because `.brv/` is git-tracked (`git ls-files .brv`) and outside the `Files you may modify` allowlist (`crates/designos-app/src/shell/tests/*`, `README.md`, `.codex-plugin/plugin.json`, `plans/reports/agy-1212-starter-seed.md`).

### 2. Conclusion & Epistemic Ledger
- **Conclusion:** Fixed `the_agents_layers_share_ids_with_the_users_last_step` in [`crates/designos-app/src/shell/tests/live_patch_agent_switch_edges.rs`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1212-starter-seed/crates/designos-app/src/shell/tests/live_patch_agent_switch_edges.rs#L115-L129) so the agent document contains a layer for every `NodeId` touched by the user's last edit rather than assuming all touched IDs fit inside `agent_doc_n(name, 80)`. Updated all 4 places in [`README.md`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1212-starter-seed/README.md) and the prompt in [`.codex-plugin/plugin.json`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-1212-starter-seed/.codex-plugin/plugin.json#L36) to use `your-screen.spec.json` with the phrase `(any NodeSpec JSON you or your agent write)`. All 6 required gates pass on the final commit (3,342 passed in full `designos` bin suite, including `undo_after_the_agent_replaces_a_library_document_in_place_leaves_the_agents_document_alone`).
- **Epistemic Ledger:**
  - `FACT`: `crates/designos-app/assets/starter/designos-studio-starter.designos` has `646` nodes with `min_id = 1`, `max_id = 636187`, and `next_id = 636189`.
  - `FACT`: When `edit(s)` duplicates the first top-level frame (`"01 Onboarding"`) on `library_sim()`, `doc.alloc_id()` allocates `NodeId`s starting at `636189`, whereas `agent_doc_n(name, 80)` only allocated `NodeId(1)..=NodeId(81)`, causing `live_patch_agent_switch_edges.rs:123` to panic deterministically on `585326d2`.
  - `FACT`: Because `next_id` in the starter asset is `636189` (sparse IDs from the source document), calling `agent_doc_n(name, max_touched_id + 1)` would have allocated `> 636,189` rectangles in a unit test; instead, inserting one `Node::rectangle(id, ...)` per `&id` in `touched` into the agent document (`Document::insert` sets `self.next_id = self.next_id.max(id.0)` and inserts `id` directly into `doc.nodes`) keeps the agent document small while guaranteeing `touched.iter().all(|id| doc.get(*id).is_some())`.
  - `FACT`: Both `undo_after_the_agent_replaces_a_saved_document_in_place_leaves_the_agents_document_alone` and `undo_after_the_agent_replaces_a_library_document_in_place_leaves_the_agents_document_alone` pass in `shell::tests::live_patch_agent_switch_edges` (9/9 passed), in two isolated reruns (2/2 passed), and in the full `cargo test -p designos-app --bin designos --locked` run.
  - `INFERENCE`: No other test in `crates/designos-app/src/shell/tests/` has a latent "small seeded file" assumption, based on a full grep audit of `library_sim`, `Welcome`, `agent_doc_n`, `80`, and `nodes.len()` across all test modules plus the full 3,354-test binary run.
  - `ASSUMPTION`: None remaining.
- **Weakest link in one line:** Full `cargo test -p designos-app --bin designos --locked` still exhibits the pre-existing `#996` PTY timing flake in `agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one` under heavy parallel CPU load.
- **Unresolved questions:** None.

### 3. Audit of `live_patch_agent_switch*.rs` and `shell/tests` Helpers for "Small Seeded File" Assumptions
Searched all `crates/designos-app/src/shell/tests/*.rs` for `library_sim`, `agent_doc_n`, `Welcome`, fixed counts (`4`, `8`, `80`), and `nodes.len()`:
1. **`crates/designos-app/src/shell/tests/live_patch_agent_switch_edges.rs`:**
   - `the_agents_layers_share_ids_with_the_users_last_step` (line 117): **Fixed.** Previously called `agent_doc_n(name, 80)` assuming the seeded library file's `next_id` was `< 80`. Now inserts a rectangle for each `&id` in `touched` directly into the agent document.
   - `another_program_rewrites_the_file` (line 36): Uses `agent_doc_n("Written by another program", 7)` (`8` nodes) on `external_sim_with(&two_rects())`, not on `library_sim()`.
   - `a_running_turn_begins_at_the_start_of_the_history_of_a_document_replaced_in_place` & `stop_pending_at_the_switch` (lines 171, 191): Use `agent_doc_n("Agent opened this", 3)` on `external_sim_with(&two_rects())`.
2. **`crates/designos-app/src/shell/tests/live_patch_agent_switch.rs`:**
   - `edit_the_users_document` (line 39): Asserts `4` nodes on screen and `3` on disk, called exclusively on `external_sim_with(&two_rects())`.
   - `a_library_edit_made_just_before_the_agent_switch_is_written_first` (line 120) and `the_agents_document_does_not_overwrite_the_library_file` (line 135): Both use `library_sim()`, record `let edited = s.app().doc.nodes.len();` dynamically after the edit, and assert against `"DESIGN-OS Studio Starter"`.
3. **`crates/designos-app/src/shell/tests/live_patch_agent_switch_guards.rs`:**
   - `text_typed_for_the_users_layer_is_not_written_on_the_agents_layer_with_the_same_id` (line 79): Uses `agent_doc_n("Agent opened this", 8)` on `external_sim_with(&two_rects())` (where `two_rects()` has 3 nodes and `edit` allocates `NodeId(4)`), not on `library_sim()`.
4. **Other `shell/tests` modules using `library_sim()` or `"Welcome"`:**
   - `unsaved_dot.rs`, `tab_disk_state.rs`, `leave_prompt.rs`, `leave_menu_quit.rs`, `palette_autofix_undo.rs`, `home.rs`: Every test using `library_sim()` reads `s.app().doc.nodes.len()` or `State` dynamically rather than asserting a small fixed count.
   - `find.rs`, `lint.rs`, `proto.rs`, `comments.rs`, `components.rs`, `effect_grid_styles.rs`, `style_edit.rs`, `text_styles.rs`, `system_paste_and_drop.rs`, `layer_drag_fast.rs`, `fast_drag_transform.rs`, `dev_mode_shortcut.rs`, `panel_keys.rs`: Reference `"Welcome"` on `Sim::new()` / `Sim::with(Options::default())` (`home: false`), which loads the in-memory `demo` fixture (`#[cfg(any(test, feature = "fixtures"))]`), not the library's seeded starter file.

### 4. Skills Loaded & Changes Made Because of Them
- **`es:dev` → `es:feature-dev`** (`~/.gemini/config/skills/es-dev/SKILL.md`, `~/.gemini/config/skills/es-feature-dev/SKILL.md`): Loaded `.project-agent.md`, `CONTEXT.md`, and queried `.brv/` (`brv query "starter seed library_sim live_patch_agent_switch"` + `.brv/context-tree/session-handoffs/latest.md`) before touching code; executed the DEV phase directly within the approved Round 2 scope without re-entering brainstorm/plan gates.
- **`es:fable-thinking` & `es:astra-work`** (`~/.gemini/config/skills/es-fable-thinking/SKILL.md`, `~/.gemini/config/skills/es-astra-work/SKILL.md`): Applied falsification-first modeling before implementing the suggested `agent_doc_n(name, max_touched_id + 1)` example. Inspecting `designos-studio-starter.designos` revealed `next_id = 636189` (sparse IDs), which would have caused `agent_doc_n(name, max_touched_id + 1)` to allocate `> 636k` nodes in a unit test. Chose the second option ("build it so it contains exactly those ids") by inserting a rectangle for each `&id` in `touched`, keeping the test fast (~2.4s) while preserving the exact invariant.
- **`es:rust` & `es:rust-gpu-canvas`** (`~/.gemini/config/skills/es-rust/SKILL.md`, `~/.gemini/config/skills/es-rust-gpu-canvas/SKILL.md`): Avoided large unnecessary node allocations (`> 636k` nodes) in test fixtures and preserved zero-warning clippy/rustfmt compliance.

### 5. Exact Gate Commands and Pasted Outputs

#### Gate 1: `cargo fmt --all -- --check`
```text
$ cargo fmt --all -- --check
(exited with code 0, 0 diff)
```

#### Gate 2: `cargo clippy --workspace --all-targets --locked -- -D warnings`
```text
$ cargo clippy --workspace --all-targets --locked -- -D warnings
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1212-starter-seed/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1212-starter-seed/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
    Checking designos-diff v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1212-starter-seed/crates/designos-diff)
    Checking designos-doc v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1212-starter-seed/crates/designos-doc)
    Checking designos-fonts v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1212-starter-seed/crates/designos-fonts)
    Checking designos-render v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1212-starter-seed/crates/designos-render)
    Checking designos-svg v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1212-starter-seed/crates/designos-svg)
    Checking designos-export v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1212-starter-seed/crates/designos-export)
    Checking designos-gpu v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1212-starter-seed/crates/designos-gpu)
    Checking designos-webcapture v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1212-starter-seed/crates/designos-webcapture)
    Checking designos-fig v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1212-starter-seed/crates/designos-fig)
    Checking designos-library v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1212-starter-seed/crates/designos-library)
    Checking designos-eval v0.6.3 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1212-starter-seed/crates/designos-eval)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 30s
```

#### Gate 3: Full `cargo test -p designos-app --bin designos --locked` (plus isolated verification & `#996` reruns)
```text
$ cargo test -p designos-app --bin designos --locked shell::tests::live_patch_agent_switch_edges -- --nocapture
running 9 tests
test shell::tests::live_patch_agent_switch_edges::an_agent_switch_waits_for_the_file_changed_on_disk_question_and_use_file_loads_the_file_first ... ok
test shell::tests::live_patch_agent_switch_edges::a_stop_pending_at_the_switch_folds_the_agents_edits_into_one_step_beside_the_users_tab ... ok
test shell::tests::live_patch_agent_switch_edges::undo_after_the_agent_replaces_a_saved_document_in_place_leaves_the_agents_document_alone ... ok
test shell::tests::live_patch_agent_switch_edges::a_running_turn_begins_at_the_start_of_the_history_of_a_document_replaced_in_place ... ok
test shell::tests::live_patch_agent_switch_edges::a_stop_pending_at_the_switch_folds_the_agents_edits_into_one_step_in_place ... ok
test shell::tests::live_patch_agent_switch_edges::users_tab_while_the_agent_runs::the_users_tab_does_not_claim_another_window_and_does_not_retry_the_lock_each_frame ... ok
test shell::tests::live_patch_agent_switch_edges::an_agent_switch_waits_for_the_file_changed_on_disk_question_and_keep_mine_saves_the_users_version ... ok
test shell::tests::live_patch_agent_switch_edges::users_tab_while_the_agent_runs::the_users_tab_asks_for_no_repaint_while_the_agent_runs_in_another_tab ... ok
test shell::tests::live_patch_agent_switch_edges::undo_after_the_agent_replaces_a_library_document_in_place_leaves_the_agents_document_alone ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 3345 filtered out; finished in 5.03s

$ cargo test -p designos-app --bin designos --locked shell::tests::live_patch_agent_switch_edges::undo_after_the_agent_replaces_a_library_document_in_place_leaves_the_agents_document_alone -- --exact --nocapture
running 1 test
test shell::tests::live_patch_agent_switch_edges::undo_after_the_agent_replaces_a_library_document_in_place_leaves_the_agents_document_alone ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3353 filtered out; finished in 2.42s

$ cargo test -p designos-app --bin designos --locked
    Finished `test` profile [unoptimized + debuginfo] target(s) in 41.10s
     Running unittests src/main.rs (target/debug/deps/designos-f103f8deb2404a79)

running 3354 tests
...
test shell::tests::live_patch_agent_switch_edges::undo_after_the_agent_replaces_a_library_document_in_place_leaves_the_agents_document_alone ... ok
...
failures:

---- shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one stdout ----

thread 'shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one' (930328) panicked at crates/designos-app/src/shell/tests/agents_tab_strip.rs:237:5:
assertion `left == right` failed: the exited tab shows a hollow ring: [CircleShape { center: [34.0 1058.0], radius: 2.5, fill: #0D_99_FF_FF, stroke: Stroke { width: 0.0, color: #00_00_00_00 } }, CircleShape { center: [70.5 18.0], radius: 2.7, fill: #FE_59_38_FF, stroke: Stroke { width: 0.0, color: #00_00_00_00 } }, CircleShape { center: [78.0 18.0], radius: 2.3, fill: #D9_77_57_FF, stroke: Stroke { width: 0.0, color: #00_00_00_00 } }, CircleShape { center: [156.5 18.0], radius: 2.7, fill: #AA_AA_AB_B2, stroke: Stroke { width: 0.0, color: #00_00_00_00 } }, CircleShape { center: [423.0 1056.0], radius: 14.0, fill: #27_27_2A_FF, stroke: Stroke { width: 0.0, color: #00_00_00_00 } }, CircleShape { center: [1380.0 20.0], radius: 10.0, fill: #F5_9E_0B_FF, stroke: Stroke { width: 0.0, color: #00_00_00_00 } }, CircleShape { center: [1395.0 430.0], radius: 8.0, fill: #FF_B7_03_FF, stroke: Stroke { width: 0.0, color: #00_00_00_00 } }, CircleShape { center: [1395.0 430.0], radius: 7.5, fill: #00_00_00_00, stroke: Stroke { width: 1.0, color: #16_16_16_16 } }, CircleShape { center: [1395.0 486.0], radius: 8.0, fill: #0C_8C_E9_FF, stroke: Stroke { width: 0.0, color: #00_00_00_00 } }, CircleShape { center: [1395.0 486.0], radius: 7.5, fill: #00_00_00_00, stroke: Stroke { width: 1.0, color: #16_16_16_16 } }, CircleShape { center: [1395.0 542.0], radius: 8.0, fill: #1E_1E_1E_FF, stroke: Stroke { width: 0.0, color: #00_00_00_00 } }, CircleShape { center: [1395.0 542.0], radius: 7.5, fill: #00_00_00_00, stroke: Stroke { width: 1.0, color: #16_16_16_16 } }, CircleShape { center: [1580.0 1070.0], radius: 14.0, fill: #2C_2C_2C_FF, stroke: Stroke { width: 0.0, color: #00_00_00_00 } }, CircleShape { center: [1580.0 1070.0], radius: 14.0, fill: #00_00_00_00, stroke: Stroke { width: 1.0, color: #38_38_38_FF } }]
  left: 0
 right: 1


failures:
    shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one

test result: FAILED. 3342 passed; 1 failed; 11 ignored; 0 measured; 0 filtered out; finished in 1578.29s

$ cargo test -p designos-app --bin designos --locked shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one -- --exact
running 1 test
test shell::tests::agents_tab_strip::an_exited_tab_has_its_own_status_dot_and_an_idle_tab_keeps_the_filled_one ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 3353 filtered out; finished in 7.67s
```

#### Gate 4: `cargo test -p designos-library --locked`
```text
$ cargo test -p designos-library --locked
     Running unittests src/lib.rs (target/debug/deps/designos_library-c45433f1b88c4af9)

running 37 tests
test store::tests::default_root_honours_the_env_override ... ok
test publish::tests::a_published_version_with_non_finite_numbers_still_loads ... ok
test publish::tests::unpublish_is_soft_delete ... ok
test publish::tests::publish_list_load_and_changelog ... ok
test publish::tests::round_trip_between_two_documents_preserves_overrides ... ok
test store::tests::folders_nest_rename_and_refuse_cycles ... ok
test store::tests::sanitize_in_memory_touches_only_affected_entries ... ok
test branches::tests::create_saves_branch_point_and_same_ids ... ok
test store::tests::first_run_seeds_once ... ok
test store::tests::times_are_strictly_increasing ... ok
test store::tests::project_crud_and_trash ... ok
test store::tests::atomic_save_leaves_no_temp_file_and_survives_a_stale_one ... ok
test versions::tests::a_damaged_index_is_rebuilt_from_the_snapshots ... ok
test branches::tests::branch_versions_separate ... ok
test branches::tests::edge_cases_name_taken_trash_and_duplicate ... ok
test versions::tests::named_versions_are_always_kept ... ok
test versions::tests::delete_moves_to_trash_and_numbers_are_not_reused ... ok
test versions::tests::retention_keeps_everything_in_the_last_hour ... ok
test versions::tests::retention_thins_to_one_per_day_for_a_month_and_drops_older ... ok
test versions::tests::retention_thins_to_one_per_hour_for_a_day ... ok
test versions::tests::index_has_the_documented_fields ... ok
test branches::tests::legacy_index_loads ... ok
test versions::tests::rename_names_a_version_and_keeps_it ... ok
test store::tests::trash_empty_removes_the_session_folder ... ok
test versions::tests::saves_lists_and_loads_versions ... ok
test versions::tests::saving_an_auto_version_prunes_old_ones_on_disk ... ok
test store::tests::import_native_json_and_svg ... ok
test store::tests::restoring_a_file_of_a_trashed_project_asks_for_the_project_first ... ok
test branches::tests::failure_leaves_manifest_unchanged ... ok
test store::tests::rollback_unlisted_file_keeps_stored_file_when_manifest_already_lists_it ... ok
test store::tests::thumbnails_are_stored_and_follow_moves ... ok
test branches::tests::archived_is_read_only ... ok
test store::tests::search_and_recents_order ... ok
test store::tests::trash_restore_and_empty ... ok
test branches::tests::versions_before_after_and_one_undo ... ok
test store::tests::file_crud_roundtrip ... ok
test branches::tests::branch_archived_after_merge ... ok

test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.01s

     Running tests/atomic_save.rs (target/debug/deps/atomic_save-31305ac7e1ba52c3)

running 16 tests
test directory_target_is_refused ... ok
test a_named_pipe_target_is_refused_without_blocking ... ok
test file_with_0644_mode_and_a_write_deny_acl_is_refused ... ok
test a_symlink_to_a_named_pipe_is_refused_without_blocking ... ok
test read_only_target_is_refused_and_left_untouched ... ok
test saving_through_a_symlink_updates_the_real_file_and_keeps_the_link ... ok
test saving_through_a_dangling_symlink_creates_the_target_and_keeps_the_link ... ok
test a_user_file_named_like_a_digit_pair_temp_is_never_swept ... ok
test publish_and_font_cache_use_the_shared_hardened_write_atomic ... ok
test missing_file_and_parent_directories_are_created ... ok
test saving_keeps_the_existing_permission_mode ... ok
test saving_through_a_symlink_into_another_directory_writes_next_to_the_real_file ... ok
test crashed_save_leftover_tmp_files_are_swept_on_next_save ... ok
test filename_of_249_or_more_bytes_succeeds_without_enametoolong ... ok
test forty_concurrent_saves_of_one_file_all_succeed_and_none_is_torn ... ok
test temp_file_is_never_world_readable_while_saving_a_0600_document ... ok

test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.64s

     Running tests/non_finite_floats.rs (target/debug/deps/non_finite_floats-c3d41530ea67fcad)

running 6 tests
test a_nan_written_by_the_library_can_be_reopened ... ok
test saving_an_edited_document_with_a_nan_keeps_the_file_openable ... ok
test a_file_saved_by_an_older_version_with_null_numbers_opens ... ok
test failed_import_rolls_back_stored_file_and_retry_imports_only_one_copy ... ok
test a_rest_import_with_overflowing_numbers_opens_and_matches_the_stored_file ... ok
test a_counted_save_reports_the_replaced_numbers_from_the_one_serialization_it_performs ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.30s

   Doc-tests designos_library

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

#### Gate 5: `python3 scripts/zero_fake_ui_gate.py`
```text
$ python3 scripts/zero_fake_ui_gate.py
============================================================================================================
 design:os studio — Zero Fake UI Enforcement Gate (DIR-28 · BR-31 · REQ-25 · D25)
============================================================================================================
ID    | STATUS | MODULE                           | INVARIANT & EVIDENCE
------------------------------------------------------------------------------------------------------------
Z01   | PASS   | terminal.rs + agent_composer.rs  | Live PTY send_to_agent in submit_chat_prompt; zero one-shot (-p / exec) calls — `runtime_binary_available` + `submit_chat_prompt` -> `send_to_agent` writes bracketed paste into the live PTY; zero one-shot (`-p` / `exec`) invocations in shipped code
Z02   | PASS   | terminal.rs                      | set_active_runtime mutates tab.preset; DESIGNOS_AGENT_RUNTIME set at spawn; one-shot chat symbols removed — `set_active_runtime` mutates `tab.preset = runtime` without PTY writes; `DESIGNOS_AGENT_RUNTIME` set at spawn; retired one-shot chat symbols absent from shipped code
Z03   | PASS   | terminal.rs                      | dispatch_figma_ai_prompt sends executable command without # comment prefix — `dispatch_figma_ai_prompt` dispatches through `send_to_agent` without `#` comment prefix
Z04   | PASS   | terminal.rs + agent_composer.rs  | All 3 Agents header & composer buttons have Sense::click() handlers; fake mic removed — All 3 header/composer controls (agents-header-ai, agents-composer-attach, agents-composer-context) have interactive Sense::click() handlers; fake mic removed
Z05   | PASS   | specs.rs                         | specs-ask-agent-bar hosts editable TextEdit::singleline(ask_input) — `specs-ask-agent-bar` hosts editable `TextEdit::singleline(&mut ...ask_input)`
Z06   | PASS   | specs.rs                         | Right Inspector dynamic active_section_toc & interactive SpecComment — Right Spec Inspector has dynamic `active_section_toc` + interactive `SpecComment` (`comment_draft` + `TextEdit::singleline`)
Z07   | PASS   | specs.rs                         | Unwrapped draw_wrapped_prose & render_tab_blueprints renders pkg.ui_contract — `draw_wrapped_prose` does not truncate at `lines.len() >= 3` and `render_tab_blueprints` renders `pkg.ui_contract`
Z08   | PASS   | panels/toolbar.rs + cmdk.rs      | Contextual sub-toolbar buttons mutate app.doc / app.tool; ctx-crop / ctx-remove-bg / cmdk Remove background stay removed — `panels/toolbar.rs` contextual buttons (`ctx-select-area`, `ctx-edit-prompt`) mutate `app.doc` / `app.tool`; no ctx-crop / ctx-remove-bg / Remove background in any shipped source
Z09   | PASS   | home/mod.rs                      | Act::ExportFig writes .fig to disk & Nav::AllFolders/Templates/Libraries filter distinctly — `home/mod.rs` `Act::ExportFig` writes `.fig` bytes through `fig_export::export_new` and `io::write_new_in` (`create_new` + `write_all`) and `AllFolders`/`Templates`/`Libraries` filter distinctly
Z10   | PASS   | panels/right.rs                  | Share copies to clipboard & Dev Mode 2nd tab renders Dev Plugins inspector — `panels/right.rs` `Share` copies to OS clipboard (`copy_text`) and Dev Mode 2nd tab renders Dev Plugins inspector
Z11   | PASS   | panels/inspect.rs + properties.rs | Dev Mode Code Connect/MCP/Redlines/Keyframes & Copy to Slides mutate state/clipboard — `panels/inspect.rs` (`dev-code-connect`, `dev-mcp-btn`, `dev-add-redlines`, `inspect_keyframes_section`) and `panels/properties.rs` (`copy-to-slides`) execute real clipboard/document mutations
Z12   | PASS   | widgets/picker.rs + panels/mod.rs + variables/view.rs | Mode::Image creates Paint::Image/doc.images & scrollbars support Sense::click_and_drag() — `widgets/picker.rs` `Mode::Image` creates real `Paint::Image` / `doc.images` entries and `panels::scrollbar` + `variables::view::hscrollbar` support `Sense::click_and_drag()`
Z13   | PASS   | panels/menus.rs                  | Zoom menu rows are the allow-listed set on real Actions; no hard-coded checked state — Zoom menu = the 11 allow-listed rows, each on its real `Action`; every `checked:` is `false` or read from `app.` state
Z14   | PASS   | updater.rs + tabs.rs             | Sample-manifest updater UI stays hidden (guard is code in tabs, sidebar card, modal) until a real manifest fetch exists — updater UI hidden=True, sample manifest used=True; guard is code in tabs.rs button, sidebar card and update modal
Z15   | PASS   | terminal.rs + cmdk.rs            | No fake template synthesis or canned Executed via transcript in Agents panel; synthesize_spec_from_prompt stays private to cmdk.rs — `terminal.rs` never calls `synthesize_spec_from_prompt` or prints `Executed via` / `PRD Context:`; `synthesize_spec_from_prompt` is private to `cmdk.rs`
Z16   | PASS   | registered label helpers in designos-app/src | UX writing: action labels at registered button/menu helpers and label fns <=3 words, no count in them (ratchet vs ux_writing_baseline.json) — 18 labelled violation(s) all in scripts/ux_writing_baseline.json; none new (design/ux-writing.md rules 1 and 3)
Z17   | PASS   | terminal.rs + mcp/server.rs      | No global /tmp/designos-live-* or designos-live-selection.spec.json fallback in shipped code — no shipped (non-test) Rust source under designos-app or designos-mcp contains a global `/tmp/designos-live-` path or `designos-live-selection.spec.json`
------------------------------------------------------------------------------------------------------------
 Gate Verdict: PASS (17/17 invariants satisfied)
============================================================================================================
```

#### Gate 6: `python3 -m unittest scripts/tests/test_shipped_resources.py`
```text
$ python3 -m unittest scripts/tests/test_shipped_resources.py
...
----------------------------------------------------------------------
Ran 3 tests in 2.839s

OK
```

