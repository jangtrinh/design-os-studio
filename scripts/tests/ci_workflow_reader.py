"""Text-level reader for `.github/workflows/ci.yml`, shared by the macOS-on-main workflow tests (no PyYAML needed).

`CI_WORKFLOW_FILE` points the tests at another copy of the workflow, which is how the "bad copy" checks run.
"""

import os
import re
import subprocess
import textwrap
from pathlib import Path
from types import SimpleNamespace

CI = Path(os.environ.get("CI_WORKFLOW_FILE") or Path(__file__).resolve().parents[2] / ".github" / "workflows" / "ci.yml")
MAIN_PUSH = "github.event_name == 'push' && github.ref == 'refs/heads/main' && vars.TRAIN_LIVE != 'true'"

PR_REF = "refs/pull/1/merge"
MAIN_REF = "refs/heads/main"
TRAIN_REF = "refs/heads/train/20261112T000000Z"
TAG_REF = "refs/tags/v0.7.14"
QUEUE_REF = "refs/heads/gh-readonly-queue/main/pr-1-abc"

KEY_ENV_PREFIXES = ("RUSTFLAGS", "CARGO", "RUST", "CC", "CFLAGS", "CXX", "CMAKE")


def workflow_lines() -> list:
    return CI.read_text(encoding="utf-8").splitlines()


def job_block(name: str) -> str:
    """The text of one top-level job: from its `  name:` line to the next job (minus the next job's leading comment)."""
    lines = workflow_lines()
    start = lines.index(f"  {name}:")
    end = len(lines)
    for i in range(start + 1, len(lines)):
        if re.match(r"^  [A-Za-z0-9_-]+:\s*$", lines[i]):
            end = i
            break
    while end > start + 1 and (not lines[end - 1].strip() or lines[end - 1].startswith("  #")):
        end -= 1
    return "\n".join(lines[start:end])


def job_if(name: str) -> str:
    match = re.search(r"^    if: (.+)$", job_block(name), re.MULTILINE)
    assert match, f"job {name} has no `if:`"
    return match.group(1)


def job_field(name: str, key: str) -> str:
    match = re.search(rf"^    {re.escape(key)}: (.+)$", job_block(name), re.MULTILINE)
    assert match, f"job {name} has no `{key}:`"
    return match.group(1)


def job_env(name: str) -> dict:
    """The job-level `env:` mapping (keys at 6 spaces, directly under a 4-space `env:`)."""
    match = re.search(r"^    env:\n((?:      .+\n)+)", job_block(name) + "\n", re.MULTILINE)
    if not match:
        return {}
    pairs = (line.strip().split(": ", 1) for line in match.group(1).splitlines())
    return {key: value.strip('"') for key, value in pairs}


def job_steps(name: str) -> list:
    """Each step as {"id", "name", "if", "uses", "run", "with": {...}, "env": {...}}. Comments are skipped."""
    lines = job_block(name).splitlines()
    start = next(i for i, line in enumerate(lines) if line == "    steps:")
    steps: list = []
    section = None
    for raw in lines[start + 1 :]:
        if not raw.strip() or raw.strip().startswith("#"):
            continue
        line = raw
        if line.startswith("      - "):
            steps.append({"with": {}, "env": {}})
            line = "        " + line[8:]
            section = None
        if not steps:
            continue
        indent = len(line) - len(line.lstrip())
        if section == "run" and indent >= 10:
            steps[-1]["run"] += ("\n" if steps[-1]["run"] else "") + line[10:]
            continue
        key, _, value = line.strip().partition(":")
        value = value.strip().strip('"')
        if indent == 8:
            section = key if key in ("with", "env") else None
            if key == "run" and value == "|":
                section, steps[-1]["run"] = "run", ""
            elif key not in ("with", "env"):
                steps[-1][key] = value
        elif indent == 10 and section in ("with", "env"):
            steps[-1][section][key] = value
    return steps


def rust_cache_steps(name: str) -> list:
    return [step for step in job_steps(name) if step.get("uses", "").startswith("Swatinem/rust-cache@")]


def matrix_legs(name: str) -> list:
    """The entries of a literal `matrix: include:` list; [] when the job has none."""
    lines = job_block(name).splitlines()
    try:
        start = lines.index("        include:")
    except ValueError:
        return []
    legs: list = []
    for line in lines[start + 1 :]:
        if not line.strip() or line.strip().startswith("#"):
            continue
        if not line.startswith("          "):
            break
        if line.startswith("          - "):
            legs.append({})
            line = "            " + line[12:]
        key, _, value = line.strip().partition(":")
        legs[-1][key] = value.strip().strip('"')
    return legs


def resolve(text: str, leg: dict) -> str:
    return re.sub(r"\$\{\{\s*matrix\.([\w-]+)\s*\}\}", lambda m: leg[m.group(1)], text)


def eval_expr(expr: str, event: str, ref: str, draft: bool = False, rust: str = "true", train_live: str = "") -> bool:
    """Evaluate a workflow `if:`-style expression for the few operators these workflows use."""
    py = re.sub(r"!(?!=)", " not ", expr).replace("&&", " and ").replace("||", " or ")
    scope = {
        "github": SimpleNamespace(event_name=event, ref=ref, event=SimpleNamespace(pull_request=SimpleNamespace(draft=draft))),
        "needs": SimpleNamespace(changes=SimpleNamespace(outputs=SimpleNamespace(rust=rust))),
        "vars": SimpleNamespace(TRAIN_LIVE=train_live),
    }
    return bool(eval(py, {"__builtins__": {}}, scope))  # noqa: S307 - the expression comes from our own workflow file


def job_runs(name: str, event: str, ref: str, **kwargs) -> bool:
    return eval_expr(job_if(name), event, ref, **kwargs)


def concurrency() -> dict:
    lines = workflow_lines()
    start = lines.index("concurrency:")
    block = {}
    for line in lines[start + 1 :]:
        if not line.startswith("  "):
            break
        key, _, value = line.strip().partition(":")
        block[key] = value.strip()
    return block


def aggregate_script() -> str:
    block = job_block("test-macos-required").splitlines()
    start = next(i for i, line in enumerate(block) if line.strip() == "run: |")
    body = []
    for line in block[start + 1 :]:
        if line.strip() and not line.startswith(" " * 10):
            break
        body.append(line)
    return textwrap.dedent("\n".join(body))


def run_aggregate(event: str, ref: str, shards: str, lint: str, changes: str = "success", rust: str = "true", draft: str = "", train_live: str = "") -> subprocess.CompletedProcess:
    env = {
        "PATH": os.environ["PATH"],
        "EVENT_NAME": event,
        "REF_NAME": ref,
        "TRAIN_LIVE": train_live,
        "PR_DRAFT": draft,
        "CHANGES_RESULT": changes,
        "RUST_CHANGED": rust,
        "SHARDS_RESULT": shards,
        "LINT_RESULT": lint,
    }
    return subprocess.run(["bash", "-c", aggregate_script()], env=env, capture_output=True, text=True, timeout=30)
