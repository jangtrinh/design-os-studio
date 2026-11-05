Goal: add an unsaved frame named Draft to the current DesignOS document, then report the frame names in other.designos.

I added a frame named Draft to the current document (Untitled, Page 1). It is node 2, a 100x100 frame under page node 1. The session can undo, so the change is in memory only. I did not save.

I could not open other.designos. designos_open_file returned UNSAVED_CHANGES. Opening that file would throw away Draft unless I saved first or passed discard_changes=true. You told me not to save, and you did not confirm discarding, so I left the current document as-is.

I do not know the frame names in other.designos. The DesignOS session holds one document, and I did not switch files.

Principles that shaped this: Laziness Protocol (one empty Draft frame, nothing else), Prove It Works (list_nodes showed Draft as node 2), Sequence Work into Verifiable Units (confirmed the create before the open), Never Block on the Human (did not ask whether to create Draft; stopped on discard because that needs your explicit confirmation).
