# Studio UX writing

The canonical writing standard for DESIGN:OS Studio UI copy: buttons, menus, toasts, empty states, dialogs, accessible names, docs and Figma copy. Help the user understand the content, choose an action and know its result. Apply it to new or edited copy. It does not rewrite existing strings; a separate change does that, and the gate's baseline (below) lists what is still to rewrite.

Authority: owner ruling, 2026-10-21: "DESIGN:OS Studio adopts VSF PCP's UX writing standard as-is, enforced by a gate + review." Then [`design/principles.md`](principles.md) (P5 on counts, P6 on honest UI), then [`design/soul.md`](soul.md). An exact owner ruling governs its case. This is a port of VSF PCP's `design/ux-writing.md`: the statements and intent of rules 1 to 14 are kept, and PCP-only evidence (i18n catalogs, PCP decision ids, PCP gates) is replaced by Studio evidence.

Studio UI is English and has no i18n catalogs: strings live in Rust source. Examples cite `crates/designos-app/src/<file>.rs:<line>`, inspected 2026-10-21 on `release/v0.6.0`; paths below are relative to `crates/designos-app/src/`. "Proposed" means a suggested edit to that real string, not a claim that it shipped. A don't may show misuse of otherwise valid copy in a different context.

Roles stay separate. Action labels (rule 1), prose and messages (rules 2, 5, 6), headings, and accessible names (rule 14) follow different limits. Do not apply the action-label cap to prose or accessible names.

## 1. Name the action briefly

**Statement.** Visible button, link-button and menu-item action labels use verb + object, at most three words, with no articles or prepositional tails. An exact product or app name counts as one word; a separate count badge does not count. Do not abbreviate a canonical name or invent hyphenation to pass. Prose, headings and accessible names are different roles.

**Do / don't.** Keep `Save as…` (`panels/menus.rs:429`) and `Reset all changes` (`panels/context.rs:101`). Replace the five-word `Mark as ready for dev` (`dev/sections.rs:67`, `panels/context.rs:110`) with Proposed `Mark ready`. Replace `Open in new window` (`home/mod.rs:508`) with Proposed `Open window`. Replace `Export selection as PNG…` (`panels/menus.rs:434`) with Proposed `Export PNG…`, keeping the selection scope in the accessible name (rule 14). Three words alone do not satisfy the rule: the object must still be named.

**Enforcement.** Gate Z16 in `scripts/zero_fake_ui_gate.py` (word count, ratchet). Review for verb + object meaning and for articles and prepositional tails, which the gate does not judge.

## 2. Keep text that changes understanding

**Statement.** Remove visible repetition of a nearby title, badge or label, descriptions of the UI itself, and filler without information or a decision. Judge the rendered context, not the string in isolation. Keep orientation, safety, distinct destinations and useful absence information. Do not delete a shared string or a required container because one use is redundant.

**Do / don't.** Keep `No projects yet` (`home/sidebar.rs:86`): it states the loaded-empty state where the list would be. Don't add a second line under it that repeats that the list is empty.

**Enforcement.** Review. Duplicate detection can suggest candidates but cannot decide this rule.

## 3. Separate counts from labels

**Statement.** Counts beside titles, links, buttons, tabs and reveals use the count pill (`Badge/StatusCount`, P5) with a plain label. Do not put the count in parentheses or inside the visible action phrase. Keep the quantity in the accessible name and in necessary hidden announcements. Show the same nearby fact once. This does not ban numbers in informative sentences or outcomes.

**Do / don't.** Don't write `Absolute fallbacks ({})` (`shell.rs:1074`) or `Notes ({})` (`shell.rs:1117`), nor the same pattern for `Font substitutions`, `Uncaptured images` and `Unsupported CSS` in the same panel. Proposed: the plain label `Notes` with a count pill, and `Notes, {n}` as its accessible name. `Layers (12)` is the example P5 already bans.

**Enforcement.** Gate Z16 (count in parentheses, a digit or a formatted `.len()` inside an action label, ratchet; it reads the registered call sites, and `draw_kv` headings that format a `.len()`). Review for contextual repetition.

## 4. Make controls explain themselves

**Statement.** A visible control communicates its consequence before activation. Apply rule 1 to the action wording, and give paired actions one primary and quieter secondaries (P2: one primary CTA per view). Only the side-panel show/hide exception permits an icon-only control here: an adjacent title supplies the object, the glyph is the design system's `Icon::Sidebar`, and a tooltip on hover and keyboard focus plus an accessible name states the action. On reopening, keep the panel's object identifiable; otherwise use a labeled control. Do not extend this exception to arbitrary icons.

**Do / don't.** Pair `Save` as the primary with `Cancel` as the quieter secondary (`comments/thread.rs:213`, `:216`). Use `Hide sidebar` / `Show sidebar` (`panels/left.rs:56`, `shell.rs:1423`) as the tooltip and accessible-name strings for the sidebar control. Don't give two buttons in one bar the primary fill, and don't use only `Sidebar` to name the hide/show action.

**Enforcement.** Review for consequence, adjacency and hierarchy. No gate yet.

## 5. Keep microcopy to one useful idea

**Statement.** Toasts, inline messages and helper text aim for roughly eight words, one idea per sentence. This is a brevity target, not a license to remove a consequence or a recovery step. Do not add field helpers, page subtitles or section captions only to explain context: improve the label, or keep implementation context in a code comment. Docs explanations are not eight-word microcopy.

**Do / don't.** Keep `Copied properties` (`ops.rs:752`) as a concise, specific result. Proposed: `Copied Spec Markdown` for `Copied Spec Markdown (.md) to clipboard` (`specs.rs:2756`), and `Copied MCP config` for `Copied designos-mcp (22 Tools) config to clipboard` (`panels/right.rs:211`), which also hard-codes a tool count that will drift.

**Enforcement.** Review. Message length is never an automatic gate: a long message may carry a consequence.

## 6. Distinguish empty, loading, failure and success

**Statement.** State what happened and the next available action. Never present a loading or failed request as an empty result. An empty message can be omitted only when the visible context already conveys the loaded-empty state. Errors name the blocked operation and a real recovery path. Success names the completed operation only after it succeeds; partial results say what did and did not happen. Before destructive actions keep the consequence and scope, even when brevity loses (P6: honest results).

**Do / don't.** Keep `No files yet` with its next step (`home/main_view.rs:181`). Keep `Export .fig failed: {e}` (`shell.rs:1655`): it names the blocked operation and the cause. Don't announce success such as `Copied Spec deep-link to clipboard` (`specs.rs:2769`) before the write has succeeded. Proposed for `No file path open to sync` (`shell.rs:1568`): `Save the file first to sync`, which names the recovery.

**Enforcement.** Review. No state-fixture gate yet.

## 7. Reuse each domain's status vocabulary

**Statement.** Use the approved label and tone for a status, not a newly coined synonym. Do not merge distinct domain sets into one global list. If no approved label fits, flag the gap for an owner decision before shipping.

**Do / don't.** Dev Mode statuses are the `DevStatus` set (`ReadyForDev`, `Completed`, `Changed`; `crates/designos-doc/src/dev.rs:17-20`): reuse those words in buttons, badges and toasts, as in `Mark completed` (`dev/sections.rs:72`). Don't invent `Done` or `Finished` for the same state. Terminal run states (`Running`, `Idle`, `Exited`, P7) belong to agent terminals only.

**Enforcement.** Review. No typed status-vocabulary gate yet.

## 8. Identify people exactly

**Statement.** A specific person's identity in attribution, ownership, approval and history is shown as the product stores it, never as a made-up short name or an "email Name" phrase. Role labels are allowed where the product defines a role (for example the viewer). The exception must not make history anonymous or the identity inaccessible.

**Do / don't.** A comment shows its stored author (`comments/ui.rs:37` takes the `author`; the avatar at `comments/ui.rs:13` is only the initial and must not be the only identity in a thread or history row). Don't replace a stored author with a friendly first name.

**Enforcement.** Review.

## 9. Use the product's vocabulary in its domain

**Statement.** Take terms and rejected synonyms from [`CONTEXT.md`](../CONTEXT.md) (`Document`, `TileView::rev`, `SceneIndex`, Compact Native Format), with the concept's domain attached. Keep product names, identifiers, file extensions and quoted user data verbatim. Do not apply an `_Avoid_` term as a global text ban: a word rejected for one concept may correctly name another. A new synonym is a vocabulary decision, not a copy cleanup.

**Do / don't.** Keep `.fig`, `.designos` and `design:os` spelled as the product spells them (`panels/inspect.rs:489`). Don't call the same thing `board`, `canvas` and `page` in neighbouring labels.

**Enforcement.** Review.

## 10. Use English chrome and deliberate localization

**Statement.** Studio chrome is English. New or edited strings are English, keep business and user data verbatim, and do not coin translated substitutes for technical terms. If Studio gains localization, an owner decision extends this rule with its own catalog; until then, do not add partial translations.

**Do / don't.** Keep technical terms in English as they appear (`Code Connect`, `MCP`). Don't add a one-off translated string to a single panel.

**Enforcement.** Review.

## 11. Make capitalization and punctuation serve meaning

**Statement.** Sentence case for actions, headings and short messages. Preserve canonical product and acronym spelling (`PNG`, `MCP`, `design:os`). Labels and short toast fragments need no terminal period; complete explanatory sentences do. Use a colon only when it introduces following content. A middle dot (` · `) separates a completed result from a shortcut hint in the same toast. Use an ellipsis (`…`) only for an actual ongoing operation or deliberately unfinished input. Do not add enthusiasm punctuation or title-case every word.

**Do / don't.** Keep `Stopping…` (`agent_stop.rs:105`): the operation is ongoing. Don't add an ellipsis to a new action label only because it opens a dialog; existing menu strings such as `Save as…` (`panels/menus.rs:429`) predate the rule and are not rewritten here. `Copied properties` (`ops.rs:752`) needs no period. `Code Connect` (`panels/inspect.rs:511`, inspected on `release/v0.6.1`) keeps both capitals because it is the feature's canonical name; use sentence case elsewhere. Write `Layers renamed · Cmd+Z to undo` (Proposed for `shell.rs:1730`): the result comes first, then ` · `, then the hint, as the Check designs toast does (`panels/check_designs.rs:299`). Don't put the hint in parentheses, as in `Applied {n} Figma AI change(s) (Cmd+Z to Undo)` (`shell.rs:1730`).

**Enforcement.** Review. No style gate yet.

## 12. Preserve quantities, dates and precision

**Statement.** Derive live quantities from source data and label units, period and scope where they are needed to read a number. Do not turn an unknown value into zero or invent freshness. Numeric values render in `Roboto Mono` (P5). State the time zone when a time would otherwise be ambiguous. Apply rule 3 to counts next to labels.

**Do / don't.** Don't write `22 Tools` into a string (`panels/right.rs:211`): derive it or drop it. Show an unknown measurement as `--`, not `0`.

**Enforcement.** Review. P5 (mono numerics) is covered by the existing token and type checks.

## 13. Tell users when a link leaves the app

**Statement.** A link that opens a browser or another app says so with the external-link glyph, and the accessible name carries the destination and the new-window behavior. Do not apply this to in-app navigation. The visible label follows rule 1 and the accessible name follows rule 14.

**Do / don't.** `design:os studio on GitHub` (`panels/menus.rs:532`) is a four-word label that leaves the app. Proposed visible `GitHub repo` with accessible name `design:os studio on GitHub (opens in browser)`.

**Enforcement.** Review.

## 14. Keep the full meaning accessible

**Statement.** Accessible names (`widgets::labelled`, `widgets/mod.rs:224`) preserve the action, object and necessary count, scope, format or destination even when the visible label is compact. Include the visible label's words in the same order, adding context without contradicting it. Name icon controls, expose their state, and keep required result announcements. Tooltips supplement accessible names, never replace them. Do not apply rule 1's word cap to them.

**Do / don't.** Visible `Export PNG…` pairs with the accessible name `Export selection as PNG`. Visible `Notes` plus a count pill pairs with `Notes, 3`.

**Enforcement.** Review. No computed-name gate yet.

## Gate and review

| Rule | Mechanical check | How |
|---|---|---|
| 1 | Word count of action labels (max 3), ratchet | `python3 scripts/zero_fake_ui_gate.py`, invariant Z16 |
| 3 | Count in a label: parentheses, digits, a formatted `.len()`; ratchet | same, Z16 |
| 2, 4 to 14 | none yet | review of the final diff against this file |

**What Z16 reads.** Three things, from shipped code only (comments and tests stripped):
1. The label argument of the registered button and menu helpers: `MenuItem::new`, the menu builders `leaf` / `submenu` / `stub_sub`, `text_button`, `primary_button`, `small_button`, `slide_button`, `paint_card_button`, `add_row`, and the module `button` helpers in `home/`, `comments/`, `plugins/` and `libraries/` (`LABEL_CALLS` in `scripts/ux_writing_labels.py`).
2. A label bound with `let` earlier in the same fn and passed by name to one of those helpers.
3. Label-returning fns, every literal in the body (`LABEL_FNS`): `fix_label`, `action_label`, `button_label` in `agent_*.rs` and `terminal.rs`.

Studio has no egui `Button::new` / `.button` / `.selectable_label` call sites, so they are not registered. Placeholders such as `{name}` are ignored when counting words; the product names listed in the scanner count as one word. A count inside the action phrase is flagged as a digit token (`Delete 3 layers`) or a placeholder formatted from a `.len()` (`&format!("Delete {}", sel.len())`); a count held in a non-`.len()` variable (`n`, `count`) cannot be seen and stays review-only. Not read: labels in data tables (for example the `(id, label, title, prompt)` tuples in `terminal.rs`), checkbox and toggle captions, and any helper not yet registered. Register a new helper in the same change that introduces it; the gate title says "registered label helpers", not the whole app.

**Ratchet.** `scripts/ux_writing_baseline.json` lists the violations that existed on adoption (file, kind, literal, count; no line numbers). A listed violation passes. A new one fails and cannot be absorbed: rewrite the label. A listed one that no longer occurs fails with "fixed, remove from baseline": run `python3 scripts/zero_fake_ui_gate.py --write-ux-baseline` and commit the smaller file. The writer refuses to add a key or raise a count, so the baseline only shrinks; a label that moved to another file is accepted as a move. `--allow-grow` exists only to bootstrap the baseline after a scanner coverage change (the registered tables grew) and needs reviewer sign-off in the PR; never use it to make a copy change pass.

**CI.** `.github/workflows/design-gates.yml` runs the gate as advisory (`continue-on-error`), and the self-test runs in no workflow. Enforcement is the release session's local run (`docs/releasing.md`, `design_os_self_evolve.py` G02). A red Z16 blocks a merge by that rule, not by a required check.

**Review.** Every change that adds or edits user-visible copy needs a saved reviewer PASS before merge: a review verdict (`PASS` or `FIX FIRST`) that cites the rule numbers, saved beside the worker report under `plans/reports/`. FIX FIRST blocks merge, and the final diff is reviewed again after a correction changes copy. Existing gate coverage and new-gate construction are separate from this review requirement. Message length (rule 5) and meaning (rules 1, 2, 4, 6 to 14) are review-only, and a passing gate does not establish semantic correctness.
