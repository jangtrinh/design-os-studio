# DESIGN:OS Studio — Agent Rules & Tool Pack

## Build-Verify-Fix Workflow

Follow this 7-step loop:

1. **Orient (Read first)** — Use bootstrap context or call `designos_get_document` and `designos_list_nodes` to inspect pages, frames, variables, styles, and components; do not dump the full document.
2. **Plan & Ground** — Plan sections and choose components/tokens via `designos_get_variables` and `designos_get_guidelines` (`topic: "design-system"`, `"archetypes"`, or `"web-app"`, `"mobile-app"`, `"landing-page"`, `"slides"`, `"table"`). Place new frames beside existing ones — never stack at `(0, 0)`.

3. **Build in sections** — Build one section per call (`designos_apply_batch` / `designos_apply_spec`). Put `in_progress: true` in the root node's spec. Use components as instances, `var:` / `style:` tokens, and auto layout (`layout`).
4. **Verify (`issues` + screenshot)** — Check `issues` (`{total, new_in_this_call, items}`) first; then render the finished frame with `designos_render_png` (<= 1024px) or `diff_image`, and inspect for hierarchy, alignment, clipping, and contrast.
5. **Fix in place** — Fix in place with update ops (`designos_update_node` or `designos_apply_batch` `update`); never delete and rebuild.
6. **Clear `in_progress` & audit** — Clear `in_progress` (`designos_set_in_progress`, `node_ids: [root]`, `value: false`), and call `designos_lint` (`preset: "agent"`, `"strict"`, or `"marketing"`).

7. **Errors & retries** — Follow `next_step`; resend `corrected_example` (or `valid_options`) when `safe_to_retry` is `true`; re-read state when `safe_to_retry` is `false`; `needs_human`: stop and ask the user; do not retry or work around; pass `idempotency_key` on writes.

Ask covers agent tabs started from Studio's Agents panel. The `designos` CLI you run yourself, and a `designos-mcp` started by another host, are not gated by Studio; that host approves its own MCP calls.

## Enabled MCP Tools

### `designos_get_document`
- **Purpose:** Summarize the current in-memory document: pages, counts,...
- **Args:** required: none
- **Errors:** `INVALID_ARGUMENTS`
- **Example:** `{}`

### `designos_get_libraries`
- **Purpose:** List this file's local publishable assets, enabled libraries...
- **Args:** required: none
- **Errors:** `INVALID_ARGUMENTS`
- **Example:** `{}`

### `designos_get_metadata`
- **Purpose:** Return a sparse XML node hierarchy (IDs, types, names,...
- **Args:** required: none (optional: `depth`, `include_hidden`, `lint`, `max_nodes`, `node_id`, `page_id`)
- **Errors:** `INVALID_ARGUMENTS`, `NODE_NOT_FOUND`, `VALUE_OUT_OF_RANGE`
- **Example:** `{"node_id":1,"depth":3}`

### `designos_search_design_system`
- **Purpose:** Search components, component sets, shared styles, and...
- **Args:** required: `query` (optional: `kinds`, `limit`, `property`)
- **Errors:** `INVALID_ARGUMENTS`, `VALUE_OUT_OF_RANGE`
- **Example:** `{"query":"button","kinds":["component"],"limit":10}`

### `designos_list_nodes`
- **Purpose:** List node summaries in numeric ID order.
- **Args:** required: none (optional: `limit`, `offset`, `parent_id`, `query`)
- **Errors:** `INVALID_ARGUMENTS`, `VALUE_OUT_OF_RANGE`, `NODE_NOT_FOUND`, `TEXT_TOO_LONG`
- **Example:** `{"parent_id":1,"limit":50}`

### `designos_get_node`
- **Purpose:** Read the full native node including geometry, paints, text,...
- **Args:** required: `node_id`
- **Errors:** `INVALID_ARGUMENTS`, `NODE_NOT_FOUND`
- **Example:** `{"node_id":2}`

### `designos_get_variables`
- **Purpose:** Read variable definitions and collections.
- **Args:** required: none (optional: `node_id`)
- **Errors:** `INVALID_ARGUMENTS`, `NODE_NOT_FOUND`
- **Example:** `{"node_id":2}`

### `designos_generate_code`
- **Purpose:** Generate developer handoff code or SVG for a layer using...
- **Args:** required: `node_id`, `language` (optional: `fragment`)
- **Errors:** `INVALID_ARGUMENTS`, `NODE_NOT_FOUND`, `UNKNOWN_LANGUAGE`
- **Example:** `{"node_id":2,"language":"react"}`

### `designos_create_node`
- **Purpose:** Create one layer in a container; defaults to x/y=0,...
- **Args:** required: `parent_id`, `kind` (optional: `diff_image`, `fill`, `fills`, `height`, `idempotency_key`, `lint`, `name`, `text`, `width`, `x`, `y`)
- **Errors:** `INVALID_ARGUMENTS`, `NODE_NOT_FOUND`, `NOT_A_CONTAINER`, `VALUE_OUT_OF_RANGE`, `UNKNOWN_VARIABLE`, `VARIABLE_TYPE_MISMATCH`, `AMBIGUOUS_VARIABLE`, `OUTSIDE_OWNED_FRAMES`
- **Example:** `{"parent_id":1,"kind":"frame","name":"Hero","width":1200,"height":640}`

### `designos_update_node`
- **Purpose:** Update specified fields of one non-page layer.
- **Args:** required: `node_id` (optional: `diff_image`, `fill`, `fills`, `height`, `idempotency_key`, `lint`, `name`, `opacity`, `text`, `visible`, `width`, `x`, `y`)
- **Errors:** `INVALID_ARGUMENTS`, `NODE_NOT_FOUND`, `PAGE_ROOT_PROTECTED`, `MOVE_CYCLE`, `UNKNOWN_VARIABLE`, `VARIABLE_TYPE_MISMATCH`, `AMBIGUOUS_VARIABLE`, `OUTSIDE_OWNED_FRAMES`
- **Example:** `{"node_id":2,"name":"Header","fill":"var:Color/Primary"}`

### `designos_delete_node`
- **Purpose:** Remove a non-page layer and its entire subtree.
- **Args:** required: `node_id` (optional: `diff_image`, `idempotency_key`, `lint`)
- **Errors:** `INVALID_ARGUMENTS`, `NODE_NOT_FOUND`, `PAGE_ROOT_PROTECTED`, `OUTSIDE_OWNED_FRAMES`
- **Example:** `{"node_id":3}`

### `designos_list_comments`
- **Purpose:** List canvas comment threads enriched with resolved world...
- **Args:** required: none (optional: `node_id`, `page_id`, `resolved`)
- **Errors:** `INVALID_ARGUMENTS`
- **Example:** `{"resolved":false}`

### `designos_add_comment`
- **Purpose:** Pin a new comment thread to a node (with local offset x/y) or...
- **Args:** required: `text` (optional: `author`, `idempotency_key`, `node_id`, `page_id`, `x`, `y`)
- **Errors:** `INVALID_ARGUMENTS`, `NODE_NOT_FOUND`, `TEXT_TOO_LONG`, `VALUE_OUT_OF_RANGE`
- **Example:** `{"node_id":2,"x":24,"y":16,"text":"Increase subtitle contrast"}`

### `designos_reply_comment`
- **Purpose:** Append a reply message to an existing canvas comment thread...
- **Args:** required: `thread_id`, `text` (optional: `author`, `idempotency_key`, `resolved`)
- **Errors:** `INVALID_ARGUMENTS`, `NODE_NOT_FOUND`, `TEXT_TOO_LONG`
- **Example:** `{"thread_id":"t1","text":"Updated to $easeui.text.primary","resolved":true}`

### `designos_resolve_comment`
- **Purpose:** Mark a canvas comment thread resolved (default true) or...
- **Args:** required: `thread_id` (optional: `idempotency_key`, `resolved`)
- **Errors:** `INVALID_ARGUMENTS`, `NODE_NOT_FOUND`
- **Example:** `{"thread_id":"t1","resolved":true}`

### `designos_apply_spec`
- **Purpose:** Build a complete UI subtree from a declarative NodeSpec...
- **Args:** required: `spec` (optional: `diff_image`, `idempotency_key`, `lint`, `parent_id`)
- **Errors:** `INVALID_ARGUMENTS`, `UNKNOWN_FIELD`, `NODE_NOT_FOUND`, `NOT_A_CONTAINER`, `UNKNOWN_COMPONENT`, `UNKNOWN_VARIABLE`, `UNKNOWN_SLOT`
- **Example:** `{"parent_id":1,"spec":{"kind":"frame","name":"Metric Card","preset":"easeui-card","layout":{"direction":"column","gap":12,"padding":[20,20,20,20]},"children":[{"kind":"text","name":"Label","text":"Net Revenue","font_size":13},{"kind":"text","name":"Value","text":"$148,920","font_size":28,"font_weight":600}]}}`

### `designos_export_spec`
- **Purpose:** Export a node and its descendants as a declarative NodeSpec...
- **Args:** required: `node_id`
- **Errors:** `INVALID_ARGUMENTS`, `NODE_NOT_FOUND`
- **Example:** `{"node_id":2}`

### `designos_set_dev_status`
- **Purpose:** Set a layer's Dev Mode handoff status ('ready', 'completed',...
- **Args:** required: `node_id` (optional: `annotation`, `idempotency_key`, `status`)
- **Errors:** `INVALID_ARGUMENTS`, `NODE_NOT_FOUND`, `TEXT_TOO_LONG`
- **Example:** `{"node_id":2,"status":"ready","annotation":"Semantic tokens"}`

### `designos_export_fig_file`
- **Purpose:** Export the current session document to a Figma binary .fig...
- **Args:** required: `output_path` (optional: `overwrite`)
- **Errors:** `INVALID_ARGUMENTS`, `INVALID_NAME`, `NAME_TOO_LONG`, `PATH_OUTSIDE_WORKSPACE`, `FILE_EXISTS`, `INTERNAL_ERROR`
- **Example:** `{"output_path":"handoff.fig"}`

### `designos_query_xpath`
- **Purpose:** Query layers in a page or subtree with a streaming...
- **Args:** required: `selector` (optional: `cursor`, `format`, `hardcoded`, `limit`, `max_scanned`, `node_id`, `page_id`, `with_problems`)
- **Errors:** `INVALID_ARGUMENTS`, `INVALID_SELECTOR`, `NODE_NOT_FOUND`, `CURSOR_STALE`, `RULE_UNAVAILABLE`
- **Example:** `{"selector":"//FRAME[@name='Metric Card']"}`

### `designos_lint`
- **Purpose:** Run the unified solver-backed geometric, text, token, and...
- **Args:** required: none (optional: `cursor`, `include_ignored`, `limit`, `node_ids`, `page_id`, `preset`, `rules`)
- **Errors:** `INVALID_ARGUMENTS`, `NODE_NOT_FOUND`, `VALUE_OUT_OF_RANGE`
- **Example:** `{"preset":"agent","limit":100}`

### `designos_get_guidelines`
- **Purpose:** List available design guideline topics or return the...
- **Args:** required: none (optional: `max_tokens`, `platform`, `topic`)
- **Errors:** `INVALID_ARGUMENTS`, `UNKNOWN_TOPIC`, `TOPIC_NOT_AVAILABLE`
- **Example:** `{"topic":"design-system","platform":"web"}`

### `designos_import_web_capture`
- **Purpose:** Import a .webcapture.json / .capture.json bundle (or inline...
- **Args:** required: none (optional: `capture`, `infer_block_auto_layout`, `page_id`, `path`, `x`, `y`)
- **Errors:** `INVALID_ARGUMENTS`, `PATH_OUTSIDE_WORKSPACE`, `FILE_TOO_LARGE`, `NODE_NOT_FOUND`
- **Example:** `{"path":"capture.webcapture.json","page_id":1}`

### `designos_undo`
- **Purpose:** Undo the latest successful edit in this session; returns...
- **Args:** required: none
- **Errors:** `INVALID_ARGUMENTS`, `HISTORY_DROPPED`
- **Example:** `{}`

### `designos_lint_fix`
- **Purpose:** Apply deterministic fixes for auto-fixable lint findings on a...
- **Args:** required: none (optional: `node_id`, `node_ids`, `page_id`, `preset`, `rules`)
- **Errors:** `INVALID_ARGUMENTS`, `NODE_NOT_FOUND`, `VALUE_OUT_OF_RANGE`
- **Example:** `{"page_id":1,"rules":["default-layer-name"]}`

### `designos_redo`
- **Purpose:** Redo the most recently undone edit in this session; returns...
- **Args:** required: none
- **Errors:** `INVALID_ARGUMENTS`, `HISTORY_DROPPED`
- **Example:** `{}`

### `designos_get_screenshot`
- **Purpose:** Render a node, a set of nodes on the same page, or a...
- **Args:** required: none (optional: `format`, `max_long_edge`, `node_id`, `node_ids`, `quality`, `region`, `scale`)
- **Errors:** `INVALID_ARGUMENTS`, `NODE_NOT_FOUND`, `VALUE_OUT_OF_RANGE`
- **Example:** `{"node_id":2,"scale":2.0}`

### `designos_set_in_progress`
- **Purpose:** Flag or clear root frames as in progress while an agent works...
- **Args:** required: `node_ids`, `value` (optional: `force`, `idempotency_key`, `stale_after_s`)
- **Errors:** `INVALID_ARGUMENTS`, `NODE_NOT_FOUND`, `IN_PROGRESS_TARGET_NOT_ROOT`, `NODE_IN_PROGRESS_BY_OTHER_RUN`
- **Example:** `{"node_ids":[2],"value":true}`

### `designos_apply_batch`
- **Purpose:** Execute 1..=200 write operations (apply_spec, insert/create,...
- **Args:** required: `ops` (optional: `diff_image`, `dry_run`, `idempotency_key`, `lint`)
- **Errors:** `INVALID_ARGUMENTS`, `BATCH_OP_FAILED`, `UNKNOWN_REF`, `DUPLICATE_AS`, `BATCH_TOO_LARGE`
- **Example:** `{"ops":[{"op":"insert","as":"card","parent_id":1,"kind":"frame","name":"Card","width":360,"height":180},{"op":"insert","parent_id":"@card","kind":"text","name":"Title","text":"Q3 Overview"}]}`

### `designos_get_design_context`
- **Purpose:** Return generated code in the target language, Dev Mode hints...
- **Args:** required: `node_id` (optional: `include_screenshot`, `language`, `react_tailwind`, `response_format`)
- **Errors:** `INVALID_ARGUMENTS`, `NODE_NOT_FOUND`
- **Example:** `{"node_id":2,"language":"css"}`

### `designos_configure_session`
- **Purpose:** Configure per-session MCP options for the automatic...
- **Args:** required: none (optional: `auto_fix`, `issues`)
- **Errors:** `INVALID_ARGUMENTS`, `VALUE_OUT_OF_RANGE`
- **Example:** `{"issues":{"mode":"preset","preset":"agent","max_items":25}}`

### `designos_get_app_state`
- **Purpose:** Return the pre-fetched design context block.
- **Args:** required: none (optional: `include_selection`, `max_tokens`, `sections`, `target_node_ids`)
- **Errors:** `INVALID_ARGUMENTS`
- **Example:** `{}`

### `designos_open_file`
- **Purpose:** Import a .designos, .designos, .fig, SVG or Figma REST JSON...
- **Args:** required: `path` (optional: `discard_changes`)
- **Errors:** `INVALID_ARGUMENTS`, `UNSAVED_CHANGES`, `PATH_OUTSIDE_WORKSPACE`, `FILE_TOO_LARGE`, `DOCUMENT_INVALID`
- **Example:** `{"path":"design.designos","discard_changes":false}`

### `designos_save_file`
- **Purpose:** Save this session to a .designos file inside the workspace.
- **Args:** required: `path` (optional: `overwrite`)
- **Errors:** `INVALID_ARGUMENTS`, `INVALID_NAME`, `NAME_TOO_LONG`, `PATH_OUTSIDE_WORKSPACE`, `FILE_EXISTS`
- **Example:** `{"path":"design.designos","overwrite":true}`

### `designos_render_png`
- **Purpose:** Rasterize a frame or layer to a NEW .png file inside the...
- **Args:** required: `path`, `node_id` (optional: `scale`)
- **Errors:** `INVALID_ARGUMENTS`, `INVALID_NAME`, `NAME_TOO_LONG`, `NODE_NOT_FOUND`, `VALUE_OUT_OF_RANGE`, `PATH_OUTSIDE_WORKSPACE`, `FILE_EXISTS`
- **Example:** `{"node_id":2,"path":"preview.png","scale":2.0}`

### `designos_new_document`
- **Purpose:** Start a blank document with one page.
- **Args:** required: `name` (optional: `discard_changes`)
- **Errors:** `INVALID_ARGUMENTS`, `UNSAVED_CHANGES`, `TEXT_TOO_LONG`
- **Example:** `{"name":"Fintech Workspace","discard_changes":true}`

## Declarative `NodeSpec` Reference (Generated from Schema)

Used by `designos_apply_spec`, `designos_export_spec`, and `$DESIGNOS_LIVE_SPEC`.

### `NodeSpec` Fields

Declarative node specification for `apply_spec` and `export_spec`.

| Field | Type | Description |
| --- | --- | --- |
| `annotation` | `string` | Single handoff annotation string. |
| `annotations` | `[string]` | Handoff annotations attached to this node. |
| `children` | `[#]` | Child `NodeSpec` items. |
| `clips_content` | `boolean` | Clip overflowing children when `true`. |
| `col_span` | `integer` | Grid column span (`&gt;= 1`). |
| `comments` | `[ResolvedCommentThread]` | Resolved canvas comment threads on export. |
| `component` | `string` | Component ref for `"instance"` (`"node:&lt;id&gt;"`, key, or `"&lt;Library&gt;/&lt;Component&gt;"`). |
| `corner_radii` | `array` | Per-corner radii `[tl, tr, br, bl]` in px (or `"var:..."`). |
| `corner_radius` | `NumberOrVar` | Uniform corner radius in px (or `"var:..."`). |
| `dev_status` | `string` | `"ready"` (`ReadyForDev`) or `"completed"` (`Completed`). |
| `effect_style` | `string` | Shared effect style (`"style:&lt;name&gt;"` or style name). |
| `fill` | `string` | Fill (`"#18181B"`, `"var:&lt;collection&gt;/&lt;name&gt;"`, `"style:&lt;name&gt;"`, or `"$easeui.bg.panel"`). |
| `font_size` | `NumberOrVar` | Font size in px (or `"var:..."`). |
| `font_weight` | `integer` | Font weight (`100..=900`, e.g. `400`, `500`, `600`, `700`). |
| `grid_style` | `string` | Layout grid style name or `"style:&lt;key&gt;"` reference (frame-like nodes). |
| `height` | `NumberOrVar` | Height in px (or `"var:..."` Number variable reference). |
| `height_sizing` | `string` | `"fixed"`, `"hug"`, or `"fill"`. |
| `id` | `integer` | Node ID (assigned on export; optional on input). |
| `in_progress` | `InProgressSpec` | Agent `in_progress` flag: `true`, `false`, or `{run_id, agent, status, stale_after_s}`. |
| `kind` | `string` | `"frame"`, `"rectangle"`, `"ellipse"`, `"text"`, `"section"`, `"component"`, or `"instance"`. |
| `layout` | `LayoutSpec` | Auto-Layout 5.0 configuration. |
| `line_height` | `NumberOrVar` | Line height in px (or `"var:..."`). |
| `max_height` | `number` | Max height in px. |
| `max_width` | `number` | Max width in px. |
| `min_height` | `number` | Min height in px. |
| `min_width` | `number` | Min width in px. |
| `mode` | `object` | Variable mode overrides `{collection: mode}` on this container. |
| `name` | `string` | Layer name (avoid default names like `"Frame 1"`). |
| `opacity` | `NumberOrVar` | Opacity (`0.0..=1.0`, default `1.0`, or `"var:..."`). |
| `overrides` | `[OverrideSpec]` | Descendant overrides inside an `"instance"`. |
| `preset` | `string` | EaseUI preset (`"easeui-screen"`, `"easeui-panel"`, `"easeui-card"`, `"easeui-button-primary"`, `"easeui-button-secondary"`, `"easeui-input"`, `"easeui-badge"`, `"easeui-row"`, `"easeui-column"`). |
| `props` | `object` | Component property overrides `{prop: value}` on an `"instance"`. |
| `row_span` | `integer` | Grid row span (`&gt;= 1`). |
| `slots` | `object` | Slot fills `{slot: [NodeSpec...] \| false}` on an `"instance"`. |
| `stroke` | `string` | Stroke (`"#RRGGBB"`, `"var:&lt;collection&gt;/&lt;name&gt;"`, `"style:&lt;name&gt;"`, or EaseUI token). |
| `stroke_weight` | `NumberOrVar` | Stroke weight in px (or `"var:..."`). |
| `text` | `string` | Text characters (for `"text"`, or auto-creates a child label on a button/badge frame). A `"var:..."` string binds `characters` to a String variable. |
| `text_color` | `string` | Text colour (`"#RRGGBB"`, `"var:&lt;collection&gt;/&lt;name&gt;"`, or `"$easeui.text.primary"`). |
| `text_style` | `string` | Shared text style (`"style:&lt;name&gt;"` or style name). |
| `variant` | `object` | Variant selection `{property: value}` for `"instance"`. |
| `visible` | `BoolOrVar` | `false` hides the node; a `"var:..."` string binds `visible` to a Boolean variable. |
| `width` | `NumberOrVar` | Width in px (or `"var:..."` Number variable reference). |
| `width_sizing` | `string` | `"fixed"`, `"hug"`, or `"fill"`. |
| `x` | `number` | Parent-relative X in px. |
| `y` | `number` | Parent-relative Y in px. |

### `LayoutSpec` Fields

Auto-Layout 5.0 configuration for a container frame.

| Field | Type | Description |
| --- | --- | --- |
| `counter_align` | `string` | `"start"`, `"center"`, `"end"`, or `"baseline"` (alias: `"align"`). |
| `counter_gap` | `NumberOrVar` | Cross-axis spacing between wrapped lines in px (or `"var:..."`). |
| `direction` | `string` | `"row"` / `"horizontal"` or `"column"` / `"vertical"`. |
| `gap` | `NumberOrVar` | Primary-axis child gap in px (or `"var:&lt;collection&gt;/&lt;name&gt;"`). |
| `grid_columns` | `integer` | Grid column count when using grid layout. |
| `grid_rows` | `integer` | Grid row count when using grid layout. |
| `padding` | `[NumberOrVar]` | `[top, right, bottom, left]` padding in px or `"var:..."` references. |
| `primary_align` | `string` | `"start"`, `"center"`, `"end"`, `"space-between"`, `"space-around"`, `"space-evenly"`, or `"baseline"` (alias: `"justify"`). |
| `wrap` | `boolean` | Wrap overflowing children onto additional lines when `true`. |

### `OverrideSpec` Fields

Descendant override inside an `"instance"` node.

| Field | Type | Description |
| --- | --- | --- |
| `fill` | `string` | Override fill (`"#RRGGBB"`, `"var:&lt;collection&gt;/&lt;name&gt;"`, or `"style:&lt;name&gt;"`). |
| `id` | `integer` | Numeric descendant node ID (alternative to `path`). |
| `opacity` | `number` | Override descendant opacity (`0.0..=1.0`). |
| `path` | `string` | Descendant path inside the instance (`"Header/Title"` or `"Card[1]/Label"`). |
| `props` | `object` | Component properties `{prop: value}` on a nested instance. |
| `slots` | `object` | Slot fills `{slot: [NodeSpec...] \| false}` on a nested instance. |
| `stroke` | `string` | Override stroke (`"#RRGGBB"`, `"var:&lt;collection&gt;/&lt;name&gt;"`, or `"style:&lt;name&gt;"`). |
| `swap` | `string` | Swap nested instance component (`"&lt;Component&gt;"` or `"&lt;Library&gt;/&lt;Component&gt;"`). |
| `text` | `string` | Override text on a descendant `"text"` layer. |
| `variant` | `object` | Variant properties `{property: value}` on a nested instance. |
| `visible` | `boolean` | Toggle descendant visibility (`true` \| `false`). |

### `SlotFillSpec`

Slot fill or disable value in `slots` (`[NodeSpec...] | false`).

| Variant | Description |
| --- | --- |
| `FalseLiteral` | Disable and hide the slot (`false`). |
| `[NodeSpec]` | Populate the slot with child `NodeSpec` items (`[NodeSpec...]`). |

### `NumberOrVar`

A numeric spec field that accepts either a literal `f32` or a `"var:..."` string reference.

| Variant | Description |
| --- | --- |
| `number` | Numeric value in px (or opacity `0.0..=1.0`). |
| `string` | Variable reference `"var:&lt;collection&gt;/&lt;name&gt;"` or `"var:&lt;Library&gt;/&lt;collection&gt;/&lt;name&gt;"`. |

## Per-Call `issues` Block & Error Recovery

- **Per-call `issues` block (E15-12):** Mutating responses attach `issues: {total, new_in_this_call, truncated, items, issues_skipped_rules}` with `{rule_id, severity, node_id, node_path, message, fix_hint, auto_fixable}`. Resolve `new_in_this_call > 0` before continuing.
- **Session lint tuning:** Use `designos_configure_session` (`issues: {mode, preset, max_items}`) or per-call `lint: false` when bulk-loading scaffolding.
- **Structured errors (E15-5):** Error responses include `code`, `message`, optional `path`, `valid_options`, `corrected_example`, `safe_to_retry`, and `next_step`.
- **Retry contract:** Retry a `safe_to_retry` error once, after the repair in step 7. For `TOOL_DISABLED`, follow `next_step` (ask the user to enable the tool).

## Design Quality & Anti-Generic Rules

Enforce these qualitative standards alongside automated linting:

- **Visual hierarchy**: Establish one primary action per view and one focal point per screen, with >= 3:1 size ratio between display headlines and body copy.
- **Layout rhythm & cards**: Do not wrap every group in a card or nest cards. Avoid a uniform grid of identical cards when a domain layout fits. Use the file's type scale and spacing scale (`var:` / `style:` tokens), <= 3 gap values per screen, and padding increasing with hierarchy (`page >= section >= card >= control`).
- **Buttons & centred controls**: Build buttons, pills, and badges with `"preset": "easeui-button-primary"` / `"easeui-button-secondary"` / `"easeui-badge"` or `"layout": {"direction": "row", "justify": "center", "align": "center"}` (a plain `"frame"` without `preset` or `layout` places labels at top-left).
- **Surface & colour discipline**: Keep gradients, shadows, and corner radii restrained. Use one primary accent per screen, reserve red/amber/green for status, and express dark-mode depth via surface luminance and borders.
- **Icons & illustrations**: Never hand-draw logos, illustrations, or icons with vector paths — use library instances (`kind: "instance"`) or labelled placeholders.
- **Typography & content realism**: Use <= 2 font families, <= 4 font sizes, 45–75 char lines, and `tabular-nums` for numbers. Use real domain copy, never lorem ipsum.
