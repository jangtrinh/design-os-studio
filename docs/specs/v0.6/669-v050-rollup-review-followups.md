# #669 Follow-ups from the v0.5 rollup integration review

| Field | Value |
|---|---|
| Issue | #669 (five non-blocking findings from the independent review of the v0.5.0 rollup integration commits) |
| Size / priority | M / P2 (five small, independent subsections; one PR with five commits is fine) |
| Depends on | none |
| Unblocks | none |
| Touches | 1: `crates/designos-fig/src/fig_export.rs`, `crates/designos-fig/src/rest_import.rs`, `crates/designos-fig/tests/fig_slots.rs`, `crates/designos-fig/tests/fig_roundtrip.rs`; 2: `crates/designos-fig/src/fig.rs`, `crates/designos-fig/tests/`; 3: `crates/designos-app/src/check_designs/hardcoded.rs` and `tests/`; 4: `crates/designos-fig/tests/fig_slots.rs`, `crates/designos-fig/tests/fig_export_real_tags.rs` (tests only); 5: none in code unless the owner rejects the visual (`crates/designos-app/src/dev/playground.rs`) |

Code baseline: `origin/main@01d48f2` (0.5.0). The rollup squashed the branch history into one commit
(`5076306`); the earlier commits named below are still in the object store and are cited by hash.

Subsections are independent. Each lists its facts, its rule, its fixtures and test table.

---

## 669.1 Component key fallback

### Current state (FACT)

- A component without a library key is exported with its GUID as key, `"{session}:{local}"`:
  `cx.comp_keys.insert(id, format!("{}:{}", g.0, g.1))` (`designos-fig/src/fig_export.rs:303-307`) and
  the preferred-values encoder falls back to the same string (`:2056`). The doc comment at `:1712` states
  "its key is its GUID as `s:l`".
- Before commit `931f7017` ("key unkeyed components by guid and use real schema shapes in passthrough test after merging #514 and #510") the
  fallback was `comp:{s}:{l}` in those two places and in the slot-default value text at `:1107`; that
  commit dropped the prefix everywhere. The slot-default text (`:1107`) is a list of child GUIDs, not
  keys, so it is correct as a bare `s:l`; only the two key sites matter here.
- Explicit keys are collected before the fallback: instance-carried keys fill `cx.comp_keys` only when
  no other component owns the key (`:292-301`, the `owned` set), and GUIDs are reserved in a first pass
  (`:189`), but the fallback at `:303-307` does not check `owned`. Exported files can contain bare
  `s:l` keys (the fallback itself), and a later import keeps them as the component's `component_key`;
  an export after GUIDs were renumbered can then give a different component a fallback equal to that
  stale key.
- Import resolves a preferred-value key through `lk.comp_by_key.get(k)` and otherwise keeps the raw
  string (`designos-fig/src/fig.rs:1805-1823`, `rest_import.rs:1237-1260`).
- The test the issue calls "#514's test" is the slot preferred-values group in
  `designos-fig/tests/fig_slots.rs` (for example
  `real_preferred_values_object_round_trips_for_slot_and_instance_swap_and_keeps_legacy_guid_list`,
  `:340`, and `a_key_carried_only_by_the_symbol_key_field_round_trips_through_fig_export`, `:627`). Which
  single assertion requires the bare form was not isolated: reading these tests, the real keys are
  `K_*` strings and the legacy list is guid-shaped, so the worker must run them red against a
  `comp:` fallback to find it (see section "Could not ground").

### Rule

1. The fallback key stays the bare `s:l` unless that string is already owned by a different component
   (an explicit key in `cx.comp_keys` values, an instance-carried key, or another fallback). When it is
   owned, the fallback becomes `comp:s:l`, and if that is owned too, `comp:s:l:2`, `:3`, ... Both key sites
   (`:303-307`, `:2056`) call one function `fallback_component_key(cx, id)` so they cannot disagree.
2. Import: a preferred-value key of the form `comp:s:l[:n]` resolves to the component whose GUID is
   `s:l` (then the same lookup as a bare `s:l`). A key found in `comp_by_key` always wins over any
   guid-shaped reading.
3. No change to files without a collision: output is byte-identical to today.

### Fixtures and tests (synthetic, hand-written, content-free)

| Case | Expected (fails before) |
|---|---|
| `unkeyed_component_fallback_does_not_equal_another_components_explicit_key` | two components: A has explicit key `"7:3"`, B is unkeyed with GUID `7:3`; exported keys differ, B's is `comp:7:3` |
| `fallback_round_trips_when_colliding` | export then import; both components keep their preferred-value links |
| `non_colliding_files_export_unchanged` | every existing preferred-value test passes untouched, and the exported bytes for a collision-free fixture equal the pre-change bytes |
| `comp_prefixed_key_resolves_on_import` | a fixture with `comp:7:3` resolves to the component with GUID `7:3` |

Run `cargo test -p designos-fig`.

---

## 669.2 Half-null GUIDs

### Current state (FACT)

- `kiwi_guid_str` (`designos-fig/src/fig.rs:100-114`) and `guid_str` (`:314-321`) return `None` only for
  the pair `(u32::MAX, u32::MAX)`; the comment says session `u32::MAX` is a real id because paste uses it
  for its synthetic page (`clipboard.rs:163`, `(u32::MAX, l)` for the first free `l`, falling back to the
  null pair only when every `l` is taken).
- A GUID with only one side equal to `u32::MAX`, for example `(u32::MAX, 5)` or `(5, u32::MAX)`, imports as
  a real id today. Existing tests cover the null pair (`tests/fig_roundtrip.rs:2200`, `:2250`, `:2495`),
  none covers a half-null pair.

### Rule

Intended behaviour, inferred from the code comment and the paste code (no Figma reference file may be used
to confirm it): only the exact pair `(MAX, MAX)` is unset; a GUID with one side `MAX` is a real id, both for
the kiwi and the JSON reader. This subsection pins that with tests; it changes no production code unless
a test shows the two readers disagree.

### Fixtures and tests

| Case | Expected |
|---|---|
| `guid_pair_max_max_is_unset` | both readers return `None` |
| `guid_session_max_local_5_is_a_real_id` | `Some("4294967295:5")` from both readers |
| `guid_session_5_local_max_is_a_real_id` | `Some("5:4294967295")` from both readers |
| `guid_session_max_minus_one_local_max_is_a_real_id` | `Some("4294967294:4294967295")` |
| `half_null_guid_round_trips_through_fig_export` | a node with `(MAX, 5)` imports, exports and re-imports with the same id string (use the paste fixture style of `fig_roundtrip.rs`) |

The functions are private, so the readers are exercised through `import_fig` / `import_rest_json` on
hand-written fixtures, or by moving the two tests into the `fig.rs` test module.

---

## 669.3 Text-style suppression

### Current state (FACT)

- `text_number_bound_in_place(doc, n)` (`designos-app/src/check_designs/hardcoded.rs:704-714`) returns
  true when a text number (`fontSize`, `lineHeight`, `fontWeight`, `letterSpacing`) is bound and the
  layer's stored text style equals its resolved one:
  `(Some(NodeKind::Text(stored)), NodeKind::Text(resolved)) => stored.style == resolved.style`, with
  `_ => true` for any other pairing.
- `TextStyle` includes `fills`. A text style does not carry fills: the style rules compare with
  `same_text_style`, which keeps the layer's own fills (`hardcoded.rs:692-694`). So a layer whose number
  binding is in place but whose bound fill resolves differently in the current mode compares unequal,
  and the function returns false: the audit then suggests a text style for numbers that are already bound
  (`text_rule`, `:732`; `style_matches_node`, `:856`).
- The `_ => true` arm is reached when the document lookup fails or the stored and resolved kinds differ.
  `resolved_node` keeps the kind, and `n.id` is always in the document, so the arm is unreachable today;
  it is conservative (suppress).

### Rule

1. Compare the stored and resolved styles without fills: reuse `same_text_style(&stored.style, &resolved.style)`
   (which already ignores fills) instead of `==`.
2. Keep `_ => true` as the conservative answer for an unreachable pairing and say so in its comment; add a
   `debug_assert!` that the two kinds are equal when both nodes exist.
3. Runs: `t.runs` styles are not part of this predicate; no change.

### Fixtures and tests

| Case | Expected (fails before) |
|---|---|
| `bound_font_size_with_mode_dependent_fill_is_not_offered_a_text_style` | text with `fontSize` bound to a variable equal in both modes, `fills/0` bound to a colour variable whose value differs by mode, a text style equal to the layer's other properties exists; in the dark mode no `ApplyStyle` row |
| `bound_font_size_resolving_differently_still_offers_the_style` | binding resolves to another size in the mode (the case the existing comment describes): the style is still offered (unchanged) |
| `kind_mismatch_arm_is_conservative` | unit test of the function with a non-text resolved node returns true |

Run `cargo test -p designos-app check_designs`.

---

## 669.4 Slot scan

### Current state (FACT)

- The scan is in a test, not production: `designos-fig/tests/fig_slots.rs:1170` filters
  `s.ends_with("slot-content-id")`, which also matches a longer key such as `x-slot-content-id`.
- Production matches exactly: `is_studio_carrier_key` uses `key == STUDIO_SLOT_CONTENT_ID_KEY`
  (`fig_export.rs:29-34`) and the importer matches the key in a `match` arm (`fig.rs:1346`). The kiwi
  entry carries the plugin id and the key as separate fields (`studio_entry`, `fig_export.rs:693`).
- A second loose check exists in `designos-fig/tests/fig_export_real_tags.rs:135`:
  `k.starts_with(&format!("{STUDIO_PLUGIN_ID}:slot-content-id"))` over `plugin_data` keys, which would
  also match `designos-studio:slot-content-id-extra`.

### Rule

Test-only change: the two checks match the exact key. In `fig_slots.rs`, compare the entry's `key` field
with `==` and its `pluginID` field with `STUDIO_PLUGIN_ID`; in `fig_export_real_tags.rs`, compare the whole
`plugin_data` key with `==` against `"designos-studio:slot-content-id"` and `"designos-studio:author-visible"`.
No production change.

### Fixtures and tests

| Case | Expected |
|---|---|
| `slot_scan_counts_only_the_exact_key` | a fixture with a node carrying `slot-content-id` and another carrying `x-slot-content-id`: the count is 1 (fails with `ends_with`) |
| `real_tags_check_rejects_a_longer_key` | the `plugin_data` leak check does not pass vacuously: a fixture key `designos-studio:slot-content-id-extra` is not treated as the reserved carrier |

---

## 669.5 Playground pixels

### Current state (FACT)

Commit `9d9b13cc` ("express merged slot, playground and check-designs geometry in theme constants")
replaced literals in `dev/playground.rs` with theme constants. Comparing `ba711d14:playground.rs` with
`origin/main`:

| Element | Before | Now |
|---|---|---|
| dot-grid first column (`gx` start) | stage left + 10 | + 8 (`SPACE_INLINE`) |
| dot-grid last column limit | stage right - 6 | - 8 |
| dot-grid first row (`gy` start) | stage top + 26 | + 24 (`SPACE_SECTION`) |
| dot-grid last row limit | stage bottom - 6 | - 8 |
| "Read-only" badge top | stage top + 6 | + 4 (`SPACE_ROW`) |
| badge corner radius | 4 | 6 (`RADIUS_CONTROL`) |
| gap after the property rows (`playground.rs:575`) | 6 | 8 (`SPACE_INLINE`) |

All other replaced literals in that commit equal their constants (`canvas/draw.rs`, `panels/check_designs.rs`,
`panels/components.rs`, the playground's other offsets: 28 = `CONTROL_STANDARD`, 24 = `CONTROL_DENSE`,
12/24/12/8 stage insets).

### Rule

This is an owner-manual visual check: an agent does not mark it done. The worker captures the Dev Mode
component playground (Dev Mode, a component selected; `designos --screenshot` with the Dev panel
fixture) at the current commit and at `ba711d14` (build in a scratch worktree) and attaches both images
and the table above to the issue. The owner confirms or rejects the 2 px changes. If rejected, the
change is a one-line edit per row of the table in `playground.rs`, with the spacing ratchet
(`scripts/check_spacing_relationships.py`, `.github/workflows/design-gates.yml`) kept green, so the old values must be expressed as constants or with
a documented exception.

### Fixtures and tests

No automated test. Evidence: two 1440 captures (before and now) and the table; status "owner-manual".

---

## Dependencies and build order

The five subsections are independent. 669.1 and 669.4 both touch `designos-fig/tests/fig_slots.rs`; do
them in one worker's two commits.

## Owner rulings

- Verified decisions are not reversed by abstract audit concerns: 669.1 and 669.2 keep today's behaviour
  unless a test shows a defect.
- Visual changes are confirmed by the owner (669.5), never claimed by an agent.
- No client files in fixtures: every fixture here is synthetic.

## Could not ground

- 669.1: which single assertion of the #514 tests forces the bare `s:l` form. The worker finds it by
  running the slot tests against a `comp:` variant first.
- 669.2: what Figma itself treats as null for a half-null GUID. No reference file may be read; the
  intended behaviour is inferred from this repository's own comments and paste code.
