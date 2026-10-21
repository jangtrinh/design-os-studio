# DESIGN:OS Studio MCP (`designos-mcp`)

A local Model Context Protocol (MCP) server for inspecting and editing design files with DESIGN:OS Studio's document model and code generators. It builds two binary aliases (`designos-mcp` and `designos-mcp`) and runs an isolated, in-memory file session over stdio.

## Build and connect

From the repository root:

```sh
cargo build --release -p designos-mcp --bin designos-mcp
```

The executable is `target/release/designos-mcp` (and `target/release/designos-mcp`).

Configure an MCP client that supports local stdio servers:

Windows:

```json
{
  "mcpServers": {
    "design-os-studio": {
      "command": "C:\\path\\to\\design-os-studio\\target\\release\\designos-mcp.exe",
      "args": ["--workspace", "C:\\path\\to\\designs"]
    }
  }
}
```

macOS/Linux:

```json
{
  "mcpServers": {
    "design-os-studio": {
      "command": "/path/to/design-os-studio/target/release/designos-mcp",
      "args": ["--workspace", "/path/to/designs"]
    }
  }
}
```

The client launches the process and speaks MCP over stdin/stdout. The server does not listen on a network port. No account, API key, Figma connection, or running editor is needed. Configuration is not installed automatically.

## File & Agent Spec Workflow

1. Call `designos_open_file` with a path inside the configured workspace, or `designos_new_document` with a name.
2. Use `designos_get_document` to discover page IDs, then `designos_list_nodes`, `designos_get_node`, or `designos_export_spec` to inspect layers and declarative subtree specs. IDs are positive integers; obtain them from tool results rather than Figma node IDs.
3. Build complete EaseUI screens or component trees in one atomic undoable step with `designos_apply_spec` (supports BrainWave 2.0 presets like `easeui-screen`, `easeui-card`, `easeui-button-primary`, `$easeui.*` design tokens, and Auto-Layout 5.0 `row`/`column`/`wrap`/`hug`/`fill` reflow), or make granular edits with `designos_create_node`, `designos_update_node`, and `designos_delete_node`.
4. Run the human-agent canvas review loop with `designos_list_comments`, `designos_add_comment`, `designos_reply_comment`, `designos_resolve_comment`, and `designos_set_dev_status`.
5. Call `designos_save_file` with a `.designos` filename (pass `"overwrite": true` to update an existing `.designos` file in place) and `designos_render_png` to rasterize visual verification previews.

Example tool arguments:

```text
designos_open_file       {"path":"input.fig"}
designos_get_document    {}
designos_apply_spec      {"parent_id":1,"spec":{"name":"IAM · Login Screen","preset":"easeui-screen","dev_status":"ready","children":[{"name":"Login Card","preset":"easeui-card","width":400,"height_sizing":"hug","children":[{"name":"Submit Button","preset":"easeui-button-primary","width_sizing":"fill","text":"Continue"}]}]}}
designos_list_comments   {"resolved":false}
designos_add_comment     {"node_id":42,"author":"Jang","text":"Make this CTA full width","x":12,"y":8}
designos_reply_comment   {"thread_id":"t1","author":"Agent","text":"Updated to width_sizing=fill"}
designos_resolve_comment {"thread_id":"t1","resolved":true}
designos_set_dev_status  {"node_id":42,"status":"completed","annotation":"Shipped in EaseUI"}
designos_export_spec     {"node_id":42}
designos_generate_code   {"node_id":42,"language":"react"}
designos_render_png      {"path":"preview.png","node_id":42,"scale":2.0}
designos_save_file       {"path":"output-v2.designos","overwrite":true}
```

`42` is illustrative: substitute an ID returned by the preceding queries. Relative paths resolve beneath `--workspace`. Absolute paths are accepted only within that same directory tree. Reads canonicalize paths and reject escapes; writes resolve the existing parent and refuse to overwrite an existing file unless `overwrite: true` is explicitly passed to `designos_save_file`.

Edits remain in memory until saved. Restarting the server loses unsaved work. Opening another file or starting a new document resets undo history and refuses to discard pending edits unless `discard_changes: true` is explicitly supplied. Tool failures return an MCP error result with an explanatory message.

Imports are limited to 64 MiB per file, 100,000 layers, and 256 levels of nesting. The server validates the imported document graph before using it, rejecting malformed parent/child relationships and cycles.

## Tools

| Tool | Purpose |
| --- | --- |
| `designos_open_file` | Import a native `.designos`, `.fig`, SVG, or supported JSON design into the session. |
| `designos_new_document` | Start an empty document with one page. |
| `designos_save_file` | Write the session to a `.designos` file (`overwrite: true` updates an existing file in place). |
| `designos_render_png` | Rasterize a frame or layer to a new `.png` file using the deterministic renderer. |
| `designos_get_document` | Read document name, pages, node/variable/comment counts, and undo/redo availability. |
| `designos_list_nodes` | Search names case-insensitively or list direct children; paginate using `next_offset`. |
| `designos_get_node` | Read a complete native node, including child IDs, paints, geometry, text, and bindings. |
| `designos_get_variables` | Read collections and tokens; optionally resolve one node's direct bindings in its inherited modes. |
| `designos_generate_code` | Return CSS, Tailwind, React, SwiftUI, Compose, or SVG source without writing a file. |
| `designos_apply_spec` | Build a complete UI subtree from a declarative `NodeSpec` (EaseUI presets, `$easeui.*` tokens, Auto-Layout 5.0) in 1 undoable step. |
| `designos_export_spec` | Export a node and its descendants as a declarative `NodeSpec` enriched with Dev Mode annotations and pinned canvas comments. |
| `designos_create_node` | Add a frame, rectangle, ellipse, or text layer to a container and reflow Auto-Layout. |
| `designos_update_node` | Change name, position, dimensions, visibility, opacity, or plain text and reflow Auto-Layout. |
| `designos_delete_node` | Remove a non-page layer and its descendants. |
| `designos_list_comments` | List canvas comment threads with resolved world coordinates, target node, and containing screen/frame. |
| `designos_add_comment` | Pin a new comment thread to a node or page coordinate. |
| `designos_reply_comment` | Append a reply message to an existing comment thread and optionally update its resolved state. |
| `designos_resolve_comment` | Mark a comment thread resolved or unresolved. |
| `designos_set_dev_status` | Set a layer's Dev Mode status (`ready`, `completed`, `clear`) and/or append an annotation. |
| `designos_export_fig_file` | Export the current session document to a Figma binary `.fig` container inside the workspace. |
| `designos_query_xpath` | Query layers on a page using an XPath-like selector or a kind/name substring. |
| `designos_lint` | Audit layers on a page for zero-bounds geometry and default layer names. |
| `designos_undo` / `designos_redo` | Traverse the session's edit history. |

`designos_list_nodes` defaults to 100 results and accepts 1–500 per call. `parent_id` filters direct children, not all descendants. Coordinates are relative to the parent. Width and height must be non-negative, geometry must be finite and within ±1,000,000, and opacity must be between 0 and 1. Replacing text clears rich-text runs and list ranges. Page roots cannot be edited or deleted by the layer tools.

## Verification and evaluation

```sh
cargo test --locked -p designos-mcp
```

`tests/evaluation.xml` contains ten independent, read-only retrieval questions based on the repository's fixed Figma REST fixture. For each question, start a fresh server with the repository root as `--workspace`, then preload `crates/designos-fig/tests/fixtures/rest_file.json` using `designos_open_file`. This preload is test setup; answering the questions requires only the read-only query/code tools. Do not create, update, delete, undo, redo, or save during evaluation. Match the response against the XML answer exactly.
