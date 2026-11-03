# Phase 05 — Mermaid diagrams

UI: [part 2](ui-spec-part-2-editor-dialogs-and-states.md) §7. Mockup: ui-mockups/png/m09 (diagram cards).

Decision: P8. Size M · 1 d spike + 3 d. Depends on 02.

## Spike first (1 d)
FACT: no Mermaid renderer exists in the workspace (no Mermaid crate in any `Cargo.toml` on origin/main 8f267b3). Compare: (a) native Rust subset — flowchart + sequence — with a layered-graph layout crate; (b) an existing Rust Mermaid crate if one renders flowchart/sequence to SVG offline; (c) offline JS engine embedding. Constraint: offline, no network, renders off the UI thread. Report choice + capture before building.

## Requirements
1. Fenced ```` ```mermaid ```` blocks render as a diagram card (embed card style), theme-matched (neutral, no purple).
2. v1 diagram types: flowchart and sequence. Other types show the source with "Diagram type not supported yet" — never a blank or a fake picture.
3. Syntax errors show the error line under the source, diagram area keeps the last good render.
4. `accTitle` / `accDescr` become the card title / accessible label.

## Files
`crates/designos-docs/src/mermaid/` or a separate crate per spike result; `project_tab/diagram.rs`.

## Acceptance
1. Golden PNG tests for 3 flowcharts and 2 sequence diagrams.
2. Error case test; unsupported-type case test.
3. Capture at 1440 of design-studio's "Feedback review flow" source rendered in Studio next to `ds-project-context-doc.jpg`.

## Non-goals
Editing diagrams graphically.

## Spike result and owner decisions (2026-11-03)
Spike report: `plans/reports/spike-1527-mermaid.md` (branch `spike/1527-mermaid`). Owner chose, via AskUserQuestion:
1. **Engine: `merman` 0.9.3** (pinned `=`), features `svg`, `diagram-flowchart`, `diagram-sequence` only (no ELK/EPL), strict parse, theme from `theme.rs`, raster with resvg on a worker thread. It errors with a line number on 4/4 bad sources; mmdr drew wrong pictures for 3/4.
2. **Toolchain bump to Rust 1.95** in its own PR before this phase (merman needs edition 2024 / rustc 1.95; repo pins 1.93.1).
3. **Accept** about +8 MB and 3 MPL-2.0 crates (cssparser family); add them to the third-party licence register.
4. **Tall diagrams:** card capped at 420 tall, diagram scaled to fit, plus an "Open full size" button that opens a pan-and-zoom view. Size and pixel caps are mandatory (a 1000-node chain peaked at 722 MB).
Controller decisions: v1 allow-list is `flowchart`/`graph` and `sequenceDiagram`; error text is "Line N: <engine message>"; the "Feedback review flow" acceptance capture uses the reconstructed source, marked as reconstructed. Estimate ≈ 3.75 d.
