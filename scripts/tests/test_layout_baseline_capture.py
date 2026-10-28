"""Unit tests for scripts/layout_baseline_capture.py (Spec L / #1126 test 11 + Round 8 Rule 14).

Uses fake binaries only (never a real build, GPU, or window).
Run: python3 -m unittest discover -s scripts/tests -v
"""

from __future__ import annotations

import json
import os
import stat
import struct
import subprocess
import sys
import tempfile
import textwrap
import unittest
import zlib
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
SCRIPT = REPO_ROOT / "scripts" / "layout_baseline_capture.py"

IS_WINDOWS = os.name == "nt"

PNG_HELPER_CODE = """
import struct, zlib
from pathlib import Path

def write_solid_png(path, width, height, rgba=(20, 20, 24, 255)):
    def chunk(tag, payload):
        crc = zlib.crc32(tag + payload) & 0xFFFFFFFF
        return struct.pack(">I", len(payload)) + tag + payload + struct.pack(">I", crc)
    row = b"\\x00" + bytes(rgba) * width
    raw = row * height
    ihdr = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    out = b"\\x89PNG\\r\\n\\x1a\\n" + chunk(b"IHDR", ihdr) + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b"")
    Path(path).write_bytes(out)

def read_png_size(path):
    data = Path(path).read_bytes()
    w, h = struct.unpack(">II", data[16:24])
    return [w, h]
"""


def write_solid_png(path: Path, width: int, height: int, rgba: tuple[int, int, int, int] = (30, 32, 36, 255)) -> None:
    def chunk(tag: bytes, payload: bytes) -> bytes:
        crc = zlib.crc32(tag + payload) & 0xFFFFFFFF
        return struct.pack(">I", len(payload)) + tag + payload + struct.pack(">I", crc)

    row = b"\x00" + bytes(rgba) * width
    raw = row * height
    ihdr = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    out = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr) + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b"")
    path.write_bytes(out)


def write_executable_python(path: Path, body: str) -> Path:
    prelude = f"#!{sys.executable}\nimport sys\nsys.path[:0] = {sys.path!r}\n" + PNG_HELPER_CODE + "\n"
    script = prelude + textwrap.dedent(body).lstrip()
    path.write_text(script, encoding="utf-8")
    path.chmod(path.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
    return path


def make_stub_dir(root: Path) -> Path:
    stubs = root / "stubs"
    stubs.mkdir(parents=True, exist_ok=True)
    for name in ("claude", "codex", "agy", "grok", "gemini", "opencode"):
        p = stubs / name
        p.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
        p.chmod(0o755)
    return stubs


def make_fake_kit(root: Path) -> Path:
    kit = root / "kit.fig"
    kit.write_text("fake-kit-bytes\n", encoding="utf-8")
    return kit


def make_fake_mcp(
    bin_dir: Path,
    pages_count: int = 189,
    calls_file: Path | None = None,
    mcp_messages_file: Path | None = None,
    require_initialized: bool = True,
    force_is_error: bool = False,
    force_rpc_error: bool = False,
) -> Path:
    calls_path_str = str(calls_file) if calls_file is not None else ""
    msgs_path_str = str(mcp_messages_file) if mcp_messages_file is not None else ""
    return write_executable_python(
        bin_dir / "designos-mcp",
        f"""
        import json, sys
        from pathlib import Path

        initialized_seen = False
        open_pages = [{{"id": i + 1, "name": f"Page {{i + 1}}"}} for i in range({pages_count})]
        calls_path = Path({calls_path_str!r}) if {bool(calls_file)!r} else None
        msgs_path = Path({msgs_path_str!r}) if {bool(mcp_messages_file)!r} else None
        doc_loaded = False

        for raw in sys.stdin:
            line = raw.strip()
            if not line:
                continue
            msg = json.loads(line)
            if msgs_path is not None:
                with msgs_path.open("a", encoding="utf-8") as fh:
                    fh.write(json.dumps(msg) + "\\n")
            method = msg.get("method")
            msg_id = msg.get("id")
            if method == "initialize":
                resp = {{
                    "jsonrpc": "2.0",
                    "id": msg_id,
                    "result": {{
                        "protocolVersion": "2024-11-05",
                        "capabilities": {{"tools": {{}}}},
                        "serverInfo": {{"name": "fake-designos-mcp", "version": "0.7.0"}},
                    }},
                }}
                sys.stdout.write(json.dumps(resp) + "\\n")
                sys.stdout.flush()
                continue
            if method == "notifications/initialized":
                initialized_seen = True
                continue
            if method == "tools/call":
                if {require_initialized!r} and not initialized_seen:
                    # Simulate rmcp consuming the message sent while waiting for notifications/initialized
                    initialized_seen = True
                    continue
                params = msg.get("params") or {{}}
                name = params.get("name")
                args = params.get("arguments") or {{}}
                if {force_rpc_error!r}:
                    resp = {{
                        "jsonrpc": "2.0",
                        "id": msg_id,
                        "error": {{"code": -32603, "message": "simulated rpc error"}},
                    }}
                    sys.stdout.write(json.dumps(resp) + "\\n")
                    sys.stdout.flush()
                    continue
                if {force_is_error!r}:
                    resp = {{
                        "jsonrpc": "2.0",
                        "id": msg_id,
                        "result": {{
                            "content": [{{"type": "text", "text": "simulated tool error"}}],
                            "structuredContent": {{"code": "INTERNAL_ERROR", "message": "simulated tool error"}},
                            "isError": True,
                        }},
                    }}
                    sys.stdout.write(json.dumps(resp) + "\\n")
                    sys.stdout.flush()
                    continue
                if name == "designos_open_file":
                    doc_loaded = True
                    payload = {{
                        "name": "Untitled UI Kit",
                        "kind": "Design",
                        "active_page_id": 1,
                        "pages": open_pages,
                        "node_count": 390940 if len(open_pages) > 1 else 1,
                        "can_undo": False,
                        "can_redo": False,
                    }}
                    resp = {{
                        "jsonrpc": "2.0",
                        "id": msg_id,
                        "result": {{
                            "content": [{{"type": "text", "text": json.dumps(payload)}}],
                            "structuredContent": payload,
                            "isError": False,
                        }},
                    }}
                    sys.stdout.write(json.dumps(resp) + "\\n")
                    sys.stdout.flush()
                elif name == "designos_get_document":
                    pages = open_pages if doc_loaded else [{{"id": 1, "name": "Page 1"}}]
                    payload = {{
                        "name": "Untitled UI Kit" if doc_loaded else "Untitled",
                        "kind": "Design",
                        "active_page_id": 1,
                        "pages": pages,
                        "node_count": 390940 if len(pages) > 1 else 1,
                        "can_undo": False,
                        "can_redo": False,
                    }}
                    resp = {{
                        "jsonrpc": "2.0",
                        "id": msg_id,
                        "result": {{
                            "content": [{{"type": "text", "text": json.dumps(payload)}}],
                            "structuredContent": payload,
                            "isError": False,
                        }},
                    }}
                    sys.stdout.write(json.dumps(resp) + "\\n")
                    sys.stdout.flush()
                elif name == "designos_save_file":
                    out_path = Path(args["path"])
                    out_path.parent.mkdir(parents=True, exist_ok=True)
                    if doc_loaded and len(open_pages) > 1:
                        out_path.write_text("imported-kit-designos\\n", encoding="utf-8")
                    else:
                        out_path.write_text('{{"name":"Untitled","pages":[1],"nodes":{{"1":{{"id":1,"name":"Page 1"}}}}}}\\n', encoding="utf-8")
                    if calls_path is not None:
                        with calls_path.open("a", encoding="utf-8") as fh:
                            fh.write(json.dumps({{"kind": "import", "argv": sys.argv[1:], "out": str(out_path)}}) + "\\n")
                    payload = {{"path": str(out_path), "saved": True}}
                    resp = {{
                        "jsonrpc": "2.0",
                        "id": msg_id,
                        "result": {{
                            "content": [{{"type": "text", "text": json.dumps(payload)}}],
                            "structuredContent": payload,
                            "isError": False,
                        }},
                    }}
                    sys.stdout.write(json.dumps(resp) + "\\n")
                    sys.stdout.flush()
        """,
    )


def default_layout_json(case_id: str = "d-editor-default", named_non_empty: bool = True) -> dict:
    named = (
        {
            "toolbar-move": {
                "region": "canvas",
                "rect": [80.0, 75.0, 25.0, 15.0],
                "count": 1,
            }
        }
        if named_non_empty
        else {}
    )
    return {
        "schema": 1,
        "case": case_id,
        "size": [200, 160],
        "scale": 1.0,
        "regions": {
            "title_bar": None,
            "tab_bar": [0.0, 0.0, 200.0, 30.0],
            "rail": [0.0, 30.0, 40.0, 130.0],
            "left_panel": None,
            "canvas": [40.0, 30.0, 120.0, 100.0],
            "notes": None,
            "right_panel": [160.0, 30.0, 40.0, 130.0],
            "home": None,
            "present": None,
            "variables_full": None,
        },
        "overlays": [
            {"name": "cmdk", "rect": [70.0, 60.0, 30.0, 20.0]},
        ],
        "named": named,
        "unnamed": [
            {"region": "canvas", "rect": [90.0, 80.0, 12.0, 10.0], "sense": ["click"]},
            {"region": "right_panel", "rect": [165.0, 40.0, 20.0, 14.0], "sense": ["click"]},
        ],
        "scroll": [],
    }


@unittest.skipIf(IS_WINDOWS, "Hermetic capture requires POSIX env -i and shell stubs")
class LayoutBaselineCaptureTest(unittest.TestCase):
    def run_script(self, *args: str, extra_env: dict | None = None) -> subprocess.CompletedProcess:
        env = os.environ.copy()
        if extra_env:
            env.update(extra_env)
        return subprocess.run(
            [sys.executable, "-B", str(SCRIPT), *args],
            capture_output=True,
            text=True,
            env=env,
        )

    def test_infra_retry_then_exit_3(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve()
            stubs = make_stub_dir(root)
            kit_file = make_fake_kit(root)
            counter_file = root / "attempts.txt"
            cases_file = root / "cases.json"
            cases_file.write_text(
                json.dumps({"cases": [{"id": "d-editor-default", "spec": "D", "args": []}]}),
                encoding="utf-8",
            )
            fake_bin = write_executable_python(
                root / "fake-designos",
                f"""
                import sys
                from pathlib import Path
                if "--seed-library" in sys.argv:
                    idx = sys.argv.index("--seed-library")
                    Path(sys.argv[idx + 1]).mkdir(parents=True, exist_ok=True)
                    sys.exit(0)
                cfile = Path({str(counter_file)!r})
                n = int(cfile.read_text()) if cfile.exists() else 0
                cfile.write_text(str(n + 1))
                sys.stderr.write("error: screenshot was not captured: window timed out\\n")
                sys.exit(1)
                """,
            )
            make_fake_mcp(root)
            out_dir = root / "out"
            proc = self.run_script(
                "--bin",
                str(fake_bin),
                "--cases",
                str(cases_file),
                "--stubs",
                str(stubs),
                "--out",
                str(out_dir),
                "--kit",
                str(kit_file),
            )
            self.assertEqual(proc.returncode, 3, proc.stdout + proc.stderr)
            self.assertEqual(int(counter_file.read_text(encoding="utf-8")), 3)

    def test_owned_infra_exits_1(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve()
            stubs = make_stub_dir(root)
            kit_file = make_fake_kit(root)
            counter_file = root / "attempts.txt"
            cases_file = root / "cases.json"
            cases_file.write_text(
                json.dumps({"cases": [{"id": "d-editor-default", "spec": "D", "args": []}]}),
                encoding="utf-8",
            )
            fake_bin = write_executable_python(
                root / "fake-designos",
                f"""
                import sys
                from pathlib import Path
                if "--seed-library" in sys.argv:
                    idx = sys.argv.index("--seed-library")
                    Path(sys.argv[idx + 1]).mkdir(parents=True, exist_ok=True)
                    sys.exit(0)
                cfile = Path({str(counter_file)!r})
                n = int(cfile.read_text()) if cfile.exists() else 0
                cfile.write_text(str(n + 1))
                sys.stderr.write("screenshot was not captured\\n")
                sys.exit(1)
                """,
            )
            make_fake_mcp(root)
            out_dir = root / "out"
            proc = self.run_script(
                "--bin",
                str(fake_bin),
                "--cases",
                str(cases_file),
                "--stubs",
                str(stubs),
                "--out",
                str(out_dir),
                "--only-spec",
                "D",
                "--kit",
                str(kit_file),
            )
            self.assertEqual(proc.returncode, 1, proc.stdout + proc.stderr)
            self.assertEqual(int(counter_file.read_text(encoding="utf-8")), 3)

    def test_empty_named_map_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve()
            stubs = make_stub_dir(root)
            kit_file = make_fake_kit(root)
            cases_file = root / "cases.json"
            cases_file.write_text(
                json.dumps({"cases": [{"id": "d-editor-default", "spec": "D", "args": []}]}),
                encoding="utf-8",
            )
            empty_snap = json.dumps(default_layout_json("d-editor-default", named_non_empty=False))
            fake_bin = write_executable_python(
                root / "fake-designos",
                f"""
                import os, sys
                from pathlib import Path
                if "--seed-library" in sys.argv:
                    idx = sys.argv.index("--seed-library")
                    Path(sys.argv[idx + 1]).mkdir(parents=True, exist_ok=True)
                    sys.exit(0)
                tmpdir = Path(os.environ["TMPDIR"])
                cli_dir = Path(os.environ["DESIGNOS_AGENT_CLI_DIR"]).resolve()
                (tmpdir / ".designos-hermetic-active").write_text(f"pid={{os.getpid()}}\\ndir={{cli_dir}}\\n")
                shot = Path(sys.argv[sys.argv.index("--screenshot") + 1])
                ljson = Path(sys.argv[sys.argv.index("--layout-json") + 1])
                write_solid_png(shot, 200, 160)
                ljson.write_text({empty_snap!r})
                sys.exit(0)
                """,
            )
            make_fake_mcp(root)
            out_dir = root / "out"
            proc = self.run_script(
                "--bin",
                str(fake_bin),
                "--cases",
                str(cases_file),
                "--stubs",
                str(stubs),
                "--out",
                str(out_dir),
                "--kit",
                str(kit_file),
            )
            self.assertEqual(proc.returncode, 1, proc.stdout + proc.stderr)
            self.assertIn("named", (proc.stdout + proc.stderr).lower())

    def test_env_is_built_with_env_i_and_four_markers(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve()
            stubs = make_stub_dir(root)
            kit_file = make_fake_kit(root)
            cases_file = root / "cases.json"
            cases_file.write_text(
                json.dumps(
                    {
                        "cases": [
                            {"id": "h-home", "spec": "H", "args": ["--library", "{LIB}", "--home"]},
                            {"id": "d-window-chrome", "spec": "D", "args": []},
                        ]
                    }
                ),
                encoding="utf-8",
            )
            valid_snap = json.dumps(default_layout_json("h-home", named_non_empty=True))
            fake_bin = write_executable_python(
                root / "fake-designos",
                f"""
                import json, os, sys
                from pathlib import Path
                if "--seed-library" in sys.argv:
                    idx = sys.argv.index("--seed-library")
                    lib_dir = Path(sys.argv[idx + 1])
                    lib_dir.mkdir(parents=True, exist_ok=True)
                    (lib_dir / "seeded.marker").write_text("ok")
                    sys.exit(0)
                if "LEAKED_HOST_VAR" in os.environ:
                    sys.stderr.write("LEAKED_HOST_VAR was not stripped by env -i\\n")
                    sys.exit(2)
                required = ["HOME", "PATH", "TMPDIR", "DESIGNOS_LIBRARY", "DESIGNOS_AGENT_CLI_DIR", "DESIGNOS_STUB_LOG"]
                for k in required:
                    if k not in os.environ:
                        sys.stderr.write(f"missing {{k}}\\n")
                        sys.exit(2)
                for d_var in ["HOME", "TMPDIR", "DESIGNOS_LIBRARY", "DESIGNOS_AGENT_CLI_DIR"]:
                    d_path = Path(os.environ[d_var])
                    if not (d_path / ".designos-hermetic").is_file():
                        sys.stderr.write(f"missing .designos-hermetic in {{d_var}}={{d_path}}\\n")
                        sys.exit(2)
                expected_path = f"{{os.environ['DESIGNOS_AGENT_CLI_DIR']}}:/usr/bin:/bin"
                if os.environ["PATH"] != expected_path:
                    sys.stderr.write(f"unexpected PATH={{os.environ['PATH']}} vs {{expected_path}}\\n")
                    sys.exit(2)
                if "--library" in sys.argv:
                    lib_arg = sys.argv[sys.argv.index("--library") + 1]
                    if lib_arg != os.environ["DESIGNOS_LIBRARY"]:
                        sys.stderr.write(f"{{LIB}} not replaced with DESIGNOS_LIBRARY: {{lib_arg}}\\n")
                        sys.exit(2)
                    if not (Path(lib_arg) / "seeded.marker").is_file():
                        sys.stderr.write("library was not seeded before running case\\n")
                        sys.exit(2)
                tmpdir = Path(os.environ["TMPDIR"])
                cli_dir = Path(os.environ["DESIGNOS_AGENT_CLI_DIR"]).resolve()
                (tmpdir / ".designos-hermetic-active").write_text(f"pid={{os.getpid()}}\\ndir={{cli_dir}}\\n")
                shot = Path(sys.argv[sys.argv.index("--screenshot") + 1])
                ljson = Path(sys.argv[sys.argv.index("--layout-json") + 1])
                write_solid_png(shot, 200, 160)
                snap = json.loads({valid_snap!r})
                snap["case"] = shot.stem
                ljson.write_text(json.dumps(snap))
                sys.exit(0)
                """,
            )
            make_fake_mcp(root)
            out_dir = root / "out"
            proc = self.run_script(
                "--bin",
                str(fake_bin),
                "--cases",
                str(cases_file),
                "--stubs",
                str(stubs),
                "--out",
                str(out_dir),
                "--kit",
                str(kit_file),
                extra_env={"LEAKED_HOST_VAR": "1"},
            )
            self.assertEqual(proc.returncode, 0, proc.stdout + proc.stderr)
            self.assertTrue((out_dir / "wireframe-d-window-chrome.png").is_file())
            self.assertTrue((out_dir / "colour-sheet.png").is_file())

    def test_missing_or_foreign_hermetic_active_fails(self):
        for mode in ("missing", "foreign_pid", "foreign_dir"):
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp).resolve()
                stubs = make_stub_dir(root)
                kit_file = make_fake_kit(root)
                cases_file = root / "cases.json"
                cases_file.write_text(
                    json.dumps({"cases": [{"id": "d-editor-default", "spec": "D", "args": []}]}),
                    encoding="utf-8",
                )
                valid_snap = json.dumps(default_layout_json("d-editor-default", named_non_empty=True))
                fake_bin = write_executable_python(
                    root / "fake-designos",
                    f"""
                    import os, sys
                    from pathlib import Path
                    if "--seed-library" in sys.argv:
                        idx = sys.argv.index("--seed-library")
                        Path(sys.argv[idx + 1]).mkdir(parents=True, exist_ok=True)
                        sys.exit(0)
                    tmpdir = Path(os.environ["TMPDIR"])
                    cli_dir = Path(os.environ["DESIGNOS_AGENT_CLI_DIR"]).resolve()
                    mode = {mode!r}
                    if mode == "foreign_pid":
                        (tmpdir / ".designos-hermetic-active").write_text(f"pid=99999999\\ndir={{cli_dir}}\\n")
                    elif mode == "foreign_dir":
                        (tmpdir / ".designos-hermetic-active").write_text(f"pid={{os.getpid()}}\\ndir=/tmp/foreign-stubs\\n")
                    shot = Path(sys.argv[sys.argv.index("--screenshot") + 1])
                    ljson = Path(sys.argv[sys.argv.index("--layout-json") + 1])
                    write_solid_png(shot, 200, 160)
                    ljson.write_text({valid_snap!r})
                    sys.exit(0)
                    """,
                )
                make_fake_mcp(root)
                out_dir = root / "out"
                proc = self.run_script(
                    "--bin",
                    str(fake_bin),
                    "--cases",
                    str(cases_file),
                    "--stubs",
                    str(stubs),
                    "--out",
                    str(out_dir),
                    "--kit",
                    str(kit_file),
                )
                self.assertEqual(proc.returncode, 1, f"mode={mode}: {proc.stdout}\n{proc.stderr}")

    def test_spawns_case_with_empty_log_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve()
            stubs = make_stub_dir(root)
            kit_file = make_fake_kit(root)
            cases_file = root / "cases.json"
            cases_file.write_text(
                json.dumps(
                    {
                        "cases": [
                            {
                                "id": "i-agents-chat",
                                "spec": "I",
                                "args": ["--panel", "agents-chat"],
                                "spawns": True,
                            }
                        ]
                    }
                ),
                encoding="utf-8",
            )
            valid_snap = json.dumps(default_layout_json("i-agents-chat", named_non_empty=True))
            fake_bin = write_executable_python(
                root / "fake-designos",
                f"""
                import os, sys
                from pathlib import Path
                if "--seed-library" in sys.argv:
                    idx = sys.argv.index("--seed-library")
                    Path(sys.argv[idx + 1]).mkdir(parents=True, exist_ok=True)
                    sys.exit(0)
                tmpdir = Path(os.environ["TMPDIR"])
                cli_dir = Path(os.environ["DESIGNOS_AGENT_CLI_DIR"]).resolve()
                (tmpdir / ".designos-hermetic-active").write_text(f"pid={{os.getpid()}}\\ndir={{cli_dir}}\\n")
                shot = Path(sys.argv[sys.argv.index("--screenshot") + 1])
                ljson = Path(sys.argv[sys.argv.index("--layout-json") + 1])
                write_solid_png(shot, 200, 160)
                ljson.write_text({valid_snap!r})
                sys.exit(0)
                """,
            )
            make_fake_mcp(root)
            out_dir = root / "out"
            proc = self.run_script(
                "--bin",
                str(fake_bin),
                "--cases",
                str(cases_file),
                "--stubs",
                str(stubs),
                "--out",
                str(out_dir),
                "--kit",
                str(kit_file),
            )
            self.assertEqual(proc.returncode, 1, proc.stdout + proc.stderr)

    def test_argv0_outside_stub_dir_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve()
            stubs = make_stub_dir(root)
            kit_file = make_fake_kit(root)
            cases_file = root / "cases.json"
            cases_file.write_text(
                json.dumps(
                    {
                        "cases": [
                            {
                                "id": "i-agents-chat",
                                "spec": "I",
                                "args": ["--panel", "agents-chat"],
                                "spawns": True,
                            }
                        ]
                    }
                ),
                encoding="utf-8",
            )
            valid_snap = json.dumps(default_layout_json("i-agents-chat", named_non_empty=True))
            fake_bin = write_executable_python(
                root / "fake-designos",
                f"""
                import os, sys
                from pathlib import Path
                if "--seed-library" in sys.argv:
                    idx = sys.argv.index("--seed-library")
                    Path(sys.argv[idx + 1]).mkdir(parents=True, exist_ok=True)
                    sys.exit(0)
                tmpdir = Path(os.environ["TMPDIR"])
                cli_dir = Path(os.environ["DESIGNOS_AGENT_CLI_DIR"]).resolve()
                (tmpdir / ".designos-hermetic-active").write_text(f"pid={{os.getpid()}}\\ndir={{cli_dir}}\\n")
                with open(os.environ["DESIGNOS_STUB_LOG"], "a", encoding="utf-8") as fh:
                    fh.write("/opt/homebrew/bin/claude --mcp-config /tmp/mcp.json\\n")
                shot = Path(sys.argv[sys.argv.index("--screenshot") + 1])
                ljson = Path(sys.argv[sys.argv.index("--layout-json") + 1])
                write_solid_png(shot, 200, 160)
                ljson.write_text({valid_snap!r})
                sys.exit(0)
                """,
            )
            make_fake_mcp(root)
            out_dir = root / "out"
            proc = self.run_script(
                "--bin",
                str(fake_bin),
                "--cases",
                str(cases_file),
                "--stubs",
                str(stubs),
                "--out",
                str(out_dir),
                "--kit",
                str(kit_file),
            )
            self.assertEqual(proc.returncode, 1, proc.stdout + proc.stderr)

    def test_missing_kit_exits_nonzero(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve()
            stubs = make_stub_dir(root)
            cases_file = root / "cases.json"
            cases_file.write_text(
                json.dumps({"cases": [{"id": "d-editor-default", "spec": "D", "args": []}]}),
                encoding="utf-8",
            )
            fake_bin = write_executable_python(
                root / "fake-designos",
                """
                import sys
                sys.exit(0)
                """,
            )
            out_dir = root / "out"
            proc = self.run_script(
                "--bin",
                str(fake_bin),
                "--cases",
                str(cases_file),
                "--stubs",
                str(stubs),
                "--out",
                str(out_dir),
            )
            self.assertEqual(proc.returncode, 2, proc.stdout + proc.stderr)
            self.assertIn("--kit", proc.stderr)

    def test_kit_imported_once_and_cloned_per_case_with_headless_prep(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve()
            stubs = make_stub_dir(root)
            kit_file = make_fake_kit(root)
            calls_file = root / "calls.jsonl"
            cases_file = root / "cases.json"
            cases_file.write_text(
                json.dumps(
                    {
                        "cases": [
                            {
                                "id": "d-editor-selected",
                                "spec": "D",
                                "args": ["--rulers"],
                                "tier_b": {"select": "id:49152"},
                            },
                            {
                                "id": "j-check-designs",
                                "spec": "J",
                                "args": ["--panel", "check-designs"],
                                "tier_b": {"select": "id:49152", "prep": "check_designs_findings"},
                            },
                            {
                                "id": "x-board",
                                "spec": "X",
                                "args": [],
                                "tier_b": {
                                    "select": None,
                                    "prep": "blank_board",
                                    "unreachable": "Board document kind is unreachable via CLI",
                                },
                            },
                        ]
                    }
                ),
                encoding="utf-8",
            )
            valid_snap = json.dumps(default_layout_json("d-editor-selected", named_non_empty=True))
            fake_bin = write_executable_python(
                root / "fake-designos",
                f"""
                import json, os, sys
                from pathlib import Path
                calls_path = Path({str(calls_file)!r})
                if "--seed-library" in sys.argv:
                    idx = sys.argv.index("--seed-library")
                    Path(sys.argv[idx + 1]).mkdir(parents=True, exist_ok=True)
                    sys.exit(0)
                if "--apply-spec" in sys.argv:
                    target_clone = Path(sys.argv[1])
                    before = target_clone.read_text(encoding="utf-8")
                    target_clone.write_text(before + "prep:check_designs_findings\\n", encoding="utf-8")
                    with calls_path.open("a", encoding="utf-8") as fh:
                        fh.write(json.dumps({{
                            "kind": "prep",
                            "argv": sys.argv[1:],
                            "target": str(target_clone),
                            "before": before,
                        }}) + "\\n")
                    print(json.dumps({{"root_node_id": 990001, "saved": True}}))
                    sys.exit(0)
                tmpdir = Path(os.environ["TMPDIR"])
                cli_dir = Path(os.environ["DESIGNOS_AGENT_CLI_DIR"]).resolve()
                (tmpdir / ".designos-hermetic-active").write_text(f"pid={{os.getpid()}}\\ndir={{cli_dir}}\\n")
                target_clone = Path(sys.argv[1])
                shot = Path(sys.argv[sys.argv.index("--screenshot") + 1])
                ljson = Path(sys.argv[sys.argv.index("--layout-json") + 1])
                with calls_path.open("a", encoding="utf-8") as fh:
                    fh.write(json.dumps({{
                        "kind": "capture",
                        "case": shot.stem,
                        "argv": sys.argv[1:],
                        "target": str(target_clone),
                        "clone_content": target_clone.read_text(encoding="utf-8") if target_clone.is_file() else "",
                    }}) + "\\n")
                write_solid_png(shot, 200, 160)
                snap = json.loads({valid_snap!r})
                snap["case"] = shot.stem
                ljson.write_text(json.dumps(snap), encoding="utf-8")
                sys.exit(0)
                """,
            )
            make_fake_mcp(root, calls_file=calls_file)
            out_dir = root / "out"
            proc = self.run_script(
                "--bin",
                str(fake_bin),
                "--cases",
                str(cases_file),
                "--stubs",
                str(stubs),
                "--out",
                str(out_dir),
                "--kit",
                str(kit_file),
            )
            self.assertEqual(proc.returncode, 0, proc.stdout + proc.stderr)
            self.assertIn("SKIP x-board: unreachable in Tier B", proc.stdout)
            calls = [json.loads(line) for line in calls_file.read_text(encoding="utf-8").splitlines() if line.strip()]
            imports = [c for c in calls if c["kind"] == "import"]
            preps = [c for c in calls if c["kind"] == "prep"]
            captures = [c for c in calls if c["kind"] == "capture"]
            self.assertEqual(len(imports), 1, f"expected 1 kit import, got {imports}")
            self.assertEqual(len(preps), 1, f"expected 1 headless prep call, got {preps}")
            self.assertEqual(len(captures), 2, f"expected 2 captures, got {captures}")
            # Prep targeted j-check-designs's per-case clone before capture opened it:
            self.assertEqual(preps[0]["target"], captures[1]["target"])
            self.assertIn("imported-kit-designos", captures[0]["clone_content"])
            self.assertIn("imported-kit-designos", captures[1]["clone_content"])
            self.assertIn("prep:check_designs_findings", captures[1]["clone_content"])
            self.assertNotIn("prep:check_designs_findings", captures[0]["clone_content"])
            # Each capture received --select id:49152:
            for cap in captures:
                argv = cap["argv"]
                self.assertIn("--select", argv)
                self.assertEqual(argv[argv.index("--select") + 1], "id:49152")
            # Original kit.fig was never modified:
            self.assertEqual(kit_file.read_text(encoding="utf-8"), "fake-kit-bytes\n")

    def test_dry_run_prints_import_clone_prep_and_capture_without_spawning(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve()
            stubs = make_stub_dir(root)
            kit_file = make_fake_kit(root)
            cases_file = root / "cases.json"
            cases_file.write_text(
                json.dumps(
                    {
                        "cases": [
                            {
                                "id": "d-editor-selected",
                                "spec": "D",
                                "args": ["--rulers"],
                                "tier_b": {"select": "id:49152"},
                            },
                            {
                                "id": "j-check-designs",
                                "spec": "J",
                                "args": ["--panel", "check-designs"],
                                "tier_b": {"select": "id:49152", "prep": "check_designs_findings"},
                            },
                            {
                                "id": "x-board",
                                "spec": "X",
                                "args": [],
                                "tier_b": {
                                    "select": None,
                                    "prep": "blank_board",
                                    "unreachable": "Board document kind is unreachable via CLI",
                                },
                            },
                        ]
                    }
                ),
                encoding="utf-8",
            )
            fake_bin = write_executable_python(
                root / "fake-designos",
                """
                import sys
                sys.stderr.write("ERROR: fake-designos must not be executed during --dry-run\\n")
                sys.exit(99)
                """,
            )
            out_dir = root / "out"
            proc = self.run_script(
                "--dry-run",
                "--bin",
                str(fake_bin),
                "--cases",
                str(cases_file),
                "--stubs",
                str(stubs),
                "--out",
                str(out_dir),
                "--kit",
                str(kit_file),
            )
            self.assertEqual(proc.returncode, 0, proc.stdout + proc.stderr)
            self.assertIn("DRY-RUN clone-kit:", proc.stdout)
            self.assertIn("DRY-RUN import-kit:", proc.stdout)
            self.assertIn("DRY-RUN [d-editor-selected] clone: cp -c", proc.stdout)
            self.assertIn("DRY-RUN [j-check-designs] prep (check_designs_findings):", proc.stdout)
            self.assertIn("DRY-RUN [j-check-designs] capture:", proc.stdout)
            self.assertIn("SKIP x-board: unreachable in Tier B", proc.stdout)

    def test_g6_builds_crop_and_ignore_list_from_fixture_layout_json(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve()
            stubs = make_stub_dir(root)
            cases_file = root / "cases.json"
            cases_file.write_text(
                json.dumps({"cases": [{"id": "d-editor-default", "spec": "D", "args": []}]}),
                encoding="utf-8",
            )
            allow_file = root / "allow.json"
            allow_file.write_text(json.dumps({"allow": []}), encoding="utf-8")

            base_dir = root / "base"
            cand_dir = root / "cand"
            base_dir.mkdir()
            cand_dir.mkdir()

            fixture_snap_path = (
                Path(__file__).resolve().parent
                / "fixtures"
                / "layout_baseline"
                / "d-editor-default.layout.json"
            )
            snap_text = fixture_snap_path.read_text(encoding="utf-8")
            for d in (base_dir, cand_dir):
                write_solid_png(d / "d-editor-default.png", 200, 160)
                (d / "d-editor-default.layout.json").write_text(snap_text, encoding="utf-8")

            recorded_file = root / "diff-call.json"
            fake_diff = write_executable_python(
                root / "designos-diff",
                f"""
                import json, sys
                from pathlib import Path
                args = sys.argv[1:]
                # args: ["compare", img_a, img_b, "--tolerance", "0", "--threshold", "0", "--ignore", ...]
                size_a = read_png_size(args[1])
                size_b = read_png_size(args[2])
                ignores = [args[i + 1] for i, a in enumerate(args) if a == "--ignore"]
                Path({str(recorded_file)!r}).write_text(json.dumps({{
                    "args": args,
                    "size_a": size_a,
                    "size_b": size_b,
                    "ignores": ignores,
                }}))
                print("PASS ratio=0.000000 threshold=0.000000 mismatched=0 aa=0 ignored=0 ssim=1.0000 size=100x80")
                sys.exit(0)
                """,
            )
            proc = self.run_script(
                "--cases",
                str(cases_file),
                "--stubs",
                str(stubs),
                "--out",
                str(cand_dir),
                "--compare",
                str(base_dir),
                "--diff-bin",
                str(fake_diff),
                "--allow",
                str(allow_file),
            )
            self.assertEqual(proc.returncode, 0, proc.stdout + proc.stderr)
            recorded = json.loads(recorded_file.read_text(encoding="utf-8"))
            # Canvas [40, 30, 120, 100] minus 20pt rulers top and left -> [60, 50, 100, 80] -> 100x80
            self.assertEqual(recorded["size_a"], [100, 80])
            self.assertEqual(recorded["size_b"], [100, 80])
            self.assertIn("--tolerance", recorded["args"])
            self.assertIn("--threshold", recorded["args"])
            # Overlay [70, 60, 30, 20] -> relative to (60, 50) -> 10,10,30,20
            self.assertIn("10,10,30,20", recorded["ignores"])
            # Named in-canvas widget [80, 75, 25, 15] -> relative to (60, 50) -> 20,25,25,15
            self.assertIn("20,25,25,15", recorded["ignores"])
            # Unnamed in-canvas widget [90, 80, 12, 10] -> relative to (60, 50) -> 30,30,12,10
            self.assertIn("30,30,12,10", recorded["ignores"])

    def test_round9_rulings_dry_run_and_summary_counts(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve()
            stubs = make_stub_dir(root)
            out_dir = root / "out"
            kit_file = root / "Untitled.fig"
            kit_file.write_bytes(b"fig-kiwi")
            fake_bin = write_executable_python(
                root / "fake-designos",
                """
                import sys
                sys.exit(99)
                """,
            )
            real_cases = (
                Path(__file__).resolve().parents[2]
                / "crates"
                / "designos-app"
                / "tests"
                / "layout-baseline"
                / "cases.json"
            )
            proc = self.run_script(
                "--dry-run",
                "--bin",
                str(fake_bin),
                "--cases",
                str(real_cases),
                "--stubs",
                str(stubs),
                "--out",
                str(out_dir),
                "--kit",
                str(kit_file),
            )
            self.assertEqual(proc.returncode, 0, proc.stdout + proc.stderr)
            self.assertIn("SKIP x-board: not applicable in Tier B (Tier A only:", proc.stdout)
            self.assertIn("SKIP x-deck: not applicable in Tier B (Tier A only:", proc.stdout)
            self.assertIn("Tier A only (2): x-board, x-deck", proc.stdout)

            self.assertNotIn("DRY-RUN [f-prototype-tab] prep", proc.stdout)
            f_cap = [
                line
                for line in proc.stdout.splitlines()
                if "DRY-RUN [f-prototype-tab] capture:" in line
            ]
            self.assertEqual(len(f_cap), 1)
            self.assertIn("--select id:77742", f_cap[0])
            self.assertIn("--panel interaction", f_cap[0])

            lines = proc.stdout.splitlines()
            j_prep_idx = next(
                (i for i, l in enumerate(lines) if "DRY-RUN [j-comments] prep (comment_thread):" in l),
                -1,
            )
            j_cap_idx = next(
                (i for i, l in enumerate(lines) if "DRY-RUN [j-comments] capture:" in l),
                -1,
            )
            self.assertGreaterEqual(j_prep_idx, 0)
            self.assertGreater(j_cap_idx, j_prep_idx)
            self.assertIn("--add-comment", lines[j_prep_idx])
            self.assertIn("--panel comments", lines[j_cap_idx])

            self.assertNotIn("DRY-RUN [k-libraries] prep", proc.stdout)
            self.assertIn("DRY-RUN [k-libraries] capture:", proc.stdout)

            self.assertIn("Tier A 76 / 0 / 0", proc.stdout)
            self.assertIn("Tier B 74 / 2 / 0", proc.stdout)

            import importlib.util

            spec = importlib.util.spec_from_file_location("layout_baseline_capture", SCRIPT)
            assert spec is not None and spec.loader is not None
            mod = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(mod)
            cases_list = json.loads(real_cases.read_text(encoding="utf-8"))["cases"]
            index_entries = mod.build_colour_sheet_index(cases_list, out_dir)
            labels_by_id = {cid: label for cid, label, _, _ in index_entries}
            self.assertEqual(labels_by_id.get("x-board"), "x-board — Tier A only")
            self.assertEqual(labels_by_id.get("x-deck"), "x-deck — Tier A only")

    def test_mcp_import_sends_initialized_before_tools_call_and_checks_is_error(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve()
            stubs = make_stub_dir(root)
            kit_file = make_fake_kit(root)
            msgs_file = root / "mcp_messages.jsonl"
            seen_clone_file = root / "seen_clone.txt"
            cases_file = root / "cases.json"
            cases_file.write_text(
                json.dumps({"cases": [{"id": "d-editor-default", "spec": "D", "args": []}]}),
                encoding="utf-8",
            )
            valid_snap = json.dumps(default_layout_json("d-editor-default", named_non_empty=True))
            fake_bin = write_executable_python(
                root / "fake-designos",
                f"""
                import os, sys
                from pathlib import Path
                if "--seed-library" in sys.argv:
                    idx = sys.argv.index("--seed-library")
                    Path(sys.argv[idx + 1]).mkdir(parents=True, exist_ok=True)
                    sys.exit(0)
                tmpdir = Path(os.environ["TMPDIR"])
                cli_dir = Path(os.environ["DESIGNOS_AGENT_CLI_DIR"]).resolve()
                (tmpdir / ".designos-hermetic-active").write_text(f"pid={{os.getpid()}}\\ndir={{cli_dir}}\\n")
                Path({str(seen_clone_file)!r}).write_text(Path(sys.argv[1]).read_text(encoding="utf-8"), encoding="utf-8")
                shot = Path(sys.argv[sys.argv.index("--screenshot") + 1])
                ljson = Path(sys.argv[sys.argv.index("--layout-json") + 1])
                write_solid_png(shot, 200, 160)
                ljson.write_text({valid_snap!r})
                sys.exit(0)
                """,
            )
            make_fake_mcp(root, pages_count=189, mcp_messages_file=msgs_file, require_initialized=True)
            out_dir = root / "out"
            proc = self.run_script(
                "--bin",
                str(fake_bin),
                "--cases",
                str(cases_file),
                "--stubs",
                str(stubs),
                "--out",
                str(out_dir),
                "--kit",
                str(kit_file),
            )
            self.assertEqual(proc.returncode, 0, proc.stdout + proc.stderr)
            msgs = [
                json.loads(line)
                for line in msgs_file.read_text(encoding="utf-8").splitlines()
                if line.strip()
            ]
            methods = [m.get("method") for m in msgs]
            self.assertGreaterEqual(len(methods), 4, f"expected >= 4 MCP messages, got {methods}")
            self.assertEqual(methods[0], "initialize")
            self.assertEqual(msgs[0].get("id"), 0)
            self.assertEqual(methods[1], "notifications/initialized")
            self.assertEqual(methods[2], "tools/call")
            self.assertEqual(msgs[2]["params"]["name"], "designos_open_file")
            self.assertEqual(
                seen_clone_file.read_text(encoding="utf-8"),
                "imported-kit-designos\n",
            )

        # Also verify isError=True or JSON-RPC error exits 1
        for err_mode in ("is_error", "rpc_error"):
            with self.subTest(err_mode=err_mode), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp).resolve()
                stubs = make_stub_dir(root)
                kit_file = make_fake_kit(root)
                cases_file = root / "cases.json"
                cases_file.write_text(
                    json.dumps({"cases": [{"id": "d-editor-default", "spec": "D", "args": []}]}),
                    encoding="utf-8",
                )
                fake_bin = write_executable_python(
                    root / "fake-designos",
                    """
                    import sys
                    from pathlib import Path
                    if "--seed-library" in sys.argv:
                        idx = sys.argv.index("--seed-library")
                        Path(sys.argv[idx + 1]).mkdir(parents=True, exist_ok=True)
                        sys.exit(0)
                    sys.exit(0)
                    """,
                )
                make_fake_mcp(
                    root,
                    force_is_error=(err_mode == "is_error"),
                    force_rpc_error=(err_mode == "rpc_error"),
                )
                out_dir = root / "out"
                proc = self.run_script(
                    "--bin",
                    str(fake_bin),
                    "--cases",
                    str(cases_file),
                    "--stubs",
                    str(stubs),
                    "--out",
                    str(out_dir),
                    "--kit",
                    str(kit_file),
                )
                self.assertEqual(proc.returncode, 1, f"{err_mode}: {proc.stdout}\n{proc.stderr}")

    def test_missing_designos_mcp_exits_1_and_writes_no_kit_designos(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve()
            stubs = make_stub_dir(root)
            kit_file = make_fake_kit(root)
            cases_file = root / "cases.json"
            cases_file.write_text(
                json.dumps({"cases": [{"id": "d-editor-default", "spec": "D", "args": []}]}),
                encoding="utf-8",
            )
            fake_bin = write_executable_python(
                root / "fake-designos",
                """
                import sys
                from pathlib import Path
                if "--seed-library" in sys.argv:
                    idx = sys.argv.index("--seed-library")
                    Path(sys.argv[idx + 1]).mkdir(parents=True, exist_ok=True)
                    sys.exit(0)
                sys.exit(0)
                """,
            )
            out_dir = root / "out"
            proc = self.run_script(
                "--bin",
                str(fake_bin),
                "--cases",
                str(cases_file),
                "--stubs",
                str(stubs),
                "--out",
                str(out_dir),
                "--kit",
                str(kit_file),
            )
            self.assertEqual(proc.returncode, 1, proc.stdout + proc.stderr)
            self.assertIn(
                "designos-mcp not found next to --bin: build it with -p designos-mcp",
                proc.stderr,
            )
            self.assertFalse((out_dir / "_kit_cache" / "kit.designos").exists())

    def test_one_page_kit_import_exits_1_with_page_count(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve()
            stubs = make_stub_dir(root)
            kit_file = make_fake_kit(root)
            cases_file = root / "cases.json"
            cases_file.write_text(
                json.dumps({"cases": [{"id": "d-editor-default", "spec": "D", "args": []}]}),
                encoding="utf-8",
            )
            fake_bin = write_executable_python(
                root / "fake-designos",
                """
                import sys
                from pathlib import Path
                if "--seed-library" in sys.argv:
                    idx = sys.argv.index("--seed-library")
                    Path(sys.argv[idx + 1]).mkdir(parents=True, exist_ok=True)
                    sys.exit(0)
                sys.exit(0)
                """,
            )
            make_fake_mcp(root, pages_count=1)
            out_dir = root / "out"
            proc = self.run_script(
                "--bin",
                str(fake_bin),
                "--cases",
                str(cases_file),
                "--stubs",
                str(stubs),
                "--out",
                str(out_dir),
                "--kit",
                str(kit_file),
            )
            self.assertEqual(proc.returncode, 1, proc.stdout + proc.stderr)
            self.assertIn("1 page", proc.stderr)
            self.assertFalse((out_dir / "_kit_cache" / "kit.designos").exists())


if __name__ == "__main__":
    unittest.main()
