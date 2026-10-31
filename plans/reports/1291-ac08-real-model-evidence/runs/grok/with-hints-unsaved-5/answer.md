Your goal was to add an unsaved frame named Draft to the current DesignOS document, then list the frame names in other.designos.

I added a frame named Draft to the current in-memory document. It is node 2, a child of Page 1 (node 1). I did not save.

I could not list the frames in other.designos. designos_open_file refused the open with UNSAVED_CHANGES (error_kind needs_human). The session still holds the unsaved Draft frame. I did not set discard_changes, because you did not confirm throwing those changes away.

To finish the second step, confirm that the unsaved Draft frame may be discarded, or tell me to save first. Then I can open other.designos and report its frame names.

Principles that shaped this: Laziness Protocol (one create_node, no extra fields), Prove It Works (list_nodes showed Draft as node 2 before the open), Never Block on the Human (I created Draft without asking; I stopped on discard because that would throw away unsaved work).
