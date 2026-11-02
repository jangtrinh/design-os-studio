# Phase 05 — Mermaid diagrams

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
