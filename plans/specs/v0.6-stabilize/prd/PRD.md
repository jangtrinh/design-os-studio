---
id: prd-v06
type: prd
title: "PRD v0.6 stabilize"
status: Written
updated: 2026-10-25
primary: true
---

## 1. Problem
Xem [contract](../contract/FC-V06.md).

## 2. Yêu cầu

| ID | Yêu cầu | Nguồn | US |
|---|---|---|---|
| REQ-01 | designos_export_fig_file follows a final symlink and overwrites without opt-in (merged in #1103): sửa xong theo AC. | #1008 | US-01 |
| REQ-02 | MCP file outputs: validate names and omit outside paths (merged in #1103): sửa xong theo AC. | #1087 | US-01 |
| REQ-03 | Ask-mode Claude loads allow rules from the session folder's .claude/settings*.json, which a Codex sandbox can write: sửa xong theo AC. | #1044 | US-01 |
| REQ-04 | Ask-mode Claude starts MCP servers from the session folder's mcp-config.json, which a Codex sandbox can write: sửa xong theo AC. | #1046 | US-01 |
| REQ-05 | Document and page names can add commands to Studio's Claude skill: sửa xong theo AC. | #1053 | US-01 |
| REQ-06 | Claude config in the agent-writable session folder (hooks.json, .claude/settings.local.json): sửa xong theo AC. | #1036 | US-01 |
| REQ-07 | Agents can answer their own approval requests: the approval folder sits in the agent's working directory: sửa xong theo AC. | #1023 | US-02 |
| REQ-08 | An agent can redirect another tab's session folder through a link in $TMPDIR: sửa xong theo AC. | #1057 | US-02 |
| REQ-09 | Studio writes agent config files through a link an agent plants inside its session folder: sửa xong theo AC. | #1061 | US-02 |
| REQ-10 | designos-mcp writes its session files through a link an agent plants in the session folder: sửa xong theo AC. | #1068 | US-02 |
| REQ-11 | Ask-mode Grok and Antigravity tabs start with an agent-edited config when their config folder is read-only: sửa xong theo AC. | #1069 | US-03, US-05 |
| REQ-12 | Create a text style from selected text: sửa xong theo AC. | #875 | US-03, US-06 |
| REQ-13 | Dev Mode code panel is too narrow to read: sửa xong theo AC. | #887 | US-03, US-07 |
| REQ-14 | run_tx records every touched node through two JSON values, which makes large edits slow: sửa xong theo AC. | #993 | US-03, US-08 |
| REQ-15 | Selection Styles section hides library styles when the file has no local style: sửa xong theo AC. | #1039 | US-04 |
| REQ-16 | designos-diff has no --version: sửa xong theo AC. | #1088 | US-04, US-11 |
| REQ-17 | Editing a main component renames its instances on unloaded pages: sửa xong theo AC. | #983 | US-04, US-09 |
| REQ-18 | Headless input checks from the QA red team (SwiftUI escape, exit codes, file names, spec errors): sửa xong theo AC. | #1003 | US-04, US-10 |
