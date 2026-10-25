# Spec readiness 2026-10-25 (23:30 UTC+7)

Nguồn: GitHub open issues, milestone v0.6.3–v0.7.x (v0.6.2 đang ship, không tính). Tiêu chí *ready*: Spec Driven v2 (kit v2.4.0), lint 0 lỗi, có AC + test steps + edge cases + mockup, không OPEN chặn. Gate G1–G5: AI không ghi APPROVED; tất cả PENDING Jang.

Đã làm: UI Revamp slice 1 (#1126, #1142, #1143) dev-ready trên nhánh `pm/spec-ui-revamp` (PR #1166, commit 246c297): 16 AC, 16 scenario test steps, edge cases, 3 mockup (`plans/specs/ui-revamp/evidence/us01-03.png`). Comment + nhãn `spec:ready` trên 3 issue.

## Tổng theo bản
| Release | ready | partial | missing |
|---|---|---|---|
| v0.6.3 | 0 | 1 | 0 |
| v0.6.4 | 1 | 3 | 1 |
| v0.6.5 | 1 | 1 | 2 |
| v0.6.6 | 1 | 2 | 0 |
| v0.6.7 | 0 | 5 | 0 |
| v0.6.8 | 0 | 3 | 0 |
| v0.6.9 | 0 | 3 | 0 |
| v0.6.10 | 0 | 2 | 0 |
| v0.6.11 | 0 | 2 | 0 |
| v0.7 | 0 | 3 | 0 |
| v0.7.1 | 0 | 4 | 2 |
| v0.7.2 | 0 | 5 | 0 |
| v0.7.3 | 0 | 2 | 0 |

## Chi tiết
| Release | Issue | Spec status | Gaps |
|---|---|---|---|
| v0.6.3 | #1072 Agent hook commands write through a link an agent plants in the sessio | partial | thiếu AC, edge cases, mockup/ảnh; chưa có US v2 |
| v0.6.4 | #875 Create a text style from selected text | partial | thiếu test steps, edge cases, mockup/ảnh; chưa có US v2 |
| v0.6.4 | #887 Dev Mode code panel is too narrow to read | partial | thiếu test steps, edge cases; chưa có US v2 |
| v0.6.4 | #993 run_tx records every touched node through two JSON values, which makes | missing | chỉ có mô tả; cần AC, test steps, edge cases, ảnh |
| v0.6.4 | #1069 Ask-mode Grok and Antigravity tabs start with an agent-edited config w | partial | thiếu AC, edge cases, mockup/ảnh; chưa có US v2 |
| v0.6.4 | #1126 Layout-unchanged gate: baseline geometry of every surface, and the che | ready | — (gate G1–G5 PENDING Jang) |
| v0.6.5 | #983 Editing a main component renames its instances on unloaded pages | missing | chỉ có mô tả; cần AC, test steps, edge cases, ảnh |
| v0.6.5 | #1003 Headless input checks from the QA red team (SwiftUI escape, exit codes | partial | thiếu AC, edge cases; chưa có US v2 |
| v0.6.5 | #1088 designos-diff has no --version | missing | chỉ có mô tả; cần AC, test steps, edge cases, ảnh |
| v0.6.5 | #1142 Spec A — Onboard Untitled UI dark with the design:os CLI: colour roles | ready | — (gate G1–G5 PENDING Jang) |
| v0.6.6 | #1141 CLI --screenshot sometimes exits 1: "screenshot was not captured" | partial | thiếu AC, test steps, edge cases; chưa có US v2 |
| v0.6.6 | #1143 Spec B — Untitled type, spacing, radius and control-size scale in `the | ready | — (gate G1–G5 PENDING Jang) |
| v0.6.6 | #1154 NodeSpec sizes hug text with a width estimate, so button labels sit of | partial | thiếu AC, edge cases, mockup/ảnh; chưa có US v2 |
| v0.6.7 | #1121 designos-mcp imports large kit files: one import limit, typed size err | partial | Spec chi tiết có AC/test/edge trên docs/specs-untitled-ui-dark; thiếu US v2 + test case file + mockup |
| v0.6.7 | #1127 The CLI addresses a node by id or Figma id, and never silently selects | partial | Spec chi tiết có AC/test/edge trên docs/specs-untitled-ui-dark; thiếu US v2 + test case file + mockup |
| v0.6.7 | #1139 — Widget kit: Untitled UI component anatomy and states for Studio's sh | partial | Spec chi tiết có AC/test/edge trên docs/specs-untitled-ui-dark; thiếu US v2 + test case file + mockup |
| v0.6.7 | #1140 A one-file, alias-resolved DTCG export per mode, so `ui ds import` onb | partial | Spec chi tiết có AC/test/edge trên docs/specs-untitled-ui-dark; thiếu US v2 + test case file + mockup |
| v0.6.7 | #1165 Untitled UI kit: an effect style imports with no name and shows its ID | partial | bug chưa rõ root cause (for:pm); cần điều tra .fig trước khi spec |
| v0.6.8 | #1119 App shell: title bar, tab bar, left rail, bottom toolbar, rulers and t | partial | Spec chi tiết có AC/test/edge trên docs/specs-untitled-ui-dark; thiếu US v2 + test case file + mockup |
| v0.6.8 | #1120 Left panel: file header, Pages list and Layers tree in Untitled UI dar | partial | Spec chi tiết có AC/test/edge trên docs/specs-untitled-ui-dark; thiếu US v2 + test case file + mockup |
| v0.6.8 | #1124 Right panel: inspector header, Design and Prototype tabs, Dev Inspect  | partial | Spec chi tiết có AC/test/edge trên docs/specs-untitled-ui-dark; thiếu US v2 + test case file + mockup |
| v0.6.9 | #1128 Dev mode (status, sections, code, playground, Compare changes, annotat | partial | Spec chi tiết có AC/test/edge trên docs/specs-untitled-ui-dark; thiếu US v2 + test case file + mockup |
| v0.6.9 | #1138 Restyle the Home workspace in Untitled UI dark (Home density) | partial | Spec chi tiết có AC/test/edge trên docs/specs-untitled-ui-dark; thiếu US v2 + test case file + mockup |
| v0.6.9 | #1144 Surface: Agents panel, terminal and agent cards (Untitled UI dark) | partial | Spec chi tiết có AC/test/edge trên docs/specs-untitled-ui-dark; thiếu US v2 + test case file + mockup |
| v0.6.10 | #1146 Surface: command bar, dialogs, toasts, context menu, Find, comments, h | partial | Spec chi tiết có AC/test/edge trên docs/specs-untitled-ui-dark; thiếu US v2 + test case file + mockup |
| v0.6.10 | #1155 Surface: Assets panel, Variables, Libraries and Plugins dialogs (Untit | partial | Spec chi tiết có AC/test/edge trên docs/specs-untitled-ui-dark; thiếu US v2 + test case file + mockup |
| v0.6.11 | #1122 Research: can a public PolyForm Shield repo ship Untitled UI Icons? | partial | Spec chi tiết có AC/test/edge trên docs/specs-untitled-ui-dark; thiếu US v2 + test case file + mockup |
| v0.6.11 | #1125 Docs and screenshot refresh after the Untitled UI dark sweep | partial | Spec chi tiết có AC/test/edge trên docs/specs-untitled-ui-dark; thiếu US v2 + test case file + mockup |
| v0.7 | #520 Drop shadows and layer effects differ from Figma on shadowed device mo | partial | thiếu edge cases; chưa có US v2 |
| v0.7 | #528 Export bounds differ from Figma by 1 to 84 px on 10 kit frames | partial | thiếu edge cases, mockup/ảnh; chưa có US v2 |
| v0.7 | #930 Approve or deny Codex tool calls from the Agents panel | partial | thiếu test steps, edge cases, mockup/ảnh; chưa có US v2 |
| v0.7.1 | #985 MCP lets an agent delete every child of an instance | missing | chỉ có mô tả; cần AC, test steps, edge cases, ảnh |
| v0.7.1 | #1009 Headless writers other than --export-fig follow symlinks and check no  | partial | thiếu test steps, edge cases, mockup/ảnh; chưa có US v2 |
| v0.7.1 | #1049 An agent can lift the user's designos tool restriction through the ses | partial | thiếu test steps, edge cases, mockup/ảnh; chưa có US v2 |
| v0.7.1 | #1050 Status hooks follow a symlink planted in the session folder | missing | chỉ có mô tả; cần AC, test steps, edge cases, ảnh |
| v0.7.1 | #1158 [CI] Windows STATUS_STACK_OVERFLOW in workspace tests + cfg(unix) for  | partial | thiếu edge cases, mockup/ảnh; chưa có US v2 |
| v0.7.1 | #1159 [CI] Force LF for rules.lock and normalise paths in rules-lock-drift ( | partial | thiếu edge cases, mockup/ảnh; chưa có US v2 |
| v0.7.2 | #1074 designos-mcp gets the whole disk as its workspace when the open docume | partial | thiếu AC, edge cases; chưa có US v2 |
| v0.7.2 | #1101 MCP and CLI writes skip the Ask gate | partial | thiếu test steps, edge cases, mockup/ảnh; chưa có US v2 |
| v0.7.2 | #1160 [CI] Switch to cargo-nextest with retries + quarantine for flaky tests | partial | thiếu edge cases, mockup/ảnh; chưa có US v2 |
| v0.7.2 | #1161 [CI] Add a fast ~2-minute check job before the test matrix | partial | thiếu edge cases, mockup/ảnh; chưa có US v2 |
| v0.7.2 | #1163 [PM] Contributor quality: CONTRIBUTING + PR template + fork-safe gate | partial | thiếu AC, edge cases, mockup/ảnh; chưa có US v2 |
| v0.7.3 | #1047 Gemini in Ask mode reads tools.allowed from the session folder's .gemi | partial | thiếu test steps, edge cases, mockup/ảnh; chưa có US v2 |
| v0.7.3 | #1048 agy, Gemini and Grok start MCP servers from agent-writable session con | partial | thiếu test steps, edge cases, mockup/ảnh; chưa có US v2 |

## Blockers / ghi chú
- Gate G1–G5 cần Jang duyệt; ready = sẵn cho dev, không phải gate APPROVED.
- Spec chi tiết UI Revamp nằm trên nhánh `docs/specs-untitled-ui-dark` (chưa merge main); dev phải re-anchor dòng trích dẫn.
- #1165 cần điều tra (.fig) trước khi viết spec.
- Batch tiếp: UI Revamp còn lại (C–K) → wrap US v2 + mockup; rồi #1158/#1159 (CI, no_ui) → thêm edge cases + US v2.
