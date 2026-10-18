# #68 Recolour text runs when a colour style is applied from the app (apply_style_in)

- Issue: https://github.com/jangtrinh/design-os-studio/issues/68 (found in the review of #44; epic #16)
- Spec base: `main` at `767acf9`. PR #45 is merged, and its 2B text-fill change is on `main`.
- Open PR: #575 (`task/68-recolour-text-runs-apply-style-in`, +49 −11). It is test-only: it covers the existing behaviour.
  - The release review, head `5d4b320`, raised one MAJOR asking that every run take the style colour. That contradicts owner ruling 2B; see §13. The finding is rejected on that ground.
  - The owner ruled on 2026-10-18 that the release session handles this PR.
- Files: `crates/designos-app/src/styles.rs` (tests), `crates/designos-app/src/check_designs/tests/text_run_fills.rs`.

## 1. Summary and user story

As a designer applying a colour style to a rich-text layer, the visible text should take the style's colour. A word I deliberately highlighted in another colour keeps it. One undo restores everything.

## 2. Current state

- FACT: `apply_style_in` (`styles.rs:81-102`), in the `Style::Fill` arm for text (`styles.rs:91-98`), replaces `t.style.fills` with the style paints. It also sets every run whose fills equal the old base fills to the style paints. Runs with other fills keep theirs. It records `style_refs["fill"]`.
- FACT: `designos_doc::set_text_fills_everywhere` (`text_fills.rs:11`) sets the base and every run. It is used by the Check designs contrast fix.
- FACT: `apply_style_values` (updating a linked style) recolours every run (`styles_ops.rs:463`, per the review).
- FACT: `apply_style` commits through one `run_tx` (`ops.rs:142`), so it is one undo step.

## 3. Behaviour rules

1. **Applying a colour (fill) style to a text layer** (`apply_style_in`, `Style::Fill`, `prop != Stroke`):
   - `t.style.fills` becomes the style paints;
   - each run whose `style.fills == old base fills` becomes the style paints;
   - each run with different fills keeps them;
   - `style_refs["fill"] = key`.

   This is owner ruling 2B.
2. **Binding a colour variable to a text fill** follows the same rule. It is implemented in `variables_apply.rs` (PR #45, `2fae41c`). This task only asserts it.
3. **Updating a linked style's value** (`apply_style_values`) keeps its all-runs behaviour (ruling 2B(b)).
4. **One undo step** restores the base fills, the matched runs and the highlighted run.
5. **Both paths share one rule.** The doc-side helper for rule 1 is `designos_doc::text_fills` (a "replace matching runs" sibling of `set_text_fills_everywhere`). `apply_style_in` calls it instead of the inline loop at `styles.rs:94-97`, so the app and doc paths cannot drift. This was the original intent of #68; the issue text ("use `set_text_fills_everywhere`") predates ruling 2B.

## 4. Data model / schema changes

None.

## 5. Contracts

`pub fn replace_text_fills_following_base(text: &mut TextProps, paints: &[Paint])` in `designos-doc/src/text_fills.rs`, exported from `lib.rs`. `apply_style_in` and `variables_apply` both call it.

## 6. File-format mapping

None. RESOLVED.

## 7. UI

None. Evidence is the terminal test output: the issue has no visual change because behaviour is already on `main`. Visual evidence rule: attach a 1440 capture of a rich-text layer before and after applying a colour style (body recoloured, highlighted word kept).

## 8. Edge cases

1. A single run covering all the text, equal to the base: the run follows the style.
2. Body blue (`#1A73E8`) runs plus one red (`#E5484D`) highlighted word; apply `Brand/Primary` (`#010203`). The body runs become `#010203`, the red word stays red.
3. No runs (plain text): the base becomes the style.
4. Base fills empty, runs with colours: no run equals empty, so the runs keep their colours. The base becomes the style.
5. Applying the same style twice: idempotent.
6. Undo after edge case 2: the base is blue, the body runs are blue, the word is red.

## 9. Non-goals

- Changing `apply_style_values` (2B(b)).
- Tooltip changes (2B(c)).

## 10. Test plan

Seam: `crates/designos-app/src/styles.rs` `#[cfg(test)]`.

- `applying_fill_style_to_rich_text_recolours_runs_and_undoes_in_one_step` (PR #575): runs `[0..5 blue bold, 5..11 blue italic, 11..15 red]`; apply a style with `#010203`. Assert:
  - the base and runs 0 and 1 are `#010203`;
  - run 2 is `rgb(229, 72, 77)`;
  - one `undo()` restores all four.
  - Fails before: no. The behaviour already landed with PR #45, so this test guards it. The fail-before requirement is met by `apply_style_in_and_variable_bind_share_one_text_fill_rule`.
- `apply_style_in_and_variable_bind_share_one_text_fill_rule`: the same input through `apply_style_in` and through binding a colour variable gives equal `TextProps`.
  - Fails before: yes until rule 5's shared helper exists. Mutation check: change the inline loop only, and the test fails.

## 11. Performance budgets

None of the `CLAUDE.md` §4 invariants is touched.

## 12. Dependencies and build order

- Depended on #45 (merged).
- Independent of the other Check designs tasks.

## 13. Owner rulings that apply

- **2B** (owner ruling on PR #45, 2026-10-18): "When a variable is bound to a text fill, or a fill style is applied, a style run whose colour equals the previous base colour follows the new value. A run with another colour keeps it. (b) `apply_style_values` keeps its current all-runs behaviour. (c) no tooltip change."
- The issue AC "recolours every run" was written on 2026-10-17, before 2B. Under the rule "decisions recorded by the owner win; never silently override", 2B governs. The release review's MAJOR on PR #575 asks for the pre-2B behaviour and is rejected. The issue AC should be amended to cite 2B; this is listed in the README.
