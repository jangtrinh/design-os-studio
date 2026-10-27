"""Unit tests for scripts/layout_baseline_capture.py (Spec L / #1126 test 11).

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
            counter_file = root / "attempts.txt"
            cases_file = root / "cases.json"
            cases_file.write_text(
                json.dumps({"cases": [{"id": "d-editor-default", "spec": "D", "args": ["--fixture", "showcase"]}]}),
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
            self.assertEqual(proc.returncode, 3, proc.stdout + proc.stderr)
            self.assertEqual(int(counter_file.read_text(encoding="utf-8")), 3)

    def test_owned_infra_exits_1(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve()
            stubs = make_stub_dir(root)
            counter_file = root / "attempts.txt"
            cases_file = root / "cases.json"
            cases_file.write_text(
                json.dumps({"cases": [{"id": "d-editor-default", "spec": "D", "args": ["--fixture", "showcase"]}]}),
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
            )
            self.assertEqual(proc.returncode, 1, proc.stdout + proc.stderr)
            self.assertEqual(int(counter_file.read_text(encoding="utf-8")), 3)

    def test_empty_named_map_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve()
            stubs = make_stub_dir(root)
            cases_file = root / "cases.json"
            cases_file.write_text(
                json.dumps({"cases": [{"id": "d-editor-default", "spec": "D", "args": ["--fixture", "showcase"]}]}),
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
            self.assertEqual(proc.returncode, 1, proc.stdout + proc.stderr)
            self.assertIn("named", (proc.stdout + proc.stderr).lower())

    def test_env_is_built_with_env_i_and_four_markers(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve()
            stubs = make_stub_dir(root)
            cases_file = root / "cases.json"
            cases_file.write_text(
                json.dumps(
                    {
                        "cases": [
                            {"id": "h-home", "spec": "H", "args": ["--library", "{LIB}", "--home"]},
                            {"id": "d-window-chrome", "spec": "D", "args": ["--fixture", "showcase"]},
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
                cases_file = root / "cases.json"
                cases_file.write_text(
                    json.dumps({"cases": [{"id": "d-editor-default", "spec": "D", "args": ["--fixture", "showcase"]}]}),
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
                self.assertEqual(proc.returncode, 1, f"mode={mode}: {proc.stdout}\n{proc.stderr}")

    def test_spawns_case_with_empty_log_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve()
            stubs = make_stub_dir(root)
            cases_file = root / "cases.json"
            cases_file.write_text(
                json.dumps(
                    {
                        "cases": [
                            {
                                "id": "i-agents-chat",
                                "spec": "I",
                                "args": ["--fixture", "showcase", "--panel", "agents-chat"],
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
            self.assertEqual(proc.returncode, 1, proc.stdout + proc.stderr)

    def test_argv0_outside_stub_dir_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve()
            stubs = make_stub_dir(root)
            cases_file = root / "cases.json"
            cases_file.write_text(
                json.dumps(
                    {
                        "cases": [
                            {
                                "id": "i-agents-chat",
                                "spec": "I",
                                "args": ["--fixture", "showcase", "--panel", "agents-chat"],
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
            self.assertEqual(proc.returncode, 1, proc.stdout + proc.stderr)

    def test_kit_case_without_kit_is_printed_as_skipped(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve()
            stubs = make_stub_dir(root)
            cases_file = root / "cases.json"
            cases_file.write_text(
                json.dumps(
                    {
                        "cases": [
                            {"id": "d-editor-default", "spec": "D", "args": ["--fixture", "showcase"]},
                            {
                                "id": "kit-editor",
                                "spec": "D",
                                "args": ["{KIT}", "--select", "Page header"],
                                "kit": True,
                            },
                        ]
                    }
                ),
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
                shot = Path(sys.argv[sys.argv.index("--screenshot") + 1])
                ljson = Path(sys.argv[sys.argv.index("--layout-json") + 1])
                write_solid_png(shot, 200, 160)
                ljson.write_text({valid_snap!r})
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
            self.assertEqual(proc.returncode, 0, proc.stdout + proc.stderr)
            self.assertIn("SKIP kit-editor: no --kit", proc.stdout)
            self.assertFalse((out_dir / "kit-editor.png").exists())

    def test_g6_builds_crop_and_ignore_list_from_fixture_layout_json(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve()
            stubs = make_stub_dir(root)
            cases_file = root / "cases.json"
            cases_file.write_text(
                json.dumps({"cases": [{"id": "d-editor-default", "spec": "D", "args": ["--fixture", "showcase"]}]}),
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


if __name__ == "__main__":
    unittest.main()
