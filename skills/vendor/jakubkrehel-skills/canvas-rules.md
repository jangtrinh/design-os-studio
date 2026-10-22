<!-- Portions of this folder are copied from or derived from jakubkrehel/skills (https://github.com/jakubkrehel/skills), commit 4c66b1fa86e9ab70187b43d7da9b6fdb29f2ef63, Copyright (c) 2026 Jakub Krehel, MIT License (see LICENSE). -->

# jakubkrehel UI Rules (Canvas Edition)

> Portions of this folder are copied from or derived from jakubkrehel/skills (https://github.com/jakubkrehel/skills), commit 4c66b1fa86e9ab70187b43d7da9b6fdb29f2ef63, Copyright (c) 2026 Jakub Krehel, MIT License (see LICENSE).

## better-accessibility

- **`better-accessibility/forms#3`** (`rubric:color`): Show a field error as inline text or an icon beside the field; a red border alone is a colour-only cue and is not enough.
- **`better-accessibility/forms#6`** (`skill:generate-design/states`): Draw the submitting state of a button with its original label kept and a spinner beside it ("Save" plus spinner), never a bare spinner, and keep the button enabled until the request starts.
- **`better-accessibility/hit-areas#1`** (`skill:generate-design/interaction`): Give every drag interaction on a design a single-pointer alternative, such as move-up and move-down buttons on a sortable row or a menu on a kanban card.
- **`better-accessibility/motion-and-zoom#1`** (`rubric:motion`): Anything that moves, blinks or updates on its own for more than 5 seconds, looping hero video included, needs a visible pause or stop control drawn in the frame.
- **`better-accessibility/motion-and-zoom#2`** (`rubric:motion`): Prefer explicit dismissal to a timer: a toast that carries an action, an error or information the user may need stays on screen until the user closes it.
- **`better-accessibility/motion-and-zoom#3`** (`rubric:motion`): Where a toast must time out, show it for at least 5 seconds and pause its timer while it is hovered or focused.
- **`better-accessibility/motion-and-zoom#4`** (`rubric:motion`): Never place critical information, such as the only link to an undo action, in an element that disappears on a timer.
- **`better-accessibility/motion-and-zoom#5`** (`lint:text-truncated`): Design text so all content and functionality survives text scaled to 200%: give text frames room to grow instead of clipping; check with `designos_lint` text-truncated.
- **`better-accessibility/motion-and-zoom#6`** (`lint:overflow-parent`): Make the 320 px frame work with vertical scrolling alone, as a 400% zoom on a 1280 px viewport would; only genuinely 2D content such as tables, maps and code blocks scrolls inside its own container; check with `designos_lint` overflow-parent.
- **`better-accessibility/semantics-and-aria#7`** (`rubric:typography`): Treat headings as structure, not styling: name and order heading layers by level (H1, H2, H3) and give each level its text style, rather than choosing a level by how large it looks.
- **`better-accessibility/semantics-and-aria#10`** (`rubric:iconography`): Give an icon-only button a name: put a descriptive layer name on it (such as `Button / Close`) so exported specs and accessibility inspection carry a label, and keep the icon layer decorative.
- **`better-accessibility/semantics-and-aria#17`** (`lint:contrast`): Disabled controls are exempt from contrast minimums: draw the disabled state with muted tokens on purpose, and hold every active control to `designos_lint` contrast.

## better-colors

- **`better-colors/color-formats#5`** (`skill:generate-library/tokens`): Keep derived colours such as a `color-mix()` tint out of the token layer: store the resulting tint as an explicit colour variable value, because a mixed colour cannot be inspected in a design tool.
- **`better-colors/color-formats#6`** (`skill:generate-library/tokens`): Do not define a colour variable by chained derivations such as relative colour syntax that darkens an accent by hand: one derivation at most, otherwise the token is unreadable.
- **`better-colors/color-formats#7`** (`skill:generate-library/tokens`): Keep both appearances of a colour in one variable as its Light and Dark modes, the canvas equivalent of `light-dark()`, instead of two unrelated variables.
- **`better-colors/contrast#1`** (`lint:contrast`): A mid-lightness background (OKLCH L near 0.75) caps achievable contrast even for black text (about Lc 60); move body-text backgrounds toward one lightness extreme; check with `designos_lint` contrast.
- **`better-colors/contrast#2`** (`rubric:color`): When pushing lightness to fix contrast, reduce saturation as needed so the colour stays inside the document gamut (Display P3) and remains renderable.
- **`better-colors/contrast#3`** (`lint:contrast`): Check every foreground and background pair in every appearance: a pair that passes in the Light mode can fail in the Dark mode; run `designos_lint` contrast on both.
- **`better-colors/contrast#4`** (`lint:contrast`): For text on a translucent or background-blur surface, test against the lightest and darkest content that can sit behind it, or make the surface opaque enough that the shift cannot break the pair.
- **`better-colors/contrast#5`** (`lint:contrast`): Measure the rendered result of computed colours (mixes, relative colours, opacity modifiers), not the declaration; `designos_lint` contrast uses the resolved colour.
- **`better-colors/contrast#6`** (`lint:contrast`): For text over an image there is no single background colour: measure the worst region, or guarantee one with a scrim behind the text.
- **`better-colors/palette-generation#1`** (`rubric:color`): Step a ramp's lightness steadily in OKLCH L; evenly spaced HSL lightness is not perceptual and bunches at one end.
- **`better-colors/palette-generation#2`** (`rubric:color`): Hold hue constant from one end of a ramp to the other so every step reads as the same colour, and so it sits against a neutral built on a different hue.
- **`better-colors/palette-generation#3`** (`rubric:color`): Let vividness peak in the middle of a ramp and fall off at both ends: the lightest and darkest steps are nearly neutral, so the 50 does not glow and the 950 does not look like spilled ink.
- **`better-colors/palette-generation#4`** (`rubric:color`): Space ramp steps finer at the light end: about 0.04 to 0.05 OKLCH L apart for steps 50 to 200 and 0.07 to 0.10 mid-ramp, so there are enough pale steps for a subtle surface.
- **`better-colors/palette-generation#5`** (`rubric:color`): Keep adjacent ramp steps at least 0.03 OKLCH L apart; closer steps mean the ramp has more steps than decisions, so drop one.
- **`better-colors/palette-generation#6`** (`rubric:color`): Stop both ends of a ramp short of pure black and pure white, so the ramp keeps its identity where the page background lives.
- **`better-colors/palette-generation#7`** (`rubric:color`): Match perceived lightness exactly across hues: the same step has the same brightness in every ramp.
- **`better-colors/palette-generation#8`** (`rubric:color`): Match vividness across hues in proportion to what each hue can reach, not by one absolute chroma; a saturated yellow and a saturated blue are not equally far from gray.
- **`better-colors/palette-generation#9`** (`rubric:color`): Bring vividness down in dark appearances: an accent that reads confident on white reads neon on near-black, so use it a step or two less vivid.
- **`better-colors/palette-generation#10`** (`rubric:color`): Give the dark end of a ramp more separation between steps: steps that read as distinct pale backgrounds collapse into each other as dark surfaces.
- **`better-colors/palette-generation#11`** (`lint:contrast`): Contrast does not survive the mirror: recheck every pair in both appearances with `designos_lint` contrast, because mirrored pairs score differently.
- **`better-colors/palette-structure#1`** (`skill:generate-library/tokens`): Define colour steps by role, as Radix does: step 9 is the solid fill in every ramp and appearance, and the dark scale is a separate ramp reusing the same step numbers so components never change.
- **`better-colors/palette-structure#2`** (`skill:generate-library/tokens`): Where steps are defined by lightness, as Tailwind does (50 light, 950 dark), the mapping inverts in dark mode: components either swap step numbers per appearance or read one semantic token that swaps once.
- **`better-colors/palette-structure#3`** (`skill:generate-library/tokens`): Keep every status hue at least 15 degrees of OKLCH hue away from the accent; if the brand is red, danger cannot also be red, so give the destructive action a distinct treatment such as an outlined button with an icon.
- **`better-colors/palette-structure#4`** (`skill:generate-library/tokens`): Give status ramps fewer steps than the accent: most render four roles (background, border, solid fill, text); generate the full ramp only where the product styles status components across the whole range.
- **`better-colors/palette-structure#5`** (`lint:hardcoded-color`): Collect every colour literal in the document, including SVG fills and strokes and chart colours, before tokenising; check with `designos_lint` hardcoded-color.
- **`better-colors/palette-structure#6`** (`skill:generate-library/tokens`): Sort the collected colours by perceived lightness within each hue family so duplicates surface as near-identical neighbours.
- **`better-colors/palette-structure#7`** (`lint:hardcoded-color`): Collapse near-duplicate colours: two colours within 0.02 OKLab delta E are one colour that drifted, so keep the most used and retire the others, never average them.
- **`better-colors/palette-structure#8`** (`lint:hardcoded-color`): Assign every surviving colour a role from the palette structure; a colour that matches no role is a missing token or a mistake, and the finding says which.
- **`better-colors/palette-structure#9`** (`skill:generate-library/tokens`): Count the ramps left: more ramps than the system needs means the palette outgrew its structure, not that the product needs more colour.

## better-interface

- **`better-interface/SKILL#1`** (`skill:critique/workflow`): Run the accessibility review domain as one of the six domains of a critique.
- **`better-interface/SKILL#2`** (`skill:critique/workflow`): Run the layout review domain as one of the six domains of a critique.
- **`better-interface/SKILL#3`** (`skill:critique/workflow`): Run the writing review domain (UI copy) as one of the six domains of a critique.
- **`better-interface/SKILL#4`** (`skill:critique/workflow`): Run the typography review domain as one of the six domains of a critique.
- **`better-interface/SKILL#5`** (`skill:critique/workflow`): Run the colour review domain as one of the six domains of a critique.
- **`better-interface/SKILL#6`** (`skill:critique/workflow`): Run the UI craft review domain (motion, icons, interaction polish) as one of the six domains of a critique.
- **`better-interface/SKILL#7`** (`skill:critique/workflow`): Rate a finding HIGH when it blocks a task, misleads the user, hides content or controls, risks data loss, or repeats as a systemic failure.
- **`better-interface/SKILL#8`** (`skill:critique/workflow`): Rate a finding MEDIUM when it meaningfully harms comprehension, efficiency, adaptability or consistency.
- **`better-interface/SKILL#9`** (`skill:critique/workflow`): Rate a finding LOW when it is isolated polish with limited task impact.
- **`better-interface/SKILL#14`** (`lint:clipped-partial`): Flag content or a control that is clipped, overlapped or unreachable at 320 px width or 200% zoom; check with `designos_lint` clipped-partial, sibling-overlap and overflow-parent.
- **`better-interface/SKILL#15`** (`lint:contrast`): Flag body or control text whose rendered contrast pair fails its required ratio; check with `designos_lint` contrast.
- **`better-interface/SKILL#16`** (`rubric:color`): Flag any state or meaning carried by colour alone; pair it with an icon, a label or a shape.
- **`better-interface/SKILL#17`** (`skill:critique/workflow`): Flag a destructive action that has no confirmation, no undo and no distinct treatment.
- **`better-interface/SKILL#18`** (`skill:critique/workflow`): Flag truncated content that gives no way to reach the full value.
- **`better-interface/SKILL#19`** (`skill:critique/workflow`): Flag content or a control reachable only past a scroll edge or behind a disclosure that has no visible cue.
- **`better-interface/SKILL#20`** (`skill:critique/workflow`): Flag an error message that names no way to recover from it.
- **`better-interface/SKILL#21`** (`skill:critique/workflow`): Flag a semantic colour used against its meaning, such as the danger hue on a non-destructive action.
- **`better-interface/SKILL#22`** (`rubric:motion`): Flag a state change carried by motion alone: a colour, icon or label must remain when the animation does not run.
- **`better-interface/SKILL#23`** (`skill:critique/workflow`): Repair by deleting first: a separator that space would carry, an animation on a high-frequency interaction, an ARIA attribute a native element makes redundant, a ramp nothing imports.
- **`better-interface/SKILL#24`** (`skill:critique/workflow`): Repair by using the platform: the native element, the native control or the browser's own focus ring in place of a custom rebuild.
- **`better-interface/SKILL#25`** (`skill:critique/workflow`): Repair by reusing what the project has: an existing token, spacing step or motion curve before any new value.
- **`better-interface/SKILL#26`** (`skill:critique/workflow`): Repair by correcting the value: the wrong easing, radius, gap or contrast pair, using the exact value the owning skill gives.
- **`better-interface/SKILL#27`** (`skill:critique/workflow`): Repair by adding only what the platform cannot supply: a new token, a wrapper, a media query, an ARIA attribute.
- **`better-interface/review-format#1`** (`skill:critique/workflow`): Take each finding's severity from ranking by user impact.
- **`better-interface/review-format#2`** (`skill:critique/workflow`): Name each finding's domain as the owning review skill without the better- prefix.
- **`better-interface/review-format#3`** (`skill:critique/workflow`): Give each finding's location as the exact frame and component (node id and layer path) when the artifact has no source file.
- **`better-interface/review-format#4`** (`skill:critique/workflow`): Show a finding's Before (the current design) and After (an actionable replacement), each in its own cell.
- **`better-interface/review-format#5`** (`skill:critique/workflow`): State in Why the violated principle and its user impact.
- **`better-interface/review-format#6`** (`skill:critique/workflow`): End the review with Block when one or more HIGH findings remain; do not ship until they are fixed.
- **`better-interface/review-format#7`** (`skill:critique/workflow`): End the review with Approve when no HIGH finding remains; MEDIUM and LOW findings stay in the table as work to do.

## better-layout

- **`better-layout/grouping-and-alignment#1`** (`lint:off-scale`): Group related elements with negative space by default: on-scale auto-layout gaps and padding, no stray off-grid offsets; check with `designos_lint` off-scale.
- **`better-layout/grouping-and-alignment#2`** (`rubric:layout`): Use a background shape, a card or filled container, where a group must read as one unit, such as a selectable row or a draggable card.
- **`better-layout/grouping-and-alignment#3`** (`rubric:layout`): Use separator lines only for dense data where space costs too much, such as tables and long settings lists.
- **`better-layout/grouping-and-alignment#4`** (`lint:off-scale`): Fix stray edges: an icon 2 px off the text edge, or a card padded unlike its neighbour; check with `designos_lint` off-scale.
- **`better-layout/grouping-and-alignment#5`** (`lint:inconsistent-spacing`): Repeat the same spacing step in deeper nesting instead of inventing a new one; check with `designos_lint` inconsistent-spacing.
- **`better-layout/grouping-and-alignment#6`** (`rubric:typography`): Right-align numbers in tables (end) and left-align text (start); tabular figures belong to typography.
- **`better-layout/spacing-and-adaptivity#1`** (`rubric:layout`): Let a row of cards peek past the container edge: a row that ends exactly at the edge looks complete and nobody scrolls it.
- **`better-layout/spacing-and-adaptivity#2`** (`rubric:layout`): Give a collapsed section a chevron or a control that names what is hidden ("Show 12 more results", not "More").
- **`better-layout/spacing-and-adaptivity#3`** (`lint:text-truncated`): Pair every truncated value with an expand control, a link to a detail view or a tooltip that also opens on focus; hover-only hints do not count; check with `designos_lint` text-truncated.
- **`better-layout/spacing-and-adaptivity#5`** (`rubric:layout`): Design a modal that fits the screen height with a scrolling body and an action row outside the scroll area.
- **`better-layout/spacing-and-adaptivity#6`** (`rubric:layout`): Keep primary actions in stable chrome, such as a footer bar with safe-area padding or the top of the view.

## better-typography

- **`better-typography/SKILL#1`** (`rubric:typography`): Balance the line lengths of headings and short descriptions so the lines even out; never on paragraphs.
- **`better-typography/SKILL#2`** (`lint:text-unexpected-wrap`): Keep a lone word off the last line of a paragraph; check with `designos_lint` text-unexpected-wrap.
- **`better-typography/SKILL#3`** (`lint:overflowing-children`): Where a long word, link or ID could escape its container, let the text break inside the frame instead of overflowing it; check with `designos_lint` overflowing-children.
- **`better-typography/SKILL#4`** (`lint:text-unexpected-wrap`): Keep labels and badges on one line where a line break looks broken; check with `designos_lint` text-unexpected-wrap.
- **`better-typography/SKILL#5`** (`rubric:typography`): Size input text up on mobile frames (16 px) so the control stays legible and does not trigger zoom.
- **`better-typography/variable-fonts-and-opentype#1`** (`rubric:typography`): A static font is one weight and style per file: regular, medium and bold are three files, so count them when choosing a family.
- **`better-typography/variable-fonts-and-opentype#2`** (`rubric:typography`): Prefer a variable font where a whole weight range lives in one file and any value in it, such as 589, is usable on a text style.
- **`better-typography/variable-fonts-and-opentype#3`** (`rubric:typography`): Use real small capitals (the font's own small-cap glyphs), not uppercase letters set at a smaller size.
- **`better-typography/variable-fonts-and-opentype#4`** (`rubric:typography`): Use proper superscript and subscript glyphs (the 2 in x squared, H2O), not shrunken and shifted digits.
- **`better-typography/wrapping-and-punctuation#1`** (`rubric:typography`): Align a paragraph of three or more lines to its own script, so an English paragraph stays start-aligned left-to-right even inside a right-to-left interface; one or two line snippets follow the UI direction.
- **`better-typography/wrapping-and-punctuation#2`** (`rubric:typography`): Keep digit runs such as a phone number in their left-to-right order inside right-to-left text; repair a value that adjacent RTL text disturbs.

## better-ui

- **`better-ui/enter-exit#1`** (`rubric:motion`): Split an entering element into semantic groups such as title, description and actions, each animated as one layer group.
- **`better-ui/enter-exit#2`** (`rubric:motion`): Stagger the entering groups 100 ms apart in the prototype transition.
- **`better-ui/enter-exit#3`** (`rubric:motion`): Split a short display headline into words staggered 80 ms apart where it should read word by word.
- **`better-ui/enter-exit#4`** (`rubric:motion`): Combine opacity, blur and a small upward translate for each group's enter.
- **`better-ui/icons#1`** (`rubric:iconography`): Use one icon library per surface and never mix libraries on one toolbar; where it has no stroke variants, keep its native stroke and use size or colour for emphasis.
- **`better-ui/icons#2`** (`rubric:iconography`): Size an inline icon at 1 to 1.25 times the adjacent font size so the pair scales together.
- **`better-ui/icons#3`** (`rubric:iconography`): Test every icon at the smallest size it renders, often 16 px; it must stay recognisable there.
- **`better-ui/icons#4`** (`rubric:iconography`): Prefer a simplified glyph for small contexts over scaling detailed artwork down.
- **`better-ui/icons#5`** (`rubric:iconography`): Where the icon set ships per-size grids (16, 20, 24), use the grid that matches the render size; a 24 px drawing scaled to 16 px lands off the pixel grid and renders soft.
- **`better-ui/icons#6`** (`rubric:iconography`): Use vector SVG icons, never raster.

## better-writing

- **`better-writing/SKILL#2`** (`skill:generate-design/content`): List the noun used for each object and the verb used for each action ("project" or "workspace", "Delete" or "Remove") and keep them consistent in every frame.
- **`better-writing/SKILL#3`** (`skill:generate-design/content`): Note the capitalisation used for each element type and any voice or content style guide, and apply it to every label and heading.
- **`better-writing/SKILL#4`** (`lint:placeholder-text`): Name the action and the object in a confirmation title ("Delete 'Q3 report'?"), never "Are you sure?"; check generic placeholder strings with `designos_lint` placeholder-text.
- **`better-writing/SKILL#5`** (`skill:generate-design/content`): In a confirmation body say what is lost and what cannot be recovered, with counts where they apply.
- **`better-writing/SKILL#6`** (`skill:generate-design/content`): Write confirmation buttons as verb plus object ("Delete project", "Cancel"), never "Yes" and "No".
- **`better-writing/SKILL#7`** (`skill:generate-design/content`): For an account, workspace or repository deletion, ask the person to type the object's name.

## build-design

- **`build-design/SKILL#1`** (`skill:designos-use/workflow`): Report what is unwired: whatever renders from props until the backend exists.
- **`build-design/SKILL#2`** (`skill:generate-design/states`): Report the missing states: what the design does not draw.
- **`build-design/SKILL#3`** (`skill:designos-use/workflow`): Report domain conflicts: where the design breaks a rule, naming the owning skill.
- **`build-design/figma#1`** (`lint:hardcoded-value`): Prefer variable names over raw values: a fill bound to `gray/600` maps to the project's `gray-600` even where the hex drifted; a raw hex with no variable goes through the mapping table; check with `designos_lint` hardcoded-value.
- **`build-design/figma#2`** (`lint:detached-instance`): Prefer a Code Connect mapping over name matching: a node with a mapping names the exact component and props, so use that component unless it cannot express the design, and report the case; check detached copies with `designos_lint` detached-instance.
- **`build-design/figma#3`** (`lint:absolute-in-auto-layout`): Prefer auto layout over absolute positions: gap and padding on an auto layout frame are the spacing intent, absolute x and y on a floating layer is where someone dropped it; check with `designos_lint` absolute-in-auto-layout.
- **`build-design/figma#4`** (`skill:designos-use/workflow`): Prefer downloaded assets over Figma URLs: asset URLs from the server expire, so save each asset into the project and leave no Figma URL in code.

## explain-interface

- **`explain-interface/find-the-effect#1`** (`lint:overflow-parent`): Compare a child's box against its frame: a child wider than the frame, or with a negative offset, is oversized on purpose so its edges never show; confirm which with `designos_lint` overflow-parent.
- **`explain-interface/find-the-effect#2`** (`rubric:depth`): Read layers in paint order: within one stacking context a later layer paints over an earlier one and a higher layer over both; confirm the order at a pixel.
- **`explain-interface/find-the-effect#3`** (`lint:contrast`): A layer with a background blur frosts whatever is beneath it, so the layer beneath is part of the answer and of the contrast measurement.
- **`explain-interface/find-the-effect#4`** (`lint:contrast`): A layer with a blend mode takes its colour from what it covers, so name what is underneath before explaining or measuring it.
- **`explain-interface/from-an-image#1`** (`rubric:color`): Convert each sampled colour to OKLCH so lightness is comparable across hues.
- **`explain-interface/from-an-image#2`** (`rubric:color`): Sort samples by lightness to see whether they form a ramp, and report the spacing found: steps are expected to sit closer together at the light end.
- **`explain-interface/from-an-image#3`** (`rubric:color`): Compare hue across the ramp: hue is expected constant end to end, so report how far it moves.
- **`explain-interface/from-an-image#4`** (`lint:contrast`): Measure contrast on every foreground and background pair that can be isolated; it is exact and the most valuable single number an image gives; check with `designos_lint` contrast.
- **`explain-interface/from-an-image#5`** (`rubric:color`): Check the grays for a few percent of the accent hue and name a tinted neutral where one is found.
- **`explain-interface/from-an-image#6`** (`rubric:typography`): Name the typeface category (geometric sans, grotesque, humanist, transitional serif or slab) and say why.
- **`explain-interface/from-an-image#7`** (`rubric:typography`): Name the tells of the typeface: single or double-storey a and g, terminal angle, aperture, x-height against cap height, lining or old-style digits, tabular figures.
- **`explain-interface/from-an-image#8`** (`rubric:typography`): Count the distinct sizes, express them as multiples of the body size and derive the ratio.
- **`explain-interface/from-an-image#9`** (`rubric:typography`): Count the weights in use and estimate how far apart they sit.
- **`explain-interface/from-an-image#10`** (`rubric:typography`): Count characters on a full line of body copy and compare it with the 60 to 75 character measure.
- **`explain-interface/from-an-image#11`** (`skill:generate-design/states`): List the interactive states shown or missing: hover, focus, active, disabled, loading, empty and error.
- **`explain-interface/from-an-image#12`** (`rubric:motion`): Record whether anything animates and how.
- **`explain-interface/from-an-image#13`** (`rubric:layout`): Record the layout's behaviour at any other width.
- **`explain-interface/from-an-image#15`** (`skill:generate-design/states`): Provide or request the other appearance, light or dark, as its own frame.

## interface-review

- **`interface-review/SKILL#11`** (`skill:generate-design/states`): Flag a new variant, size or theme drawn for some states but not all of hover, focus, active, disabled, loading and selected.
- **`interface-review/SKILL#13`** (`skill:generate-design/states`): Flag a new component with no empty, loading, error, disabled or narrow-width state.
- **`interface-review/SKILL#14`** (`rubric:consistency`): Flag a control added to one surface but not to the sibling surfaces that already carry its peers.
- **`interface-review/removed-signals#5`** (`lint:hardcoded-color`): A colour literal replaced by a variable of the same value is a valid tokenisation fix, not a regression; check remaining literals with `designos_lint` hardcoded-color.

## state-machine

- **`state-machine/switcher#1`** (`skill:generate-library/components`): Give every group of a state showcase a heading, even when there is only one, and put the same small arrow icon on every heading.
- **`state-machine/switcher#2`** (`skill:generate-library/components`): List states and groups in the order they were written down.
- **`state-machine/switcher#10`** (`skill:generate-library/components`): With room, lay the showcase out as two columns: the state list, then the component at its production width.
- **`state-machine/switcher#11`** (`skill:generate-library/components`): With no room, put the state selector above the component in the page flow.

## technical-drawing

- **`technical-drawing/SKILL#1`** (`rubric:layout`): Draw the outer silhouette and major volumes of a technical illustration first.
- **`technical-drawing/SKILL#2`** (`rubric:layout`): Then place interfaces, cavities and fasteners where components join.
- **`technical-drawing/SKILL#3`** (`rubric:layout`): Then add the distinguishing functional details, such as ribs, slots, seals and strain reliefs.
- **`technical-drawing/SKILL#4`** (`rubric:layout`): Finally resolve occlusion and remove construction lines that do not explain the object.

## variant

- **`variant/SKILL#4`** (`lint:clipped-partial`): Make sure nothing in a variant clips, overlaps or becomes unreachable at 320 px width or 200% zoom; check with `designos_lint` clipped-partial and sibling-overlap.
- **`variant/SKILL#5`** (`lint:contrast`): Body and control text in every variant must pass its required contrast ratio; check with `designos_lint` contrast.
- **`variant/SKILL#6`** (`rubric:color`): In every variant no state or meaning rides on colour alone and no state change on motion alone.
- **`variant/SKILL#7`** (`rubric:consistency`): In every variant a destructive action has a confirmation, an undo or a distinct treatment.
- **`variant/SKILL#8`** (`lint:text-truncated`): Truncated content in a variant has a way to reach the full value; check with `designos_lint` text-truncated.
- **`variant/SKILL#9`** (`rubric:layout`): Nothing in a variant hides past a scroll edge or behind a disclosure with no visible cue.
- **`variant/SKILL#10`** (`rubric:consistency`): Every error shown in a variant names a way to recover.
- **`variant/SKILL#11`** (`rubric:color`): No semantic colour in a variant is used against its meaning, such as the danger hue on a non-destructive action.
- **`variant/SKILL#12`** (`rubric:consistency`): Before exploring variants, read the project's styling system, component library and any motion library, and stay inside them.
- **`variant/SKILL#13`** (`lint:hardcoded-value`): Read the tokens (colour, spacing, radius, type scale, easing) and bind each variant to them instead of raw values; check with `designos_lint` hardcoded-value.
- **`variant/SKILL#14`** (`rubric:consistency`): Respect the product's density and voice: a dense professional tool bounds how far the boldest variant may go.
- **`variant/SKILL#15`** (`rubric:consistency`): Design each variant for where the piece renders: against what background, beside which neighbours, at which widths.
- **`variant/picker#1`** (`skill:generate-design/variants`): Give every group of the variant picker a heading, even when there is only one, and put the same small arrow icon on every heading.
- **`variant/picker#2`** (`skill:generate-design/variants`): List variants and groups in the order they were written down.
- **`variant/picker#10`** (`skill:generate-design/variants`): With room, lay the variants out as two columns: the variant list, then the piece at its production width.
- **`variant/picker#11`** (`skill:generate-design/variants`): With no room, put the variant selector above the piece in the page flow.
