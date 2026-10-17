# design-os-studio: current feature inventory (Haiku scan, 2026-10-17, branch review-fixes-261016 @ f5f02bc)

Read-only scan; cargo not run. "README claim" means the README says so and the code was not checked.

## Parity meter caveat (FACT)
- `scripts/figma_parity_meter.py` checks only that a regex pattern exists in source (`has_any`/`has_all`, lines 60-75). It does not test behaviour.
- The scorecard (`plans/specs/designos-studio-v2/research/parity-scorecard.json`) reports 387/387: P1 kiwi schema 203, P2 menu actions 68, P3 per-element checks 85, P4 UI3 surfaces 16, P5 AI/PTY/bridge 15.
- README:410 says 487/487, which does not match.
- Conflicts found:
  - The meter passes M36/M37/M41 (Outlines, Pixel preview, Annotations), but those rows were removed as fake (panels/menus.rs:1034 test).
  - A04 claims EN/VI/JA rewrite; there is no JA mode.
  - A01 "Make Design" is template synthesis, not generative.

## By area
**Canvas & tools.**
- Tools (canvas/tools.rs:7): Move, Hand, Scale, ShapeBuilder, Frame, Section, Slice, Rect, Line, Arrow, Ellipse, Polygon, Star, Image, Pen, Pencil, Text, Comment, Sticky, BoardShape, Connector, Stamp, Marker, Highlighter.
- Bottom toolbelt and dev-mode tool swap (toolbar.rs).
- Zoom presets, pixel grid, snap, rulers, layout guides.
- Cmd+K palette (cmdk.rs).
- Find/replace (find/).
- Multi-tab bar (tabs.rs).
- Context menu (panels/context.rs).
- README claims: eyedropper, deep-hit picker.
- NO multiplayer cursors.

**Layers & selection.**
- Layers tree with visibility, lock and drag reorder (panels/layers.rs); pages list and drag (left.rs).
- Group, frame selection, ungroup.
- Select all / inverse / same fill-stroke-effect-font-instance; select parent/children.
- Mask (node.rs:62 is_mask); ready-for-dev status.

**Shapes/vector/boolean.**
- NodeKind Rect, Ellipse(arc), Line, Polygon, Star, Vector (network with per-vertex radius); vector edit mode (canvas/vector.rs, panels/vertex.rs).
- Boolean ops union/subtract/intersect/exclude (render/boolean.rs).
- Flatten, outline stroke (vecops.rs).
- Join/split/simplify/shape builder are menu labels only (meter M57-60).
- Stroke: per-side weights, align, cap, join, miter, dashes (paint.rs:254, panels/stroke.rs).

**Text.**
- TextProps: runs, auto_resize, max_lines, paragraph, lists, direction, decoration_skip_ink (node.rs:533).
- Rich text runs; in-canvas editing with undo coalescing.
- B/I/U/strike/lists; typography panel with a 3-tab type-settings popover.
- Text measurement; background open-source font loading.
- Hyperlink field (no UI checked).

**Auto layout/grid/constraints.**
- AutoLayout: direction, wrap, gaps, padding, align, hug/fill/fixed, stroke_included, reverse_z (node.rs:628).
- Child: absolute, grow, min/max, grid col/row and spans.
- CSS grid via FrameProps.grids with track pills.
- Constraints min/max/center/stretch/scale (node.rs:672).
- Scale tool K.
- README claims: suggest auto layout, tidy up.

**Components/variants.**
- Component, ComponentSet, Instance; ComponentLink with overrides, props, values, bindings, variant_props (node.rs:219).
- Create component, combine as variants, detach, go to main, reset (comp_ops.rs).
- Instance inspector with props and swap (panels/components.rs).
- Assets panel with library components and search.

**Styles & variables.**
- Variables: color/number/string/bool/alias; modes; collections and groups (variables/); `=` bind picker.
- Color and text styles (styles.rs).
- Library styles/variables (libraries/).
- .fig variable import; CSS var codegen.

**Effects & paints.**
- Effects: drop/inner shadow, layer/background blur, noise, texture, glass (paint.rs:267).
- Paints: solid, linear/radial/angular/diamond gradients, image (paint.rs:188).
- 18 blend modes.
- Color picker with WCAG contrast.
- Per-corner radius and smoothing.
- Copy/paste properties.

**Prototyping.**
- Triggers: click, drag, hover, press, enter/leave, down/up, delay, key, gamepad (enum only; runtime unverified).
- Actions: navigate, overlay open/swap/close, back, scroll-to, url, change-to.
- Transitions: dissolve, smart animate, move/push/slide.
- Easing including spring.
- Flows, device, noodles, present player, smart-animate matching, video fills, auto-wire flows.

**Dev Mode.**
- Dev toggle and tools.
- Codegen: CSS, Tailwind, React, SwiftUI, Compose (no Vue/HTML/Flutter).
- Dev status ready/completed/changed; dev links, annotations, compare changes.
- Measure tool; inspect panel with Code Connect label and tokens.
- Copy as SVG/CSS/PNG; Design Gate a11y audit.

**Comments/collab.**
- Local single-user comment threads, pins, resolve and reply (comments/).
- MCP comment tools; send selection to agent terminal.
- NO real-time multiplayer and NO share links.

**Export/import.**
- Export PNG/JPG/SVG/PDF (slides) with scale and per-node presets.
- .fig open and save; REST JSON, .pen JSON and SVG import (file and paste); .designos with repair.
- Image/video place; headless screenshot CLI; spec export.

**Files/projects/libraries/versions.**
- Home browser: recents, drafts, starred, templates, libraries.
- New design / board / deck; open, save, save-as, save version; autosave.
- Version history with auto and named versions, view, restore.
- Libraries: publish, enable, update review.
- Updater (currently hidden).
- Preferences: user name, GPU toggle.

**Plugins/widgets.**
- Native plugin bundles with loopback browser panels and scoped document access (plugins/); plugin store with network access.
- Dev "Plugins & Codegen" panel.
- No Figma-API-compatible plugin runtime and no FigJam widgets (INFERENCE from module docs).

**AI.**
- Make Design: hard-coded templates (cmdk.rs:615-858), no LLM client.
- Rule-based: rename layers, rewrite/shorten/translate EN↔VI, populate content, boost resolution.
- "Edit image with prompt" is text-only.
- Design gate audit and auto-fix.
- Agent PTY terminal (Claude/Codex/agy/Grok) with live spec bridge; spec reader.

**FigJam/board.**
- Board mode with its own toolbar, sticky, shape-with-text, connectors with magnets, stamp, marker, highlighter.

**Slides.**
- Native decks with speaker notes, PDF export, present.

**Sites.** None found.

**MCP.** 22 tools (engine.rs 18 + server.rs 4). generate_code supports css/tailwind/react/swiftui/compose/svg.

**NodeKind (node.rs:353).** Page, Frame, Group, Section, Rectangle, Ellipse, Line, Polygon, Star, Vector, Text, BooleanOp, Component, ComponentSet, Instance, Sticky, Connector, ShapeWithText, Stamp, Unknown.
