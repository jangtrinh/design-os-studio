"""Component modules of the Studio UI file. One module per registry row group; the assembler imports every module in this
package, so adding a module is the whole registration: no shared list to edit.

A module exports `component_sets()` returning a list of specs (dicts):

    name    registry name, exact (`Button/Primary`). The gate fails a set that is not a registry record.
    row     canvas row (a section of the page): one of ROWS (Actions, Forms & Inputs, Badges & Status, Menus & Overlays, Lists & Trees, Navigation) for a registry
            `element` or `component` on page `02 Core`, one of STUDIO_ROWS (Headers & Tabs, Menus & Popovers, Dialogs, Bars & Feedback, Cards, Data & Code, Agents, Regions)
            for a `composite` or `region` on page `03 Studio` (every `region` goes in the last row, `Regions`, which the assembler lowers after the composites it nests).
            The page follows the record's registry category; a set on the wrong page fails the gate.
    axes    ordered {axis: [values]}, exactly the registry's variant values. The assembler builds the FULL matrix (at most
            MAX_VARIANTS); a set over the cap is reported with a split proposal by the module's author, never split silently. A record the registry gives no variants
            has `axes` `{}` and is ONE plain COMPONENT named after the record (no set, no variant property); `build` is called once with `{}`.
    props   [{"name", "type": "TEXT"|"BOOLEAN"|"INSTANCE_SWAP"|"SLOT", "default"}]. TEXT `Label`; BOOLEAN `Has <slot>` / `Show <part>`;
            INSTANCE_SWAP default is an icon name (preferred values: every Icon/*). SLOT has no default: the container frame carries `F(..., slot="<name>")` and its
            children are the neutral default content (S5: a body that differs per caller is a native Slot, not a spacer or a variant). Optional.
    build   build(values: {axis: value}, env) -> IR frame (see rest_ir): one variant, auto layout, hug/fill, nested Icon instances.
            Every colour, stroke, weight, radius, gap and padding is a Semantic variable name; `env` turns theme constants and
            kit-states.json looks into those names (components/env.py). An abs-placed auto-layout frame with children needs fixed sizing.
    description  optional one-line text for the set: the instance panel shows it, so a deviation or anatomy note (why a state is drawn as it is) lives on the node, not only in a module docstring
                 (`library_core.described`: the COMPONENT_SET node, or the plain COMPONENT for a record with no variants).
    uses    names of other sets this one nests as instances (`X(...)` in rest_ir; ordered first by the assembler). Optional. A nested variant that has
            children is `X(name, set, variant, props={...}, sx=, sy=)`: the importer expands it from its main, and the post-pass gives its children the main's bindings
            (mirror_sources), so a module never binds an instance's child. A childless variant (the focus ring) is `X(..., abs=(x, y, w, h))`.

`check_specs(specs, registry)` fails loudly on an unknown name, wrong axis values, a duplicate or an over-cap set.
"""
import importlib
import itertools
import pathlib
import pkgutil
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[2]))
from studio_ui_gate_registry import registry_pages  # noqa: E402,F401  (the gate owns the category -> page rule)

ROWS = ("Actions", "Forms & Inputs", "Badges & Status", "Menus & Overlays", "Lists & Trees", "Navigation")
STUDIO_ROWS = ("Headers & Tabs", "Menus & Popovers", "Dialogs", "Bars & Feedback", "Cards", "Data & Code", "Agents", "Regions")
PAGE_ROWS = {"02 Core": ROWS, "03 Studio": STUDIO_ROWS}
MAX_VARIANTS = 30
SPEC_KEYS = {"name", "row", "axes", "build"}


def variant_name(values):
    return ", ".join(f"{a}={v}" for a, v in values.items())


def matrix(axes):
    """Every combination of the axes as an ordered dict, first axis varying slowest."""
    return [dict(zip(axes, combo)) for combo in itertools.product(*axes.values())]


def discover():
    """All specs of all modules in this package, with `uses` dependencies first (ties keep module then declaration order)."""
    specs = []
    for mod in sorted(m.name for m in pkgutil.iter_modules(__path__)):
        module = importlib.import_module(f"{__name__}.{mod}")
        if hasattr(module, "component_sets"):
            for s in module.component_sets():
                missing = SPEC_KEYS - s.keys()
                if missing:
                    raise ValueError(f"{mod}: spec {s.get('name')!r} lacks {sorted(missing)}")
                specs.append(dict(s, module=mod))
    by_name, ordered, state = {}, [], {}
    for s in specs:
        if s["name"] in by_name:
            raise ValueError(f"component set {s['name']} is defined twice ({by_name[s['name']]['module']}, {s['module']})")
        by_name[s["name"]] = s

    def visit(s):
        if state.get(s["name"]) == "done":
            return
        if state.get(s["name"]) == "open":
            raise ValueError(f"{s['name']}: circular `uses`")
        state[s["name"]] = "open"
        for u in s.get("uses", []):
            if u not in by_name:
                raise ValueError(f"{s['name']} uses {u}, which no module defines")
            visit(by_name[u])
        state[s["name"]] = "done"
        ordered.append(s)

    for s in specs:
        visit(s)
    return ordered


def registry_axes(registry):
    """{set name: {axis: [values in registry order]}} from the flat `Axis=Value` entries of design/component-registry.json."""
    out = {}
    for rec in registry["components"]:
        axes = {}
        for v in rec.get("variants", []):
            axis, _, value = v.partition("=")
            if value not in axes.setdefault(axis, []):
                axes[axis].append(value)
        out[rec["name"]] = axes
    return out


def check_specs(specs, registry):
    known = registry_axes(registry)
    for s in specs:
        want = known.get(s["name"])
        if want is None:
            raise ValueError(f"{s['name']} is not a record of design/component-registry.json")
        page = registry_pages(registry)[s["name"]]
        if page not in PAGE_ROWS:
            raise ValueError(f"{s['name']}: the registry puts it on page {page}, which has no component rows")
        if s["row"] not in PAGE_ROWS[page]:
            raise ValueError(f"{s['name']}: row {s['row']!r} is not one of {PAGE_ROWS[page]} (the registry puts it on page {page})")
        if {a: set(v) for a, v in s["axes"].items()} != {a: set(v) for a, v in want.items()}:
            raise ValueError(f"{s['name']}: axes {s['axes']} differ from the registry {want}")
        n = len(matrix(s["axes"]))
        if n > MAX_VARIANTS:
            raise ValueError(f"{s['name']}: {n} variants exceed the cap of {MAX_VARIANTS}; report a split proposal")
    return specs
