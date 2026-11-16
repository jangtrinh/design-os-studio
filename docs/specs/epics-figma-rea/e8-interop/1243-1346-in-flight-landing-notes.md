# In-flight E8 bugs with open PRs: landing notes (#1243, #1346)

Code baseline `origin/main@d22dad970`. Both bugs are still real on main; each has an open PR that targets it. No new build; the controller lands or re-scopes the PR.

## #1243 Auto layout wraps a title row's buttons where Figma keeps them on one line — PR #1680

- Still real on main: FACT per PR #1680's base regression ("action y=24, expected y=0" on main) — not re-run here.
- PR #1680 (`fix/issue-1243-auto-layout-wrap-v2`, "Closes #1243"): wrapped line packing treated primary-axis Fill children as fixed natural width; fix treats them as zero-basis flex items and distributes leftover space per wrapped line. Validation in PR: `cargo test -p designos-doc layout_solver` 44/44, clippy, fmt, independent review approved.
- CI state (gh, 2026-11-06): `mergeStateStatus = UNSTABLE`; `Test (macos-15)`, Clippy, Rustfmt, Check green; `Test (windows-latest)` red (advisory). FACT: ruleset "Protect main" requires `Test (macos-15), Clippy, Rustfmt, Check` (gh api rulesets), so #1680 meets the required checks. CLAUDE.md §Git Workflow still says `Test (macos-14)` — stale doc line, flag for a docs fix.
- #1411's rollup shows `Test (macos-14)` (older run); after `update-branch` it must run `Test (macos-15)`.
- Controller ledger already places #1680 in contributor "Wave B, after #1667 and #1672 merge (they share files)" (`plans/261106-2245-epic-waves/controller-ledger.md`).
- Gaps against the issue's AC: AC-2 ("break points match Figma within 1 px" when children do not fit) and AC-3 (synthetic fixture for both cases) — PR text shows the fit case only. Ask the PR to add the not-fit case to the regression before merge.
- Tier for any follow-up: Grok build-fast (solver test, machine-checkable).

## #1346 .fig import: text in a scaled instance renders at the main component's style size — PR #1411

- Still real on main: `scale_contents` (`crates/designos-fig/src/rest_import.rs:2270`) scales `font_size` of text and runs (`:2306`, `:2308` within the function) and leaves the text style link and typography bindings, so `resolve_node` restores the style size (issue's diagnosis; line numbers moved from `:2003` / `:2039` cited in the issue).
- PR #1411 (`fix/issue-1346-scaled-instance-text`): detaches inherited text style + typography bindings on scaled instance text and scales size, pixel line height, letter and paragraph spacing, indents, runs. CI: required checks green, Windows red (advisory); `mergeStateStatus = BEHIND` (needs `gh pr update-branch 1411 --rebase`).
- Owner ruling on record: "#1411 (owner left it out on 11-04)" for this release (`controller-ledger.md`). Not a rejection of the fix; release scope only.
- Open before calling it done (from the issue): Figma's behaviour for the 0.875 and 0.833 scale classes — does Figma drop the text style for every scaled instance, and does it scale line height? Owner-manual Figma read (`figma-agent inspect` on two kit instances of each class). The PR scales line height; if Figma does not, AC fails.
- Tier for a rebase/follow-up: Claude Sonnet (HOT FILE `rest_import.rs`; text-style semantics).
