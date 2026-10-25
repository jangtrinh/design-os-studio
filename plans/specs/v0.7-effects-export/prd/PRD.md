---
id: prd-v07
type: prd
title: "PRD v0.7 Effects & Export"
status: Written
updated: 2026-10-25
primary: true
---

## 1. Problem
Xem [contract](../contract/FC-V07.md).

## 2. Yêu cầu

| ID | Yêu cầu | Nguồn | US |
|---|---|---|---|
| REQ-01 | Shadow trên device mockup khớp Figma: sửa xong theo AC. | #520 | US-01 |
| REQ-02 | Export bounds khớp Figma trên 10 kit frame: sửa xong theo AC. | #528 | US-02 |
| REQ-03 | Duyệt / từ chối tool call của Codex trong Agents panel: sửa xong theo AC. | #930 | US-03 |
| REQ-04 | MCP không xoá con của instance: sửa xong theo AC. | #985 | US-04 |
| REQ-05 | Writer headless từ chối symlink và kiểm đuôi file: sửa xong theo AC. | #1009 | US-05 |
| REQ-06 | Agent không gỡ được tool restriction của người dùng: sửa xong theo AC. | #1049 | US-06 |
| REQ-07 | Status hook không ghi theo symlink trong session: sửa xong theo AC. | #1050 | US-07 |
| REQ-08 | Document chưa lưu không cấp cả ổ đĩa cho designos-mcp: sửa xong theo AC. | #1074 | US-08 |
| REQ-09 | Ghi qua MCP và CLI đi qua Ask gate: sửa xong theo AC. | #1101 | US-09 |
| REQ-10 | CI dùng cargo-nextest với retry và quarantine: sửa xong theo AC. | #1160 | US-10 |
| REQ-11 | Job check nhanh ~2 phút trước test matrix: sửa xong theo AC. | #1161 | US-11 |
| REQ-12 | Chất lượng contributor: CONTRIBUTING, PR template, gate fork-safe: sửa xong theo AC. | #1163 | US-12 |
| REQ-13 | Gemini Ask mode không đọc tools.allowed từ session: sửa xong theo AC. | #1047 | US-13 |
| REQ-14 | agy, Gemini, Grok không chạy MCP từ config agent ghi được: sửa xong theo AC. | #1048 | US-14 |

## 3. Ràng buộc
- Mọi thay đổi `.github/workflows` (REQ-10..12) cần Jang review khi merge.
- Ship v0.6.3+ khi Windows đỏ là quyết định mở của Jang; spec không giả định.
- AI không ghi `GATE: APPROVED`.
