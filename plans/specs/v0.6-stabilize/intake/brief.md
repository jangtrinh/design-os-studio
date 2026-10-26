---
id: intake-v06
type: intake
title: "Intake — v0.6 stabilize day"
status: Written
updated: 2026-10-25
---

- Nguồn: GitHub milestones v0.6.2–v0.6.5 của jangtrinh/design-os-studio, chỉ thị của Jang ngày 2026-10-25 (D1, D2).
- Nature: `roadmap` + `defect`.
- Duplicate search: #1080 vs #1103 (D3); không có spec v2 trùng — các file cũ ở `docs/specs/v0.6` và `docs/specs/v0.6.x` là v1 theo issue và được dẫn như nguồn.
- Lý do xử lý: các lỗi agent sandbox cho phép agent tự cấp quyền; cần đóng trước khi thêm tính năng.

## Issue nguồn

| Issue | Tiêu đề | Patch |
|---|---|---|
| #1008 | designos_export_fig_file follows a final symlink and overwrites without opt-in (merged in #1103) | v0.6.2 |
| #1087 | MCP file outputs: validate names and omit outside paths (merged in #1103) | v0.6.2 |
| #1044 | Ask-mode Claude loads allow rules from the session folder's .claude/settings*.json, which a Codex sandbox can write | v0.6.2 |
| #1046 | Ask-mode Claude starts MCP servers from the session folder's mcp-config.json, which a Codex sandbox can write | v0.6.2 |
| #1053 | Document and page names can add commands to Studio's Claude skill | v0.6.2 |
| #1036 | Claude config in the agent-writable session folder (hooks.json, .claude/settings.local.json) | v0.6.2 |
| #1023 | Agents can answer their own approval requests: the approval folder sits in the agent's working directory | v0.6.3 |
| #1057 | An agent can redirect another tab's session folder through a link in $TMPDIR | v0.6.3 |
| #1061 | Studio writes agent config files through a link an agent plants inside its session folder | v0.6.3 |
| #1068 | designos-mcp writes its session files through a link an agent plants in the session folder | v0.6.3 |
| #1069 | Ask-mode Grok and Antigravity tabs start with an agent-edited config when their config folder is read-only | v0.6.4 |
| #875 | Create a text style from selected text | v0.6.4 |
| #887 | Dev Mode code panel is too narrow to read | v0.6.4 |
| #993 | run_tx records every touched node through two JSON values, which makes large edits slow | v0.6.4 |
| #1039 | Selection Styles section hides library styles when the file has no local style | v0.6.5 |
| #1088 | designos-diff has no --version | v0.6.5 |
| #983 | Editing a main component renames its instances on unloaded pages | v0.6.5 |
| #1003 | Headless input checks from the QA red team (SwiftUI escape, exit codes, file names, spec errors) | v0.6.5 |
