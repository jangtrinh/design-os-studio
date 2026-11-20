---
id: detail-1125-docs-and-screenshot-refresh
type: note
title: "M Docs and screenshot refresh after the Untitled UI dark sweep"
updated: "2026-10-25"
status: draft
---

## Nguồn
Bản sao từ nhánh `docs/specs-untitled-ui-dark` (`docs/specs/ui-revamp-untitled-dark/1125-docs-and-screenshot-refresh.md`), lấy ngày 25/10/2026 để pack tự đủ. Nguồn chuẩn chi tiết cho US tương ứng.


| Field | Value |
|---|---|
| Issue | #1125 (epic #1118, spec key M) |
| Size | M, upper end. Two to three days (was one to two). Thirteen images instead of about ten: 11 distinct Studio captures (6 README files, plus 5 site-only files: `designos-hero.png`, `designos-agent-spec.png`, `designos-components.png`, `designos-overview.png`, `v2/anatomy/studio-window-061.webp`) plus `og-designos-studio.png` and `cover.png`. The 5 site-only files have no recorded command, so each scene (page, selection, zoom, panel, size) must be rebuilt from the existing image before it can be recaptured (rule 6), and the `.webp` anatomy capture comes from the site v2 pipeline (#1093), which this spec has not traced. Also about ten text lines, the site palette, and the checks in section 10. |
| Depends on | A (colour roles and names, merged), B (scale), C (widget kit), D, E, F, G, H, I, J, K (every surface swept and merged), L (capture list and gate). Phase 2: starts only after the last of D–K has merged. |
| Unblocks | The release task of the pack that ships the revamp (`docs/releasing.md` "Closing a pack", step 6). |
| Touches | `README.md`:<br>- hero `:4-5`<br>- "What's new" images `:160`, `:164`<br>- feature captures and their `Command:` lines `:217-218`, `:227-228`, `:236-237`<br>`docs/index.html`:<br>- `theme-color` `:10`<br>- `:root` palette `:86-89`<br>- `body` `:96`, scrollbar `:95`<br>- og and twitter image `:19`, `:27`<br>- chrome captures `:306-310`, `:334`, `:347`, `:360`, `:383`, `:414`, `:424`, `:433`, `:455` (placed by hand)<br>New captures under `docs/images/v<pack>/` (new folder; older folders stay)<br>`docs/images/og-designos-studio.png`, `docs/images/cover.png`<br>`.project-agent.md:6`<br>`CLAUDE.md:6`, `:31`<br>`AGENTS.md:30`<br>`CONTRIBUTING.md:8` |

Code baseline: `origin/main@205c72ef` (fetched 2026-10-25). Every file:line was re-anchored from local `main@ec47da26` using a line diff of each file (`git show` at both commits). Lines marked "placed by hand" sit in content that changed: site v2 (#1093) rewrote `docs/index.html`, and release 0.6.1 (#1017) fixed the `egui` version in `CLAUDE.md:31`. The site inventory below was rebuilt from `git show 205c72ef:docs/index.html | grep -n 'src="images/'`. Re-anchor every cited line against HEAD and report drift before editing. HEAD will have moved by the time this runs, because D–K land first.

## 1. Summary and user story

As someone reading the README or the site after the revamp ships, I see the Studio I will download: Untitled UI dark chrome with Studio orange. The words describe that theme too. As an agent reading `.project-agent.md` or `CLAUDE.md`, I get the current theme facts, not the retired "EaseUI BrainWave 2.0" ones.

## 2. Current state (FACT, read at HEAD)

**Images that show Studio chrome:**

| Where | Image | How it was made |
|---|---|---|
| README hero `:4-5` | `docs/images/v0.6/studio-hero.png` | 0.6.0 release capture on a copy of the Untitled kit |
| README "What's new" 0.6.0 `:160`, `:164` | `docs/images/v0.6/check-designs-color-fixes.png`, `ask-before-acting-approval.png` | 0.6.0 release captures |
| README features `:217-218` | `docs/images/designos-agents-chat.png` | `designos-studio untitled-ui-kit.fig --select "Avatar group" --zoom 100 --panel agents --gpu --size 1440x900 --scale 2 --screenshot …` (command printed under the image) |
| README features `:227-228` | `docs/images/designos-variables.png` | `designos-studio untitled-ui-kit.fig --panel variables --gpu --size 1440x900 --scale 2 --screenshot …` |
| README features `:236-237` | `docs/images/designos-devmode.png` | `designos-studio untitled-ui-kit.fig --select "Avatar group" --zoom 100 --dev react --gpu --size 1440x900 --scale 2 --screenshot …` |
| Site hero `docs/index.html:306-310` (placed by hand) | `images/v0.6/studio-hero.png` | The README hero file |
| Site proof card and feature tabs `:334`, `:347`, `:360` | `images/designos-hero.png`, `designos-agent-spec.png`, `designos-devmode.png` | Untitled-kit captures (#998); the page records no command |
| Site anatomy `:383` | `images/v2/anatomy/studio-window-061.webp` | 0.6.1 capture for site v2 (#1093); no command recorded |
| Site workflow posters `:414`, `:424`, `:433` | `images/designos-components.png`, `designos-agents-chat.png`, `designos-devmode.png` | The last two are the README feature files |
| Site scale poster `:455` | `images/designos-overview.png` | Kit page at 6% zoom; no command recorded |
| Site og and twitter card `:19`, `:27` | `images/og-designos-studio.png` | Marketing card |
| Site "What's new" `:464-548` (placed by hand) | `images/v0.6/check-designs-color-fixes.png`, `ask-before-acting-approval.png` (`:488`, `:494`) | The README "What's new" files; 0.5.0, 0.4.0 and 0.3.0 are folded entries with no images |

**Text that states the old theme:**
- `.project-agent.md:6` "Theme: EaseUI BrainWave 2.0 Dark (`#121215` canvas, `#18181B` panels, …)"
- `CLAUDE.md:6` ("EaseUI BrainWave 2.0 theme tokens") and `CLAUDE.md:31` ("EaseUI BrainWave 2.0 theme (`theme.rs`)"). At `205c72ef` that line already defers the `egui`/`eframe` versions to `crates/designos-app/Cargo.toml` (fixed by release 0.6.1), so only the theme wording is left to change.
- `AGENTS.md:30` ("EaseUI BrainWave 2.0 dark/light theme")
- `CONTRIBUTING.md:8` ("EaseUI BrainWave 2.0 Aesthetic")

**The site's own palette mirrors the old app chrome.** `docs/index.html:86-89` holds `--bg:#121215;--panel:#18181B;--surface:#1E1E22;--hover:#232329;--border:#27272A;… --orange:#E36323`. Line `:10` sets `theme-color` to `#121215`, and `:96` sets the body background to `#121215`.

**Not chrome. These stay** (they are document features, not the editor theme):
- The `designos_apply_spec` presets `easeui-*` and `$easeui.*` tokens (`README.md:437`, `CLAUDE.md:61`, `docs/mcp-parity.md:31`, `skills/design-os-studio/SKILL.md:56`). These are generated from `server.rs` by `gen-tool-docs` and describe NodeSpec content.
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
2. **Same command, new pixels.** An image that the README prints with its command (`:218`, `:228`, `:237`) is recaptured with exactly that command. The only change is the output path, which moves to the new `docs/images/v<pack>/` folder. The command printed under the image is updated to the new path.
3. **Real UI only.** Captures use the app's CLI flags on a copy of the kit (`untitled-ui-kit.fig`, or the `.designos` copy the marketing rule describes). Nothing is overlaid, retouched or recoloured. A state no flag can reach is left out, and the PR says so (releasing.md step 6).
4. **New folder; old images stay.** New captures go to `docs/images/v<pack>/`, where `<pack>` is the pack version, for example `v0.7`. Old images are not deleted. The README and the site point at the new files.
5. **The hero.** `README.md:4-5` points at a new hero capture. Its `alt` text describes what the new capture shows, written fresh from the image and not edited from the old text.
6. **The site's chrome captures** (`docs/index.html:306-310`, `:334`, `:347`, `:360`, `:383`, `:414`, `:424`, `:433`, `:455`; placed by hand) are recaptured on the release build. A file shared with the README uses the README's command (rule 2). Others reuse their old scene: same kit page, same selection, same panel, same size as the existing PNG. The PR records the command it used for each one, because the page records none. The 0.3.0 film still that this rule used to cover is no longer on the site at `205c72ef`.
7. **The og card** (`og-designos-studio.png`) and `cover.png` are rebuilt from the new hero capture: same crop and size as today, with the size measured from the current files.
8. **Text facts.** The four text files state the new theme with Untitled token names and hex values, copied from spec A's merged table in `theme.rs` and never invented here:
   - `.project-agent.md:6` Theme line
   - `CLAUDE.md:6` and `:31` (theme wording only; the `egui` version was already fixed upstream)
   - `AGENTS.md:30`
   - `CONTRIBUTING.md:8`

   The Purple Ban and the canvas Figma-parity colours (selection blue, component cyan) stay in the text.
9. **The site palette** (`docs/index.html:10`, `:86-89`, `:95`, `:96`) moves to the Untitled dark neutrals (owner ruling R10.6, 2026-10-25: "A · Đổi sang xám Untitled"):
   - The neutrals take the Untitled dark values spec A merged, mapped role for role: `--bg` → `BG` `#0A0A0A`, `--panel` → `BG_SECONDARY` `#171717` (one fill step per role kept so the page keeps its bands; owner board 3, Q6), `--surface`, `--hover` and `--border` likewise from token-contract.md.
   - `--orange`, `--blue` and `--emerald` stay.
   - `theme-color` and the body background follow `--bg`.
   - Contrast for `--text` and `--muted` on the new `--bg` and `--panel` is computed with the WCAG formula (section 10, check 5; board 3 measured `--muted` 7.85:1 on `--bg`, 7.11:1 on `--panel`).
10. **Out of this spec's reach** (non-goals): version-labelled release copy (the site's and the README's "What's new" entries and bullets), the films, and the docs pages' own light theme (`docs/assets/site.css`, `docs/_layouts/default.html`). That theme comes from the design-os GitHub page, not the app chrome.

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
| `designos-hero.png`, `designos-agent-spec.png`, `designos-components.png`, `designos-overview.png`, `v2/anatomy/studio-window-061.webp` | Rule 6: same scene as the existing file; the PR records the command. |

**Text template for `.project-agent.md:6`.** `<role name>` and `<hex>` come from spec A's merged table:
`- **Theme**: Untitled UI PRO v8.0 Dark, adapted to Studio's layout (`bg-primary` <hex> …, `bg-secondary` <hex> panels, `border-secondary` <hex> hairlines, `text-primary` <hex>, brand Studio orange `#E36323` with `#0A0A0A` label text, focus ring Orange-500 `#F97316`; canvas keeps Figma-parity selection blue `#0D99FF`/`#3582FF` and component cyan `#06B6D4`; Purple Ban).`

## 6. File-format mapping

None.

## 7. UI

None in the app. On the site, only the palette variables change (R10.6). Layout, copy and sections are unchanged, so `es-designer` is not needed for `docs/index.html`.

## 8. Edge cases

- **A README command uses a flag that changed during D–K.** For example, spec O may add `--select id:`. Keep the command if it still works. Otherwise use the nearest working command, and the PR names the change.
- **A capture shows personal data.** For example, Home recents with real file names, or the terminal with a home path. Use `--library <seeded dir>` and the kit copy only. Never capture the owner's own library.
- **The Agents panel lists agent CLIs installed on the capture machine.** The 0.6.0 caption already shows them (Claude, Codex, …). Keep the same machine state across captures, and say so in the PR.
- **The release has not been cut when D–K finish.** Use the dry-run `.dmg` of the commit that will be tagged (rule 1). If that commit then changes before tagging, recapture.
- **`docs/images/v<pack>/` already exists** because the release task created it. Add files to it; never overwrite a file this spec did not create.

## 9. Non-goals

- The films (`docs/videos/*`) and their posters. The site no longer embeds a film at `205c72ef`. Existing films show the version they were made for. New films follow the marketing rule in `CLAUDE.md`. Owner question 3.
- Version-labelled release copy: the site's "What's new" (`docs/index.html:464-548`), the README "What's new" entries and bullets. These belong to the pack's release task (`docs/releasing.md` step 6, and the "Manual follow-up" list of the release-sync PR).
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
| 5. Site contrast (R10.6) | `--text` and `--muted` against the new `--bg` and `--panel`, computed with the WCAG formula; values in the PR | ≥ 4.5:1 for `--text`, ≥ 4.5:1 for `--muted` on `--bg`, or the PR names any pair below 4.5 and its use (e.g. only large text) | yes, until the palette exists |
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
6. Change the site palette (R10.6).
7. Run checks 1–7.
8. PR (squash).

This is the last PR of the revamp. If it ships in the pack's release task, it goes in the same PR as the "What's new" entry (`docs/releasing.md` step 6), so the site and README change once.

## 13. Owner rulings that apply

- R1/R2 (2026-10-25): Untitled UI dark, 100% of its style, current layout kept.
- R4: brand Studio orange `#E36323` with `#0A0A0A` label; focus ring `#F97316`; Purple Ban stays.
- Marketing videos (2026-10-17, `CLAUDE.md`): demo UI from the Untitled UI PRO v8.0 kit, worked on a copy; kit content keeps its colours; credit in the end card.
- `docs/releasing.md` step 6: captures from the release build, real UI through CLI flags, older image folders kept.
- 2026-09-22: visual evidence in every PR.
- R10.6 (2026-10-25, board 3 Q6): the site palette moves to the Untitled dark neutrals; orange, blue and emerald stay.

## Decisions on the open questions (writer, 2026-10-25)

1. **New folder or overwrite `docs/images/designos-*.png`?** A new `docs/images/v<pack>/` folder. releasing.md asks to keep older folders; the site still uses them; and a versioned path makes it clear which build a picture shows.
2. **Run `scripts/capture-visual-proof.sh`?** No. It captures the `showcase` fixture into the paths the README uses for kit captures, which would break both the marketing rule and the README's printed commands. A follow-up retires it or rewrites it.
3. **Fix `egui 0.31` in `CLAUDE.md:31` here?** Nothing to do: release 0.6.1 already replaced it with a pointer to `crates/designos-app/Cargo.toml` (`CLAUDE.md:31` at `205c72ef`).

## Open questions for the owner

1. **Decided (R10.6, owner 2026-10-25: "A · Đổi sang xám Untitled").** Was: should the website's own colours (`docs/index.html`) move to the Untitled dark neutrals? Recommended: **yes, neutrals only.** The site's palette was copied from the old app chrome (`:87`), so after the revamp the screenshots and the page around them would disagree. Orange, blue and emerald stay. Alternative: keep the site as it is; it is marketing, not the app.
2. Withdrawn on 2026-10-25: the film still is no longer on the site at `205c72ef`. Site v2 (#1093) replaced it with Untitled-kit captures, which rule 6 covers.
3. **Decided (controller default, plan.md 21:35 sweep): no new film in this spec.** Films. The existing films are dated by version, and the next pack's launch film (marketing rule) will show the new chrome. Alternative: a short "new look" clip, specced separately.

**Changelog:** 2026-10-25 (round 2): Size revised from "M, one to two days" to "M, upper end, two to three days". Site v2 grew the image list from about ten to thirteen, and five of the new site captures need their scenes rebuilt because no command was recorded. Re-anchored to `205c72ef` earlier the same day.
