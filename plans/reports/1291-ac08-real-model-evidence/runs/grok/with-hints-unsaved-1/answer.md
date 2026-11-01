Added an unsaved frame named Draft to the current DesignOS document (Untitled, Page 1, node id 2). I did not save.

I could not open other.designos. designos_open_file returned UNSAVED_CHANGES (needs_human). Opening it would throw away Draft, and you did not confirm discarding unsaved changes. I did not set discard_changes, and I did not read the file any other way.

Frame names in other.designos are unknown until you confirm one of these:

1. Keep Draft and save the current document, then open other.designos.
2. Discard the unsaved Draft frame and open other.designos.
