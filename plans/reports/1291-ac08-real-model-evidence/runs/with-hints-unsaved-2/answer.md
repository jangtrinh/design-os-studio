Goal: add an unsaved frame named Draft to the current DesignOS document, then report the frame names in other.designos.

I added a frame named Draft to the current in-memory document. It is node 2, a Frame, child of Page 1 (node 1). designos_get_document then showed node_count 2, Page 1 child_count 1, and can_undo true. I did not call designos_save_file.

I could not list the frames in other.designos. designos_open_file with path other.designos returned UNSAVED_CHANGES, error_kind needs_human. The tool said to stop and tell you the open document has unsaved changes, and not to discard them. safe_to_retry was false. I did not retry, discard, save, or read the file another way.

To continue, save the current document, or tell me to discard the unsaved Draft and open other.designos.

Principles: Laziness Protocol (ran the specified MCP steps, no extra playbook), Sequence Work into Verifiable Units (create, then get_document/list_nodes, then open), Prove It Works (Draft confirmed by list_nodes), Never Block on the Human (overridden by the MCP needs_human stop).
