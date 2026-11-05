Added an unsaved frame named Draft to the current Untitled document.

Draft is node 2 on Page 1 (node 1). Size is 100x100 at (0, 0). `designos_list_nodes` returned that frame. The session still has unsaved changes (`can_undo` is true). I did not call `designos_save_file`.

`designos_open_file` on `other.designos` returned `UNSAVED_CHANGES`. Switching files would throw away the Draft frame unless you confirm `discard_changes=true`. I do not have that confirmation, so I cannot list the frames in `other.designos`.

If you confirm discarding the unsaved Draft frame, I can open `other.designos` and report its frame names.
