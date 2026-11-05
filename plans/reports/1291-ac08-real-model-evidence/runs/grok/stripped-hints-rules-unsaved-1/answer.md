You asked me to add an unsaved Draft frame in the current DesignOS document, then report the frame names in other.designos.

I added a frame named Draft (node id 2) on Page 1 of the current Untitled document. designos_list_nodes shows that frame in memory. I did not save.

I then called designos_open_file on other.designos without discard_changes. The tool returned UNSAVED_CHANGES. I cannot list frames in other.designos while those unsaved changes remain, and I will not discard them or save unless you say so.

Principles that shaped this: Laziness Protocol (one create_node call, no extra properties), Sequence Work into Verifiable Units (confirm Draft exists before opening the other file), Prove It Works (list_nodes after the create).
