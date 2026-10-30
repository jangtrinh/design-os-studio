---
id: fc-v07
type: contract
title: "FC-V07 v0.7 Effects & Export"
status: Draft
updated: 2026-10-25
primary: true
---
- Version 0.1 (draft G2) · owner PM (jangtrinh) · inherits `plans/specs/designos-studio-v2/contract/FC-001.md`

## 1. Problem and outcome
Agents can still write hooks and session config, and can skip the Ask gate through MCP/CLI; export bounds are off from Figma by 1–84 px. Outcome by the end of 26/10: five patches v0.7.0–v0.7.4 ship, no write path remains through a link or agent-writable config, and export bounds match Figma on the 10 kit frames.

## 2. Scope

| ID | In scope | Patch |
|---|---|---|
| SC-1 | Core: agent hook safety, Codex approvals, export bounds | v0.7.0 |
| SC-2 | Headless and MCP write safety | v0.7.1 |
| SC-3 | Ask gate for MCP/CLI, MCP workspace | v0.7.2 |
| SC-4 | Session config for agy/Gemini/Grok | v0.7.3 |
| SC-5 | Effects: shadow blend modes, effect caps | v0.7.4 |

Out of scope: the rest of Effects & Export (Backlog, from:v0.7), plugin runtime, Vue/Flutter/Web Components codegen.

## 3. Selection
PRD and stories are written at the prd/us stage on the evening of 25/10, migrating ACs from `docs/specs/v0.7/161-*`, `162-*`, `528-*`. One story per patch; at least one AC per issue.

## 4. Acceptance (contract level)

| Patch | Done when |
|---|---|
| v0.7.0 | Hooks don't write through a link; Codex tool calls can be approved/denied in the panel; export bounds of the 10 kit frames match Figma ±0 px; shadows on device mockups match Figma (#520). |
| v0.7.1 | Every headless writer refuses symlinks and checks the file extension; an agent can't lift the tool restriction; status hooks don't follow links; MCP doesn't delete all of an instance's children. |
| v0.7.2 | MCP/CLI writes go through the Ask gate; a document without a path doesn't grant the whole disk. |
| v0.7.3 | agy/Gemini/Grok don't start MCP from agent-writable config; Gemini Ask mode doesn't read tools.allowed from the session. |
| v0.7.4 | Drop/inner shadows have blend modes and round-trip through .fig; the + button caps the effect count without cutting imported effects. |

## 5. Gates and handoff
G1 is this intake, G2 is this contract: PM locks them on the evening of 25/10 (D2 of spec v0.6). G3 after the us stage. Ship per `pm/RELEASE-RUNBOOK.md`.
