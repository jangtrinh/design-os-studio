---
id: FC-UIR
type: contract
title: "Contract UI Revamp, slice 1 và 2"
updated: "2026-10-25"
status: draft
primary: true
---

## Metadata
| Mục | Giá trị |
|---|---|
| Version | 0.2 (thêm slice 2, chờ Jang duyệt) |
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
| BR-05 | Surface D–K chỉ vẽ widget qua kit C; ngoại lệ tương phản phải có tên trong token-contract | #1139, R10.7 |

### Trong phạm vi
- SC-01: gate layout-unchanged và baseline hình học (L); nguồn: #1126.
- SC-02: onboard màu Untitled dark, token store, `theme.rs`, soul, principles, design agents (A); nguồn: #1142.
- SC-03: scale type, spacing, radius, control size (B); nguồn: #1143.

- SC-04: widget kit và các surface C–K (#1119, #1120, #1124, #1128, #1138, #1139, #1144, #1146, #1155), slice 2; nguồn: #1118 Phases.
- SC-05: công cụ dogfood (#1121, #1127, #1140, #1141, #1154), slice 2, chạy song song; nguồn: #1118.
- SC-06: docs/screenshot (#1125), sau khi D–K ship; nguồn: #1118.

### Hoãn
- SC-08: licence icon Untitled (#1122); lý do: câu hỏi pháp lý chờ owner (khuyến nghị Option A, giữ Phosphor/Lucide); xem lại khi: Jang trả lời; nguồn: detail/1122-research-untitled-icons-licence.md.

### Ngoài phạm vi
- SC-07: đổi layout, màu canvas/overlay, ANSI terminal, nội dung tài liệu; lý do: owner loại; nguồn: #1118.

## Nghiệm thu
| ID | Given | When | Then | Source |
|---|---|---|---|---|
| AC-01 | Baseline hình học đã commit | Một PR đổi style chạy `cargo test` | Gate pass khi vùng/độ rộng/thứ tự không đổi, fail khi đổi | #1126 |
| AC-02 | Token store và `theme.rs` đã đổi giá trị | Chạy G01 parity, Purple Ban, AA, soul, agents check | Tất cả xanh | #1142 |
| AC-03 | Constant scale mới trong `theme.rs` | Chạy theme tests và ratchet | Mọi constant nằm trên scale Untitled; tests xanh | #1143 |
| AC-04 | Một PR surface C–K | Chạy census chrome literal, gate L, gate C-1, gate spacing | 0 site ngoài allowlist; layout không đổi; mọi cặp đạt min | #1119…#1155 |
| AC-05 | Một PR công cụ dogfood | Chạy test đỏ-trước của spec và lệnh dogfood | Test xanh; terminal capture khớp kỳ vọng | #1121, #1127, #1140, #1141, #1154 |
| AC-06 | Release sau D–K | Chạy check docs/ảnh | Không còn mô tả theme cũ; ảnh khớp app; site ≥ 4.5:1 | #1125 |
PRD-UIR, US-01..US-18 mô tả chi tiết.

## Flow, UIUX và dependency
Thứ tự: L (#1126) trước mọi PR revamp; A (#1142) rồi B (#1143) (cùng file, một writer). DS: Untitled UI PRO v8.0 dark. Gate chung: CI xanh (Rustfmt, Clippy, Test ubuntu/macos/windows, Codegen, Design gates).

## Scenario và bằng chứng
Mỗi PR có bằng chứng hình (screenshot hoặc terminal capture). G2 không cần built evidence.

## Bàn giao và giới hạn demo
Receiver: Claude (for:claude). Milestones v0.6.4 (#1166, #1126, due 27/10), v0.6.5 (#1142, due 28/10), v0.6.6 (#1143, due 29/10). PR vào nhánh release theo runbook; PM verify, Jang merge/tag.
