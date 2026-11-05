"""Popover/Panel `Kind` registry: the builders of every kind other than Default, in the registry's order (the set's module, menus_popovers.py, builds Default itself). Not a component module.

    ORDER      the Kind values after Default, in the order of design/component-registry.json
    KINDS      {Kind: builder(env) -> IR frame}: the settings popovers (popover_kinds_settings.py) and the ones with their own painter
    build(kind, env)   the IR frame of one kind
"""
from components.popover_kinds_dialogs import DIALOGS
from components.popover_kinds_settings import TABLES
from components.popover_kinds_shell import LABEL_X_REASON, settings_popover  # noqa: F401  (LABEL_X_REASON: re-exported for Default's header)
from components.popover_kinds_skills import SKILLS
from components.popover_kinds_type import TYPE

ORDER = ("EditStyleColor", "EditStyleEffect", "EditStyleGrid", "EditStyleText", "Stroke", "Noise", "Glass", "Texture", "TypeSettings", "Interaction", "Erase", "Expand", "CommentThread", "Skills",
         "Permissions")


def _settings(table):
    def build(env):
        title, rows = table(env)
        return settings_popover(env, title, rows)
    return build


_BUILDERS = {**{kind: _settings(table) for kind, table in TABLES.items()}, **DIALOGS, **TYPE, **SKILLS}
if set(_BUILDERS) != set(ORDER):
    raise ValueError(f"popover kinds without a builder {sorted(set(ORDER) - set(_BUILDERS))}, builders without a kind {sorted(set(_BUILDERS) - set(ORDER))}")
KINDS = {kind: _BUILDERS[kind] for kind in ORDER}


def build(kind, env):
    return KINDS[kind](env)
