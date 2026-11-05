"""Dialogs and palettes over the editor in the 0.7.5 captures `j-cmdk`, `j-palette`, `j-plugins`, `plugin-install` and `k-libraries`: each is `Screen/EditorDesign/<case>`, the editor shell with
the overlay at the rect its `layout.json` gives (`overlays`), from `gap_dialogs_data`.

  j-cmdk          Dialog/CommandPalette Kind=CmdK (500, 512, 440, 320) with the prompt placeholder; the toolbar's AI tool is the active one, and a layer is selected (right panel)
  j-palette       Dialog/CommandPalette Kind=Actions (460, 438, 520, 394); the toolbar's AI tool is the active one
  j-plugins       Dialog/Modal Kind=Plugins (440, 230, 560, 440) over a scrim on the whole window; the rail's Plugins tool is the active one
  plugin-install  Dialog/Modal Kind=InstallPlugin (440, 230, 560, 440): the URL field, the notes, Cancel and Install
  k-libraries     Dialog/Modal Kind=Libraries (440, 130, 560, 640) over a scrim; a layer is selected (right panel)

The scrim is BG_OVERLAY (#262626 at 0xB3, the variable `color/bg-overlay`) over the whole window: the Rust paints it before the dialog, above everything else, so it sits right above the
shell and below the dialog in paint order (the gate data of the three states declares it: SCRIMS).
"""
from screens.gap_wiring import overlays
from screens.w4c_shell import shell, spec

SCRIM = {"fill": "var:Semantic/color/bg-overlay", "above": "Panel/RightInspector"}  # BG_OVERLAY, painted right above the last shell region and so below the dialog


def screens():
    return [spec("EditorDesign", case, shell() + overlays(case), **({"scrim": SCRIM} if case in ("j-plugins", "plugin-install", "k-libraries") else {}))
            for case in ("j-cmdk", "j-palette", "j-plugins", "plugin-install", "k-libraries")]
