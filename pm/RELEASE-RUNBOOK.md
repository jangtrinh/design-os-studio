# Release runbook: daily train (owner: PM)

Applies from 2026-10-25. Cadence: 4–5 patches per day, one minor per day (decision D1). PM owns release management end to end (D2). Spec: `plans/specs/v0.<n>-*/` in PRODUCT:OS Spec Driven Format v2. Each patch is one `delivery/<version>.md` slice plus one story with acceptance criteria (AC).

## Roles

| Step | Who | Output |
|---|---|---|
| Spec, slice, AC, gates G1–G3 | PM | `plans/specs/...`, milestone, lint 0 errors |
| Code, PRs, CI fixes, **cutting tags and releases** | Claude | PRs with `Fixes #n`, tag `vX.Y.Z`, GitHub Release |
| Smoke against the AC | QA | `[QA] PASS/FAIL` comment on the patch's tracking issue, with evidence |
| Verify G4/G5, close issues, update the board | PM | `[PM] G5 PASS vX.Y.Z` comment, log `[OWN]` |
| Exception / scope decisions | PM (per D2); escalate to Jang only for: deleting things, spending money, breaking product direction |

## Per-patch flow

1. **Slice ready (G3).** PM confirms the slice's issues are in milestone `vX.Y.Z`, every AC has a scenario, and lint passes: `node tools/lint/lint.mjs plans/specs/<spec>`.
2. **Merge.** Claude opens PRs into the day's release branch (`release/vX.Y.2` for the v0.6 day; from v0.7 on, `release/vX.Y`). Each PR body contains `Fixes #n`. Review gate: CI green (Rustfmt, Clippy, Test ubuntu/macos/windows, Codegen, Design gates).
3. **Release candidate.** Once the slice's PRs are all merged, Claude re-runs CI on the head of the release branch. **Red CI = stop**: fix it before step 4.
4. **QA smoke.** PM posts a `[PM] for:qa` comment with the slice link. QA runs each SCN (manual/visual) on the release-branch head build and reports PASS/FAIL per AC. Slices with UI attach a 1440 px screenshot (G4).
5. **Tag + release.** On QA PASS, Claude creates tag `vX.Y.Z` on the head that was smoked and a GitHub Release with notes listing the issues and PRs. PM does not tag.
6. **G5 + close.** PM checks that the tag/release exists, the CI and QA links are in place, then:
   - `Fixes #n` only auto-closes when merging into the **default branch (`main`)**. Merges into `release/*` do not close issues. So after the release is verified, **PM closes each issue in the slice** with a `[PM] Shipped in vX.Y.Z` comment and the release link.
   - When the release branch is merged back into `main`, any remaining `Fixes #` lines close automatically.
7. **Failure.** An AC that fails gets its issue moved to the next patch (PM records a decision in `decisions/decision-log.md`); the rest of the patch still ships.

## Daily rhythm (UTC+7)

| Time | Work |
|---|---|
| day −1, evening | PM writes the next minor's intake + contract (G1, G2) and creates the patch milestones |
| morning | .0 / first patch: core + safety first |
| every 2–3 h | next patch |
| end of day | release branch merges into `main`; PM retro and logs |

## Checklist per patch (PM copies into the comment)

- [ ] Slice lint 0 errors
- [ ] All PRs merged, `Fixes #` present
- [ ] CI on the release head is green
- [ ] QA PASS for every AC (+ G4 screenshot if there is UI)
- [ ] Tag + Release created by Claude
- [ ] PM closes issues, comments with the release link

## Release branch strategy (PM proposal, 25/10/2026 22:50 UTC+7)

**Fact (git, 25/10):** `release/v0.6.2` was cut from `main` at `2a20e55`. Since then `main` has 140 more commits (features such as #1153, #1131, #1137), and the release branch has 3 commits of its own (head `184e145`, "release: 0.6.2 (#1135)").

**Proposal:**
1. **Patch lines (v0.6.3, v0.6.4, v0.6.5)** are cut from the **`v0.6.2` tag**, not from `main`. The patch carries only the slice's fixes and does not pull in 140 unreviewed feature commits from `main`. This is an empty branch, so it needs no code merge.
   - `git push origin v0.6.2:refs/heads/release/v0.6.3` once the tag exists.
   - v0.6.4 is cut from the `v0.6.3` tag, and so on.
2. **The minor line (v0.7)** is cut from **`main`** as `release/v0.7` after `release/v0.6.x` has merged back into `main`. That way main holds every patch fix before the minor is cut.
3. **PRs already targeting `release/v0.6.2`** that move to v0.6.3 (#1085, #1117) get their base changed to `release/v0.6.3` once that branch exists. Claude or Jang does the retarget. Expect a rebase onto the v0.6.2 tag; there should be no conflicts, because v0.6.2 = the current base + `release: 0.6.2`.
4. **Status 25/10 22:50:** the `v0.6.2` tag **does not exist yet**, so PM has **not created** `release/v0.6.3`. PM creates it right after Jang tags v0.6.2.
