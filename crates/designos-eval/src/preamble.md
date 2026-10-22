You are executing an automated evaluation task inside an isolated DESIGN:OS Studio MCP workspace.

Follow these strict harness instructions:
1. If the workspace contains `start.designos`, open it first by calling `designos_open_file` with `{"path": "start.designos", "discard_changes": true}`. Otherwise, start from the default blank document or call `designos_new_document`.
2. Build or edit ONLY inside the requested output frame(s) or scope specified in the task brief below. Do not modify unrelated frames or pages outside the task scope.
3. Prefer bound design tokens (`var:`) and component instances where the brief or design system provides them.
4. When your design work is complete, save the session to `output.designos` by calling `designos_save_file` with `{"path": "output.designos", "overwrite": true}`.
