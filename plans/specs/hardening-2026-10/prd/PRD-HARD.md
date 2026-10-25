---
id: PRD-HARD
type: prd
title: "PRD hardening 2026-10"
updated: "2026-10-25"
status: draft
primary: true
---

## Vấn đề và kết quả
Bốn lỗi chặn chất lượng release: style lạ trong picker (#1165), CI Windows đỏ (#1158, #1159), hook ghi xuyên link (#1072). Kết quả: mỗi issue có US dev-ready, G1–G5 chờ Jang.

## Phạm vi
Trong: bốn story dưới đây. Ngoài: cho Windows optional, path filters, cache CI (Jang chưa duyệt).

## Yêu cầu và quy tắc
| ID | Requirement | Source | US |
|---|---|---|---|
| REQ-01 | Style picker chỉ liệt kê style Figma có, đúng tên Figma: đạt mọi AC trong US-01 | #1165 | US-01 |
| REQ-02 | Test workspace xanh trên Windows: hết STATUS_STACK_OVERFLOW, test chỉ-Unix gắn cfg: đạt mọi AC trong US-02 | #1158 | US-02 |
| REQ-03 | Validate vendored skills xanh trên Windows (rules-lock-drift): đạt mọi AC trong US-03 | #1159 | US-03 |
| REQ-04 | Hook agent không ghi xuyên link trong session folder (phần còn lại qua PR #1085): đạt mọi AC trong US-04 | #1072 | US-04 |
