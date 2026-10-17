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

1. Make sure `main` is green (tests on Linux, Windows and macOS, Clippy).
2. Bump `version` under `[workspace.package]` in the root `Cargo.toml`, run `cargo build --workspace` so `Cargo.lock` updates, and commit both (`release: 0.2.0`).
3. Tag and push:

   ```sh
   git tag -a v0.2.0 -m "designos 0.2.0"
   git push origin v0.2.0
   ```

   A tag containing a hyphen (`v0.2.0-rc.1`) is published as a pre-release.
4. Watch the Release workflow. When all builds pass, a GitHub Release is created with auto-generated notes. Edit the notes if needed.

## What gets built

| Job | Runner | Output |
|---|---|---|
| `build` `x86_64-pc-windows-msvc` | `windows-latest` | `designos-<version>-<target>.zip` (the exe has the icon and version info embedded by `crates/designos-app/build.rs`) |
| `build` `x86_64-unknown-linux-gnu` | `ubuntu-latest` | `designos-<version>-<target>.tar.gz` (adds `designos.desktop`, `icons/`, `install.sh`) |
| `macos` | `macos-14` (builds `aarch64-apple-darwin` and cross-compiles `x86_64-apple-darwin`; `macos-13` is retired) | `designos-<version>-macos.dmg` and `designos-<version>-macos-universal.tar.gz` |

The Windows and Linux archives hold `designos`, `designos-diff`, `LICENSE-MIT`, `LICENSE-APACHE` and `README.md`. They are packaged by `scripts/package.sh <target> <version>`, which you can run locally after a release build.

The macOS job runs `scripts/package-macos.sh <version>`. It `lipo`s a universal `designos` and `designos-diff`, assembles `designos.app` (bundle id `os.design.studio`, `Info.plist` from `packaging/macos/Info.plist.in`, `designos.icns` built with `iconutil` from `assets/icon/designos.iconset`), ad-hoc signs it, and writes the `.dmg` (app, `Applications` link, `Read me first.txt`) and the universal `.tar.gz` (app, licenses, README). It needs a Mac, so test changes to it with a dry run (below).

The release carries a combined `SHA256SUMS` covering every file (verify with `sha256sum -c SHA256SUMS`).

## App icons

The masters are `assets/icon/designos-icon.svg` (full bleed, Windows and Linux) and `assets/icon/designos-icon-macos.svg` (824 px tile on a 1024 canvas). All generated files are committed; regenerate them after editing a master with:

```sh
cargo run -p designos-app --example gen_icons
```

This rewrites `assets/icon/png/`, `designos.ico`, `designos.iconset/` and `designos.icns`. The window icon is the embedded 256 px PNG.

## Dry run

Run the workflow from the Actions tab (Release, Run workflow) on any branch. It builds and packages everything and uploads the archives as workflow artifacts, but creates no release. Run it after any change to `scripts/package-macos.sh` or the macOS plist, then open the downloaded `.dmg` on a Mac to check the icon, the drag-to-Applications layout and the Open Anyway flow. The version is stamped `0.0.0-dev.<sha>`.

## Not covered yet

- macOS: the app is ad-hoc signed but not signed with a Developer ID or notarized, so Gatekeeper asks for a one-time Open Anyway. Double-clicking a file in Finder does not pass it to designos (winit 0.30 does not expose `application:openFiles:`); "Open With" lists designos through `CFBundleDocumentTypes`.
- Windows binaries are unsigned (SmartScreen warning). There is no installer; the zip is the download.
- Linux binaries link the build runner's glibc; they run on that distribution or newer.
