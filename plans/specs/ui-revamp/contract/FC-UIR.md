---
id: FC-UIR
type: contract
title: "Contract UI Revamp, slice 1"
updated: "2026-10-25"
status: draft
primary: true
---

## Metadata
| Mục | Giá trị |
|---|---|
| Version | 0.1 |
| Owner | Jang (owner) |
| Product | Epic #1118 |
| Temporary authority | Owner rulings R1–R7 trong #1118; spec A (#1142), B (#1143), L (#1126) |

## Vấn đề và kết quả
Cần chuyển chrome sang Untitled UI dark mà không đổi layout. Slice 1 dựng nền móng: gate layout, token màu, scale kích thước.

## Phạm vi và nguồn chuẩn
| ID | Rule | Source |
|---|---|---|
| BR-01 | Layout không đổi: cùng vùng, cùng độ rộng panel, cùng thứ tự control; chiều cao chỉ tăng khi R5 tăng size | #1126, R1 |
| BR-02 | Màu chrome lấy từ token Untitled dark; brand `#E36323`, label `#0A0A0A`, focus `#F97316`; Purple Ban giữ | #1142, R4 |
| BR-03 | Editor panel dùng đầu nhỏ của scale (text-xs 12/18, row 32, button xs 32, radius md 8); Home/dialog dùng sm 36 / md 40 | #1143, R5 |
| BR-04 | Mỗi task có spec trước khi code | R7 |

### Trong phạm vi
- SC-01: gate layout-unchanged và baseline hình học (L); nguồn: #1126.
- SC-02: onboard màu Untitled dark, token store, `theme.rs`, soul, principles, design agents (A); nguồn: #1142.
- SC-03: scale type, spacing, radius, control size (B); nguồn: #1143.

### Hoãn
- SC-04: widget kit và các surface C–K (#1119, #1120, #1124, #1128, #1138, #1139, #1144, #1146, #1155); lý do: cần A, B, L xong trước; xem lại khi: slice 1 ship; nguồn: #1118 Phases.
- SC-05: công cụ dogfood (#1121, #1127, #1140, #1141, #1154); lý do: enabler chạy song song, không chặn slice 1; xem lại khi: Claude còn capacity; nguồn: #1118.
- SC-06: docs/screenshot (#1125) và licence icons (#1122); lý do: phase 2 / nghiên cứu; xem lại khi: surfaces xong; nguồn: #1118.

### Ngoài phạm vi
- SC-07: đổi layout, màu canvas/overlay, ANSI terminal, nội dung tài liệu; lý do: owner loại; nguồn: #1118.

## Nghiệm thu
| ID | Given | When | Then | Source |
|---|---|---|---|---|
| AC-01 | Baseline hình học đã commit | Một PR đổi style chạy `cargo test` | Gate pass khi vùng/độ rộng/thứ tự không đổi, fail khi đổi | #1126 |
| AC-02 | Token store và `theme.rs` đã đổi giá trị | Chạy G01 parity, Purple Ban, AA, soul, agents check | Tất cả xanh | #1142 |
| AC-03 | Constant scale mới trong `theme.rs` | Chạy theme tests và ratchet | Mọi constant nằm trên scale Untitled; tests xanh | #1143 |
PRD-UIR, US-01..US-03 mô tả chi tiết.

## Flow, UIUX và dependency
Thứ tự: L (#1126) trước mọi PR revamp; A (#1142) rồi B (#1143) (cùng file, một writer). DS: Untitled UI PRO v8.0 dark. Gate chung: CI xanh (Rustfmt, Clippy, Test ubuntu/macos/windows, Codegen, Design gates).

## Scenario và bằng chứng
Mỗi PR có bằng chứng hình (screenshot hoặc terminal capture). G2 không cần built evidence.

## Bàn giao và giới hạn demo
Receiver: Claude (for:claude). Milestone `v0.7.5 UI Revamp slice 1` (30/10/2026). PR vào nhánh release theo runbook; PM verify, Jang merge/tag.
