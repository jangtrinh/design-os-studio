# #166 Gate dependencies on licence with cargo-deny

| Field | Value |
|---|---|
| Issue | #166 (task X3, epic #19; milestone v0.7 Effects & Export, the issue body says "Pack: v0.7 Agent design context") |
| Size | S |
| Depends on | none |
| Unblocks | #254 (plugin runtime spike, lists #166 as a dependency); every later task that adds a crate (the audio, video and plugin-runtime epics name this gate) |
| Touches | new `deny.toml` (repository root), `.github/workflows/ci.yml` (new `licences` job), new `scripts/check_licence_gate.sh`, new `scripts/licence-gate-fixtures/` (four tiny crates), `CONTRIBUTING.md` (one line, `cargo deny check licenses`, in the command list at `:26-29`) |

Code baseline: `origin/main@0e8a9a5` (`docs(specs): v0.6 task specs, part 4 (agent terminal UX and constraint solver) (#697)`). v0.7 opens after v0.6 is tagged. The worker re-anchors every cited line against HEAD before writing code and reports drift first.

## 1. Summary and user story

As a maintainer who ships this app under MIT OR Apache-2.0, I want CI to fail when a dependency brings in a licence we cannot ship, so a copyleft crate cannot enter through a routine `cargo add`. One policy file lists what is allowed; every exception is named and explained.

## 2. Current state (FACT)

1. **No policy exists.** There is no `deny.toml` in the repository (checked on this commit). CI has three jobs: `Test (<os>)` on ubuntu, windows and macos-14 (`.github/workflows/ci.yml:17-52`), `Clippy` on ubuntu (`:54-76`) and `Rustfmt` on ubuntu (`:78-92`). `design-gates.yml` is an advisory job that cannot fail the run (`continue-on-error: true`, its header comment) and is the precedent for a non-required check.
2. **Our own licence.** `[workspace.package] license = "MIT OR Apache-2.0"` (root `Cargo.toml:20`); all 11 workspace crates inherit it (`cargo metadata --locked`, this commit); `LICENSE-MIT` and `LICENSE-APACHE` exist at the root.
3. **The tree today** (`cargo metadata --format-version 1 --locked` on this commit): 580 packages, 11 workspace members, 569 external crates, 33 distinct SPDX expressions across all targets; 381 packages are in the `aarch64-apple-darwin` graph. The expressions that are not plain MIT, Apache-2.0 or an OR of them:

   | Licence | Crates (this commit) | In the macOS graph |
   |---|---|---|
   | `MPL-2.0` | `option-ext 0.2.0`, required by `dirs-sys 0.4.1` <- `dirs 5.0.1` <- `designos-fonts`, `designos-library` | yes |
   | `(MIT OR Apache-2.0) AND OFL-1.1 AND Ubuntu-font-1.0` | `epaint_default_fonts 0.35.0` (fonts bundled by egui's `epaint 0.35.0`) | yes |
   | `Unicode-3.0` | 18 crates (`icu_*`, `litemap`, ...) and `unicode-ident 1.0.26` (`(MIT OR Apache-2.0) AND Unicode-3.0`) | yes |
   | `CDLA-Permissive-2.0` | `webpki-roots 0.26.11` and `1.0.9`, required by `ureq 2.12.1` | yes |
   | `CC0-1.0` | `hexf-parse 0.2.1` | yes |
   | `BSL-1.0` | `clipboard-win 5.4.1`, `error-code 3.4.0` (Windows only, via `arboard 3.6.1`) | no |
   | `Apache-2.0 OR GPL-2.0-only` | `self_cell 1.3.0` (via `cosmic-text`, `epaint`) | yes |
   | `MIT OR Apache-2.0 OR LGPL-2.1-or-later` | `r-efi 5.3.0`, `6.0.0` (UEFI only, via `getrandom`) | no |
   | `BSD-2-Clause`, `BSD-3-Clause`, `ISC`, `Zlib`, `0BSD OR ...`, `Unlicense OR MIT` | various | yes |

4. **The issue's literal allow list fails today.** With `allow = [MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Zlib]`, `cargo-deny 0.20.2 check licenses` on this commit prints `licenses FAILED` with 26 `rejected` diagnostics. So acceptance criterion 1 ("the current tree passes") needs more than the issue's list: `Unicode-3.0`, `CDLA-Permissive-2.0`, `CC0-1.0`, `BSL-1.0`, and exceptions for the MPL-2.0 and font-licence crates.
5. **A policy that passes today** (run locally, `cargo-deny 0.20.2`, default features and again with `all-features = true`, no warnings, so every allowance is used): `allow = [MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Zlib, Unicode-3.0, CC0-1.0, BSL-1.0, CDLA-Permissive-2.0]` plus `exceptions` for `option-ext` (`MPL-2.0`) and `epaint_default_fonts` (`OFL-1.1`, `Ubuntu-font-1.0`) printed `licenses ok`. `Cargo.lock` was not changed by the run.
6. **The ruling and the tree disagree.** R4.16 (issue): "MPL-2.0 allowed as an explicit exception" and acceptance criterion 3 names symphonia as the only MPL-2.0 case. The tree already holds one MPL-2.0 crate (`option-ext`), so the exception list must name `option-ext` today. Symphonia is not in the tree (audio is planned later); it is more than one crate (UNKNOWN: which `symphonia-*` crates are MPL-2.0; check: `cargo metadata` after the audio task adds them).
7. **Config keys** (cargo-deny documentation, read 2026-10-19): `[licenses]` takes `allow`, `exceptions` (`{ crate, allow }`), `clarify`, `private`, `confidence-threshold`, `unused-allowed-license` and `unused-license-exception` (both default to a warning); the old `unlicensed`, `deny`, `copyleft`, `default` and `allow-osi-fsf-free` keys are removed and now error. Any licence not listed is denied.
8. **Running it on macOS.** The `EmbarkStudios/cargo-deny-action` is a container action and its examples all use ubuntu runners (UNKNOWN for macOS; its page does not say). `taiki-e/install-action` says it is tested on Ubuntu, macOS and Windows runners and installs a pinned tool with `tool: cargo-deny@<version>`; whether cargo-deny is in its prebuilt list or falls back to `cargo-binstall` is not stated on the page (UNKNOWN; check: the first CI run's log).
9. `rust-toolchain.toml` pins `1.93.1` (`channel`, profile minimal) and `ci.yml:13` sets `RUST_VERSION: "1.93.1"`. `cargo deny` calls `cargo metadata`, so the job must install the pinned toolchain the way the other jobs do, or `rustup` installs it on the first call.
10. `docs/releasing.md:21` lists the hot files; this task touches none of them.

## 3. Behaviour rules

1. **Policy file.** `deny.toml` holds only a `[graph]` table (`all-features = true`, so a non-default feature cannot hide a licence) and a `[licenses]` table: the ten `allow` entries of fact 5, the two `exceptions` of fact 5, `unused-allowed-license` and `unused-license-exception` left at their default warning. A comment above each exception states the crate, why it is accepted and who ruled it (R4.16 for MPL-2.0; the font licences are an owner question, open question 2).
2. **Scope.** The job runs `cargo deny check licenses` only. Advisories, bans and sources are not checked (non-goals).
3. **CI job.** A job named `licences` in `ci.yml`: `runs-on: macos-14`; `actions/checkout@v4`; `dtolnay/rust-toolchain@master` with `toolchain: ${{ env.RUST_VERSION }}`; install `cargo-deny` pinned to the version the worker verified (0.20.2 on 2026-10-19); `cargo deny check licenses`; then `scripts/check_licence_gate.sh`. Triggers are the file's: push to `main` and pull requests. No build, no cache needed.
4. **The job fails the run.** No `continue-on-error`. Whether it also blocks merges is a repository-ruleset change, which is the owner's (open question 3).
5. **Gate self-test.** `scripts/check_licence_gate.sh` runs `cargo deny --manifest-path <fixture>/Cargo.toml --config deny.toml check licenses` over four fixtures under `scripts/licence-gate-fixtures/`, each a one-crate workspace with its own `[workspace]` table (so the root workspace does not absorb it) and `publish = false`: `gpl-crate` (`GPL-3.0-only`, must fail), `mpl-not-listed` (`MPL-2.0`, crate name not in `exceptions`, must fail), `mpl-listed` (`MPL-2.0`, named `option-ext`, must pass), `dual-choice` (`Apache-2.0 OR GPL-2.0-only`, must pass: a crate that offers a permitted alternative is permitted). The script exits non-zero when any fixture behaves differently and prints the fixture name and the expected result.
6. **Adding a licence.** A new allowance or exception is a reviewed change to `deny.toml` with a comment; there is no wildcard exception.
7. The symphonia exceptions are added by the PR that adds the first `symphonia` crate, one entry per MPL-2.0 crate, so that the "unused exception" warning stays meaningful today.
8. Nothing here changes `Cargo.toml`, `Cargo.lock` or any dependency.

## 4. Data model / schema changes

None.

## 5. Contracts

- Command: `cargo deny check licenses` from the repository root; exit 0 on pass.
- Files: `deny.toml`; `scripts/check_licence_gate.sh` (POSIX shell, `set -eu`); `scripts/licence-gate-fixtures/<case>/Cargo.toml` and `src/lib.rs`.
- CI check name: `Licences`.

## 6. File-format mapping

None.

## 7. UI

None. Evidence is terminal output: the green `cargo deny check licenses` on the PR commit, the red run with the issue's literal list (26 rejected), the self-test script's four lines, and the Actions run of the `licences` job on macOS. Responsive: SKIPPED (owner default). `es-designer` is not needed (no UI).

## 8. Edge cases

1. A crate with an `OR` expression that includes a permitted alternative passes through that alternative (`self_cell`, `r-efi`). The gate checks that a permitted choice exists, not that no copyleft option exists.
2. An `AND` expression needs every part allowed (`unicode-ident`, `epaint_default_fonts`): the second is why `epaint_default_fonts` has an exception.
3. A crate that publishes no licence field or file fails (cargo-deny's default); the fix is a `clarify` entry with file hashes, reviewed.
4. Windows-only and UEFI-only crates are in the graph because `graph.targets` is not set; this is deliberate, so a Windows-only copyleft crate cannot slip in (fact 3 shows `BSL-1.0` crates that macOS does not build).
5. A dev-dependency or build-dependency: build-dependencies are checked (default); dev-dependencies are not (default `include-dev = false`). If a dev-only crate ships in a tarball, that is a later decision.
6. A new `cargo-deny` release that renames a key: the job pins the version, and the bump is a deliberate PR.
7. Two versions of one crate with different licences: each is checked on its own.

## 9. Non-goals

- RustSec advisories, duplicate-version bans and source (registry) checks.
- Licence checks of npm packages, fonts as files, images or the bundled design kit.
- Making `licences` a required merge check (the owner's call, open question 3).
- Auditing the `option-ext` and font licences legally; this task records them, it does not give legal advice.
- The release workflow: no licence file generation.

## 10. Test plan (red first)

Seams: `cargo deny check licenses` (the issue's seam) plus the fixture script. "Fails before" means: the check fails or cannot run on `main` without this task.

| Case | Expected | Fails before |
|---|---|---|
| `current_tree_passes` | `cargo deny check licenses` exits 0 on the PR commit | yes (no config: cargo-deny denies every licence and exits non-zero) |
| `issue_literal_list_is_too_narrow` | the same command with only MIT, Apache-2.0, BSD-2/3, ISC, Zlib exits non-zero (26 rejected on `0e8a9a5`); run once by the worker with the config temporarily trimmed, and pasted in the PR | n/a (evidence, not a committed test) |
| `gpl_crate_fails_the_gate` | fixture `gpl-crate`: non-zero | yes (script absent) |
| `mpl_fails_unless_named` | `mpl-not-listed`: non-zero; `mpl-listed`: zero | yes |
| `dual_licence_with_a_permitted_choice_passes` | `dual-choice`: zero | yes |
| `ci_job_runs_on_macos_and_fails_the_run` | the Actions run shows `licences` on `macos-14`; a throwaway PR commit adding the GPL fixture to the root workspace turns it red, then is dropped | yes (job absent) |
| `no_unused_allowance_warnings` | the green run prints no `unmatched license allowance` warning | yes |
| `workspace_untouched` | `git diff --stat` shows no `Cargo.toml` or `Cargo.lock` change | guard |

Run: `cargo deny check licenses`, `sh scripts/check_licence_gate.sh`, `bash -n scripts/check_licence_gate.sh`.

## 11. Performance budgets

- The `licences` job finishes in under 4 minutes on `macos-14` including the tool install (UNKNOWN: measured on the first run; record it). It runs in parallel with the other jobs and adds no build time.
- Local `cargo deny check licenses` on the current tree: seconds after the first metadata fetch (measured once on an M4 Max: a single-digit number of seconds, recorded in the PR).

## 12. Dependencies and build order

No dependency. Land early in v0.7: later tasks add crates (the plan names `rquickjs`, `wry`, `rav1e`, `symphonia`) and each of them should meet this gate. `ci.yml` is shared with other pack tasks (for example the codegen validator job of #249, #251 and #252): rebase, never hand-merge. #254 lists #166 as its dependency.

## 13. Owner rulings

- R4.16: MPL-2.0 is allowed as an explicit exception (`symphonia`). (Issue text.)
- Allow MIT, Apache, BSD, ISC and Zlib; flag GPL and LGPL; land before new crates are added. (Issue text.)
- The project is dual-licensed MIT OR Apache-2.0 (root `Cargo.toml:20`, `LICENSE-MIT`, `LICENSE-APACHE`).

## Open questions for the owner (recommended first)

1. **Policy beyond the issue's list.** The issue's list fails today (26 rejections). Recommended: also allow `Unicode-3.0`, `CDLA-Permissive-2.0`, `CC0-1.0` and `BSL-1.0`. All four are permissive licences already in the shipped tree. Alternative: name each of those crates as an exception, which is longer and hides that they are permissive.
2. **`option-ext` (MPL-2.0) and the egui font crate (OFL-1.1 and Ubuntu-font-1.0).** These already ship. Recommended: accept both as named exceptions with a comment (R4.16 already accepts MPL-2.0 in principle; OFL-1.1 and the Ubuntu font licence are font licences that allow bundling with attribution). Alternative: replace `dirs` with a crate that has no MPL dependency, or stop bundling `epaint_default_fonts`. That is a separate task.
3. **Required check.** Recommended: ship `licences` as a visible, failing job now, and add it to the repository ruleset's required checks after three green runs on `main` (the precedent is the eval gate's rule in the v0.6 specs). Alternative: required from the first merge. Adding a required check is a ruleset change only the owner can make.
