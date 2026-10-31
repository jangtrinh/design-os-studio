Layout gate finding (shell::tests::layout_unchanged::e, case e-layers-selected):
after the change every right-panel widget below the instance header moves down by exactly +28 px
(the "From this file" source line now shows for every instance; before, only library instances had a row).
Header widgets (instance-swap, go-to-main, instance-more) do not move. 'mode:Starter progress and icons'
moves from region right_panel to unknown (pushed past the visible edge) -> G3 region change.
Resolved (v0.7.6): scope is narrow.
- allow.json: two entries, both case `e-layers-selected` only (regions right_panel and unknown), `dy: 28` (+-0.5) only; no dw/dh; right_panel also `inventory_max_delta: 1`.
- Baseline: one field edited in `e-layers-selected.layout.json` (`mode:Starter progress and icons` region right_panel -> unknown), because a named-widget region change cannot be allowed by an entry.
- Mutation proof: with right_panel `dy` set to 29, `layout_unchanged_e` fails with a G3 rect drift on `distribute` (baseline y 301 vs current 329). Entries restored afterwards.
