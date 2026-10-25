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
