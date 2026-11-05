"""Publish the staged library file only after every check passed.

    staged = stage_path(workspace, out)      # where `finish` writes the file and its allow-list: inside the workspace, never at `out`
    ... checks run on `staged` and raise SystemExit on a failure ...
    publish(staged, out)                     # the last step of `library`: replace `out` and the allow-list next to it

A failing check leaves the committed `design/studio-ui.designos` and `design/studio-ui.gate-allowlist.json` as they were. A fault inside
`publish` (a full disk, an interrupt, a kill) cannot leave a half pair that passes: both temporaries are written first, the file is replaced
before the allow-list, and the allow-list records the sha256 of the file it belongs with (studio_ui_gate_pair), so a new file beside the old
allow-list is refused by the gate. Before the first replacement the old pair is untouched.
"""
import os
import pathlib
import shutil

import studio_ui_theme_policy as policy

ALLOWLIST_NAME = pathlib.Path(policy.ALLOWLIST_REL).name


def stage_path(workspace, out):
    """The staged copy of `out`: the same file name inside `<workspace>/staged`."""
    return pathlib.Path(workspace) / "staged" / pathlib.Path(out).name


def staged_allowlist(staged):
    return pathlib.Path(staged).parent / ALLOWLIST_NAME


def publish(staged, out):
    """Replace `out` and the allow-list beside it with the staged files. Both are copied to temporary names first, so the step that can run out
    of disk happens while nothing is replaced; then the two renames run back to back, the file first and the allow-list last. Temporaries
    are removed whatever happens."""
    out = pathlib.Path(out)
    pairs = ((pathlib.Path(staged), out), (staged_allowlist(staged), out.parent / ALLOWLIST_NAME))
    missing = [str(src) for src, _ in pairs if not src.is_file()]
    if missing:
        raise SystemExit(f"nothing was published ({out} is unchanged): {', '.join(missing)} does not exist")
    out.parent.mkdir(parents=True, exist_ok=True)
    temps = [(src, dst.with_name(dst.name + ".publishing"), dst) for src, dst in pairs]
    try:
        for src, tmp, _ in temps:
            shutil.copyfile(src, tmp)
        for _, tmp, dst in temps:
            os.replace(tmp, dst)
    finally:
        for _, tmp, _ in temps:
            tmp.unlink(missing_ok=True)
