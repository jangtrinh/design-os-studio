# Spec readiness 2026-10-25 (cập nhật 23:35 UTC+7, batch 2)

Nguồn: GitHub open issues, milestone v0.6.3–v0.7.x (v0.6.2 đang ship, không tính). Tiêu chí *ready*: Spec Driven v2 (kit v2.4.0), lint 0 lỗi, có AC + test steps + edge cases + mockup, không OPEN chặn. Gate G1–G5: AI không ghi APPROVED; tất cả PENDING Jang.

Đã làm: UI Revamp slice 1 (#1126, #1142, #1143) dev-ready trên nhánh `pm/spec-ui-revamp` (PR #1166, commit 246c297): 16 AC, 16 scenario test steps, edge cases, 3 mockup (`plans/specs/ui-revamp/evidence/us01-03.png`). Comment + nhãn `spec:ready` trên 3 issue.

Batch 2: UI Revamp slice 2 (C–K + #1121 #1125 #1127 #1140 #1141 #1154) thành US-04..18 trên PR #1166 (commit 7447bd5), pack tự đủ với `detail/` copy từ `docs/specs-untitled-ui-dark`; lint 48 doc, 89 AC, 0 lỗi. Pack mới `plans/specs/hardening-2026-10` trên `pm/spec-v0.6` (commit 7e2d7be): #1165, #1158, #1159, #1072, 21 AC, 0 lỗi. 19 issue có comment + nhãn `spec:ready`; #1122 chỉ comment (chờ owner).

## Tổng theo bản
| Release | ready | partial | missing |
|---|---|---|---|
| v0.6.3 | 1 | 0 | 0 |
| v0.6.4 | 1 | 3 | 1 |
| v0.6.5 | 1 | 1 | 2 |
| v0.6.6 | 3 | 0 | 0 |
| v0.6.7 | 5 | 0 | 0 |
| v0.6.8 | 3 | 0 | 0 |
| v0.6.9 | 3 | 0 | 0 |
| v0.6.10 | 2 | 0 | 0 |
| v0.6.11 | 1 | 1 | 0 |
| v0.7 | 0 | 3 | 0 |
| v0.7.1 | 2 | 2 | 2 |
| v0.7.2 | 0 | 5 | 0 |
| v0.7.3 | 0 | 2 | 0 |

## Chi tiết
| Release | Issue | Spec status | Gaps |
|---|---|---|---|
| v0.6.3 | #1072 Agent hook commands write through a link an agent plants in the sessio  | ready | — `hardening-2026-10` US-04 (gate G1–G5 PENDING Jang) |
| v0.6.4 | #875 Create a text style from selected text | partial | thiếu test steps, edge cases, mockup/ảnh; chưa có US v2 |
| v0.6.4 | #887 Dev Mode code panel is too narrow to read | partial | thiếu test steps, edge cases; chưa có US v2 |
| v0.6.4 | #993 run_tx records every touched node through two JSON values, which makes | missing | chỉ có mô tả; cần AC, test steps, edge cases, ảnh |
| v0.6.4 | #1069 Ask-mode Grok and Antigravity tabs start with an agent-edited config w | partial | thiếu AC, edge cases, mockup/ảnh; chưa có US v2 |
| v0.6.4 | #1126 Layout-unchanged gate: baseline geometry of every surface, and the che | ready | — (gate G1–G5 PENDING Jang) |
| v0.6.5 | #983 Editing a main component renames its instances on unloaded pages | missing | chỉ có mô tả; cần AC, test steps, edge cases, ảnh |
| v0.6.5 | #1003 Headless input checks from the QA red team (SwiftUI escape, exit codes | partial | thiếu AC, edge cases; chưa có US v2 |
| v0.6.5 | #1088 designos-diff has no --version | missing | chỉ có mô tả; cần AC, test steps, edge cases, ảnh |
| v0.6.5 | #1142 Spec A — Onboard Untitled UI dark with the design:os CLI: colour roles | ready | — (gate G1–G5 PENDING Jang) |
| v0.6.6 | #1141 CLI --screenshot sometimes exits 1: "screenshot was not captured"  | ready | — `ui-revamp` US-17 (gate G1–G5 PENDING Jang) |
| v0.6.6 | #1143 Spec B — Untitled type, spacing, radius and control-size scale in `the | ready | — (gate G1–G5 PENDING Jang) |
| v0.6.6 | #1154 NodeSpec sizes hug text with a width estimate, so button labels sit of  | ready | — `ui-revamp` US-18 (gate G1–G5 PENDING Jang) |
| v0.6.7 | #1121 designos-mcp imports large kit files: one import limit, typed size err  | ready | — `ui-revamp` US-13 (gate G1–G5 PENDING Jang) |
| v0.6.7 | #1127 The CLI addresses a node by id or Figma id, and never silently selects  | ready | — `ui-revamp` US-15 (gate G1–G5 PENDING Jang) |
| v0.6.7 | #1139 — Widget kit: Untitled UI component anatomy and states for Studio's sh  | ready | — `ui-revamp` US-09 (gate G1–G5 PENDING Jang) |
| v0.6.7 | #1140 A one-file, alias-resolved DTCG export per mode, so `ui ds import` onb  | ready | — `ui-revamp` US-16 (gate G1–G5 PENDING Jang) |
| v0.6.7 | #1165 Untitled UI kit: an effect style imports with no name and shows its ID  | ready | — `hardening-2026-10` US-01 (gate G1–G5 PENDING Jang) |
| v0.6.8 | #1119 App shell: title bar, tab bar, left rail, bottom toolbar, rulers and t  | ready | — `ui-revamp` US-04 (gate G1–G5 PENDING Jang) |
| v0.6.8 | #1120 Left panel: file header, Pages list and Layers tree in Untitled UI dar  | ready | — `ui-revamp` US-05 (gate G1–G5 PENDING Jang) |
| v0.6.8 | #1124 Right panel: inspector header, Design and Prototype tabs, Dev Inspect   | ready | — `ui-revamp` US-06 (gate G1–G5 PENDING Jang) |
| v0.6.9 | #1128 Dev mode (status, sections, code, playground, Compare changes, annotat  | ready | — `ui-revamp` US-07 (gate G1–G5 PENDING Jang) |
| v0.6.9 | #1138 Restyle the Home workspace in Untitled UI dark (Home density)  | ready | — `ui-revamp` US-08 (gate G1–G5 PENDING Jang) |
| v0.6.9 | #1144 Surface: Agents panel, terminal and agent cards (Untitled UI dark)  | ready | — `ui-revamp` US-10 (gate G1–G5 PENDING Jang) |
| v0.6.10 | #1146 Surface: command bar, dialogs, toasts, context menu, Find, comments, h  | ready | — `ui-revamp` US-11 (gate G1–G5 PENDING Jang) |
| v0.6.10 | #1155 Surface: Assets panel, Variables, Libraries and Plugins dialogs (Untit  | ready | — `ui-revamp` US-12 (gate G1–G5 PENDING Jang) |
| v0.6.11 | #1122 Research: can a public PolyForm Shield repo ship Untitled UI Icons?  | partial | research; chờ owner chọn licence (FC-UIR SC-08, đề xuất Option A) |
| v0.6.11 | #1125 Docs and screenshot refresh after the Untitled UI dark sweep  | ready | — `ui-revamp` US-14 (gate G1–G5 PENDING Jang) |
| v0.7 | #520 Drop shadows and layer effects differ from Figma on shadowed device mo | partial | thiếu edge cases; chưa có US v2 |
| v0.7 | #528 Export bounds differ from Figma by 1 to 84 px on 10 kit frames | partial | thiếu edge cases, mockup/ảnh; chưa có US v2 |
| v0.7 | #930 Approve or deny Codex tool calls from the Agents panel | partial | thiếu test steps, edge cases, mockup/ảnh; chưa có US v2 |
| v0.7.1 | #985 MCP lets an agent delete every child of an instance | missing | chỉ có mô tả; cần AC, test steps, edge cases, ảnh |
| v0.7.1 | #1009 Headless writers other than --export-fig follow symlinks and check no  | partial | thiếu test steps, edge cases, mockup/ảnh; chưa có US v2 |
| v0.7.1 | #1049 An agent can lift the user's designos tool restriction through the ses | partial | thiếu test steps, edge cases, mockup/ảnh; chưa có US v2 |
| v0.7.1 | #1050 Status hooks follow a symlink planted in the session folder | missing | chỉ có mô tả; cần AC, test steps, edge cases, ảnh |
| v0.7.1 | #1158 [CI] Windows STATUS_STACK_OVERFLOW in workspace tests + cfg(unix) for   | ready | — `hardening-2026-10` US-02 (gate G1–G5 PENDING Jang) |
| v0.7.1 | #1159 [CI] Force LF for rules.lock and normalise paths in rules-lock-drift (  | ready | — `hardening-2026-10` US-03 (gate G1–G5 PENDING Jang) |
| v0.7.2 | #1074 designos-mcp gets the whole disk as its workspace when the open docume | partial | thiếu AC, edge cases; chưa có US v2 |
| v0.7.2 | #1101 MCP and CLI writes skip the Ask gate | partial | thiếu test steps, edge cases, mockup/ảnh; chưa có US v2 |
| v0.7.2 | #1160 [CI] Switch to cargo-nextest with retries + quarantine for flaky tests | partial | thiếu edge cases, mockup/ảnh; chưa có US v2 |
| v0.7.2 | #1161 [CI] Add a fast ~2-minute check job before the test matrix | partial | thiếu edge cases, mockup/ảnh; chưa có US v2 |
| v0.7.2 | #1163 [PM] Contributor quality: CONTRIBUTING + PR template + fork-safe gate | partial | thiếu AC, edge cases, mockup/ảnh; chưa có US v2 |
| v0.7.3 | #1047 Gemini in Ask mode reads tools.allowed from the session folder's .gemi | partial | thiếu test steps, edge cases, mockup/ảnh; chưa có US v2 |
| v0.7.3 | #1048 agy, Gemini and Grok start MCP servers from agent-writable session con | partial | thiếu test steps, edge cases, mockup/ảnh; chưa có US v2 |

## Blockers / ghi chú
- Gate G1–G5 cần Jang duyệt; ready = sẵn cho dev, không phải gate APPROVED. FC-UIR v0.2 chờ Jang duyệt.
- PR #1166 chỉ chứa spec docs (`plans/specs/ui-revamp`, gồm `detail/`), không có code; mang được spec chi tiết của `docs/specs-untitled-ui-dark`. Chưa merge gì.
- #1165: root cause chưa chứng minh trên file thật (kit chỉ có trên Mac của Jang). Story bắt Bước 0 dump node thô trước khi code; nghi vấn chính là style đã xoá mềm/REMOVED vẫn được layer tham chiếu.
- #1159: `rules.lock` đã `-text` nên giả thuyết CRLF có thể sai; story bắt in dòng diff đầu tiên trước.
- #1072: phần còn lại là PR #1085 (mở). Ship v0.6.3+ với Windows đỏ vẫn là OPEN DECISION của Jang.
- Batch tiếp: các issue partial/missing còn lại từ v0.6.4 trở đi (#875, #887, #993, #1069, #983, #1003, #1088, …).
