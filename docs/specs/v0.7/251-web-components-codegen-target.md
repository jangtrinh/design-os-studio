# #251 Add the Web Components codegen target

| Field | Value |
|---|---|
| Issue | #251 (task T7.30, epic #20; milestone v0.7 Effects & Export) |
| Size | M (the issue says S; escaping across two languages, slots, attributes and a DOM smoke test add the rest) |
| Depends on | issue: none. Expected on `main` before v0.7 opens (v0.6): T7.3 (one `CodeLanguage` list) and T7.29 (class-name rules and the Node validator job). Same fallbacks as #249, section 12. Land after #249 (section 12). |
| Unblocks | the `webcomponents` option of `designos_generate_code` and of T7.4's design context |
| Touches | new `crates/designos-export/src/webcomponents.rs`, `crates/designos-export/src/lib.rs` (`mod webcomponents`, `pub use`, beside `:22` and `:39`), `crates/designos-export/src/language.rs` (T7.3: add `WebComponents`; today the app's `Language`, `crates/designos-app/src/dev/mod.rs:19-56`), `crates/designos-export/src/slots.rs` (`Lang` `:13`: add `WebComponents`, JS reserved words), new `crates/designos-export/tests/golden/webcomponents/` (golden module, stub element, `package.json`, `tsconfig.json`, smoke script), `.github/workflows/ci.yml` (one step in the `codegen-validators` job), the MCP language lists only if T7.3 has not unified them (`crates/designos-mcp/src/engine.rs:112-126`, `:747`, `:971`) |

Code baseline: `origin/main@0e8a9a5` (`docs(specs): v0.6 task specs, part 4 (agent terminal UX and constraint solver) (#697)`). v0.7 opens after v0.6 is tagged. The worker re-anchors every cited line against HEAD before writing code and reports drift first.

## 1. Summary and user story

As a developer or agent, I generate a standards-based custom element for a frame: a TypeScript module that defines one `HTMLElement` subclass with a shadow root, the layers as markup, the same CSS the CSS target prints, `<slot>`s for slots, and attributes for component properties. It needs no framework and compiles with `tsc` in strict mode.

## 2. Current state (FACT)

1. **No such target.** The seven Dev Mode languages and the three hand-kept MCP lists are in `crates/designos-app/src/dev/mod.rs:19-32` and `crates/designos-mcp/src/engine.rs:112-126`, `:747`, `:971`; `designos-export` has no `webcomponents.rs` (`crates/designos-export/src/lib.rs:14-30`). T7.3 and T7.29 (accepted v0.6 specs) provide the single language list, the class-name rules and the validator job, as described in #249's section 2.
2. **Reusable pieces, as for React and Vue:** `css_for` (`crates/designos-export/src/css.rs:10`) with variables already printed as `var(--Color-Primary, #0D99FF)` and WEB code syntax verbatim (`crates/designos-export/src/codegen.rs:195-215`), `view_name`, `kebab`, `pascal`, `camel` (`codegen.rs:103`, `:88`, `:70`, `:79`), `visible_children`, `is_vector_asset` (`:234`, `:244`), `instance_args` (`:296`), and the slot helpers `slot_infos`, `slot_site`, `instance_slots`, `prop_params` (`slots.rs:242`, `:287`, `:363`, `:315`).
3. **Code-syntax strings are user input.** The existing tests `crates/designos-export/tests/codegen_escapes_variable_code_syntax.rs` check that a code-syntax string cannot break out of an XML attribute or a JS string literal. A web target that prints those strings into a `<style>` block and a JS template literal needs the same guard.
4. **No Node tooling in the repository** (`git ls-files` lists no `package.json`; CI jobs are `test`, `lint`, `fmt`, `.github/workflows/ci.yml:17`, `:54`, `:78`). The fixture is `cfg(test)` (`codegen.rs:724-725`), so golden tests live inside the crate.
5. **tsc, checked on this machine (2026-10-19).** A hand-written exemplar of the output below (a module with a `template` element built from a template literal containing a `<style>` and the markup, `export class DsCard extends HTMLElement` with `static observedAttributes`, a private `#render`, `customElements.define`, and a `HTMLElementTagNameMap` augmentation) passes `tsc --noEmit` 5.9.3 with `strict: true`, `target: ES2022`, `lib: ["ES2022","DOM"]` (Node v22.23.1). `static observedAttributes = [...]` is a class field, which needs `target` ES2022 or `useDefineForClassFields`; the validator config sets ES2022. `tsc` does not look inside the template literal, so a broken HTML string would still pass: the module needs a DOM-level check as well (rule 11).
6. **Latest versions today** (`npm view`, 2026-10-19): `typescript` 7.0.2 and `lit` 3.3.3. TypeScript 7 stopped `vue-tsc` 3.3.13 from starting (#249, fact 6); the same pin, `typescript` 5.9.3, is used here for one toolchain.
7. **Custom element rules the output must meet** (HTML Standard, from the platform's definition of a valid custom element name, INFERENCE from memory, check against the spec text when implementing): the name is lower-case, contains a hyphen, starts with a letter, and a few names are reserved.

## 3. Behaviour rules

1. **API.** `pub fn webcomponents_for(doc: &Document, node: NodeId) -> String` returns the text of one `.ts` module. No options. Unknown node: empty string.
2. **Naming.** The element tag is `ds-` plus the kebab name of the layer (`Card` -> `ds-card`, `Primary Button` -> `ds-primary-button`); a name that already starts with `ds-` is not prefixed twice. The class is `Ds` plus the PascalCase name (`DsCard`). An empty or all-symbol name falls back to the `kebab` and `pascal` defaults (`layer`, `Layer`). A name that would equal a reserved custom element name gets the suffix `-element`.
3. **Module shape.** In order: side-effect imports of the modules that define element tags used inside (one per distinct instance component, path `./ds-<name>`, under the comment `// Adjust these import paths to where your elements live.`); the `template` element with `innerHTML` set to a template literal holding `<style>` then the markup; `export class <Class> extends HTMLElement`; the guarded definition `if (!customElements.get('ds-card')) { customElements.define('ds-card', DsCard); }`; the global `HTMLElementTagNameMap` augmentation. Indent two spaces.
4. **Class body.** `constructor()` calls `super()`, attaches an open shadow root and appends a clone of the template. For each non-slot component property of the exported root (`prop_params`): the name is listed in `static observedAttributes` (kebab attribute name), with a typed accessor: boolean through `hasAttribute` and `toggleAttribute`, text and variant through `getAttribute` with the declared default. `attributeChangedCallback` is present only when there is at least one property. No framework, no decorators, no dependency.
5. **Styles.** The `<style>` block starts with `:host { display: block; }`, then one rule per layer class from the declarations of `css_for` in printed order, the root without `position`, `left` and `top`. Classes follow the T7.29 rule (lower-case `[a-z0-9-]`, `n-` prefix for a leading digit, `-2`, `-3` for repeats). CSS custom properties (`var(--...)`) reach into the shadow root, so bound variables work without a `:root` block (INFERENCE: standard behaviour of custom properties; check in a browser render when the owner runs the capture of section 7).
6. **Markup.** The same element mapping as the HTML and Vue targets: frames, groups, components, sections -> `<div>` (`<section>` for sections); text -> `<p>` (a paragraph break is `<br>`); vector asset -> `<img src="/assets/<kebab>.svg" alt="..." width height>`; ellipse -> `<div>` with the radius from `css_for`. Hidden layers are omitted.
7. **Instances.** An instance prints `<ds-button class="button" label="Save" disabled></ds-button>`; text and variant properties are attributes, a true boolean is the bare attribute, a false boolean is omitted, a number is its decimal text. The tag is `ds-` plus the kebab name of the component (the component set for variants).
8. **Slots.** A component with slots prints `<slot name="icon">default content</slot>` (`<slot>` for the only slot). An instance fills a slot by adding `slot="icon"` to each filled element; the default slot's content has no attribute. Slot names are the kebab form of `slot_ident` names.
9. **Escaping, three layers, in this order.** (a) HTML: `&` `<` `>` in text and `&` `"` in attribute values. (b) CSS: every `<` in a declaration value is written as the CSS escape `\3C ` so a value can never close the `<style>` block. (c) JavaScript: in the whole template-literal text, `\` becomes `\\`, a backtick becomes `` \` `` and `${` becomes `\${`. A code-syntax string, a layer name, a text run or a property name containing any of these characters cannot leave its context.
10. **Property names** from component properties are camelCased for accessors and kebab-cased for attributes; a name that is not a valid identifier or is a JS reserved word gets the suffix of `slot_ident` (`Lang::WebComponents`), and the attribute keeps the kebab text.
11. **Verification outside `tsc`.** The validator project's smoke script compiles the golden with `tsc` to JavaScript, loads it in jsdom, creates the element, and checks that its shadow root holds the expected class names, one `<style>`, and the slot elements (this is what proves the HTML string parses).
12. **Variables (A5, A7).** Whatever `css_for` prints (`var(--<name>, <fallback>)`, WEB code syntax verbatim). No new variable logic.
13. **Determinism.** Same document, same bytes.

## 4. Data model / schema changes

`CodeLanguage::WebComponents` (`id: "webcomponents"`, `label: "Web Components"`), placed after Vue in the web group of the dropdown. No document change.

## 5. Contracts

- `designos_generate_code` with `language: "webcomponents"` (enum generated from `CodeLanguage::ALL`); reply shape unchanged. The returned text is a `.ts` module (the language label tells the host the file type).
- Shape of the fixture output (the golden fixes the bytes):

  ```ts
  // Adjust these import paths to where your elements live.
  import './ds-button';

  const template = document.createElement('template');
  template.innerHTML = `
    <style>
      :host { display: block; }
      .card { display: flex; width: 312px; ... background: #FFF; }
    </style>
    <div class="card">
      <p class="..."> ... </p>
      <ds-button class="button" label="Save" disabled></ds-button>
      <div class="ellipse"></div>
    </div>
  `;

  export class DsCard extends HTMLElement {
    constructor() {
      super();
      this.attachShadow({ mode: 'open' }).appendChild(template.content.cloneNode(true));
    }
  }

  if (!customElements.get('ds-card')) {
    customElements.define('ds-card', DsCard);
  }

  declare global {
    interface HTMLElementTagNameMap {
      'ds-card': DsCard;
    }
  }
  ```
- Validator project under `crates/designos-export/tests/golden/webcomponents/`: `package.json` pinning `typescript` 5.9.3 and `jsdom` (version re-checked at implementation, never `latest`); `tsconfig.json` with `strict`, `noEmit` for the type check, `target: ES2022`, `lib: ["ES2022","DOM"]`; a stub `ds-button.ts`; the golden `ds-card.ts`; `smoke.mjs` for rule 11.

## 6. File-format mapping

None.

## 7. UI

Dev Mode's language dropdown gains "Web Components" (from `CodeLanguage::ALL`); the code panel shows the module with the existing code view and copy button. No other UI. Evidence: a desktop capture at 1440 of Dev Mode with the target selected on the fixture card; the `tsc` and smoke terminal capture; and one browser capture of the golden element rendered with its variables defined (this settles the INFERENCE in rule 5). Responsive: SKIPPED (owner default). Load `es-designer` before touching the dropdown code. Update Figma frames from the capture first (owner rule).

## 8. Edge cases

1. Text containing a backtick, `${x}`, `</style>`, `</script>` or a backslash: each stays inert (rule 9), with a test per character.
2. A WEB code syntax of `</style><script>alert(1)</script>`: printed as inert CSS text (`\3C /style>...`), never closing the block.
3. A layer name with spaces, symbols or emoji: the tag and class fall back as in rule 2 and #249 edge 1.
4. Two instances of the same component: one import line.
5. A component whose property is named `class` or `style`: accessor `class_`-style suffix per rule 10; the attribute is `class`, which collides with the global attribute, so the property is skipped from `observedAttributes` and a comment says why.
6. An instance of the exported component itself (recursion): no import; the tag is defined by this module.
7. A frame with no children: an empty `<div class="..."></div>`.
8. Slots with default content and an instance that fills none: the default shows (slot fallback content).
9. Very large subtree (5,000 nodes): generated as one template literal; T7.4's size fallback applies.
10. A hidden layer inside a slot's default content: omitted.

## 9. Non-goals

- Lit, FAST, Stencil or any library; decorators; JSX; build tooling.
- Light-DOM mode, form-associated custom elements, ARIA roles beyond what the layers imply, and event wiring.
- Writing the files of the elements an instance imports, or the SVG assets.
- Interactions, prototype links and Code Connect mapping.

## 10. Test plan (red first)

Seams: unit golden tests inside `webcomponents.rs`; the `codegen-validators` job over the committed golden directory (`tsc` strict, then the jsdom smoke script).

| Case | Expected | Fails before |
|---|---|---|
| `card_golden` | `webcomponents_for(fixture::card())` equals the committed `tests/golden/webcomponents/ds-card.ts` (`include_str!`) | yes (function absent) |
| `card_compiles_with_tsc_strict` | CI: `npm ci`, `npx tsc --noEmit -p .` exits 0 | yes |
| `card_defines_and_renders_in_jsdom` | CI: `node smoke.mjs` exits 0: `customElements.get('ds-card')` is defined, the shadow root has the expected classes, one `<style>`, the `ds-button` child | yes |
| `validator_rejects_a_type_error` | the same project with a deliberate type error in the golden copy exits non-zero (a script step; proves strict mode is on) | yes |
| `bound_variables_print_code_syntax` | WEB code syntax `var(--brand-primary)` printed per `css_for`; without it `var(--Color-Primary, #0D99FF)` | guard |
| `escaping_html_css_and_js_layers` | text and names with `` ` ``, `${x}`, `\`, `<`, `&`, `"`: the module compiles and the rendered text equals the input | yes |
| `code_syntax_cannot_close_the_style_block` | WEB code syntax with `</style><script>`: output has no raw `</style>` before the block's own end; the smoke test finds no `<script>` element | yes |
| `tag_and_class_naming` | `Card`, `Primary Button`, `ds-thing`, `123`, emoji: the tags are valid custom element names, unique and prefixed once | yes |
| `props_become_observed_attributes` | root component with `Label` and `Disabled`: `observedAttributes` lists `label` and `disabled`; accessors typed `string` and `boolean` | yes |
| `slots_become_slot_elements` | slot `icon` with default content: `<slot name="icon">...</slot>`; the instance fill carries `slot="icon"` | yes |
| `output_is_deterministic` | two runs equal | yes |
| `language_listed_once` | `CodeLanguage::ALL` contains `WebComponents`; the MCP enum has `webcomponents` with no hand-edited list; the dropdown test (`shell/tests/dev.rs:81-97`) still fits | yes |

Run: `cargo test -p designos-export webcomponents`, `cargo test -p designos-mcp`, `cargo test -p designos-app dev`, and locally `cd crates/designos-export/tests/golden/webcomponents && npm ci && npx tsc --noEmit -p . && node smoke.mjs`.

## 11. Performance budgets

At most 20 ms for a 2,000-node frame (T7.29's budget; `test_timing::fastest_within`); one pass, one `css_for` call per layer; escaping is linear in output size. The CI step finishes in under 2 minutes including `npm ci` (UNKNOWN until the first run; record it).

## 12. Dependencies and build order

Same dependency handling as #249, section 12 (T7.3 and T7.29 expected; fallbacks listed there). Order among the three codegen tasks: #249, #251, #252, one at a time; each rebases on the previous, and the shared lines are `lib.rs`, `language.rs`, `slots.rs` (`Lang`), `ci.yml` and the MCP enum. `crates/designos-mcp/src/engine.rs` is a hot spot of the pack: untouched when T7.3 has landed. The class-name function is shared with #249 and the HTML target; whichever lands first owns the module.

## 13. Owner rulings

- A5 and A7: bound variables print code syntax (WEB for web targets) with resolved fallbacks. (Issue text.)
- Full scope, never MVP.
- Acceptance: the fixture card output compiles with `tsc`. (Issue.)

## Open questions for the owner (recommended first)

1. **Vanilla custom elements or Lit.**
   - A (recommended): vanilla `HTMLElement` with a shadow root. No dependency, no decorator settings, compiles with plain `tsc`, and the user can paste it anywhere.
   - B: Lit (`lit` 3.3.3). Less boilerplate for props and re-rendering, but adds a runtime dependency and decorator or class-field configuration that often breaks a plain `tsc` run.
2. **Tag prefix.** Recommended: a fixed `ds-` prefix (every custom element name needs a hyphen). Alternative: no prefix when the layer name already has a hyphen (inconsistent tags), or a user-set prefix option in a later task.
3. **Pinned validator versions.** Recommended: `typescript` 5.9.3, the same as #249, until the owner wants to move both validators to TypeScript 7.
