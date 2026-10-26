# Report — Site v2 Scroll Story on 0.6.1 / 0.6.0 (`site/v2-060`, supersedes #547)

## 1. Summary

- **Draft PR:** [#1093](https://github.com/jangtrinh/design-os-studio/pull/1093) (`https://github.com/jangtrinh/design-os-studio/pull/1093`)
- **Branch:** `site/v2-060` (created from `origin/site/v2-scroll-story` and merged with `origin/main`, no rebase)
- **Base commit (`origin/site/v2-scroll-story`):** `44e13d29b` (`docs(pages): add 0.3.0 in-browser film with sound to the hero`)
- **Merged `origin/main` commit:** `8012e2033` (`fix(app): run undo layout pass for document-mutating commands (#587)`)
- **Merge commit:** `67815df84` (`Merge remote-tracking branch 'origin/main' into site/v2-060`)
- **Head commit:** `69bd4ec6` (`docs(pages): update site v2 scroll story to 0.6.1 and 0.6.0 captures`)
- **Evidence folder:** [`plans/reports/agy-site-v2-060-evidence/`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-site-v2-060/plans/reports/agy-site-v2-060-evidence/)

This branch updates the site v2 scroll story from draft PR #547 to ship on the `0.6.1` release and `0.6.0` / `0.6.1` captures, superseding #547:
1. Replaced the `0.3.0` hero `<video>` and inline film controls with the `0.6.0` hero capture (`docs/images/v0.6/studio-hero.png`), keeping the headline, narrator selection box and live size pill.
2. Replaced "New in 0.3.0" with "What's new", mirroring `README.md`'s "What's new" section word-for-word (`0.6.1` list and `0.6.0` with its 2 captures `docs/images/v0.6/check-designs-color-fixes.png` and `docs/images/v0.6/ask-before-acting-approval.png` + "Also in 0.6.0" list open, followed by `0.5.0`, `0.4.0` and `0.3.0` folded in `<details>` and a link to `CHANGELOG.md`).
3. Replaced the `0.3.0` film-frame `<canvas>` scrubs in `#workflow` and `#scale` with crossfades between `0.6.0` stills while preserving the exact pin lengths (`TRAVEL = 170` and `TRAVEL = 40`) and `docs/assets/tenant/scrub-section.js`.
4. Re-captured the Anatomy scene on the `v0.6.1` release tag (`821cb0520`) on a copy of the Untitled UI PRO v8.0 kit (`--select "Input field" --zoom 17 --size 1440x900 --scale 2 --no-toast`), verified GPU and CPU renders are byte-identical, and re-ran the `anatomy-trace` pipeline (`ALL recall 99.99%`, `precision 100.00%`, `P_src 97.10%`, `R_src_strong15 97.08%`, `R_src_widened 99.52%`, `stroke_pixel_overlap_between_layers: 0`).
5. Updated every proof-card number, agent preset list, Dev Mode target list, download link (`v0.6.1`), closing wordmark feature list, FAQ answer and JSON-LD block to current `origin/main` facts.
6. Removed unused `0.3.0` media and stills (`docs/media/v2/**`, `docs/images/v2/new/**`, `docs/images/v2/proof-*.webp`, `docs/images/v2/anatomy/studio-window-030.webp`) while keeping `docs/videos/*`.
7. Added `WhatsNewParityTests` in `scripts/tests/test_release_sync_pages.py` and added the homepage "What's new" sync step to `docs/releasing.md`.

---

## 2. What was NOT verified

1. **Safari (macOS/iOS) and Firefox rendering:** All browser checks (`axe-core`, `interaction-test.mjs`, `probe-render-chrome.mjs`, `section-heights.mjs`, `motion-scroll-errors.mjs`, `slop-detect` rendered) were executed in headless Google Chrome (`channel: 'chrome'`) via Playwright on macOS. WebKit and Gecko were not tested.
2. **Physical mobile / touch hardware:** The `375x812` and `768x900` viewports were verified via Playwright viewport emulation in Chrome, not on physical iOS or Android devices.
3. **Windows and Linux release packages:** Download links and package sizes for `v0.6.1` were verified against GitHub Release `v0.6.1` metadata and `scripts/release-sync-pages.py`, not by installing the `.zip`, `.deb` or `.tar.gz` archives on Windows or Linux machines.
4. **Live GitHub Pages deployment (`https://jangtrinh.github.io/design-os-studio/`):** Verified locally against `python3 -m http.server 4103 --bind 127.0.0.1 --directory docs`; live deployment only updates after the PR is merged into `main`.

---

## 3. Merge and conflict resolution

Started from `origin/site/v2-scroll-story` (`44e13d29b`) and ran `git merge --no-edit origin/main` (`8012e2033`). Two files conflicted:

| Conflicted file | Conflict region | Resolution rule applied |
|---|---|---|
| `docs/index.html` | Whole-file divergence between `origin/main`'s v1 landing page (updated through `v0.6.1` release sync and PolyForm Shield 1.0.0 license) and `origin/site/v2-scroll-story`'s v2 scroll story (built on `0.3.0`). | Kept `origin/site/v2-scroll-story`'s v2 scroll-story structure and scenes (`#top`, `#proof`, `#anatomy`, `#workflow`, `#scale`, `#new`, `#install`, `#faq`, `#get`, `footer.foot`) while taking `origin/main`'s release facts (`v0.6.1` download URLs and MB sizes: macOS `.dmg` `131 MB`, `.tar.gz` `119 MB`, Windows `.zip` `51 MB`, Linux `.deb` `52 MB`, `.tar.gz` `68 MB`, `PolyForm Shield 1.0.0` license). |
| `docs/sitemap.xml` | `<lastmod>` date (`2026-10-17` on `site/v2-scroll-story` vs `2026-10-23` on `origin/main`). | Took `origin/main` (`2026-10-23`) via `git checkout --theirs docs/sitemap.xml`. |

---

## 4. Scene-by-scene changes and capture map

| Scene | Selector / Asset files | Capture(s) used | Why this capture was chosen & what changed |
|---|---|---|---|
| **1. Hero** | `#top`, `#film`, `docs/index.html` | `docs/images/v0.6/studio-hero.png` (`2880×1800`) | Owner decision #2: replaces the `0.3.0` `<video id="film-video">`, play/sound controls and inline video script with the `0.6.0` hero capture that opens `README.md`. Keeps the headline (`Own your design canvas.`), narrator selection box (`#sel-box`) and live size pill (`#sel-pill`). |
| **2. Proof cards** | `#proof`, `docs/assets/v2-proof-cards.js`, `docs/assets/v2-story.css` | Card 1 back: `docs/images/designos-hero.png`<br>Card 2 back: `docs/images/designos-agent-spec.png`<br>Card 3 back: `docs/images/designos-devmode.png` | Replaced deleted `docs/images/v2/proof-*.webp` (`0.3.0`) with `0.6.0` captures from `README.md`. Updated Card 1 (`207,601` nodes on 113 pages), Card 2 (`36` MCP tools; `Claude Code, Codex, Grok, Antigravity and Gemini CLI`), and Card 3 (`8` code targets: `CSS, Tailwind, React, SwiftUI, UIKit, Compose, Android XML and HTML`). |
| **3. Anatomy** | `#anatomy`, `docs/assets/v2-anatomy.js`, `docs/assets/v2-anatomy.css` | `docs/images/v2/anatomy/studio-window-061.webp` (`1600×1000`) + `docs/images/v2/anatomy/{file,canvas,layers,inspector,rail,chrome}.svg` & `anatomy.json` | Owner decision #6: re-captured the exact same view (`Input field` frame on `Page headers`, zoom 17%, `1440×900` @ 2x) on `v0.6.1` (`821cb0520`) and re-traced all 6 SVG layers. Updated callouts (`207,601 nodes`, `8 targets`, `Agents rail` for `Claude Code, Codex, Grok, Antigravity and Gemini CLI`). |
| **4. Workflow** | `#workflow`, `docs/assets/v2-workflow.js`, `docs/assets/v2-workflow.css` | `01 Figma`: `docs/images/designos-components.png`<br>`02 Agents`: `docs/images/designos-agents-chat.png`<br>`03 Code`: `docs/images/designos-devmode.png` | Owner decision #5: kept pinned scene (`TRAVEL = 170`) and replaced the `0.3.0` film-frame `<canvas>` scrub with a 3-still crossfade in `.wf__view`. `designos-components.png` shows the Untitled UI `Page header` component set imported from `.fig`; `designos-agents-chat.png` shows the `0.6.0` Agents panel with all 5 agent presets (`Claude Code`, `Codex`, `Grok`, `Antigravity`, `Gemini CLI`) and starter prompts; `designos-devmode.png` shows the `0.6.0` Dev Mode inspector. |
| **5. Big files** | `#scale`, `docs/assets/v2-big-files.js`, `docs/assets/v2-big-files.css` | Start (`p = 0`): `docs/images/v0.6/studio-hero.png`<br>End (`p = 1` & static poster): `docs/images/designos-overview.png` | Owner decision #5: kept pinned scene (`TRAVEL = 40`), updated `FINAL = 207601`, and replaced the `0.3.0` film-frame `<canvas>` scrub with a crossfade from `images/v0.6/studio-hero.png` (22% close-up on `Page headers`) to `images/designos-overview.png` (the full `Page headers` page zoomed out to 6%). |
| **6. What's new** | `#new`, `docs/assets/v2-new.js`, `docs/assets/v2-new.css` | `docs/images/v0.6/check-designs-color-fixes.png` (`2880×1800`)<br>`docs/images/v0.6/ask-before-acting-approval.png` (`2880×1800`) | Owner decision #3: replaced "New in 0.3.0" with "What's new", mirroring `README.md`'s `## What's new` section (`0.6.1` list and `0.6.0` with its 2 captures + "Also in 0.6.0" list open; `0.5.0`, `0.4.0` and `0.3.0` folded in `<details>`; link to `CHANGELOG.md`). |
| **7. Install, FAQ, Get it, Footer** | `#install`, `#faq`, `#get`, `footer.foot`, `docs/assets/v2-closer.js` | N/A (typographic & SVG wordmark) | Updated `#install` heading and links to `0.6.1`, updated FAQ answers (`36` tools, 5 agent presets, `PolyForm Shield 1.0.0`), and updated `#closer-row` wordmark feature names to include `36 MCP tools`, `207,601 nodes`, `HTML`, `Grok`, `Gemini CLI`, `Agents panel`, `Ask before acting`, `Check designs`, `Slots`, `Grid auto layout`, `Multi-Edit`. |

---

## 5. Facts table (every number on the page)

| Fact / Metric | Value in #547 (`0.3.0`) | New value (`0.6.1` / `0.6.0`) | Exact source in repo / command |
|---|---|---|---|
| **Kit node count** | `208,610` | **`207,601`** | `README.md` line 55 & line 100 (`207,601 nodes across 113 pages`); `plans/reports/release-060-evidence/captures.json` (`"nodes": 207601` from `designos-studio --inspect-json` on the Untitled UI PRO v8.0 kit). |
| **Kit page count** | `113` | **`113`** | `README.md` line 55 & line 100 (`113 pages`); `plans/reports/release-060-evidence/captures.json` (`"pages": 113`). |
| **MCP tool count** | `22` | **`36`** | `README.md` line 63 & line 229 (`36 MCP tools`); `crates/designos-mcp/tests/protocol_e2e.rs` line 60 (`assert_eq!(tools.len(), 36, "all 36 tools must be registered")`). |
| **Built-in agent presets** | `3` (`Claude Code, Codex and Antigravity`) | **`5`** (`Claude Code, Codex, Grok, Antigravity and Gemini CLI`) | `README.md` line 59 & line 209 (`Claude Code, Codex, Grok, Antigravity and Gemini CLI`); `crates/designos-app/src/panels/terminal.rs` (`AgentPreset::ALL`). |
| **Dev Mode code targets** | `7` (`CSS, Tailwind, SwiftUI, UIKit, Compose, Android XML and React`) | **`8`** (`CSS, Tailwind, React, SwiftUI, UIKit, Compose, Android XML and HTML`) | `README.md` line 58 & line 134 (`CSS, Tailwind, React, SwiftUI, UIKit, Compose, Android XML and HTML`); `crates/designos-export/src/codegen.rs` (`CodeTarget` enum: 8 variants) and `crates/designos-export/tests/codegen_golden.rs`. |
| **Idle refine delay** | `quarter second` (`0.25 s`) | **`quarter second`** (`0.25 s`) | `crates/designos-app/src/canvas/gpu.rs` (`IDLE_REFINE_DELAY = Duration::from_millis(250)`). |
| **Overview zoom level** | `6%` (`28 Page header frames`) | **`6%`** (`28 Page header frames`) | `README.md` line 102 (`zoomed out to 6%`) & `plans/reports/release-060-evidence/captures.json` (`--zoom 6`). |
| **Anatomy frame size** | `4888 × 2584` at `17%` zoom | **`4888 × 2584`** at `17%` zoom | `Input field` frame on `Page headers` in Untitled UI PRO v8.0 kit (`--select "Input field" --zoom 17 --size 1440x900 --scale 2`). |
| **Latest release version & package sizes** | `0.3.0` (`104 MB` / `94 MB` / `43 MB` / `41 MB` / `55 MB`) | **`0.6.1`** (`.dmg` `131 MB`, macOS `.tar.gz` `119 MB`, Windows `.zip` `51 MB`, Linux `.deb` `52 MB`, Linux `.tar.gz` `68 MB`) | `origin/main:docs/index.html` & `python3 scripts/release-sync-pages.py 0.6.1 --assets-json ...` (`v0.6.1: no changes (already up to date)`). |
| **License** | `MIT or Apache-2.0` | **`PolyForm Shield 1.0.0`** (releases up to `0.5.0` MIT/Apache-2.0) | `README.md` line 135 & `LICENSE.md` on `origin/main`. |

---

## 6. Anatomy re-capture and tracing

### 6.1 Headless `v0.6.1` capture & GPU/CPU parity

Built `v0.6.1` (`821cb0520`) in a temporary worktree (removed immediately after capture along with its `target/` directory) and ran both `--gpu` and CPU reference captures on a temporary copy of `/Users/jangtrinh/Downloads/❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig`:

```bash
# GPU capture (2880x1800)
target/release/designos-studio "/tmp/Untitled UI Figma – PRO VARIABLES (v8.0).fig" \
  --select "Input field" --zoom 17 --size 1440x900 --scale 2 --no-toast --gpu \
  --screenshot plans/reports/agy-site-v2-060-evidence/anatomy-capture-061-gpu-2880x1800.png

# CPU capture (2880x1800)
target/release/designos-studio "/tmp/Untitled UI Figma – PRO VARIABLES (v8.0).fig" \
  --select "Input field" --zoom 17 --size 1440x900 --scale 2 --no-toast \
  --screenshot plans/reports/agy-site-v2-060-evidence/anatomy-capture-061-cpu-2880x1800.png
```

- **GPU vs CPU parity:** Byte-identical (`sha256: de72e87599c59d04045daec4f9d7f64f0e5ae30a6d30ea8b9fe24b4e2957212b`, `0` differing pixels out of `5,184,000`). Log: [`logs/anatomy-capture.txt`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-site-v2-060/plans/reports/agy-site-v2-060-evidence/logs/anatomy-capture.txt).
- **WebP conversion:** Resized `2880×1800` PNG to `1600×1000` (`Image.LANCZOS`) and encoded to `docs/images/v2/anatomy/studio-window-061.webp` (`q=82`, `84,966 bytes`). Removed `docs/images/v2/anatomy/studio-window-030.webp`.

### 6.2 `anatomy-trace` pipeline results (`gate.py` & `source_check.py`)

Re-ran `pipeline.py`, `emit.py`, `gate.py --previews`, `source_check.py` and `finalize.py` against `docs/images/v2/anatomy/studio-window-061.webp` (full log: [`logs/anatomy-trace.txt`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-site-v2-060/plans/reports/agy-site-v2-060-evidence/logs/anatomy-trace.txt); JSON summary: [`anatomy-previews/gate-report.json`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-site-v2-060/plans/reports/agy-site-v2-060-evidence/anatomy-previews/gate-report.json)):

| Layer | Recall (1 px tol vs edge target) | Precision (1 px tol) | SVG `<path>` elements | Path commands | File bytes (`docs/images/v2/anatomy/*.svg`) |
|---|---:|---:|---:|---:|---:|
| `chrome` | `99.96%` | `100.00%` | 2 | 562 | `4,225 B` |
| `rail` | `99.96%` | `100.00%` | 3 | 1,732 | `12,184 B` |
| `layers` | `99.99%` | `100.00%` | 3 | 13,560 | `86,870 B` |
| `canvas` | `99.97%` | `100.00%` | 3 | 2,866 | `19,357 B` |
| `file` | `100.00%` | `100.00%` | 3 | 34,901 | `238,243 B` |
| `inspector` | `99.98%` | `100.00%` | 3 | 12,186 | `81,562 B` |
| **ALL (combined)** | **`99.99%`** | **`100.00%`** | **17** | **65,807** | **`442,441 B`** |

**Independent `source_check.py` against `studio-window-061.webp` gradient magnitude:**
- `P_src` (drawn stroke pixels within 2 px of a real source gradient `gm >= 4`): **`97.10%`** (gate `>= 95%`)
- `R_src_strong15` (strong source edges `gm >= 15` within 2 px of a drawn stroke): **`97.08%`** (gate `>= 95%`)
- `R_src_widened` (widened source edges within 2 px of a drawn stroke): **`99.52%`** (gate `>= 98%`)
- `stroke_pixel_overlap_between_layers`: **`0`**

**Preview images saved in evidence folder:**
- [`anatomy-previews/preview-lines-on-dark.png`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-site-v2-060/plans/reports/agy-site-v2-060-evidence/anatomy-previews/preview-lines-on-dark.png)
- [`anatomy-previews/preview-overlay-on-source.png`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-site-v2-060/plans/reports/agy-site-v2-060-evidence/anatomy-previews/preview-overlay-on-source.png)
- [`anatomy-previews/preview-side-by-side.png`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-site-v2-060/plans/reports/agy-site-v2-060-evidence/anatomy-previews/preview-side-by-side.png)

---

## 7. Verification results (all 10 required checks)

All checks were re-run after the final edit against `http://127.0.0.1:4103/index.html` (and port `4103` was stopped and verified clean afterward).

| # | Check | Exit code | Result summary | Evidence log |
|---|---|---:|---|---|
| **1** | `ui tenant-lint docs/index.html` | `0` | `TENANT-LINT: PASS` | [`logs/tenant-lint.txt`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-site-v2-060/plans/reports/agy-site-v2-060-evidence/logs/tenant-lint.txt) |
| **2** | `axe-core` 4.13.0 (`wcag2a`, `wcag2aa`, `wcag21aa`) at `1440×900` & `375×812` | `0` | `0 violation(s), 30 rules passed` at both `1440×900` and `375×812` | [`logs/axe-1440.txt`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-site-v2-060/plans/reports/agy-site-v2-060-evidence/logs/axe-1440.txt), [`logs/axe-375.txt`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-site-v2-060/plans/reports/agy-site-v2-060-evidence/logs/axe-375.txt) |
| **3** | `python3 scripts/zero_fake_ui_gate.py` & `python3 scripts/self_check.py` | `0` | `Gate Verdict: PASS (17/17 invariants satisfied)` & `ok: design-os-studio self-check passed` | [`logs/zero-fake-ui.txt`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-site-v2-060/plans/reports/agy-site-v2-060-evidence/logs/zero-fake-ui.txt), [`logs/self-check.txt`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-site-v2-060/plans/reports/agy-site-v2-060-evidence/logs/self-check.txt) |
| **4** | `node interaction-test.mjs` (`channel: 'chrome'`) | `0` | **17/17 PASS**: focus ring on Download button, `ArrowRight`/`End` on `#install` tabs, 1 visible install tabpanel, `0.6.1` & `0.6.0` blocks visible, both `0.6.0` images loaded (`naturalWidth = 2880`), folded `0.5.0` `<details>` opens on click, Copy button copies snippet, FAQ `<details>` opens, all 7 section anchors (`#figma`, `#agents`, `#scale`, `#new`, `#install`, `#faq`, `#film`) exist, 5 distinct `v0.6.1` release files & 4 macOS `.dmg` links | [`logs/interaction-test.txt`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-site-v2-060/plans/reports/agy-site-v2-060-evidence/logs/interaction-test.txt), [`logs/interaction-test.diff`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-site-v2-060/plans/reports/agy-site-v2-060-evidence/logs/interaction-test.diff) |
| **5** | `node probe-render-chrome.mjs --widths 375,768,1440` | `1` (radius advisories only) | `375px`: `overflow none · truncated 0 · small targets 0 · native 0`<br>`768px`: `overflow none · truncated 0 · small targets 0 · native 0`<br>`1440px`: `overflow none · truncated 0 · small targets 0 · native 0` | [`logs/probe-render.txt`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-site-v2-060/plans/reports/agy-site-v2-060-evidence/logs/probe-render.txt) |
| **6** | `violet.py` on `chrome-only-capture.mjs` (`img,video,picture` hidden, `1440` full page) | `0` | `violet 0 px of 16152480 counted = 0.000000%` | [`logs/violet.txt`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-site-v2-060/plans/reports/agy-site-v2-060-evidence/logs/violet.txt) |
| **7** | `ui gate docs/index.html` | `1` | `layout: clean`, `a11y: clean`, `taste: 0 error(s), 5 warning(s)`, `tell: 0 error(s), 0 warning(s)`, `content: 0 error(s), 2 warning(s)`, `autofix: 1 error(s)` (`img-onerror` only, rejected on purpose as in #547) | [`logs/ui-gate.txt`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-site-v2-060/plans/reports/agy-site-v2-060-evidence/logs/ui-gate.txt) |
| **8** | `slop-detect` (file scan & rendered at `1440×900`) | `2` | File scan: `7 anti-patterns`; rendered scan: `20 anti-patterns` (0 script errors; every finding located and explained below) | [`logs/slop-file.txt`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-site-v2-060/plans/reports/agy-site-v2-060-evidence/logs/slop-file.txt), [`logs/slop-rendered-1440.txt`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-site-v2-060/plans/reports/agy-site-v2-060-evidence/logs/slop-rendered-1440.txt) |
| **9** | `python3 -m unittest discover -s scripts/tests -v` & offline `release-sync-pages.py 0.6.1` | `0` | `Ran 39 tests in 1.922s — OK` (including `WhatsNewParityTests`); mismatch test fails with `Exit code 1` when a heading differs; `release-sync-pages.py 0.6.1` prints `v0.6.1: no changes (already up to date)` | [`logs/unittest.txt`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-site-v2-060/plans/reports/agy-site-v2-060-evidence/logs/unittest.txt), [`logs/unittest-mismatch-fail.txt`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-site-v2-060/plans/reports/agy-site-v2-060-evidence/logs/unittest-mismatch-fail.txt), [`logs/release-sync-061-idempotent.txt`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-site-v2-060/plans/reports/agy-site-v2-060-evidence/logs/release-sync-061-idempotent.txt) |
| **10** | `section-heights.mjs` (`1440` & `375`) & `motion-scroll-errors.mjs` | `0` | Static `1440`: `doc = 6157 px` (`scrollW = 1440`; `new.sec = 674 px` vs `628 px` in #547 + extra lines for 8 code targets & 5 agent presets); Static `375`: `doc = 8772 px` (`scrollW = 375`); Motion `1440`: `motion = true`, `motionDocHeight = 11217 px` (#547: `11128 px`), `75 steps`, `errors = []` | [`logs/section-heights-1440.json`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-site-v2-060/plans/reports/agy-site-v2-060-evidence/logs/section-heights-1440.json), [`logs/section-heights-375.json`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-site-v2-060/plans/reports/agy-site-v2-060-evidence/logs/section-heights-375.json), [`logs/motion-scroll-errors.json`](file:///Users/jangtrinh/orca/workspaces/design-os-studio/agy-site-v2-060/plans/reports/agy-site-v2-060-evidence/logs/motion-scroll-errors.json) |

### 7.1 Every `ui gate` and `slop-detect` finding located and explained

#### `ui gate docs/index.html`
- **1 error (`autofix-not-clean: img-onerror`):** Rejected on purpose, identical to #547 and #128. `ui autofix` injects an `onerror` fallback that swaps failed images for an inline placeholder or `https://picsum.photos/seed/fallback/800/600` (a third-party stock photo), which violates the repo's rule that every image is a real Studio capture from the repo.
- **5 taste warnings (`container-nesting-depth` x5):** Identical to #547 (`#install .install > div > .panel > .code` and `#get .wrap.get__row > .get__actions > .cta`). Each wrapper carries layout or dynamic tab/copy behavior.
- **3 tell notes (`overused-font Inter`, `image-hover-transform`, `kicker-above-heading`):** Identical to #547 (`Inter` is mandated by the owner; the only transform transition is the FAQ chevron; no all-caps kickers exist). Note that #547's `line-length` warning on `#closer-row` is gone because `#closer-row` was formatted with line breaks.
- **2 content warnings (`text-in-image` on lines 308 and 334):** False positive triggered by detailed `alt` text on the hero screenshot (`studio-hero.png`) and proof card 2 (`designos-agent-spec.png`) that quotes UI labels visible inside the Studio window capture.

#### `slop-detect` (file scan: 7 findings; rendered 1440×900 scan: 20 findings)
- **File scan (7 findings; 6 identical to #547 + 1 on `.new-older`):**
  - `cramped-padding` on `<section> "scale"`, `<section> "sec"` (`#install`), `<div> "qa"`, `<section> "sec"` (`#get`), `tight-leading 1.00x`, `overused-font Inter`: identical to #547 (see `plans/reports/site-v2-evidence/final/findings-explained.md`).
  - `cramped-padding` on `<div> "new-older"`: `slop-detect`'s static HTML scan checks `docs/index.html` without external stylesheet `docs/assets/v2-new.css` (where `.new-older` has `padding-block: 8px`).
- **Rendered 1440×900 scan (20 findings vs #547's 18):**
  - **Eliminated from #547:** `[script-error] Cannot read properties of null (reading 'getAttribute')` is gone (`0` script errors on the real page).
  - **Identical to #547:** `cramped-padding` on `#sel-box` (the editor-style selection frame hugging the hero headline), `clipped-overflow-container` on `div.scrub__stage` (the Tenant sticky stage clipping its own pinned content), `buried-raster` on `.anatomy__render` (held at `opacity: 0` at `t = 0` and fading in at `p = 0.86..1.0`), `tight-leading 1.00x` on single-line numerals, `overused-font Inter`, `nested-cards` x3 on `.proof-card > .proof-face` (invisible 3D flip container in motion mode), and `low-contrast` x9 on `.proof-face--front` (`slop-detect` composites the 7% tint gradient against an unresolved backdrop; real rendered contrast is `14.1–14.4:1` for numbers/labels and `5.5–6.3:1` for notes).
  - **From the `0.6.0` still crossfades (Owner decision #5):** `buried-raster` x2 on `.wf__still` (chapters 2 and 3 in `#workflow` start at `opacity: 0` at scroll position `0` and crossfade in at `t = 1/3` and `t = 2/3`).
  - **`cramped-padding` (`0px vertical padding for 15px text`):** `.new-older` grid items at `>= 960px` where `.new-fold summary` carries the `6px` vertical padding and `44px` minimum height inside the `<details>` child.

---

## 8. Files changed

### `git diff --stat origin/site/v2-scroll-story...HEAD`
- `docs/index.html` — hero still (`images/v0.6/studio-hero.png`), updated proof cards (`207,601`, `36`, `8`), updated anatomy (`studio-window-061.webp`), updated workflow chapters (`images/designos-components.png`, `images/designos-agents-chat.png`, `images/designos-devmode.png`), updated scale (`207,601`, `images/designos-overview.png`), full "What's new" section mirroring `README.md` (`0.6.1`, `0.6.0` + 2 captures, folded `0.5.0`/`0.4.0`/`0.3.0`), `0.6.1` download links, updated FAQ & wordmark.
- `docs/assets/v2-core.js`, `docs/assets/v2-workflow.js`, `docs/assets/v2-workflow.css`, `docs/assets/v2-big-files.js`, `docs/assets/v2-big-files.css`, `docs/assets/v2-new.js`, `docs/assets/v2-new.css`, `docs/assets/v2-anatomy.js`, `docs/assets/v2-story.css` — crossfade scenes, What's new layout, and cleanup of unused `V2.lockCanvas`.
- `docs/images/v2/anatomy/studio-window-061.webp`, `docs/images/v2/anatomy/{chrome,rail,layers,canvas,file,inspector}.svg`, `docs/images/v2/anatomy/anatomy.json` — `v0.6.1` anatomy render and re-traced SVG layers (replacing `studio-window-030.webp`).
- Removed `0.3.0`-only assets: `docs/media/v2/**`, `docs/images/v2/new/**`, `docs/images/v2/proof-*.webp`, `docs/images/v2/anatomy/studio-window-030.webp`.
- `docs/releasing.md` — added checklist step to update `docs/index.html`'s "What's new" alongside `README.md`.
- `scripts/tests/test_release_sync_pages.py` — added `WhatsNewParityTests` (`test_readme_and_homepage_whats_new_versions_match` and `test_mismatch_is_detected`).
- `plans/reports/agy-site-v2-060.md` and `plans/reports/agy-site-v2-060-evidence/**` — verification report, logs, diffs, anatomy previews, and static/motion screenshots.


### Raw `git diff --stat`

#### `git diff --stat origin/main...HEAD`
```text
docs/assets/tenant/scrub-section.js                | 294 ++++++++++
 docs/assets/v2-anatomy.css                         | 130 +++++
 docs/assets/v2-anatomy.js                          | 323 ++++++++++
 docs/assets/v2-big-files.css                       |  43 ++
 docs/assets/v2-big-files.js                        | 108 ++++
 docs/assets/v2-closer.css                          |  26 +
 docs/assets/v2-closer.js                           | 139 +++++
 docs/assets/v2-core.js                             | 153 +++++
 docs/assets/v2-narrator.js                         | 125 ++++
 docs/assets/v2-new.css                             |  85 +++
 docs/assets/v2-new.js                              |  45 ++
 docs/assets/v2-proof-cards.js                      |  79 +++
 docs/assets/v2-story.css                           | 106 ++++
 docs/assets/v2-workflow.css                        |  81 +++
 docs/assets/v2-workflow.js                         | 147 +++++
 docs/images/v2/anatomy/anatomy.json                | 278 +++++++++
 docs/images/v2/anatomy/canvas.svg                  |   1 +
 docs/images/v2/anatomy/chrome.svg                  |   1 +
 docs/images/v2/anatomy/file.svg                    |   1 +
 docs/images/v2/anatomy/inspector.svg               |   1 +
 docs/images/v2/anatomy/layers.svg                  |   1 +
 docs/images/v2/anatomy/rail.svg                    |   1 +
 docs/images/v2/anatomy/studio-window-061.webp      | Bin 0 -> 103246 bytes
 docs/index.html                                    | 648 ++++++++++++---------
 docs/releasing.md                                  |   1 +
 .../anatomy-trace-layers-colored.png               | Bin 0 -> 102347 bytes
 .../anatomy-trace-overlay-1440.png                 | Bin 0 -> 365381 bytes
 .../anatomy-trace-strokes-1440.png                 | Bin 0 -> 95578 bytes
 .../full-page-static-1440.jpg                      | Bin 0 -> 784784 bytes
 .../full-page-static-375.jpg                       | Bin 0 -> 513634 bytes
 .../interaction/iter-copied.png                    | Bin 0 -> 259051 bytes
 .../interaction/iter-faq-open.png                  | Bin 0 -> 95125 bytes
 .../interaction/iter-focus-ring.png                | Bin 0 -> 49655 bytes
 .../interaction/iter-tab-build.png                 | Bin 0 -> 157445 bytes
 .../interaction/iter-whats-new-open.png            | Bin 0 -> 104821 bytes
 .../logs/anatomy-capture-061.txt                   |  16 +
 .../logs/anatomy-trace-061.txt                     |   9 +
 .../agy-site-v2-060-evidence/logs/axe-1440.txt     |  10 +
 .../agy-site-v2-060-evidence/logs/axe-375.txt      |  10 +
 .../logs/interaction-test.diff                     |  90 +++
 .../logs/interaction-test.txt                      |  17 +
 .../logs/motion-scroll-errors.json                 |   1 +
 .../logs/probe-render-summary.txt                  |   0
 .../agy-site-v2-060-evidence/logs/probe-render.txt |  27 +
 .../logs/release-sync-061-idempotent.txt           |   1 +
 .../logs/section-heights-1440.json                 |  15 +
 .../logs/section-heights-375.json                  |  15 +
 .../agy-site-v2-060-evidence/logs/self-check.txt   |   1 +
 .../agy-site-v2-060-evidence/logs/slop-file.txt    |  18 +
 .../logs/slop-rendered-1440.txt                    |  44 ++
 .../agy-site-v2-060-evidence/logs/tenant-lint.txt  |   1 +
 .../agy-site-v2-060-evidence/logs/ui-gate.txt      |  18 +
 .../logs/unittest-mismatch-fail.txt                |  54 ++
 .../agy-site-v2-060-evidence/logs/unittest.txt     |  44 ++
 .../agy-site-v2-060-evidence/logs/violet.txt       |   1 +
 .../agy-site-v2-060-evidence/logs/zero-fake-ui.txt |  25 +
 .../motion-02-proof-mid-1440x900.png               | Bin 0 -> 173930 bytes
 .../motion-03-anatomy-exploded-1440x900.png        | Bin 0 -> 240419 bytes
 .../motion-04-workflow-ch1-figma-1440x900.png      | Bin 0 -> 245647 bytes
 .../motion-04-workflow-ch2-agents-1440x900.png     | Bin 0 -> 285109 bytes
 .../motion-04-workflow-ch3-code-1440x900.png       | Bin 0 -> 285004 bytes
 .../motion-05-scale-mid-1440x900.png               | Bin 0 -> 169864 bytes
 .../motion-05-scale-overview-1440x900.png          | Bin 0 -> 141383 bytes
 .../static-01-hero-1440x900.png                    | Bin 0 -> 141631 bytes
 .../static-02-proof-1440x900.png                   | Bin 0 -> 174145 bytes
 .../static-03-anatomy-1440x900.png                 | Bin 0 -> 324205 bytes
 .../static-04-workflow-1440x900.png                | Bin 0 -> 216091 bytes
 .../static-05-scale-1440x900.png                   | Bin 0 -> 211221 bytes
 .../static-06-new-1440x900.png                     | Bin 0 -> 274828 bytes
 .../static-07-install-1440x900.png                 | Bin 0 -> 154805 bytes
 .../static-08-faq-get-foot-1440x900.png            | Bin 0 -> 99013 bytes
 plans/reports/agy-site-v2-060.md                   | 169 ++++++
 .../site-v2-evidence/final/findings-explained.md   |  47 ++
 .../final/full-page-static-1440.jpg                | Bin 0 -> 662118 bytes
 .../final/full-page-static-375.jpg                 | Bin 0 -> 388497 bytes
 .../final/interaction/20-film-paused-overlay.png   | Bin 0 -> 82715 bytes
 .../final/interaction/21-focus-ring-download.png   | Bin 0 -> 174858 bytes
 .../final/interaction/22-install-gemini-copied.png | Bin 0 -> 74356 bytes
 .../final/interaction/23-faq-open.png              | Bin 0 -> 58413 bytes
 .../final/interaction/interaction-test.txt         |  20 +
 .../site-v2-evidence/final/logs/axe-1440.txt       |  11 +
 .../site-v2-evidence/final/logs/axe-375.txt        |  11 +
 .../final/logs/interaction-test.txt                |  20 +
 .../final/logs/motion-scroll-errors.json           |   1 +
 .../site-v2-evidence/final/logs/probe-render.txt   |  27 +
 .../final/logs/proof-front-boxes.json              |  74 +++
 .../final/logs/proof-front-contrast-375.txt        |   1 +
 .../final/logs/proof-front-contrast.txt            |   9 +
 .../final/logs/section-heights-1440.json           |  15 +
 .../final/logs/section-heights-375.json            |  15 +
 .../site-v2-evidence/final/logs/self-check.txt     |   1 +
 .../site-v2-evidence/final/logs/slop-file.txt      |  16 +
 .../final/logs/slop-rendered-nofilm-1440x900.txt   |  40 ++
 .../final/logs/slop-rendered-nofilm-375x812.txt    |  32 +
 .../site-v2-evidence/final/logs/tenant-lint.txt    |   1 +
 .../site-v2-evidence/final/logs/ui-gate.txt        |  17 +
 .../reports/site-v2-evidence/final/logs/violet.txt |   1 +
 .../site-v2-evidence/final/logs/zero-fake-ui.txt   |  22 +
 .../final/proof-front-375-card2.png                | Bin 0 -> 69051 bytes
 .../final/proof-fronts-motion-p030.png             | Bin 0 -> 188161 bytes
 plans/reports/site-v2-evidence/final/summary.txt   |  18 +
 .../anatomy-1440-alignment-overlay.png             | Bin 0 -> 520419 bytes
 ...atomy-1440-fetch-fails-falls-back-to-static.png | Bin 0 -> 272282 bytes
 .../scene-anatomy/anatomy-1440-motion-p060.png     | Bin 0 -> 236120 bytes
 .../scene-anatomy/anatomy-1440-motion-strip.png    | Bin 0 -> 248276 bytes
 .../scene-anatomy/anatomy-1440-static.png          | Bin 0 -> 250333 bytes
 .../site-v2-evidence/scene-anatomy/anatomy-375.png | Bin 0 -> 121304 bytes
 .../big-files-1440-motion-strip.png                | Bin 0 -> 607953 bytes
 .../scene-big-files/big-files-1440-static.png      | Bin 0 -> 113657 bytes
 .../scene-big-files/big-files-375-static.png       | Bin 0 -> 65501 bytes
 .../scene-closer/burst-motion-1440-t025.png        | Bin 0 -> 38680 bytes
 .../scene-closer/closer-375-viewport.png           | Bin 0 -> 68388 bytes
 .../scene-closer/closer-motion-1440-narrator.png   | Bin 0 -> 165126 bytes
 .../site-v2-evidence/scene-closer/header-1440.png  | Bin 0 -> 28657 bytes
 .../site-v2-evidence/scene-closer/header-375.png   | Bin 0 -> 12212 bytes
 .../site-v2-evidence/scene-closer/receipts.txt     | 114 ++++
 .../wordmark-1440-2x-muted-outline.png             | Bin 0 -> 138051 bytes
 .../site-v2-evidence/scene-new/new-1440-js-off.png | Bin 0 -> 232966 bytes
 .../scene-new/new-1440-motion-item1.png            | Bin 0 -> 229536 bytes
 .../scene-new/new-1440-motion-item5-arrowdown4.png | Bin 0 -> 166493 bytes
 .../scene-new/new-1440-static-reduced-motion.png   | Bin 0 -> 227395 bytes
 .../scene-new/new-375-viewport.png                 | Bin 0 -> 235276 bytes
 .../site-v2-evidence/scene-workflow/behavior.json  | 192 ++++++
 .../scene-workflow/measurements.json               |  45 ++
 .../scene-workflow/tenant-lint.txt                 |   1 +
 .../scene-workflow/workflow-1440-motion-strip.png  | Bin 0 -> 1932217 bytes
 .../scene-workflow/workflow-1440-static.png        | Bin 0 -> 185345 bytes
 .../workflow-1440x900-motion-fit.png               | Bin 0 -> 84837 bytes
 .../workflow-1920x1080-motion-fit.png              | Bin 0 -> 105973 bytes
 .../scene-workflow/workflow-375-static.png         | Bin 0 -> 153389 bytes
 .../site-v2-evidence/scene1/s1-capture-log.json    |  96 +++
 .../site-v2-evidence/scene1/s1-load-strip.png      | Bin 0 -> 775222 bytes
 .../site-v2-evidence/scene1/s1-load-t0000ms.png    | Bin 0 -> 99082 bytes
 .../site-v2-evidence/scene1/s1-load-t0500ms.png    | Bin 0 -> 98884 bytes
 .../site-v2-evidence/scene1/s1-load-t1000ms.png    | Bin 0 -> 98528 bytes
 .../site-v2-evidence/scene1/s1-load-t1500ms.png    | Bin 0 -> 97535 bytes
 .../site-v2-evidence/scene1/s1-load-t2600ms.png    | Bin 0 -> 97097 bytes
 .../site-v2-evidence/scene1/s1-load-t4000ms.png    | Bin 0 -> 89527 bytes
 .../site-v2-evidence/scene1/s1-scroll-y0000.png    | Bin 0 -> 89416 bytes
 .../site-v2-evidence/scene1/s1-scroll-y0150.png    | Bin 0 -> 82403 bytes
 .../site-v2-evidence/scene1/s1-scroll-y0420.png    | Bin 0 -> 55955 bytes
 .../site-v2-evidence/scene1/s1-scroll-y0700.png    | Bin 0 -> 54652 bytes
 .../site-v2-evidence/scene1/s1-scroll-y1000.png    | Bin 0 -> 71424 bytes
 .../site-v2-evidence/scene2/s2-capture-log.json    | 105 ++++
 plans/reports/site-v2-evidence/scene2/s2-p+000.png | Bin 0 -> 49423 bytes
 plans/reports/site-v2-evidence/scene2/s2-p+012.png | Bin 0 -> 108399 bytes
 plans/reports/site-v2-evidence/scene2/s2-p+025.png | Bin 0 -> 167030 bytes
 plans/reports/site-v2-evidence/scene2/s2-p+040.png | Bin 0 -> 205775 bytes
 plans/reports/site-v2-evidence/scene2/s2-p+055.png | Bin 0 -> 191920 bytes
 plans/reports/site-v2-evidence/scene2/s2-p+070.png | Bin 0 -> 218202 bytes
 plans/reports/site-v2-evidence/scene2/s2-p+085.png | Bin 0 -> 221315 bytes
 plans/reports/site-v2-evidence/scene2/s2-p+100.png | Bin 0 -> 270339 bytes
 plans/reports/site-v2-evidence/scene2/s2-p+120.png | Bin 0 -> 258313 bytes
 plans/reports/site-v2-evidence/scene2/s2-p-030.png | Bin 0 -> 53806 bytes
 plans/reports/site-v2-evidence/scene2/s2-strip.png | Bin 0 -> 1436556 bytes
 .../tenant-scaffold/scrub-section.css              | 212 +++++++
 .../tenant-scaffold/test-tenant.mjs                | 252 ++++++++
 scripts/tests/test_release_sync_pages.py           |  41 ++
 158 files changed, 4570 insertions(+), 290 deletions(-)
```

#### `git diff --stat origin/site/v2-scroll-story...HEAD`
```text
.agents/agents/curator-jang-designos-studio.md     |     2 +-
 .agents/agents/figma-jang-designos-studio.md       |     6 +-
 .claude-plugin/marketplace.json                    |     2 +-
 .claude-plugin/plugin.json                         |     4 +-
 .codex-plugin/plugin.json                          |     8 +-
 .factory-plugin/marketplace.json                   |     2 +-
 .factory-plugin/plugin.json                        |     4 +-
 .gitattributes                                     |     1 +
 .github/workflows/ci.yml                           |    74 +
 .github/workflows/design-gates.yml                 |    44 +
 .github/workflows/release-sync-pages.yml           |   168 +
 .project-agent.md                                  |     6 +-
 CHANGELOG.md                                       |   356 +
 CLAUDE.md                                          |    52 +-
 Cargo.lock                                         |   441 +-
 Cargo.toml                                         |     9 +-
 LICENSE                                            |   105 +-
 LICENSE-APACHE                                     |   202 -
 PRIVACY.md                                         |    17 +-
 README.md                                          |   478 +-
 THIRD_PARTY_LICENSES.md                            |   181 +
 assets/skills/accessibility-wcag/SKILL.md          |    16 +
 assets/skills/component-craft/SKILL.md             |    16 +
 assets/skills/design-system-rhythm/SKILL.md        |    16 +
 commands/doctor.md                                 |     2 +-
 crates/designos-app/Cargo.toml                     |    12 +-
 .../assets/icons/phosphor/caret-up.svg             |     1 +
 crates/designos-app/build.rs                       |    36 +-
 crates/designos-app/src/agent_a11y.rs              |   668 +
 crates/designos-app/src/agent_approval_card.rs     |   307 +
 crates/designos-app/src/agent_busy_frames.rs       |   772 +
 crates/designos-app/src/agent_cli_catalog.rs       |  1112 ++
 .../designos-app/src/agent_cli_export_fig_tests.rs |   244 +
 crates/designos-app/src/agent_composer.rs          |  2736 ++++
 .../designos-app/src/agent_failure_patterns.toml   |   124 +
 crates/designos-app/src/agent_failure_redact.rs    |   285 +
 crates/designos-app/src/agent_mention.rs           |   944 ++
 crates/designos-app/src/agent_mention_popup.rs     |   541 +
 crates/designos-app/src/agent_notify.rs            |   314 +
 crates/designos-app/src/agent_open_flow.rs         |  1581 ++
 crates/designos-app/src/agent_permissions.rs       |   845 +
 crates/designos-app/src/agent_picker.rs            |  1331 ++
 crates/designos-app/src/agent_presence.rs          |   816 +
 crates/designos-app/src/agent_sample_prompts.rs    |   546 +
 crates/designos-app/src/agent_sample_prompts.toml  |    67 +
 crates/designos-app/src/agent_selection_chip.rs    |   398 +
 crates/designos-app/src/agent_session_failure.rs   |  1217 ++
 crates/designos-app/src/agent_session_files.rs     |  1231 ++
 crates/designos-app/src/agent_sessions.rs          |  1925 +++
 crates/designos-app/src/agent_start_failure.rs     |   580 +
 crates/designos-app/src/agent_stop.rs              |  1544 ++
 crates/designos-app/src/agent_turn_card.rs         |   380 +
 crates/designos-app/src/agent_turn_ledger.rs       |  1772 +++
 crates/designos-app/src/agent_turn_marks.rs        |   157 +
 crates/designos-app/src/agent_turn_status.rs       |   874 ++
 crates/designos-app/src/agent_usage.rs             |  1258 ++
 crates/designos-app/src/app.rs                     |   153 +-
 .../src/apply_node_spec_variable_tests.rs          |    29 +
 crates/designos-app/src/board/bar.rs               |    34 +-
 crates/designos-app/src/board/canvas.rs            |     6 +-
 crates/designos-app/src/board/toolbar.rs           |    40 +-
 crates/designos-app/src/canvas/agent_banner.rs     |   125 +
 .../designos-app/src/canvas/agent_changes_chip.rs  |   838 +
 crates/designos-app/src/canvas/agent_overlay.rs    |   175 +
 crates/designos-app/src/canvas/create.rs           |    55 +-
 crates/designos-app/src/canvas/draw.rs             |   625 +-
 crates/designos-app/src/canvas/gpu.rs              |    14 +-
 crates/designos-app/src/canvas/gpu_default.rs      |   165 +
 crates/designos-app/src/canvas/grids.rs            |   528 +-
 crates/designos-app/src/canvas/hit.rs              |   179 +-
 crates/designos-app/src/canvas/interact.rs         |   988 +-
 crates/designos-app/src/canvas/mod.rs              |    18 +
 crates/designos-app/src/canvas/proto.rs            |     4 +-
 crates/designos-app/src/canvas/rulers.rs           |     2 +-
 crates/designos-app/src/canvas/textsession.rs      |   465 +-
 crates/designos-app/src/canvas/tiles.rs            |    41 +-
 crates/designos-app/src/canvas/tools.rs            |    49 +-
 crates/designos-app/src/canvas/vector.rs           |     4 +-
 crates/designos-app/src/check_designs/contrast.rs  |    28 +
 crates/designos-app/src/check_designs/detached.rs  |    52 +
 crates/designos-app/src/check_designs/fix.rs       |   229 +
 crates/designos-app/src/check_designs/fixture.rs   |   147 +
 crates/designos-app/src/check_designs/hardcoded.rs |   296 +
 .../src/check_designs/library_assets.rs            |   101 +
 crates/designos-app/src/check_designs/mod.rs       |   523 +
 crates/designos-app/src/check_designs/state.rs     |   223 +
 crates/designos-app/src/check_designs/tests.rs     |   748 +
 .../src/check_designs/tests/backdrop.rs            |   181 +
 .../src/check_designs/tests/covering_sibling.rs    |   264 +
 .../src/check_designs/tests/fix_all.rs             |   108 +
 .../src/check_designs/tests/instances.rs           |   750 +
 .../src/check_designs/tests/libraries.rs           |   449 +
 .../check_designs/tests/not_painted_as_computed.rs |    83 +
 .../src/check_designs/tests/numbers.rs             |   283 +
 .../src/check_designs/tests/outside_the_box.rs     |   224 +
 .../src/check_designs/tests/painted_behind.rs      |   174 +
 .../src/check_designs/tests/resolved_styles.rs     |   114 +
 .../designos-app/src/check_designs/tests/rules.rs  |   487 +
 .../designos-app/src/check_designs/tests/safety.rs |   373 +
 .../designos-app/src/check_designs/tests/scale.rs  |   295 +
 .../src/check_designs/tests/subtree_bounds.rs      |   145 +
 .../src/check_designs/tests/text_run_fills.rs      |    99 +
 .../check_designs/tests/under_an_opaque_fill.rs    |   225 +
 .../designos-app/src/check_designs/text_styles.rs  |    50 +
 crates/designos-app/src/cli.rs                     |   216 +-
 crates/designos-app/src/cli_export_write.rs        |   134 +
 crates/designos-app/src/clipboard.rs               |   579 +-
 crates/designos-app/src/cmdk.rs                    |   497 +-
 crates/designos-app/src/comments/mod.rs            |    13 +
 crates/designos-app/src/comments/panel.rs          |    20 +-
 crates/designos-app/src/comments/settings.rs       |   147 +
 crates/designos-app/src/comments/thread.rs         |    10 +-
 crates/designos-app/src/comments/ui.rs             |    18 +-
 crates/designos-app/src/comp_ops.rs                |   189 +-
 crates/designos-app/src/decks/export.rs            |    17 +-
 crates/designos-app/src/decks/ui.rs                |     6 +-
 crates/designos-app/src/dev/annot.rs               |     6 +-
 crates/designos-app/src/dev/compare.rs             |    20 +-
 crates/designos-app/src/dev/data.rs                |   456 +-
 crates/designos-app/src/dev/highlight.rs           |     2 +-
 crates/designos-app/src/dev/list.rs                |    14 +-
 crates/designos-app/src/dev/mod.rs                 |    65 +-
 crates/designos-app/src/dev/playground.rs          |   780 +
 crates/designos-app/src/dev/sections.rs            |    93 +-
 crates/designos-app/src/edit.rs                    |   258 +-
 crates/designos-app/src/fig_export.rs              |    99 +
 .../src/fig_export/comment_notice_tests.rs         |   106 +
 crates/designos-app/src/fig_export/tests.rs        |   382 +
 crates/designos-app/src/find/filter.rs             |     2 +-
 crates/designos-app/src/find/panel.rs              |    66 +-
 crates/designos-app/src/font_refit.rs              |   314 +
 crates/designos-app/src/font_refit/tests.rs        |   345 +
 .../src/font_refit/tests/after_edits.rs            |   291 +
 crates/designos-app/src/font_refit/tests/guards.rs |    81 +
 .../designos-app/src/font_refit/tests/open_edit.rs |    31 +
 .../src/font_refit/tests/recorded_edits.rs         |    66 +
 .../src/font_refit/tests/undone_edits.rs           |   140 +
 crates/designos-app/src/fonts.rs                   |   345 +-
 crates/designos-app/src/history/banner.rs          |     4 +-
 crates/designos-app/src/history/modal.rs           |     4 +-
 crates/designos-app/src/history/panel.rs           |    60 +-
 crates/designos-app/src/history/session.rs         |     4 +
 crates/designos-app/src/home/dialog.rs             |   107 +-
 crates/designos-app/src/home/leave.rs              |   318 +
 crates/designos-app/src/home/main_view.rs          |    26 +-
 crates/designos-app/src/home/mod.rs                |   281 +-
 crates/designos-app/src/home/session.rs            |   329 +-
 crates/designos-app/src/home/sidebar.rs            |     8 +-
 .../src/home/spec_bundle_hidden_tests.rs           |   209 +
 crates/designos-app/src/home/unsaved.rs            |   144 +
 crates/designos-app/src/icons.rs                   |    10 +-
 crates/designos-app/src/image_jobs.rs              |  2301 +++
 crates/designos-app/src/io.rs                      |   570 +-
 .../src/io/comments_in_saved_files_tests.rs        |   121 +
 crates/designos-app/src/io/dialog_probe.rs         |   244 +
 crates/designos-app/src/io/fallback_save.rs        |   179 +
 crates/designos-app/src/io/file_dialog.rs          |   194 +
 .../designos-app/src/io/file_dialog_flow_tests.rs  |   341 +
 crates/designos-app/src/layout.rs                  |   233 +-
 .../designos-app/src/layout/solver_parity_tests.rs |    64 +-
 crates/designos-app/src/libraries/mod.rs           |    60 +-
 crates/designos-app/src/libraries/modal.rs         |    28 +-
 crates/designos-app/src/libraries/review.rs        |    16 +-
 crates/designos-app/src/lint.rs                    |     4 +-
 crates/designos-app/src/live_canvas_scope.rs       |   107 +-
 crates/designos-app/src/live_doc_epoch_tests.rs    |    29 +
 crates/designos-app/src/live_log_wake.rs           |   245 +
 crates/designos-app/src/live_patch_apply.rs        |   201 +-
 crates/designos-app/src/live_patch_log.rs          |   452 +-
 crates/designos-app/src/live_spec_apply.rs         |    53 +
 .../src/live_sync_interaction_tests.rs             |     5 +
 crates/designos-app/src/live_sync_tests.rs         |   235 +
 crates/designos-app/src/macos_menu.rs              |   229 +
 crates/designos-app/src/main.rs                    |   246 +-
 crates/designos-app/src/ops.rs                     |   824 +-
 crates/designos-app/src/panels/assets.rs           |    91 +-
 crates/designos-app/src/panels/check_designs.rs    |   572 +
 crates/designos-app/src/panels/components.rs       |   321 +-
 crates/designos-app/src/panels/context.rs          |    40 +-
 crates/designos-app/src/panels/effects.rs          |    54 +-
 crates/designos-app/src/panels/export.rs           |   224 +-
 crates/designos-app/src/panels/inspect.rs          |    79 +-
 crates/designos-app/src/panels/layers.rs           |   200 +-
 crates/designos-app/src/panels/layoutguide.rs      |   118 +-
 crates/designos-app/src/panels/left.rs             |    48 +-
 crates/designos-app/src/panels/menus.rs            |   159 +-
 crates/designos-app/src/panels/mod.rs              |     1 +
 crates/designos-app/src/panels/properties.rs       |   412 +-
 crates/designos-app/src/panels/proto_actions.rs    |     8 +-
 crates/designos-app/src/panels/prototype.rs        |    41 +-
 crates/designos-app/src/panels/rail.rs             |    42 +-
 crates/designos-app/src/panels/right.rs            |    20 +-
 crates/designos-app/src/panels/style_edit.rs       |    49 +-
 .../src/panels/style_edit_grid_rows.rs             |     1 +
 crates/designos-app/src/panels/toolbar.rs          |   139 +-
 crates/designos-app/src/panels/typography.rs       |    66 +-
 crates/designos-app/src/panels/var_bind.rs         |   112 +-
 crates/designos-app/src/panels/vertex.rs           |     4 +-
 crates/designos-app/src/paste_placement.rs         |   252 +
 crates/designos-app/src/plugins/ui.rs              |    20 +-
 crates/designos-app/src/present.rs                 |    63 +-
 crates/designos-app/src/proto/mod.rs               |     4 +
 crates/designos-app/src/proto/player.rs            |    22 +-
 crates/designos-app/src/proto/smart.rs             |    41 +-
 crates/designos-app/src/richtext.rs                |    57 +
 crates/designos-app/src/shell.rs                   |   742 +-
 crates/designos-app/src/shell/tests.rs             |   730 +-
 .../src/shell/tests/agent_panel_keyboard.rs        |    36 +
 .../agent_panel_keyboard/clipped_other_cli_form.rs |   104 +
 .../agent_panel_keyboard/dev_mode_shortcut.rs      |    34 +
 .../agent_panel_keyboard/hand_built_popovers.rs    |   162 +
 .../agent_panel_keyboard/hidden_other_cli_form.rs  |   181 +
 .../tests/agent_panel_keyboard/overlay_escape.rs   |   446 +
 .../agent_panel_keyboard/review_card_focus.rs      |    87 +
 .../session_rename_keyboard.rs                     |    32 +
 .../src/shell/tests/agent_picker_return.rs         |   277 +
 .../src/shell/tests/agents_panel_only.rs           |   195 +
 .../src/shell/tests/agents_tab_strip.rs            |   268 +
 .../designos-app/src/shell/tests/check_designs.rs  |   136 +
 crates/designos-app/src/shell/tests/comments.rs    |    28 +
 crates/designos-app/src/shell/tests/components.rs  |   588 +
 .../src/shell/tests/components_cycles.rs           |   169 +
 .../src/shell/tests/components_main_copies.rs      |   622 +
 .../src/shell/tests/deferred_page_instances.rs     |   139 +
 crates/designos-app/src/shell/tests/dev.rs         |   221 +-
 .../src/shell/tests/dev_mode_shortcut.rs           |   141 +
 .../src/shell/tests/duplicate_order.rs             |   141 +
 .../src/shell/tests/effect_grid_styles.rs          |     2 +-
 .../src/shell/tests/empty_slot_flows.rs            |   570 +
 .../designos-app/src/shell/tests/fast_drag_draw.rs |   400 +
 .../src/shell/tests/fast_drag_transform.rs         |   616 +
 .../src/shell/tests/fig_export_toast.rs            |   181 +
 crates/designos-app/src/shell/tests/figma_paste.rs |     5 +-
 .../src/shell/tests/grid_track_editing.rs          |   743 +
 crates/designos-app/src/shell/tests/history.rs     |    17 +
 crates/designos-app/src/shell/tests/home.rs        |    83 +-
 .../src/shell/tests/home_agents_card.rs            |    78 +
 .../designos-app/src/shell/tests/idle_repaint.rs   |   217 +
 .../src/shell/tests/layer_drag_fast.rs             |   548 +
 .../src/shell/tests/leave_menu_quit.rs             |   277 +
 .../designos-app/src/shell/tests/leave_prompt.rs   |   614 +
 .../src/shell/tests/leave_prompt_edges.rs          |   431 +
 crates/designos-app/src/shell/tests/libraries.rs   |   303 +-
 crates/designos-app/src/shell/tests/lint.rs        |     1 +
 .../src/shell/tests/live_agent_edits.rs            |    35 +-
 .../src/shell/tests/live_patch_agent_switch.rs     |   240 +
 .../shell/tests/live_patch_agent_switch_edges.rs   |   304 +
 .../shell/tests/live_patch_agent_switch_guards.rs  |   114 +
 crates/designos-app/src/shell/tests/menus.rs       |   229 +-
 .../src/shell/tests/palette_autofix_undo.rs        |   690 +
 crates/designos-app/src/shell/tests/panel_keys.rs  |   335 +
 .../src/shell/tests/relayout_budget.rs             |   108 +-
 .../src/shell/tests/resized_shape_geometry.rs      |   441 +
 .../src/shell/tests/slot_canvas_edits.rs           |   479 +
 .../shell/tests/stale_canvas_after_agent_edit.rs   |   385 +
 .../designos-app/src/shell/tests/stray_layers.rs   |   138 +
 crates/designos-app/src/shell/tests/style_edit.rs  |    54 +-
 crates/designos-app/src/shell/tests/svg.rs         |     9 +-
 .../src/shell/tests/system_paste_and_drop.rs       |  1039 ++
 .../designos-app/src/shell/tests/tab_disk_state.rs |   409 +
 .../src/shell/tests/terminal_undo_chord.rs         |   444 +
 .../src/shell/tests/undo_exact_layout.rs           |   167 +
 .../src/shell/tests/undo_layout_epoch.rs           |   208 +
 crates/designos-app/src/shell/tests/unsaved_dot.rs |   752 +
 crates/designos-app/src/shell/tests/variables.rs   |   175 +
 crates/designos-app/src/shell/toast_layout.rs      |    68 +
 crates/designos-app/src/skills/delivery.rs         |    61 +
 crates/designos-app/src/skills/hook_delivery.rs    |   387 +
 crates/designos-app/src/skills/mod.rs              |  1517 ++
 crates/designos-app/src/skills/pills.rs            |    18 +
 crates/designos-app/src/skills/run_record.rs       |   112 +
 crates/designos-app/src/specs.rs                   |   203 +-
 crates/designos-app/src/styles.rs                  |   108 +-
 .../designos-app/src/styles/fill_binding_tests.rs  |   163 +
 crates/designos-app/src/system_paste.rs            |   212 +
 crates/designos-app/src/tabs.rs                    |   374 +-
 crates/designos-app/src/tabs/agent_document.rs     |    88 +
 crates/designos-app/src/terminal.rs                | 15611 ++++++++++++++-----
 crates/designos-app/src/terminal/ask_mode_tests.rs |   294 +
 .../src/terminal/ask_mode_tests/golden.rs          |   345 +
 crates/designos-app/src/terminal_undo_chord.rs     |    93 +
 crates/designos-app/src/test_timing.rs             |   107 +
 crates/designos-app/src/textmeasure.rs             |    19 +
 crates/designos-app/src/theme.rs                   |   362 +-
 crates/designos-app/src/updater.rs                 |    46 +-
 crates/designos-app/src/variables/library.rs       |     6 +-
 crates/designos-app/src/variables/mod.rs           |   231 +-
 crates/designos-app/src/variables/popover.rs       |     6 +-
 crates/designos-app/src/variables/view.rs          |   278 +-
 crates/designos-app/src/variables/widgets.rs       |    19 +-
 crates/designos-app/src/widgets/field.rs           |    79 +-
 crates/designos-app/src/widgets/field_tab.rs       |   217 +
 crates/designos-app/src/widgets/menu.rs            |   295 +-
 crates/designos-app/src/widgets/mod.rs             |    68 +-
 crates/designos-app/src/widgets/picker.rs          |    62 +-
 crates/designos-app/src/widgets/popover.rs         |    17 +-
 crates/designos-app/src/widgets/popup_open.rs      |    96 +
 crates/designos-app/tests/cli_version.rs           |   134 +
 crates/designos-diff/Cargo.toml                    |     2 +-
 crates/designos-diff/src/lib.rs                    |   142 +
 crates/designos-diff/src/main.rs                   |    11 +-
 crates/designos-diff/src/suite.rs                  |     4 +-
 crates/designos-diff/tests/cli.rs                  |    86 +
 crates/designos-doc/Cargo.toml                     |     6 +-
 crates/designos-doc/src/branch_diff.rs             |  1882 +++
 crates/designos-doc/src/branch_diff_tests.rs       |   502 +
 crates/designos-doc/src/branch_merge.rs            |  2400 +++
 crates/designos-doc/src/comments.rs                |     1 +
 crates/designos-doc/src/component_cycle_tests.rs   |   498 +
 crates/designos-doc/src/components.rs              |  2167 ++-
 crates/designos-doc/src/diff.rs                    |   244 +
 crates/designos-doc/src/finite_json/mod.rs         |   229 +
 crates/designos-doc/src/history.rs                 |  1391 +-
 crates/designos-doc/src/layout_apply.rs            |   301 +-
 crates/designos-doc/src/layout_constraints.rs      |   460 +
 crates/designos-doc/src/layout_solver.rs           |   134 +-
 crates/designos-doc/src/layout_solver_grid.rs      |   199 +-
 .../src/layout_solver_grid_placement.rs            |     8 +-
 crates/designos-doc/src/layout_solver_types.rs     |   237 +-
 crates/designos-doc/src/lib.rs                     |   987 +-
 crates/designos-doc/src/library_ops.rs             |    88 +-
 crates/designos-doc/src/lint/config.rs             |   317 +
 crates/designos-doc/src/lint/finding.rs            |   230 +
 crates/designos-doc/src/lint/fix.rs                |   336 +
 crates/designos-doc/src/lint/mod.rs                |   334 +
 crates/designos-doc/src/lint/pass_cache.rs         |    55 +
 crates/designos-doc/src/lint/progress_tests.rs     |    78 +
 crates/designos-doc/src/lint/rect_tree.rs          |   205 +
 crates/designos-doc/src/lint/registry.rs           |   501 +
 crates/designos-doc/src/lint/rules/contrast.rs     |   715 +
 .../src/lint/rules/contrast/backdrop.rs            |   300 +
 .../designos-doc/src/lint/rules/contrast/behind.rs |   456 +
 .../rules/contrast/behind/equivalence_tests.rs     |   277 +
 .../src/lint/rules/contrast/behind/reference.rs    |   193 +
 .../src/lint/rules/contrast/children.rs            |    99 +
 .../designos-doc/src/lint/rules/contrast/extent.rs |   114 +
 .../src/lint/rules/contrast/subtree.rs             |   294 +
 .../designos-doc/src/lint/rules/contrast/xpath.rs  |   100 +
 crates/designos-doc/src/lint/rules/detached.rs     |   178 +
 crates/designos-doc/src/lint/rules/mod.rs          |   861 +
 crates/designos-doc/src/lint/rules/scale.rs        |  1770 +++
 crates/designos-doc/src/lint/style_bindings.rs     |    46 +
 crates/designos-doc/src/lint/token_match.rs        |   763 +
 crates/designos-doc/src/lint/traverse.rs           |   764 +
 .../designos-doc/src/live_patch/log_line_tests.rs  |    19 +
 crates/designos-doc/src/live_patch/mod.rs          |   131 +-
 crates/designos-doc/src/live_patch/record.rs       |   207 +-
 crates/designos-doc/src/live_patch/replay.rs       |   484 +-
 .../designos-doc/src/live_patch/replay_tokens.rs   |    85 +
 crates/designos-doc/src/live_patch/tests.rs        |  1031 +-
 .../designos-doc/src/live_patch/version_tests.rs   |   107 +
 crates/designos-doc/src/node.rs                    |   117 +-
 crates/designos-doc/src/openpencil.rs              |   829 +-
 crates/designos-doc/src/paint.rs                   |    97 +-
 crates/designos-doc/src/query/mod.rs               |  2067 +++
 .../designos-doc/src/query/release_parity_tests.rs |   366 +
 crates/designos-doc/src/renumber_ids.rs            |    11 +-
 crates/designos-doc/src/renumber_ids_tests.rs      |    12 +
 crates/designos-doc/src/settings.rs                |    87 +
 crates/designos-doc/src/slot_model_tests.rs        |   614 +-
 crates/designos-doc/src/spec.rs                    |  1534 +-
 crates/designos-doc/src/spec_bind.rs               |  2367 +++
 .../src/spec_bind_reference_forms_tests.rs         |   346 +
 .../src/spec_bind_rollback_label_tests.rs          |   239 +
 crates/designos-doc/src/spec_data.rs               |    95 +
 crates/designos-doc/src/spec_instance.rs           |  3597 +++++
 crates/designos-doc/src/styles_ops.rs              |   128 +-
 crates/designos-doc/src/text_fills.rs              |    79 +
 crates/designos-doc/src/variables.rs               |   110 +-
 crates/designos-doc/src/variables_apply.rs         |   451 +-
 crates/designos-doc/src/variables_edit.rs          |    18 +-
 crates/designos-doc/src/writer_command.rs          |   588 +
 crates/designos-doc/tests/changelog_pillars.rs     |     6 +
 crates/designos-eval/Cargo.toml                    |    30 +
 crates/designos-eval/schema/results-v1.json        |   144 +
 crates/designos-eval/src/agent.rs                  |   319 +
 crates/designos-eval/src/cli.rs                    |   111 +
 crates/designos-eval/src/config.rs                 |   113 +
 crates/designos-eval/src/hash.rs                   |    87 +
 crates/designos-eval/src/jsonl.rs                  |   183 +
 crates/designos-eval/src/lib.rs                    |    22 +
 crates/designos-eval/src/main.rs                   |   121 +
 crates/designos-eval/src/mcp_client.rs             |   151 +
 crates/designos-eval/src/preamble.md               |     7 +
 crates/designos-eval/src/rng.rs                    |    71 +
 crates/designos-eval/src/runner.rs                 |   917 ++
 crates/designos-eval/src/sandbox.rs                |    95 +
 crates/designos-eval/src/split.rs                  |   146 +
 crates/designos-eval/src/stats.rs                  |   115 +
 crates/designos-eval/src/task.rs                   |   223 +
 crates/designos-eval/src/transcript.rs             |   108 +
 crates/designos-eval/tests/harness.rs              |   379 +
 crates/designos-eval/tests/real_engine.rs          |   156 +
 crates/designos-eval/tests/support/mod.rs          |   189 +
 crates/designos-export/Cargo.toml                  |     2 +-
 crates/designos-export/src/agent_context.rs        |  1915 +++
 crates/designos-export/src/android_xml.rs          |    18 +
 crates/designos-export/src/bounds.rs               |     7 +-
 crates/designos-export/src/codegen.rs              |   262 +-
 crates/designos-export/src/compose.rs              |   164 +-
 crates/designos-export/src/css.rs                  |   286 +-
 crates/designos-export/src/design_context.rs       |   815 +
 crates/designos-export/src/html.rs                 |  1111 ++
 crates/designos-export/src/language.rs             |   115 +
 crates/designos-export/src/lib.rs                  |    21 +-
 crates/designos-export/src/pdf.rs                  |     4 +-
 crates/designos-export/src/raster.rs               |    44 +-
 crates/designos-export/src/react.rs                |    21 +-
 crates/designos-export/src/slots.rs                |   638 +
 crates/designos-export/src/svg.rs                  |     2 +-
 crates/designos-export/src/swiftui.rs              |   196 +-
 crates/designos-export/src/tailwind.rs             |   135 +-
 crates/designos-export/src/uikit.rs                |    18 +-
 crates/designos-export/src/util.rs                 |    15 +
 .../tests/codegen_escapes_variable_code_syntax.rs  |     3 +
 .../tests/grid_codegen_placement_normalisation.rs  |   173 +
 .../tests/grid_codegen_spacing_bindings.rs         |     3 +
 .../tests/grid_codegen_tailwind_scale.rs           |     8 +-
 .../designos-export/tests/pdf_vector_fidelity.rs   |     5 +-
 crates/designos-export/tests/slot_codegen.rs       |   755 +
 .../tests/variable_code_syntax_per_platform.rs     |     3 +
 crates/designos-fig/Cargo.toml                     |     3 +-
 crates/designos-fig/FIELDS.md                      |     2 +-
 crates/designos-fig/src/export.rs                  |    35 +-
 crates/designos-fig/src/fig.rs                     |   665 +-
 crates/designos-fig/src/fig_export.rs              |   743 +-
 crates/designos-fig/src/fig_export/carrier.rs      |    32 +
 crates/designos-fig/src/fig_export/grid.rs         |   165 +
 crates/designos-fig/src/figma_schema.txt           |  2972 ++++
 crates/designos-fig/src/kiwi.rs                    |    98 +-
 crates/designos-fig/src/lib.rs                     |    24 +-
 crates/designos-fig/src/pen.rs                     |  1589 --
 crates/designos-fig/src/pen/export.rs              |   763 +
 crates/designos-fig/src/pen/instances.rs           |   951 ++
 crates/designos-fig/src/pen/migrate.rs             |   303 +
 crates/designos-fig/src/pen/mod.rs                 |   459 +
 crates/designos-fig/src/pen/node.rs                |  1088 ++
 crates/designos-fig/src/pen/paint.rs               |   820 +
 crates/designos-fig/src/pen/variables.rs           |   853 +
 crates/designos-fig/src/rest_import.rs             |  1351 +-
 crates/designos-fig/src/testkit.rs                 |  1404 +-
 crates/designos-fig/src/variables.rs               |    89 +-
 crates/designos-fig/src/variables_dtcg.rs          |  3653 +++++
 crates/designos-fig/src/web_capture.rs             |    97 +
 crates/designos-fig/tests/common/mod.rs            |   281 +
 .../designos-fig/tests/data/figma-real-schema.txt  |  4804 ++++++
 .../designos-fig/tests/fig_effect_grid_styles.rs   |   251 +-
 .../designos-fig/tests/fig_export_field_shapes.rs  |   345 +
 .../designos-fig/tests/fig_export_grid_shapes.rs   |   131 +
 crates/designos-fig/tests/fig_export_non_finite.rs |    19 +-
 .../designos-fig/tests/fig_export_real_schema.rs   |   172 +
 crates/designos-fig/tests/fig_export_real_tags.rs  |   241 +
 .../tests/fig_grid_fixed_row_export.rs             |    58 +
 .../tests/fig_grid_override_anchors.rs             |   761 +-
 .../designos-fig/tests/fig_import_legacy_export.rs |    55 +
 crates/designos-fig/tests/fig_roundtrip.rs         |  1464 +-
 crates/designos-fig/tests/fig_slots.rs             |  1174 +-
 crates/designos-fig/tests/fig_upgrade.rs           |    17 +-
 .../tests/fixtures/legacy-0.4.0-grids.fig          |   Bin 0 -> 10831 bytes
 .../tests/fixtures/legacy-0.4.0-rich.fig           |   Bin 0 -> 11061 bytes
 crates/designos-fig/tests/fixtures/pen/v1_0.pen    |    46 +
 crates/designos-fig/tests/fixtures/pen/v2_13.pen   |    35 +
 crates/designos-fig/tests/fixtures/pen/v2_17.pen   |    45 +
 crates/designos-fig/tests/fixtures/pen/v2_21.pen   |   103 +
 crates/designos-fig/tests/fixtures/pen/v2_8.pen    |    51 +
 .../designos-fig/tests/grid_child_align_patch.rs   |   434 +
 .../tests/kiwi_zero_size_struct_limit.rs           |    10 +-
 crates/designos-fig/tests/pen_schema_2_21.rs       |  2119 +++
 crates/designos-fig/tests/real_files.rs            |   380 +-
 crates/designos-fig/tests/rest_upgrade.rs          |    41 +-
 .../designos-fig/tests/stack_wrap_and_text_wrap.rs |    50 +
 .../tests/variables_library_collections.rs         |     1 +
 crates/designos-fonts/Cargo.toml                   |     4 +-
 crates/designos-fonts/assets/README.md             |    11 +
 crates/designos-fonts/assets/roboto-mono/OFL.txt   |    93 +
 .../assets/roboto-mono/RobotoMono[wght].ttf        |   Bin 0 -> 183700 bytes
 crates/designos-fonts/src/assets.rs                |    11 +-
 crates/designos-fonts/src/ensure.rs                |    18 +-
 crates/designos-fonts/src/lib.rs                   |     6 +-
 crates/designos-fonts/src/request.rs               |   125 +-
 crates/designos-fonts/src/scripts.rs               |     2 +-
 crates/designos-gpu/Cargo.toml                     |     2 +-
 crates/designos-gpu/src/compile.rs                 |    79 +-
 crates/designos-gpu/src/geom.rs                    |    84 +-
 crates/designos-gpu/tests/fidelity.rs              |   315 +
 crates/designos-gpu/tests/mask_without_own_fill.rs |    49 +
 crates/designos-gpu/tests/resized_shapes.rs        |   182 +
 crates/designos-gpu/tests/text_hug_box.rs          |   140 +
 crates/designos-library/Cargo.toml                 |     2 +-
 crates/designos-library/src/branches.rs            |   783 +
 crates/designos-library/src/lib.rs                 |     2 +
 crates/designos-library/src/store.rs               |   434 +-
 crates/designos-library/src/versions.rs            |    37 +
 crates/designos-library/tests/non_finite_floats.rs |    30 +
 crates/designos-mcp/Cargo.toml                     |    14 +-
 crates/designos-mcp/README.md                      |     3 +
 crates/designos-mcp/build.rs                       |    69 +
 crates/designos-mcp/generated/rules.md             |   359 +
 crates/designos-mcp/guidelines/archetypes/.gitkeep |     1 +
 crates/designos-mcp/guidelines/surfaces/.gitkeep   |     1 +
 crates/designos-mcp/parity/figma-mcp-parity.json   |   268 +
 crates/designos-mcp/rules/design-quality.md        |    15 +
 crates/designos-mcp/rules/workflow.md              |    13 +
 crates/designos-mcp/src/bin/gen-tool-docs.rs       |   248 +
 crates/designos-mcp/src/component_cycle_tests.rs   |   133 +
 crates/designos-mcp/src/engine.rs                  |  3007 +++-
 crates/designos-mcp/src/lib.rs                     |     8 +
 crates/designos-mcp/src/main.rs                    |    18 +-
 crates/designos-mcp/src/prompts.rs                 |  1386 ++
 crates/designos-mcp/src/resources.rs               |    29 +
 crates/designos-mcp/src/rules.rs                   |  1173 ++
 crates/designos-mcp/src/server.rs                  |  2714 +++-
 .../designos-mcp/src/server_idempotency_tests.rs   |   546 +
 crates/designos-mcp/src/tools/batch.rs             |  3534 +++++
 crates/designos-mcp/src/tools/bootstrap.rs         |   342 +
 crates/designos-mcp/src/tools/design_context.rs    |  1178 ++
 crates/designos-mcp/src/tools/errors.rs            |  1215 ++
 crates/designos-mcp/src/tools/fill_write.rs        |    98 +
 crates/designos-mcp/src/tools/guidelines.rs        |   896 ++
 crates/designos-mcp/src/tools/issues.rs            |  1106 ++
 crates/designos-mcp/src/tools/ledger.rs            |   726 +
 crates/designos-mcp/src/tools/ledger_reconnect.rs  |    67 +
 crates/designos-mcp/src/tools/ledger_tests.rs      |    69 +
 crates/designos-mcp/src/tools/libraries.rs         |   416 +
 crates/designos-mcp/src/tools/library_source.rs    |   126 +
 crates/designos-mcp/src/tools/metadata.rs          |   594 +
 crates/designos-mcp/src/tools/mod.rs               |   259 +
 crates/designos-mcp/src/tools/query.rs             |   340 +
 crates/designos-mcp/src/tools/screenshot.rs        |  1379 ++
 .../designos-mcp/src/tools/search_design_system.rs |  2466 +++
 crates/designos-mcp/src/tools/session_options.rs   |   245 +
 crates/designos-mcp/tests/colour_detach.rs         |   283 +
 crates/designos-mcp/tests/colour_var_library.rs    |    92 +
 crates/designos-mcp/tests/colour_var_refs.rs       |   275 +
 .../tests/fixtures/mcp-schema-2025-11-25.json      |  4059 +++++
 crates/designos-mcp/tests/list_lint_rules.rs       |    75 +
 crates/designos-mcp/tests/live_doc_epoch.rs        |     3 +-
 crates/designos-mcp/tests/session.rs               |    73 +
 crates/designos-mcp/tests/snapshots/tools.json     |  1850 +++
 crates/designos-mcp/tests/spec_text_colour.rs      |   195 +
 crates/designos-mcp/tests/stdio.rs                 |   113 +-
 crates/designos-mcp/tests/support/mod.rs           |   120 +
 crates/designos-mcp/tests/text_fill.rs             |   141 +
 crates/designos-mcp/tests/tools_snapshot.rs        |   326 +
 crates/designos-mcp/tests/version.rs               |    38 +
 crates/designos-render/Cargo.toml                  |     4 +-
 crates/designos-render/src/backdrop_index.rs       |    51 +
 crates/designos-render/src/diff_image.rs           |   121 +
 crates/designos-render/src/index.rs                |   141 +-
 crates/designos-render/src/lib.rs                  |   238 +-
 crates/designos-render/src/lint_env.rs             |   349 +
 crates/designos-render/src/mask.rs                 |    25 +-
 crates/designos-render/src/outline.rs              |   135 +-
 crates/designos-render/src/text.rs                 |   230 +-
 crates/designos-render/src/text/flow.rs            |   100 +-
 crates/designos-render/src/tiles.rs                |    42 +-
 crates/designos-render/tests/backdrop_index.rs     |   207 +
 crates/designos-render/tests/font_arrival.rs       |   116 +
 crates/designos-render/tests/fonts.rs              |    35 +-
 crates/designos-render/tests/geometry.rs           |    19 +
 crates/designos-render/tests/gradients.rs          |    20 +
 crates/designos-render/tests/lint_contrast.rs      |   469 +
 crates/designos-render/tests/lint_rules.rs         |   951 ++
 .../designos-render/tests/mask_without_own_fill.rs |    55 +
 crates/designos-render/tests/render.rs             |   110 +
 crates/designos-render/tests/text_fill_source.rs   |    27 +
 .../tests/tiles_edit_while_converging.rs           |   127 +
 crates/designos-render/tests/variables.rs          |   132 +
 crates/designos-svg/Cargo.toml                     |     2 +-
 crates/designos-webcapture/Cargo.toml              |    18 +
 crates/designos-webcapture/js/capture.js           |  1578 ++
 crates/designos-webcapture/src/import.rs           |  1701 ++
 crates/designos-webcapture/src/layout_infer.rs     |   622 +
 crates/designos-webcapture/src/lib.rs              |    17 +
 crates/designos-webcapture/src/paint_map.rs        |   838 +
 crates/designos-webcapture/src/schema.rs           |   293 +
 crates/designos-webcapture/src/text_map.rs         |   436 +
 crates/designos-webcapture/tests/capture_tests.rs  |   244 +
 .../tests/playwright/fixtures/card.capture.json    |  1485 ++
 .../tests/playwright/fixtures/card.html            |   107 +
 .../fixtures/card.truncated.capture.json           |   573 +
 .../tests/playwright/fixtures/cascade.html         |    89 +
 .../tests/playwright/fixtures/pseudo.html          |    19 +
 .../tests/playwright/fixtures/ref_article.png      |   Bin 0 -> 13040 bytes
 .../tests/playwright/fixtures/ref_card.png         |   Bin 0 -> 9603 bytes
 .../tests/playwright/fixtures/ref_form.png         |   Bin 0 -> 7139 bytes
 .../tests/playwright/fixtures/ref_nav.png          |   Bin 0 -> 4591 bytes
 .../tests/playwright/fixtures/ref_pricing_grid.png |   Bin 0 -> 6518 bytes
 .../tests/playwright/fixtures/scrolled.html        |    18 +
 .../tests/playwright/fixtures/stacking.html        |    40 +
 .../tests/playwright/run_capture_tests.mjs         |   399 +
 design/DESIGN.md                                   |    10 +-
 design/design.tokens.json                          |    18 +-
 design/ds.manifest.json                            |    13 +-
 design/marketing/designos-launch-4k-bento-grid.png |   Bin 1661142 -> 0 bytes
 design/marketing/screenshots/01-hero-canvas.png    |   Bin 723210 -> 0 bytes
 design/marketing/screenshots/02-agents-chat.png    |   Bin 807950 -> 0 bytes
 design/marketing/screenshots/03-spec-reader.png    |   Bin 657445 -> 0 bytes
 .../screenshots/04-cmdk-ai-synthesizer.png         |   Bin 821332 -> 0 bytes
 .../marketing/screenshots/05-dev-mode-codegen.png  |   Bin 837443 -> 0 bytes
 .../marketing/screenshots/06-variables-tokens.png  |   Bin 215143 -> 0 bytes
 .../screenshots/07-components-variants.png         |   Bin 591263 -> 0 bytes
 .../marketing/screenshots/08-prototype-flows.png   |   Bin 544610 -> 0 bytes
 .../marketing/screenshots/09-micro-autolayout.png  |   Bin 513187 -> 0 bytes
 design/marketing/screenshots/10-micro-css-grid.png |   Bin 705814 -> 0 bytes
 .../screenshots/11-micro-smart-selection.png       |   Bin 505293 -> 0 bytes
 .../screenshots/12-micro-shape-controls.png        |   Bin 506190 -> 0 bytes
 .../marketing/screenshots/13-micro-vector-edit.png |   Bin 515501 -> 0 bytes
 .../screenshots/14-micro-layer-picker.png          |   Bin 872914 -> 0 bytes
 .../screenshots/15-micro-dev-boxmodel.png          |   Bin 919820 -> 0 bytes
 .../marketing/screenshots/16-micro-scale-tool.png  |   Bin 496385 -> 0 bytes
 design/marketing/screenshots/micro/m01-step1.png   |   Bin 506897 -> 0 bytes
 design/marketing/screenshots/micro/m01-step2.png   |   Bin 508210 -> 0 bytes
 design/marketing/screenshots/micro/m01-step3.png   |   Bin 520101 -> 0 bytes
 design/marketing/screenshots/micro/m01-step4.png   |   Bin 524429 -> 0 bytes
 design/marketing/screenshots/micro/m01-step5.png   |   Bin 524434 -> 0 bytes
 design/marketing/screenshots/micro/m01-step6.png   |   Bin 515084 -> 0 bytes
 design/marketing/screenshots/micro/m02-step1.png   |   Bin 665367 -> 0 bytes
 design/marketing/screenshots/micro/m02-step2.png   |   Bin 691936 -> 0 bytes
 design/marketing/screenshots/micro/m02-step3.png   |   Bin 714004 -> 0 bytes
 design/marketing/screenshots/micro/m02-step4.png   |   Bin 709493 -> 0 bytes
 design/marketing/screenshots/micro/m02-step5.png   |   Bin 702462 -> 0 bytes
 design/marketing/screenshots/micro/m02-step6.png   |   Bin 790201 -> 0 bytes
 design/marketing/screenshots/micro/m03-step1.png   |   Bin 521773 -> 0 bytes
 design/marketing/screenshots/micro/m03-step2.png   |   Bin 505841 -> 0 bytes
 design/marketing/screenshots/micro/m03-step3.png   |   Bin 507067 -> 0 bytes
 design/marketing/screenshots/micro/m03-step4.png   |   Bin 507770 -> 0 bytes
 design/marketing/screenshots/micro/m03-step5.png   |   Bin 507292 -> 0 bytes
 design/marketing/screenshots/micro/m03-step6.png   |   Bin 508619 -> 0 bytes
 design/marketing/screenshots/micro/m04-step1.png   |   Bin 499896 -> 0 bytes
 design/marketing/screenshots/micro/m04-step2.png   |   Bin 501919 -> 0 bytes
 design/marketing/screenshots/micro/m04-step3.png   |   Bin 505321 -> 0 bytes
 design/marketing/screenshots/micro/m04-step4.png   |   Bin 508951 -> 0 bytes
 design/marketing/screenshots/micro/m04-step5.png   |   Bin 509320 -> 0 bytes
 design/marketing/screenshots/micro/m04-step6.png   |   Bin 578627 -> 0 bytes
 design/marketing/screenshots/micro/m05-step1.png   |   Bin 515459 -> 0 bytes
 design/marketing/screenshots/micro/m05-step2.png   |   Bin 516840 -> 0 bytes
 design/marketing/screenshots/micro/m05-step3.png   |   Bin 526594 -> 0 bytes
 design/marketing/screenshots/micro/m05-step4.png   |   Bin 547405 -> 0 bytes
 design/marketing/screenshots/micro/m05-step5.png   |   Bin 500063 -> 0 bytes
 design/marketing/screenshots/micro/m05-step6.png   |   Bin 525932 -> 0 bytes
 design/marketing/screenshots/micro/m06-step1.png   |   Bin 872914 -> 0 bytes
 design/marketing/screenshots/micro/m06-step2.png   |   Bin 840520 -> 0 bytes
 design/marketing/screenshots/micro/m06-step3.png   |   Bin 839123 -> 0 bytes
 design/marketing/screenshots/micro/m06-step4.png   |   Bin 821205 -> 0 bytes
 design/marketing/screenshots/micro/m06-step5.png   |   Bin 822756 -> 0 bytes
 design/marketing/screenshots/micro/m06-step6.png   |   Bin 851268 -> 0 bytes
 design/marketing/screenshots/micro/m07-step1.png   |   Bin 974511 -> 0 bytes
 design/marketing/screenshots/micro/m07-step2.png   |   Bin 757874 -> 0 bytes
 design/marketing/screenshots/micro/m07-step3.png   |   Bin 786227 -> 0 bytes
 design/marketing/screenshots/micro/m07-step4.png   |   Bin 1083287 -> 0 bytes
 design/marketing/screenshots/micro/m07-step5.png   |   Bin 693406 -> 0 bytes
 design/marketing/screenshots/micro/m07-step6.png   |   Bin 975356 -> 0 bytes
 design/marketing/screenshots/micro/m08-step1.png   |   Bin 484565 -> 0 bytes
 design/marketing/screenshots/micro/m08-step2.png   |   Bin 499143 -> 0 bytes
 design/marketing/screenshots/micro/m08-step3.png   |   Bin 512994 -> 0 bytes
 design/marketing/screenshots/micro/m08-step4.png   |   Bin 528057 -> 0 bytes
 design/marketing/screenshots/micro/m08-step5.png   |   Bin 538303 -> 0 bytes
 design/marketing/screenshots/micro/m08-step6.png   |   Bin 512954 -> 0 bytes
 ...ignos-studio-launch-hero-hyperframes-draft.html |    89 -
 .../01-designos-studio-launch-hero.html            |  1293 --
 ...a-100-micro-interactions-hyperframes-draft.html |    89 -
 .../02-figma-100-micro-interactions.html           |  1342 --
 ...-first-specs-and-devmode-hyperframes-draft.html |    89 -
 .../03-agent-first-specs-and-devmode.html          |  1266 --
 .../marketing/svg-animation/hyperframes-beats.mjs  |    75 -
 .../svg-animation/micro-01-autolayout-5.html       |   167 +-
 .../marketing/svg-animation/micro-02-css-grid.html |   167 +-
 .../svg-animation/micro-03-smart-selection.html    |   179 +-
 .../svg-animation/micro-04-shape-controls.html     |   169 +-
 .../svg-animation/micro-05-vector-network.html     |   173 +-
 .../svg-animation/micro-06-layer-eyedropper.html   |   179 +-
 .../svg-animation/micro-07-devmode-boxmodel.html   |   169 +-
 .../svg-animation/micro-08-scale-navigation.html   |   171 +-
 .../svg-animation/micro-kit/m01-step1.webp         |   Bin 0 -> 163950 bytes
 .../svg-animation/micro-kit/m01-step2.webp         |   Bin 0 -> 164414 bytes
 .../svg-animation/micro-kit/m01-step3.webp         |   Bin 0 -> 164244 bytes
 .../svg-animation/micro-kit/m01-step4.webp         |   Bin 0 -> 163830 bytes
 .../svg-animation/micro-kit/m01-step5.webp         |   Bin 0 -> 146398 bytes
 .../svg-animation/micro-kit/m01-step6.webp         |   Bin 0 -> 167160 bytes
 .../svg-animation/micro-kit/m02-step1.webp         |   Bin 0 -> 152686 bytes
 .../svg-animation/micro-kit/m02-step2.webp         |   Bin 0 -> 154838 bytes
 .../svg-animation/micro-kit/m02-step3.webp         |   Bin 0 -> 155362 bytes
 .../svg-animation/micro-kit/m02-step4.webp         |   Bin 0 -> 164278 bytes
 .../svg-animation/micro-kit/m02-step5.webp         |   Bin 0 -> 191430 bytes
 .../svg-animation/micro-kit/m02-step6.webp         |   Bin 0 -> 176424 bytes
 .../svg-animation/micro-kit/m03-step1.webp         |   Bin 0 -> 165908 bytes
 .../svg-animation/micro-kit/m03-step2.webp         |   Bin 0 -> 168332 bytes
 .../svg-animation/micro-kit/m03-step3.webp         |   Bin 0 -> 167598 bytes
 .../svg-animation/micro-kit/m03-step4.webp         |   Bin 0 -> 168158 bytes
 .../svg-animation/micro-kit/m03-step5.webp         |   Bin 0 -> 168624 bytes
 .../svg-animation/micro-kit/m03-step6.webp         |   Bin 0 -> 169756 bytes
 .../svg-animation/micro-kit/m04-step1.webp         |   Bin 0 -> 187014 bytes
 .../svg-animation/micro-kit/m04-step2.webp         |   Bin 0 -> 192198 bytes
 .../svg-animation/micro-kit/m04-step3.webp         |   Bin 0 -> 191412 bytes
 .../svg-animation/micro-kit/m04-step4.webp         |   Bin 0 -> 213400 bytes
 .../svg-animation/micro-kit/m04-step5.webp         |   Bin 0 -> 182700 bytes
 .../svg-animation/micro-kit/m04-step6.webp         |   Bin 0 -> 208686 bytes
 .../svg-animation/micro-kit/m05-step1.webp         |   Bin 0 -> 222492 bytes
 .../svg-animation/micro-kit/m05-step2.webp         |   Bin 0 -> 216514 bytes
 .../svg-animation/micro-kit/m05-step3.webp         |   Bin 0 -> 221540 bytes
 .../svg-animation/micro-kit/m05-step4.webp         |   Bin 0 -> 245734 bytes
 .../svg-animation/micro-kit/m05-step5.webp         |   Bin 0 -> 247122 bytes
 .../svg-animation/micro-kit/m05-step6.webp         |   Bin 0 -> 220906 bytes
 .../svg-animation/micro-kit/m06-step1.webp         |   Bin 0 -> 144760 bytes
 .../svg-animation/micro-kit/m06-step2.webp         |   Bin 0 -> 158196 bytes
 .../svg-animation/micro-kit/m06-step3.webp         |   Bin 0 -> 166712 bytes
 .../svg-animation/micro-kit/m06-step4.webp         |   Bin 0 -> 295680 bytes
 .../svg-animation/micro-kit/m06-step5.webp         |   Bin 0 -> 295168 bytes
 .../svg-animation/micro-kit/m06-step6.webp         |   Bin 0 -> 265912 bytes
 .../svg-animation/micro-kit/m07-step1.webp         |   Bin 0 -> 315672 bytes
 .../svg-animation/micro-kit/m07-step2.webp         |   Bin 0 -> 315672 bytes
 .../svg-animation/micro-kit/m07-step3.webp         |   Bin 0 -> 349406 bytes
 .../svg-animation/micro-kit/m07-step4.webp         |   Bin 0 -> 315908 bytes
 .../svg-animation/micro-kit/m07-step5.webp         |   Bin 0 -> 336716 bytes
 .../svg-animation/micro-kit/m07-step6.webp         |   Bin 0 -> 328160 bytes
 .../svg-animation/micro-kit/m08-step1.webp         |   Bin 0 -> 133888 bytes
 .../svg-animation/micro-kit/m08-step2.webp         |   Bin 0 -> 130556 bytes
 .../svg-animation/micro-kit/m08-step3.webp         |   Bin 0 -> 133908 bytes
 .../svg-animation/micro-kit/m08-step4.webp         |   Bin 0 -> 131992 bytes
 .../svg-animation/micro-kit/m08-step5.webp         |   Bin 0 -> 127480 bytes
 .../svg-animation/micro-kit/m08-step6.webp         |   Bin 0 -> 133888 bytes
 ...ignos-studio-launch-hero-beat-contact-sheet.png |   Bin 1090875 -> 0 bytes
 .../beat_00.png                                    |   Bin 300576 -> 0 bytes
 .../beat_01.png                                    |   Bin 303276 -> 0 bytes
 .../beat_02.png                                    |   Bin 425375 -> 0 bytes
 .../beat_03.png                                    |   Bin 426843 -> 0 bytes
 .../beat_04.png                                    |   Bin 428114 -> 0 bytes
 .../beat_05.png                                    |   Bin 425079 -> 0 bytes
 .../beat_06.png                                    |   Bin 426381 -> 0 bytes
 .../beat_07.png                                    |   Bin 431274 -> 0 bytes
 .../beat_08.png                                    |   Bin 435080 -> 0 bytes
 .../beat_09.png                                    |   Bin 452931 -> 0 bytes
 .../beat_10.png                                    |   Bin 458559 -> 0 bytes
 .../beat_11.png                                    |   Bin 456893 -> 0 bytes
 .../beat_12.png                                    |   Bin 454948 -> 0 bytes
 .../beat_13.png                                    |   Bin 416810 -> 0 bytes
 .../beat_14.png                                    |   Bin 420597 -> 0 bytes
 .../beat_15.png                                    |   Bin 419211 -> 0 bytes
 .../beat_16.png                                    |   Bin 275978 -> 0 bytes
 .../beat_17.png                                    |   Bin 274215 -> 0 bytes
 .../beat_18.png                                    |   Bin 274355 -> 0 bytes
 .../beat_19.png                                    |   Bin 405341 -> 0 bytes
 .../beat_20.png                                    |   Bin 406719 -> 0 bytes
 .../beat_21.png                                    |   Bin 404434 -> 0 bytes
 .../beat_22.png                                    |   Bin 449545 -> 0 bytes
 .../beat_23.png                                    |   Bin 459241 -> 0 bytes
 .../beat_24.png                                    |   Bin 457495 -> 0 bytes
 .../beat_25.png                                    |   Bin 303455 -> 0 bytes
 .../beat_26.png                                    |   Bin 302394 -> 0 bytes
 .../beat_27.png                                    |   Bin 300494 -> 0 bytes
 ...a-100-micro-interactions-beat-contact-sheet.png |   Bin 1034809 -> 0 bytes
 .../beat_00.png                                    |   Bin 305818 -> 0 bytes
 .../beat_01.png                                    |   Bin 304614 -> 0 bytes
 .../beat_02.png                                    |   Bin 379075 -> 0 bytes
 .../beat_03.png                                    |   Bin 365440 -> 0 bytes
 .../beat_04.png                                    |   Bin 365522 -> 0 bytes
 .../beat_05.png                                    |   Bin 378706 -> 0 bytes
 .../beat_06.png                                    |   Bin 380257 -> 0 bytes
 .../beat_07.png                                    |   Bin 381731 -> 0 bytes
 .../beat_08.png                                    |   Bin 351482 -> 0 bytes
 .../beat_09.png                                    |   Bin 352583 -> 0 bytes
 .../beat_10.png                                    |   Bin 352473 -> 0 bytes
 .../beat_11.png                                    |   Bin 361756 -> 0 bytes
 .../beat_12.png                                    |   Bin 361280 -> 0 bytes
 .../beat_13.png                                    |   Bin 363682 -> 0 bytes
 .../beat_14.png                                    |   Bin 358993 -> 0 bytes
 .../beat_15.png                                    |   Bin 363501 -> 0 bytes
 .../beat_16.png                                    |   Bin 363379 -> 0 bytes
 .../beat_17.png                                    |   Bin 473852 -> 0 bytes
 .../beat_18.png                                    |   Bin 473676 -> 0 bytes
 .../beat_19.png                                    |   Bin 476465 -> 0 bytes
 .../beat_20.png                                    |   Bin 479775 -> 0 bytes
 .../beat_21.png                                    |   Bin 478352 -> 0 bytes
 .../beat_22.png                                    |   Bin 483442 -> 0 bytes
 .../beat_23.png                                    |   Bin 482630 -> 0 bytes
 .../beat_24.png                                    |   Bin 345435 -> 0 bytes
 .../beat_25.png                                    |   Bin 346943 -> 0 bytes
 .../beat_26.png                                    |   Bin 344076 -> 0 bytes
 .../beat_27.png                                    |   Bin 316136 -> 0 bytes
 ...-first-specs-and-devmode-beat-contact-sheet.png |   Bin 925126 -> 0 bytes
 .../beat_00.png                                    |   Bin 300193 -> 0 bytes
 .../beat_01.png                                    |   Bin 303135 -> 0 bytes
 .../beat_02.png                                    |   Bin 459431 -> 0 bytes
 .../beat_03.png                                    |   Bin 462798 -> 0 bytes
 .../beat_04.png                                    |   Bin 460577 -> 0 bytes
 .../beat_05.png                                    |   Bin 460628 -> 0 bytes
 .../beat_06.png                                    |   Bin 443094 -> 0 bytes
 .../beat_07.png                                    |   Bin 444710 -> 0 bytes
 .../beat_08.png                                    |   Bin 443625 -> 0 bytes
 .../beat_09.png                                    |   Bin 443967 -> 0 bytes
 .../beat_10.png                                    |   Bin 421173 -> 0 bytes
 .../beat_11.png                                    |   Bin 419867 -> 0 bytes
 .../beat_12.png                                    |   Bin 419538 -> 0 bytes
 .../beat_13.png                                    |   Bin 270205 -> 0 bytes
 .../beat_14.png                                    |   Bin 270451 -> 0 bytes
 .../beat_15.png                                    |   Bin 269746 -> 0 bytes
 .../beat_16.png                                    |   Bin 269621 -> 0 bytes
 .../beat_17.png                                    |   Bin 393056 -> 0 bytes
 .../beat_18.png                                    |   Bin 399255 -> 0 bytes
 .../beat_19.png                                    |   Bin 396419 -> 0 bytes
 .../beat_20.png                                    |   Bin 466587 -> 0 bytes
 .../beat_21.png                                    |   Bin 470543 -> 0 bytes
 .../beat_22.png                                    |   Bin 470542 -> 0 bytes
 .../beat_23.png                                    |   Bin 470426 -> 0 bytes
 .../beat_24.png                                    |   Bin 376743 -> 0 bytes
 .../beat_25.png                                    |   Bin 379739 -> 0 bytes
 .../beat_26.png                                    |   Bin 379446 -> 0 bytes
 .../beat_27.png                                    |   Bin 314842 -> 0 bytes
 .../proofs/micro-01-autolayout-5-16x9-proof.png    |   Bin 448312 -> 0 bytes
 .../proofs/micro-01-autolayout-5-9x16-proof.png    |   Bin 508101 -> 0 bytes
 .../proofs/micro-02-css-grid-16x9-proof.png        |   Bin 480335 -> 0 bytes
 .../proofs/micro-02-css-grid-9x16-proof.png        |   Bin 532548 -> 0 bytes
 .../proofs/micro-03-smart-selection-16x9-proof.png |   Bin 442818 -> 0 bytes
 .../proofs/micro-03-smart-selection-9x16-proof.png |   Bin 508314 -> 0 bytes
 .../proofs/micro-04-shape-controls-16x9-proof.png  |   Bin 454001 -> 0 bytes
 .../proofs/micro-04-shape-controls-9x16-proof.png  |   Bin 521906 -> 0 bytes
 .../proofs/micro-05-vector-network-16x9-proof.png  |   Bin 452476 -> 0 bytes
 .../proofs/micro-05-vector-network-9x16-proof.png  |   Bin 517579 -> 0 bytes
 .../micro-06-layer-eyedropper-16x9-proof.png       |   Bin 578934 -> 0 bytes
 .../micro-06-layer-eyedropper-9x16-proof.png       |   Bin 622497 -> 0 bytes
 .../micro-07-devmode-boxmodel-16x9-proof.png       |   Bin 577479 -> 0 bytes
 .../micro-07-devmode-boxmodel-9x16-proof.png       |   Bin 584861 -> 0 bytes
 .../micro-08-scale-navigation-16x9-proof.png       |   Bin 445589 -> 0 bytes
 .../micro-08-scale-navigation-9x16-proof.png       |   Bin 505049 -> 0 bytes
 .../videos/01-designos-studio-launch-hero.mp4      |   Bin 7460151 -> 0 bytes
 .../videos/02-figma-100-micro-interactions.mp4     |   Bin 7082596 -> 0 bytes
 .../videos/03-agent-first-specs-and-devmode.mp4    |   Bin 6116440 -> 0 bytes
 .../videos/16x9/micro-01-autolayout-5-16x9.mp4     |   Bin 4439032 -> 0 bytes
 .../videos/16x9/micro-02-css-grid-16x9.mp4         |   Bin 4684422 -> 0 bytes
 .../videos/16x9/micro-03-smart-selection-16x9.mp4  |   Bin 4244060 -> 0 bytes
 .../videos/16x9/micro-04-shape-controls-16x9.mp4   |   Bin 4344519 -> 0 bytes
 .../videos/16x9/micro-05-vector-network-16x9.mp4   |   Bin 4261012 -> 0 bytes
 .../videos/16x9/micro-06-layer-eyedropper-16x9.mp4 |   Bin 5223577 -> 0 bytes
 .../videos/16x9/micro-07-devmode-boxmodel-16x9.mp4 |   Bin 5558525 -> 0 bytes
 .../videos/16x9/micro-08-scale-navigation-16x9.mp4 |   Bin 4334661 -> 0 bytes
 .../videos/9x16/micro-01-autolayout-5-9x16.mp4     |   Bin 3478153 -> 0 bytes
 .../videos/9x16/micro-02-css-grid-9x16.mp4         |   Bin 3590901 -> 0 bytes
 .../videos/9x16/micro-03-smart-selection-9x16.mp4  |   Bin 3382460 -> 0 bytes
 .../videos/9x16/micro-04-shape-controls-9x16.mp4   |   Bin 3534016 -> 0 bytes
 .../videos/9x16/micro-05-vector-network-9x16.mp4   |   Bin 3474028 -> 0 bytes
 .../videos/9x16/micro-06-layer-eyedropper-9x16.mp4 |   Bin 4243806 -> 0 bytes
 .../videos/9x16/micro-07-devmode-boxmodel-9x16.mp4 |   Bin 4150223 -> 0 bytes
 .../videos/9x16/micro-08-scale-navigation-9x16.mp4 |   Bin 3377054 -> 0 bytes
 .../videos/designos-studio-launch-preview.gif      |   Bin 8617435 -> 0 bytes
 .../videos/micro-01-autolayout-5-16x9.mp4          |   Bin 3318459 -> 0 bytes
 .../videos/micro-01-autolayout-5-9x16.mp4          |   Bin 3210187 -> 0 bytes
 design/marketing/videos/micro-01-autolayout-5.mp4  |   Bin 4439032 -> 0 bytes
 design/marketing/videos/micro-02-css-grid-16x9.mp4 |   Bin 3751679 -> 0 bytes
 design/marketing/videos/micro-02-css-grid-9x16.mp4 |   Bin 3524491 -> 0 bytes
 design/marketing/videos/micro-02-css-grid.mp4      |   Bin 4684422 -> 0 bytes
 .../videos/micro-03-smart-selection-16x9.mp4       |   Bin 3244799 -> 0 bytes
 .../videos/micro-03-smart-selection-9x16.mp4       |   Bin 3134056 -> 0 bytes
 .../marketing/videos/micro-03-smart-selection.mp4  |   Bin 4244060 -> 0 bytes
 .../videos/micro-04-shape-controls-16x9.mp4        |   Bin 2966919 -> 0 bytes
 .../videos/micro-04-shape-controls-9x16.mp4        |   Bin 2858403 -> 0 bytes
 .../marketing/videos/micro-04-shape-controls.mp4   |   Bin 4344519 -> 0 bytes
 .../videos/micro-05-vector-network-16x9.mp4        |   Bin 2967239 -> 0 bytes
 .../videos/micro-05-vector-network-9x16.mp4        |   Bin 3021605 -> 0 bytes
 .../marketing/videos/micro-05-vector-network.mp4   |   Bin 4261012 -> 0 bytes
 .../videos/micro-06-layer-eyedropper-16x9.mp4      |   Bin 7302973 -> 0 bytes
 .../videos/micro-06-layer-eyedropper-9x16.mp4      |   Bin 6041800 -> 0 bytes
 .../marketing/videos/micro-06-layer-eyedropper.mp4 |   Bin 5223577 -> 0 bytes
 .../videos/micro-07-devmode-boxmodel-16x9.mp4      |   Bin 20204586 -> 0 bytes
 .../videos/micro-07-devmode-boxmodel-9x16.mp4      |   Bin 22109154 -> 0 bytes
 .../marketing/videos/micro-07-devmode-boxmodel.mp4 |   Bin 5558525 -> 0 bytes
 .../videos/micro-08-scale-navigation-16x9.mp4      |   Bin 3447935 -> 0 bytes
 .../videos/micro-08-scale-navigation-9x16.mp4      |   Bin 3296618 -> 0 bytes
 .../marketing/videos/micro-08-scale-navigation.mp4 |   Bin 4334661 -> 0 bytes
 .../videos/micro-09-inspector-math-16x9.mp4        |   Bin 2964856 -> 0 bytes
 .../videos/micro-09-inspector-math-9x16.mp4        |   Bin 2881534 -> 0 bytes
 .../videos/micro-10-selection-corner-16x9.mp4      |   Bin 2908602 -> 0 bytes
 .../videos/micro-10-selection-corner-9x16.mp4      |   Bin 3012308 -> 0 bytes
 .../videos/micro-11-typography-multiedit-16x9.mp4  |   Bin 2982193 -> 0 bytes
 .../videos/micro-11-typography-multiedit-9x16.mp4  |   Bin 2764534 -> 0 bytes
 .../videos/micro-12-components-variables-16x9.mp4  |   Bin 3373717 -> 0 bytes
 .../videos/micro-12-components-variables-9x16.mp4  |   Bin 3215062 -> 0 bytes
 design/memory.events.count.json                    |     2 +-
 design/memory.events.jsonl                         |     2 +
 design/principles.md                               |    94 +-
 design/soul.md                                     |    51 +-
 design/specimen.html                               |     2 +-
 design/tokens.json                                 |     2 +-
 design/ux-writing.md                               |   142 +
 docs/_config.yml                                   |     4 +-
 docs/agent-cli-usage-sources.md                    |    37 +
 docs/assets/v2-anatomy.js                          |     2 +-
 docs/assets/v2-big-files.css                       |    14 +-
 docs/assets/v2-big-files.js                        |    73 +-
 docs/assets/v2-core.js                             |    21 +-
 docs/assets/v2-narrator.js                         |     2 +-
 docs/assets/v2-new.css                             |   126 +-
 docs/assets/v2-new.js                              |   270 +-
 docs/assets/v2-story.css                           |     4 +
 docs/assets/v2-workflow.css                        |    20 +-
 docs/assets/v2-workflow.js                         |    70 +-
 docs/cookbook.md                                   |    15 +-
 docs/eval-licences.md                              |   265 +
 docs/images/01-hero-canvas.png                     |   Bin 723210 -> 0 bytes
 docs/images/02-agents-chat.png                     |   Bin 807950 -> 0 bytes
 docs/images/03-spec-reader.png                     |   Bin 657445 -> 0 bytes
 docs/images/04-cmdk-ai-synth.png                   |   Bin 821332 -> 0 bytes
 docs/images/04-cmdk-ai-synthesizer.png             |   Bin 821332 -> 0 bytes
 docs/images/05-dev-mode-codegen.png                |   Bin 837443 -> 0 bytes
 docs/images/05-variables-modes.png                 |   Bin 215143 -> 0 bytes
 docs/images/06-components-assets.png               |   Bin 591263 -> 0 bytes
 docs/images/06-variables-tokens.png                |   Bin 215143 -> 0 bytes
 docs/images/07-components-variants.png             |   Bin 591263 -> 0 bytes
 docs/images/08-prototype-flows.png                 |   Bin 544610 -> 0 bytes
 docs/images/09-micro-autolayout.png                |   Bin 364310 -> 0 bytes
 docs/images/10-micro-css-grid.png                  |   Bin 434247 -> 0 bytes
 docs/images/11-micro-smart-selection.png           |   Bin 346619 -> 0 bytes
 docs/images/12-micro-shape-controls.png            |   Bin 372120 -> 0 bytes
 docs/images/13-micro-vector-edit.png               |   Bin 367264 -> 0 bytes
 docs/images/14-micro-layer-picker.png              |   Bin 779297 -> 0 bytes
 docs/images/15-micro-dev-boxmodel.png              |   Bin 773197 -> 0 bytes
 docs/images/16-micro-scale-tool.png                |   Bin 373654 -> 0 bytes
 docs/images/17-micro-inspector-math.png            |   Bin 348670 -> 0 bytes
 docs/images/18-micro-selection-corner.png          |   Bin 342349 -> 0 bytes
 docs/images/19-micro-typography-multiedit.png      |   Bin 414682 -> 0 bytes
 docs/images/20-micro-components-variables.png      |   Bin 393325 -> 0 bytes
 docs/images/cover.png                              |   Bin 740310 -> 932793 bytes
 docs/images/designos-actions-cmdk.png              |   Bin 793224 -> 1000114 bytes
 docs/images/designos-agent-spec.png                |   Bin 841593 -> 664663 bytes
 docs/images/designos-agents-chat.png               |   Bin 715831 -> 782196 bytes
 docs/images/designos-assets-panel.png              |   Bin 815768 -> 966087 bytes
 docs/images/designos-color-picker.png              |   Bin 1032915 -> 1038131 bytes
 docs/images/designos-comments.png                  |   Bin 570630 -> 0 bytes
 docs/images/designos-components.png                |   Bin 602490 -> 889084 bytes
 docs/images/designos-devmode.png                   |   Bin 845095 -> 853960 bytes
 docs/images/designos-hero.png                      |   Bin 740310 -> 932793 bytes
 docs/images/designos-home.png                      |   Bin 394688 -> 216291 bytes
 docs/images/designos-launch-4k-bento-grid.png      |   Bin 1661142 -> 0 bytes
 docs/images/designos-main-menu.png                 |   Bin 762600 -> 940351 bytes
 docs/images/designos-overview.png                  |   Bin 729785 -> 662636 bytes
 docs/images/designos-prototype.png                 |   Bin 547826 -> 615053 bytes
 docs/images/designos-spec-reader.png               |   Bin 1481832 -> 0 bytes
 docs/images/designos-variables.png                 |   Bin 224064 -> 200380 bytes
 .../auto-layout-space-evenly-vertical-wrap.png     |   Bin 301926 -> 580727 bytes
 docs/images/v0.3/dev-mode-android-xml.png          |   Bin 342334 -> 746903 bytes
 docs/images/v0.3/dev-mode-uikit.png                |   Bin 342681 -> 757839 bytes
 docs/images/v0.3/styles-edit-effect-style.png      |   Bin 373971 -> 773511 bytes
 docs/images/v0.3/styles-effect-and-grid-styles.png |   Bin 232399 -> 891051 bytes
 docs/images/v0.3/text-wrap-balance.png             |   Bin 472609 -> 1045339 bytes
 docs/images/v0.6/ask-before-acting-approval.png    |   Bin 0 -> 619126 bytes
 docs/images/v0.6/check-designs-color-fixes.png     |   Bin 0 -> 782283 bytes
 docs/images/v0.6/studio-hero.png                   |   Bin 0 -> 324561 bytes
 docs/images/v2/anatomy/anatomy.json                |    94 +-
 docs/images/v2/anatomy/canvas.svg                  |     2 +-
 docs/images/v2/anatomy/file.svg                    |     2 +-
 docs/images/v2/anatomy/inspector.svg               |     2 +-
 docs/images/v2/anatomy/layers.svg                  |     2 +-
 docs/images/v2/anatomy/rail.svg                    |     2 +-
 docs/images/v2/anatomy/studio-window-030.webp      |   Bin 98648 -> 0 bytes
 docs/images/v2/anatomy/studio-window-061.webp      |   Bin 0 -> 103246 bytes
 docs/images/v2/new/agent-edits-open-file.webp      |   Bin 49244 -> 0 bytes
 docs/images/v2/new/dev-mode-android-xml.webp       |   Bin 55286 -> 0 bytes
 docs/images/v2/new/edit-style-everywhere.webp      |   Bin 68594 -> 0 bytes
 docs/images/v2/new/effect-and-grid-styles.webp     |   Bin 50582 -> 0 bytes
 docs/images/v2/new/paste-from-figma.webp           |   Bin 67102 -> 0 bytes
 docs/images/v2/new/space-evenly-and-wrap.webp      |   Bin 46260 -> 0 bytes
 docs/images/v2/new/text-wrap-balance.webp          |   Bin 46366 -> 0 bytes
 docs/images/v2/proof-code.webp                     |   Bin 42164 -> 0 bytes
 docs/images/v2/proof-mcp.webp                      |   Bin 44512 -> 0 bytes
 docs/images/v2/proof-nodes.webp                    |   Bin 68636 -> 0 bytes
 docs/index.html                                    |   386 +-
 docs/install.md                                    |    23 +-
 LICENSE-MIT => docs/legal/licences/ease-design.txt |     3 +-
 docs/legal/licences/jakubkrehel-skills.txt         |    21 +
 docs/llms.txt                                      |     8 +-
 docs/mcp-parity.md                                 |   112 +
 docs/media/v2/scale/fallback/scale.webp            |   Bin 18176 -> 0 bytes
 docs/media/v2/scale/frames-lq/0001.webp            |   Bin 14102 -> 0 bytes
 docs/media/v2/scale/frames-lq/0002.webp            |   Bin 10966 -> 0 bytes
 docs/media/v2/scale/frames-lq/0003.webp            |   Bin 10584 -> 0 bytes
 docs/media/v2/scale/frames-lq/0004.webp            |   Bin 9612 -> 0 bytes
 docs/media/v2/scale/frames-lq/0005.webp            |   Bin 8862 -> 0 bytes
 docs/media/v2/scale/frames-lq/0006.webp            |   Bin 8916 -> 0 bytes
 docs/media/v2/scale/frames-lq/0007.webp            |   Bin 10214 -> 0 bytes
 docs/media/v2/scale/frames-lq/0008.webp            |   Bin 9174 -> 0 bytes
 docs/media/v2/scale/frames-lq/0009.webp            |   Bin 8980 -> 0 bytes
 docs/media/v2/scale/frames-lq/0010.webp            |   Bin 7944 -> 0 bytes
 docs/media/v2/scale/frames-lq/0011.webp            |   Bin 7690 -> 0 bytes
 docs/media/v2/scale/frames-lq/0012.webp            |   Bin 7408 -> 0 bytes
 docs/media/v2/scale/frames-lq/0013.webp            |   Bin 5984 -> 0 bytes
 docs/media/v2/scale/frames-lq/0014.webp            |   Bin 6930 -> 0 bytes
 docs/media/v2/scale/frames-lq/0015.webp            |   Bin 6364 -> 0 bytes
 docs/media/v2/scale/frames-lq/0016.webp            |   Bin 5796 -> 0 bytes
 docs/media/v2/scale/frames-lq/0017.webp            |   Bin 5088 -> 0 bytes
 docs/media/v2/scale/frames-lq/0018.webp            |   Bin 6870 -> 0 bytes
 docs/media/v2/scale/frames-lq/0019.webp            |   Bin 8342 -> 0 bytes
 docs/media/v2/scale/frames-lq/0020.webp            |   Bin 9206 -> 0 bytes
 docs/media/v2/scale/frames-lq/0021.webp            |   Bin 7570 -> 0 bytes
 docs/media/v2/scale/frames-lq/0022.webp            |   Bin 6786 -> 0 bytes
 docs/media/v2/scale/frames-lq/0023.webp            |   Bin 5882 -> 0 bytes
 docs/media/v2/scale/frames-lq/0024.webp            |   Bin 5468 -> 0 bytes
 docs/media/v2/scale/frames-lq/0025.webp            |   Bin 5008 -> 0 bytes
 docs/media/v2/scale/frames-lq/0026.webp            |   Bin 4892 -> 0 bytes
 docs/media/v2/scale/frames-lq/0027.webp            |   Bin 5350 -> 0 bytes
 docs/media/v2/scale/frames-lq/0028.webp            |   Bin 5532 -> 0 bytes
 docs/media/v2/scale/frames-lq/0029.webp            |   Bin 5660 -> 0 bytes
 docs/media/v2/scale/frames-lq/0030.webp            |   Bin 5956 -> 0 bytes
 docs/media/v2/scale/frames-lq/0031.webp            |   Bin 5754 -> 0 bytes
 docs/media/v2/scale/frames-lq/0032.webp            |   Bin 6052 -> 0 bytes
 docs/media/v2/scale/frames-lq/0033.webp            |   Bin 6240 -> 0 bytes
 docs/media/v2/scale/frames-lq/0034.webp            |   Bin 6204 -> 0 bytes
 docs/media/v2/scale/frames-lq/0035.webp            |   Bin 6108 -> 0 bytes
 docs/media/v2/scale/frames-lq/0036.webp            |   Bin 6150 -> 0 bytes
 docs/media/v2/scale/frames-lq/0037.webp            |   Bin 6208 -> 0 bytes
 docs/media/v2/scale/frames-lq/0038.webp            |   Bin 6200 -> 0 bytes
 docs/media/v2/scale/frames-lq/0039.webp            |   Bin 6192 -> 0 bytes
 docs/media/v2/scale/frames/0001.webp               |   Bin 34912 -> 0 bytes
 docs/media/v2/scale/frames/0002.webp               |   Bin 26732 -> 0 bytes
 docs/media/v2/scale/frames/0003.webp               |   Bin 24582 -> 0 bytes
 docs/media/v2/scale/frames/0004.webp               |   Bin 22804 -> 0 bytes
 docs/media/v2/scale/frames/0005.webp               |   Bin 24404 -> 0 bytes
 docs/media/v2/scale/frames/0006.webp               |   Bin 25512 -> 0 bytes
 docs/media/v2/scale/frames/0007.webp               |   Bin 29004 -> 0 bytes
 docs/media/v2/scale/frames/0008.webp               |   Bin 28710 -> 0 bytes
 docs/media/v2/scale/frames/0009.webp               |   Bin 29574 -> 0 bytes
 docs/media/v2/scale/frames/0010.webp               |   Bin 27062 -> 0 bytes
 docs/media/v2/scale/frames/0011.webp               |   Bin 28890 -> 0 bytes
 docs/media/v2/scale/frames/0012.webp               |   Bin 30276 -> 0 bytes
 docs/media/v2/scale/frames/0013.webp               |   Bin 25602 -> 0 bytes
 docs/media/v2/scale/frames/0014.webp               |   Bin 27750 -> 0 bytes
 docs/media/v2/scale/frames/0015.webp               |   Bin 24708 -> 0 bytes
 docs/media/v2/scale/frames/0016.webp               |   Bin 23204 -> 0 bytes
 docs/media/v2/scale/frames/0017.webp               |   Bin 21068 -> 0 bytes
 docs/media/v2/scale/frames/0018.webp               |   Bin 28030 -> 0 bytes
 docs/media/v2/scale/frames/0019.webp               |   Bin 28668 -> 0 bytes
 docs/media/v2/scale/frames/0020.webp               |   Bin 31408 -> 0 bytes
 docs/media/v2/scale/frames/0021.webp               |   Bin 26002 -> 0 bytes
 docs/media/v2/scale/frames/0022.webp               |   Bin 23810 -> 0 bytes
 docs/media/v2/scale/frames/0023.webp               |   Bin 20164 -> 0 bytes
 docs/media/v2/scale/frames/0024.webp               |   Bin 19246 -> 0 bytes
 docs/media/v2/scale/frames/0025.webp               |   Bin 16422 -> 0 bytes
 docs/media/v2/scale/frames/0026.webp               |   Bin 14762 -> 0 bytes
 docs/media/v2/scale/frames/0027.webp               |   Bin 14898 -> 0 bytes
 docs/media/v2/scale/frames/0028.webp               |   Bin 15554 -> 0 bytes
 docs/media/v2/scale/frames/0029.webp               |   Bin 16574 -> 0 bytes
 docs/media/v2/scale/frames/0030.webp               |   Bin 17654 -> 0 bytes
 docs/media/v2/scale/frames/0031.webp               |   Bin 17866 -> 0 bytes
 docs/media/v2/scale/frames/0032.webp               |   Bin 18068 -> 0 bytes
 docs/media/v2/scale/frames/0033.webp               |   Bin 18624 -> 0 bytes
 docs/media/v2/scale/frames/0034.webp               |   Bin 18490 -> 0 bytes
 docs/media/v2/scale/frames/0035.webp               |   Bin 18838 -> 0 bytes
 docs/media/v2/scale/frames/0036.webp               |   Bin 18512 -> 0 bytes
 docs/media/v2/scale/frames/0037.webp               |   Bin 18168 -> 0 bytes
 docs/media/v2/scale/frames/0038.webp               |   Bin 18092 -> 0 bytes
 docs/media/v2/scale/frames/0039.webp               |   Bin 18176 -> 0 bytes
 docs/media/v2/scale/manifest.json                  |    49 -
 docs/media/v2/workflow/fallback/agent.webp         |   Bin 18636 -> 0 bytes
 docs/media/v2/workflow/fallback/code.webp          |   Bin 34910 -> 0 bytes
 docs/media/v2/workflow/fallback/figma.webp         |   Bin 35624 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0001.webp         |   Bin 5774 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0002.webp         |   Bin 12418 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0003.webp         |   Bin 12414 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0004.webp         |   Bin 12336 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0005.webp         |   Bin 12334 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0006.webp         |   Bin 12344 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0007.webp         |   Bin 12326 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0008.webp         |   Bin 12310 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0009.webp         |   Bin 12424 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0010.webp         |   Bin 12538 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0011.webp         |   Bin 12510 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0012.webp         |   Bin 12524 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0013.webp         |   Bin 12594 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0014.webp         |   Bin 12678 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0015.webp         |   Bin 12714 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0016.webp         |   Bin 12736 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0017.webp         |   Bin 12680 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0018.webp         |   Bin 12762 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0019.webp         |   Bin 12716 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0020.webp         |   Bin 12646 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0021.webp         |   Bin 12684 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0022.webp         |   Bin 12670 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0023.webp         |   Bin 12684 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0024.webp         |   Bin 12668 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0025.webp         |   Bin 12684 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0026.webp         |   Bin 12668 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0027.webp         |   Bin 12684 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0028.webp         |   Bin 12668 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0029.webp         |   Bin 12654 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0030.webp         |   Bin 12694 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0031.webp         |   Bin 12650 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0032.webp         |   Bin 12678 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0033.webp         |   Bin 12394 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0034.webp         |   Bin 12344 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0035.webp         |   Bin 12338 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0036.webp         |   Bin 12304 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0037.webp         |   Bin 12466 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0038.webp         |   Bin 12512 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0039.webp         |   Bin 12512 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0040.webp         |   Bin 12528 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0041.webp         |   Bin 12658 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0042.webp         |   Bin 12674 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0043.webp         |   Bin 12696 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0044.webp         |   Bin 12660 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0045.webp         |   Bin 12676 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0046.webp         |   Bin 12680 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0047.webp         |   Bin 12676 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0048.webp         |   Bin 12648 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0049.webp         |   Bin 9786 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0050.webp         |   Bin 7974 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0051.webp         |   Bin 8280 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0052.webp         |   Bin 8194 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0053.webp         |   Bin 9010 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0054.webp         |   Bin 10062 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0055.webp         |   Bin 10062 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0056.webp         |   Bin 10064 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0057.webp         |   Bin 10022 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0058.webp         |   Bin 10028 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0059.webp         |   Bin 10050 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0060.webp         |   Bin 10048 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0061.webp         |   Bin 10018 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0062.webp         |   Bin 10018 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0063.webp         |   Bin 10018 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0064.webp         |   Bin 10018 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0065.webp         |   Bin 10066 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0066.webp         |   Bin 10038 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0067.webp         |   Bin 10164 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0068.webp         |   Bin 10088 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0069.webp         |   Bin 3708 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0070.webp         |   Bin 3720 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0071.webp         |   Bin 3668 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0072.webp         |   Bin 3664 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0073.webp         |   Bin 3802 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0074.webp         |   Bin 3828 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0075.webp         |   Bin 3822 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0076.webp         |   Bin 3822 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0077.webp         |   Bin 8440 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0078.webp         |   Bin 8408 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0079.webp         |   Bin 8354 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0080.webp         |   Bin 8350 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0081.webp         |   Bin 944 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0082.webp         |   Bin 1278 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0083.webp         |   Bin 1626 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0084.webp         |   Bin 1894 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0085.webp         |   Bin 2318 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0086.webp         |   Bin 2698 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0087.webp         |   Bin 3074 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0088.webp         |   Bin 3308 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0089.webp         |   Bin 3622 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0090.webp         |   Bin 3620 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0091.webp         |   Bin 3620 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0092.webp         |   Bin 3674 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0093.webp         |   Bin 4314 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0094.webp         |   Bin 5726 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0095.webp         |   Bin 7190 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0096.webp         |   Bin 8740 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0097.webp         |   Bin 9766 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0098.webp         |   Bin 9242 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0099.webp         |   Bin 9044 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0100.webp         |   Bin 8870 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0101.webp         |   Bin 8840 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0102.webp         |   Bin 8956 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0103.webp         |   Bin 9090 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0104.webp         |   Bin 9332 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0105.webp         |   Bin 9348 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0106.webp         |   Bin 9420 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0107.webp         |   Bin 9472 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0108.webp         |   Bin 9474 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0109.webp         |   Bin 5186 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0110.webp         |   Bin 5082 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0111.webp         |   Bin 5792 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0112.webp         |   Bin 6522 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0113.webp         |   Bin 7160 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0114.webp         |   Bin 7822 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0115.webp         |   Bin 8498 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0116.webp         |   Bin 9300 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0117.webp         |   Bin 9956 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0118.webp         |   Bin 10562 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0119.webp         |   Bin 11342 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0120.webp         |   Bin 11992 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0121.webp         |   Bin 11990 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0122.webp         |   Bin 11990 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0123.webp         |   Bin 8518 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0124.webp         |   Bin 8614 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0125.webp         |   Bin 9992 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0126.webp         |   Bin 11494 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0127.webp         |   Bin 13148 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0128.webp         |   Bin 14608 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0129.webp         |   Bin 14518 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0130.webp         |   Bin 14420 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0131.webp         |   Bin 14128 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0132.webp         |   Bin 14148 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0133.webp         |   Bin 14170 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0134.webp         |   Bin 14134 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0135.webp         |   Bin 14174 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0136.webp         |   Bin 14148 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0137.webp         |   Bin 1512 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0138.webp         |   Bin 1878 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0139.webp         |   Bin 1722 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0140.webp         |   Bin 1428 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0141.webp         |   Bin 1480 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0142.webp         |   Bin 1016 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0143.webp         |   Bin 862 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0144.webp         |   Bin 930 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0145.webp         |   Bin 3380 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0146.webp         |   Bin 3340 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0147.webp         |   Bin 3348 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0148.webp         |   Bin 3350 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0149.webp         |   Bin 3348 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0150.webp         |   Bin 3358 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0151.webp         |   Bin 3366 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0152.webp         |   Bin 3372 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0153.webp         |   Bin 3510 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0154.webp         |   Bin 3542 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0155.webp         |   Bin 3544 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0156.webp         |   Bin 3542 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0157.webp         |   Bin 3534 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0158.webp         |   Bin 3720 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0159.webp         |   Bin 3934 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0160.webp         |   Bin 4016 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0161.webp         |   Bin 4340 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0162.webp         |   Bin 4528 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0163.webp         |   Bin 4638 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0164.webp         |   Bin 4748 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0165.webp         |   Bin 4858 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0166.webp         |   Bin 5082 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0167.webp         |   Bin 5196 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0168.webp         |   Bin 5324 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0169.webp         |   Bin 5618 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0170.webp         |   Bin 5654 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0171.webp         |   Bin 5698 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0172.webp         |   Bin 5622 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0173.webp         |   Bin 5686 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0174.webp         |   Bin 5706 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0175.webp         |   Bin 5674 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0176.webp         |   Bin 5712 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0177.webp         |   Bin 5722 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0178.webp         |   Bin 5712 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0179.webp         |   Bin 5738 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0180.webp         |   Bin 5744 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0181.webp         |   Bin 5732 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0182.webp         |   Bin 5734 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0183.webp         |   Bin 5732 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0184.webp         |   Bin 5734 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0185.webp         |   Bin 3842 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0186.webp         |   Bin 3786 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0187.webp         |   Bin 3690 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0188.webp         |   Bin 4268 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0189.webp         |   Bin 4202 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0190.webp         |   Bin 4222 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0191.webp         |   Bin 4194 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0192.webp         |   Bin 4172 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0193.webp         |   Bin 4172 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0194.webp         |   Bin 3700 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0195.webp         |   Bin 3274 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0196.webp         |   Bin 2846 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0197.webp         |   Bin 2616 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0198.webp         |   Bin 2624 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0199.webp         |   Bin 2606 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0200.webp         |   Bin 2608 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0201.webp         |   Bin 10896 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0202.webp         |   Bin 5340 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0203.webp         |   Bin 5294 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0204.webp         |   Bin 5322 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0205.webp         |   Bin 5266 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0206.webp         |   Bin 5312 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0207.webp         |   Bin 5440 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0208.webp         |   Bin 5592 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0209.webp         |   Bin 5688 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0210.webp         |   Bin 5542 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0211.webp         |   Bin 5596 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0212.webp         |   Bin 5566 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0213.webp         |   Bin 5650 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0214.webp         |   Bin 5664 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0215.webp         |   Bin 5656 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0216.webp         |   Bin 5642 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0217.webp         |   Bin 4176 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0218.webp         |   Bin 10094 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0219.webp         |   Bin 11832 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0220.webp         |   Bin 13962 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0221.webp         |   Bin 14044 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0222.webp         |   Bin 14074 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0223.webp         |   Bin 14060 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0224.webp         |   Bin 14060 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0225.webp         |   Bin 14060 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0226.webp         |   Bin 14058 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0227.webp         |   Bin 14060 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0228.webp         |   Bin 14060 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0229.webp         |   Bin 14060 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0230.webp         |   Bin 14058 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0231.webp         |   Bin 14164 -> 0 bytes
 docs/media/v2/workflow/frames-lq/0232.webp         |   Bin 14106 -> 0 bytes
 docs/media/v2/workflow/frames/0001.webp            |   Bin 21030 -> 0 bytes
 docs/media/v2/workflow/frames/0002.webp            |   Bin 37558 -> 0 bytes
 docs/media/v2/workflow/frames/0003.webp            |   Bin 37410 -> 0 bytes
 docs/media/v2/workflow/frames/0004.webp            |   Bin 37370 -> 0 bytes
 docs/media/v2/workflow/frames/0005.webp            |   Bin 37236 -> 0 bytes
 docs/media/v2/workflow/frames/0006.webp            |   Bin 37266 -> 0 bytes
 docs/media/v2/workflow/frames/0007.webp            |   Bin 37246 -> 0 bytes
 docs/media/v2/workflow/frames/0008.webp            |   Bin 37274 -> 0 bytes
 docs/media/v2/workflow/frames/0009.webp            |   Bin 37480 -> 0 bytes
 docs/media/v2/workflow/frames/0010.webp            |   Bin 38036 -> 0 bytes
 docs/media/v2/workflow/frames/0011.webp            |   Bin 37770 -> 0 bytes
 docs/media/v2/workflow/frames/0012.webp            |   Bin 37754 -> 0 bytes
 docs/media/v2/workflow/frames/0013.webp            |   Bin 37916 -> 0 bytes
 docs/media/v2/workflow/frames/0014.webp            |   Bin 38038 -> 0 bytes
 docs/media/v2/workflow/frames/0015.webp            |   Bin 38524 -> 0 bytes
 docs/media/v2/workflow/frames/0016.webp            |   Bin 38678 -> 0 bytes
 docs/media/v2/workflow/frames/0017.webp            |   Bin 38854 -> 0 bytes
 docs/media/v2/workflow/frames/0018.webp            |   Bin 38738 -> 0 bytes
 docs/media/v2/workflow/frames/0019.webp            |   Bin 39026 -> 0 bytes
 docs/media/v2/workflow/frames/0020.webp            |   Bin 38616 -> 0 bytes
 docs/media/v2/workflow/frames/0021.webp            |   Bin 38596 -> 0 bytes
 docs/media/v2/workflow/frames/0022.webp            |   Bin 38672 -> 0 bytes
 docs/media/v2/workflow/frames/0023.webp            |   Bin 38594 -> 0 bytes
 docs/media/v2/workflow/frames/0024.webp            |   Bin 38676 -> 0 bytes
 docs/media/v2/workflow/frames/0025.webp            |   Bin 38584 -> 0 bytes
 docs/media/v2/workflow/frames/0026.webp            |   Bin 38666 -> 0 bytes
 docs/media/v2/workflow/frames/0027.webp            |   Bin 38594 -> 0 bytes
 docs/media/v2/workflow/frames/0028.webp            |   Bin 38676 -> 0 bytes
 docs/media/v2/workflow/frames/0029.webp            |   Bin 38674 -> 0 bytes
 docs/media/v2/workflow/frames/0030.webp            |   Bin 38624 -> 0 bytes
 docs/media/v2/workflow/frames/0031.webp            |   Bin 38572 -> 0 bytes
 docs/media/v2/workflow/frames/0032.webp            |   Bin 38698 -> 0 bytes
 docs/media/v2/workflow/frames/0033.webp            |   Bin 37558 -> 0 bytes
 docs/media/v2/workflow/frames/0034.webp            |   Bin 37638 -> 0 bytes
 docs/media/v2/workflow/frames/0035.webp            |   Bin 37588 -> 0 bytes
 docs/media/v2/workflow/frames/0036.webp            |   Bin 37528 -> 0 bytes
 docs/media/v2/workflow/frames/0037.webp            |   Bin 38134 -> 0 bytes
 docs/media/v2/workflow/frames/0038.webp            |   Bin 38284 -> 0 bytes
 docs/media/v2/workflow/frames/0039.webp            |   Bin 38268 -> 0 bytes
 docs/media/v2/workflow/frames/0040.webp            |   Bin 38278 -> 0 bytes
 docs/media/v2/workflow/frames/0041.webp            |   Bin 38526 -> 0 bytes
 docs/media/v2/workflow/frames/0042.webp            |   Bin 38574 -> 0 bytes
 docs/media/v2/workflow/frames/0043.webp            |   Bin 38484 -> 0 bytes
 docs/media/v2/workflow/frames/0044.webp            |   Bin 38478 -> 0 bytes
 docs/media/v2/workflow/frames/0045.webp            |   Bin 38462 -> 0 bytes
 docs/media/v2/workflow/frames/0046.webp            |   Bin 38504 -> 0 bytes
 docs/media/v2/workflow/frames/0047.webp            |   Bin 38462 -> 0 bytes
 docs/media/v2/workflow/frames/0048.webp            |   Bin 38668 -> 0 bytes
 docs/media/v2/workflow/frames/0049.webp            |   Bin 38070 -> 0 bytes
 docs/media/v2/workflow/frames/0050.webp            |   Bin 26872 -> 0 bytes
 docs/media/v2/workflow/frames/0051.webp            |   Bin 29398 -> 0 bytes
 docs/media/v2/workflow/frames/0052.webp            |   Bin 28978 -> 0 bytes
 docs/media/v2/workflow/frames/0053.webp            |   Bin 33302 -> 0 bytes
 docs/media/v2/workflow/frames/0054.webp            |   Bin 35562 -> 0 bytes
 docs/media/v2/workflow/frames/0055.webp            |   Bin 35648 -> 0 bytes
 docs/media/v2/workflow/frames/0056.webp            |   Bin 35636 -> 0 bytes
 docs/media/v2/workflow/frames/0057.webp            |   Bin 35620 -> 0 bytes
 docs/media/v2/workflow/frames/0058.webp            |   Bin 35624 -> 0 bytes
 docs/media/v2/workflow/frames/0059.webp            |   Bin 35640 -> 0 bytes
 docs/media/v2/workflow/frames/0060.webp            |   Bin 35574 -> 0 bytes
 docs/media/v2/workflow/frames/0061.webp            |   Bin 35608 -> 0 bytes
 docs/media/v2/workflow/frames/0062.webp            |   Bin 35616 -> 0 bytes
 docs/media/v2/workflow/frames/0063.webp            |   Bin 35602 -> 0 bytes
 docs/media/v2/workflow/frames/0064.webp            |   Bin 35606 -> 0 bytes
 docs/media/v2/workflow/frames/0065.webp            |   Bin 35700 -> 0 bytes
 docs/media/v2/workflow/frames/0066.webp            |   Bin 35806 -> 0 bytes
 docs/media/v2/workflow/frames/0067.webp            |   Bin 35716 -> 0 bytes
 docs/media/v2/workflow/frames/0068.webp            |   Bin 35728 -> 0 bytes
 docs/media/v2/workflow/frames/0069.webp            |   Bin 12626 -> 0 bytes
 docs/media/v2/workflow/frames/0070.webp            |   Bin 12622 -> 0 bytes
 docs/media/v2/workflow/frames/0071.webp            |   Bin 12440 -> 0 bytes
 docs/media/v2/workflow/frames/0072.webp            |   Bin 12452 -> 0 bytes
 docs/media/v2/workflow/frames/0073.webp            |   Bin 12706 -> 0 bytes
 docs/media/v2/workflow/frames/0074.webp            |   Bin 12718 -> 0 bytes
 docs/media/v2/workflow/frames/0075.webp            |   Bin 12616 -> 0 bytes
 docs/media/v2/workflow/frames/0076.webp            |   Bin 12602 -> 0 bytes
 docs/media/v2/workflow/frames/0077.webp            |   Bin 29982 -> 0 bytes
 docs/media/v2/workflow/frames/0078.webp            |   Bin 29994 -> 0 bytes
 docs/media/v2/workflow/frames/0079.webp            |   Bin 29908 -> 0 bytes
 docs/media/v2/workflow/frames/0080.webp            |   Bin 29796 -> 0 bytes
 docs/media/v2/workflow/frames/0081.webp            |   Bin 2564 -> 0 bytes
 docs/media/v2/workflow/frames/0082.webp            |   Bin 3478 -> 0 bytes
 docs/media/v2/workflow/frames/0083.webp            |   Bin 4182 -> 0 bytes
 docs/media/v2/workflow/frames/0084.webp            |   Bin 4874 -> 0 bytes
 docs/media/v2/workflow/frames/0085.webp            |   Bin 5798 -> 0 bytes
 docs/media/v2/workflow/frames/0086.webp            |   Bin 6814 -> 0 bytes
 docs/media/v2/workflow/frames/0087.webp            |   Bin 7664 -> 0 bytes
 docs/media/v2/workflow/frames/0088.webp            |   Bin 8168 -> 0 bytes
 docs/media/v2/workflow/frames/0089.webp            |   Bin 8836 -> 0 bytes
 docs/media/v2/workflow/frames/0090.webp            |   Bin 8838 -> 0 bytes
 docs/media/v2/workflow/frames/0091.webp            |   Bin 8736 -> 0 bytes
 docs/media/v2/workflow/frames/0092.webp            |   Bin 9276 -> 0 bytes
 docs/media/v2/workflow/frames/0093.webp            |   Bin 10498 -> 0 bytes
 docs/media/v2/workflow/frames/0094.webp            |   Bin 13702 -> 0 bytes
 docs/media/v2/workflow/frames/0095.webp            |   Bin 17916 -> 0 bytes
 docs/media/v2/workflow/frames/0096.webp            |   Bin 21862 -> 0 bytes
 docs/media/v2/workflow/frames/0097.webp            |   Bin 24022 -> 0 bytes
 docs/media/v2/workflow/frames/0098.webp            |   Bin 22982 -> 0 bytes
 docs/media/v2/workflow/frames/0099.webp            |   Bin 22580 -> 0 bytes
 docs/media/v2/workflow/frames/0100.webp            |   Bin 22082 -> 0 bytes
 docs/media/v2/workflow/frames/0101.webp            |   Bin 21950 -> 0 bytes
 docs/media/v2/workflow/frames/0102.webp            |   Bin 22520 -> 0 bytes
 docs/media/v2/workflow/frames/0103.webp            |   Bin 22364 -> 0 bytes
 docs/media/v2/workflow/frames/0104.webp            |   Bin 23206 -> 0 bytes
 docs/media/v2/workflow/frames/0105.webp            |   Bin 23222 -> 0 bytes
 docs/media/v2/workflow/frames/0106.webp            |   Bin 23366 -> 0 bytes
 docs/media/v2/workflow/frames/0107.webp            |   Bin 23720 -> 0 bytes
 docs/media/v2/workflow/frames/0108.webp            |   Bin 23710 -> 0 bytes
 docs/media/v2/workflow/frames/0109.webp            |   Bin 12974 -> 0 bytes
 docs/media/v2/workflow/frames/0110.webp            |   Bin 12584 -> 0 bytes
 docs/media/v2/workflow/frames/0111.webp            |   Bin 14178 -> 0 bytes
 docs/media/v2/workflow/frames/0112.webp            |   Bin 15904 -> 0 bytes
 docs/media/v2/workflow/frames/0113.webp            |   Bin 17482 -> 0 bytes
 docs/media/v2/workflow/frames/0114.webp            |   Bin 19070 -> 0 bytes
 docs/media/v2/workflow/frames/0115.webp            |   Bin 20552 -> 0 bytes
 docs/media/v2/workflow/frames/0116.webp            |   Bin 22384 -> 0 bytes
 docs/media/v2/workflow/frames/0117.webp            |   Bin 23684 -> 0 bytes
 docs/media/v2/workflow/frames/0118.webp            |   Bin 25270 -> 0 bytes
 docs/media/v2/workflow/frames/0119.webp            |   Bin 26780 -> 0 bytes
 docs/media/v2/workflow/frames/0120.webp            |   Bin 28320 -> 0 bytes
 docs/media/v2/workflow/frames/0121.webp            |   Bin 28340 -> 0 bytes
 docs/media/v2/workflow/frames/0122.webp            |   Bin 28340 -> 0 bytes
 docs/media/v2/workflow/frames/0123.webp            |   Bin 20382 -> 0 bytes
 docs/media/v2/workflow/frames/0124.webp            |   Bin 20822 -> 0 bytes
 docs/media/v2/workflow/frames/0125.webp            |   Bin 23960 -> 0 bytes
 docs/media/v2/workflow/frames/0126.webp            |   Bin 27528 -> 0 bytes
 docs/media/v2/workflow/frames/0127.webp            |   Bin 31938 -> 0 bytes
 docs/media/v2/workflow/frames/0128.webp            |   Bin 35508 -> 0 bytes
 docs/media/v2/workflow/frames/0129.webp            |   Bin 35386 -> 0 bytes
 docs/media/v2/workflow/frames/0130.webp            |   Bin 35152 -> 0 bytes
 docs/media/v2/workflow/frames/0131.webp            |   Bin 34450 -> 0 bytes
 docs/media/v2/workflow/frames/0132.webp            |   Bin 34698 -> 0 bytes
 docs/media/v2/workflow/frames/0133.webp            |   Bin 34566 -> 0 bytes
 docs/media/v2/workflow/frames/0134.webp            |   Bin 34596 -> 0 bytes
 docs/media/v2/workflow/frames/0135.webp            |   Bin 34518 -> 0 bytes
 docs/media/v2/workflow/frames/0136.webp            |   Bin 34588 -> 0 bytes
 docs/media/v2/workflow/frames/0137.webp            |   Bin 4784 -> 0 bytes
 docs/media/v2/workflow/frames/0138.webp            |   Bin 6092 -> 0 bytes
 docs/media/v2/workflow/frames/0139.webp            |   Bin 5896 -> 0 bytes
 docs/media/v2/workflow/frames/0140.webp            |   Bin 4304 -> 0 bytes
 docs/media/v2/workflow/frames/0141.webp            |   Bin 4704 -> 0 bytes
 docs/media/v2/workflow/frames/0142.webp            |   Bin 3116 -> 0 bytes
 docs/media/v2/workflow/frames/0143.webp            |   Bin 2618 -> 0 bytes
 docs/media/v2/workflow/frames/0144.webp            |   Bin 2746 -> 0 bytes
 docs/media/v2/workflow/frames/0145.webp            |   Bin 11936 -> 0 bytes
 docs/media/v2/workflow/frames/0146.webp            |   Bin 11984 -> 0 bytes
 docs/media/v2/workflow/frames/0147.webp            |   Bin 11944 -> 0 bytes
 docs/media/v2/workflow/frames/0148.webp            |   Bin 11972 -> 0 bytes
 docs/media/v2/workflow/frames/0149.webp            |   Bin 11944 -> 0 bytes
 docs/media/v2/workflow/frames/0150.webp            |   Bin 11982 -> 0 bytes
 docs/media/v2/workflow/frames/0151.webp            |   Bin 11896 -> 0 bytes
 docs/media/v2/workflow/frames/0152.webp            |   Bin 11876 -> 0 bytes
 docs/media/v2/workflow/frames/0153.webp            |   Bin 12254 -> 0 bytes
 docs/media/v2/workflow/frames/0154.webp            |   Bin 12220 -> 0 bytes
 docs/media/v2/workflow/frames/0155.webp            |   Bin 12268 -> 0 bytes
 docs/media/v2/workflow/frames/0156.webp            |   Bin 12268 -> 0 bytes
 docs/media/v2/workflow/frames/0157.webp            |   Bin 12258 -> 0 bytes
 docs/media/v2/workflow/frames/0158.webp            |   Bin 12660 -> 0 bytes
 docs/media/v2/workflow/frames/0159.webp            |   Bin 12926 -> 0 bytes
 docs/media/v2/workflow/frames/0160.webp            |   Bin 13158 -> 0 bytes
 docs/media/v2/workflow/frames/0161.webp            |   Bin 13808 -> 0 bytes
 docs/media/v2/workflow/frames/0162.webp            |   Bin 14350 -> 0 bytes
 docs/media/v2/workflow/frames/0163.webp            |   Bin 14640 -> 0 bytes
 docs/media/v2/workflow/frames/0164.webp            |   Bin 14874 -> 0 bytes
 docs/media/v2/workflow/frames/0165.webp            |   Bin 15186 -> 0 bytes
 docs/media/v2/workflow/frames/0166.webp            |   Bin 15848 -> 0 bytes
 docs/media/v2/workflow/frames/0167.webp            |   Bin 15990 -> 0 bytes
 docs/media/v2/workflow/frames/0168.webp            |   Bin 16422 -> 0 bytes
 docs/media/v2/workflow/frames/0169.webp            |   Bin 18714 -> 0 bytes
 docs/media/v2/workflow/frames/0170.webp            |   Bin 18484 -> 0 bytes
 docs/media/v2/workflow/frames/0171.webp            |   Bin 18500 -> 0 bytes
 docs/media/v2/workflow/frames/0172.webp            |   Bin 18536 -> 0 bytes
 docs/media/v2/workflow/frames/0173.webp            |   Bin 18826 -> 0 bytes
 docs/media/v2/workflow/frames/0174.webp            |   Bin 18552 -> 0 bytes
 docs/media/v2/workflow/frames/0175.webp            |   Bin 18626 -> 0 bytes
 docs/media/v2/workflow/frames/0176.webp            |   Bin 18640 -> 0 bytes
 docs/media/v2/workflow/frames/0177.webp            |   Bin 18634 -> 0 bytes
 docs/media/v2/workflow/frames/0178.webp            |   Bin 18636 -> 0 bytes
 docs/media/v2/workflow/frames/0179.webp            |   Bin 18648 -> 0 bytes
 docs/media/v2/workflow/frames/0180.webp            |   Bin 18676 -> 0 bytes
 docs/media/v2/workflow/frames/0181.webp            |   Bin 18656 -> 0 bytes
 docs/media/v2/workflow/frames/0182.webp            |   Bin 18622 -> 0 bytes
 docs/media/v2/workflow/frames/0183.webp            |   Bin 18698 -> 0 bytes
 docs/media/v2/workflow/frames/0184.webp            |   Bin 18622 -> 0 bytes
 docs/media/v2/workflow/frames/0185.webp            |   Bin 12954 -> 0 bytes
 docs/media/v2/workflow/frames/0186.webp            |   Bin 12814 -> 0 bytes
 docs/media/v2/workflow/frames/0187.webp            |   Bin 12654 -> 0 bytes
 docs/media/v2/workflow/frames/0188.webp            |   Bin 13878 -> 0 bytes
 docs/media/v2/workflow/frames/0189.webp            |   Bin 13860 -> 0 bytes
 docs/media/v2/workflow/frames/0190.webp            |   Bin 13866 -> 0 bytes
 docs/media/v2/workflow/frames/0191.webp            |   Bin 13792 -> 0 bytes
 docs/media/v2/workflow/frames/0192.webp            |   Bin 13762 -> 0 bytes
 docs/media/v2/workflow/frames/0193.webp            |   Bin 13794 -> 0 bytes
 docs/media/v2/workflow/frames/0194.webp            |   Bin 12048 -> 0 bytes
 docs/media/v2/workflow/frames/0195.webp            |   Bin 10464 -> 0 bytes
 docs/media/v2/workflow/frames/0196.webp            |   Bin 8806 -> 0 bytes
 docs/media/v2/workflow/frames/0197.webp            |   Bin 7822 -> 0 bytes
 docs/media/v2/workflow/frames/0198.webp            |   Bin 7832 -> 0 bytes
 docs/media/v2/workflow/frames/0199.webp            |   Bin 7788 -> 0 bytes
 docs/media/v2/workflow/frames/0200.webp            |   Bin 7790 -> 0 bytes
 docs/media/v2/workflow/frames/0201.webp            |   Bin 36402 -> 0 bytes
 docs/media/v2/workflow/frames/0202.webp            |   Bin 14482 -> 0 bytes
 docs/media/v2/workflow/frames/0203.webp            |   Bin 14506 -> 0 bytes
 docs/media/v2/workflow/frames/0204.webp            |   Bin 14570 -> 0 bytes
 docs/media/v2/workflow/frames/0205.webp            |   Bin 14308 -> 0 bytes
 docs/media/v2/workflow/frames/0206.webp            |   Bin 14416 -> 0 bytes
 docs/media/v2/workflow/frames/0207.webp            |   Bin 14466 -> 0 bytes
 docs/media/v2/workflow/frames/0208.webp            |   Bin 14700 -> 0 bytes
 docs/media/v2/workflow/frames/0209.webp            |   Bin 14852 -> 0 bytes
 docs/media/v2/workflow/frames/0210.webp            |   Bin 14538 -> 0 bytes
 docs/media/v2/workflow/frames/0211.webp            |   Bin 14642 -> 0 bytes
 docs/media/v2/workflow/frames/0212.webp            |   Bin 15174 -> 0 bytes
 docs/media/v2/workflow/frames/0213.webp            |   Bin 14780 -> 0 bytes
 docs/media/v2/workflow/frames/0214.webp            |   Bin 15026 -> 0 bytes
 docs/media/v2/workflow/frames/0215.webp            |   Bin 14976 -> 0 bytes
 docs/media/v2/workflow/frames/0216.webp            |   Bin 14972 -> 0 bytes
 docs/media/v2/workflow/frames/0217.webp            |   Bin 11072 -> 0 bytes
 docs/media/v2/workflow/frames/0218.webp            |   Bin 25458 -> 0 bytes
 docs/media/v2/workflow/frames/0219.webp            |   Bin 29672 -> 0 bytes
 docs/media/v2/workflow/frames/0220.webp            |   Bin 34492 -> 0 bytes
 docs/media/v2/workflow/frames/0221.webp            |   Bin 34772 -> 0 bytes
 docs/media/v2/workflow/frames/0222.webp            |   Bin 34716 -> 0 bytes
 docs/media/v2/workflow/frames/0223.webp            |   Bin 34754 -> 0 bytes
 docs/media/v2/workflow/frames/0224.webp            |   Bin 34756 -> 0 bytes
 docs/media/v2/workflow/frames/0225.webp            |   Bin 34760 -> 0 bytes
 docs/media/v2/workflow/frames/0226.webp            |   Bin 34736 -> 0 bytes
 docs/media/v2/workflow/frames/0227.webp            |   Bin 34754 -> 0 bytes
 docs/media/v2/workflow/frames/0228.webp            |   Bin 34756 -> 0 bytes
 docs/media/v2/workflow/frames/0229.webp            |   Bin 34760 -> 0 bytes
 docs/media/v2/workflow/frames/0230.webp            |   Bin 34736 -> 0 bytes
 docs/media/v2/workflow/frames/0231.webp            |   Bin 34952 -> 0 bytes
 docs/media/v2/workflow/frames/0232.webp            |   Bin 34910 -> 0 bytes
 docs/media/v2/workflow/manifest.json               |    88 -
 docs/releasing.md                                  |    30 +-
 docs/sitemap.xml                                   |     2 +-
 ...red-values-object-for-slot-and-instance-swap.md |   175 +
 .../specs/v0.5/108-slot-content-id-per-property.md |   126 +
 ...09-paste-from-figma-remaps-slot-and-swap-ids.md |   102 +
 ...10-export-fig-with-figma-real-kiwi-numbering.md |   119 +
 .../111-reject-raw-number-slot-list-fallback.md    |   103 +
 ...oc-op-semantics-and-unknown-slot-config-keys.md |   110 +
 ...aceholder-preferred-picker-stretch-on-insert.md |   141 +
 ...te-instance-swap-or-boolean-regions-to-slots.md |   126 +
 ...2-emit-slots-in-codegen-and-dev-mode-inspect.md |   138 +
 ...hard-coded-numbers-matching-number-variables.md |   126 +
 ...-variables-and-styles-from-enabled-libraries.md |   125 +
 ...check-designs-scan-whole-pages-incrementally.md |   117 +
 ...designs-figma-rule-set-thresholds-and-ignore.md |   185 +
 ...9-read-only-component-playground-in-dev-mode.md |   126 +
 ...rify-vsf-pcp-slot-data-and-synthetic-fixture.md |   150 +
 ...2-slot-authoring-ui-create-slot-and-settings.md |   188 +
 ...-slot-content-in-instances-without-detaching.md |   150 +
 ...mport-global-clone-budget-and-review-backlog.md |   108 +
 ...lti-paragraph-and-mixed-style-text-overrides.md |    94 +
 ...ns-audit-text-and-overrides-inside-instances.md |   120 +
 ...nstances-apply-stretch-and-scale-constraints.md |   124 +
 ...5-nested-instance-swap-applies-variant-shell.md |    93 +
 ...er-contrast-over-opaque-parents-and-siblings.md |   120 +
 ...ecolour-text-runs-when-applying-colour-style.md |    89 +
 docs/specs/v0.5/README.md                          |   118 +
 ...nder-png-refuses-to-write-when-a-check-fails.md |   137 +
 .../897-usage-summary-section-in-get-app-state.md  |   180 +
 .../898-copy-properties-op-in-apply-batch.md       |   154 +
 ...-library-audit-rules-and-check-designs-group.md |   183 +
 .../v0.6.x/900-designer-change-feed-for-agents.md  |   183 +
 .../901-exec-js-script-tool-in-designos-mcp.md     |   157 +
 docs/specs/v0.6.x/README.md                        |    46 +
 ...check-designs-scan-whole-pages-incrementally.md |   117 +
 ...ck-designs-rule-parity-thresholds-and-ignore.md |   125 +
 ...mport-global-clone-budget-and-review-backlog.md |   146 +
 ...son-scene-format-schema-2-21-migration-chain.md |   288 +
 .../578-codegen-counter-fill-hugging-parent.md     |   141 +
 ...ntent-mark-for-mcp-live-patch-and-fig-import.md |   170 +
 ...eadless-direction-flip-single-stretch-signal.md |   139 +
 ...t-codegen-declare-variant-and-property-props.md |   165 +
 ...-codegen-grid-child-counter-fill-solved-size.md |   123 +
 ...stretch-on-drag-library-drop-and-media-place.md |   148 +
 ...te-to-slot-unbound-instance-in-a-variant-set.md |   137 +
 .../v0.6/651-agent-open-flow-resume-or-new.md      |   173 +
 ...-session-picker-installed-clis-version-login.md |   175 +
 .../v0.6/653-stable-per-file-agent-sessions.md     |   204 +
 .../v0.6/654-live-selection-chip-click-to-send.md  |   166 +
 .../655-at-mention-picker-in-the-agent-composer.md |   170 +
 ...-ask-permission-mode-with-in-panel-approvals.md |   200 +
 ...scan-cap-contrast-suppression-bound-override.md |   192 +
 .../specs/v0.6/669-v050-rollup-review-followups.md |   226 +
 ...failure-classify-output-before-not-installed.md |   139 +
 .../v0.6/677-agent-turn-review-keep-or-revert.md   |   197 +
 .../v0.6/678-agent-tab-token-and-cost-usage.md     |   159 +
 .../679-more-agent-clis-in-the-new-tab-picker.md   |   165 +
 .../v0.6/681-agents-panel-no-fake-agent-output.md  |   120 +
 ...2-agent-cursor-and-write-flash-on-the-canvas.md |   130 +
 ...tus-turn-receipt-and-background-notification.md |   138 +
 .../specs/v0.6/684-stop-an-agent-from-the-panel.md |   121 +
 ...e-document-while-an-agent-terminal-has-focus.md |    98 +
 ...sion-agent-failures-and-show-the-next-action.md |   120 +
 ...ssibility-announcements-and-labelled-regions.md |   105 +
 ...-empty-state-sample-prompts-and-attach-hints.md |   117 +
 ...gent-composer-send-into-the-live-cli-session.md |   124 +
 ...ion-paths-cleanup-and-off-ui-thread-snapshot.md |   113 +
 ...traint-pass-one-shared-solver-matching-figma.md |   137 +
 ...-edits-inside-a-frame-an-agent-is-working-on.md |   119 +
 ...changes-show-chip-instead-of-moving-the-view.md |   115 +
 ...ped-live-canvas-carries-variables-and-styles.md |   115 +
 .../v0.6/E1-4-styles-hold-variable-aliases.md      |   178 +
 docs/specs/v0.6/E1-9-variables-figma-json-dtcg.md  |   315 +
 docs/specs/v0.6/E15-1-apply-spec-instances.md      |   272 +
 .../v0.6/E15-10-wcag-contrast-resolved-backdrop.md |   170 +
 .../v0.6/E15-11-spacing-type-scale-token-lint.md   |   171 +
 docs/specs/v0.6/E15-12-per-call-issues-block.md    |   168 +
 docs/specs/v0.6/E15-13-engine-auto-fix.md          |   187 +
 .../v0.6/E15-14-auto-fix-settings-and-report-ui.md |   141 +
 docs/specs/v0.6/E15-15-generated-rules-pack.md     |   166 +
 docs/specs/v0.6/E15-16-first-turn-bootstrap.md     |   172 +
 .../v0.6/E15-17-streaming-large-document-query.md  |   181 +
 .../specs/v0.6/E15-18-style-archetype-catalogue.md |   206 +
 docs/specs/v0.6/E15-19-apply-style-compiler.md     |   155 +
 docs/specs/v0.6/E15-2-apply-spec-slot-fills.md     |   166 +
 docs/specs/v0.6/E15-20-style-preset-gallery.md     |   108 +
 .../E15-21-surface-guides-and-lint-profiles.md     |   174 +
 ...E15-22-surface-templates-and-slide-contracts.md |   152 +
 .../E15-23-eval-and-skills-licence-register.md     |   156 +
 ...15-24-deterministic-large-document-generator.md |   121 +
 docs/specs/v0.6/E15-25-designos-eval-harness.md    |   162 +
 .../v0.6/E15-26-deterministic-eval-metrics.md      |   133 +
 .../v0.6/E15-27-suite-s1-recreate-fidelity.md      |   109 +
 docs/specs/v0.6/E15-28-suite-s2-brief-to-screen.md |   109 +
 .../v0.6/E15-29-suite-s3-large-file-edits-drift.md |   110 +
 .../v0.6/E15-3-apply-spec-variable-bindings.md     |   267 +
 .../specs/v0.6/E15-30-suite-s4-flow-consistency.md |   107 +
 docs/specs/v0.6/E15-31-checklist-pairwise-judge.md |   120 +
 .../v0.6/E15-32-suite-s5-judge-calibration.md      |   127 +
 docs/specs/v0.6/E15-33-ci-eval-regression-gate.md  |   126 +
 docs/specs/v0.6/E15-34-rubric-critique.md          |   127 +
 .../v0.6/E15-35-scoped-repair-regression-guard.md  |   119 +
 docs/specs/v0.6/E15-36-variant-explorer.md         |   124 +
 docs/specs/v0.6/E15-37-rank-and-merge-variants.md  |   107 +
 docs/specs/v0.6/E15-38-variant-explorer-ui.md      |   100 +
 docs/specs/v0.6/E15-39-generate-image-assets.md    |   113 +
 docs/specs/v0.6/E15-4-apply-batch-transactions.md  |   236 +
 docs/specs/v0.6/E15-40-stock-images-attribution.md |   105 +
 docs/specs/v0.6/E15-41-stream-generated-svg.md     |    96 +
 .../v0.6/E15-42-import-css-tailwind-tokens.md      |   140 +
 docs/specs/v0.6/E15-43-web-capture-css-tokens.md   |   109 +
 .../v0.6/E15-44-web-import-component-detection.md  |   102 +
 docs/specs/v0.6/E15-45-verify-generated-code.md    |   107 +
 docs/specs/v0.6/E15-46-adopt-mit-ui-skills.md      |   111 +
 docs/specs/v0.6/E15-47-designos-skills-pack.md     |   100 +
 .../v0.6/E15-48-apply-spec-image-fills-effects.md  |   165 +
 .../v0.6/E15-5-named-errors-structured-output.md   |   200 +
 docs/specs/v0.6/E15-6-idempotency-keys.md          |   258 +
 docs/specs/v0.6/E15-7-in-progress-flag.md          |   188 +
 docs/specs/v0.6/E15-8-before-after-diff-images.md  |   129 +
 docs/specs/v0.6/E15-9-unified-geometric-linter.md  |   269 +
 docs/specs/v0.6/E5-10-agent-cli-image-tools.md     |   126 +
 .../v0.6/RD-3-parallel-agents-frame-ownership.md   |   199 +
 docs/specs/v0.6/RD-4-skills-attachments.md         |   128 +
 docs/specs/v0.6/README-part2.md                    |   123 +
 docs/specs/v0.6/README-part3.md                    |   162 +
 docs/specs/v0.6/README-part4.md                    |   198 +
 docs/specs/v0.6/README.md                          |   183 +
 docs/specs/v0.6/T10.1-branch-storage.md            |   117 +
 docs/specs/v0.6/T10.2-three-way-diff.md            |   150 +
 docs/specs/v0.6/T10.3-branch-merge.md              |   123 +
 docs/specs/v0.6/T7.1-get-screenshot.md             |   148 +
 docs/specs/v0.6/T7.2-get-metadata.md               |   139 +
 docs/specs/v0.6/T7.29-html-codegen.md              |   119 +
 docs/specs/v0.6/T7.3-shared-design-context-data.md |   104 +
 docs/specs/v0.6/T7.38-get-libraries.md             |   120 +
 docs/specs/v0.6/T7.4-get-design-context.md         |   140 +
 .../specs/v0.6/T7.40-design-system-rules-prompt.md |   154 +
 docs/specs/v0.6/T7.6-mcp-parity-matrix.md          |   118 +
 docs/specs/v0.6/T7.9-search-design-system.md       |   148 +
 docs/specs/v0.6/T8.25-web-capture-script.md        |   140 +
 docs/specs/v0.6/T8.27-web-capture-import.md        |   146 +
 .../161-blend-modes-on-drop-and-inner-shadows.md   |   140 +
 .../162-cap-effect-counts-in-the-plus-control.md   |   127 +
 ...ill-adjustments-rotation-tile-scale-and-crop.md |   167 +
 .../specs/v0.7/166-licence-gate-with-cargo-deny.md |   126 +
 ...-mcp-targets-the-editor-selection-by-default.md |   151 +
 .../244-whoami-mcp-tool-returns-the-local-user.md  |   120 +
 ...new-file-without-replacing-the-open-document.md |   134 +
 docs/specs/v0.7/249-vue-codegen-target.md          |   149 +
 .../v0.7/251-web-components-codegen-target.md      |   157 +
 docs/specs/v0.7/252-flutter-codegen-target.md      |   174 +
 ...plugin-compatibility-corpus-of-eight-plugins.md |   129 +
 ...-runtime-spike-quickjs-worker-and-design-doc.md |   148 +
 ...ressive-blur-model-render-and-fig-round-trip.md |   176 +
 ...splay-bevel-noise-xy-size-and-texture-parity.md |   220 +
 ...preserve-custom-shader-effect-and-paint-refs.md |   146 +
 ...-effects-differ-from-figma-on-device-mockups.md |   131 +
 ...fills-variable-mode-image-and-crop-transform.md |   145 +
 .../528-export-bounds-match-figma-per-side-rule.md |   159 +
 docs/specs/v0.7/README-part2.md                    |   150 +
 docs/specs/v0.7/README.md                          |   144 +
 ...ignos-studio-launch-hero-beat-contact-sheet.png |   Bin 1090875 -> 0 bytes
 docs/videos/01-designos-studio-launch-hero.mp4     |   Bin 7460151 -> 0 bytes
 ...a-100-micro-interactions-beat-contact-sheet.png |   Bin 1034809 -> 0 bytes
 docs/videos/02-figma-100-micro-interactions.mp4    |   Bin 7082596 -> 0 bytes
 ...-first-specs-and-devmode-beat-contact-sheet.png |   Bin 925126 -> 0 bytes
 docs/videos/03-agent-first-specs-and-devmode.mp4   |   Bin 6116440 -> 0 bytes
 docs/videos/designos-studio-launch-preview.gif     |   Bin 8617435 -> 0 bytes
 docs/videos/designos-studio-promo-16x9-poster.png  |   Bin 45462 -> 0 bytes
 docs/videos/designos-studio-promo-16x9.mp4         |   Bin 24889631 -> 0 bytes
 docs/videos/designos-studio-promo-9x16-poster.png  |   Bin 62091 -> 0 bytes
 docs/videos/designos-studio-promo-9x16.mp4         |   Bin 19319811 -> 0 bytes
 docs/videos/designos-studio-promo-teaser.gif       |   Bin 5996091 -> 0 bytes
 docs/videos/micro-01-autolayout-5-16x9.gif         |   Bin 1160207 -> 0 bytes
 docs/videos/micro-01-autolayout-5-16x9.mp4         |   Bin 3318459 -> 0 bytes
 docs/videos/micro-01-autolayout-5-9x16.gif         |   Bin 1046080 -> 0 bytes
 docs/videos/micro-01-autolayout-5-9x16.mp4         |   Bin 3210187 -> 0 bytes
 docs/videos/micro-02-css-grid-16x9.gif             |   Bin 1447865 -> 0 bytes
 docs/videos/micro-02-css-grid-16x9.mp4             |   Bin 3751679 -> 0 bytes
 docs/videos/micro-02-css-grid-9x16.gif             |   Bin 1269717 -> 0 bytes
 docs/videos/micro-02-css-grid-9x16.mp4             |   Bin 3524491 -> 0 bytes
 docs/videos/micro-03-smart-selection-16x9.gif      |   Bin 1316199 -> 0 bytes
 docs/videos/micro-03-smart-selection-16x9.mp4      |   Bin 3244799 -> 0 bytes
 docs/videos/micro-03-smart-selection-9x16.gif      |   Bin 1176962 -> 0 bytes
 docs/videos/micro-03-smart-selection-9x16.mp4      |   Bin 3134056 -> 0 bytes
 docs/videos/micro-04-shape-controls-16x9.gif       |   Bin 926336 -> 0 bytes
 docs/videos/micro-04-shape-controls-16x9.mp4       |   Bin 2966919 -> 0 bytes
 docs/videos/micro-04-shape-controls-9x16.gif       |   Bin 862469 -> 0 bytes
 docs/videos/micro-04-shape-controls-9x16.mp4       |   Bin 2858403 -> 0 bytes
 docs/videos/micro-05-vector-network-16x9.gif       |   Bin 925801 -> 0 bytes
 docs/videos/micro-05-vector-network-16x9.mp4       |   Bin 2967239 -> 0 bytes
 docs/videos/micro-05-vector-network-9x16.gif       |   Bin 960832 -> 0 bytes
 docs/videos/micro-05-vector-network-9x16.mp4       |   Bin 3021605 -> 0 bytes
 docs/videos/micro-06-layer-eyedropper-16x9.gif     |   Bin 2203011 -> 0 bytes
 docs/videos/micro-06-layer-eyedropper-16x9.mp4     |   Bin 7302973 -> 0 bytes
 docs/videos/micro-06-layer-eyedropper-9x16.gif     |   Bin 1840280 -> 0 bytes
 docs/videos/micro-06-layer-eyedropper-9x16.mp4     |   Bin 6041800 -> 0 bytes
 docs/videos/micro-07-devmode-boxmodel-16x9.gif     |   Bin 6766837 -> 0 bytes
 docs/videos/micro-07-devmode-boxmodel-16x9.mp4     |   Bin 20204586 -> 0 bytes
 docs/videos/micro-07-devmode-boxmodel-9x16.gif     |   Bin 6086727 -> 0 bytes
 docs/videos/micro-07-devmode-boxmodel-9x16.mp4     |   Bin 22109154 -> 0 bytes
 docs/videos/micro-08-scale-navigation-16x9.gif     |   Bin 1345631 -> 0 bytes
 docs/videos/micro-08-scale-navigation-16x9.mp4     |   Bin 3447935 -> 0 bytes
 docs/videos/micro-08-scale-navigation-9x16.gif     |   Bin 1238773 -> 0 bytes
 docs/videos/micro-08-scale-navigation-9x16.mp4     |   Bin 3296618 -> 0 bytes
 docs/videos/micro-09-inspector-math-16x9.gif       |   Bin 915839 -> 0 bytes
 docs/videos/micro-09-inspector-math-16x9.mp4       |   Bin 2964856 -> 0 bytes
 docs/videos/micro-09-inspector-math-9x16.gif       |   Bin 876590 -> 0 bytes
 docs/videos/micro-09-inspector-math-9x16.mp4       |   Bin 2881534 -> 0 bytes
 docs/videos/micro-10-selection-corner-16x9.gif     |   Bin 845791 -> 0 bytes
 docs/videos/micro-10-selection-corner-16x9.mp4     |   Bin 2908602 -> 0 bytes
 docs/videos/micro-10-selection-corner-9x16.gif     |   Bin 898074 -> 0 bytes
 docs/videos/micro-10-selection-corner-9x16.mp4     |   Bin 3012308 -> 0 bytes
 docs/videos/micro-11-typography-multiedit-16x9.gif |   Bin 948462 -> 0 bytes
 docs/videos/micro-11-typography-multiedit-16x9.mp4 |   Bin 2982193 -> 0 bytes
 docs/videos/micro-11-typography-multiedit-9x16.gif |   Bin 852086 -> 0 bytes
 docs/videos/micro-11-typography-multiedit-9x16.mp4 |   Bin 2764534 -> 0 bytes
 docs/videos/micro-12-components-variables-16x9.gif |   Bin 1219474 -> 0 bytes
 docs/videos/micro-12-components-variables-16x9.mp4 |   Bin 3373717 -> 0 bytes
 docs/videos/micro-12-components-variables-9x16.gif |   Bin 1082552 -> 0 bytes
 docs/videos/micro-12-components-variables-9x16.mp4 |   Bin 3215062 -> 0 bytes
 examples/landing/close-on-canvas.png               |   Bin 331432 -> 0 bytes
 examples/landing/compare-on-canvas.png             |   Bin 337389 -> 0 bytes
 examples/landing/designos-studio-landing.designos  |     2 +-
 examples/landing/hero-on-canvas.png                |   Bin 285471 -> 0 bytes
 examples/landing/mechanism-on-canvas.png           |   Bin 357031 -> 0 bytes
 examples/landing/page-on-canvas.png                |   Bin 209116 -> 0 bytes
 examples/specs/designos-studio-landing.spec.json   |    52 +-
 examples/specs/easeui-agent-console.spec.json      |    33 +-
 llms.txt                                           |     2 +-
 packaging/macos/Info.plist.in                      |     2 +-
 packaging/windows/install.ps1                      |     2 +-
 packaging/windows/installer.nsi                    |     5 +-
 plans/261017-epics-full-scope/add_field_options.py |     4 +-
 plans/261017-epics-full-scope/build_e15_tasks.py   |   825 +
 plans/261017-epics-full-scope/e15-tasks.json       |   941 ++
 plans/261017-epics-full-scope/file_e15_tasks.py    |   104 +
 plans/261017-epics-full-scope/filed-state.json     |   113 +-
 .../reports/final-gates.png                        |   Bin 28715 -> 0 bytes
 .../screens/S2/after-effect-style-linked.png       |   Bin 309118 -> 0 bytes
 .../screens/S2/after-effect-style-menu.png         |   Bin 315925 -> 0 bytes
 .../screens/S2/after-grid-style-menu.png           |   Bin 336346 -> 0 bytes
 .../screens/S2/after-local-styles.png              |   Bin 284236 -> 0 bytes
 .../screens/S2/before-effects.png                  |   Bin 305533 -> 0 bytes
 .../screens/s1-paste-figma/after.png               |   Bin 100412 -> 0 bytes
 .../screens/s1-paste-figma/before.png              |   Bin 93937 -> 0 bytes
 .../screens/s3-pdf/after-export-panel-pdf-row.png  |   Bin 164056 -> 0 bytes
 .../screens/s3-pdf/before-export-panel-pdf-row.png |   Bin 164056 -> 0 bytes
 .../s3-pdf/sample-2-frames-page1-quicklook.png     |   Bin 9682 -> 0 bytes
 .../screens/s3-pdf/sample-2-frames-render-1.png    |   Bin 695 -> 0 bytes
 .../screens/s3-pdf/sample-2-frames-render-2.png    |   Bin 695 -> 0 bytes
 .../after-panel-vertical-wrap-space-around.png     |   Bin 165273 -> 0 bytes
 .../screens/s4-layout-text/after-type-settings.png |   Bin 219877 -> 0 bytes
 .../before-panel-vertical-wrap-space-around.png    |   Bin 165365 -> 0 bytes
 .../s4-layout-text/before-type-settings.png        |   Bin 215703 -> 0 bytes
 .../s4-layout-text/compare-auto-layout-flow.png    |   Bin 127479 -> 0 bytes
 .../compare-canvas-text-auto-vs-balance.png        |   Bin 49776 -> 0 bytes
 .../compare-type-settings-wrap-style.png           |   Bin 101025 -> 0 bytes
 .../s5-vars-codegen/after-dev-android-xml.png      |   Bin 266231 -> 0 bytes
 .../s5-vars-codegen/after-dev-css-tokens-crop.png  |   Bin 122883 -> 0 bytes
 .../s5-vars-codegen/after-dev-css-tokens.png       |   Bin 539862 -> 0 bytes
 .../screens/s5-vars-codegen/after-dev-uikit.png    |   Bin 269645 -> 0 bytes
 .../s5-vars-codegen/before-dev-css-tokens-crop.png |   Bin 119983 -> 0 bytes
 .../s5-vars-codegen/before-dev-css-tokens.png      |   Bin 537157 -> 0 bytes
 .../s6-style-edit-ui/after-edit-style-color.png    |   Bin 527586 -> 0 bytes
 .../s6-style-edit-ui/after-edit-style-effect.png   |   Bin 553071 -> 0 bytes
 .../s6-style-edit-ui/after-edit-style-grid.png     |   Bin 711328 -> 0 bytes
 .../s6-style-edit-ui/after-edit-style-text.png     |   Bin 541153 -> 0 bytes
 .../s6-style-edit-ui/after-effect-styles-menu.png  |   Bin 538746 -> 0 bytes
 .../after-instance-own-text-refit.png              |   Bin 108795 -> 0 bytes
 .../after-library-style-edit-disabled.png          |   Bin 222022 -> 0 bytes
 .../s6-style-edit-ui/before-effect-styles-menu.png |   Bin 535190 -> 0 bytes
 .../before-instance-own-text-refit.png             |   Bin 108130 -> 0 bytes
 .../task30-solver-unify/after/autolayout-step1.png |   Bin 170310 -> 0 bytes
 .../task30-solver-unify/after/css-grid-step1.png   |   Bin 220357 -> 0 bytes
 .../task30-solver-unify/after/css-grid-step3.png   |   Bin 233068 -> 0 bytes
 .../task30-solver-unify/after/css-grid-step4.png   |   Bin 230986 -> 0 bytes
 .../before/autolayout-step1.png                    |   Bin 170310 -> 0 bytes
 .../task30-solver-unify/before/css-grid-step1.png  |   Bin 217947 -> 0 bytes
 .../task30-solver-unify/before/css-grid-step3.png  |   Bin 230774 -> 0 bytes
 .../task30-solver-unify/before/css-grid-step4.png  |   Bin 227313 -> 0 bytes
 .../screens/1-rectangle-before-drag.jpg            |   Bin 31603 -> 0 bytes
 .../screens/2-after-drag-rename-not-yet-drawn.jpg  |   Bin 84910 -> 0 bytes
 .../screens/3-agent-rename-landed-after-drag.jpg   |   Bin 85769 -> 0 bytes
 .../screens/4-picker-open-agent-rename-held.jpg    |   Bin 102592 -> 0 bytes
 .../5-picker-closed-agent-rename-landed.jpg        |   Bin 88250 -> 0 bytes
 .../agent-terminal-ux-tasks.json                   |    31 +
 .../file_agent_terminal_ux_tasks.py                |    57 +
 plans/261019-agent-terminal-ux/filed-state.json    |    14 +
 .../anatomy-trace-layers-colored.png               |   Bin 0 -> 102347 bytes
 .../anatomy-trace-overlay-1440.png                 |   Bin 0 -> 365381 bytes
 .../anatomy-trace-strokes-1440.png                 |   Bin 0 -> 95578 bytes
 .../full-page-static-1440.jpg                      |   Bin 0 -> 784784 bytes
 .../full-page-static-375.jpg                       |   Bin 0 -> 513634 bytes
 .../interaction/iter-copied.png                    |   Bin 0 -> 259051 bytes
 .../interaction/iter-faq-open.png                  |   Bin 0 -> 95125 bytes
 .../interaction/iter-focus-ring.png                |   Bin 0 -> 49655 bytes
 .../interaction/iter-tab-build.png                 |   Bin 0 -> 157445 bytes
 .../interaction/iter-whats-new-open.png            |   Bin 0 -> 104821 bytes
 .../logs/anatomy-capture-061.txt                   |    16 +
 .../logs/anatomy-trace-061.txt                     |     9 +
 .../agy-site-v2-060-evidence/logs/axe-1440.txt     |    10 +
 .../agy-site-v2-060-evidence/logs/axe-375.txt      |    10 +
 .../logs/interaction-test.diff                     |    90 +
 .../logs/interaction-test.txt                      |    17 +
 .../logs/motion-scroll-errors.json                 |     1 +
 .../logs/probe-render-summary.txt                  |     0
 .../agy-site-v2-060-evidence/logs/probe-render.txt |    27 +
 .../logs/release-sync-061-idempotent.txt           |     1 +
 .../logs/section-heights-1440.json                 |    15 +
 .../logs/section-heights-375.json                  |    15 +
 .../agy-site-v2-060-evidence/logs/self-check.txt   |     1 +
 .../agy-site-v2-060-evidence/logs/slop-file.txt    |    18 +
 .../logs/slop-rendered-1440.txt                    |    44 +
 .../agy-site-v2-060-evidence/logs/tenant-lint.txt  |     1 +
 .../agy-site-v2-060-evidence/logs/ui-gate.txt      |    18 +
 .../logs/unittest-mismatch-fail.txt                |    54 +
 .../agy-site-v2-060-evidence/logs/unittest.txt     |    44 +
 .../agy-site-v2-060-evidence/logs/violet.txt       |     1 +
 .../agy-site-v2-060-evidence/logs/zero-fake-ui.txt |    25 +
 .../motion-02-proof-mid-1440x900.png               |   Bin 0 -> 173930 bytes
 .../motion-03-anatomy-exploded-1440x900.png        |   Bin 0 -> 240419 bytes
 .../motion-04-workflow-ch1-figma-1440x900.png      |   Bin 0 -> 245647 bytes
 .../motion-04-workflow-ch2-agents-1440x900.png     |   Bin 0 -> 285109 bytes
 .../motion-04-workflow-ch3-code-1440x900.png       |   Bin 0 -> 285004 bytes
 .../motion-05-scale-mid-1440x900.png               |   Bin 0 -> 169864 bytes
 .../motion-05-scale-overview-1440x900.png          |   Bin 0 -> 141383 bytes
 .../static-01-hero-1440x900.png                    |   Bin 0 -> 141631 bytes
 .../static-02-proof-1440x900.png                   |   Bin 0 -> 174145 bytes
 .../static-03-anatomy-1440x900.png                 |   Bin 0 -> 324205 bytes
 .../static-04-workflow-1440x900.png                |   Bin 0 -> 216091 bytes
 .../static-05-scale-1440x900.png                   |   Bin 0 -> 211221 bytes
 .../static-06-new-1440x900.png                     |   Bin 0 -> 274828 bytes
 .../static-07-install-1440x900.png                 |   Bin 0 -> 154805 bytes
 .../static-08-faq-get-foot-1440x900.png            |   Bin 0 -> 99013 bytes
 plans/reports/agy-site-v2-060.md                   |   169 +
 .../cli-help-261019/gemini-help.txt                |    40 +
 .../cli-help-261019/gemini-mcp-add-help.txt        |    18 +
 .../cli-help-261019/opencode-help.txt              |    41 +
 .../01-panel-terminal-agents-panel-no-dock.png     |   Bin 0 -> 261844 bytes
 .../02-default-no-panel-no-dock.png                |   Bin 0 -> 231379 bytes
 .../remove-dock-838/03-panel-agents-no-dock.png    |   Bin 0 -> 263935 bytes
 prompts/doctor.md                                  |     2 +-
 .../__pycache__/zero_fake_ui_gate.cpython-314.pyc  |   Bin 27590 -> 0 bytes
 scripts/build-micro-8x30s-videos.mjs               |     2 +-
 scripts/build-svg-animation-videos.mjs             |  1971 ---
 scripts/capture-visual-proof.sh                    |   268 +-
 scripts/check_eval_licences.py                     |   461 +
 scripts/check_spacing_relationships.py             |   203 +
 scripts/check_vendor_skills.py                     |   671 +
 scripts/design_os_self_evolve.py                   |   178 +-
 scripts/figma_parity_meter.py                      |     2 +-
 scripts/non_colour_token_parity.py                 |   148 +
 scripts/package-macos.sh                           |     3 +-
 scripts/package.sh                                 |     8 +-
 scripts/release-smoke-macos.sh                     |   515 +
 scripts/release-sync-pages.py                      |   328 +
 scripts/render-svg-animation-videos.mjs            |   279 -
 scripts/rust_literal_scan.py                       |   317 +
 scripts/self_check.py                              |     4 -
 scripts/spacing_baseline.json                      |   323 +
 scripts/test_check_eval_licences.py                |   263 +
 scripts/test_check_vendor_skills.py                |   277 +
 scripts/tests/fixtures/release-sync/README.md      |    17 +
 scripts/tests/fixtures/release-sync/index.html     |    54 +
 scripts/tests/fixtures/release-sync/install.md     |     7 +
 scripts/tests/fixtures/release-sync/llms.txt       |     1 +
 .../release-sync/release-0.4.0-mac-only.json       |    23 +
 .../release-sync/release-0.4.0-no-dmg.json         |    31 +
 .../release-sync/release-0.4.0-prerelease.json     |    35 +
 .../tests/fixtures/release-sync/release-0.4.0.json |    35 +
 scripts/tests/fixtures/release-sync/sitemap.xml    |     9 +
 scripts/tests/fixtures/spacing/baseline_clean.json |     1 +
 scripts/tests/fixtures/spacing/baseline_empty.json |     1 +
 .../fixtures/spacing/clean/concentric_pair.rs      |     6 +
 scripts/tests/fixtures/spacing/clean/theme.rs      |     4 +
 .../tests/fixtures/spacing/clean/uses_constants.rs |     7 +
 scripts/tests/fixtures/spacing/dirty/bad_font.rs   |     3 +
 scripts/tests/fixtures/spacing/dirty/bad_inset.rs  |     3 +
 scripts/tests/fixtures/spacing/dirty/bad_offset.rs |     3 +
 scripts/tests/fixtures/spacing/dirty/bad_pair.rs   |     6 +
 scripts/tests/fixtures/spacing/dirty/bad_radius.rs |     3 +
 scripts/tests/fixtures/spacing/dirty/theme.rs      |     4 +
 scripts/tests/test_check_spacing_relationships.py  |   116 +
 scripts/tests/test_non_colour_token_parity.py      |    98 +
 scripts/tests/test_release_sync_pages.py           |   276 +
 scripts/ux_writing_baseline.json                   |    89 +
 scripts/ux_writing_labels.py                       |   331 +
 scripts/zero_fake_ui_gate.py                       |   226 +-
 scripts/zero_fake_ui_gate_selftest.py              |   197 +
 skills/design-os-studio/SKILL.md                   |    58 +-
 skills/design-os-studio/rules.md                   |   359 +
 skills/vendor/jakubkrehel-skills/ATTRIBUTION.md    |     1 +
 skills/vendor/jakubkrehel-skills/LICENSE           |    21 +
 skills/vendor/jakubkrehel-skills/UPSTREAM.toml     |    97 +
 skills/vendor/jakubkrehel-skills/canvas-rules.md   |   206 +
 skills/vendor/jakubkrehel-skills/rule-map.toml     |  1641 ++
 skills/vendor/jakubkrehel-skills/rules.lock        |   275 +
 skills/vendor/jakubkrehel-skills/upstream/LICENSE  |    21 +
 .../vendor/jakubkrehel-skills/upstream/README.md   |    39 +
 .../upstream/skills/better-accessibility/SKILL.md  |   140 +
 .../skills/better-accessibility/agents/openai.yaml |     3 +
 .../better-accessibility/focus-and-keyboard.md     |   151 +
 .../upstream/skills/better-accessibility/forms.md  |    77 +
 .../skills/better-accessibility/hit-areas.md       |    93 +
 .../skills/better-accessibility/motion-and-zoom.md |    48 +
 .../skills/better-accessibility/screen-readers.md  |    99 +
 .../better-accessibility/semantics-and-aria.md     |    81 +
 .../upstream/skills/better-colors/SKILL.md         |    95 +
 .../skills/better-colors/agents/openai.yaml        |     3 +
 .../upstream/skills/better-colors/color-formats.md |    90 +
 .../upstream/skills/better-colors/color-usage.md   |   109 +
 .../upstream/skills/better-colors/contrast.md      |    70 +
 .../skills/better-colors/palette-generation.md     |   108 +
 .../skills/better-colors/palette-structure.md      |    72 +
 .../upstream/skills/better-colors/token-naming.md  |    99 +
 .../upstream/skills/better-interface/SKILL.md      |   134 +
 .../skills/better-interface/agents/openai.yaml     |     3 +
 .../skills/better-interface/review-format.md       |    46 +
 .../upstream/skills/better-layout/SKILL.md         |    97 +
 .../skills/better-layout/agents/openai.yaml        |     3 +
 .../skills/better-layout/grouping-and-alignment.md |   103 +
 .../skills/better-layout/spacing-and-adaptivity.md |   156 +
 .../upstream/skills/better-typography/SKILL.md     |   165 +
 .../skills/better-typography/agents/openai.yaml    |     3 +
 .../skills/better-typography/choosing-fonts.md     |    53 +
 .../skills/better-typography/css-cheat-sheet.md    |    67 +
 .../better-typography/details-and-accessibility.md |    84 +
 .../skills/better-typography/spacing-and-sizing.md |    87 +
 .../variable-fonts-and-opentype.md                 |   100 +
 .../better-typography/wrapping-and-punctuation.md  |    23 +
 .../upstream/skills/better-ui/SKILL.md             |   133 +
 .../upstream/skills/better-ui/agents/openai.yaml   |     3 +
 .../upstream/skills/better-ui/animations.md        |   170 +
 .../upstream/skills/better-ui/enter-exit.md        |   157 +
 .../upstream/skills/better-ui/icon-transitions.md  |    72 +
 .../upstream/skills/better-ui/icons.md             |   105 +
 .../upstream/skills/better-ui/performance.md       |    63 +
 .../upstream/skills/better-ui/surfaces.md          |   166 +
 .../upstream/skills/better-writing/SKILL.md        |   151 +
 .../skills/better-writing/agents/openai.yaml       |     3 +
 .../upstream/skills/better-writing/patterns.md     |    56 +
 .../upstream/skills/break/SKILL.md                 |    85 +
 .../upstream/skills/break/agents/openai.yaml       |     5 +
 .../upstream/skills/break/scenarios.md             |    78 +
 .../upstream/skills/build-design/SKILL.md          |    83 +
 .../skills/build-design/agents/openai.yaml         |     3 +
 .../upstream/skills/build-design/figma.md          |    29 +
 .../upstream/skills/explain-interface/SKILL.md     |   106 +
 .../skills/explain-interface/agents/openai.yaml    |     5 +
 .../skills/explain-interface/find-the-effect.md    |   111 +
 .../skills/explain-interface/from-an-image.md      |    57 +
 .../skills/explain-interface/no-browser.md         |    70 +
 .../skills/explain-interface/read-the-system.md    |   201 +
 .../upstream/skills/interface-review/SKILL.md      |   151 +
 .../skills/interface-review/agents/openai.yaml     |     5 +
 .../skills/interface-review/removed-signals.md     |    43 +
 .../skills/interface-review/scope-resolution.md    |    92 +
 .../upstream/skills/state-machine/SKILL.md         |    90 +
 .../skills/state-machine/agents/openai.yaml        |     5 +
 .../upstream/skills/state-machine/switcher.md      |   205 +
 .../upstream/skills/technical-drawing/SKILL.md     |   142 +
 .../skills/technical-drawing/agents/openai.yaml    |     3 +
 .../skills/technical-drawing/assets/export.js      |   117 +
 .../skills/technical-drawing/assets/geometry.js    |   324 +
 .../skills/technical-drawing/assets/projection.js  |   140 +
 .../technical-drawing/assets/render-queue.js       |    81 +
 .../skills/technical-drawing/assets/theme.js       |    29 +
 .../skills/technical-drawing/assets/viewer.html    |   111 +
 .../skills/technical-drawing/assets/viewer.js      |   417 +
 .../skills/technical-drawing/assets/worker.js      |     8 +
 .../skills/technical-drawing/geometry-library.md   |   114 +
 .../upstream/skills/technical-drawing/geometry.md  |    83 +
 .../skills/technical-drawing/preview-page.md       |   164 +
 .../technical-drawing/scripts/build-viewer.cjs     |    74 +
 .../technical-drawing/scripts/test-geometry.cjs    |   128 +
 .../scripts/test-render-queue.cjs                  |   103 +
 .../technical-drawing/scripts/test-theme.cjs       |    32 +
 .../scripts/test-viewer-builder.cjs                |    29 +
 .../skills/technical-drawing/sheet-and-svg.md      |    99 +
 .../skills/technical-drawing/verification.md       |    50 +
 .../skills/technical-drawing/viewer-assets.md      |    92 +
 .../technical-drawing/views-and-annotations.md     |    73 +
 .../upstream/skills/variant/SKILL.md               |   117 +
 .../upstream/skills/variant/agents/openai.yaml     |     5 +
 .../upstream/skills/variant/picker.md              |   205 +
 .../agent-failures/antigravity-billing.txt         |     1 +
 .../agent-failures/antigravity-context-full.txt    |     1 +
 .../agent-failures/antigravity-login-expired.txt   |     1 +
 .../agent-failures/antigravity-network-offline.txt |     1 +
 .../agent-failures/antigravity-rate-limited.txt    |     1 +
 .../agent-failures/antigravity-server-error.txt    |     1 +
 tests/fixtures/agent-failures/claude-billing.txt   |     1 +
 .../agent-failures/claude-context-full.txt         |     1 +
 .../agent-failures/claude-login-expired.txt        |     1 +
 .../agent-failures/claude-network-offline.txt      |     1 +
 .../agent-failures/claude-rate-limited.txt         |     1 +
 .../agent-failures/claude-server-error.txt         |     1 +
 tests/fixtures/agent-failures/codex-billing.txt    |     1 +
 .../fixtures/agent-failures/codex-context-full.txt |     1 +
 .../agent-failures/codex-login-expired.txt         |     1 +
 .../agent-failures/codex-network-offline.txt       |     1 +
 .../fixtures/agent-failures/codex-rate-limited.txt |     1 +
 .../fixtures/agent-failures/codex-server-error.txt |     1 +
 tests/fixtures/agent-failures/grok-billing.txt     |     1 +
 .../fixtures/agent-failures/grok-context-full.txt  |     1 +
 .../fixtures/agent-failures/grok-login-expired.txt |     1 +
 .../agent-failures/grok-network-offline.txt        |     1 +
 .../fixtures/agent-failures/grok-rate-limited.txt  |     1 +
 .../fixtures/agent-failures/grok-server-error.txt  |     1 +
 2030 files changed, 252235 insertions(+), 20400 deletions(-)
```
