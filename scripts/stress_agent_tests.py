#!/usr/bin/env python3
"""Run cargo test for designos-app's `agent` filter M times under N synthetic CPU burners."""

from __future__ import annotations

import argparse
import os
import re
import signal
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FAILURE_HEADER_RE = re.compile(r"^failures:\s*$", re.MULTILINE)
RESULT_RE = re.compile(r"test result: [^\r\n]+")


def parse_failed_tests(output: str) -> list[str]:
    failed: list[str] = []
    lines = output.splitlines()
    in_failures_list = False
    for line in lines:
        if line.strip() == "failures:":
            in_failures_list = True
            failed.clear()
            continue
        if in_failures_list:
            stripped = line.strip()
            if not stripped or stripped.startswith("test result:"):
                in_failures_list = False
            elif not stripped.startswith("---- "):
                failed.append(stripped)
    return failed


def stop_burners(burners: list[subprocess.Popen[bytes]]) -> None:
    for proc in burners:
        if proc.poll() is None:
            try:
                proc.kill()
            except OSError:
                pass
    for proc in burners:
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            pass


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--burners", type=int, default=16, help="Number of CPU burner processes (default: 16)")
    parser.add_argument("--runs", type=int, default=10, help="Number of test filter runs (default: 10)")
    parser.add_argument("--filter", default="agent", help="Test name filter (default: agent)")
    args = parser.parse_args()

    env = os.environ.copy()
    cargo_cmd_base = [
        "cargo",
        "test",
        "-p",
        "designos-app",
        "--locked",
        "--bin",
        "designos",
    ]

    build_proc = subprocess.run(
        [*cargo_cmd_base, "--no-run"],
        cwd=str(ROOT),
        env=env,
        capture_output=True,
        text=True,
    )
    if build_proc.returncode != 0:
        sys.stderr.write(build_proc.stdout)
        sys.stderr.write(build_proc.stderr)
        return build_proc.returncode

    burners: list[subprocess.Popen[bytes]] = []

    def handle_signal(signum: int, _frame: object) -> None:
        stop_burners(burners)
        raise SystemExit(128 + signum)

    signal.signal(signal.SIGINT, handle_signal)
    signal.signal(signal.SIGTERM, handle_signal)

    passed = 0
    failed = 0

    try:
        for _ in range(max(0, args.burners)):
            burners.append(
                subprocess.Popen(
                    [sys.executable, "-c", "while True:\n    pass\n"],
                    stdin=subprocess.DEVNULL,
                    stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL,
                )
            )
        burner_pids = [p.pid for p in burners]
        print(
            f"Started {len(burners)} CPU burners (PIDs: {burner_pids}); "
            f"running `cargo test -p designos-app --locked --bin designos {args.filter}` {args.runs} times...",
            flush=True,
        )

        for idx in range(1, args.runs + 1):
            t0 = time.monotonic()
            proc = subprocess.run(
                [*cargo_cmd_base, args.filter],
                cwd=str(ROOT),
                env=env,
                capture_output=True,
                text=True,
            )
            elapsed = time.monotonic() - t0
            combined = (proc.stdout or "") + "\n" + (proc.stderr or "")
            results = RESULT_RE.findall(combined)
            summary = results[-1] if results else f"exit={proc.returncode}"
            if proc.returncode == 0:
                passed += 1
                print(f"[run {idx}/{args.runs}] PASS ({elapsed:.2f}s) — {summary}", flush=True)
            else:
                failed += 1
                failed_tests = parse_failed_tests(combined)
                print(
                    f"[run {idx}/{args.runs}] FAIL ({elapsed:.2f}s) — {summary} — failed: {failed_tests}",
                    flush=True,
                )
                for line in combined.splitlines():
                    if (
                        "panicked at" in line
                        or line.strip().startswith("---- ")
                        or "assertion" in line
                    ):
                        print(f"    {line}", flush=True)
    finally:
        stop_burners(burners)

    print(
        f"Stress summary (burners={args.burners}, runs={args.runs}, filter={args.filter}): "
        f"{passed}/{args.runs} passed, {failed}/{args.runs} failed",
        flush=True,
    )
    return 0 if failed == 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())
