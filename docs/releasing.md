# Releasing

Releases are tag-driven. Pushing a tag `vX.Y.Z` runs `.github/workflows/release.yml`.

## Release packs (standard process)

Work ships in **packs**. A pack is one theme a user can describe in one sentence, such as "Layout 2.0".
- A pack ends as a minor release `v0.X.0`.
- Once one pack ships, the next one starts.
- Hotfixes ship as `v0.X.Y` from `main` at any time and do not wait for a pack.

**Where packs live:**
- Each pack is a GitHub milestone, for example `v0.4 Layout 2.0`.
- It is also an option of the **Pack** field on the roadmap project: https://github.com/users/jangtrinh/projects/9.
- Every task belongs to exactly one pack. Its release notes are built from the milestone.

**Rules while a pack is open:**
- **Picking:** workers pick Ready tasks from the current pack first. They take a later pack's task only when the current pack has nothing Ready and unclaimed.
- **Merging:** the coordinator merges only PRs of the current pack. A later pack's PRs are verified, then wait until the current pack is tagged. Those workers keep them rebased.
- **Composition:** each pack contains one headline feature, plus the review follow-ups of the earlier pack, plus one or two infra items.
- **Hot files:** tasks that touch the same hot file (`fig.rs`, `kiwi.rs`, `rest_import.rs`) run one after another, never in parallel.

**Closing a pack.** One **release task** per pack, labelled `release`, is created when the pack opens and is picked up by one machine.

1. **Freeze.** Every other issue in the milestone is closed. Feature merges stop.
2. **Gates on a clean checkout of `origin/main`:**
   - `cargo fmt --all --check`
   - `cargo clippy --workspace --all-targets --locked -- -D warnings`
   - `cargo test --workspace --locked --no-fail-fast`
   - `python3 scripts/zero_fake_ui_gate.py`
   - `cargo test -p designos-gpu --test large_file_perf` on an otherwise idle machine
   - Paste every output into the release task.
3. **Dry run.** Run the Release workflow from the Actions tab on `main` (see Dry run below). Download the macOS `.dmg` and open it on a Mac.
4. **Smoke test** the packaged app, not a debug build. Attach a desktop screenshot for each step to the release task.
   1. Launch it.
   2. Open one real `.fig` file.
   3. Exercise the headline feature of the pack.
   4. Save, close and reopen the file.
   5. Export a PNG.

   A step that needs a person, such as copying from Figma desktop, is marked **owner-manual** in the task and done by the owner. It is never claimed by an agent.
5. **Bump, tag and publish** with the checklist below.
   - The version is `0.X.0` of the milestone.
   - Edit the release notes into user language, grouped by the pack's features.
6. **After the release:**
   - Close the milestone.
   - Set the next pack's tasks to Ready.
   - Merge the waiting verified PRs of the next pack. Each worker rebases first, then the coordinator re-runs the gates.

If a gate fails, the release task files a blocker issue in the same milestone, and the pack stays open. Never tag over a red gate.

## Checklist

1. Make sure `main` is green. macOS tests and Clippy must pass; we verify releases on macOS, and Windows and Linux are tested by their users.
2. On a branch, bump `version` under `[workspace.package]` in the root `Cargo.toml` and run `cargo update -w` so `Cargo.lock` updates. Commit both (`release: X.Y.Z`) and open a PR. `main` is protected, so the release commit lands through a squash merge like any other change. The workflow's `version-check` job fails the release when the tag and this version differ.
3. Once the release PR is merged, run the dry run (below) on `main` and smoke-test its `.dmg`. Then tag that same squash commit and push the tag. Release tags cannot be moved or deleted afterwards, so the tagged commit must be the one you tested:

   ```sh
   git fetch origin
   git tag -a vX.Y.Z origin/main -m "designos X.Y.Z"
   git push origin vX.Y.Z
   ```

   A tag containing a hyphen (`v0.2.0-rc.1`) is published as a pre-release.
4. Watch the Release workflow. When the macOS build passes, a GitHub Release is created with auto-generated notes. Windows and Linux packages are attached if their builds pass, and a failure there does not block the release. Edit the notes if needed.
5. Let the **Release sync pages** workflow update the README, the homepage and the docs.
   - It runs by itself when the Release workflow finishes on a tag (`workflow_run`, as long as its "Publish release" job succeeded, even if a Windows or Linux leg failed; the release is published with `GITHUB_TOKEN`, which never fires `release: published`) or when a person publishes a release from the UI (`.github/workflows/release-sync-pages.yml`, script `scripts/release-sync-pages.py`). It waits for the release assets, rewrites the files, checks that every new download URL returns HTTP 200, and opens one PR on the branch `release-sync/vX.Y.Z`. It never pushes to `main`.
   - What it rewrites: the `releases/download/v<old>/…` links and their `<small>NN MB</small>` sizes in `docs/index.html` (and any such link in `README.md`, `docs/install.md`, `docs/llms.txt`), the `Download Studio <ver>` heading, every `build-release.sh <ver>` example, and the `<lastmod>` in `docs/sitemap.xml`. It links only assets the release lists: a missing macOS `.dmg` fails the run, a missing optional Windows or Linux package is dropped from the page with a warning in the PR body.
   - What it leaves alone: copy that names a version, such as "New in 0.3.0", the film captions and the `#new-in-v030` anchors. They need real new content. The PR body lists each leftover mention under **Manual follow-up**; write that copy in a separate PR.
   - Merge the PR after the release is published. GitHub Pages deploys on every push to `main`, so links bumped before the assets exist would 404. The repository setting "Allow GitHub Actions to create and approve pull requests" is off today, so the workflow can only open the PR when the `RELEASE_SYNC_TOKEN` secret (PAT or app token, contents and pull-requests write) is set. That token also makes the PR start the required checks; a PR opened with `GITHUB_TOKEN` does not, and needs a close and reopen. `RELEASE_SYNC_TOKEN` needs no Actions permission: the workflow reads the Release run's jobs with the built-in token, and fails the run (never skips the sync) when it cannot read them.
   - Re-run for a release that is already published: Actions → Release sync pages → Run workflow, with `version` set to `X.Y.Z`. The run is idempotent: on an up-to-date tree it opens nothing.
   - **Manual fallback** (workflow broken or the PR is needed locally): on a branch from `main`, run `python3 scripts/release-sync-pages.py X.Y.Z --verify-urls --report /tmp/report.md`. It reads `gh release view vX.Y.Z`, so you need `gh` signed in. Review `git diff`, commit, and open the PR. The tests are `python3 -m unittest discover -s scripts/tests -v`. Without the script, read the names from `gh release view vX.Y.Z --json assets --jq '.assets[].name'`, edit `docs/index.html` and the files above by hand, and link only the assets that list shows.
   - Not covered: the `EXPECT_VERSION` default in `scripts/release-smoke-macos.sh`. The smoke test runs before the tag exists, so a post-release sync would always leave it one version behind. Pass `EXPECT_VERSION=X.Y.Z` when you run it.

## What gets built

| Job | Runner | Output |
|---|---|---|
| `build` `x86_64-pc-windows-msvc` | `windows-latest` | `designos-<version>-<target>.zip` with `install.ps1` (the exe has the icon and version info embedded by `crates/designos-app/build.rs`). A `-setup.exe` NSIS installer is added only when `makensis` is on the runner's `PATH`; `windows-latest` does not have it, so v0.2.0 and v0.3.0 shipped the zip only. |
| `build` `x86_64-unknown-linux-gnu` | `ubuntu-latest` | `designos-<version>-<target>.tar.gz` (adds `designos.desktop`, `icons/`, `install.sh`) and `designos_<version>_amd64.deb` (built when `dpkg-deb` is available) |
| `macos` | `macos-14` (builds `aarch64-apple-darwin` and cross-compiles `x86_64-apple-darwin`; `macos-13` is retired) | `designos-<version>-macos.dmg` and `designos-<version>-macos-universal.tar.gz` |

Every package holds all four tools (`designos-studio`, `designos`, `designos-mcp`, `designos-diff`) plus `LICENSE-MIT`, `LICENSE-APACHE` and `README.md`. They are packaged by `scripts/package.sh <target> <version>`, which you can run locally after a release build.

The macOS job runs `scripts/package-macos.sh <version>`. It `lipo`s a universal build of each of the four tools, assembles `designos.app` (bundle id `os.design.studio`, `Info.plist` from `packaging/macos/Info.plist.in`, `designos.icns` built with `iconutil` from `assets/icon/designos.iconset`), ad-hoc signs it, and writes the `.dmg` (app, `Applications` link, `install-cli.command`, `Read me first.txt`) and the universal `.tar.gz` (app, `install-cli.command`, licenses, README). It needs a Mac, so test changes to it with a dry run (below).

The release carries a combined `SHA256SUMS` covering every file (verify with `sha256sum -c SHA256SUMS`).

## App icons

The masters are `assets/icon/designos-icon.svg` (full bleed, Windows and Linux) and `assets/icon/designos-icon-macos.svg` (824 px tile on a 1024 canvas). All generated files are committed; regenerate them after editing a master with:

```sh
cargo run -p designos-app --example gen_icons
```

This rewrites `assets/icon/png/`, `designos.ico`, `designos.iconset/` and `designos.icns`. The window icon is the embedded 256 px PNG.

## Dry run

Run the workflow from the Actions tab (Release, Run workflow) on any branch, or `gh workflow run release.yml --ref <branch>`. It builds and packages everything and uploads the archives as workflow artifacts, but creates no release. The file names are stamped `0.0.0-dev.<sha>`, and the app inside reports the `Cargo.toml` version.

Before every release tag, run it on `main` after the version bump has merged, so the commit you smoke-test is the commit you tag. Also run it on a branch after any change to `release.yml`, `scripts/package-macos.sh` or the macOS plist. Release tags cannot be moved or deleted, so a broken tagged build costs a version number. Then open the downloaded `.dmg` on a Mac and check:

- the icon, the drag-to-Applications layout and the Open Anyway flow;
- the About dialog shows the new version;
- a large `.fig` opens and renders.

## Not covered yet

- macOS: the app is ad-hoc signed but not signed with a Developer ID or notarized, so Gatekeeper asks for a one-time Open Anyway. Double-clicking a file in Finder does not pass it to designos (winit 0.30 does not expose `application:openFiles:`); "Open With" lists designos through `CFBundleDocumentTypes`.
- Windows binaries are unsigned (SmartScreen warning). In practice there is no installer, because the runner has no `makensis`; the zip with `install.ps1` is the download. Windows and Linux builds are tested by their users; releases are verified on macOS.
- Linux binaries link the build runner's glibc; they run on that distribution or newer.
