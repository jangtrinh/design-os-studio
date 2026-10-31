"""Chrome discovery and Rust fixture loader for grid Chrome harness."""

import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
from typing import Any, Dict, List, Optional

CHROME_CANDIDATES = (
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/Applications/Chromium.app/Contents/MacOS/Chromium",
    "google-chrome",
    "chromium",
    "chromium-browser",
    "chrome",
)


def _resolve_executable(candidate: str) -> Optional[str]:
    if os.path.isfile(candidate) and os.access(candidate, os.X_OK):
        return candidate
    return shutil.which(candidate)


def find_chrome(explicit_bin: Optional[str] = None) -> Optional[str]:
    if explicit_bin is not None:
        return _resolve_executable(explicit_bin)
    env_bin = os.environ.get("CHROME_BIN")
    if env_bin is not None and env_bin.strip() != "":
        return _resolve_executable(env_bin)
    for cand in CHROME_CANDIDATES:
        resolved = _resolve_executable(cand)
        if resolved:
            return resolved
    return None


def load_fixtures_from_rust(repo_root: Path) -> List[Dict[str, Any]]:
    real_home = os.environ.get("HOME", str(Path.home()))
    t0 = os.environ.get("TMPDIR", "/tmp").rstrip("/")
    cargo_target = os.environ.get("CARGO_TARGET_DIR", str(repo_root / "out" / "cargo"))
    with tempfile.TemporaryDirectory(prefix="x.", dir=t0) as sandbox:
        s_home = Path(sandbox) / "home"
        s_home.mkdir(parents=True, exist_ok=True)
        env = dict(os.environ)
        env.update(
            {
                "PATH": f"{real_home}/.cargo/bin:{env.get('PATH', '')}",
                "HOME": str(s_home),
                "TMPDIR": f"{sandbox}/",
                "CARGO_HOME": f"{real_home}/.cargo",
                "RUSTUP_HOME": f"{real_home}/.rustup",
                "GEMINI_CLI_HOME": str(s_home),
                "CODEX_HOME": str(s_home / ".codex"),
                "CLAUDE_CONFIG_DIR": str(s_home / ".claude"),
                "CARGO_TARGET_DIR": cargo_target,
                "GRID_CHROME_DUMP_JSON": "1",
            }
        )
        cmd = [
            "cargo",
            "test",
            "-p",
            "designos-export",
            "--test",
            "grid_chrome_fixtures",
            "--locked",
            "equal_tracks_css_and_tailwind",
            "--",
            "--exact",
            "--nocapture",
        ]
        proc = subprocess.run(cmd, cwd=repo_root, env=env, capture_output=True, text=True, check=False)
        if proc.returncode != 0:
            raise RuntimeError(
                f"cargo test grid_chrome_fixtures failed (exit {proc.returncode}):\n{proc.stdout}\n{proc.stderr}"
            )
        combined = proc.stdout + "\n" + proc.stderr
        begin = "===GRID_CHROME_JSON_BEGIN==="
        end = "===GRID_CHROME_JSON_END==="
        if begin not in combined or end not in combined:
            raise RuntimeError(f"Missing JSON markers in cargo output:\n{combined}")
        payload = combined.split(begin, 1)[1].split(end, 1)[0].strip()
        return json.loads(payload)


def inline_css(css_lines: str) -> str:
    return " ".join(line.strip() for line in css_lines.strip().splitlines() if line.strip())
