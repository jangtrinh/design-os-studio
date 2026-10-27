"""Builds a terminal-style HTML page from the #1072 round-3 evidence logs (lines copied verbatim from the logs, one table joined from two
of them) for a PNG capture. Usage: python3 -I render-gate-summary-1072-r3.py <evidence folder> <out.html> <head sha> <pr number>"""
import html
import re
import sys
from pathlib import Path

E = Path(sys.argv[1])
out = Path(sys.argv[2])
head, pr = sys.argv[3], sys.argv[4]
NEW_TESTS = (
    r"capture_lock_tests|unwritable_hook_folder_tests"
)


def lines_of(name):
    path = E / name
    return path.read_text(errors="replace").splitlines() if path.exists() else [f"(missing log: {name})"]


def grab(name, patterns):
    return [line for line in lines_of(name) if any(re.search(p, line) for p in patterns)]


def results(name):
    found = {}
    for line in lines_of(name):
        m = re.match(r"^test (\S*(?:" + NEW_TESTS + r")\S*) \.\.\. (ok|FAILED)$", line)
        if m:
            found[m.group(1)] = m.group(2)
    return found


def short(name):
    return name.replace("agent_session_files::", "").replace("terminal::", "").replace("agent_composer::tests::", "")[:118]


red, green = {**results("r3b-red-on-5384e79f.log"), **results("r3-red-on-c9b2d8b1.log")}, results("gate-r3-test-app.log")
table = [f"{red.get(name, 'new'):>6} -> {green[name]:<2}  {short(name)}" for name in sorted(green, key=short)]
race = grab("gate-r3-race-loop-designos.log", [r"run 1 "]) + grab("gate-r3-race-loop-designos-studio.log", [r"run 1 "])
muts = []
for name in sorted(p.name for p in E.glob("r3-mut-*.log")):
    text = lines_of(name)
    failed = [line for line in text if line.startswith("test ") and line.endswith("FAILED")]
    result = [line for line in text if line.startswith("test result")]
    muts.append(f"{name[3:-4]:<66} {result[0].split(';')[0].replace('test result: ', '') if result else 'no result'}; {len(failed)} test(s) fail")
sections = [
    ("home mtimes before the gate run (names + mtimes only)", grab("gate-r3-mtimes-before.txt", [r"\d"])),
    ("cargo fmt --all -- --check", grab("gate-r3-fmt.log", [r"exit="])),
    ("cargo clippy --workspace --all-targets --locked -- -D warnings", grab("gate-r3-clippy.log", [r"Finished", r"exit="])),
    ("python3 scripts/zero_fake_ui_gate.py", grab("gate-r3-zero-fake-ui.log", [r"Gate Verdict", r"exit="])),
    ("cargo test -p designos-mcp --locked  (isolated HOME, scratch TMPDIR with no '//')", grab("gate-r3-test-mcp.log", [r"^test result: ok\. 2\d\d", r"FAILED", r"^mcp exit="])),
    ("scan test: every file-system call and session-file name is routed or listed", grab("gate-r3-scan.log", [r"^test result", r"^scan exit="])),
    ("cargo test -p designos-app --bin designos --locked", grab("gate-r3-test-app.log", [r"^test result", r"^app exit=", r"FAILED$"])),
    ("cargo test -p designos-app --bin designos-studio --locked", grab("gate-r3-test-studio.log", [r"^test result", r"^studio exit=", r"FAILED$"])),
    ("race test, once per bin (the other bin's full suite runs at the same time)", race),
    (
        "both bins again, each by itself with --test-threads 6 (the lines above ran next to each other with the race test, on a machine at load 8 to 12)",
        grab("gate-r3-test-app-threads6.log", [r"^test result"]) + grab("gate-r3-test-studio-threads6.log", [r"^test result"]),
    ),
    ("each round-3 test: on c9b2d8b1 (and the third-tab test on 5384e79f), tests alone, before the fixes -> on the head", table),
    ("mutations (code changed on the head, the tests run, the code put back; each diff is saved beside its log)", muts),
    ("home mtimes after the gate run", grab("gate-r3-mtimes-after.txt", [r"\d"])),
]

home = str(Path.home())
rows = []
for title, lines in sections:
    rows.append(f'<div class="cmd">$ {html.escape(title)}</div>')
    for line in lines:
        line = line.replace(home, "~")
        cls = "bad" if re.search(r"FAILED|panicked|exit=[12]|HIT=[1-9]|other=[1-9]", line) and "-> ok" not in line else "ok"
        if re.search(r"test result: ok|exit=0|PASS|HIT=0 *$|Finished|Gate Verdict: PASS|-> ok", line):
            cls = "good"
        if re.search(r"[1-9]\d* test\(s\) fail", line):
            cls = "good"
        rows.append(f'<div class="{cls}">{html.escape(line[:215])}</div>')

page = f"""<!doctype html><meta charset="utf-8"><style>
body{{background:#0d1117;color:#c9d1d9;font:12px/1.4 Menlo,monospace;margin:0;padding:18px 22px;width:1560px}}
.cmd{{color:#79c0ff;margin-top:10px}} .good{{color:#7ee787}} .bad{{color:#ffa198}} .ok{{color:#c9d1d9}}
h1{{font-size:14px;color:#e6edf3;margin:0 0 6px}}
</style><h1>#1072 round 3 (review of a52f1f5e): a capture writes and unlocks only under its own lock; Claude's prompt hook ends quietly too. PR #{html.escape(pr)}, head {html.escape(head)}, 2026-10-25</h1>
{''.join(rows)}"""
out.write_text(page)
print(out)
