# E15-22 Ship surface templates, including slide layout contracts with an intent-to-layout map

- Issue: #610 (task E15-22, epic #585, L / P0)
- Spec base: `main` at `767acf9`.
- Files: `assets/templates/surfaces/components.json` (new, the template component set as NodeSpec), `assets/templates/surfaces/<surface>/<template>.json` (new), `assets/templates/surfaces/slides/contracts.json` (new), `assets/templates/surfaces/slides/intent-map.json` (new), `crates/designos-doc/src/templates/{mod.rs,import.rs,contracts.rs,tests.rs}` (new), the `get_guidelines` module (#248).

## 1. Summary and user story

As an agent, I want to start from a verified skeleton (a mobile shell, a web app shell, landing sections, a table, a design-system page, or one of 20 slide layouts) that already passes its surface lint and follows the active style, instead of an empty frame.

As an agent building slides, I want to say what a slide must do ("compare two options") and get the layout contract that fits.

## 2. Current state

- FACT: no template library exists. The only canned structures are the EaseUI presets in `apply_spec` (`crates/designos-doc/src/spec.rs:201-290`).
- FACT: `apply_spec` builds a NodeSpec subtree in a trial copy and inserts it as one `Command::Insert` (`crates/designos-mcp/src/engine.rs:251-269`).
- FACT: E15-1 (#586) adds `kind: "instance"`, E15-2 (#587) slot fills, E15-3 (#588) `var:` bindings; E15-19 (#606) compiles the style variables these templates bind to; E15-21 (#609) defines the surface profiles they must pass.

## 3. Behaviour rules

1. **Template component set.** `components.json` defines our own components, all bound only to E15-19 names, always in the collection-qualified form of E15-19 rule 11 (`var:designos-style:color/color/*`, `var:designos-style:shape/radius/*`, `var:designos-style:space/space/*`, the text styles by name `text_style: "<Name>"` for keys `designos-style:text/<Name>`, the effect styles `effect_style: "Elevation/<n>"` for keys `designos-style:effect/elevation-<n>`): `Status Bar`, `Tab Bar` (slot of 3–5 `Tab Item`), `Tab Item` (variants State=Default/Selected), `Top Bar`, `Sidebar` (slot of `Nav Item`), `Nav Item` (State=Default/Selected/Hover), `Button` (Variant=Primary/Secondary/Ghost/Destructive, Size=Sm/Md/Lg), `Input` (State=Default/Focus/Error/Disabled), `Card`, `Table` (header row + slot of `Table Row`), `Table Row`, `Table Cell` (Type=Text/Number/Status/Action), `Badge`, `Empty State`, `Section Header`, `Swatch`, `Type Specimen Row`, `Slide Title`, `Slide Body`, `Slide Footer`.
2. **Templates** (each a NodeSpec frame tagged with its surface, built from instances of the set):
   - web app: `web-app/shell` (top bar + sidebar + content), `web-app/list-detail`, `web-app/settings`, plus state frames `… / Empty`, `… / Loading`, `… / Error`, `… / Success`, `… / Permission` for the list;
   - landing: `landing-page/hero`, `landing-page/features`, `landing-page/proof`, `landing-page/pricing`, `landing-page/faq`, `landing-page/cta`, `landing-page/footer` (sections, each a full-width frame);
   - mobile: `mobile/shell` (status bar, content wrapper with 16 px side padding, tab bar last in the stack, never absolutely positioned), `mobile/list`, `mobile/detail`;
   - table: `table/data-table` with Empty, Loading, Error states;
   - design system: `design-system/page` (colour swatches from every `color/*` variable, type specimen per `designos-style:text/<Name>` style, radius and elevation samples, component sheet).
   - slides: one template per contract `slides/L01` … `slides/L20`.
3. **Insertion through `apply_spec`.** A spec `{ "kind": "instance", "component": "designos.template/mobile/shell" }` places a template:
   - on first use, `import.rs` adds the template component set to the document on a page named `design:os templates` (created if missing, hidden from the page list's default sort only by being last), and the requested template's main frame as a component on the same page;
   - later uses reuse the imported components by their keys `designos.template/<id>`; the imported set records the catalogue version in its component description so a newer app can offer an update (non-goal here);
   - the instance is placed like any E15-1 instance.
4. **Style requirement.** Templates need the E15-19 variables. If `designos-style:color` is missing, `apply_spec` fails with `style_required` (corrected example: a `designos_apply_style` call with the first catalogue preset). The template import never applies a style on its own.
5. **Lint gate (AC1).** Every template, inserted into a document with each catalogue preset applied, passes its surface profile with zero `error` findings and zero global lint errors (E15-9). Warnings are allowed only where the template documents why in its `notes`.
6. **Re-theme (AC2).** Applying another archetype re-themes every template because every visual value is a binding.
7. **Slide layout contracts.** Each contract has `id`, `name`, `grid`, `regions`, `type_ranges`, `rules`. Common grid at the design size: 1920×1080, 12 columns, outer margin 96, gutter 24, baseline 8. On a deck of another size every pixel value of a contract (grid, margin, gutter, baseline, region bounds, `type_ranges`) is multiplied by `k = frame width / 1920`, the same factor E15-21 rule 11 applies to the `slides` lint thresholds, so a contract and its lint agree at every size. Regions are column spans (1-based, inclusive) and row bands in 8 px baseline units measured from the top margin; the safe area is 888 px tall, so row indices run 0–111. Our 20 contracts:

| Id | Name | Regions |
|---|---|---|
| L01 | Cover | title (cols 1–10), subtitle (1–8), meta footer |
| L02 | Agenda | title, ordered list of 3–7 items |
| L03 | Section divider | section number, section title |
| L04 | Statement | one sentence, centred, display size |
| L05 | Title and bullets | title, 3–6 bullets |
| L06 | Two columns | title, two equal text columns |
| L07 | Three columns | title, three cards |
| L08 | Image left | image (cols 1–6, full height), text (7–12) |
| L09 | Image right | text (1–6), image (7–12) |
| L10 | Full-bleed image | image, caption band |
| L11 | Big number | metric (display), label, context line |
| L12 | Quote | quote, attribution |
| L13 | Comparison | title, two option columns with matching rows |
| L14 | Chart and takeaway | chart (cols 1–8), takeaway (9–12) |
| L15 | Timeline | title, 3–6 milestones on one axis |
| L16 | Process | title, 3–5 numbered steps |
| L17 | Card grid | title, 2×2 or 2×3 cards |
| L18 | People | title, 2–8 portrait cards |
| L19 | Call to action | ask, action, contact |
| L20 | Closing | closing line, contact or thanks |

   `type_ranges` per contract: title 40–72 px, body 24–32 px, caption 18–22 px, display 96–200 px (where used). `rules` are machine checks in E15-21 rule-kind form plus contract-only kinds: `max_items { region, n }`, `max_words_per_item { region, n }`, `region_bounds` (every child of a region stays inside it).
8. **Intent-to-layout map.** `intent-map.json` maps intents to ordered contract lists: `open`→[L01], `agenda`→[L02], `new-section`→[L03], `one-idea`→[L04, L05], `list-points`→[L05, L07], `compare`→[L13, L06], `show-metric`→[L11, L14], `show-trend`→[L14], `sequence`→[L16, L15], `timeline`→[L15], `quote`→[L12], `show-image`→[L10, L08, L09], `people`→[L18], `overview`→[L17, L07], `ask`→[L19], `close`→[L20]. AC3: every listed intent returns a contract that exists and validates.

## 4. Data model / schema

```json
// assets/templates/surfaces/slides/contracts.json (one entry)
{ "id": "L13", "name": "Comparison", "intents": ["compare"],
  "grid": { "width": 1920, "height": 1080, "columns": 12, "margin": 96, "gutter": 24, "baseline": 8 },
  "regions": [ { "id": "title", "cols": [1, 12], "rows": [0, 12] },
               { "id": "left", "cols": [1, 6], "rows": [16, 111] }, { "id": "right", "cols": [7, 12], "rows": [16, 111] } ],
  "type_ranges": { "title": [40, 72], "body": [24, 32], "caption": [18, 22] },
  "rules": [ { "kind": "max_items", "region": "left", "n": 6 }, { "kind": "max_words_per_item", "region": "left", "n": 12 },
             { "kind": "region_bounds" } ] }
```

```rust
pub struct SlideContract { pub id: String, pub name: String, pub intents: Vec<String>, pub grid: SlideGrid,
    pub regions: Vec<Region>, pub type_ranges: BTreeMap<String, [f32; 2]>, pub rules: Vec<ContractRule> }
pub fn contracts() -> &'static [SlideContract];
pub fn select_layout(intent: &str) -> Result<Vec<&'static SlideContract>, TemplateError>;
pub fn import_template(doc: &mut Document, id: &str) -> Result<TemplateImport, TemplateError>; // used by apply_spec
```

No change to the document format: templates import as ordinary components and instances.

## 5. Contracts

- `designos_get_guidelines { "category": "templates" }` → `{ "templates": [ { "id": "mobile/shell", "surface": "mobile", "name": "Mobile shell", "size": [390, 844], "component_key": "designos.template/mobile/shell", "slots": ["content"] } … ] }`.
- `designos_get_guidelines { "category": "slide-layout", "intent": "compare" }` → `{ "intent": "compare", "contracts": [ { …L13… }, { …L06… } ] }`. Unknown intent → `slide_intent_unknown`, details `{ "allowed": [intents] }`, corrected example `{ "category": "slide-layout", "intent": "compare" }`.
- `apply_spec` with an unknown template key → E15-1's unknown-component error, with `details.templates` listing valid template keys.
- `style_required` (§3.4), `safe_to_retry: true` after the corrected call.

## 6. File-format mapping

Imported templates are ordinary components; `.fig` export writes them as components on the `design:os templates` page.

## 7. UI

No new panel. Imported templates appear in the Assets panel under the `design:os templates` page like any local component.

## 8. Edge cases

1. Template inserted into a frame narrower than its size: the instance keeps its size; the parent's auto layout decides placement; lint reports overflow as usual.
2. The user deleted the `design:os templates` page: the next insertion re-imports the set; existing instances stay detached-safe because E15-1 instance placement fails if a key is missing rather than creating broken links.
3. Two documents import different catalogue versions: each keeps its own; no cross-document state.
4. Slide contract used on a deck whose frames are 1280×720 (owner ruling 2026-10-19): the contract geometry (grid, margin, gutter, baseline, regions) and the `type_ranges` scale by `k = 0.667` (width/1920), the E15-21 `slides` lint thresholds scale by the same `k` (E15-21 rule 11), and the scale factor is reported. A slide laid out proportionally passes `slides` lint at 1280×720 with no finding; the same slide at 14 px body fails its scaled 16 px minimum. A frame that is not 16:9 still fails `surface.slides.frame-size`.
5. Archetype whose elevation set has an empty level: E15-19 writes `Elevation/1`, `Elevation/2` and `Elevation/3` every time, an empty level with an empty effect list, so templates that use `Elevation/2` (key `designos-style:effect/elevation-2`) always find the style; an empty level renders as no shadow, not an error, and never `UNKNOWN_STYLE`.
6. Dark mode: templates contain no mode overrides; they follow the frame's inherited mode.

## 9. Non-goals

- Updating already-imported templates to a newer catalogue version.
- User-authored templates.
- Generating slide content.

## 10. Test plan

Seams: `cargo test -p designos-doc templates` (lint over all templates × all presets), render snapshots via `designos-render`, MCP contract tests in `designos-mcp`.

Fail-before: `every_template_passes_surface_lint` fails on `main`.

| Test | Input | Expected |
|---|---|---|
| `every_template_passes_surface_lint` (AC1) | each template × each preset | 0 error findings (surface + global) |
| `templates_retheme` (AC2) | insert all templates under preset A, render; apply preset B, render | every template's render differs; zero literal values found in the inserted subtrees |
| `intent_map_resolves` (AC3) | every intent | non-empty list; each id exists; contract validates (regions inside grid, type ranges ordered) |
| `slide_templates_follow_contract` | `slides/Lxx` vs contract `Lxx` | every child inside its region; font sizes inside `type_ranges` |
| `style_required_named_error` | insert template in a document with no style | `style_required` |
| `second_insert_reuses_components` | insert twice | component count unchanged after the second insert |
| `slide_contract_scales_at_1280x720` | `slides/L13` inserted into a 1280×720 deck, then linted | grid margin 64, gutter 16, regions and type ranges × 0.667; every child inside its scaled region; zero `surface.slides.*` findings |
| `slide_contract_at_1280x720_small_body_fails` | same slide with body text 14 px | `surface.slides.body-min-size` with threshold 16 and `scale` 0.667 |
| `empty_elevation_template_inserts` | archetype with `"2": []`; insert a template that binds `Elevation/2` | no error; the card draws no shadow |
| `mobile_tab_bar_last_in_stack` | `mobile/shell` | tab bar is the last child and not absolutely positioned |

Visual evidence: one contact sheet PNG of all templates under one preset, rendered on the CPU path.

## 11. Performance budgets

- First template import ≤ 30 ms; later inserts are ordinary instance placement.
- CI lint run over templates × presets (≈ 40 templates × ≥ 26 presets) ≤ 60 s in a release build.
- No canvas-side work beyond the normal document revision bump (CLAUDE.md §4.1).

## 12. Dependencies and build order

- Needs E15-21 (#609), E15-1 (#586), E15-2 (#587), E15-3 (#588), E15-19 (#606).
- Used by E15-28 (briefs may start from templates) and E15-47 (generate-design skill).

## 13. Owner rulings that apply

- Full scope: every surface listed plus all 20 slide contracts.
- Built from our own components and our own layout definitions.
