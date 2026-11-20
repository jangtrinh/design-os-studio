---
id: design-tokens
type: design
title: "Stage 8 Design System & Token Mapping — Figma UI3, product:os Operate, EaseUI & Unified Phosphor Icons"
status: Verified
updated: 2026-10-15
owner_role: Developer
design_system: "figma-ui3-dark + pcp-dark-operate-v2 + phosphor-icons-core"
trace:
  decisions: [D2, D6, D9, D10, D14, D15, D17]
  us: [US-02, US-04, US-05, US-06, US-08, US-09, US-10, US-11, US-12, US-13, US-14]
---

This document defines the Design Token mapping table and the official Phosphor Icons system across the 3 surface systems in **`design`:os studio**:
1. **Figma UI3 Dark Mode (`com.figma.Desktop` `126.9.13`)**: Applies to the whole Canvas editor (`S01`), Home File Manager (`S04`), Type Settings & Zoom Menu (`S05`), Bottom Actions `Cmd+K` (`S06`), Dev Mode (`S07`), Color/Variables Picker (`S08`), Auto-Update & Figma Schema Watchdog (`S09`), Multi-Tab Project Strip (`S10`), Figma AI Agent Hub (`S11`) and the `Agents` Left Side Panel (`S13`).
2. **`product-os-spec-driven` Operate Dark (`pcp-dark-operate-v2`)**: Applies only to the **Spec Driven v2 Reader (`S02`)** and **5-Pillar Figma Parity Scorecard (`S12`)** surfaces, following the reader theme separation rule of `SPEC.md §10` and decisions `D2`, `D14`.
3. **Unified Phosphor Icons System (`https://phosphoricons.com/` · `@phosphor-icons/core` · `DIR-19` · `D17`)**: Applied uniformly across all 13 screens `S01..S13` through `crates/designos-app/src/icons.rs` and `crates/designos-app/assets/icons/phosphor/*.svg`.

---

## 1. Canonical Color Token Ladder

| Token Name | Figma UI3 Canonical (sRGB) | Figma UI3 Display-P3 Sampled | `product:os` Reader (`S02`/`S12`) | EaseUI `.spec.json` Blueprint | Semantic Usage |
|---|---|---|---|---|---|
| `bg-canvas` | **`#1E1E1E`** (`rgb(30,30,30)`) | `#1C1C1C` | **`#16171B`** | `#09090B` / `#121215` | Default Canvas page background (`S01`) and main Spec Reader background (`S02`/`S12`). |
| `bg-panel` | **`#2C2C2C`** (`rgb(44,44,44)`) | `#272727` | **`#1F2024`** | `#18181B` | Background of the Left Rail (`48px`), Left Sidebar (`240px`), `Agents` Left Side Panel (`S13`), Right Inspector (`240px`), Bottom Toolbar (`528×48px`), Auto-Update Modal (`S09`) and level-1 Cards in the Reader (`S02`/`S12`). |
| `bg-input` / `bg-elevated` | **`#383838`** (`rgb(56,56,56)`) | `#313131` / `#3C3C3C` | **`#26272C`** | `#27272A` | Background of inputs (`X`, `Y`, `W`, `H`, `100%`), segmented selectors (`Flow`, `Channel`), the selected page row and table headers in the Reader (`S02`/`S12`). |
| `border-default` | **`#444444`** / **`#3C3C3C`** | `#3C3C3C` | **`#2E2F36`** | `#27272A` | `1px` dividers between panels, popover borders and data table dividers. |
| `text-primary` | **`#FFFFFF`** | `#FFFFFF` | **`#F3F4F6`** | `#FAFAFA` | Heading text, input values, page names and main content. |
| `text-secondary` | **`#B3B3B3`** / **`#999999`** | `#999999` | **`#9CA3AF`** | `#A1A1AA` | Property labels (`Position`, `Layout`), the `Team project` subtitle and breadcrumbs. |
| `accent-primary` | **`#0D99FF`** | `#0F81E6` / `#71BCF7` | **`#3B82F6`** | **`#FF6A3D`** (BrainWave Orange) | Selected tool button on the Bottom Toolbar (`Move`, `Frame`...), the `"Ask for changes"` input border in `Agents` (`S13`), the `Share` button, the update download progress bar (`S09`), node selection outlines on the Canvas. |
| `accent-selection-row` | **`#323B55`** (`#0D99FF` @ 22%) | `#2D3340` | **`#1E293B`** | `#27272A` | Background of the selected Layer row and the active icon button on the `48px` Left Rail. |
| `accent-component` | **`#9747FF`** / **`#C58FFF`** | `#9C80CB` / `#352B47` | **`#8B5CF6`** | `#8B5CF6` | Icon and text colour for `Component (❖)`, `ComponentSet` and `Instance (◇)` in the Layers tree and on the Canvas. |
| `accent-dev-mode` | **`#24CB71`** | `#5FA773` | **`#10B981`** | `#10B981` | `</>` Ready-for-Dev badge, `SHA-256 Verified ✓` / `Update Ready` badges (`S09`), the `>=99%` Parity bar (`S12`) and the `APPROVED` Gate state. |
| `accent-warning` | **`#F24822`** / **`#FFCD29`** | `#E5484D` / `#F5A623` | **`#F59E0B`** / **`#EF4444`** | `#EF4444` | `Alt` distance guides, pink `padding`/`gap` handles (`#FF4081`), schema / `SHA-256` mismatch warnings (`S09`) and `PENDING` / `MISSING` Gates. |

---

## 2. Figma UI3 Standard Frame Geometry & Proportions (Macro Layout Geometry)

| Surface Region | Standard size (1x Logical Pixels) | `Radius` | `Border & Padding` | Verification source |
|---|---|---|---|---|
| **Far-Left Navigation Rail** | **`48px`** wide (`x = 0..48px`), full window height | `28×28px` icon buttons, **`6px`** radius | Standard order: `File`, `Agents` (`✦`), `Assets`, `Variables`, `Tools`, `Specs` + Sync button with a `#0D99FF` dot at the bottom (`DIR-17`, `DIR-18`) | `figma_live_reference.png`, `US-13` |
| **Left Side Panel (`File` / `Agents` / `Assets`)** | **`240px–320px`** wide (`x = 49..289px`) | `28px` tall page rows, **`6px`** radius | `File` shows `<Project> / <File>` + `Pages` + `Layers`; `Agents` (`S13`) and `Assets` take the full height from the panel top (`UF-02`) | `US-05`, `US-13` |
| **Canvas Rulers (Top & Left)** | **`20px`** thick (`#2C2C2C`) | `0px` | A blue `#1E3A5F` band with `#0D99FF` text projected from the selected element's bounding box | `figma_live_editor.png`, `US-06`, `US-11` |
| **Right Design / Prototype / Dev Inspector** | **`240px`** wide (`268px` when scrolled/expanded) | `#383838` inputs **`28px`** tall, **`6px`** radius | Row 1 (`38px`): Avatar + `Present ▷ v` + `Share` button (`#0D99FF`); a floating `?` help button in the bottom-right corner (`DIR-18`) | `US-05`, `US-13` |
| **Bottom Floating Toolbar & Sub-Toolbar** | **`528px × 48px`** (`12px` from the bottom) | **`12px`** outer radius, **`6px`** inner buttons | Selected tool filled blue `#0D99FF` + the `Design \| Prototype \| </>` group + a contextual sub-bar above (`Crop \| Select area \| Remove bg \| Edit with prompt \| More v`) | `US-05`, `US-13` |
| **Bottom-Docked `Actions (Cmd+K)`** | **`480px × 320px`** (docked `68px` from the bottom) | **`12px`** radius | `#2C2C2C` background, `1px` `#444444` border, 4 underlined text tabs `All \| Assets \| Plugins & widgets \| Design Gate` + `[AI]` / `[Development]` labels (`DIR-18`) | `US-05`, `US-11`, `US-13` |
| **Auto-Update & Schema Watchdog (`S09`)** | **`440px × 340px`** modal + **`208×28px`** Home pill | **`12px`** modal radius, **`6px`** pill | `#2C2C2C` background, `1px` `#444444` border, 3-segment channel selector `Stable \| Beta \| Nightly` (`#383838`) | `US-09`, `S09` |
| **Desktop Multi-Tab Project Strip (`S10`)** | **`36px`** tall, full width at the top of the window | **`6px`** tab radius (active `#2C2C2C`, inactive `#1E1E1E`) | `Home` button (`36×28px`) + the **`design`:os studio** brand cluster + the project tab strip + a `+` button (`28×28px`) | `US-10`, `S10` |

---

## 3. Unified Phosphor Icons System (`https://phosphoricons.com/` · `DIR-19` · `BR-22` · `D17`)

| Property | Technical standard in **`design`:os studio** | Verification source |
|---|---|---|
| **Canonical asset source** | The official **`@phosphor-icons/core` (`https://phosphoricons.com/`)** icon set with `viewBox="0 0 256 256"` and `fill="currentColor"`. | `DIR-19`, `US-14` |
| **Bundle path** | `crates/designos-app/assets/icons/phosphor/*.svg`, statically embedded in the binary via `include_str!` in `crates/designos-app/src/icons.rs`. | `crates/designos-app/src/icons.rs` |
| **Raster & Cache engine** | Parsed with `resvg::usvg::Tree::from_str`, rasterised at physical pixel size `px = (size * pixels_per_point).round()` into `SVG_CACHE: Mutex<HashMap<(Icon, u32), TextureHandle>>` with `TextureOptions::LINEAR`. | `SCN-14-01`, `SCN-14-02` |
| **Pixel-grid snapping (`DPR Snap`)** | Every icon draw rect goes through `round_to_pixel(pos, pixels_per_point)` (`step = 1.0 / ppp`) so it never blurs on half pixels on Retina/HiDPI screens. | `SCN-14-02` |
| **The only brand exception** | Only `Icon::Logo` keeps the **`design`:os studio** brand shape; every other `Icon` variant on `S01..S13` must use Phosphor Icons. | `BR-22`, `SCN-14-03` |
| **Pure PTY Chat UI Wrapper (`S13` · `DIR-21`)** | CLI selector pill (`Claude Code \| Codex \| Antigravity`): active `#0D99FF`28 with `#0D99FF` border, inactive `#383838`; `User` bubble (`AgentChatRole::User`) `#26354A` background, `8px` radius; `Assistant` bubble (`AgentChatRole::Assistant`) `#232323` background, `#383838` border, `8px` radius; inline `NodeSpec` badge `#1E3A2F` background with `#24CB71` text. | `BR-23`, `SCN-15-01..03` |

---

## 4. Design Bans & Brand Rules (`es:designer` Craft Floor & Brand Rules)

1. **Zero Emoji Icons & Zero Mixed Icon Families (`BR-02`, `BR-22`, `DIR-19`)**: Unicode emoji characters are strictly banned, mixing icon families (Lucide/Feather/custom paths) is banned, and hand-drawing standalone UI chrome icons outside `crate::icons::Icon` (Phosphor Icons `@phosphor-icons/core`) is banned.
2. **No Cards-in-Cards**: In the Spec Reader (`S02`/`S12`), Inspector (`S05..S08`) and Auto-Update Modal (`S09`), nesting a bordered/filled card inside another card is banned; separate sub-groups with thin `1px` lines (`#2E2F36` / `#3C3C3C`) or a `#26272C` table header row.
3. **Tabular Numerals & Safe Truncation**: Every numeric cell (`X`, `Y`, `W`, `H`, `%`, `SHA-256` prefix, KPI Strip, Parity Scorecard `S12`, `ui-contract.md` table) must align numerals and truncate text safely (`elide`) without overflowing its frame.
4. **All-Lowercase Bold-Prefix Brand Wordmark (`BR-14` / `D10`)**: The product brand name is always written all lowercase as **`design`:os studio** (or **`design`:os**), where **`design`** must use the **Bold (`700`)** font and `:os studio` uses the **Regular (`400`)** font.
5. **Zero Raw ANSI Escape Noise in Agent Chat Bubbles (`BR-23` / `D18` / `DIR-21`)**: The stdout stream from the `claude`, `codex`, `agy` PTY must always pass through `strip_ansi_codes` when synced into `AgentChatTurn` bubbles, fully removing CSI/OSC control sequences before display to the user.

