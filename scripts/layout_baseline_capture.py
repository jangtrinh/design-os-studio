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


def clone_file_apfs(src: Path, dst: Path) -> None:
    dst.parent.mkdir(parents=True, exist_ok=True)
    if dst.exists():
        dst.unlink()
    if sys.platform == "darwin":
        res = subprocess.run(
            ["/bin/cp", "-c", str(src), str(dst)],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        )
        if res.returncode == 0:
            return
    shutil.copy2(src, dst)


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


def import_kit_once(
    bin_path: Path,
    kit_path: Path,
    cache_dir: Path,
    stubs_src: Path,
    dry_run: bool = False,
) -> Path:
    cache_dir.mkdir(parents=True, exist_ok=True)
    kit_source = cache_dir / f"kit-source{kit_path.suffix or '.fig'}"
    kit_designos = cache_dir / "kit.designos"
    mcp_bin = bin_path.parent / "designos-mcp"

    if dry_run:
        print(f"DRY-RUN clone-kit: cp -c {kit_path} {kit_source}")
        print(
            f"DRY-RUN import-kit: {mcp_bin} "
            f"designos_open_file({kit_source}) -> designos_save_file({kit_designos})"
        )
        return kit_designos

    if not mcp_bin.is_file():
        sys.stderr.write("designos-mcp not found next to --bin: build it with -p designos-mcp\n")
        sys.exit(1)

    clone_file_apfs(kit_path, kit_source)
    home_dir, lib_dir, tmp_dir, stubs_dir = prepare_run_dir(
        cache_dir / "import-env",
        stubs_src,
        warm_stubs=False,
    )
    env_kv = [
        f"HOME={home_dir}",
        f"PATH={stubs_dir}:/usr/bin:/bin",
        f"TMPDIR={tmp_dir}",
        f"DESIGNOS_LIBRARY={lib_dir}",
    ]

    proc = subprocess.Popen(
        ["/usr/bin/env", "-i", *env_kv, str(mcp_bin), "--workspace", str(cache_dir)],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        bufsize=1,
    )
    assert proc.stdin is not None and proc.stdout is not None and proc.stderr is not None

    def fail_mcp(reason: str) -> None:
        try:
            proc.stdin.close()
        except Exception:
            pass
        try:
            proc.terminate()
            proc.wait(timeout=5)
        except Exception:
            proc.kill()
        err_out = ""
        try:
            err_out = proc.stderr.read()
        except Exception:
            pass
        if kit_designos.exists():
            kit_designos.unlink()
        sys.stderr.write(f"ERROR: designos-mcp failed to import kit {kit_source}: {reason}\n{err_out}")
        sys.exit(1)

    def send_msg(msg: dict[str, Any]) -> None:
        try:
            proc.stdin.write(json.dumps(msg) + "\n")
            proc.stdin.flush()
        except Exception as exc:
            fail_mcp(f"stdin write failed: {exc}")

    def recv_id(expected_id: int) -> dict[str, Any]:
        while True:
            line = proc.stdout.readline()
            if not line:
                fail_mcp(f"EOF waiting for reply id={expected_id}")
            line = line.strip()
            if not line:
                continue
            try:
                msg = json.loads(line)
            except Exception:
                continue
            if msg.get("id") != expected_id:
                continue
            if "error" in msg and msg["error"] is not None:
                fail_mcp(f"JSON-RPC error on id={expected_id}: {msg['error']}")
            result = msg.get("result")
            if isinstance(result, dict) and result.get("isError"):
                fail_mcp(f"tool call error on id={expected_id}: {result}")
            return msg

    send_msg(
        {
            "jsonrpc": "2.0",
            "id": 0,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "layout-baseline-capture", "version": "1.0"},
            },
        }
    )
    recv_id(0)
    send_msg({"jsonrpc": "2.0", "method": "notifications/initialized"})

    send_msg(
        {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "designos_open_file",
                "arguments": {"path": str(kit_source)},
            },
        }
    )
    open_reply = recv_id(1)
    open_result = open_reply.get("result") or {}
    doc_info: dict[str, Any] = {}
    if isinstance(open_result.get("structuredContent"), dict):
        doc_info = open_result["structuredContent"]
    else:
        content_list = open_result.get("content") or []
        if content_list and isinstance(content_list[0], dict):
            try:
                doc_info = json.loads(content_list[0].get("text") or "{}")
            except Exception:
                doc_info = {}
    pages = doc_info.get("pages")
    page_count = len(pages) if isinstance(pages, list) else 0
    if page_count <= 1:
        fail_mcp(f"expected > 1 pages after designos_open_file, got {page_count} page(s)")

    send_msg(
        {
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "designos_save_file",
                "arguments": {"path": str(kit_designos)},
            },
        }
    )
    recv_id(2)

    try:
        proc.stdin.close()
    except Exception:
        pass
    rc = proc.wait(timeout=30)
    if rc != 0 or not kit_designos.is_file():
        err_out = proc.stderr.read() if proc.stderr else ""
        sys.stderr.write(
            f"ERROR: designos-mcp failed to import kit {kit_source} -> {kit_designos} (exit {rc}):\n{err_out}\n"
        )
        sys.exit(1)
    return kit_designos


def ensure_scratch_page_on_clone(case_clone: Path) -> None:
    if not case_clone.is_file():
        return
    try:
        raw = case_clone.read_text(encoding="utf-8")
        doc = json.loads(raw)
    except Exception:
        return
    if not isinstance(doc, dict):
        return
    pages = doc.get("pages")
    nodes = doc.get("nodes")
    if not isinstance(pages, list) or not isinstance(nodes, dict) or not pages:
        return
    for pid in pages:
        pnode = nodes.get(str(pid))
        if isinstance(pnode, dict) and pnode.get("name") == "Layout baseline scratch":
            return
    first_page = nodes.get(str(pages[0]))
    if not isinstance(first_page, dict):
        return
    scratch_id = int(doc.get("next_id", 990000))
    doc["next_id"] = scratch_id + 1
    scratch_page = dict(first_page)
    scratch_page["id"] = scratch_id
    scratch_page["source_id"] = None
    scratch_page["name"] = "Layout baseline scratch"
    scratch_page["children"] = []
    pages.append(scratch_id)
    nodes[str(scratch_id)] = scratch_page
    case_clone.write_text(json.dumps(doc), encoding="utf-8")


def build_prep_command(
    bin_path: Path,
    case_id: str,
    case_clone: Path,
    prep_kind: str,
    select_target: str | None,
    scratch_dir: Path,
    dry_run: bool,
) -> list[str]:
    if prep_kind == "comment_thread":
        cmd = [str(bin_path), str(case_clone)]
        if select_target:
            cmd.extend(["--select", str(select_target)])
        cmd.extend(["--add-comment", "Align CTA spacing to 8px grid"])
        return cmd

    if prep_kind != "check_designs_findings":
        raise ValueError(f"case {case_id}: unsupported Tier B prep {prep_kind!r}")

    spec_path = scratch_dir / f"prep-{case_id}.spec.json"
    if not dry_run:
        scratch_dir.mkdir(parents=True, exist_ok=True)
        ensure_scratch_page_on_clone(case_clone)
        spec_payload: dict[str, Any] = {
            "kind": "frame",
            "name": "Audit drift card",
            "width": 320,
            "height": 180,
            "fill": "#13161b",
            "children": [
                {
                    "kind": "text",
                    "name": "Drift label",
                    "text": "Low contrast note",
                    "font_size": 11,
                    "color": "#26272b",
                }
            ],
        }
        spec_path.write_text(json.dumps(spec_payload), encoding="utf-8")

    return [
        str(bin_path),
        str(case_clone),
        "--select",
        "name:Layout baseline scratch",
        "--apply-spec",
        str(spec_path),
    ]


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
    kit_designos: Path,
    dry_run: bool = False,
) -> bool:
    case_id = case["id"]
    tier_b = case.get("tier_b") or {}
    not_applicable_reason = tier_b.get("not_applicable")
    if not_applicable_reason:
        print(f"SKIP {case_id}: not applicable in Tier B (Tier A only: {not_applicable_reason})")
        return False
    unreachable_reason = tier_b.get("unreachable")
    if unreachable_reason:
        print(f"SKIP {case_id}: unreachable in Tier B ({unreachable_reason})")
        return False

    screenshot_path = out_dir / f"{case_id}.png"
    layout_path = out_dir / f"{case_id}.layout.json"

    raw_args = list(case.get("args", []))
    is_home = "--home" in raw_args
    select_target = tier_b.get("select")
    prep_kind = tier_b.get("prep")
    case_clone = run_dir / f"{case_id}.designos"

    if dry_run:
        lib_dir = run_dir / "lib"
        case_args = substitute_args(raw_args, lib_dir, case_clone)
        if not is_home:
            print(f"DRY-RUN [{case_id}] clone: cp -c {kit_designos} {case_clone}")
            if str(case_clone) not in case_args:
                case_args = [str(case_clone), *case_args]
        if prep_kind:
            prep_cmd = build_prep_command(
                bin_path=bin_path,
                case_id=case_id,
                case_clone=case_clone,
                prep_kind=str(prep_kind),
                select_target=str(select_target) if select_target else None,
                scratch_dir=run_dir / "tmp",
                dry_run=True,
            )
            print(f"DRY-RUN [{case_id}] prep ({prep_kind}): {' '.join(prep_cmd)}")
        if select_target and not is_home and "--select" not in case_args:
            case_args.extend(["--select", str(select_target)])
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
        print(f"DRY-RUN [{case_id}] capture: {' '.join(full_cmd)}")
        return True

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
        if seed_rc != 0 and "requires a build with the 'fixtures' feature" not in f"{seed_out}\n{seed_err}":
            sys.stderr.write(
                f"ERROR: case {case_id}: --seed-library failed (exit {seed_rc}):\n{seed_out}\n{seed_err}\n"
            )
            sys.exit(1)
        touch_hermetic_marker(lib_dir)

        case_args = substitute_args(raw_args, lib_dir, case_clone)
        if not is_home:
            clone_file_apfs(kit_designos, case_clone)
            if str(case_clone) not in case_args:
                case_args = [str(case_clone), *case_args]

        if prep_kind:
            prep_cmd = build_prep_command(
                bin_path=bin_path,
                case_id=case_id,
                case_clone=case_clone,
                prep_kind=str(prep_kind),
                select_target=str(select_target) if select_target else None,
                scratch_dir=tmp_dir,
                dry_run=False,
            )
            _, prep_rc, prep_out, prep_err = run_env_i(seed_env_kv, prep_cmd)
            if prep_rc != 0:
                sys.stderr.write(
                    f"ERROR: case {case_id}: headless prep '{prep_kind}' failed (exit {prep_rc}):\n{prep_out}\n{prep_err}\n"
                )
                sys.exit(1)

        if select_target and not is_home and "--select" not in case_args:
            case_args.extend(["--select", str(select_target)])

        env_kv = build_hermetic_env(home_dir, lib_dir, tmp_dir, stubs_dir, stub_log)

        log_lines_before = len(read_stub_log_lines(stub_log))
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
) -> None:
    if not diff_bin.is_file():
        sys.stderr.write(f"ERROR: designos-diff binary not found at {diff_bin}\n")
        sys.exit(1)

    with tempfile.TemporaryDirectory(prefix="designos-g6-") as tmp_str:
        tmp_dir = Path(tmp_str)
        for case in cases:
            case_id = case["id"]
            tier_b = case.get("tier_b") or {}
            if tier_b.get("not_applicable") or tier_b.get("unreachable"):
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


def build_colour_sheet_index(
    cases: list[dict[str, Any]],
    out_dir: Path,
) -> list[tuple[str, str, Path | None, Path | None]]:
    entries: list[tuple[str, str, Path | None, Path | None]] = []
    for case in cases:
        cid = case["id"]
        tier_b = case.get("tier_b") or {}
        if tier_b.get("not_applicable"):
            entries.append((cid, f"{cid} — Tier A only", None, None))
            continue
        png_path = out_dir / f"{cid}.png"
        layout_path = out_dir / f"{cid}.layout.json"
        if png_path.is_file():
            entries.append((cid, cid, png_path, layout_path))
    return entries


def render_colour_sheet(
    cases: list[dict[str, Any]],
    out_dir: Path,
    sheet_path: Path,
) -> None:
    index_entries = build_colour_sheet_index(cases, out_dir)
    if not any(png_path is not None for _, _, png_path, _ in index_entries):
        return

    tier_a_only_ids = [
        c["id"] for c in cases if (c.get("tier_b") or {}).get("not_applicable")
    ]
    cols = min(4, max(1, len(index_entries)))
    rows = int(math.ceil(len(index_entries) / cols))
    thumb_w = 280
    thumb_h = 175
    pad = 10
    label_h = 18
    footer_h = 24 if tier_a_only_ids else 0

    sheet_w = pad + cols * (thumb_w + pad)
    sheet_h = pad + rows * (thumb_h + label_h + pad) + footer_h
    sheet = Image.new("RGB", (sheet_w, sheet_h), (15, 15, 18))
    draw = ImageDraw.Draw(sheet)

    for idx, (cid, index_label, png_path, layout_path) in enumerate(index_entries):
        col = idx % cols
        row = idx // cols
        x0 = pad + col * (thumb_w + pad)
        y0 = pad + row * (thumb_h + label_h + pad)

        if png_path is not None and layout_path is not None:
            with Image.open(png_path) as raw_img:
                rgb_img = raw_img.convert("RGB")
                if cid == "h-home":
                    rgb_img = _mask_home_volatile_labels(rgb_img, layout_path)
                thumb = rgb_img.resize((thumb_w, thumb_h), Image.Resampling.BILINEAR)
                sheet.paste(thumb, (x0, y0 + label_h))
        else:
            draw.rectangle(
                [x0, y0 + label_h, x0 + thumb_w, y0 + label_h + thumb_h],
                fill=(24, 24, 27),
            )
            draw.text(
                (x0 + 12, y0 + label_h + (thumb_h // 2) - 6),
                "Tier A only",
                fill=(161, 161, 170),
            )

        draw.rectangle(
            [x0, y0 + label_h, x0 + thumb_w, y0 + label_h + thumb_h],
            outline=(63, 63, 70),
            width=1,
        )
        draw.text((x0 + 4, y0 + 2), index_label, fill=(228, 228, 231))

    if tier_a_only_ids:
        footer_y = sheet_h - footer_h + 4
        draw.text(
            (pad, footer_y),
            f"Index — Tier A only: {', '.join(tier_a_only_ids)}",
            fill=(161, 161, 170),
        )

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
        help="Required path to external Untitled UI kit .fig for Tier B capture",
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
    parser.add_argument(
        "--dry-run",
        action="store_true",
        default=False,
        help="Print import, clone, prep, and capture steps without spawning processes or opening windows",
    )
    return parser.parse_args(argv)


def compute_tier_counts(
    cases: list[dict[str, Any]],
    tier_key: str,
) -> tuple[int, int, int]:
    reachable = 0
    not_applicable = 0
    unreachable = 0
    for case in cases:
        target = case.get(tier_key) or {}
        if target.get("not_applicable"):
            not_applicable += 1
        elif target.get("unreachable"):
            unreachable += 1
        else:
            reachable += 1
    return reachable, not_applicable, unreachable


def all_cases_already_captured(
    cases: list[dict[str, Any]],
    out_dir: Path,
) -> bool:
    for case in cases:
        tier_b = case.get("tier_b") or {}
        if tier_b.get("not_applicable") or tier_b.get("unreachable"):
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

    already_captured = all_cases_already_captured(cases, out_dir)
    skip_capture = (
        not args.dry_run
        and compare_dir is not None
        and already_captured
        and (bin_path is None or compare_dir == out_dir)
    )

    if not skip_capture:
        if bin_path is None:
            sys.stderr.write("ERROR: --bin is required when capturing layout cases\n")
            return 2
        if kit_path is None:
            sys.stderr.write("ERROR: --kit is required when capturing layout cases\n")
            return 2
        if not args.dry_run and not kit_path.is_file():
            sys.stderr.write(f"ERROR: --kit file not found: {kit_path}\n")
            return 2
        if not args.dry_run:
            if stub_log.exists():
                stub_log.unlink()
            stub_log.write_text("", encoding="utf-8")

        t0 = time.monotonic()
        captured_count = 0
        skipped_count = 0
        with tempfile.TemporaryDirectory(prefix="designos-tierb-run-") as temp_root_str:
            temp_root = Path(temp_root_str).resolve()
            cache_dir = temp_root / "kit-cache"
            run_dir = temp_root / "run"
            kit_designos = import_kit_once(
                bin_path=bin_path,
                kit_path=kit_path,
                cache_dir=cache_dir,
                stubs_src=stubs_src,
                dry_run=args.dry_run,
            )
            for case in cases:
                if run_single_case(
                    case=case,
                    bin_path=bin_path,
                    stubs_src=stubs_src,
                    out_dir=out_dir,
                    run_dir=run_dir,
                    stub_log=stub_log,
                    only_spec=args.only_spec,
                    kit_designos=kit_designos,
                    dry_run=args.dry_run,
                ):
                    captured_count += 1
                else:
                    skipped_count += 1
            elapsed = time.monotonic() - t0
            tier_a_r, tier_a_na, tier_a_u = compute_tier_counts(cases, "tier_a")
            tier_b_r, tier_b_na, tier_b_u = compute_tier_counts(cases, "tier_b")
            tier_a_only_ids = [
                c["id"] for c in cases if (c.get("tier_b") or {}).get("not_applicable")
            ]
            print("Every case has its acknowledgement.")
            print("Every spawns case logged a stub line.")
            print(f"Every $0 in invocations.log starts with the stub dir ({run_dir / 'stubs'}/).")
            print(
                f"Summary (reachable / not applicable / unreachable): "
                f"Tier A {tier_a_r} / {tier_a_na} / {tier_a_u}, "
                f"Tier B {tier_b_r} / {tier_b_na} / {tier_b_u}"
            )
            if tier_a_only_ids:
                print(f"Tier A only ({len(tier_a_only_ids)}): {', '.join(tier_a_only_ids)}")
            print(
                f"Cases: {captured_count} captured, {skipped_count} skipped ({len(cases)} total), wall time {elapsed:.2f}s"
            )

    if args.dry_run:
        return 0

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
        )

    return 0


if __name__ == "__main__":
    sys.exit(main())
