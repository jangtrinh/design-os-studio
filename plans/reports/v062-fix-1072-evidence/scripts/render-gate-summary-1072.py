"""Builds a terminal-style HTML page from the #1072 evidence logs (lines copied verbatim from the logs, one table joined from two of them)
for a PNG capture. Usage: python3 -I render-gate-summary-1072.py <evidence folder> <out.html> <head sha> <pr number>"""
import html
import re
import sys
from pathlib import Path

E = Path(sys.argv[1])
out = Path(sys.argv[2])
head, pr = sys.argv[3], sys.argv[4]
NEW_TESTS = (
    r"hook_output_tests|locked_entry_tests|pty_error_tests|hookless_tab_tests|agent_hook_outputs::tests|no_follow::set_aside::tests|"
    r"edits_with_no_marks_and_no_status_change"
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
    return name.replace("agent_session_files::", "").replace("terminal::", "").replace("no_follow::", "")[:118]


red, green = results("r1-red-on-dc23fdd4.log"), results("gate-test-app.log")
table = [f"{red.get(name, 'new'):>6} -> {green[name]:<2}  {short(name)}" for name in sorted(green, key=short)]
race = grab("gate-race-loop-designos.log", [r"run 1 "]) + grab("gate-race-loop-designos-studio.log", [r"run 1 "])
muts = []
for name in sorted(p.name for p in E.glob("r1-mut-*.log")):
    text = lines_of(name)
    failed = [line for line in text if line.startswith("test ") and line.endswith("FAILED")]
    result = [line for line in text if line.startswith("test result")]
    muts.append(f"{name[3:-4]:<56} {result[0].split(';')[0].replace('test result: ', '') if result else 'no result'}; {len(failed)} test(s) fail")
sections = [
    ("home mtimes before the gate run (names + mtimes only)", grab("gate-mtimes-before.txt", [r"\d"])),
    ("cargo fmt --all -- --check", grab("gate-fmt.log", [r"exit="])),
    ("cargo clippy --workspace --all-targets --locked -- -D warnings", grab("gate-clippy.log", [r"Finished", r"exit="])),
    ("python3 scripts/zero_fake_ui_gate.py", grab("gate-zero-fake-ui.log", [r"Gate Verdict", r"exit="])),
    ("cargo test -p designos-mcp --locked  (isolated HOME, scratch TMPDIR with no '//')", grab("gate-test-mcp.log", [r"^test result: ok\. 2\d\d", r"FAILED", r"^mcp exit="])),
    ("scan test: every file-system call and session-file name is routed or listed (no row mentions #1072)", grab("gate-scan.log", [r"^test result", r"^scan exit="])),
    ("cargo test -p designos-app --bin designos --locked", grab("gate-test-app.log", [r"^test result", r"^app exit=", r"FAILED$"])),
    ("cargo test -p designos-app --bin designos-studio --locked", grab("gate-test-studio.log", [r"^test result", r"^studio exit=", r"FAILED$"])),
    ("race test, once per bin (the other bin's full suite runs at the same time)", race),
    ("each new test: on dc23fdd4 (the PR base, tests alone) -> on the head", table),
    ("mutations (code changed on the head, the new tests run, the code put back)", muts),
    ("home mtimes after the gate run", grab("gate-mtimes-after.txt", [r"\d"])),
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
</style><h1>#1072: every hook of every agent CLI reports to a private folder no agent grant reaches. PR #{html.escape(pr)}, head {html.escape(head)}, on release/v0.6.2 dc23fdd4, 2026-10-25</h1>
{''.join(rows)}"""
out.write_text(page)
print(out)
