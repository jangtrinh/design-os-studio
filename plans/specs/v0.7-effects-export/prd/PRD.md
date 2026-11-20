---
id: prd-v07
type: prd
title: "PRD v0.7 Effects & Export"
status: Written
updated: 2026-10-25
primary: true
---

## 1. Problem
See [contract](../contract/FC-V07.md).

## 2. Requirements

| ID | Requirement | Source | US |
|---|---|---|---|
| REQ-01 | Shadows on device mockups match Figma, fixed per the AC. | #520 | US-01 |
| REQ-02 | Export bounds match Figma on the 10 kit frames, fixed per the AC. | #528 | US-02 |
| REQ-03 | Approve / deny Codex tool calls in the Agents panel, fixed per the AC. | #930 | US-03 |
| REQ-04 | MCP does not delete an instance's children, fixed per the AC. | #985 | US-04 |
| REQ-05 | Headless writers refuse symlinks and check the file extension, fixed per the AC. | #1009 | US-05 |
| REQ-06 | An agent cannot lift the user's tool restriction, fixed per the AC. | #1049 | US-06 |
| REQ-07 | Status hooks do not write through a symlink in the session, fixed per the AC. | #1050 | US-07 |
| REQ-08 | An unsaved document does not give designos-mcp the whole disk, fixed per the AC. | #1074 | US-08 |
| REQ-09 | MCP and CLI writes go through the Ask gate, fixed per the AC. | #1101 | US-09 |
| REQ-10 | CI uses cargo-nextest with retry and quarantine, fixed per the AC. | #1160 | US-10 |
| REQ-11 | A fast ~2-minute check job before the test matrix, fixed per the AC. | #1161 | US-11 |
| REQ-12 | Contributor quality: CONTRIBUTING, PR template, fork-safe gate, fixed per the AC. | #1163 | US-12 |
| REQ-13 | Gemini Ask mode does not read tools.allowed from the session, fixed per the AC. | #1047 | US-13 |
| REQ-14 | agy, Gemini and Grok do not start MCP from agent-writable config, fixed per the AC. | #1048 | US-14 |

## 3. Constraints
- Every `.github/workflows` change (REQ-10..12) needs Jang's review at merge.
- Shipping v0.6.3+ while Windows is red is an open decision for Jang; the spec assumes nothing.
- AI does not write `GATE: APPROVED`.
