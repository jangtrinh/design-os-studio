Goal: add an unsaved frame named Draft in the current DesignOS document, then list the frame names in other.designos.

I added a frame named Draft to the current Untitled document. It is node 2, a child of Page 1. The change is in memory only. I did not save.

Opening other.designos was refused with UNSAVED_CHANGES. The session still holds the unsaved Draft frame. I did not set discard_changes, because you have not confirmed throwing that work away. I also did not save, because you asked me to leave it unsaved.

I could not list the frames in other.designos. Confirm that the unsaved Draft may be discarded, or allow a save first, and I can open the file and report the names.

Principles that shaped this: Laziness Protocol (one create_node, no extra writes), Prove It Works (list_nodes showed Draft before the open attempt), Sequence Work into Verifiable Units (verify the create, then attempt the open), Never Block on the Human (I stopped here because discarding unsaved work is irreversible without your confirmation).
