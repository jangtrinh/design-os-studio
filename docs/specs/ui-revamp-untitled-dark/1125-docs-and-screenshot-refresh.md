# M Docs and screenshot refresh after the Untitled UI dark sweep

| Field | Value |
|---|---|
| Issue | #1125 (epic #1118, spec key M) |
| Size | M. About two days (was two to three). Nine captures: 4 shared with the README (`studio-hero.png`, `designos-agents-chat.png`, `designos-variables.png`, `designos-devmode.png`) and 5 site-only (`designos-hero.png`, `designos-components.png`, `designos-overview.png`, `designos-agent-spec.png`, `v2/anatomy/studio-window-<build>.webp`). Every one of them has a recorded command (section 5), so no scene has to be rebuilt from an old image. Also a copy for `cover.png`, a recolour of the og card, about ten text lines, the site palette (nine `:root` values, six literals, three `rgba` triplets) and the checks in section 10. The six anatomy SVG overlays and `anatomy.json` are re-traced only if the trace tool turns up (open question 1). |
| Depends on | A (colour roles and names, merged), B (scale), C (widget kit), D, E, F, G, H, I, J, K (every surface swept and merged), L (capture rules, document sources and the geometry report), O (shipped in v0.6.5: `--select id:<n>` and `fig:<id>`, and a miss exits 2 with no PNG). Phase 2: starts only after the last of D–K has merged. |
| Unblocks | The release task of the pack that ships the revamp (`docs/releasing.md` "Closing a pack", step 6). |
| Touches | `README.md`: hero `:4-5`, credit line `:8`, feature-capture provenance `:249`, the three feature captures and their `Command:` lines `:254-255`, `:264-265`, `:273-274`.<br>`docs/index.html`: `theme-color` `:10`; og and twitter image `:19`, `:27`; `:root` values `:87-88`; scrollbar `:95`; body `:96`; literals `:122`, `:126`, `:154`; chrome captures `:319`, `:345`, `:358`, `:371`, `:394`, `:425`, `:435`, `:444`, `:466` (placed by hand).<br>`docs/assets/v2-anatomy.css` `:19`, `:72`; `docs/assets/v2-anatomy.js` `:122`.<br>New captures under `docs/images/v<pack>/` (new folder; older folders stay); `docs/images/v2/anatomy/` (the `.webp`, six SVGs and `anatomy.json` change together or not at all).<br>`docs/images/og-designos-studio.png` (colours only), `docs/images/cover.png` (copy of the new `designos-hero.png`; `docs/_config.yml:16` points at it).<br>`.project-agent.md:6`, `CLAUDE.md:6`, `:31`, `AGENTS.md:30`, `CONTRIBUTING.md:8`. |

Code baseline: `origin/main@8dfaee93` (v0.7.0 merged, #1314). First written at `ec47da26`, re-anchored to `205c72ef`, then to `8dfaee93` on 2026-10-28 with `git show 8dfaee93:<path>` (no checkout). Drift that mattered: the README grew from the 0.6.1 to the 0.7.0 notes, so its captures and provenance lines moved from `:160-237` to `:197-274`; the site's image lines moved by +13 (`:306-310` to `:319`; the `:root` neutrals stay on `:87-88`); `shell.rs` gained the library default and the `pages_height` read at `:106-118`; `--select` misses exit 2 (`main.rs:237`). Lines marked "placed by hand" sit in content that site v2 (#1093) rewrote. Re-anchor every cited line against HEAD and report drift before editing, because D–K land first and HEAD will have moved.

## 1. Summary and user story

As someone reading the README or the site after the revamp ships, I see the Studio I will download: Untitled UI dark chrome with Studio orange. The words describe that theme too, and every caption says which build made the picture. As an agent reading `.project-agent.md` or `CLAUDE.md`, I get the current theme facts, not the retired "EaseUI BrainWave 2.0" ones.

## 2. Current state (FACT, read at `8dfaee93`)

**Images that show Studio chrome.** "Recorded" means a command exists in a tracked file or in a report.

| Where | Image | How it was made (recorded source) | What the pixels show (viewed) |
|---|---|---|---|
| README hero `:4-5`; site hero `docs/index.html:319`; scale poster start (`v2-big-files`) | `docs/images/v0.6/studio-hero.png` | 0.6.0 release capture: `designos-studio kit.fig --select "Page header"` plus the common flags (section 5; `plans/reports/release-060-evidence/p0-readme-audit.md:28-32`, an untracked local report, inlined in section 5) | the Page header component set selected |
| README `:254-255`, `:264-265`, `:273-274`; site `:371`, `:435`, `:444` | `designos-agents-chat.png`, `designos-variables.png`, `designos-devmode.png` | same report, `:33-35`; the commands are printed under each image | as the file names say |
| Site proof card `:345`; cover | `designos-hero.png`, `cover.png` (a copy) | `scripts/capture-visual-proof.sh:74-75` (#998, `18e19a2f`) | the kit's `Desktop` dashboard frame at 55 %, not the Page header its `alt` claims |
| Site proof card `:358` | `designos-agent-spec.png` | same script, `:130-163`: two kit frames exported as NodeSpec, edited and applied back | `Metric group (from NodeSpec)` selected, not the Agents picker its `alt` claims |
| Site workflow `:425` | `designos-components.png` | same script, `:81` | the `Buttons/Button` set at 35 % |
| Site scale poster `:466` | `designos-overview.png` | same script, `:84` | the kit's first page, "Getting started", at 8 %, not the "Page headers" page at 6 % its `alt` claims |
| Site anatomy `:394` | `v2/anatomy/studio-window-061.webp`, six traced SVG overlays, `anatomy.json` | `plans/reports/agy-site-v2-060-evidence/logs/anatomy-capture-061.txt` (tracked) and `plans/reports/agy-site-v2-060.md` §6.1-6.2 | the `Input field` frame at 17 % |
| Site og and twitter card `:19`, `:27` | `og-designos-studio.png` (1200 × 630, #128, no recorded generator) | none | a composed card: wordmark, "Own your design canvas.", a tagline, and a kit canvas crop with no Studio chrome; flat colours `#121215`, `#F4F4F5` (text), `#A1A1AA` (tagline), `#E36323` ("Own") |
| Site "What's new" `:541`, `:547`; README `:197`, `:201` | `v0.6/check-designs-color-fixes.png`, `ask-before-acting-approval.png` | 0.6.0 release captures | release copy (non-goal) |

Three site `alt` texts disagree with their images (`:345`, `:358`, `:466`). The image wins: the scene stays, and the `alt` is rewritten from the new capture (rule 5).

**Text that states the old theme** (check 1 returns exactly these nine lines at `8dfaee93`): `.project-agent.md:6`, `AGENTS.md:30`, `CLAUDE.md:6` and `:31`, `CONTRIBUTING.md:8`, `docs/index.html:10`, `:87`, `:88`, `:96`.

**The site's palette mirrors the old app chrome.** `docs/index.html:87-88` holds `--bg:#121215;--panel:#18181B;--surface:#1E1E22;--hover:#232329;--border:#27272A;--line:#52525B;--text:#F4F4F5;--muted:#A1A1AA;--ink:#121215`, and `:89` the accents. Old-palette values also sit outside `:root`: `:10` (`theme-color`), `:95`, `:96`, `:122`, `:126`, `:154`, and three `rgba` triplets in the site's own assets, `docs/assets/v2-anatomy.css:19` and `:72` and `docs/assets/v2-anatomy.js:122`. Check 1b finds all eleven lines.

**Not chrome. These stay** (document features, not the editor theme): the `designos_apply_spec` sentence with "EaseUI BrainWave 2.0 presets like easeui-" (`README.md:474`, `CLAUDE.md:61`, `docs/mcp-parity.md:31`, `skills/design-os-studio/SKILL.md:56`, generated from `server.rs` by `gen-tool-docs`), the `"#18181B"` fill example (`skills/design-os-studio/rules.md:258`) and `examples/specs/*.spec.json`.

**Owned by other specs.** `design/` and the agent files in `.agents/agents/` and `.claude/agents/` (spec A).

**Capture rules already in force.**
- `docs/releasing.md` step 6 (`:45-47`): captures come from the published release build, on a copy of the Untitled UI PRO v8.0 kit, with `--screenshot … --scale 2`, under `docs/images/vX.Y/`; every capture is real UI reached through the app's own flags, nothing overlaid or pixel-edited; older folders stay.
- The marketing rule in `CLAUDE.md`: demo UI comes from the kit, worked on a copy; kit content keeps its colours; the original is never shown or edited.
- **Document sources** (owner ruling 2026-10-27, spec L rule 14): the Untitled UI kit copy only. Never `--fixture` (any name: `showcase`, `demo`, …), never the old built-in samples. Release builds since 0.6.5 reject `--fixture` anyway (#1252).
- `scripts/capture-visual-proof.sh` is kit-only since #998 (`:5-12`, `DESIGNOS_PROOF_DOC`). It records the commands above, but it is not run (rule 6): it builds a dev binary when none exists (`:41-44`), writes into `docs/images/` by default (`:39`), requires a `.designos` copy, and its `agents-chat` step starts a live agent session (`:76`).

**Machine state that reaches a capture at `8dfaee93`.** `Shell::new` takes the library from `--library`; with none, a screenshot run reads the default root for `pages_height` (`shell.rs:115-118`), and `default_root()` follows `$DESIGNOS_LIBRARY` or the per-user data directory (`store.rs:467`). So the Pages and Layers split follows the capturing machine unless `--library` names a scratch folder (rule 4).

## 3. Behaviour rules

1. **One build.** Every new capture comes from one build: the Release workflow dry-run `.dmg` of `main`, taken after the last of D–K merged (`docs/releasing.md` "Dry run", `:104`). If the pack is released first, use the published `.dmg` (step 6). The build version and commit go in the PR body and in the README provenance lines (rule 7). Dev builds are never used.
2. **The recorded commands, not the script.** Each image is recaptured with the command recorded for it (section 5), plus the recorded common flags. Only the output path and the kit path change. The printed command under a README image gains `--no-toast` and `--library <empty folder>` so it matches what ran; the kit path there stays the `untitled-ui-kit.fig` shorthand.
3. **Real UI only, kit copy only, outside the repository.** The original `.fig` is only read. Work from an APFS clone (`cp -c`) at an absolute path outside any git work tree and outside `$HOME`, for example `/tmp/designos-m-<run>/untitled-ui-kit.fig`; every command names that absolute path. Hash the original and the copy before and after (`shasum -a 256`); all four values are equal and go in the PR. Nothing is overlaid, retouched or recoloured. A state no flag can reach is left out, and the PR says so. Derived documents (the agent-spec pipeline) live in the same scratch folder. Check 8 fails if a `.fig` or `.designos` file is staged, modified or untracked in the work tree; the tracked starter and test fixtures are untouched by this PR.
4. **No machine state or personal data in a picture.** Every command passes `--library <empty scratch folder>`; never the owner's library, never a client file. The PR records a visual check of each image: no user name, no `$HOME` path, no session list, no recent files. The Agents capture still lists the agent CLIs installed on the capturing machine (the README says so); keep the same machine for all captures.
5. **The selection is proven.** A `--select` miss exits 2 and writes no PNG (`main.rs:237`), so exit 0 is the proof. Record stderr: a note "matched N layers; selected id X" or "matched no layer name; selected node id X" means the name was ambiguous or a bare number. Where a note appears, rerun with `--select id:<n>` and put the id and the layer name in the PR. Compare the framing with the image being replaced; if "Page header" now resolves to another layer than in 0.6.0 (O changed the lookup order), use `id:<n>` of the layer the 0.6.0 hero shows. Every replaced image gets a fresh `alt`, written from the new picture and not edited from the old text; the three wrong `alt`s of section 2 are rewritten the same way.
6. **The site's chrome captures** (`:345`, `:358`, `:425`, `:466`, and the hero and anatomy) keep their scene: same kit page, same selection, same panel, same size as the file they replace. A file shared with the README uses the README's command. Others use the commands in section 5, quoted from the script and from the anatomy log, not run through the script. The script's bare node ids (`81249`, `46971`, `79550`, `44616`) are not copied: ids belong to one import (`id_stable: false`), so each run derives them from `--inspect-json` of its own copy.
7. **Version strings follow the captures.** `README.md:5` (`alt`), `:8` and `:249` name the build that took the new images. They are provenance, not "What's new" copy. `README.md:192` and the "What's new" entries stay as they are (non-goal).
8. **The hero.** `README.md:4-5` points at the new `studio-hero.png`, and gets a `Command:` line like the feature images (check 4).
9. **Text facts.** The four text files state the new theme with Untitled token names and hex values, copied from spec A's merged table in `theme.rs` and never invented here (the Theme line credits Untitled UI, R14):
   - `.project-agent.md:6` Theme line
   - `CLAUDE.md:6` and `:31` (theme wording only; the `egui` versions already point at `Cargo.toml`)
   - `AGENTS.md:30`
   - `CONTRIBUTING.md:8`

   The Purple Ban and the canvas Figma-parity colours (selection blue, component cyan) stay in the text.
10. **The site palette** moves to the Untitled dark neutrals (owner R10.6: "A · Đổi sang xám Untitled"; mapping shown on board 3 Q6, `specs/owner-questions-board-3-spec.py:177-180`). Section 5 has one row per mapping: old value, Untitled role name, new value, ratio. `--orange`, `--blue` and `--emerald` stay. `theme-color` and the body background follow `--bg`. The values that were not on the board (three literals, three `rgba` triplets) take the nearest Untitled neutral (a controller default, recorded in section 5; the owner may reverse it).
11. **The og card keeps its composition** (R1, controller ruling C-R): same 1200 × 630, wordmark, headline, tagline and canvas crop. Only its colours change, to the site's new `--bg`, `--text` and `--muted`; the orange and the kit crop stay. No generator is recorded, so the method is part of the PR: measure the current flat colours (the table in section 2), then remap the three neutrals while keeping each pixel's coverage between old background and old foreground. The card is a marketing composition, not a UI capture, so a flat-colour remap is allowed; the kit crop must compare identical (`magick compare -metric AE` on that rectangle prints 0). A recomposition goes to the owner through AskUserQuestion, or is out of scope.
12. **Out of this spec's reach** (non-goals): version-labelled release copy (the site's and the README's "What's new" entries and bullets), the films, and the docs pages' own light theme (`docs/assets/site.css`, `docs/_layouts/default.html`).

## 4. Data model / schema changes

None.

## 5. Contracts

**Capture method.** From the 0.6.0 evidence report, inlined here because the report is local and untracked (it does not exist in a worker's worktree; `git ls-tree 8dfaee93` finds nothing), quoted verbatim:

> Every capture: `--gpu --size 1440x900 --scale 2 --no-toast --library <scratch>` (2880x1800), a helper retried until the PNG existed (the GPU capture fails on some runs unless the display is awake; `caffeinate -u -d` PID 85180 plus `caffeinate -u -t 4` before each run). Lossless optimisation: `magick -strip -define png:compression-level=9 -define png:compression-strategy=0 -define png:compression-filter=4 -define png:exclude-chunk=all`; `magick compare -metric AE` raw vs committed = 0 for all four.

> `docs/images/v0.6/studio-hero.png` (new, README hero) | `designos-studio kit.fig --select "Page header"` | Window with the Page header component set selected, Pages and Layers on the left, Design panel with Properties on the right

The other three README rows of that report (`:33-35`) read `--panel variables`, `--select "Avatar group" --zoom 100 --dev react` and `--select "Avatar group" --zoom 100 --panel agents`. The hero has no `--zoom`.

A capture helper that follows it (`caffeinate -t` is ignored when a utility is given, `man caffeinate`, so the 4 s call has no command; set `BIN` to the `designos-studio` inside the `.dmg`'s app, `KIT` to the absolute path of the scratch copy, `LIB` to an empty scratch folder; run one capture at a time on an awake display):

```bash
caffeinate -u -d & CAFF=$!; trap 'kill "$CAFF"' EXIT   # held for the whole session, as in the 0.6.0 run
capture() {  # capture OUT.png [flags...]
  out="$1"; shift
  for attempt in 1 2 3; do
    rm -f "$out"; caffeinate -u -t 4                       # blocks 4 s and turns a sleeping display on
    "$BIN" "$KIT" "$@" --gpu --size 1440x900 --scale 2 --no-toast --library "$LIB" --screenshot "$out" && [ -s "$out" ] && return 0
    echo "attempt $attempt produced no $out" >&2
  done
  return 1
}
shasum -a 256 "$ORIGINAL_KIT" "$KIT"     # before the first capture and after the last: both pairs equal
magick RAW.png -strip -define png:compression-level=9 -define png:compression-strategy=0 -define png:compression-filter=4 -define png:exclude-chunk=all OUT.png
magick compare -metric AE RAW.png OUT.png null: 2>&1   # prints 0
```

**Capture list.** The worker runs this list and pastes it into the PR: each item with its command, exit code, selected id (rule 5), PNG size and the `shasum` pairs. `<f>` = the common flags the helper adds.

| Output (`docs/images/v<pack>/`) | Recorded source | Command (after `"$BIN" "$KIT"`; `<f>` then `--screenshot <out>`) |
|---|---|---|
| `studio-hero.png` | `p0-readme-audit.md:32` | `--select "Page header"` (no `--zoom`) |
| `designos-agents-chat.png` | `p0-readme-audit.md:35` | `--select "Avatar group" --zoom 100 --panel agents` (never `agents-chat`: it starts a live session) |
| `designos-variables.png` | `p0-readme-audit.md:33` | `--panel variables` |
| `designos-devmode.png` | `p0-readme-audit.md:34` | `--select "Avatar group" --zoom 100 --dev react` |
| `designos-hero.png`, then `cover.png` as a copy | `capture-visual-proof.sh:74-75` | `--select "Desktop" --zoom 55` (the kit's 1440 × 960 dashboard; the script comment at `:72`) |
| `designos-components.png` | `capture-visual-proof.sh:81` | `--select "Buttons/Button" --zoom 35` |
| `designos-overview.png` | `capture-visual-proof.sh:84` | `--select none --rulers` (no selection; the current image shows the kit's first page at 8 %) |
| `designos-agent-spec.png` | `capture-visual-proof.sh:130-163` | the pipeline below, then `--select "Metric group (from NodeSpec)" --zoom 100` on the derived document |
| `v2/anatomy/studio-window-<build>.webp` | `anatomy-capture-061.txt` | `--select "Input field" --zoom 17`, then the resize below |
| `check-designs-color-fixes.png`, `ask-before-acting-approval.png` | 0.6.0 release captures | only if the release task keeps them in "What's new" (non-goal); otherwise not recaptured |

**Agent-spec pipeline** (the script's `:130-163`, with ids derived, not copied). Work in the scratch folder: make a `.designos` import of the copy once with `designos-mcp` (`designos_open_file` of the copy, `designos_save_file` to a new name), run `"$BIN" "$KITDOC" --inspect-json` and read the ids of the frames named `Badges` and `Metric group` and of the pages `Modals` and `Dashboards 01`; export the two frames with `--select id:<n> --export-spec`, check that the exported roots are named `Badges` and `Metric group` (the script's own guard, `:149-150`), strip ids and apply the script's wrap layout (`:137-158`), then `--apply-spec … --out "$WORK/kit-with-specs.designos"` on the two pages. Nothing is written to the repository.

**Anatomy scene.** The recorded capture (`anatomy-capture-061.txt`, verbatim, with the paths of that run):

`/tmp/agy-v061-scratch/target/release/designos-studio /tmp/agy-v061-scratch/untitled-ui-kit.fig --select "Input field" --zoom 17 --size 1440x900 --scale 2 --gpu --screenshot /tmp/agy-v061-scratch/renders/anatomy-window-gpu.png --library /tmp/agy-v061-scratch/renders/library --no-toast`

Its log shows the GPU and CPU renders byte-identical; check that again. The 2880 × 1800 PNG then becomes the site file: `magick RAW.png -filter Lanczos -resize 1600x1000! -quality 82 studio-window-<build>.webp` (the report used Pillow `Image.LANCZOS`, q 82; `magick` has the same encoder here and `python3 -I` has no Pillow). The file name carries the build, as `-030` and `-061` do; `docs/index.html:394` points at it. **The `.webp`, the six SVG overlays and `anatomy.json` change together or not at all.** The re-trace runs after spec L's gate has run on the release commit, because L's G1 report (the `--layout-json` region rectangles of the editor) says which regions moved; the overlay anchors in `anatomy.json` are checked against it, and the six layers are traced again from the new `.webp`. The report names the tool (`pipeline.py`, `emit.py`, `gate.py --previews`, `source_check.py`, `finalize.py`, gates: recall and precision of the 1 px edge match and a source-gradient check at 95 %, `agy-site-v2-060.md` §6.2), but it is in no tracked file at `8dfaee93` and was not found on the writer's machine (open question 1).

**Site palette: one row per mapping.** Names are the Untitled role names, with the `theme.rs` constant that board 3 Q6 used in brackets (token-contract (a)); "new" ratios are WCAG 2.x, computed by script, with the old ratio in brackets. Decorative pixels are marked.

| Old value | Where | New name | New value | Ratio (backdrop) |
|---|---|---|---|---|
| `#121215` | `--bg` `:87` | `bg-primary` (`BG`), R10.6 | `#0A0A0A` | `--text` on it 18.97 (17.01) |
| `#18181B` | `--panel` `:87` | `bg-secondary` (`BG_SECONDARY`), R10.6 | `#171717` | `--text` 17.18 (16.12); edge against `--bg` 1.10 (1.06) |
| `#1E1E22` | `--surface` `:87` | `bg-tertiary` (`BG_TERTIARY`), board 3 Q6 | `#262626` | `--text` 14.50 (15.11); against `--bg` 1.31 (1.13) |
| `#232329` | `--hover` `:87` | `bg-secondary_hover` (`BG_SECONDARY_HOVER`), Q6 | `#262626` | as `--surface` (the two roles take one value) |
| `#27272A` | `--border` `:87` | `border-secondary` (`BORDER`), Q6 | `#262626` | against `--bg` 1.31 (1.26) |
| `#52525B` | `--line` `:87` | `border-primary` (`BORDER_STRONG`), Q6 | `#404040` | against `--bg` 1.91 (2.42) |
| `#F4F4F5` | `--text` `:88` | `text-primary` (`TEXT`), Q6 | `#FAFAFA` | on `--bg` 18.97 (17.01), on `--panel` 17.18 (16.12) |
| `#A1A1AA` | `--muted` `:88` | `text-tertiary` (`TEXT_TERTIARY`), Q6 | `#A3A3A3` | on `--bg` 7.85 (7.30), `--panel` 7.11 (6.91), `--surface` 6.00 (6.48) |
| `#121215` | `--ink` `:88` | Neutral-950 (`TEXT_ON_BRAND`), Q6 | `#0A0A0A` | on `--text` 18.97 (17.01), `--orange` 5.73 (5.41), `--blue` 6.62 (6.25), `--emerald` 6.82 (6.45) |
| `#121215` | `theme-color` `:10`, body `:96` | `bg-primary` (`BG`) | `#0A0A0A` | follows `--bg` |
| `#3F3F46` | scrollbar thumb `:95` | Neutral-700 (`border-primary`, `BORDER_STRONG`), nearest | `#404040` | on `--bg` 1.91 (1.79), decorative |
| `#D4D4D8` | `.btn-primary:active` `:122` | Neutral-300 (`text-secondary`, `TEXT_SECONDARY`), nearest | `#D4D4D4` | `--ink` label on it 13.36 (12.65) |
| `#3F3F46` | `.btn-primary small` `:126` | Neutral-700 (`border-primary`), nearest | `#404040` | on the `--text` fill 9.93 (9.50) |
| `#2C2C33` | hero dot grid `:154` | Neutral-800 (`bg-tertiary`, `BG_TERTIARY`), nearest | `#262626` | on `--bg` 1.31 (1.35), decorative |
| `rgba(244,244,245,…)` | `v2-anatomy.css:19` (`.5`), `v2-anatomy.js:122` | `text-primary` (`TEXT`) | `rgba(250,250,250,…)` | `.5` over `--bg` 5.15 (4.94), decorative word dim that ends at `var(--text)` |
| `rgba(161,161,170,.7)` | `v2-anatomy.css:72` | `text-tertiary` (`TEXT_TERTIARY`) | `rgba(163,163,163,.7)` | over `--bg` 4.30 (4.15), decorative trace layer |
| `#121215`, `#F4F4F5`, `#A1A1AA` | og card flat colours (rule 11) | `bg-primary`, `text-primary`, `text-tertiary` | `#0A0A0A`, `#FAFAFA`, `#A3A3A3` | 18.97 and 7.85 against the new background (17.01, 7.30) |

"Nearest" is the least Euclidean distance in sRGB to one of the eleven Untitled Neutral steps (950 to 50): `#3F3F46` to `#404040` is 6.2, `#D4D4D8` to `#D4D4D4` 4.0, `#2C2C33` to `#262626` 15.5 (`#404040` is 31.1). None of the three literals was on the board; this is the controller default. **Stay, and why:** `--orange` `#E36323`, `--blue` `#0D99FF`, `--emerald` `#14AE5C` (owner R10.6); `#5CB8FF` (`a:hover`, `:229`, a light step of the blue family); the `--tint` triplets `227 99 35` and `13 153 255` (`:338`, `:351`, orange and blue); `#fff` (`:233`, `:237`; the kit's `fg-primary` is `#FFFFFF` in Dark); `#000` and `#000000` in `:155` and in a quoted layer name (`:541`; a mask colour that is never painted, and kit content); `rgba(255,255,255,.1)` (`:168` and four site asset files; the kit's `alpha-black-10` resolves to white at 10 % in Dark) and the `.08` and `.06` white overlays in `v2-story.css` and `v2-workflow.css`. One consequence to know: `--line`, the 1 px border of the ghost button, drops from 2.42:1 to 1.91:1 against `--bg`. The owner chose option A on board 3 Q6; the button's label carries its meaning (`--text` 18.97:1). `--surface` and `--hover` now share one value, as on the board.

**Text template for `.project-agent.md:6`.** `<role name>` and `<hex>` come from spec A's merged table:
`- **Theme**: Untitled UI PRO v8.0 Dark, adapted to Studio's layout (`bg-primary` <hex> …, `bg-secondary` <hex> panels, `border-secondary` <hex> hairlines, `text-primary` <hex>, brand Studio orange `#E36323` with `#0A0A0A` label text, focus ring Orange-500 `#F97316`; canvas keeps Figma-parity selection blue `#0D99FF`/`#3582FF` and component cyan `#06B6D4`; Purple Ban).`

## 6. File-format mapping

None.

## 7. UI

None in the app. On the site, only the palette values change (R10.6). Layout, copy and sections are unchanged, so `es-designer` is not needed for `docs/index.html`.

## 8. Edge cases

- **A README command uses a flag that changed during D–K.** Keep the command if it still works. Otherwise use the nearest working command, and the PR names the change.
- **A capture shows personal data.** Rule 4. If a file name, a path or a session list shows, fix the setup and recapture; never crop or paint over it (rule 3).
- **The Agents panel lists the CLIs installed on the capture machine.** The README caption already says so. Keep the same machine for every capture and say so in the PR.
- **`--select "Page header"` resolves to another layer than in 0.6.0.** Rule 5.
- **The release has not been cut when D–K finish.** Use the dry-run `.dmg` of the commit that will be tagged (rule 1). If that commit then changes before tagging, recapture.
- **`docs/images/v<pack>/` already exists** because the release task created it. Add files to it; never overwrite a file this spec did not create.
- **The GPU capture produces no PNG.** The helper retries three times on an awake display (`caffeinate`); after that, stop and report. Never fall back to a CPU render for a README image without saying so (the anatomy log shows the two are byte-identical, which is a check, not a licence).

## 9. Non-goals

- The films (`docs/videos/*`) and their posters. The site no longer embeds a film. New films follow the marketing rule in `CLAUDE.md`. Owner question 3.
- Version-labelled release copy: the site's "What's new" (`docs/index.html` `:541`, `:547` and their entries), the README "What's new" entries and bullets (`:192-201`). These belong to the pack's release task (`docs/releasing.md` step 6).
- `design/` files and the design agent files (spec A).
- `easeui-*` NodeSpec presets and `$easeui.*` tokens (document features; generated text).
- Editing or retiring `scripts/capture-visual-proof.sh` (a follow-up). It is kit-only since #998 and not run here (rule 2).
- The docs pages' light theme (`docs/assets/site.css`, `docs/_layouts/default.html`).
- `.brv/` session knowledge (`crate-map.md:114`, `:123`): agent memory, refreshed by the next handoff.
- Recomposing the og card (rule 11).

## 10. Test plan (red first)

Each command is run from the repository root. Check 1 and check 1b, run against `8dfaee93`, return the lines below; that is what "fails before" means. The Python and shell blocks were run against an extract of `8dfaee93` and against a copy with the mapping applied (red, then green).

**Check 1. No old-theme statement in the living docs.** Plain alternation, one fenced block (no table escape to copy by mistake). Expected after: no output (`git grep` exits 1). The generated `presets like easeui-` sentences do not match this pattern.

```bash
git grep -n -i -E "brainwave 2\.0 (dark|theme|aesthetic|dark/light)|#121215|#18181b|#1e1e22" -- README.md docs/index.html .project-agent.md CLAUDE.md AGENTS.md CONTRIBUTING.md
```

Fails before, 9 lines: `.project-agent.md:6`, `AGENTS.md:30`, `CLAUDE.md:6`, `CLAUDE.md:31`, `CONTRIBUTING.md:8`, `docs/index.html:10`, `:87`, `:88`, `:96`.

**Check 1b. No mapped old value is left on the page or in its assets** (every hex and `rgba` triplet of the section 5 table). Expected after: no output.

```bash
git grep -n -i -E "#121215|#18181b|#1e1e22|#232329|#27272a|#52525b|#f4f4f5|#a1a1aa|#3f3f46|#d4d4d8|#2c2c33|244, ?244, ?245|161, ?161, ?170" -- docs/index.html 'docs/assets/v2-*'
```

Fails before, 11 lines: `docs/assets/v2-anatomy.css:19`, `:72`, `docs/assets/v2-anatomy.js:122`, `docs/index.html:10`, `:87`, `:88`, `:95`, `:96`, `:122`, `:126`, `:154`.

**Check 2. Every image reference resolves** (inline in the PR, not committed). Expected: `0 missing`, exit 0; guard (16 references at `8dfaee93`).

```bash
python3 -I - <<'PY'
import os, re, sys
bad = 0
for doc in ("README.md", "docs/index.html"):
    refs = re.findall(r'''(?:src|href)=["']([^"'#?]*images/[^"'#?]+)["']''', open(doc).read())
    for r in sorted(set(refs)):
        if r.startswith("http"): continue
        path = os.path.normpath(os.path.join(os.path.dirname(doc), r))
        if not os.path.isfile(path): print(f"{doc}: missing {path}"); bad += 1
print(f"{bad} missing"); sys.exit(1 if bad else 0)
PY
```

**Check 3. Declared sizes match the files.** `sips` reads PNG and WebP (the stock `python3 -I` has no Pillow). Expected: `0 mismatch(es)`; guard (11 images with `width` and `height` at `8dfaee93`).

```bash
python3 -I - <<'PY'
import os, re, subprocess, sys
bad = n = 0
for m in re.finditer(r"<img\b[^>]*>", open("docs/index.html").read()):
    tag = m.group(0)
    src, w, h = (re.search(p, tag) for p in (r'src="([^"]+)"', r'width="(\d+)"', r'height="(\d+)"'))
    if not (src and w and h) or src.group(1).startswith("http"): continue
    path = os.path.normpath(os.path.join("docs", src.group(1)))
    out = subprocess.run(["sips", "-g", "pixelWidth", "-g", "pixelHeight", path], capture_output=True, text=True).stdout
    pw, ph = (int(re.search(rf"{k}: (\d+)", out).group(1)) for k in ("pixelWidth", "pixelHeight"))
    n += 1
    if (pw, ph) != (int(w.group(1)), int(h.group(1))): print(f"{path}: file {pw}x{ph}, attributes {w.group(1)}x{h.group(1)}"); bad += 1
print(f"{n} images with width and height, {bad} mismatch(es)"); sys.exit(1 if bad else 0)
PY
```

**Check 4. The README prints the command that made each captured image** (the hero and the three features). Expected: `4 of 4`; fails before with `3 of 4` (the hero has no `Command:` line).

```bash
python3 -I - <<'PY'
import re, sys
md = open("README.md").read(); bad = 0
want = ("studio-hero.png", "designos-agents-chat.png", "designos-variables.png", "designos-devmode.png")
for name in want:
    m = re.search(rf'<img src="(docs/images/[^"]*{re.escape(name)})"', md)
    if not m: print(f"{name}: no <img> in README.md"); bad += 1; continue
    if f"--screenshot {m.group(1)}" not in md[m.end(): m.end() + 900].split("</p>")[0]:
        print(f"{name}: the Command: line does not print --screenshot {m.group(1)}"); bad += 1
print(f"{len(want) - bad} of {len(want)} images print the command that made them"); sys.exit(1 if bad else 0)
PY
```

**Check 4b. No stale build label on the replaced images.**

```bash
git grep -n -E "0\.6\.0 (with|on a copy)|from the v0\.6\.0 macOS build" -- README.md
```

Expected after: one line, `README.md:192` (the 0.6.0 "What's new" images, non-goal). Fails before, 4 lines: `:5`, `:8`, `:192`, `:249`.

**Check 5. Site palette and contrast (R10.6).** It asserts the exact values of section 5 and then the ratios: `--text` and `--muted` on `--bg` and `--panel` at least 4.5:1, and `--ink` on `--text`, `--orange`, `--blue` and `--emerald` at least 4.5:1. Expected: `0 failure(s)`; fails before with 9 value mismatches.

```bash
python3 -I - <<'PY'
import re, sys
css = open("docs/index.html").read()
root = dict(re.findall(r"(--[a-z]+):(#[0-9A-Fa-f]{6})", css.split(":root{")[1].split("}")[0]))
want = {"--bg": "#0A0A0A", "--panel": "#171717", "--surface": "#262626", "--hover": "#262626", "--border": "#262626", "--line": "#404040",
        "--text": "#FAFAFA", "--muted": "#A3A3A3", "--ink": "#0A0A0A", "--blue": "#0D99FF", "--orange": "#E36323", "--emerald": "#14AE5C"}
def lum(h):
    c = [int(h[i:i + 2], 16) / 255 for i in (1, 3, 5)]
    c = [x / 12.92 if x <= 0.04045 else ((x + 0.055) / 1.055) ** 2.4 for x in c]
    return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
def ratio(a, b):
    hi, lo = sorted((lum(root[a]), lum(root[b])), reverse=True)
    return (hi + 0.05) / (lo + 0.05)
bad = 0
for k, v in want.items():
    if root.get(k, "").upper() != v: print(f"{k}: page has {root.get(k)}, expected {v}"); bad += 1
if not bad:
    for a, b in (("--text", "--bg"), ("--text", "--panel"), ("--muted", "--bg"), ("--muted", "--panel"), ("--ink", "--text"), ("--ink", "--orange"), ("--ink", "--blue"), ("--ink", "--emerald")):
        r = ratio(a, b); bad += r < 4.5; print(f"{a:>7} on {b:<8} {r:5.2f}:1 {'ok' if r >= 4.5 else 'FAIL'}")
print(f"{bad} failure(s)"); sys.exit(1 if bad else 0)
PY
```

**Check 6. Visual evidence.** The new hero and the three feature captures, and the five site captures, viewed in the PR next to the images they replace: Untitled dark chrome, same regions and framing, kit content unchanged, no user name, `$HOME` path or session list (rule 4). A screenshot of the local site (`python3 -m http.server` in `docs/`) shows the hero and the agents sections on the new palette.

**Check 7. Layout gate still green.** Spec L's gate on the release commit: green. Its G1 report also feeds the anatomy re-trace.

**Check 8. No kit file in the work tree.** Run before every commit and again before the push. Expected: exit 0; it exits 1 when a `.fig` or `.designos` file is staged, modified or untracked. The tracked starter and test fixtures are untouched by this PR, so they never appear. Proved on a throwaway repository: clean tree exit 0; a staged `untitled-ui-kit.fig`, an untracked `kit-copy.designos` and a modified tracked `.designos` each exit 1.

```bash
bad=$({ git diff --cached --name-only --diff-filter=ACMR; git diff --name-only --diff-filter=ACMR; git ls-files --others --exclude-standard; } | grep -E '\.(fig|designos)(\.tmp)?$' | sort -u)
[ -z "$bad" ] || { echo "kit file in the repository work tree:" >&2; echo "$bad" >&2; exit 1; }
echo "no .fig or .designos file staged, modified or untracked"
```

Also run: `python3 scripts/zero_fake_ui_gate.py` (no app change expected), and `git status --short` at the end shows only the files in Touches.

## 11. Performance budgets

- Image weight: each new PNG is no more than 1.5× the file it replaces, or the PR states why (after the lossless `magick` pass of section 5). The 2× captures at 1440x900 are the largest. The site loads them lazily (`loading="lazy"` stays).
- No app code changes, so there is no runtime budget.

## 12. Dependencies and build order

Order:
1. Wait for D–K to merge and for the release dry run of `main`. Wait for spec L's gate result on that commit.
2. Make the scratch copy and hash it (rule 3); run the capture list in section 5; run check 8.
3. Update the README and site references, the `alt` texts and the version strings (rules 5 and 7).
4. Recolour the og card and copy `cover.png` (rules 11 and section 5).
5. Make the text edits in the four files.
6. Change the site palette, the literals and the three `rgba` triplets (rule 10).
7. Anatomy re-trace (decision 4): the `.webp`, the six SVG overlays and `anatomy.json` are redone together after L's geometry report, with the trace tool if it is found, otherwise by hand from L's geometry JSON. Never ship a new `.webp` under the old overlays.
8. Run checks 1 to 8.
9. PR (squash).

This is the last PR of the revamp. If it ships in the pack's release task, it goes in the same PR as the "What's new" entry (`docs/releasing.md` step 6), so the site and README change once.

## 13. Owner rulings that apply

- R1/R2 (2026-10-25): Untitled UI dark, 100% of its style, current layout kept.
- R4: brand Studio orange `#E36323` with `#0A0A0A` label; focus ring `#F97316`; Purple Ban stays.
- R10.6 (2026-10-25, board 3 Q6): the site palette moves to the Untitled dark neutrals; orange, blue and emerald stay.
- R14 (2026-10-28): the design store uses Studio's own names (specs A and T); the living-doc Theme line credits Untitled UI once (rule 9).
- Marketing videos (2026-10-17, `CLAUDE.md`): demo UI from the Untitled UI PRO v8.0 kit, worked on a copy; kit content keeps its colours; credit.
- 2026-10-27 (spec L rule 14): documents for verification are the kit copy and the shipped starter, never the old samples or `--fixture`.
- `docs/releasing.md` step 6: captures from the release build, real UI through CLI flags, older image folders kept.
- 2026-09-22: visual evidence in every PR. 2026-10-10: responsive checks only on request (SKIPPED here).
- Controller rulings (plan.md C-R): the og card keeps its composition and only its colours change; CLI ids are per session.

## Decisions on the open questions (writer, 2026-10-25, revised 2026-10-28)

1. **New folder or overwrite `docs/images/designos-*.png`?** A new `docs/images/v<pack>/` folder. releasing.md asks to keep older folders; the site still uses them; and a versioned path makes it clear which build a picture shows.
2. **Run `scripts/capture-visual-proof.sh`?** No, and for a different reason than first written. It is kit-only since #998 and records the commands this spec reuses, but it builds a dev binary, writes into `docs/images/` and starts a live agent session. The commands are copied, the script is not run.
3. **Fix `egui 0.31` in `CLAUDE.md:31` here?** Nothing to do: release 0.6.1 already replaced it with a pointer to `crates/designos-app/Cargo.toml`.
4. **Anatomy re-trace without the tool?** Redraw by hand. The `.webp`, the six SVG overlays and `anatomy.json` are recaptured as one set after L's geometry report; overlays are redrawn by hand from L's geometry JSON when the trace tool is still missing (controller ruling, plan.md C/M rulings, 2026-10-29 00:10). A new picture under old overlays would misalign, so the set never ships half-done.

## Open questions

1. **Moved to decision 4 (controller, plan.md C/M rulings, 2026-10-29 00:10).** The trace tool (`pipeline.py`, `emit.py`, `gate.py`, `source_check.py`, `finalize.py`, named in `plans/reports/agy-site-v2-060.md` §6.2) is in no tracked file at `8dfaee93` and was not found on this machine; that no longer blocks the anatomy set.
2. **Decided (controller, plan.md C-R): the og card keeps its composition.** Only its colours change (rule 11). A recomposition is the owner's call, through AskUserQuestion.
3. **Decided (controller default, plan.md 21:35 sweep): no new film in this spec.** The next pack's launch film follows the marketing rule.

**Changelog:**
- 2026-10-25 (round 2): Size revised to two to three days; site v2 grew the image list; re-anchored to `205c72ef`.
- 2026-10-28 (round 3, re-anchored to `8dfaee93`; review `reviews/MOP-engineering-review.md` M-1 to M-11 and the controller's rulings):
  - M-1: the five site-only captures have recorded commands. §2, rule 6, section 5, the Size, decision 2 and the non-goals are rewritten from `capture-visual-proof.sh` at `8dfaee93` and the tracked anatomy log; the script's reasons for not running it are stated; non-goal 5 (rebuild scenes from the old images) is dropped; the Size is revised down. The anatomy trace is named, ordered after L's geometry report, and its missing tool is an open question. Found on the way: three site `alt` texts (`:345`, `:358`, `:466`) disagree with their images, so the image wins and the `alt` is rewritten.
  - Check round (`reviews/CM-fix-check.md`, M-1 mismatch): open question 1 froze the anatomy set while §5, the Size and the Touches recaptured it. Ruled: decision 4, step 7 and open question 1 now say the set is redone together, by hand if the tool is missing.
  - M-2: the 0.6.0 method (hero without `--zoom`, common flags, hash-checked copy, retry helper, lossless `magick`) is inlined verbatim in section 5; rule 5 requires proof that the selection resolved (exit 0, stderr note, `id:<n>`). The helper keeps the recorded `caffeinate` method: `-u -d` for the whole session and a blocking `-u -t 4` before each run (`-t` is ignored when a command is given).
  - M-3: check 1 is a fenced block without table escapes; "fails before" lists the nine lines that really hit; check 1b covers the mapped values and the `rgba` triplets, including the three lines in the site's own assets.
  - M-4: rule 10 and section 5 give one row per mapping (old value, name, new value, ratio), cited to board 3 Q6 and R10.6; the three literals and three `rgba` triplets are mapped to the nearest Untitled neutral (controller default); check 5 asserts the values and adds `--ink` on `--text`, `--orange`, `--blue` and `--emerald`.
  - M-5: the kit is never opened by a path inside the repository: the original is read only, captures use an absolute path of a clone outside the work tree, and check 8 fails if a `.fig` or `.designos` file is staged, modified or untracked.
  - M-6: the og card keeps its composition and changes colours only (rule 11); `cover.png` is a copy of the new `designos-hero.png` (as the script does), not of the README hero.
  - M-7: README `:5`, `:8` and `:249` follow the build that took the captures; check 4b guards it.
  - M-8: every command passes `--library <scratch>`; the cause is `shell.rs:115-118` and `store.rs:467`; the PR carries a personal-data check (rule 4).
  - Also: M-9 (the recorded common flags are printed under each image), M-10 (selection confirmed), M-11 (`sips` instead of Pillow); citations re-anchored to `8dfaee93` (README `:197-274`, site `:319-466`, `:root` `:87-88`); the 0.7.0 `--select` exit code and the owner ruling of 2026-10-27 on document sources are applied.
