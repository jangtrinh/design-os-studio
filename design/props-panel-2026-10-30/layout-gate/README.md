Layout gate finding (shell::tests::layout_unchanged::e, case e-layers-selected):
after the change every right-panel widget below the instance header moves down by exactly +28 px
(the "From this file" source line now shows for every instance; before, only library instances had a row).
Header widgets (instance-swap, go-to-main, instance-more) do not move. 'mode:Starter progress and icons'
moves from region right_panel to unknown (pushed past the visible edge) -> G3 region change.
Fix needed: allow.json entries (spec E/rule 11 bless procedure) for case e-layers-selected; not done.
