#!/usr/bin/env python3
"""
DESIGN:OS Studio — design-system ontology parity gate

Checks the links between the component registry, the Rust kit, the theme names and the token store.
It reads files only: no `ui` CLI, no cargo build, so it runs in the design-gates job.

  P1  registry <-> Rust     every record names its `rust=` symbol; `exists` and `new-in-C` symbols resolve
                            at HEAD (a `kit/` path is the module under `widgets/kit/` when the short path is
                            not a file); a symbol a later spec creates (`new-in-D|K`) is reported until that
                            spec's PR; a symbol serves one family (first name segment) only
  P2  painted code          the chrome colour-literal ratchet and the spacing/radius/type ratchet pass
  P3  names <-> store       every theme.rs colour name is in one of four classes (G01), mapped and shared
                            names equal their store key, pending names equal their pinned full value; per
                            record, a bound `colors.*` path equals a painted constant and every painted
                            colour is bound or pending
  P4  tiers                 category is one of five tiers, `data-ds` children exist, nest no higher and do not
                            cycle, and no record uses a `colors.ramp-*` primitive
  P5  states                the stored `State=` variants equal the states C declares (C's testdata, or an
                            explicit --states-source); report-only while neither exists
  P6  flow                  each flow.json screen id is in exactly one record's `flow=` list; each artifact
                            file exists
  P7  absences              the pinned sites of an upper-case tag, a brand-filled selected state, a chrome
                            gradient and monospace chrome text equal their pins (ds_ontology_site_pins.py)
  P9  shipped names         the design store holds no kit role name (needs the token contract: --contract),
                            `Untitled UI` appears on the two credit lines only, no `UTILITY_*` constant in
                            crates/, and every registry variant axis is one of the seven Studio axes

Usage:
  python3 scripts/check_ds_ontology_parity.py
  python3 scripts/check_ds_ontology_parity.py --only P3,P4 --root <tree>
  python3 scripts/check_ds_ontology_parity.py --contract <token-contract.md> --states-source <states.json>

Exit 0 when every selected check passes, 1 when one fails, 2 on a usage or input error.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Callable, Dict, Iterable, List, Optional, Sequence, Set, Tuple

SCRIPTS_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPTS_DIR))

import design_os_self_evolve as g01  # noqa: E402
import ds_ontology_site_pins as site_pins  # noqa: E402
from non_colour_token_parity import RUST_ONLY  # noqa: E402

TIERS = ("element", "component", "composite", "region", "screen")
TIER_RANK = {tier: rank for rank, tier in enumerate(TIERS)}
# new-in-D and new-in-K report instead of failing. new-in-C resolves like exists. Any other status is a failure.
RUST_STATUS_RE = re.compile(r"^(exists|new-in-[CDK])$")
SYMBOL_RE = re.compile(r"^[A-Za-z0-9_]+(?:/[A-Za-z0-9_]+)*\.rs(?:::[a-z_][a-z0-9_]*){1,2}$")
STATE_NAMES = ("Default", "Hover", "Active", "Focus", "Disabled", "Selected")
# C's six state names (its specimen rows) in the CLI's stored spelling.
C_STATE_TO_STORED = {
    "Default": "Default",
    "Hover": "Hover",
    "Pressed": "Active",
    "Focused": "Focus",
    "Disabled": "Disabled",
    "Selected": "Selected",
}
ALLOWED_VARIANT_AXES = ("Size", "Tone", "Kind", "State", "Rail", "Bar", "Axis")
COLOUR_PREFIX = "colors."
PRIMITIVE_PREFIX = "colors.ramp-"

APP_SRC_REL = Path("crates") / "designos-app" / "src"
TESTDATA_REL = APP_SRC_REL / "widgets" / "kit" / "testdata"
STORE_SET = (
    "design/soul.md",
    "design/principles.md",
    "design/DESIGN.md",
    "design/design.tokens.json",
    "design/tokens.json",
    "design/component-registry.json",
    "design/ds.manifest.json",
    "design/specimen.html",
    "crates/designos-app/src/theme.rs",
)
CREDIT_FILES = ("design/soul.md", "design/principles.md")
CREDIT_STRING = "Untitled UI"
UTILITY_CONSTANT_RE = re.compile(r"\bUTILITY_[A-Z]+_[0-9]+\b")
TEXT_SUFFIXES = {".rs", ".md", ".json", ".toml", ".txt", ".wgsl", ".html", ".py", ".sh", ".js", ".ts", ".yml", ".yaml"}
# A kit role string belongs to one of these families; prose words in the contract's token cells
# (`light-mode`, `third-party`) do not.
KIT_ROLE_FAMILIES = ("alpha", "bg", "border", "fg", "focus", "icon", "shadow", "text", "tooltip", "utility")


class GateError(Exception):
    """An input the gate cannot read (a usage or tree problem, not a failed check)."""


@dataclass
class Record:
    name: str
    category: str
    description: str
    markup: str
    tokens: List[str]
    variants: List[str]
    fields: Optional[Dict[str, str]]

    def list_field(self, key: str) -> List[str]:
        raw = (self.fields or {}).get(key, "")
        return [part.strip() for part in raw.split(",") if part.strip()]

    @property
    def family(self) -> str:
        return self.name.split("/", 1)[0]

    def data_ds(self) -> List[str]:
        return re.findall(r'data-ds="([^"]+)"', self.markup)


@dataclass
class Result:
    check: str
    title: str
    problems: List[str] = field(default_factory=list)
    notes: List[str] = field(default_factory=list)
    summary: str = ""

    @property
    def passed(self) -> bool:
        return not self.problems


@dataclass
class Context:
    root: Path
    contract: Optional[Path] = None
    states_source: Optional[Path] = None
    _records: Optional[List[Record]] = None

    @property
    def app_src(self) -> Path:
        return self.root / APP_SRC_REL

    @property
    def records(self) -> List[Record]:
        if self._records is None:
            self._records = load_records(self.root / "design" / "component-registry.json")
        return self._records


# --------------------------------------------------------------------------- registry reading

_FIELD_SPLIT = re.compile(r";\s+(?=[a-z_]+=)")


def parse_description(text: str) -> Optional[Dict[str, str]]:
    """The `tier=...; rust=...; ...` line of a record's description, as a dict; None when there is none.

    `today=` values hold `;` and `=` (`cmdk.rs::show; panels/menus.rs:907-1085`, `hidden: x = false`), so a
    field starts only at `; ` followed by `key=`, and a field splits at its first `=`.
    """
    for line in text.splitlines():
        line = line.strip()
        if not line.startswith("tier="):
            continue
        fields: Dict[str, str] = {}
        for part in _FIELD_SPLIT.split(line):
            key, sep, value = part.partition("=")
            if not sep:
                return None
            fields[key.strip()] = value.strip()
        return fields
    return None


def load_records(path: Path) -> List[Record]:
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise GateError(f"registry file not found: {path}") from exc
    except json.JSONDecodeError as exc:
        raise GateError(f"registry file is not valid JSON: {path}: {exc}") from exc
    components = data.get("components") if isinstance(data, dict) else None
    if not isinstance(components, list):
        raise GateError(f"registry file has no components list: {path}")
    records = []
    for raw in components:
        description = str(raw.get("description", ""))
        records.append(
            Record(
                name=str(raw.get("name", "")),
                category=str(raw.get("category", "")),
                description=description,
                markup=str(raw.get("markup", "")),
                tokens=[str(t) for t in raw.get("tokensUsed", [])],
                variants=[str(v) for v in raw.get("variants", [])],
                fields=parse_description(description),
            )
        )
    return records


def read_text(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except FileNotFoundError as exc:
        raise GateError(f"file not found: {path}") from exc


def read_json(path: Path) -> object:
    try:
        return json.loads(read_text(path))
    except json.JSONDecodeError as exc:
        raise GateError(f"not valid JSON: {path}: {exc}") from exc


# --------------------------------------------------------------------------- P1

def resolve_symbol(src_root: Path, symbol: str) -> Optional[str]:
    """None when `<file>::<fn>` (or `<file>::<mod>::<fn>` for an inline module) resolves; else the reason.

    Registry rows name the kit as `kit/<file>.rs`. That module lives at `widgets/kit/<file>.rs`.
    """
    parts = symbol.split("::")
    rel, path = parts[0], parts[1:]
    file = src_root / rel
    if not file.is_file() and rel.startswith("kit/") and rel.endswith(".rs"):
        aliased = f"widgets/{rel}"
        if (src_root / aliased).is_file():
            rel = aliased
            file = src_root / aliased
    if not file.is_file():
        return f"file {rel} does not exist"
    text = file.read_text(encoding="utf-8", errors="replace")
    func = path[-1]
    if len(path) == 2:
        module = re.search(rf"^\s*(?:pub(?:\([a-z]+\))?\s+)?mod {path[0]}\s*\{{", text, re.M)
        if not module:
            return f"inline module {path[0]} not found in {rel}"
        text = text[module.end():]
    if not re.search(rf"^\s*(?:pub(?:\([a-z]+\))?\s+)?(?:const\s+)?(?:async\s+)?(?:unsafe\s+)?fn {func}\b", text, re.M):
        return f"fn {func} not found in {rel}"
    return None


def check_p1(ctx: Context) -> Result:
    result = Result("P1", "registry <-> Rust symbols")
    claims: Dict[str, Dict[str, Set[str]]] = {}
    resolved: Set[str] = set()
    pending_by_status: Dict[str, List[str]] = {}
    for rec in ctx.records:
        if rec.fields is None:
            result.problems.append(f"{rec.name}: no description line (`tier=...; rust=...; rust_status=...`)")
            continue
        symbols, status = rec.list_field("rust"), rec.fields.get("rust_status", "")
        if not symbols:
            result.problems.append(f"{rec.name}: the description names no rust= symbol")
        if not RUST_STATUS_RE.match(status):
            result.problems.append(f"{rec.name}: rust_status={status!r} is not exists, new-in-C, new-in-D or new-in-K")
            continue
        resolves = status in ("exists", "new-in-C")
        if not resolves:
            pending_by_status.setdefault(status, []).append(rec.name)
        for symbol in symbols:
            if not SYMBOL_RE.match(symbol):
                result.problems.append(f"{rec.name}: rust={symbol} is not <file>::<fn> or <file>::<mod>::<fn>")
                continue
            claims.setdefault(symbol, {}).setdefault(rec.family, set()).add(rec.name)
            if resolves:
                reason = resolve_symbol(ctx.app_src, symbol)
                if reason:
                    result.problems.append(f"{rec.name}: rust={symbol} does not resolve: {reason}")
                else:
                    resolved.add(symbol)
    for symbol, families in sorted(claims.items()):
        if len(families) > 1:
            who = "; ".join(f"{fam}: {', '.join(sorted(names))}" for fam, names in sorted(families.items()))
            result.problems.append(f"{symbol} is claimed by more than one family ({who})")
    reported = sum(len(v) for v in pending_by_status.values())
    for status, names in sorted(pending_by_status.items()):
        result.notes.append(f"report: {len(names)} record(s) with rust_status={status}: {', '.join(sorted(names))}")
    result.summary = (
        f"{len(ctx.records)} records, {len(resolved)} distinct symbols resolve, "
        f"{reported} records report-only (new-in-*, not blocking until the owning spec)"
    )
    return result


# --------------------------------------------------------------------------- P2

P2_SCRIPTS = ("check_chrome_colour_literals.py", "check_spacing_relationships.py")


def check_p2(ctx: Context) -> Result:
    result = Result("P2", "painted code (colour and spacing ratchets)")
    env = dict(os.environ, PYTHONDONTWRITEBYTECODE="1")
    procs = []
    for name in P2_SCRIPTS:
        script = ctx.root / "scripts" / name
        if not script.is_file():
            result.problems.append(f"{name} is missing under {ctx.root / 'scripts'}")
            continue
        procs.append(
            (name, subprocess.Popen([sys.executable, "-B", str(script)], cwd=ctx.root, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True))
        )
    for name, proc in procs:
        out, _ = proc.communicate()
        tail = out.strip().splitlines()
        if proc.returncode != 0:
            result.problems.append(f"{name} exited {proc.returncode}: " + " | ".join(tail[-3:]))
        elif tail:
            result.notes.append(f"{name}: {tail[-1]}")
    result.summary = f"{len(procs)} ratchet scripts run"
    return result


# --------------------------------------------------------------------------- P3

def check_p3(ctx: Context) -> Result:
    result = Result("P3", "theme names <-> store <-> records (colours)")
    theme_src = read_text(ctx.app_src / "theme.rs")
    store = read_json(ctx.root / "design" / "design.tokens.json")
    ds_colors = g01.extract_design_token_colors(store)  # type: ignore[arg-type]
    classes = g01.check_colour_classes(theme_src, ds_colors)
    result.problems.extend(f"names: {p}" for p in classes["problems"])
    counts = classes["counts"]
    result.summary = (
        f"{classes['total']} theme.rs colour names ({counts['mapped']} mapped, {counts['shared-key']} shared-key, "
        f"{counts['pending']} pending, {counts['exempt']} exempt)"
    )

    theme_values = g01.parse_theme_colour_values(theme_src)
    theme_consts = set(re.findall(r"^pub const ([A-Z0-9_]+):", theme_src, re.M))
    store_values = {COLOUR_PREFIX + key: value.upper() for key, value in ds_colors.items()}
    checked = 0
    for rec in ctx.records:
        if rec.fields is None:
            continue
        paints, pending = rec.list_field("paints"), rec.list_field("pending")
        colour_tokens = [t for t in rec.tokens if t.startswith(COLOUR_PREFIX)]
        if not paints and not pending and not colour_tokens:
            continue
        checked += 1
        for const in paints:
            if const not in theme_consts:
                result.problems.append(f"{rec.name}: paints={const} has no constant in theme.rs")
        colour_paints = [c for c in paints if c in theme_values]
        covered: Set[str] = set()
        for token in colour_tokens:
            value = store_values.get(token)
            if value is None:
                result.problems.append(f"{rec.name}: {token} is not in the token store")
                continue
            hit = [c for c in colour_paints if theme_values[c] == value]
            if not hit:
                result.problems.append(
                    f"{rec.name}: {token} ({value}) equals the value of no painted constant (paints={','.join(paints) or '-'})"
                )
            covered.update(hit)
        for const in colour_paints:
            if const not in covered and const not in pending:
                result.problems.append(
                    f"{rec.name}: paints={const} ({theme_values[const]}) is neither bound by a colors.* token nor listed in pending="
                )
        for const in pending:
            if const in theme_values:
                if const not in g01.RUST_COLOUR_PENDING_STORE:
                    result.problems.append(f"{rec.name}: pending={const} is not in the pending colour class")
                if const not in paints:
                    result.problems.append(f"{rec.name}: pending={const} is not in paints=; pending is a subset of paints")
            elif const in theme_consts and const not in RUST_ONLY:
                result.problems.append(f"{rec.name}: pending={const} is a non-colour constant with no RUST_ONLY row")
            elif const not in theme_consts:
                result.problems.append(f"{rec.name}: pending={const} has no constant in theme.rs")
    result.summary += f"; {checked} records with colours checked"
    return result


# --------------------------------------------------------------------------- P4

def _expand_child(child: str, names: Sequence[str]) -> List[str]:
    if child.endswith("/*"):
        family = child[:-1]
        return [n for n in names if n.startswith(family)]
    return [child] if child in names else []


def check_p4(ctx: Context) -> Result:
    result = Result("P4", "tiers and composition")
    by_name = {rec.name: rec for rec in ctx.records}
    names = list(by_name)
    edges: Dict[str, List[str]] = {}
    for rec in ctx.records:
        if rec.category not in TIERS:
            result.problems.append(f"{rec.name}: category {rec.category!r} is not one of {', '.join(TIERS)}")
            continue
        tier_line = (rec.fields or {}).get("tier")
        if tier_line is not None and tier_line != rec.category:
            result.problems.append(f"{rec.name}: description tier={tier_line} differs from category {rec.category}")
        for token in rec.tokens:
            if token.startswith(PRIMITIVE_PREFIX):
                result.problems.append(f"{rec.name}: {token} is a primitive; no record may use one (colors.ramp-*)")
        ds = rec.data_ds()
        if not ds or ds[0] != rec.name:
            result.problems.append(f"{rec.name}: markup root data-ds is {ds[0] if ds else 'missing'}, expected {rec.name}")
        children = ds[1:]
        composes = rec.list_field("composes")
        if composes and sorted(composes) != sorted(children):
            result.problems.append(
                f"{rec.name}: composes={','.join(composes)} differs from the markup children {','.join(children) or '-'}"
            )
        for child in children:
            members = [m for m in _expand_child(child, names) if m != rec.name]
            if not members:
                result.problems.append(f"{rec.name}: child {child} names no record")
                continue
            for member in members:
                edges.setdefault(rec.name, []).append(member)
                if by_name[member].category in TIER_RANK and TIER_RANK[by_name[member].category] > TIER_RANK[rec.category]:
                    result.problems.append(
                        f"{rec.name} ({rec.category}) composes {member} ({by_name[member].category}), a higher tier"
                    )
    for cycle in _cycles(edges):
        result.problems.append("cycle in composition: " + " -> ".join(cycle))
    result.summary = f"{len(ctx.records)} records, {sum(len(v) for v in edges.values())} composition edges"
    return result


def _cycles(edges: Dict[str, List[str]]) -> List[List[str]]:
    found: List[List[str]] = []
    state: Dict[str, int] = {}

    def visit(node: str, stack: List[str]) -> None:
        state[node] = 1
        stack.append(node)
        for nxt in edges.get(node, []):
            if state.get(nxt, 0) == 1:
                found.append(stack[stack.index(nxt):] + [nxt])
            elif state.get(nxt, 0) == 0:
                visit(nxt, stack)
        stack.pop()
        state[node] = 2

    for node in list(edges):
        if state.get(node, 0) == 0:
            visit(node, [])
    return found


# --------------------------------------------------------------------------- P5

def _stored_states(rec: Record) -> Set[str]:
    return {v for v in rec.variants if v.startswith("State=") and v.split("=", 1)[1] in STATE_NAMES}


def _declared_from_rows(source: Path, data: Dict[str, object]) -> Dict[str, Set[str]]:
    declared: Dict[str, Set[str]] = {}
    for name, rows in data["rows"].items():  # type: ignore[union-attr]
        states = set()
        for state, row in rows.items():
            if state not in C_STATE_TO_STORED:
                raise GateError(f"{source}: {name} has an unknown state {state!r}")
            if row is not None:
                states.add("State=" + C_STATE_TO_STORED[state])
        declared[name] = states
    return declared


def _declared_from_records(data: List[Dict[str, object]]) -> Dict[str, Set[str]]:
    declared: Dict[str, Set[str]] = {}
    for item in data:
        if item.get("states") is None:
            continue
        states = {"State=" + str(s).capitalize() for s in item["states"]}  # type: ignore[union-attr]
        if item.get("selected"):
            states.add("State=Selected")
        declared[str(item["name"])] = states
    return declared


def load_declared_states(ctx: Context) -> Tuple[Optional[Dict[str, Set[str]]], str]:
    """C's declared states per record: its checked-in testdata, or the file given by --states-source."""
    sources: List[Path] = []
    if ctx.states_source is not None:
        sources = sorted(ctx.states_source.glob("*.json")) if ctx.states_source.is_dir() else [ctx.states_source]
    elif (ctx.root / TESTDATA_REL).is_dir():
        sources = sorted((ctx.root / TESTDATA_REL).glob("*.json"))
    declared: Dict[str, Set[str]] = {}
    for source in sources:
        data = read_json(source)
        if isinstance(data, dict) and isinstance(data.get("rows"), dict):
            declared.update(_declared_from_rows(source, data))
        elif isinstance(data, list):
            declared.update(_declared_from_records(data))  # type: ignore[arg-type]
    if not declared:
        return None, ""
    return declared, ", ".join(p.name for p in sources)


def check_p5(ctx: Context) -> Result:
    result = Result("P5", "states")
    declared, source = load_declared_states(ctx)
    if declared is None:
        result.summary = "report-only: no states source (C's widgets/kit/testdata is not in this tree; pass --states-source)"
        return result
    by_name = {rec.name: rec for rec in ctx.records}
    for name, states in sorted(declared.items()):
        rec = by_name.get(name)
        if rec is None:
            result.problems.append(f"{name}: C declares states for it but the registry has no such record")
            continue
        stored = _stored_states(rec)
        if stored != states:
            missing, extra = sorted(states - stored), sorted(stored - states)
            result.problems.append(
                f"{name}: stored states differ from C's declared set; missing {missing or '-'}, extra {extra or '-'}"
            )
    result.summary = f"{len(declared)} records compared with {source}"
    return result


# --------------------------------------------------------------------------- P6

def check_p6(ctx: Context) -> Result:
    result = Result("P6", "flow screens <-> records")
    flow = read_json(ctx.root / "design" / "flow.json")
    screens = flow.get("screens", []) if isinstance(flow, dict) else []
    owners: Dict[str, List[str]] = {}
    for rec in ctx.records:
        for flow_id in rec.list_field("flow"):
            owners.setdefault(flow_id, []).append(rec.name)
    screen_ids = {s.get("id") for s in screens}
    for screen in screens:
        flow_id = screen.get("id")
        who = owners.get(flow_id, [])
        if len(who) != 1:
            result.problems.append(
                f"flow screen {flow_id} is in {len(who)} records' flow= lists ({', '.join(who) or 'none'}); exactly one is required"
            )
        artifact = screen.get("artifact")
        if not artifact or not (ctx.root / artifact).is_file():
            result.problems.append(f"flow screen {flow_id}: artifact {artifact} does not exist")
    for flow_id, who in sorted(owners.items()):
        if flow_id not in screen_ids:
            result.problems.append(f"{', '.join(who)}: flow={flow_id} is not a screen in flow.json")
    result.summary = f"{len(screens)} flow screens, {len(owners)} flow ids in records"
    return result


# --------------------------------------------------------------------------- P7

def check_p7(ctx: Context) -> Result:
    result = Result("P7", "absences (pinned sites)")
    actual = site_pins.scan_tree(ctx.app_src)
    result.problems.extend(site_pins.compare(actual))
    result.summary = ", ".join(f"{k} {sum(actual[k].values())} sites" for k in site_pins.SET_IDS)
    return result


# --------------------------------------------------------------------------- P9

def parse_contract_roles(contract_text: str) -> Tuple[Set[str], Set[str]]:
    """(kit role strings, constant names) from the "Untitled token" and constant columns of the contract's
    (a) and (b) tables. Only a kit-family string counts as a role: prose words in the cell do not."""
    roles: Set[str] = set()
    consts: Set[str] = set()
    cols: Optional[Tuple[int, int]] = None
    role_re = re.compile(r"\b(?:" + "|".join(KIT_ROLE_FAMILIES) + r")(?:[-_][a-z0-9]+)+\b")
    for line in contract_text.splitlines():
        if not line.startswith("|"):
            cols = None
            continue
        cells = [c.strip() for c in line.split("|")]
        if "Untitled token" in cells and any(c.startswith(("Constant", "New constant")) for c in cells):
            cols = (
                next(i for i, c in enumerate(cells) if c.startswith(("Constant", "New constant"))),
                cells.index("Untitled token"),
            )
            continue
        if cols and re.match(r"^N?\d+$", cells[1] if len(cells) > 1 else ""):
            consts.update(re.findall(r"[A-Z][A-Z0-9_]{2,}", cells[cols[0]]))
            roles.update(role_re.findall(cells[cols[1]]))
    return roles, consts


def kit_role_set(contract_text: str) -> Set[str]:
    """The kit role strings no Studio name equals: a role is dropped when it is a constant name or the
    kebab form of one (the V1 path segment of a pending name)."""
    roles, consts = parse_contract_roles(contract_text)
    shipped = set(consts) | {c.lower().replace("_", "-") for c in consts}
    return roles - shipped


def find_contract(ctx: Context) -> Optional[Path]:
    if ctx.contract is not None:
        return ctx.contract
    env = os.environ.get("DESIGN_OS_TOKEN_CONTRACT")
    if env and Path(env).is_file():
        return Path(env)
    for hit in sorted((ctx.root / "docs" / "specs").glob("**/*token-contract*.md")) if (ctx.root / "docs" / "specs").is_dir() else []:
        return hit
    return None


def _store_set_files(root: Path) -> List[Path]:
    files = [root / rel for rel in STORE_SET if (root / rel).is_file()]
    testdata = root / TESTDATA_REL
    if testdata.is_dir():
        files.extend(sorted(testdata.glob("*.json")))
    return files


def _crate_text_files(root: Path) -> Iterable[Path]:
    crates = root / "crates"
    for current, dirs, names in os.walk(crates):
        dirs[:] = [d for d in dirs if d not in ("target", ".git", "node_modules")]
        for name in names:
            if Path(name).suffix in TEXT_SUFFIXES:
                yield Path(current) / name


def check_p9(ctx: Context) -> Result:
    result = Result("P9", "shipped names (the design store holds no kit role name)")
    files = _store_set_files(ctx.root)

    contract_path = find_contract(ctx)
    if contract_path is None:
        result.notes.append("kit role scan NOT RUN (no token contract: pass --contract or set DESIGN_OS_TOKEN_CONTRACT)")
        roles: Set[str] = set()
    else:
        roles = kit_role_set(read_text(contract_path))
        result.notes.append(f"kit role scan: {len(roles)} roles from {contract_path.name}")
    token_re = re.compile(r"[A-Za-z0-9_-]+")
    for file in files:
        rel = file.relative_to(ctx.root).as_posix()
        for lineno, line in enumerate(file.read_text(encoding="utf-8", errors="replace").splitlines(), 1):
            for token in token_re.findall(line):
                if token in roles:
                    result.problems.append(f"{rel}:{lineno}: kit role name {token!r} in the store set")

    for rel in CREDIT_FILES:
        path = ctx.root / rel
        hits = [n for n, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1) if CREDIT_STRING in line] if path.is_file() else []
        if len(hits) != 1:
            result.problems.append(f"{rel}: {CREDIT_STRING!r} is on {len(hits)} lines; the credit sentence is on exactly one")
    for file in files:
        rel = file.relative_to(ctx.root).as_posix()
        if rel in CREDIT_FILES:
            continue
        for lineno, line in enumerate(file.read_text(encoding="utf-8", errors="replace").splitlines(), 1):
            if CREDIT_STRING in line:
                result.problems.append(f"{rel}:{lineno}: {CREDIT_STRING!r} belongs on the two credit lines only")

    utility = 0
    for file in _crate_text_files(ctx.root):
        for lineno, line in enumerate(file.read_text(encoding="utf-8", errors="replace").splitlines(), 1):
            for hit in UTILITY_CONSTANT_RE.findall(line):
                utility += 1
                result.problems.append(f"{file.relative_to(ctx.root).as_posix()}:{lineno}: {hit} (the UTILITY_* names are renamed)")

    for rec in ctx.records:
        for variant in rec.variants:
            axis = variant.split("=", 1)[0]
            if axis not in ALLOWED_VARIANT_AXES:
                result.problems.append(f"{rec.name}: variant {variant} uses the axis {axis}, outside {', '.join(ALLOWED_VARIANT_AXES)}")
    result.summary = f"{len(files)} store-set files, {len(roles)} kit roles, {utility} UTILITY_* hits"
    return result


# --------------------------------------------------------------------------- runner

CHECKS: Dict[str, Callable[[Context], Result]] = {
    "P1": check_p1,
    "P2": check_p2,
    "P3": check_p3,
    "P4": check_p4,
    "P5": check_p5,
    "P6": check_p6,
    "P7": check_p7,
    "P9": check_p9,
}
REGISTRY_CHECKS = {"P1", "P3", "P4", "P5", "P6", "P9"}


def run(ctx: Context, selected: Sequence[str]) -> List[Result]:
    return [CHECKS[check_id](ctx) for check_id in selected]


def parse_args(argv: Optional[Sequence[str]]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Design-system ontology parity gate: registry, Rust kit, theme names and token store agree")
    parser.add_argument("--root", type=Path, default=SCRIPTS_DIR.parent, help="repository root (default: this checkout)")
    parser.add_argument("--only", default="", help="comma-separated check ids to run (default: all)")
    parser.add_argument("--contract", type=Path, help="token contract markdown for the P9 kit role scan")
    parser.add_argument("--states-source", type=Path, help="C's declared states: a testdata JSON, a records JSON, or a directory")
    parser.add_argument("--verbose", action="store_true", help="print every note")
    return parser.parse_args(argv)


def main(argv: Optional[Sequence[str]] = None) -> int:
    args = parse_args(argv)
    selected = [c.strip().upper() for c in args.only.split(",") if c.strip()] or list(CHECKS)
    unknown = [c for c in selected if c not in CHECKS]
    if unknown:
        print(f"unknown check id(s): {', '.join(unknown)}; known: {', '.join(CHECKS)}", file=sys.stderr)
        return 2
    ctx = Context(root=args.root.resolve(), contract=args.contract, states_source=args.states_source)
    try:
        results = run(ctx, selected)
    except GateError as exc:
        print(f"ds ontology parity gate: {exc}", file=sys.stderr)
        return 2
    failed = [r for r in results if not r.passed]
    for r in results:
        print(f"{r.check} {r.title}: {'PASS' if r.passed else 'FAIL'}  {r.summary}")
        for problem in r.problems:
            print(f"    - {problem}")
        for note in r.notes:
            if args.verbose or note.startswith(("report", "kit role scan")):
                print(f"    {note}")
    if failed:
        print(f"RESULT: FAIL ({', '.join(r.check for r in failed)})")
        return 1
    print(f"RESULT: PASS ({', '.join(r.check for r in results)})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
