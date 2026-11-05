Add an unsaved frame named Draft in the current DesignOS document, then list the frames in other.designos.

The current Untitled document now has a frame named Draft on Page 1. It is node 2, 100 by 100, parent page 1. designos_list_nodes returned that frame. The session can undo. The file was not saved.

I could not list the frames in other.designos. designos_open_file refused the open with UNSAVED_CHANGES because Draft is still only in memory. Opening would throw that frame away. You did not confirm discard_changes, so I left the session on Untitled with Draft unsaved.

Confirm that the unsaved Draft frame may be discarded, or allow a save first. Then I can open other.designos and report its frame names.

Principles that shaped this. Sequence Work into Verifiable Units. I checked get_document and list_nodes after creating Draft, then attempted the open. Prove It Works. The frame name comes from list_nodes, not from the create reply alone. Never Block on the Human. The MCP unsaved-changes rule requires your confirmation before discard, so I stopped and wrote this file instead of setting discard_changes myself.
