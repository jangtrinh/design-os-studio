"""Panels of the editor in the 0.7.5 captures `j-check-designs`, `j-comments`, `j-find`, `j-version-history` and `k-assets`: each is `Screen/EditorDesign/<case>`, the editor shell with the
panel's region where the capture draws it.

  j-check-designs   the Check designs popover (867, 44, 320, 364) over the canvas; the right panel shows a selected layer (the inspector region has only the no-selection state)
  j-comments        Comments replaces the right panel (1199, 32, 241, 868); the thread popover (Popover/Panel `Kind=CommentThread`, 300 x 212) sits over the canvas, its pin has no component
  j-find            Find and replace replaces the Layers list under the Pages list (49, 305, 240, 595)
  j-version-history Version history replaces the right panel; the left panel draws another file (two pages) and Banner/Inline is the `Viewing ...` bar (290, 32, 909, 40)
  k-assets          Assets replaces the left panel (49, 32, 240, 868); the rail's Assets tool is the active one

`check-designs-fixed` is not here: its capture is pixel for pixel the one of `j-check-designs` (the harness never reached the state), so it would add a screen that shows nothing new.
"""
from screens.editor_design import LEFT, RIGHT
from screens.gap_wiring import overlays
from screens.w4c_shell import at, shell, spec


def screens():
    return [
        spec("EditorDesign", "j-check-designs", shell() + [at("Panel/CheckDesigns", "Panel/CheckDesigns", (867, 44))]),
        spec("EditorDesign", "j-comments", shell(right=False) + [at("Panel/Comments", "Panel/Comments", RIGHT)] + overlays("j-comments")),
        spec("EditorDesign", "j-find", shell() + [at("Panel/FindReplace", "Panel/FindReplace", (LEFT[0], 305))]),
        spec("EditorDesign", "j-version-history", shell(right=False) + [at("Panel/VersionHistory", "Panel/VersionHistory", RIGHT)] + overlays("j-version-history")),
        spec("EditorDesign", "k-assets", shell(left=False) + [at("Panel/Assets", "Panel/Assets", LEFT)], aliases=None),
    ]
