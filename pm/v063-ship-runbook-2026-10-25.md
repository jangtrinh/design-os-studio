# v0.6.3 release runbook, target 26/10/2026 (written 25/10 22:55, UTC+7)

## 1. Blockers
| # | Blocker | Owner | Status |
|---|---|---|---|
| B1 | No `v0.6.2` tag yet, so `release/v0.6.3` can't be cut | Jang (tag at `184e145`) → PM cuts the branch | waiting |
| B3 | #1085 Test (ubuntu-latest) is **pending**; earlier run 40197935120 was ✅ | CI | re-check |

### OPEN DECISION for Jang: Windows red (#1158)
- **Windows Test is red** on #1085 (`beafe3e`) and #1117 (`d5c6628`): STATUS_STACK_OVERFLOW (#1158). **Jang has NOT decided** whether to accept Windows red for releases after v0.6.2. Until he does, the gate is ALL green. Option A: Jang accepts it as a known issue. Option B: land #1158 first (move it from v0.7.1 into v0.6.3) and then re-run CI on #1085/#1117. Correction at 23:04: an earlier version wrongly said Jang had accepted it.
- #1068, #1061, #1057 and #1023 have left v0.6.3. Their fixes (#1071, #1067, #1060, #1035) are already in `184e145`, so they ship with v0.6.2.

## 2. PRs in the slice
| PR | Fixes | Head | CI | Base now → target |
|---|---|---|---|---|
| #1085 fix(agents): keep every hook output in a folder no agent grant reaches | #1072 | `beafe3e` | Clippy/Rustfmt/Codegen/Design/macOS ✅, Ubuntu ⏳, Windows ❌ | release/v0.6.2 → release/v0.6.3 |
| #1117 fix(cli,mcp): refuse broken --screenshot files, humanize spec errors | (no Fixes line) | `d5c6628` | all ✅ except Windows ❌ | release/v0.6.2 → release/v0.6.3 |

## 3. Steps
1. **Jang** tags `v0.6.2` at `184e145` and publishes the release.
2. **PM** runs `git push origin v0.6.2:refs/heads/release/v0.6.3`. This is an empty branch with no code merge (RELEASE-RUNBOOK "Release branch strategy").
3. **Claude** rebases #1085 and #1117 onto `v0.6.2` and retargets them to `release/v0.6.3`.
4. **Gate:** all checks green, Windows included, unless Jang has explicitly accepted Windows red (see OPEN DECISION). Any red = stop.
5. **Jang** merges #1085 and #1117. Claude re-runs CI on the head of `release/v0.6.3`.
6. **QA smoke** (PM posts `[PM] for:qa`), one PASS/FAIL per issue.
7. Claude opens `release: 0.6.3` (Cargo version); **Jang** tags `v0.6.3` and publishes.
8. **PM** closes the slice issues with `[PM] Shipped in v0.6.3` (merging into release/* does not auto-close), then logs [TRAIN].

## 4. Release-notes draft
**DESIGN:OS Studio 0.6.3: agent safety, part 3**
- Agent hooks now write every output into a private folder that no agent grant can reach, so a link planted in the session folder no longer redirects hook writes (#1072, PR #1085). Hook prompts no longer fail on Ubuntu's `dash` shell.
- The CLI and MCP refuse broken `--screenshot` files and show readable spec errors (PR #1117).

**(Only if Jang picks option A) Known issue:** the Windows test suite fails with a stack overflow in CI (#1158). Remove this line if #1158 lands first.
