---
id: intake-ui-revamp
type: intake
title: "UI Revamp: Untitled UI dark"
updated: "2026-10-25"
status: draft
---

## Nguồn và phân loại
| Mục | Giá trị |
|---|---|
| Source | Epic #1118 (owner rulings R1–R7), sub-issues #1119–#1155 |
| Sender | Jang (owner) |
| Time | 2026-10-25 14:14 (UTC+7) tạo epic; 21:25 (UTC+7) nâng P0 |
| Nature | product_change |
| Handling | spec_then_build |
| Duplicate search | Chỉ #1118 khớp "UI revamp"; spec chi tiết từng issue ở `docs/specs/ui-revamp-untitled-dark/` (nhánh `docs/specs-untitled-ui-dark`) |
| Reason | Owner yêu cầu đổi chrome sang Untitled UI PRO v8.0 dark, giữ nguyên layout |

## Vấn đề và kết quả
Chrome của Studio đang dùng palette Zinc cũ. Owner muốn 100% Untitled UI dark (màu, type, spacing, radius, control, focus) với brand cam `#E36323`, giữ nguyên vùng, thứ tự panel, độ rộng panel và vị trí control (R1, R2, R4, R5). Kết quả: mọi surface đổi style mà layout không đổi, chứng minh bằng gate hình học.

## Giới hạn
- Canvas, overlay Dev, nội dung tài liệu và bảng màu ANSI terminal không đổi (#1118).
- Không dùng Figma; kit được đọc bằng CLI/MCP của Studio (R3).
- Licence Untitled Icons chưa rõ: OPEN, theo #1122 (R6).
- Spec chi tiết A/B/L chưa merge vào `main`: OPEN, phụ thuộc PR của nhánh `docs/specs-untitled-ui-dark`.
