# Handoff 2026-11-04 — Project docs workspace (epic #1522), continue on Mac Studio

Owner moved this work to the Mac Studio (the laptop is stopped, memory was low). Nothing is merged yet from phases 01/02/06-read.

## Owner rulings in force (do not re-ask)
- Merge rule: a PR merges only after CI green + independent Opus review APPROVE + controller reruns gates + controller has seen captures. Squash only, never push to main, no auto-merge, no repo-setting changes (merge queue declined).
- "Keep rebasing": when a PR is BEHIND, `gh pr update-branch <n> --rebase` and wait for CI again.
- Speed-up (2026-11-04): bundle #1528 read tools + phase 02 + #1572 spike reports into ONE PR after #1571 merges; #1578 stays separate. 3 CI cycles instead of 5.
- Overview page title = the project name (spec part 2 §2, mockup m02), not "About this project". Phase-02 req 6 already updated.
- Figma not needed for this epic.

## State of each branch (all pushed)
| Branch | Head | What | State |
|---|---|---|---|
| `feat/1523-docs-model-storage` (PR #1571) | b17173f2 | phase 01, `designos-docs` crate | 7th rebase; Test (macos-15) was pending, all other checks green; reviewed APPROVE earlier |
| `feat/1523-docs-bundle` | this commit | #1571 + #1528 read tools + phase 02 (#1524) + #1572 spike reports, 35 commits on top of b17173f2 | NOT a PR yet |
| `feat/1524-project-tab` | 6ac38f0a | phase 02 alone (backup, superseded by bundle) | reviewed APPROVE (2 rounds) |
| `feat/1528-mcp-docs-read` | abdd2e49 | #1528 read tools alone (backup, superseded by bundle) | reviewed APPROVE (round 4) |
| #1572 `docs/1526-spike-report` | d83a1d28 | spike reports, folded into bundle | close as superseded once bundle PR opens |
| #1578 | 58e9a5f8 | Rust 1.95 toolchain + the 3 clippy fixes | BEHIND; merge after bundle |

## Bundle: conflict resolved + verified
- Conflict in `crates/designos-docs/src/store.rs`: #1528 added symlink confinement (`confine`, `read_confined`, removed `read_limited`); phase 02 had renamed `page_path` -> `checked_page_path` without confinement. Resolution: `checked_page_path` keeps #1528's body (path check + `confine`); `store_files.rs::check_hash` now takes `&DocsStore` and reads through `read_confined` (commit "fix(docs): page moves, removals and renames read through the symlink-confined path").
- Independent Opus review of the resolution: APPROVE, 0 blockers. 9 probe tests (symlink escapes on remove/move/rename/adopt all refused), 3-way merge-file comparison shows nothing lost. Notes for follow-ups: N1 dangling-directory-link destination is safe only because mkdir fails and the check runs before the lock (re-check destination under the lock after `create_dir_all`); N2 `create_dir_all` errors show the absolute path as plain Io; N3 pages with >1 hard link can no longer be removed/renamed (read already refused them); N5 intermediate commit does not compile alone (squash merge makes it moot).
- Controller gates on the bundle (laptop, before rebase onto main): fmt 0; `cargo test -p designos-docs` 113 pass; `-p designos-mcp` 599 pass; `-p designos-library` 70 pass; Zero Fake UI 17/17. Clippy: only the 3 known errors in untouched files (`agent_permissions.rs:1176,1194`, `shell/tests/inspector_var_binds.rs:1182`), fixed by #1578, not flagged by CI's pinned toolchain. App bin tests were NOT finished on the laptop (stopped for memory); reviewer ran `--bin designos-studio project_tab` 73 pass.

## Next steps on Mac Studio
1. `git fetch origin && gh pr checks 1571`. If Test (macos-15) green and not BEHIND: `gh pr merge 1571 --squash`, then confirm `crates/designos-docs` is on main. If BEHIND: `gh pr update-branch 1571 --rebase`, wait for CI.
2. Rebase the bundle onto main: `git rebase --onto origin/main b17173f2 feat/1523-docs-bundle` (b17173f2 = last #1571 commit; use the #1571 head that actually merged if it was rebased again).
3. Rerun gates yourself: `cargo fmt --all -- --check`, `cargo test -p designos-docs -p designos-mcp -p designos-library`, `cargo test -p designos-app --bin designos-studio` (designos-app has no lib target), `cargo clippy --workspace --all-targets -- -D warnings`, `python3 scripts/zero_fake_ui_gate.py`. Rules pack limit: `generated/rules.md` must stay <= 23,000 UTF-8 chars (was 22,986; count with Python, not `wc -m`).
4. Push and open ONE PR: body says Closes #1524, part of #1528 (read tools only; write tools + live co-editing stay in #1528 for phase 06), supersedes #1572 (then close #1572 with a link). Note in the body: N1/N2 follow-ups, LOW notes from phase 02 review (Home writes can wait up to 2 s on the docs lock; a missing-file row may remain after a corrupt-manifest delete), and the old-build caveat (an older Studio does not list Page trash items but erases them on Empty trash). Evidence: `plans/reports/evidence-1524/` (real-08 shows all 7 Save/Export/version-history rows greyed on a Project tab). End the body with the Claude Code attribution line.
5. Merge when CI green and up to date; then rebase + merge #1578.
6. Remove merged worktrees and their `target/` dirs.

## Then (wave 3, after the bundle merges)
- Phase 03 embeds + pins; phase 04 block editor (custom egui surface, Telex/IME bug first); phase 05 Mermaid (merman 0.9.3, after #1578); phase 07 spec v2 mapping, KPI strip, "Copy cho AI", retire the old Spec Reader (`linked_specs` reads must stay confined to the linked folder).
- Phase 06 write tools + live co-editing after phase 04.
- Whichever of #1318 / #1528-write merges second registers `doc_text` in the untrusted fence and trims the rules pack.
