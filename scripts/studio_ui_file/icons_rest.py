"""Phosphor icons -> REST `COMPONENT Icon/<EnumName>` > `VECTOR` with `fillGeometry`, 256 -> 16 px.

Reads the `Icon` enum and its `phosphor_name()` map from crates/designos-app/src/icons.rs and the
SVGs from crates/designos-app/assets/icons/phosphor. `Icon::Logo` has no Phosphor file and is
reported as skipped. Arcs keep rotation and flags and scale rx, ry, x, y; the importer turns arcs
into cubics. Any malformed path, missing file or unmapped variant is fatal.

    nodes, comps, skipped = build_components(repo, section_x, section_y)
"""
import pathlib
import re

SCALE = 16 / 256
ARGC = {"M": 2, "L": 2, "H": 1, "V": 1, "C": 6, "S": 4, "Q": 4, "T": 2, "A": 7, "Z": 0}
NUM = re.compile(r"[-+]?(?:\d+\.?\d*|\.\d+)(?:[eE][-+]?\d+)?")
COLS, CELL_W, CELL_H, PAD, HEADER = 12, 112, 64, 200, 56


def fmt(v):
    s = f"{v:.4f}".rstrip("0").rstrip(".")
    return "0" if s in ("-0", "") else s


def scale_path(d, scale=SCALE):
    out, i, n = [], 0, len(d)

    def skip():
        nonlocal i
        while i < n and (d[i].isspace() or d[i] == ","):
            i += 1

    while True:
        skip()
        if i >= n:
            return " ".join(out)
        cmd = d[i]
        if cmd.upper() not in ARGC:
            raise ValueError(f"unexpected `{d[i:i + 12]}` in path")
        i += 1
        argc, first = ARGC[cmd.upper()], True
        if argc == 0:
            out.append(cmd)
            continue
        while True:
            skip()
            if i >= n or d[i].isalpha():
                if first:
                    raise ValueError(f"command {cmd} without arguments")
                break
            args = []
            for k in range(argc):
                skip()
                flag = cmd.upper() == "A" and k in (3, 4)
                m = re.compile(r"[01]").match(d, i) if flag else NUM.match(d, i)
                if not m:
                    raise ValueError(f"command {cmd} needs {argc} arguments, got {k} near `{d[i:i + 12]}`")
                i = m.end()
                v = float(m.group(0))
                args.append(fmt(v) if flag or (cmd.upper() == "A" and k == 2) else fmt(v * scale))
            letter = cmd if first else {"M": "L", "m": "l"}.get(cmd, cmd)
            out.append(letter + " ".join(args))
            first = False


def read_icon_map(repo):
    src = (pathlib.Path(repo) / "crates/designos-app/src/icons.rs").read_text(encoding="utf-8")
    enum_body = re.search(r"pub enum Icon \{(.*?)\n\}", src, re.S).group(1)
    variants = re.findall(r"^\s*([A-Z][A-Za-z0-9]*),\s*$", enum_body, re.M)
    mapping = dict(re.findall(r'Icon::(\w+)\s*=>\s*"([a-z0-9-]+)"', src))
    if len(set(variants)) != len(variants):
        raise SystemExit("duplicate Icon variants")
    missing = [v for v in variants if v != "Logo" and v not in mapping]
    extra = [v for v in mapping if v not in variants]
    if missing or extra:
        raise SystemExit(f"icons.rs map mismatch: unmapped {missing}, unknown {extra}")
    return variants, mapping


def read_svg(repo, stem):
    p = pathlib.Path(repo) / "crates/designos-app/assets/icons/phosphor" / f"{stem}.svg"
    svg = p.read_text(encoding="utf-8")
    paths = re.findall(r'<path\b[^>]*\bd="([^"]+)"', svg)
    if len(paths) != 1 or 'viewBox="0 0 256 256"' not in svg or re.search(r"<(?!path|/?svg)[a-z]", svg):
        raise SystemExit(f"{p.name}: expected one <path> on a 256 viewBox")
    return paths[0], "EVENODD" if 'fill-rule="evenodd"' in svg else "NONZERO"


def grid_xy(index):
    """Icon origin inside the Icons section: header row, then COLS columns."""
    return PAD + (index % COLS) * CELL_W, PAD + HEADER + (index // COLS) * CELL_H


def section_size(count):
    rows = -(-count // COLS)
    return COLS * CELL_W + 2 * PAD, HEADER + rows * CELL_H + 2 * PAD


def build_components(repo, section_x=0, section_y=0):
    variants, mapping = read_icon_map(repo)
    nodes, comps, skipped = [], {}, []
    for variant in variants:
        if variant == "Logo":
            skipped.append({"name": "Icon/Logo", "reason": "Icon::Logo has no Phosphor file (brand asset)"})
            continue
        d, rule = read_svg(repo, mapping[variant])
        k = len(nodes)
        x, y = grid_xy(k)
        box = {"x": section_x + x, "y": section_y + y, "width": 16, "height": 16}
        cid = f"icon:{k}"
        nodes.append({"id": cid, "type": "COMPONENT", "name": f"Icon/{variant}", "absoluteBoundingBox": box, "fills": [], "children": [
            {"id": cid + ":v", "type": "VECTOR", "name": "Glyph", "absoluteBoundingBox": dict(box),
             "fills": [{"type": "SOLID", "color": {"r": 1, "g": 1, "b": 1, "a": 1}}],
             "fillGeometry": [{"path": scale_path(d), "windingRule": rule}]}]})
        comps[cid] = {"key": f"icon-{variant}", "name": f"Icon/{variant}"}
    return nodes, comps, skipped
