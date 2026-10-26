---
id: intake-hardening-2026-10
type: intake
title: "Intake: hardening tháng 10/2026"
status: Draft
updated: 2026-10-25
---

- Nature: `defect`. Bốn issue for:claude/for:pm cần dev-ready, không thuộc epic UI Revamp.
- Gate: G1–G5 PENDING Jang. AI không ghi `GATE: APPROVED`.
- Ràng buộc chung: thay đổi `.github/workflows` cần Jang review; ship v0.6.3+ với Windows đỏ là quyết định mở của Jang, spec không giả định là được.

| Story | Issue | Release | Tiêu đề |
|---|---|---|---|
| [US-01](../us/US-01.md) | #1165 | v0.6.7 | Style picker chỉ liệt kê style Figma có, đúng tên Figma |
| [US-02](../us/US-02.md) | #1158 | v0.7.1 | Test workspace xanh trên Windows: hết STATUS_STACK_OVERFLOW, test chỉ-Unix gắn cfg |
| [US-03](../us/US-03.md) | #1159 | v0.7.1 | Validate vendored skills xanh trên Windows (rules-lock-drift) |
| [US-04](../us/US-04.md) | #1072 | v0.6.3 | Hook agent không ghi xuyên link trong session folder (phần còn lại qua PR #1085) |
