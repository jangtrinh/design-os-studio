"""Builds a terminal-style HTML page from the round-4 evidence logs of #1067 (lines copied verbatim from the logs) for a PNG capture.
Usage: python3 -I render-gate-summary-round4.py <evidence folder> <out.html> <head sha> <pr number>"""
import html
import re
import sys
from pathlib import Path

E = Path(sys.argv[1])
out = Path(sys.argv[2])
head, pr = sys.argv[3], sys.argv[4]


def grab(name, patterns):
    path = E / name
    if not path.exists():
        return [f"(missing log: {name})"]
    text = path.read_text(errors="replace").splitlines()
    return [line for line in text if any(re.search(p, line) for p in patterns)]


race = grab("gate4-race-loop-designos.log", [r"run (1|5|10) "]) + grab("gate4-race-loop-designos-studio.log", [r"run (1|5|10) "])
scan_reds = []
for name in ["round4-scan-red-routing-removed.log", "round4-scan-red-fresh-read.log", "round4-scan-red-hook-redirect.log", "round4-scan-red-stale-entries.log"]:
    scan_reds += [f"[{name[len('round4-scan-red-'):-4]}]"] + [line[:200] for line in grab(name, [r"^not listed", r"^listed ", r"^stale", r"joins ", r"^test result"])]
sections = [
    ("home mtimes before the round-4 gate run (names + mtimes only)", grab("gate4-mtimes-before.txt", [r"\d"])),
    ("cargo fmt --all -- --check", grab("gate4-fmt.log", [r"exit="])),
    ("cargo clippy -p designos-app --all-targets --locked -- -D warnings", grab("gate4-clippy.log", [r"Finished", r"exit="])),
    ("python3 scripts/zero_fake_ui_gate.py", grab("gate4-zero-fake-ui.log", [r"Gate Verdict", r"exit="])),
    ("cargo test -p designos-mcp --locked  (isolated HOME, scratch TMPDIR with no '//')", grab("gate4-test-mcp.log", [r"^test result: ok\. 2\d\d", r"FAILED", r"^mcp exit="])),
    ("cargo test -p designos-app --bin designos --locked", grab("gate4-test-app.log", [r"^test result", r"^app exit=", r"FAILED$"])),
    ("cargo test -p designos-app --bin designos-studio --locked", grab("gate4-test-studio.log", [r"^test result", r"^studio exit=", r"FAILED$"])),
    ("race test x10 per bin while the other bin's full suite runs (runs 1, 5, 10 shown; all 20 are in the logs)", race),
    ("scan test, green on the allowlist", grab("round4-scan-green.log", [r"^test .*ok$", r"^test result"])),
    ("scan test, RED on four mutations (a routed read made std::fs; a fresh std::fs::read_to_string(session_dir.join(\"x\")); a hook redirect; stale and miscounted entries)", scan_reds),
    ("4 fallback-id tests x60 in parallel: on 17bce1bb (before) and on the head (after)", grab("round4-fallback-loop-base-17bce1bb.log", [r"of 60 runs failed", r"exit=101"])[-3:] + grab("round4-fallback-loop-head-e720db3d.log", [r"of 60 runs failed"])),
    ("RED on 17bce1bb: live-spec.json through a link or FIFO, a link removed after the failed open, a tailer behind a link", grab("round4-red-on-17bce1bb.log", [r"^test .*(FAILED|ok)$", r"^test result"])),
    ("home mtimes after the round-4 gate run", grab("gate4-mtimes-after.txt", [r"\d"])),
]

home = str(Path.home())
rows = []
for title, lines in sections:
    rows.append(f'<div class="cmd">$ {html.escape(title)}</div>')
    for line in lines:
        line = line.replace(home, "~")
        cls = "bad" if re.search(r"FAILED|panicked|exit=[12]|HIT=[1-9]|other=[1-9]|blocked on|victim|^not listed|^listed|^stale|joins ", line) else "ok"
        if re.search(r"test result: ok|exit=0|PASS|HIT=0 *$|Finished|Gate Verdict: PASS|0 of 60|ok$", line):
            cls = "good"
        rows.append(f'<div class="{cls}">{html.escape(line[:215])}</div>')

page = f"""<!doctype html><meta charset="utf-8"><style>
body{{background:#0d1117;color:#c9d1d9;font:13px/1.45 Menlo,monospace;margin:0;padding:18px 22px;width:1560px}}
.cmd{{color:#79c0ff;margin-top:10px}} .good{{color:#7ee787}} .bad{{color:#ffa198}} .ok{{color:#c9d1d9}}
h1{{font-size:14px;color:#e6edf3;margin:0 0 6px}}
</style><h1>#1061 round 4: PR #{html.escape(pr)}, code head {html.escape(head)}, on release/v0.6.2 894c2286, 2026-10-24</h1>
{''.join(rows)}"""
out.write_text(page)
print(out)
