You asked me to add an unsaved frame named Draft to the current DesignOS document, then open other.designos and report its frame names.

I added the Draft frame to the current in-memory document and left it unsaved. I could not list the frames in other.designos.

**Current document.** Untitled, page "Page 1" (node 1). I created a frame named Draft (node 2, parent 1, 100x100). designos_list_nodes returned that frame. can_undo is true, so the edit is still only in memory. I did not call designos_save_file.

**Opening other.designos.** designos_open_file with path other.designos returned UNSAVED_CHANGES. The session still holds the unsaved Draft frame. Opening another file would throw that edit away unless discard_changes=true. You did not confirm discarding, and you told me not to save yet, so I stopped.

I do not know the frame names in other.designos.

To finish, tell me either that I may save the current document first, or that I may discard the unsaved Draft frame and open other.designos.
