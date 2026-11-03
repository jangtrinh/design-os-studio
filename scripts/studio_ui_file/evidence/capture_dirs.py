"""Where the compare finds the 0.7.5 captures, without Pillow.

    captures_dir(rel, override)    the run's capture folder: `--captures`, else `rel` under the repo, else under the main checkout (captures are untracked, so a worktree lacks them)
    captures_for(entry, default)   the folder of one entry's capture: its own `captures` (a repo-relative folder, resolved like the default), else the run's

An entry carries its own `captures` when its picture is not in the run's folder: the empty Home (`h-home-empty`, the Recents view of a library with no file) is a capture of the same 0.7.5 build
that the 87 older rows were taken from. It is tracked in `scripts/studio_ui_file/evidence/captures` (Studio chrome only: no kit content),
so a fresh checkout or a scratch copy has it and needs no manual copy.
"""
import pathlib
import subprocess

REPO = pathlib.Path(__file__).resolve().parents[3]


def captures_dir(rel, override):
    if override:
        return pathlib.Path(override)
    here = REPO / rel
    if here.exists():
        return here
    common = subprocess.check_output(["git", "-C", str(REPO), "rev-parse", "--git-common-dir"], text=True).strip()
    return (pathlib.Path(common) if pathlib.Path(common).is_absolute() else REPO / common).resolve().parent / rel


def captures_for(entry, default):
    return captures_dir(entry["captures"], None) if entry.get("captures") else default
