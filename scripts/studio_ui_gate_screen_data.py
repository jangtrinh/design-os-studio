"""The per-batch gate data of the state screens: every module `studio_ui_file/screens/<batch>_gate.py` declares, per state (a capture's file name), what the screen gate needs to judge that screen.

A batch adds its own data module and no hook line anywhere: `load()` finds the modules by their file name, and `screen_caps`, `screen_masks`, `screens.check_specs` and the
file gate (G3 screens) read the merged tables. A module is plain data (it imports nothing of the package), so the file gate, which runs without Pillow, MCP or the screens
package, can read it. The tables, each `{state: value}`:

    CAPS        {region: cap}     the mean-difference cap of every region the state reports (and `screen`); the editor-default cap where the region has one, else a pinned measured value
    CAUSES      {region: cause}   why a cap is not the editor-default cap of its region; `worst tile` for the tile override. No self-calibration: default, or pinned with a cause
    WORST_TILE  number            the worst-tile cap of the state, when it cannot meet the default 34 (its cause is CAUSES[state]["worst tile"])
    GEOMETRY    [{key, layout, node, cause}]   a named widget rect known to be off by more than 1 px, with the measured numbers: the gate passes the pinned numbers and fails any others
    TEXTS       [(rect, what, neutral text)]   the kit strings the screen replaces with neutral ones, left out of the comparison
    CONTENT     (content rect, toolbar rect) | None   the document's own content left out of the comparison (None: the state has none)
    REGIONS     {name: rect}      the named rects whose numbers are reported next to the whole screen's
    KEEP        [rect]            rects inside the content area that stay measured (a dialog or popover over the canvas is chrome)
    SCRIMS      {"variable": variable name or None, "reason": str}   the state's screen draws the app's scrim: one rectangle `Scrim` at window size

    tables = load(folder=SCREENS)       {table: {state: value}} of every `*_gate.py` module of the folder
    merge(base, tables, table)          `base` (a framework table) plus the batches' states; a state in two places raises
"""
import importlib.util
import pathlib

SCREENS = pathlib.Path(__file__).resolve().parent / "studio_ui_file" / "screens"
TABLES = ("CAPS", "CAUSES", "WORST_TILE", "GEOMETRY", "TEXTS", "CONTENT", "REGIONS", "KEEP", "SCRIMS")


def _module(path):
    spec = importlib.util.spec_from_file_location(f"_studio_ui_screen_gate_{path.stem}", path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def load(folder=SCREENS):
    out, owner = {t: {} for t in TABLES}, {}
    for path in sorted(pathlib.Path(folder).glob("*_gate.py")):
        mod = _module(path)
        unknown = sorted(n for n in vars(mod) if n.isupper() and not n.startswith("_") and n not in TABLES)
        if unknown:
            raise ValueError(f"{path.name}: {unknown} is not a gate table (known: {', '.join(TABLES)})")
        for table in TABLES:
            rows = getattr(mod, table, None)
            if rows is None:
                continue
            if not isinstance(rows, dict):
                raise ValueError(f"{path.name}: {table} must be a dict of state -> value")
            for state, value in rows.items():
                if state in out[table]:
                    raise ValueError(f"{table}[{state!r}] is declared by {owner[(table, state)]} and by {path.name}")
                out[table][state], owner[(table, state)] = value, path.name
    return out


def merge(base, tables, table):
    clash = sorted(set(base) & set(tables[table]))
    if clash:
        raise ValueError(f"{table}: {clash} is in the framework table and in a batch's gate data")
    return {**base, **tables[table]}
