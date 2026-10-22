# v0.6.x Agent tools from the Figma plugin study: task specs

**Pack goal.** Bring over the parts of the DESIGN:OS Figma plugin (`jangtrinh/design-os-figma-plugin` @ `7c1ef8bb`) that make agents working in Studio safer and cheaper:
- a structure check before a PNG is written
- a cheap summary of how a file is built
- copying properties over MCP
- a component-library audit
- a feed of the designer's changes
- a Studio-native script tool

Both repositories are PolyForm Shield 1.0.0 under the same licensor. The plugin is TypeScript on the Figma Plugin API, so nothing is copied; ideas and pure logic are ported to Rust.

Scope (owner, 2026-10-22): the three small items are next-patch work after v0.6.1. The three larger ones are specced now and scheduled later. No issue has a milestone yet.

Code baseline of every spec: `origin/main@42a0d027` (`docs: make the README, privacy page and plugin descriptions true for 0.6.0 (#839)`). Line numbers move as v0.6.1 fixes merge, so each spec tells the worker to re-anchor every cited line against HEAD and report drift before writing code. Every spec follows the 13-section layout of the v0.6 and v0.7 specs. Each ends with **Decisions on the open questions**: the owner delegated these calls on 2026-10-22, so each question was settled with its recommended answer, and the reason is kept so the owner can reverse it.

The number in a file name is the issue number.

## Index

| Issue | Title | Spec | Size | When |
|---|---|---|---|---|
| #896 | Refuse to write a PNG when a structure check fails (designos_render_png) | [896-render-png-refuses-to-write-when-a-check-fails.md](896-render-png-refuses-to-write-when-a-check-fails.md) | S | next patch |
| #897 | Summarize how a file uses variables, radii, spacing and fonts (designos_get_app_state) | [897-usage-summary-section-in-get-app-state.md](897-usage-summary-section-in-get-app-state.md) | S/M | next patch |
| #898 | Copy and paste properties between layers from MCP (designos_apply_batch) | [898-copy-properties-op-in-apply-batch.md](898-copy-properties-op-in-apply-batch.md) | M (issue: S) | next patch |
| #899 | Audit the component library for unused, duplicate and dead components (lint + Check designs) | [899-component-library-audit-rules-and-check-designs-group.md](899-component-library-audit-rules-and-check-designs-group.md) | M | later |
| #900 | Tell agents which layers the designer changed since their last call (MCP) | [900-designer-change-feed-for-agents.md](900-designer-change-feed-for-agents.md) | M/L | later |
| #901 | Run an agent script against the open document in one undo step (designos_exec_js) | [901-exec-js-script-tool-in-designos-mcp.md](901-exec-js-script-tool-in-designos-mcp.md) | L | later |

Sizes: S is under about a day, M is one to three days, and L is more than three days, for one worker with the spec's fixtures and gates. Where a spec's size differs from its issue's, the spec gives the reason.

## Owner rulings for this pack (2026-10-22)

- **Library audit (#899):** "Cái gì hữu ích thì plan đi spec driven rồi tạo task" (whatever is useful: plan it spec-driven, then create tasks). Ruling: lint rules through `designos_lint` and a new Library group in Check designs.
- **Designer-change feed (#900):** on by default.
- **Scripting (#901):** "nên đưa vào phần mcp cho AI agent connect vào design os studio. Figma không còn trong bức tranh này nữa rồi" (it belongs in the MCP that AI agents use to connect to Studio; Figma is out of the picture). The script API is Studio's own, not the Figma Plugin API. The Figma-compatible plugin host stays a separate track (#254, milestone v0.12).
- **Approval for scripts:** follow the session mode. Ask mode shows the script for approval, Skip mode runs it, and a read-only script never asks.

## Dependencies

- #896, #897 and #898 are independent of each other and of the larger items, with one shared limit: the generated rules pack (`crates/designos-mcp/generated/rules.md`) is at 23,968 of its 24,000-character cap (`rules.rs:21-22`, test `rules_under_token_budget`). Decision (controller, 2026-10-22): keep the cap. Whichever of these PRs pushes the pack over the cap trims prose in the pack in the same PR.
- #898 is M rather than S because the editor's Paste properties does not carry variable bindings or style links today (`designos-app/src/ops.rs:821-845`). Acceptance criteria 1 and 2 together force one shared module, and that visibly changes the editor's paste.
- #897 on a file over 25,000 nodes: an attached MCP holds only a scoped copy (`designos-app/src/terminal.rs:2372`, `live_canvas_scope.rs:13-15`). The summary is therefore honest-partial, and a full-file summary computed by Studio is a follow-up.
- #901 needs the licence gate (#166) before a JavaScript engine crate lands.
- #900 is independent; #901 may use #900's feed but does not require it.
- See each spec's section 12 for build order.
