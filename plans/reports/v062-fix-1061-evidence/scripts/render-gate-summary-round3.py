"""Builds a terminal-style HTML page from the round-3 evidence logs of #1067 (lines copied verbatim from the logs) for a PNG capture.
Usage: python3 -I render-gate-summary-round3.py <evidence folder> <out.html> <head sha> <pr number>"""
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


race = grab("gate3-race-loop-designos.log", [r"run (1|5|10) "]) + grab("gate3-race-loop-designos-studio.log", [r"run (1|5|10) "])
sections = [
    ("home mtimes before the round-3 gate run (names + mtimes only)", grab("gate3-mtimes-before.txt", [r"\d"])),
    ("cargo fmt --all -- --check", grab("gate3-fmt.log", [r"exit="])),
    ("cargo clippy -p designos-app --all-targets --locked -- -D warnings", grab("gate3-clippy.log", [r"Finished", r"exit="])),
    ("python3 scripts/zero_fake_ui_gate.py", grab("gate3-zero-fake-ui.log", [r"Gate Verdict", r"exit="])),
    ("cargo test -p designos-mcp --locked  (isolated HOME, scratch TMPDIR with no '//')", grab("gate3-test-mcp.log", [r"^test result: ok\. 2\d\d", r"FAILED", r"^mcp exit="])),
    ("cargo test -p designos-app --bin designos --locked", grab("gate3-test-app.log", [r"^test result", r"^app exit=", r"FAILED$"])),
    ("cargo test -p designos-app --bin designos-studio --locked", grab("gate3-test-studio.log", [r"^test result", r"^studio exit=", r"FAILED$"])),
    ("race test x10 per bin while the other bin's full suite runs (runs 1, 5, 10 shown; all 20 are in the logs)", race),
    ("RED on 50fe8710: a FIFO or a link at a file Studio polls", grab("round3-red-on-50fe8710.log", [r"^test .*(FAILED|ok)$", r"blocked on the FIFO", r"victim", r"^test result"])),
    ("home mtimes after the round-3 gate run", grab("gate3-mtimes-after.txt", [r"\d"])),
]

home = str(Path.home())
rows = []
for title, lines in sections:
    rows.append(f'<div class="cmd">$ {html.escape(title)}</div>')
    for line in lines:
        line = line.replace(home, "~")
        cls = "bad" if re.search(r"FAILED|panicked|exit=[12]|HIT=[1-9]|other=[1-9]|blocked on|victim", line) else "ok"
        if re.search(r"test result: ok|exit=0|PASS|HIT=0 *$|Finished|Gate Verdict: PASS", line):
            cls = "good"
        rows.append(f'<div class="{cls}">{html.escape(line[:215])}</div>')

page = f"""<!doctype html><meta charset="utf-8"><style>
body{{background:#0d1117;color:#c9d1d9;font:13px/1.45 Menlo,monospace;margin:0;padding:18px 22px;width:1560px}}
.cmd{{color:#79c0ff;margin-top:10px}} .good{{color:#7ee787}} .bad{{color:#ffa198}} .ok{{color:#c9d1d9}}
h1{{font-size:14px;color:#e6edf3;margin:0 0 6px}}
</style><h1>#1061 round 3: PR #{html.escape(pr)}, code head {html.escape(head)}, on release/v0.6.2 f8f2186f, 2026-10-24</h1>
{''.join(rows)}"""
out.write_text(page)
print(out)
