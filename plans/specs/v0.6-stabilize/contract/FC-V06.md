---
id: fc-v06
type: contract
title: "FC-V06 v0.6 stabilize"
status: Written
updated: 2026-10-25
primary: true
---

- Version 1.0 · owner PM (jangtrinh) · inherits `plans/specs/designos-studio-v2/contract/FC-001.md`

## 1. Problem và outcome
Agent trong Studio ghi được file cấu hình, approval và link mà Studio sau đó tin. Outcome: hết ngày 25/10 không còn đường nào để agent tự cấp quyền, và bốn patch đã ship.

## 2. Scope

| ID | Trong scope | Patch |
|---|---|---|
| SC-1 | Agent sandbox safety I | v0.6.2 |
| SC-2 | Agent sandbox safety II | v0.6.3 |
| SC-3 | Agent config and editor quality | v0.6.4 |
| SC-4 | Styles, tooling and headless checks | v0.6.5 |

Ngoài scope: tính năng v0.7 (Effects & Export) và các mục đã chuyển Backlog.

## 3. Selection
PRD [REQ-01..REQ-18](../prd/PRD.md); stories US-01..US-04, mỗi story một patch.

## 4. Gates

| Gate | Điều kiện | Ai |
|---|---|---|
| G1 | Intake đủ nguồn | PM |
| G2 | Contract này được khoá | PM theo D2 |
| G3 | Mỗi AC có scenario; QA smoke chạy được | PM + QA |
| G4 | Chỉ cho slice có UI (US-03, US-04): ảnh bằng chứng | QA |
| G5 | Handoff: CI xanh, QA PASS, tag do Claude cắt | PM xác minh |

## 5. Acceptance và handoff
Một patch ship khi mọi AC của story đạt, CI trên release branch xanh và QA smoke PASS. Quy trình: `pm/RELEASE-RUNBOOK.md`.
