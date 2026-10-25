---
id: PRD-UIR
type: prd
title: "PRD UI Revamp, slice 1"
updated: "2026-10-25"
status: draft
primary: true
---

## Vấn đề và kết quả
Chrome của Studio chưa theo Untitled UI dark. Slice 1 cho surface C–K một nền chung: gate layout, token màu, scale kích thước.

## Phạm vi
Trong: SC-01..SC-03 của FC-UIR. Ngoài: SC-04..SC-07.

## Yêu cầu và quy tắc
| ID | Requirement | Source | US |
|---|---|---|---|
| REQ-01 | Mọi PR revamp chứng minh layout không đổi bằng check hình học tất định | FC-UIR.AC-01 | US-01 |
| REQ-02 | Màu chrome = token Untitled dark, map 1-1 sang constant `theme.rs` | FC-UIR.AC-02 | US-02 |
| REQ-03 | Type/spacing/radius/control dùng constant theo bước Untitled, có invariant test | FC-UIR.AC-03 | US-03 |

## Flow và dependency
US-01 (L) trước; US-02 (A) rồi US-03 (B). Spec chi tiết: `docs/specs/ui-revamp-untitled-dark/1126-…`, `1142-…`, `1143-…`.

## Giả định và bàn giao
OPEN: spec chi tiết chưa merge vào `main`. Re-anchor mọi dòng trích dẫn với HEAD trước khi code. Ước lượng: L 2–3 ngày, A ~2 ngày, B ~1.5 ngày (theo issue).
