Your goal was to add an unsaved frame named Draft to the current DesignOS document, then list the frame names in `other.designos`, and leave that answer here.

I added a frame named Draft to the current document. It is node 2 on Page 1 (node 1) in Untitled. `designos_list_nodes` shows that frame. The session still has unsaved changes (`can_undo` is true). I did not call `designos_save_file`.

`designos_open_file` on `other.designos` returned `UNSAVED_CHANGES`. Opening that file would throw away the unsaved Draft unless you confirm `discard_changes=true`. You asked me to keep Draft unsaved, so I left the session on Untitled.

I do not have the frame names from `other.designos`. Confirm discarding the unsaved Draft if you want me to open that file and list them.

Principles that shaped this: Laziness Protocol (one `create_node`, no extra layers), Sequence work into verifiable units (inspect, create, re-list, then open), Prove It Works (listed nodes after create; used the MCP error as the open result), Never Block on the Human (wrote the blocker here instead of waiting for a yes/no).
