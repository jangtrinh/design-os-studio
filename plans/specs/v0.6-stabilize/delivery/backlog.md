---
id: slice-backlog
type: build
title: "Backlog: lỗi panel Agents (epic #585)"
status: Written
updated: 2026-10-26
---

- Milestone GitHub: "Backlog: AI design 2 roadmap & overflow". Chưa gán release; PM gán khi xếp train.
- Branch: từ `main`; PR riêng cho mỗi story. Hai story cùng chạm terminal/panel Agents: làm US-13 trước (nhỏ hơn), rồi US-12 rebase.

## Chi tiết dev-ready (26/10)

| Issue | Story | Trạng thái code 26/10 |
|---|---|---|
| #1171 | [US-12](../us/US-12.md) | chưa có PR; Bước 0 ghi byte PTY Antigravity trước khi sửa |
| #1173 | [US-13](../us/US-13.md) | chưa có PR; Bước 0 tìm nguyên nhân wheel chết ở terminal |

Ship gate: CI xanh (Windows theo quyết định mở của Jang về #1158) + QA PASS mọi AC trên Windows 2K/4K và macOS + PM ghi G5.
