#!/usr/bin/env python3
"""Tier B hermetic layout baseline capture, G6 canvas diff, and sheet generator."""

from __future__ import annotations

import argparse
import json
import math
import shutil
import stat
import subprocess
import sys
import tempfile
import time
from pathlib import Path
import struct
from typing import Any
import zlib

try:
    from PIL import Image, ImageDraw
except ImportError:
    for _cand in sorted(Path("/Users").glob("*/Library/Python/*/lib/python/site-packages")):
        if (_cand / "PIL").is_dir():
            sys.path.append(str(_cand))
            break
    try:
        from PIL import Image, ImageDraw
    except ImportError:
        class _StdlibImage:
            """Minimal pure-stdlib PNG reader/writer fallback when Pillow is absent in CI."""

            class Resampling:
                BILINEAR = 1

            class Quantize:
                FASTOCTREE = 1

            def __init__(self, width: int, height: int, rows: list[bytearray] | None = None, fill: tuple[int, ...] = (0, 0, 0, 255)) -> None:
                self.width = int(width)
                self.height = int(height)
                self.size = (self.width, self.height)
                r = fill[0] if len(fill) > 0 else 0
                g = fill[1] if len(fill) > 1 else 0
                b = fill[2] if len(fill) > 2 else 0
                a = fill[3] if len(fill) > 3 else 255
                if rows is not None:
                    self._rows = rows
                else:
                    px = bytes((r & 0xFF, g & 0xFF, b & 0xFF, a & 0xFF))
                    self._rows = [bytearray(px * self.width) for _ in range(self.height)]

            def __enter__(self) -> "_StdlibImage":
                return self

            def __exit__(self, exc_type: Any, exc: Any, tb: Any) -> None:
                return None

            @classmethod
            def new(cls, _mode: str, size: tuple[int, int], color: tuple[int, ...] = (0, 0, 0, 255)) -> "_StdlibImage":
                return cls(size[0], size[1], fill=color)

            @classmethod
            def open(cls, path: str | Path) -> "_StdlibImage":
                data = Path(path).read_bytes()
                if data[:8] != b"\x89PNG\r\n\x1a\n":
                    raise ValueError(f"Not a PNG file: {path}")
                pos = 8
                w = h = 0
                color_type = 6
                idat = bytearray()
                while pos + 8 <= len(data):
                    length = struct.unpack(">I", data[pos:pos + 4])[0]
                    ctype = data[pos + 4:pos + 8]
                    chunk = data[pos + 8:pos + 8 + length]
                    pos += 12 + length
                    if ctype == b"IHDR":
                        w, h, _bd, color_type, _comp, _flt, _inter = struct.unpack(">IIBBBBB", chunk[:13])
                    elif ctype == b"IDAT":
                        idat.extend(chunk)
                    elif ctype == b"IEND":
                        break
                raw = zlib.decompress(bytes(idat)) if idat else b""
                bpp = 4 if color_type == 6 else 3
                stride = w * bpp
                rows: list[bytearray] = []
                offset = 0
                for _ in range(h):
                    if offset >= len(raw):
                        rows.append(bytearray(w * 4))
                        continue
                    _filter = raw[offset]
                    scan = raw[offset + 1:offset + 1 + stride]
                    offset += 1 + stride
                    if bpp == 4:
                        rows.append(bytearray(scan.ljust(w * 4, b"\x00")))
                    else:
                        row = bytearray(w * 4)
                        for x in range(w):
                            si = x * 3
                            di = x * 4
                            if si + 3 <= len(scan):
                                row[di:di + 3] = scan[si:si + 3]
                            row[di + 3] = 255
                        rows.append(row)
                return cls(w, h, rows=rows)

            def copy(self) -> "_StdlibImage":
                return _StdlibImage(self.width, self.height, rows=[bytearray(r) for r in self._rows])

            def convert(self, _mode: str) -> "_StdlibImage":
                return self.copy()

            def quantize(self, **_kwargs: Any) -> "_StdlibImage":
                return self.copy()

            def crop(self, box: tuple[int, int, int, int]) -> "_StdlibImage":
                x0, y0, x1, y1 = [int(v) for v in box]
                cw = max(0, x1 - x0)
                ch = max(0, y1 - y0)
                rows: list[bytearray] = []
                for y in range(y0, y0 + ch):
                    if 0 <= y < self.height:
                        src = self._rows[y]
                        row = bytearray(cw * 4)
                        for x in range(cw):
                            sx = x0 + x
                            if 0 <= sx < self.width:
                                row[x * 4:(x + 1) * 4] = src[sx * 4:(sx + 1) * 4]
                        rows.append(row)
                    else:
                        rows.append(bytearray(cw * 4))
                return _StdlibImage(cw, ch, rows=rows)

            def resize(self, size: tuple[int, int], _resample: Any = None) -> "_StdlibImage":
                nw, nh = max(1, int(size[0])), max(1, int(size[1]))
                rows: list[bytearray] = []
                for y in range(nh):
                    sy = min(self.height - 1, (y * self.height) // nh) if self.height > 0 else 0
                    src = self._rows[sy] if self.height > 0 else bytearray(4)
                    row = bytearray(nw * 4)
                    for x in range(nw):
                        sx = min(self.width - 1, (x * self.width) // nw) if self.width > 0 else 0
                        row[x * 4:(x + 1) * 4] = src[sx * 4:(sx + 1) * 4]
                    rows.append(row)
                return _StdlibImage(nw, nh, rows=rows)

            def paste(self, im: "_StdlibImage", box: tuple[int, int]) -> None:
                ox, oy = int(box[0]), int(box[1])
                for y in range(im.height):
                    dy = oy + y
                    if not (0 <= dy < self.height):
                        continue
                    dst_row = self._rows[dy]
                    src_row = im._rows[y]
                    for x in range(im.width):
                        dx = ox + x
                        if 0 <= dx < self.width:
                            dst_row[dx * 4:(dx + 1) * 4] = src_row[x * 4:(x + 1) * 4]

            def save(self, path: str | Path, format: str = "PNG", **_kwargs: Any) -> None:
                del format
                def _chunk(tag: bytes, payload: bytes) -> bytes:
                    crc = zlib.crc32(tag + payload) & 0xFFFFFFFF
                    return struct.pack(">I", len(payload)) + tag + payload + struct.pack(">I", crc)

                raw = bytearray()
                for row in self._rows:
                    raw.append(0)
                    raw.extend(row)
                ihdr = struct.pack(">IIBBBBB", self.width, self.height, 8, 6, 0, 0, 0)
                idat = zlib.compress(bytes(raw), level=6)
                out = b"\x89PNG\r\n\x1a\n" + _chunk(b"IHDR", ihdr) + _chunk(b"IDAT", idat) + _chunk(b"IEND", b"")
                Path(path).write_bytes(out)

        class _StdlibDraw:
            def __init__(self, img: _StdlibImage, _mode: str | None = None) -> None:
                self._img = img

            def _set_px(self, x: int, y: int, color: tuple[int, ...]) -> None:
                if not (0 <= x < self._img.width and 0 <= y < self._img.height):
                    return
                r = color[0] if len(color) > 0 else 0
                g = color[1] if len(color) > 1 else 0
                b = color[2] if len(color) > 2 else 0
                a = color[3] if len(color) > 3 else 255
                row = self._img._rows[y]
                idx = x * 4
                row[idx] = r & 0xFF
                row[idx + 1] = g & 0xFF
                row[idx + 2] = b & 0xFF
                row[idx + 3] = a & 0xFF

            def rectangle(self, xy: list[int], fill: tuple[int, ...] | None = None, outline: tuple[int, ...] | None = None, width: int = 1) -> None:
                x0, y0, x1, y1 = [int(v) for v in xy]
                if x1 < x0:
                    x0, x1 = x1, x0
                if y1 < y0:
                    y0, y1 = y1, y0
                if fill is not None:
                    for y in range(max(0, y0), min(self._img.height, y1 + 1)):
                        for x in range(max(0, x0), min(self._img.width, x1 + 1)):
                            self._set_px(x, y, fill)
                if outline is not None:
                    for w in range(max(1, width)):
                        for x in range(max(0, x0 + w), min(self._img.width, x1 - w + 1)):
                            self._set_px(x, y0 + w, outline)
                            self._set_px(x, y1 - w, outline)
                        for y in range(max(0, y0 + w), min(self._img.height, y1 - w + 1)):
                            self._set_px(x0 + w, y, outline)
                            self._set_px(x1 - w, y, outline)

            def text(self, _xy: tuple[int, int], _text: str, fill: tuple[int, ...] | None = None) -> None:
                del fill

        class _StdlibImageDrawModule:
            ImageDraw = _StdlibDraw

            @staticmethod
            def Draw(img: _StdlibImage, mode: str | None = None) -> _StdlibDraw:
                return _StdlibDraw(img, mode)

        Image = _StdlibImage  # type: ignore[misc,assignment]
        ImageDraw = _StdlibImageDrawModule  # type: ignore[misc,assignment]

RULER_SIZE = 20.0
WINDOW_WIDTH = 1440
WINDOW_HEIGHT = 900
INFRA_SUBSTRING = "screenshot was not captured"
MAX_INFRA_ATTEMPTS = 3

REGION_COLORS: dict[str, tuple[int, int, int, int]] = {
    "top_bar": (59, 130, 246, 220),
    "title_bar": (59, 130, 246, 220),
    "tab_bar": (99, 102, 241, 220),
    "left_bar": (16, 185, 129, 220),
    "rail": (16, 185, 129, 220),
    "layers": (245, 158, 11, 220),
    "left_panel": (245, 158, 11, 220),
    "inspector": (236, 72, 153, 220),
    "right_panel": (236, 72, 153, 220),
    "bottom_bar": (14, 165, 233, 220),
    "notes": (14, 165, 233, 220),
    "canvas": (148, 163, 184, 180),
    "home": (34, 197, 94, 180),
    "present": (168, 85, 247, 180),
    "variables_full": (251, 146, 60, 180),
    "overlays": (239, 68, 68, 230),
}

LEGEND_REGIONS: list[str] = [
    "top_bar",
    "left_bar",
    "layers",
    "inspector",
    "bottom_bar",
    "canvas",
    "overlays",
]


def touch_hermetic_marker(directory: Path) -> None:
    directory.mkdir(parents=True, exist_ok=True)
    (directory / ".designos-hermetic").write_text("hermetic\n", encoding="utf-8")


def copy_stubs_with_marker(
    stubs_src: Path,
    stubs_dst: Path,
    warm_stubs: bool = False,
) -> None:
    if stubs_dst.exists():
        shutil.rmtree(stubs_dst)
    stubs_dst.mkdir(parents=True, exist_ok=True)
    copied_bins: list[Path] = []
    for item in stubs_src.iterdir():
        target = stubs_dst / item.name
        if item.is_dir():
            shutil.copytree(item, target)
        else:
            shutil.copy2(item, target)
            mode = target.stat().st_mode
            target.chmod(mode | stat.S_IXUSR | stat.S_IRUSR)
            if not item.name.startswith("."):
                copied_bins.append(target)
    touch_hermetic_marker(stubs_dst)
    if warm_stubs:
        # Warm the OS exec/AMFI vnode cache for freshly copied stub scripts using `--version`
        # (which exits 0 without touching DESIGNOS_STUB_LOG) so a fast 12-frame screenshot run
        # never closes the PTY before the stub script's shell reaches its log line.
        for bin_file in copied_bins:
            try:
                subprocess.run(
                    ["/usr/bin/env", "-i", "PATH=/usr/bin:/bin", str(bin_file), "--version"],
                    stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL,
                    timeout=5,
                    check=False,
                )
            except Exception:
                pass


def prepare_run_dir(
    run_dir: Path,
    stubs_src: Path,
    warm_stubs: bool = False,
) -> tuple[Path, Path, Path, Path]:
    run_dir = run_dir.resolve()
    home_dir = run_dir / "home"
    lib_dir = run_dir / "lib"
    tmp_dir = run_dir / "tmp"
    stubs_dir = run_dir / "stubs"

    for sub in (home_dir, lib_dir, tmp_dir):
        if sub.exists():
            shutil.rmtree(sub)
        touch_hermetic_marker(sub)

    copy_stubs_with_marker(stubs_src.resolve(), stubs_dir, warm_stubs=warm_stubs)
    return home_dir, lib_dir, tmp_dir, stubs_dir


def build_hermetic_env(
    home_dir: Path,
    lib_dir: Path,
    tmp_dir: Path,
    stubs_dir: Path,
    stub_log: Path,
) -> list[str]:
    return [
        f"HOME={home_dir}",
        f"PATH={stubs_dir}:/usr/bin:/bin",
        f"TMPDIR={tmp_dir}",
        f"DESIGNOS_LIBRARY={lib_dir}",
        f"DESIGNOS_AGENT_CLI_DIR={stubs_dir}",
        f"DESIGNOS_STUB_LOG={stub_log}",
    ]


def run_env_i(env_kv: list[str], cmd: list[str]) -> tuple[int, int, str, str]:
    proc = subprocess.Popen(
        ["/usr/bin/env", "-i", *env_kv, *cmd],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    stdout, stderr = proc.communicate()
    return proc.pid, proc.returncode, stdout, stderr


def parse_hermetic_active(active_file: Path) -> dict[str, str]:
    result: dict[str, str] = {}
    for raw_line in active_file.read_text(encoding="utf-8").splitlines():
        line = raw_line.strip()
        if not line or "=" not in line:
            continue
        key, val = line.split("=", 1)
        result[key.strip()] = val.strip()
    return result


def read_stub_log_lines(stub_log: Path) -> list[str]:
    if not stub_log.exists():
        return []
    return [
        line.strip()
        for line in stub_log.read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]


def extract_argv0(line: str) -> str:
    if "\t" in line:
        tab_fields = line.split("\t")
        if len(tab_fields) >= 2 and not tab_fields[0].startswith("/"):
            return tab_fields[1].split()[0]
    parts = line.split()
    return parts[0] if parts else ""


def verify_post_case_checks(
    case: dict[str, Any],
    pid: int,
    tmp_dir: Path,
    stubs_dir: Path,
    stub_log: Path,
    log_lines_before: int,
) -> None:
    case_id = case["id"]

    # Check 1: <run>/tmp/.designos-hermetic-active names this run's pid and stub dir.
    active_path = tmp_dir / ".designos-hermetic-active"
    if not active_path.is_file():
        raise RuntimeError(
            f"case {case_id}: check 1 failed: missing {active_path}"
        )
    active_kv = parse_hermetic_active(active_path)
    expected_pid = str(pid)
    expected_dir = str(stubs_dir.resolve())
    actual_pid = active_kv.get("pid")
    actual_dir = active_kv.get("dir")
    if actual_pid != expected_pid:
        raise RuntimeError(
            f"case {case_id}: check 1 failed: .designos-hermetic-active pid={actual_pid!r} != expected pid={expected_pid!r}"
        )
    if actual_dir != expected_dir:
        raise RuntimeError(
            f"case {case_id}: check 1 failed: .designos-hermetic-active dir={actual_dir!r} != expected dir={expected_dir!r}"
        )

    # Check 2: A "spawns": true case added at least one stub line to the log.
    lines_after = read_stub_log_lines(stub_log)
    added_lines = lines_after[log_lines_before:]
    if case.get("spawns", False) and len(added_lines) < 1:
        raise RuntimeError(
            f"case {case_id}: check 2 failed: 'spawns': true case added 0 lines to {stub_log}"
        )

    # Check 3: Every $0 in the log starts with the run's stub dir.
    stub_prefix = str(stubs_dir.resolve()) + "/"
    for idx, line in enumerate(lines_after, start=1):
        argv0 = extract_argv0(line)
        if not argv0.startswith(stub_prefix):
            raise RuntimeError(
                f"case {case_id}: check 3 failed: stub log line {idx} argv0={argv0!r} does not start with {stub_prefix!r}"
            )


def verify_layout_json(case_id: str, layout_path: Path) -> dict[str, Any]:
    if not layout_path.is_file():
        raise RuntimeError(f"case {case_id}: missing layout json {layout_path}")
    data = json.loads(layout_path.read_text(encoding="utf-8"))
    named = data.get("named")
    if not isinstance(named, dict) or len(named) == 0:
        raise RuntimeError(
            f"case {case_id}: layout json {layout_path} has empty 'named' map"
        )
    return data


def substitute_args(raw_args: list[str], lib_dir: Path, kit_path: Path | None) -> list[str]:
    resolved: list[str] = []
    for arg in raw_args:
        updated = arg.replace("{LIB}", str(lib_dir))
        if kit_path is not None:
            updated = updated.replace("{KIT}", str(kit_path))
        resolved.append(updated)
    return resolved


def run_single_case(
    case: dict[str, Any],
    bin_path: Path,
    stubs_src: Path,
    out_dir: Path,
    run_dir: Path,
    stub_log: Path,
    only_spec: str | None,
    kit_path: Path | None,
) -> bool:
    case_id = case["id"]
    if case.get("kit") and kit_path is None:
        print(f"SKIP {case_id}: no --kit")
        return False

    screenshot_path = out_dir / f"{case_id}.png"
    layout_path = out_dir / f"{case_id}.layout.json"

    for attempt in range(1, MAX_INFRA_ATTEMPTS + 1):
        home_dir, lib_dir, tmp_dir, stubs_dir = prepare_run_dir(
            run_dir,
            stubs_src,
            warm_stubs=bool(case.get("spawns")),
        )

        # Seed library in <run>/lib before entering hermetic agent CLI mode.
        seed_env_kv = [
            f"HOME={home_dir}",
            f"PATH={stubs_dir}:/usr/bin:/bin",
            f"TMPDIR={tmp_dir}",
            f"DESIGNOS_LIBRARY={lib_dir}",
        ]
        _, seed_rc, seed_out, seed_err = run_env_i(
            seed_env_kv,
            [str(bin_path), "--seed-library", str(lib_dir)],
        )
        if seed_rc != 0:
            sys.stderr.write(
                f"ERROR: case {case_id}: --seed-library failed (exit {seed_rc}):\n{seed_out}\n{seed_err}\n"
            )
            sys.exit(1)
        touch_hermetic_marker(lib_dir)

        env_kv = build_hermetic_env(home_dir, lib_dir, tmp_dir, stubs_dir, stub_log)

        log_lines_before = len(read_stub_log_lines(stub_log))
        case_args = substitute_args(case.get("args", []), lib_dir, kit_path)
        full_cmd = [
            str(bin_path),
            *case_args,
            "--size",
            f"{WINDOW_WIDTH}x{WINDOW_HEIGHT}",
            "--no-toast",
            "--screenshot",
            str(screenshot_path),
            "--layout-json",
            str(layout_path),
        ]

        pid, rc, stdout, stderr = run_env_i(env_kv, full_cmd)
        combined = f"{stdout}\n{stderr}"

        if rc == 0:
            try:
                verify_post_case_checks(
                    case=case,
                    pid=pid,
                    tmp_dir=tmp_dir,
                    stubs_dir=stubs_dir,
                    stub_log=stub_log,
                    log_lines_before=log_lines_before,
                )
                verify_layout_json(case_id, layout_path)
                if not screenshot_path.is_file():
                    raise RuntimeError(
                        f"case {case_id}: missing screenshot {screenshot_path}"
                    )
            except RuntimeError as exc:
                sys.stderr.write(f"ERROR: {exc}\n")
                sys.exit(1)
            return True

        is_infra = rc == 1 and INFRA_SUBSTRING in combined
        if is_infra:
            if attempt < MAX_INFRA_ATTEMPTS:
                continue
            if only_spec is not None and case.get("spec") == only_spec:
                sys.stderr.write(
                    f"ERROR: case {case_id} (owned by --only-spec {only_spec}) failed screenshot capture after {MAX_INFRA_ATTEMPTS} attempts\n"
                )
                sys.exit(1)
            print("INFRA: screenshot capture unavailable")
            sys.exit(3)

        sys.stderr.write(
            f"ERROR: case {case_id} exited with code {rc}:\n{stdout}\n{stderr}\n"
        )
        sys.exit(1)

    return False


def get_layout_dimensions(layout: dict[str, Any]) -> tuple[float, float]:
    dims = layout.get("size") or layout.get("window") or [WINDOW_WIDTH, WINDOW_HEIGHT]
    if isinstance(dims, list) and len(dims) >= 2 and dims[0] and dims[1]:
        return float(dims[0]), float(dims[1])
    return float(WINDOW_WIDTH), float(WINDOW_HEIGHT)


def collect_canvas_ignore_rects(
    base_layout: dict[str, Any],
    cand_layout: dict[str, Any],
    canvas_rect: list[float],
    scale_x: float,
    scale_y: float,
    crop_w: int,
    crop_h: int,
) -> list[str]:
    cx, cy, _, _ = canvas_rect
    origin_x = cx + RULER_SIZE
    origin_y = cy + RULER_SIZE

    raw_rects: list[list[float]] = []
    for layout in (cand_layout, base_layout):
        for ov in layout.get("overlays", []):
            rect = ov.get("rect")
            if isinstance(rect, list) and len(rect) == 4:
                raw_rects.append(rect)
        for entry in layout.get("named", {}).values():
            if isinstance(entry, dict) and entry.get("region") == "canvas":
                rect = entry.get("rect")
                if isinstance(rect, list) and len(rect) == 4:
                    raw_rects.append(rect)
        for entry in layout.get("unnamed", []):
            if isinstance(entry, dict) and entry.get("region") == "canvas":
                rect = entry.get("rect")
                if isinstance(rect, list) and len(rect) == 4:
                    raw_rects.append(rect)

    ignores: list[str] = []
    seen: set[str] = set()
    for rx, ry, rw, rh in raw_rects:
        ix0 = max(0, min(crop_w, int(math.floor((rx - origin_x) * scale_x))))
        iy0 = max(0, min(crop_h, int(math.floor((ry - origin_y) * scale_y))))
        ix1 = max(0, min(crop_w, int(math.ceil((rx + rw - origin_x) * scale_x))))
        iy1 = max(0, min(crop_h, int(math.ceil((ry + rh - origin_y) * scale_y))))
        iw = ix1 - ix0
        ih = iy1 - iy0
        if iw > 0 and ih > 0:
            spec = f"{ix0},{iy0},{iw},{ih}"
            if spec not in seen:
                seen.add(spec)
                ignores.append(spec)
    return ignores


def load_allow_entries(allow_path: Path) -> list[dict[str, Any]]:
    if not allow_path.is_file():
        return []
    allow_doc = json.loads(allow_path.read_text(encoding="utf-8"))
    if isinstance(allow_doc, list):
        return [e for e in allow_doc if isinstance(e, dict)]
    if isinstance(allow_doc, dict):
        items = allow_doc.get("entries") or allow_doc.get("allow") or []
        return [e for e in items if isinstance(e, dict)]
    return []


def is_canvas_diff_allowed(
    allow_entries: list[dict[str, Any]],
    case_id: str,
    only_spec: str | None,
) -> bool:
    for entry in allow_entries:
        if entry.get("case") != case_id or entry.get("region") != "canvas":
            continue
        entry_spec = entry.get("spec")
        if not entry_spec:
            continue
        if only_spec is None or entry_spec == only_spec:
            return True
    return False


def run_g6_compare(
    cases: list[dict[str, Any]],
    compare_dir: Path,
    out_dir: Path,
    diff_bin: Path,
    allow_entries: list[dict[str, Any]],
    only_spec: str | None,
    kit_path: Path | None,
) -> None:
    if not diff_bin.is_file():
        sys.stderr.write(f"ERROR: designos-diff binary not found at {diff_bin}\n")
        sys.exit(1)

    with tempfile.TemporaryDirectory(prefix="designos-g6-") as tmp_str:
        tmp_dir = Path(tmp_str)
        for case in cases:
            case_id = case["id"]
            if case.get("kit") and kit_path is None:
                continue

            base_layout_path = compare_dir / f"{case_id}.layout.json"
            cand_layout_path = out_dir / f"{case_id}.layout.json"
            base_png_path = compare_dir / f"{case_id}.png"
            cand_png_path = out_dir / f"{case_id}.png"

            if not base_layout_path.is_file() or not cand_layout_path.is_file():
                sys.stderr.write(
                    f"ERROR: G6 missing layout json for {case_id} ({base_layout_path} vs {cand_layout_path})\n"
                )
                sys.exit(1)
            if not base_png_path.is_file() or not cand_png_path.is_file():
                sys.stderr.write(
                    f"ERROR: G6 missing screenshot for {case_id} ({base_png_path} vs {cand_png_path})\n"
                )
                sys.exit(1)

            base_layout = json.loads(base_layout_path.read_text(encoding="utf-8"))
            cand_layout = json.loads(cand_layout_path.read_text(encoding="utf-8"))

            canvas_rect = cand_layout.get("regions", {}).get("canvas")
            if not canvas_rect:
                continue

            with Image.open(base_png_path) as base_img, Image.open(cand_png_path) as cand_img:
                win_w, win_h = get_layout_dimensions(cand_layout)
                scale_x = base_img.width / win_w
                scale_y = base_img.height / win_h

                cx, cy, cw, ch = [float(v) for v in canvas_rect]
                crop_x0 = int(round((cx + RULER_SIZE) * scale_x))
                crop_y0 = int(round((cy + RULER_SIZE) * scale_y))
                crop_w = max(0, int(round((cw - RULER_SIZE) * scale_x)))
                crop_h = max(0, int(round((ch - RULER_SIZE) * scale_y)))
                crop_x1 = min(base_img.width, crop_x0 + crop_w)
                crop_y1 = min(base_img.height, crop_y0 + crop_h)
                actual_w = crop_x1 - crop_x0
                actual_h = crop_y1 - crop_y0
                if actual_w <= 0 or actual_h <= 0:
                    continue

                base_crop_path = tmp_dir / f"{case_id}-base-canvas.png"
                cand_crop_path = tmp_dir / f"{case_id}-cand-canvas.png"
                base_img.crop((crop_x0, crop_y0, crop_x1, crop_y1)).save(base_crop_path)
                cand_img.crop((crop_x0, crop_y0, crop_x1, crop_y1)).save(cand_crop_path)

            ignores = collect_canvas_ignore_rects(
                base_layout=base_layout,
                cand_layout=cand_layout,
                canvas_rect=[float(v) for v in canvas_rect],
                scale_x=scale_x,
                scale_y=scale_y,
                crop_w=actual_w,
                crop_h=actual_h,
            )

            cmd = [
                str(diff_bin),
                "compare",
                str(base_crop_path),
                str(cand_crop_path),
                "--tolerance",
                "0",
                "--threshold",
                "0",
            ]
            for ig in ignores:
                cmd.extend(["--ignore", ig])

            proc = subprocess.run(cmd, capture_output=True, text=True, check=False)
            if proc.returncode != 0:
                if is_canvas_diff_allowed(allow_entries, case_id, only_spec):
                    print(f"G6 ALLOW {case_id}: canvas diff permitted by allow.json")
                    continue
                sys.stderr.write(
                    f"ERROR: G6 canvas diff failed for case {case_id}:\n{proc.stdout}\n{proc.stderr}\n"
                )
                sys.exit(1)
    print("G6: 0 mismatches.")


def _draw_single_layout_panel(
    draw: ImageDraw.ImageDraw,
    layout: dict[str, Any],
    offset_x: int,
    offset_y: int,
    scale: float,
    title: str,
) -> None:
    win_w, win_h = get_layout_dimensions(layout)
    panel_w = int(round(win_w * scale))
    panel_h = int(round(win_h * scale))

    draw.rectangle(
        [offset_x, offset_y, offset_x + panel_w, offset_y + panel_h],
        fill=(18, 18, 22, 255),
        outline=(82, 82, 91, 255),
        width=1,
    )

    regions = layout.get("regions", {})
    for region_name, rect in regions.items():
        if not isinstance(rect, list) or len(rect) != 4:
            continue
        color = REGION_COLORS.get(region_name, (161, 161, 170, 180))
        rx, ry, rw, rh = [float(v) for v in rect]
        x0 = offset_x + int(round(rx * scale))
        y0 = offset_y + int(round(ry * scale))
        x1 = offset_x + int(round((rx + rw) * scale))
        y1 = offset_y + int(round((ry + rh) * scale))
        fill_rgba = (color[0], color[1], color[2], 36)
        draw.rectangle([x0, y0, x1, y1], fill=fill_rgba, outline=color, width=2)
        draw.text((x0 + 4, y0 + 4), region_name, fill=color)

    for entry in layout.get("unnamed", []):
        if not isinstance(entry, dict):
            continue
        rect = entry.get("rect")
        if not isinstance(rect, list) or len(rect) != 4:
            continue
        region_name = str(entry.get("region", "canvas"))
        color = REGION_COLORS.get(region_name, (161, 161, 170, 180))
        rx, ry, rw, rh = [float(v) for v in rect]
        x0 = offset_x + int(round(rx * scale))
        y0 = offset_y + int(round(ry * scale))
        x1 = offset_x + int(round((rx + rw) * scale))
        y1 = offset_y + int(round((ry + rh) * scale))
        draw.rectangle([x0, y0, x1, y1], outline=(color[0], color[1], color[2], 120), width=1)

    for entry in layout.get("named", {}).values():
        if not isinstance(entry, dict):
            continue
        rect = entry.get("rect")
        if not isinstance(rect, list) or len(rect) != 4:
            continue
        region_name = str(entry.get("region", "canvas"))
        color = REGION_COLORS.get(region_name, (250, 250, 250, 220))
        rx, ry, rw, rh = [float(v) for v in rect]
        x0 = offset_x + int(round(rx * scale))
        y0 = offset_y + int(round(ry * scale))
        x1 = offset_x + int(round((rx + rw) * scale))
        y1 = offset_y + int(round((ry + rh) * scale))
        draw.rectangle([x0, y0, x1, y1], outline=color, width=1)

    for ov in layout.get("overlays", []):
        if not isinstance(ov, dict):
            continue
        rect = ov.get("rect")
        if not isinstance(rect, list) or len(rect) != 4:
            continue
        color = REGION_COLORS["overlays"]
        rx, ry, rw, rh = [float(v) for v in rect]
        x0 = offset_x + int(round(rx * scale))
        y0 = offset_y + int(round(ry * scale))
        x1 = offset_x + int(round((rx + rw) * scale))
        y1 = offset_y + int(round((ry + rh) * scale))
        draw.rectangle([x0, y0, x1, y1], outline=color, width=2)

    draw.text((offset_x + 6, offset_y - 18), title, fill=(244, 244, 245, 255))


def render_wireframe_sheet(
    layout_a_path: Path,
    layout_b_path: Path,
    out_path: Path,
    scale: float = 0.5,
) -> None:
    layout_a = json.loads(layout_a_path.read_text(encoding="utf-8"))
    layout_b = json.loads(layout_b_path.read_text(encoding="utf-8"))

    win_w, win_h = get_layout_dimensions(layout_a)
    panel_w = int(round(win_w * scale))
    panel_h = int(round(win_h * scale))
    pad = 24
    header_h = 32
    legend_h = 32

    sheet_w = max(900, pad * 3 + panel_w * 2)
    sheet_h = header_h + panel_h + legend_h + pad

    img = Image.new("RGBA", (sheet_w, sheet_h), (12, 12, 15, 255))
    draw = ImageDraw.Draw(img, "RGBA")

    _draw_single_layout_panel(
        draw,
        layout_a,
        offset_x=pad,
        offset_y=header_h,
        scale=scale,
        title=f"A: {layout_a_path.name}",
    )
    _draw_single_layout_panel(
        draw,
        layout_b,
        offset_x=pad * 2 + panel_w,
        offset_y=header_h,
        scale=scale,
        title=f"B: {layout_b_path.name}",
    )

    legend_x = pad
    legend_y = header_h + panel_h + 8
    for region_name in LEGEND_REGIONS:
        color = REGION_COLORS[region_name]
        draw.rectangle(
            [legend_x, legend_y, legend_x + 12, legend_y + 12],
            fill=color,
            outline=color,
        )
        draw.text((legend_x + 16, legend_y), region_name, fill=(228, 228, 231, 255))
        legend_x += 120

    out_path.parent.mkdir(parents=True, exist_ok=True)
    img.convert("RGB").save(out_path, format="PNG", optimize=True)


def _mask_home_volatile_labels(img: Image.Image, layout_path: Path) -> Image.Image:
    masked = img.copy()
    draw = ImageDraw.Draw(masked)

    if layout_path.is_file():
        layout = json.loads(layout_path.read_text(encoding="utf-8"))
        win_w, win_h = get_layout_dimensions(layout)
        scale_x = masked.width / win_w
        scale_y = masked.height / win_h

        # Mask version label and relative time labels in h-home.
        named = layout.get("named", {})
        for name, entry in named.items():
            if not isinstance(entry, dict):
                continue
            if "version" in name or "time" in name or "updated" in name or "modified" in name:
                rect = entry.get("rect")
                if isinstance(rect, list) and len(rect) == 4:
                    rx, ry, rw, rh = [float(v) for v in rect]
                    draw.rectangle(
                        [
                            int(round(rx * scale_x)),
                            int(round(ry * scale_y)),
                            int(round((rx + rw) * scale_x)),
                            int(round((ry + rh) * scale_y)),
                        ],
                        fill=(24, 24, 27),
                    )

        for entry in layout.get("unnamed", []):
            if not isinstance(entry, dict):
                continue
            region = entry.get("region")
            rect = entry.get("rect")
            if (
                isinstance(rect, list)
                and len(rect) == 4
                and region in ("home", "title_bar", "tab_bar", "top_bar", "canvas")
            ):
                rx, ry, rw, rh = [float(v) for v in rect]
                if rh <= 22.0 and (region in ("title_bar", "tab_bar", "top_bar") or ry > 120.0):
                    draw.rectangle(
                        [
                            int(round(rx * scale_x)),
                            int(round(ry * scale_y)),
                            int(round((rx + rw) * scale_x)),
                            int(round((ry + rh) * scale_y)),
                        ],
                        fill=(24, 24, 27),
                    )
    return masked


def render_colour_sheet(
    cases: list[dict[str, Any]],
    out_dir: Path,
    sheet_path: Path,
) -> None:
    present_cases: list[tuple[str, Path, Path]] = []
    for case in cases:
        cid = case["id"]
        png_path = out_dir / f"{cid}.png"
        layout_path = out_dir / f"{cid}.layout.json"
        if png_path.is_file():
            present_cases.append((cid, png_path, layout_path))

    if not present_cases:
        return

    cols = min(4, max(1, len(present_cases)))
    rows = int(math.ceil(len(present_cases) / cols))
    thumb_w = 280
    thumb_h = 175
    pad = 10
    label_h = 18

    sheet_w = pad + cols * (thumb_w + pad)
    sheet_h = pad + rows * (thumb_h + label_h + pad)
    sheet = Image.new("RGB", (sheet_w, sheet_h), (15, 15, 18))
    draw = ImageDraw.Draw(sheet)

    for idx, (cid, png_path, layout_path) in enumerate(present_cases):
        col = idx % cols
        row = idx // cols
        x0 = pad + col * (thumb_w + pad)
        y0 = pad + row * (thumb_h + label_h + pad)

        with Image.open(png_path) as raw_img:
            rgb_img = raw_img.convert("RGB")
            if cid == "h-home":
                rgb_img = _mask_home_volatile_labels(rgb_img, layout_path)
            thumb = rgb_img.resize((thumb_w, thumb_h), Image.Resampling.BILINEAR)
            sheet.paste(thumb, (x0, y0 + label_h))

        draw.rectangle(
            [x0, y0 + label_h, x0 + thumb_w, y0 + label_h + thumb_h],
            outline=(63, 63, 70),
            width=1,
        )
        draw.text((x0 + 4, y0 + 2), cid, fill=(228, 228, 231))

    sheet_path.parent.mkdir(parents=True, exist_ok=True)
    sheet.save(sheet_path, format="PNG", optimize=True)
    if sheet_path.stat().st_size >= 1_000_000:
        quantized = sheet.quantize(colors=128, method=Image.Quantize.FASTOCTREE)
        quantized.save(sheet_path, format="PNG", optimize=True)


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Capture and compare Tier B hermetic layout baselines."
    )
    parser.add_argument("--bin", default=None, help="Path to designos binary")
    parser.add_argument("--cases", required=True, help="Path to cases.json")
    parser.add_argument("--stubs", required=True, help="Path to hermetic stubs directory")
    parser.add_argument("--out", required=True, help="Output directory for captured artifacts")
    parser.add_argument(
        "--compare",
        default=None,
        help="Optional baseline directory for G6 canvas diff and side-by-side wireframe sheet",
    )
    parser.add_argument(
        "--only-spec",
        default=None,
        help="Active spec letter/key for owned-case INFRA failure and allow.json scoping",
    )
    parser.add_argument(
        "--kit",
        default=None,
        help="Optional path to external kit .fig for kit: true cases",
    )
    parser.add_argument(
        "--diff-bin",
        default=None,
        help="Optional path to designos-diff binary (defaults to sibling of --bin)",
    )
    parser.add_argument(
        "--allow",
        default=None,
        help="Optional path to allow.json (defaults to sibling of --cases)",
    )
    return parser.parse_args(argv)


def all_cases_already_captured(
    cases: list[dict[str, Any]],
    out_dir: Path,
    kit_path: Path | None,
) -> bool:
    for case in cases:
        if case.get("kit") and kit_path is None:
            continue
        cid = case["id"]
        if not (out_dir / f"{cid}.png").is_file():
            return False
        if not (out_dir / f"{cid}.layout.json").is_file():
            return False
    return True


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv if argv is not None else sys.argv[1:])

    bin_path = Path(args.bin).resolve() if args.bin else None
    cases_path = Path(args.cases).resolve()
    stubs_src = Path(args.stubs).resolve()
    out_dir = Path(args.out).resolve()
    compare_dir = Path(args.compare).resolve() if args.compare else None
    kit_path = Path(args.kit).resolve() if args.kit else None

    if args.diff_bin:
        diff_bin = Path(args.diff_bin).resolve()
    elif bin_path is not None:
        diff_bin = bin_path.parent / "designos-diff"
    else:
        diff_bin = Path("designos-diff")

    if args.allow:
        allow_path = Path(args.allow).resolve()
    else:
        allow_path = cases_path.parent / "allow.json"

    cases_doc = json.loads(cases_path.read_text(encoding="utf-8"))
    cases: list[dict[str, Any]] = cases_doc.get("cases", [])
    allow_entries = load_allow_entries(allow_path)

    out_dir.mkdir(parents=True, exist_ok=True)
    stub_log = out_dir / "invocations.log"

    already_captured = all_cases_already_captured(cases, out_dir, kit_path)
    skip_capture = compare_dir is not None and already_captured and (
        bin_path is None or compare_dir == out_dir
    )

    if not skip_capture:
        if bin_path is None:
            sys.stderr.write("ERROR: --bin is required when capturing layout cases\n")
            return 2
        if stub_log.exists():
            stub_log.unlink()
        stub_log.write_text("", encoding="utf-8")

        t0 = time.monotonic()
        captured_count = 0
        skipped_count = 0
        with tempfile.TemporaryDirectory(prefix="designos-tierb-run-") as temp_root_str:
            run_dir = Path(temp_root_str).resolve() / "run"
            for case in cases:
                if run_single_case(
                    case=case,
                    bin_path=bin_path,
                    stubs_src=stubs_src,
                    out_dir=out_dir,
                    run_dir=run_dir,
                    stub_log=stub_log,
                    only_spec=args.only_spec,
                    kit_path=kit_path,
                ):
                    captured_count += 1
                else:
                    skipped_count += 1
            elapsed = time.monotonic() - t0
            print("Every case has its acknowledgement.")
            print("Every spawns case logged a stub line.")
            print(f"Every $0 in invocations.log starts with the stub dir ({run_dir / 'stubs'}/).")
            print(
                f"Cases: {captured_count} captured, {skipped_count} skipped ({len(cases)} total), wall time {elapsed:.2f}s"
            )

    # Render wireframe sheet (prefer d-window-chrome when present, else first captured case).
    target_wireframe_id = "d-window-chrome"
    if not (out_dir / f"{target_wireframe_id}.layout.json").is_file():
        for case in cases:
            cid = case["id"]
            if (out_dir / f"{cid}.layout.json").is_file():
                target_wireframe_id = cid
                break

    cand_wf = out_dir / f"{target_wireframe_id}.layout.json"
    if cand_wf.is_file():
        base_wf = (
            compare_dir / f"{target_wireframe_id}.layout.json"
            if compare_dir is not None
            and (compare_dir / f"{target_wireframe_id}.layout.json").is_file()
            else cand_wf
        )
        wf_out = out_dir / f"wireframe-{target_wireframe_id}.png"
        render_wireframe_sheet(base_wf, cand_wf, wf_out)

    # Render colour contact sheet (masking h-home relative time & version label).
    render_colour_sheet(cases, out_dir, out_dir / "colour-sheet.png")

    if compare_dir is not None:
        run_g6_compare(
            cases=cases,
            compare_dir=compare_dir,
            out_dir=out_dir,
            diff_bin=diff_bin,
            allow_entries=allow_entries,
            only_spec=args.only_spec,
            kit_path=kit_path,
        )

    return 0


if __name__ == "__main__":
    sys.exit(main())
