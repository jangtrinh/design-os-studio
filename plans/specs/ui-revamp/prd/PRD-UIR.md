---
id: PRD-UIR
type: prd
title: "PRD UI Revamp, slice 1 và 2"
updated: "2026-10-25"
status: draft
primary: true
---

## Vấn đề và kết quả
Chrome của Studio chưa theo Untitled UI dark. Slice 1 cho surface C–K một nền chung: gate layout, token màu, scale kích thước.

## Phạm vi
Trong: SC-01..SC-06 của FC-UIR (slice 1: SC-01..03; slice 2: SC-04..06). Ngoài: SC-07, SC-08.

## Yêu cầu và quy tắc
| ID | Requirement | Source | US |
|---|---|---|---|
| REQ-01 | Mọi PR revamp chứng minh layout không đổi bằng check hình học tất định | FC-UIR.AC-01 | US-01 |
| REQ-02 | Màu chrome = token Untitled dark, map 1-1 sang constant `theme.rs` | FC-UIR.AC-02 | US-02 |
| REQ-03 | Type/spacing/radius/control dùng constant theo bước Untitled, có invariant test | FC-UIR.AC-03 | US-03 |
| REQ-04 | App shell: tab, rail, toolbar, ruler, menu: đạt mọi AC trong US-04 (spec chi tiết #1119) | FC-UIR.AC-04 | US-04 |
| REQ-05 | Panel trái: Pages và Layers: đạt mọi AC trong US-05 (spec chi tiết #1120) | FC-UIR.AC-04 | US-05 |
| REQ-06 | Panel phải: Inspector: đạt mọi AC trong US-06 (spec chi tiết #1124) | FC-UIR.AC-04 | US-06 |
| REQ-07 | Dev mode và spec reader: đạt mọi AC trong US-07 (spec chi tiết #1128) | FC-UIR.AC-04 | US-07 |
| REQ-08 | Home workspace: đạt mọi AC trong US-08 (spec chi tiết #1138) | FC-UIR.AC-04 | US-08 |
| REQ-09 | Widget kit theo anatomy Untitled: đạt mọi AC trong US-09 (spec chi tiết #1139) | FC-UIR.AC-04 | US-09 |
| REQ-10 | Agents panel, terminal, agent card: đạt mọi AC trong US-10 (spec chi tiết #1144) | FC-UIR.AC-04 | US-10 |
| REQ-11 | Popover, menu, modal, toast, Cmd+K, Find, comment: đạt mọi AC trong US-11 (spec chi tiết #1146) | FC-UIR.AC-04 | US-11 |
| REQ-12 | Assets, Variables, Libraries, Styles: đạt mọi AC trong US-12 (spec chi tiết #1155) | FC-UIR.AC-04 | US-12 |
| REQ-13 | MCP mở được file kit lớn: đạt mọi AC trong US-13 (spec chi tiết #1121) | FC-UIR.AC-05 | US-13 |
| REQ-14 | Refresh docs và screenshot: đạt mọi AC trong US-14 (spec chi tiết #1125) | FC-UIR.AC-06 | US-14 |
| REQ-15 | CLI chọn node theo id và Figma id: đạt mọi AC trong US-15 (spec chi tiết #1127) | FC-UIR.AC-05 | US-15 |
| REQ-16 | Export DTCG một file cho onboarding: đạt mọi AC trong US-16 (spec chi tiết #1140) | FC-UIR.AC-05 | US-16 |
| REQ-17 | Screenshot CLI không flake: đạt mọi AC trong US-17 (spec chi tiết #1141) | FC-UIR.AC-05 | US-17 |
| REQ-18 | Text NodeSpec đo thật, không ước lượng: đạt mọi AC trong US-18 (spec chi tiết #1154) | FC-UIR.AC-05 | US-18 |

## Flow và dependency
US-01 (L) trước; US-02 (A) rồi US-03 (B). Slice 2: C (US-09) trước D–K (US-04..08, 10..12); công cụ US-13, 15–18 chạy song song; US-14 sau khi D–K ship. Spec chi tiết đã copy vào `detail/` (pack tự đủ).

## Giả định và bàn giao
Spec chi tiết nằm trong `detail/` của pack này (copy từ nhánh `docs/specs-untitled-ui-dark`, 25/10). Re-anchor mọi dòng trích dẫn với HEAD trước khi code. Ước lượng: L 2–3 ngày, A ~2 ngày, B ~1.5 ngày (theo issue).
