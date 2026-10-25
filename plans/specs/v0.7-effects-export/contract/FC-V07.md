---
id: fc-v07
type: contract
title: "FC-V07 v0.7 Effects & Export"
status: Draft
updated: 2026-10-25
primary: true
---

- Version 0.1 (draft G2) · owner PM (jangtrinh) · inherits `plans/specs/designos-studio-v2/contract/FC-001.md`

## 1. Problem và outcome
Agent còn ghi được hook, session config và bỏ qua Ask gate qua MCP/CLI; export bounds lệch Figma 1–84 px. Outcome cuối 26/10: năm patch v0.7.0–v0.7.4 ship, không còn đường ghi qua link hoặc config agent ghi được, export bounds khớp Figma trên 10 kit frame.

## 2. Scope

| ID | Trong scope | Patch |
|---|---|---|
| SC-1 | Core: agent hook safety, Codex approvals, export bounds | v0.7.0 |
| SC-2 | Headless and MCP write safety | v0.7.1 |
| SC-3 | Ask gate for MCP/CLI, MCP workspace | v0.7.2 |
| SC-4 | Session config for agy/Gemini/Grok | v0.7.3 |
| SC-5 | Effects: shadow blend modes, effect caps | v0.7.4 |

Ngoài scope: phần Effects & Export khác (Backlog, from:v0.7), plugin runtime, codegen Vue/Flutter/Web Components.

## 3. Selection
PRD và stories viết ở stage prd/us tối 25/10, migrate AC từ `docs/specs/v0.7/161-*`, `162-*`, `528-*`. Mỗi patch một story; mỗi issue ít nhất một AC.

## 4. Acceptance (cấp contract)

| Patch | Done khi |
|---|---|
| v0.7.0 | Hook không ghi theo link; Codex tool call duyệt/từ chối được trong panel; export bounds của 10 kit frame khớp Figma ±0 px; shadow trên device mockup khớp Figma (#520). |
| v0.7.1 | Mọi writer headless từ chối symlink và kiểm đuôi file; agent không gỡ được tool restriction; status hook không theo link; MCP không xoá hết con của instance. |
| v0.7.2 | Ghi qua MCP/CLI đi qua Ask gate; document không có path không cấp cả ổ đĩa. |
| v0.7.3 | agy/Gemini/Grok không chạy MCP từ config agent ghi được; Gemini Ask mode không đọc tools.allowed từ session. |
| v0.7.4 | Drop/inner shadow có blend mode và round-trip .fig; nút + giới hạn số effect mà không cắt effect import. |

## 5. Gates và handoff
G1 intake này, G2 contract này: PM khoá tối 25/10 (D2 của spec v0.6). G3 sau khi viết us. Ship theo `pm/RELEASE-RUNBOOK.md`.
