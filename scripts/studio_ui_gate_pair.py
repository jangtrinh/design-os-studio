"""Pair guard: the allow-list records the sha256 of the `.designos` it was written for, so a half-published pair cannot pass.

`design/studio-ui.designos` and `design/studio-ui.gate-allowlist.json` are two files replaced one after the other (`library_publish`). A fault
between the two replacements, or an edit of one outside `library`, would leave a file next to an allow-list written for another. The
allow-list therefore carries `designos_sha256`, and `library` replaces the file first and the allow-list last: the states a publish can
leave are old + old, new + old (the recorded hash is the old file's, so the gate refuses it) and new + new. A one-way recorded hash is
enough: the allow-list is a single file replaced atomically, so it is always either whole old or whole new, and the file cannot be new
beside an allow-list that names it unless both were published.
"""
from __future__ import annotations

import hashlib
import json
from pathlib import Path

from studio_ui_theme_policy import ALLOWLIST_REL

KEY = "designos_sha256"


def file_sha256(path) -> str:
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def allowlist_document(file_path, entries: list) -> dict:
    """The allow-list JSON for the file at `file_path`: its sha256 and the entries."""
    return {KEY: file_sha256(file_path), "allow": entries}


def write_allowlist(file_path, entries: list) -> Path:
    """Write the allow-list for the file at `file_path` beside it (`design/studio-ui.gate-allowlist.json` next to `design/studio-ui.designos`)."""
    target = Path(file_path).parent / Path(ALLOWLIST_REL).name
    target.write_text(json.dumps(allowlist_document(file_path, entries), indent=2) + "\n", encoding="utf-8")
    return target


def check_pair(file_path, allow_doc: dict) -> tuple[list, str]:
    """(failures, the file's sha256): the allow-list names exactly this file."""
    actual, recorded = file_sha256(file_path), allow_doc.get(KEY)
    if not isinstance(recorded, str) or not recorded:
        return [f"the allow-list does not record the sha256 of the file it belongs with ({KEY}): rebuild both with "
                f"`python3 scripts/studio_ui_file library`"], actual
    if recorded != actual:
        return [f"the file (sha256 {actual[:12]}...) is not the one its allow-list was written for ({recorded[:12]}...): a half-published pair, "
                f"or the file was edited outside `library`; rebuild both with `python3 scripts/studio_ui_file library`"], actual
    return [], actual
