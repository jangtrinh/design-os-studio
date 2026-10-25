# M Docs and screenshot refresh after the Untitled UI dark sweep

| Field | Value |
|---|---|
| Issue | #1125 (epic #1118, spec key M) |
| Size | M. One to two days: about ten captures, about ten text lines, the site palette, and the checks in section 10. |
| Depends on | A (colour roles and names, merged), B (scale), C (widget kit), D, E, F, G, H, I, J, K (every surface swept and merged), L (capture list and gate). Phase 2: starts only after the last of D–K has merged. |
| Unblocks | The release task of the pack that ships the revamp (`docs/releasing.md` "Closing a pack", step 6). |
| Touches | `README.md`:<br>- hero `:4-5`<br>- "What's new" images `:148`, `:152`<br>- feature captures and their `Command:` lines `:205-206`, `:215-216`, `:224-225`<br>`docs/index.html`:<br>- `theme-color` `:10`<br>- `:root` palette `:82-85`<br>- `body` `:92`, scrollbar `:91`<br>- og and twitter image `:19`, `:27`<br>- agents still `:434-435`<br>New captures under `docs/images/v<pack>/` (new folder; older folders stay)<br>`docs/images/og-designos-studio.png`, `docs/images/cover.png`<br>`.project-agent.md:6`<br>`CLAUDE.md:6`, `:31`<br>`AGENTS.md:30`<br>`CONTRIBUTING.md:8` |

Code baseline: local `main@ec47da26`, not origin. `origin/main` is `35a65796`, 4 commits ahead (#998 kept only Untitled-kit captures in `docs/images`, #1022 and #1093 changed the site). Between the two commits `README.md` changed by 30 lines, `docs/index.html` by 668, `.project-agent.md` by 6 and `CLAUDE.md` by 2, so the inventory line numbers below are off at `35a65796` and some images may already be replaced. Re-anchor every cited line against HEAD and report drift before editing. HEAD will have moved by the time this runs, because D–K land first.

## 1. Summary and user story

As someone reading the README or the site after the revamp ships, I see the Studio I will download: Untitled UI dark chrome with Studio orange. The words describe that theme too. As an agent reading `.project-agent.md` or `CLAUDE.md`, I get the current theme facts, not the retired "EaseUI BrainWave 2.0" ones.

## 2. Current state (FACT, read at HEAD)

**Images that show Studio chrome:**

| Where | Image | How it was made |
|---|---|---|
| README hero `:4-5` | `docs/images/v0.6/studio-hero.png` | 0.6.0 release capture on a copy of the Untitled kit |
| README "What's new" 0.6.0 `:148`, `:152` | `docs/images/v0.6/check-designs-color-fixes.png`, `ask-before-acting-approval.png` | 0.6.0 release captures |
| README features `:205-206` | `docs/images/designos-agents-chat.png` | `designos-studio untitled-ui-kit.fig --select "Avatar group" --zoom 100 --panel agents --gpu --size 1440x900 --scale 2 --screenshot …` (command printed under the image) |
| README features `:215-216` | `docs/images/designos-variables.png` | `designos-studio untitled-ui-kit.fig --panel variables --gpu --size 1440x900 --scale 2 --screenshot …` |
| README features `:224-225` | `docs/images/designos-devmode.png` | `designos-studio untitled-ui-kit.fig --select "Avatar group" --zoom 100 --dev react --gpu --size 1440x900 --scale 2 --screenshot …` |
| Site agents section `docs/index.html:434-435` | `images/agent-edits-open-file-030.png` | A still from the 0.3.0 launch film |
| Site og and twitter card `:19`, `:27` | `images/og-designos-studio.png` | Marketing card |
| Site "New in 0.3.0" gallery `:467-519` | `images/v0.3/*.png` | 0.3.0 captures, labelled with that version |
| Site hero film `:388` | `videos/designos-studio-v030-*` | Launch film |

**Text that states the old theme:**
- `.project-agent.md:6` "Theme: EaseUI BrainWave 2.0 Dark (`#121215` canvas, `#18181B` panels, …)"
- `CLAUDE.md:6` ("EaseUI BrainWave 2.0 theme tokens") and `CLAUDE.md:31` ("EaseUI BrainWave 2.0 theme (`theme.rs`)"). The same line says `egui 0.31` + `eframe 0.31`, but `crates/designos-app/Cargo.toml:40`, `:51` pin `eframe 0.35.0` and `egui 0.35`, so that line is already wrong.
- `AGENTS.md:30` ("EaseUI BrainWave 2.0 dark/light theme")
- `CONTRIBUTING.md:8` ("EaseUI BrainWave 2.0 Aesthetic")

**The site's own palette mirrors the old app chrome.** `docs/index.html:82-85` holds `--bg:#121215;--panel:#18181B;--surface:#1E1E22;--hover:#232329;--border:#27272A;… --orange:#E36323`. Line `:10` sets `theme-color` to `#121215`, and `:92` sets the body background to `#121215`.

**Not chrome. These stay** (they are document features, not the editor theme):
- The `designos_apply_spec` presets `easeui-*` and `$easeui.*` tokens (`README.md:425`, `CLAUDE.md:61`, `docs/mcp-parity.md:31`, `skills/design-os-studio/SKILL.md:56`). These are generated from `server.rs` by `gen-tool-docs` and describe NodeSpec content.
- The `"#18181B"` example in `skills/design-os-studio/rules.md:256` is a fill example in a document spec.
- `examples/specs/*.spec.json`.

**Owned by other specs:**
- `design/` (tokens, soul, principles, DESIGN.md, specimen) and the agent files in `.agents/agents/` and `.claude/agents/` belong to spec A (plan.md, row A).

**Capture rules already in force:**
- `docs/releasing.md` step 6: captures come from the published release build, on a copy of the Untitled UI PRO v8.0 kit, with `--screenshot … --scale 2`, saved under `docs/images/vX.Y/`. Every capture is real UI reached through the app's own CLI flags; nothing is overlaid or pixel-edited. Older `docs/images/` folders are kept.
- The marketing rule in `CLAUDE.md`: demo UI comes from the kit, worked on a copy, and the kit keeps its own colours.
- `scripts/capture-visual-proof.sh` regenerates `docs/images/designos-*.png` from the built-in `showcase` fixture with `./target/release/designos-studio` and `/tmp`. The README no longer uses those fixture captures for its feature images; it uses kit captures. Running the script would overwrite kit captures with fixture captures.

## 3. Behaviour rules

1. **One build.** Every new capture comes from one build: the Release workflow dry-run `.dmg` of `main`, taken after the last of D–K merged (`docs/releasing.md` "Dry run"). If the pack is released first, use the published `.dmg` instead (step 6). The build version and commit go in the PR body. Dev builds are never used.
2. **Same command, new pixels.** An image that the README prints with its command (`:206`, `:216`, `:225`) is recaptured with exactly that command. The only change is the output path, which moves to the new `docs/images/v<pack>/` folder. The command printed under the image is updated to the new path.
3. **Real UI only.** Captures use the app's CLI flags on a copy of the kit (`untitled-ui-kit.fig`, or the `.designos` copy the marketing rule describes). Nothing is overlaid, retouched or recoloured. A state no flag can reach is left out, and the PR says so (releasing.md step 6).
4. **New folder; old images stay.** New captures go to `docs/images/v<pack>/`, where `<pack>` is the pack version, for example `v0.7`. Old images are not deleted. The README and the site point at the new files.
5. **The hero.** `README.md:4-5` points at a new hero capture. Its `alt` text describes what the new capture shows, written fresh from the image and not edited from the old text.
6. **The agents still on the site** (`docs/index.html:434-435`) is a film still. It is replaced by a real capture of the same scene (the kit file open with an agent's pricing section, Agents panel visible), or kept with its caption saying it is from the 0.3.0 film. Owner question 2.
7. **The og card** (`og-designos-studio.png`) and `cover.png` are rebuilt from the new hero capture: same crop and size as today, with the size measured from the current files.
8. **Text facts.** The four text files state the new theme with Untitled token names and hex values, copied from spec A's merged table in `theme.rs` and never invented here:
   - `.project-agent.md:6` Theme line
   - `CLAUDE.md:6` and `:31`; `:31` also corrects `egui 0.31` + `eframe 0.31` to the versions in `crates/designos-app/Cargo.toml`
   - `AGENTS.md:30`
   - `CONTRIBUTING.md:8`

   The Purple Ban and the canvas Figma-parity colours (selection blue, component cyan) stay in the text.
9. **The site palette** (`docs/index.html:10`, `:82-85`, `:91`, `:92`) follows owner question 1. If the answer is yes:
   - The neutrals take the Untitled dark values spec A merged, mapped role for role: `--bg` → the Untitled role A uses for the app background, `--panel` → panels, `--surface`, `--hover` and `--border` likewise.
   - `--orange`, `--blue` and `--emerald` stay.
   - `theme-color` and the body background follow `--bg`.
   - `ui ds a11y`-style contrast is checked for `--text` and `--muted` on the new `--bg` and `--panel` (section 10, check 5).
10. **Out of this spec's reach** (non-goals): version-labelled release copy ("New in 0.3.0", "What's new" bullets), the films, and the docs pages' own light theme (`docs/assets/site.css`, `docs/_layouts/default.html`). That theme comes from the design-os GitHub page, not the app chrome.

## 4. Data model / schema changes

None.

## 5. Contracts

**Capture list.** The worker runs this list and pastes it into the PR, each item with its exit code and PNG size:

| Output (`docs/images/v<pack>/`) | Command (on the release build, kit copy) |
|---|---|
| `studio-hero.png` | Same framing as the 0.6.0 hero. The 0.6.0 command is not recorded: PR #839, which added the image, does not contain it in its body or comments (checked 2026-10-25). Use `designos-studio untitled-ui-kit.fig --select "Page header" --zoom 100 --gpu --size 1440x900 --scale 2 --no-toast --screenshot docs/images/v<pack>/studio-hero.png`, which matches the hero's alt text ("The Page header component set is selected"). Add the command under the hero image the same way the feature images carry theirs. |
| `designos-agents-chat.png` | `designos-studio untitled-ui-kit.fig --select "Avatar group" --zoom 100 --panel agents --gpu --size 1440x900 --scale 2 --no-toast --screenshot docs/images/v<pack>/designos-agents-chat.png` |
| `designos-variables.png` | `designos-studio untitled-ui-kit.fig --panel variables --gpu --size 1440x900 --scale 2 --no-toast --screenshot docs/images/v<pack>/designos-variables.png` |
| `designos-devmode.png` | `designos-studio untitled-ui-kit.fig --select "Avatar group" --zoom 100 --dev react --gpu --size 1440x900 --scale 2 --no-toast --screenshot docs/images/v<pack>/designos-devmode.png` |
| `check-designs-color-fixes.png`, `ask-before-acting-approval.png` | Only if the release task keeps these two in "What's new" (see the non-goals). Otherwise not recaptured. |
| `agent-edits-open-file.png` | Only on owner answer 2 = "recapture". |

**Text template for `.project-agent.md:6`.** `<role name>` and `<hex>` come from spec A's merged table:
`- **Theme**: Untitled UI PRO v8.0 Dark, adapted to Studio's layout (`bg-primary` <hex> …, `bg-secondary` <hex> panels, `border-secondary` <hex> hairlines, `text-primary` <hex>, brand Studio orange `#E36323` with `#0A0A0A` label text, focus ring Orange-500 `#F97316`; canvas keeps Figma-parity selection blue `#0D99FF`/`#3582FF` and component cyan `#06B6D4`; Purple Ban).`

## 6. File-format mapping

None.

## 7. UI

None in the app. On the site, only the palette variables change, and only if the owner says yes to question 1. Layout, copy and sections are unchanged, so `es-designer` is not needed for `docs/index.html`.

## 8. Edge cases

- **A README command uses a flag that changed during D–K.** For example, spec O may add `--select id:`. Keep the command if it still works. Otherwise use the nearest working command, and the PR names the change.
- **A capture shows personal data.** For example, Home recents with real file names, or the terminal with a home path. Use `--library <seeded dir>` and the kit copy only. Never capture the owner's own library.
- **The Agents panel lists agent CLIs installed on the capture machine.** The 0.6.0 caption already shows them (Claude, Codex, …). Keep the same machine state across captures, and say so in the PR.
- **The release has not been cut when D–K finish.** Use the dry-run `.dmg` of the commit that will be tagged (rule 1). If that commit then changes before tagging, recapture.
- **`docs/images/v<pack>/` already exists** because the release task created it. Add files to it; never overwrite a file this spec did not create.

## 9. Non-goals

- The films (`docs/videos/*`), their posters and the film still's caption. Existing films show the version they were made for. New films follow the marketing rule in `CLAUDE.md`. Owner question 3.
- Version-labelled release copy: "New in 0.3.0" (`docs/index.html:467-519`), the README "What's new" entries and bullets. These belong to the pack's release task (`docs/releasing.md` step 6, and the "Manual follow-up" list of the release-sync PR).
- `design/` files and the design agent files (spec A).
- `easeui-*` NodeSpec presets and `$easeui.*` tokens (document features; generated text).
- Rewriting `scripts/capture-visual-proof.sh`. It captures fixture images that the README no longer uses. File a follow-up to retire it, or point it at the README's kit commands.
- The docs pages' light theme (`docs/assets/site.css`, `docs/_layouts/default.html`).
- `.brv/` session knowledge (`crate-map.md:114`, `:123`). That is agent memory, which the next handoff refreshes.

## 10. Test plan (red first)

| Check | Command | Expected after | Fails before |
|---|---|---|---|
| 1. No old-theme statement in the living docs | `git grep -n -i -E "brainwave 2\.0 (dark\|theme\|aesthetic\|dark/light)\|#121215\|#18181b\|#1e1e22" -- README.md docs/index.html .project-agent.md CLAUDE.md AGENTS.md CONTRIBUTING.md` | No match, except the generated `easeui-*` preset sentences. Allowed pattern: lines containing `presets like easeui-`. | yes (`.project-agent.md:6`, `CLAUDE.md:6`, `:31`, `AGENTS.md:30`, `CONTRIBUTING.md:8`, `docs/index.html:10`, `:83`, `:92`) |
| 2. Every image reference resolves | A short Python check (inline in the PR, not committed) that every `src=`/`href=` under `docs/images/` in `README.md` and `docs/index.html` exists on disk | 0 missing | guard |
| 3. Declared sizes match the files | Same check: each `<img width height>` and `data-size="W × H"` in `docs/index.html` equals the PNG's pixel size (PIL) | 0 mismatches | yes for any replaced image until its attributes are updated |
| 4. The README prints the command that made each feature image | Every `Command:` line under a README image names the file it sits under | 3 of 3 | guard |
| 5. Site contrast (only on owner answer 1 = yes) | `--text` and `--muted` against the new `--bg` and `--panel`, computed with the WCAG formula; values in the PR | ≥ 4.5:1 for `--text`, ≥ 4.5:1 for `--muted` on `--bg`, or the PR names any pair below 4.5 and its use (e.g. only large text) | yes, until the palette exists |
| 6. Visual evidence | The new hero and the three feature captures, viewed in the PR next to the 0.6.0 images | Untitled dark chrome; same regions and framing; kit content unchanged | — |
| 7. Layout gate still green | Spec L's gate on the release commit | green | — |

Also run: `python3 scripts/zero_fake_ui_gate.py` (no app change expected), and render the site locally (`python3 -m http.server` in `docs/`) for a screenshot of the hero and agents sections.

## 11. Performance budgets

- Image weight: each new PNG is no more than 1.5× the file it replaces, or the PR states why. The 2× captures at 1440x900 are the largest. The site loads them lazily (`loading="lazy"` stays).
- No app code changes, so there is no runtime budget.

## 12. Dependencies and build order

Order:
1. Wait for D–K to merge and for the release dry run of `main`.
2. Capture the list in section 5.
3. Update the README and site references and `alt` texts.
4. Rebuild og and cover.
5. Make the text edits in the four files.
6. Change the site palette, if approved.
7. Run checks 1–7.
8. PR (squash).

This is the last PR of the revamp. If it ships in the pack's release task, it goes in the same PR as the "What's new" entry (`docs/releasing.md` step 6), so the site and README change once.

## 13. Owner rulings that apply

- R1/R2 (2026-10-25): Untitled UI dark, 100% of its style, current layout kept.
- R4: brand Studio orange `#E36323` with `#0A0A0A` label; focus ring `#F97316`; Purple Ban stays.
- Marketing videos (2026-10-17, `CLAUDE.md`): demo UI from the Untitled UI PRO v8.0 kit, worked on a copy; kit content keeps its colours; credit in the end card.
- `docs/releasing.md` step 6: captures from the release build, real UI through CLI flags, older image folders kept.
- 2026-09-22: visual evidence in every PR.

## Decisions on the open questions (writer, 2026-10-25)

1. **New folder or overwrite `docs/images/designos-*.png`?** A new `docs/images/v<pack>/` folder. releasing.md asks to keep older folders; the site still uses them; and a versioned path makes it clear which build a picture shows.
2. **Run `scripts/capture-visual-proof.sh`?** No. It captures the `showcase` fixture into the paths the README uses for kit captures, which would break both the marketing rule and the README's printed commands. A follow-up retires it or rewrites it.
3. **Fix `egui 0.31` in `CLAUDE.md:31` here?** Yes. That line is being edited for the theme anyway, and leaving a known-wrong version on an edited line would mislead the next agent. The source is `crates/designos-app/Cargo.toml:40`, `:51`.

## Open questions for the owner

1. **Should the website's own colours (`docs/index.html`) move to the Untitled dark neutrals?** Recommended: **yes, neutrals only.** The site's palette was copied from the old app chrome (`:83`), so after the revamp the screenshots and the page around them would disagree. Orange, blue and emerald stay. Alternative: keep the site as it is; it is marketing, not the app.
2. **The film still on the site (`images/agent-edits-open-file-030.png`).** Recommended: **recapture the same scene as a real screenshot** on the release build: the kit file with the agent-added pricing section and the Agents panel open. A real capture is truer than an old film frame. Alternative: keep it, with the caption saying it shows 0.3.0.
3. **Films.** Recommended: **no new film in this spec.** The existing films are dated by version, and the next pack's launch film (marketing rule) will show the new chrome. Alternative: a short "new look" clip, specced separately.
