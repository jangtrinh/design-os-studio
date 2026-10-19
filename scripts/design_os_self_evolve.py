#!/usr/bin/env python3
"""
design:os studio — Self-Evolution Engine & Live Token Bridge (`scripts/design_os_self_evolve.py`)
==================================================================================================
Orchestrates the closed-loop self-evolving design verification and pulse for `design:os studio`:

  1. G01 · `check_rust_token_parity`:
     Parses `crates/designos-app/src/theme.rs` for the 15 core hex constants (`BG_PRIMARY`,
     `BG_SECONDARY`, `BG_TERTIARY`, `BG_HOVER`, `BG_PRESSED`, `BORDER`, `BORDER_SUBTLE`,
     `BORDER_STRONG`, `BG_BRAND`, `TEXT_COMPONENT`, `COMPONENT`, `CANVAS_COMPONENT`,
     `DEV_READY`, `DEV_COMPLETED`, `DEV_CHANGED`) and verifies:
       - Zero Purple Ban violations (`#9747FF`, `#8755E9`, `#C4A1FF`, `#A855F7` never used on active UI/component tokens).
       - Exact hex parity against `design/design.tokens.json` (`colors.bg`, `colors.surface`,
         `colors.secondary`, `colors.bg-hover`, `colors.bg-active`, `colors.border`,
         `colors.border-subtle`, `colors.border-strong`, `colors.primary`, `colors.component`,
         `colors.success`).
       - When `--record-gaps` is enabled, automatically records any detected drift via
         `ui memory record gap`.

     The same gate also compares the non-colour tokens (radius, spacing, size) with their
     `theme.rs` constants through `scripts/non_colour_token_parity.py`.

  2. G02 · `zero_fake_ui_gate`:
     Executes `python3 scripts/zero_fake_ui_gate.py --json` and verifies every invariant passes (`passed == total`).

  3. G03 · `ui_ds_status`:
     Executes `ui ds status --dir . --json` and verifies compiled manifest + token & component registry health.

  4. G04 · `ui_ds_a11y`:
     Executes `ui ds a11y --dir . --json` and verifies `0` WCAG contrast failures across all paired & state tokens.

  5. G05 · `ui_ds_soul_check`:
     Executes `ui ds soul check --dir . --json` and verifies `0` findings (`errorCount == 0`, `warningCount == 0`)
     and `status: ratified` in `design/soul.md`.

  6. G06 · `ui_agents_check`:
     Executes `ui agents check --dir . --json` and verifies all 3 stamped roster agents (`designer`, `curator`,
     `figma-hand`) under `.claude/agents/` + all 4 cross-runtime agents under `.agents/agents/` have `0` errors.

  7. G07 · `self_evolution_pulse` (`design-os harvest` + `design-os heartbeat` + `ui memory compile` + `design-os evolution`):
     In default / `--evolve` mode, executes the full harvest -> heartbeat -> memory compile pulse, then runs
     `design-os evolution --dir . --json` and verifies:
       - `verdict == "ALIVE"`
       - `proof.level in ("APPLIED", "IMPROVING")`
       - `len(proof.findings) == 0`
     In `--check` mode, skips state-mutating harvest/heartbeat/compile steps and evaluates `design-os evolution` read-only.

  8. G08 · `ui_gate_specimen`:
     Executes `ui gate design/specimen.html --tokens design/design.tokens.json --json` and
     `ui ds-usage-lint design/specimen.html --dir . --json`, verifying `exit_code == 0` and `0` errors
     across all 6 gate families (`layout`, `a11y`, `taste`, `tell`, `content`, `autofix`) plus `0`
     token-usage errors.

  9. G09 · `design_os_project_audit`:
     Executes `design-os audit . --json` across all HTML/DS surfaces, plus `ui audit design/studio-ui-nodes.json`
     and `ui flow lint design/flow.json` when present, verifying `0` errors across the entire repository.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

from non_colour_token_parity import check_non_colour_parity

REPO_ROOT = Path(__file__).resolve().parent.parent
THEME_RS = REPO_ROOT / "crates" / "designos-app" / "src" / "theme.rs"
TOKENS_JSON = REPO_ROOT / "design" / "design.tokens.json"
REGISTRY_JSON = REPO_ROOT / "design" / "component-registry.json"
SPECIMEN_HTML = REPO_ROOT / "design" / "specimen.html"
STUDIO_UI_NODES_JSON = REPO_ROOT / "design" / "studio-ui-nodes.json"
FLOW_JSON = REPO_ROOT / "design" / "flow.json"
SOUL_MD = REPO_ROOT / "design" / "soul.md"
ZERO_FAKE_GATE = REPO_ROOT / "scripts" / "zero_fake_ui_gate.py"

BANNED_PURPLES = {"#9747ff", "#8755e9", "#c4a1ff", "#a855f7"}

CORE_RUST_CONSTANTS = [
    "BG_PRIMARY",
    "BG_SECONDARY",
    "BG_TERTIARY",
    "BG_HOVER",
    "BG_PRESSED",
    "BORDER",
    "BORDER_SUBTLE",
    "BORDER_STRONG",
    "BG_BRAND",
    "TEXT_COMPONENT",
    "COMPONENT",
    "CANVAS_COMPONENT",
    "DEV_READY",
    "DEV_COMPLETED",
    "DEV_CHANGED",
    "AGENT_1",
    "AGENT_2",
    "AGENT_3",
    "AGENT_4",
]

# Maps Rust `theme.rs` constants to canonical `design/design.tokens.json` paths under `colors.<key>`
RUST_TO_DESIGN_TOKEN_MAP: Dict[str, str] = {
    "BG_PRIMARY": "bg",
    "BG_SECONDARY": "surface",
    "BG_TERTIARY": "secondary",
    "BG_HOVER": "bg-hover",
    "BG_PRESSED": "bg-active",
    "BORDER": "border",
    "BORDER_SUBTLE": "border-subtle",
    "BORDER_STRONG": "border-strong",
    "BG_BRAND": "primary",
    "COMPONENT": "component",
    "CANVAS_COMPONENT": "component",
    "DEV_READY": "success",
    "DEV_COMPLETED": "component",
    "AGENT_1": "agent-1",
    "AGENT_2": "agent-2",
    "AGENT_3": "agent-3",
    "AGENT_4": "agent-4",
}

EXPECTED_STANDALONE_HEX: Dict[str, str] = {
    "TEXT_COMPONENT": "#22d3ee",
    "DEV_CHANGED": "#ffa629",
}

CROSS_RUNTIME_AGENTS = [
    "designer-jang-designos-studio.md",
    "curator-jang-designos-studio.md",
    "figma-jang-designos-studio.md",
    "librarian-jang-designos-studio.md",
]


def build_env() -> Dict[str, str]:
    env = os.environ.copy()
    home = str(Path.home())
    extra_paths = [
        f"{home}/.local/bin",
        f"{home}/.cargo/bin",
        "/opt/homebrew/bin",
        "/usr/local/bin",
    ]
    current_path = env.get("PATH", "")
    env["PATH"] = ":".join(extra_paths + ([current_path] if current_path else []))
    return env


def run_cli(cmd: List[str], cwd: Path = REPO_ROOT) -> Tuple[int, str, str]:
    proc = subprocess.run(
        cmd,
        cwd=str(cwd),
        env=build_env(),
        capture_output=True,
        text=True,
    )
    return proc.returncode, proc.stdout.strip(), proc.stderr.strip()


def run_cli_json(cmd: List[str], cwd: Path = REPO_ROOT) -> Tuple[int, Dict[str, Any], str]:
    code, out, err = run_cli(cmd, cwd=cwd)
    if not out:
        return code, {}, err
    try:
        return code, json.loads(out), err
    except json.JSONDecodeError:
        # Try parsing last JSON line if stdout had prefix logs
        for line in reversed(out.splitlines()):
            line = line.strip()
            if line.startswith("{") and line.endswith("}"):
                try:
                    return code, json.loads(line), err
                except json.JSONDecodeError:
                    pass
        return code, {}, err or out


def record_gap_event(message: str, target: str = "crates/designos-app/src/theme.rs") -> None:
    payload = json.dumps({"text": message, "target": target})
    run_cli(["ui", "memory", "record", "gap", "--dir", ".", "--data", payload])


def parse_rust_theme_constants(theme_src: str) -> Dict[str, str]:
    """Extracts `pub const <NAME>: Color32 = rgb(0xRRGGBB);` into lowercase `#rrggbb` strings."""
    pattern = re.compile(
        r"pub\s+const\s+([A-Z0-9_]+)\s*:\s*Color32\s*=\s*rgb\(\s*0x([0-9a-fA-F]{6})\s*\)\s*;"
    )
    extracted: Dict[str, str] = {}
    for match in pattern.finditer(theme_src):
        name = match.group(1)
        hex_val = f"#{match.group(2).lower()}"
        extracted[name] = hex_val
    return extracted


def extract_design_token_colors(tokens_data: Dict[str, Any]) -> Dict[str, str]:
    colors_obj = tokens_data.get("colors", {})
    out: Dict[str, str] = {}
    if isinstance(colors_obj, dict):
        for k, v in colors_obj.items():
            if isinstance(v, dict) and "$value" in v:
                out[k] = str(v["$value"]).strip().lower()
            elif isinstance(v, str):
                out[k] = v.strip().lower()
    return out


def check_rust_token_parity(record_gaps: bool = False) -> Tuple[bool, str, Dict[str, Any]]:
    if not THEME_RS.exists():
        msg = f"Missing Rust theme file: {THEME_RS}"
        if record_gaps:
            record_gap_event(msg)
        return False, msg, {}
    if not TOKENS_JSON.exists():
        msg = f"Missing design tokens file: {TOKENS_JSON}"
        if record_gaps:
            record_gap_event(msg, target="design/design.tokens.json")
        return False, msg, {}

    theme_src = THEME_RS.read_text(encoding="utf-8", errors="replace")
    rust_consts = parse_rust_theme_constants(theme_src)
    tokens_data = json.loads(TOKENS_JSON.read_text(encoding="utf-8"))
    ds_colors = extract_design_token_colors(tokens_data)

    missing_consts: List[str] = []
    purple_violations: List[str] = []
    token_drifts: List[str] = []
    verified_pairs: Dict[str, str] = {}

    for cname in CORE_RUST_CONSTANTS:
        actual_hex = rust_consts.get(cname)
        if not actual_hex:
            missing_consts.append(cname)
            continue
        if actual_hex in BANNED_PURPLES:
            purple_violations.append(f"{cname}={actual_hex}")
        if cname in RUST_TO_DESIGN_TOKEN_MAP:
            ds_key = RUST_TO_DESIGN_TOKEN_MAP[cname]
            expected_hex = ds_colors.get(ds_key)
            if not expected_hex:
                token_drifts.append(f"colors.{ds_key} missing in design.tokens.json")
            elif actual_hex != expected_hex:
                token_drifts.append(f"{cname} ({actual_hex}) != colors.{ds_key} ({expected_hex})")
            else:
                verified_pairs[cname] = f"{actual_hex} == colors.{ds_key}"
        elif cname in EXPECTED_STANDALONE_HEX:
            expected_hex = EXPECTED_STANDALONE_HEX[cname]
            if actual_hex != expected_hex:
                token_drifts.append(f"{cname} ({actual_hex}) != expected ({expected_hex})")
            else:
                verified_pairs[cname] = actual_hex

    # Also verify no banned purple exists anywhere in design/design.tokens.json
    for ds_key, ds_hex in ds_colors.items():
        if ds_hex in BANNED_PURPLES:
            purple_violations.append(f"colors.{ds_key}={ds_hex}")

    non_colour = check_non_colour_parity(theme_src, tokens_data)
    non_colour_issues = (
        non_colour["drifts"] + non_colour["stale_exemptions"] + non_colour["problems"]
    )
    token_drifts.extend(non_colour_issues)

    ok = (not missing_consts) and (not purple_violations) and (not token_drifts)
    details = {
        "non_colour": non_colour,
        "checked_constants": len(CORE_RUST_CONSTANTS),
        "verified_pairs": verified_pairs,
        "missing_constants": missing_consts,
        "purple_violations": purple_violations,
        "token_drifts": token_drifts,
    }
    if ok:
        summary = (
            f"{len(CORE_RUST_CONSTANTS)}/{len(CORE_RUST_CONSTANTS)} Rust theme.rs constants match "
            f"design.tokens.json; 0 Purple Ban violations; "
            f"{len(non_colour['verified'])} non-colour constants match"
        )
        if non_colour["known_drift"]:
            summary += f"; KNOWN DRIFT x{len(non_colour['known_drift'])}: " + " | ".join(
                non_colour["known_drift"]
            )
        return True, summary, details

    issues: List[str] = []
    if missing_consts:
        issues.append(f"missing={missing_consts}")
    if purple_violations:
        issues.append(f"purple_ban_violations={purple_violations}")
    if token_drifts:
        issues.append(f"drifts={token_drifts}")
    summary = "; ".join(issues)
    if record_gaps:
        record_gap_event(
            f"Rust theme.rs <-> design.tokens.json parity violation: {summary}",
            target="crates/designos-app/src/theme.rs",
        )
    return False, summary, details


def check_zero_fake_ui_gate() -> Tuple[bool, str, Dict[str, Any]]:
    code, payload, err = run_cli_json([sys.executable, str(ZERO_FAKE_GATE), "--json"])
    passed = payload.get("passed", 0)
    total = payload.get("total", 0)
    status = payload.get("status", "FAIL")
    ok = (code == 0) and (status == "PASS") and (total > 0) and (passed == total)
    summary = f"{passed}/{total} Zero-Fake-UI invariants PASS (DIR-28 / BR-31 / REQ-25)"
    if not ok and err:
        summary = f"{summary} ({err})"
    return ok, summary, payload


def check_ui_ds_status() -> Tuple[bool, str, Dict[str, Any]]:
    code, payload, err = run_cli_json(["ui", "ds", "status", "--dir", ".", "--json"])
    ok = (code == 0) and bool(payload.get("ok"))
    data = payload.get("data", {})
    if ok:
        comp_count = data.get("componentCount", 0)
        compiled_hash = str(data.get("compiledHash", ""))[:19]
        summary = f"DS manifest active ({comp_count} registered components, hash={compiled_hash}…)"
    else:
        err_msg = payload.get("error", {}).get("message") or err or "ui ds status failed"
        summary = f"DS status error: {err_msg}"
    return ok, summary, payload


def check_ui_ds_a11y() -> Tuple[bool, str, Dict[str, Any]]:
    code, payload, err = run_cli_json(["ui", "ds", "a11y", "--dir", ".", "--json"])
    data = payload.get("data", {})
    failures = data.get("failures", [])
    unresolved = data.get("unresolved", [])
    pairs = data.get("checkedPairs", 0)
    state_pairs = data.get("checkedStatePairs", 0)
    ok = (code == 0) and bool(payload.get("ok")) and len(failures) == 0 and len(unresolved) == 0
    if ok:
        summary = f"0 contrast failures across {pairs} foreground/surface pairs + {state_pairs} state pairs"
    else:
        summary = f"failures={len(failures)}, unresolved={len(unresolved)} ({err})"
    return ok, summary, payload


def check_ui_ds_soul() -> Tuple[bool, str, Dict[str, Any]]:
    code, payload, err = run_cli_json(["ui", "ds", "soul", "check", "--dir", ".", "--json"])
    data = payload.get("data", {})
    findings = data.get("findings", [])
    err_count = data.get("errorCount", 0)
    soul_text = SOUL_MD.read_text(encoding="utf-8", errors="replace") if SOUL_MD.exists() else ""
    ratified = bool(re.search(r"(?m)^status:\s*ratified\b", soul_text))
    ok = (code == 0) and bool(payload.get("ok")) and len(findings) == 0 and err_count == 0 and ratified
    if ok:
        summary = "0 soul findings; design/soul.md status=ratified"
    else:
        summary = f"findings={len(findings)}, errorCount={err_count}, ratified={ratified} ({err})"
    return ok, summary, payload


def check_ui_agents() -> Tuple[bool, str, Dict[str, Any]]:
    code, payload, err = run_cli_json(["ui", "agents", "check", "--dir", ".", "--json"])
    data = payload.get("data", {})
    findings = data.get("findings", [])
    err_count = data.get("errorCount", 0)
    warn_count = data.get("warningCount", 0)

    claude_agents_dir = REPO_ROOT / ".claude" / "agents"
    claude_agent_files = sorted(p.name for p in claude_agents_dir.glob("*.md")) if claude_agents_dir.exists() else []

    cross_agents_dir = REPO_ROOT / ".agents" / "agents"
    missing_cross = [
        name for name in CROSS_RUNTIME_AGENTS if not (cross_agents_dir / name).exists()
    ]

    ok = (
        (code == 0)
        and bool(payload.get("ok"))
        and err_count == 0
        and warn_count == 0
        and len(claude_agent_files) >= 3
        and not missing_cross
    )
    if ok:
        summary = (
            f"{len(claude_agent_files)} stamped .claude/agents + "
            f"{len(CROSS_RUNTIME_AGENTS)} cross-runtime .agents/agents fresh (0 errors)"
        )
    else:
        summary = (
            f"errorCount={err_count}, warningCount={warn_count}, "
            f"claude_agents={len(claude_agent_files)}, missing_cross_runtime={missing_cross}"
        )
    return ok, summary, {
        "ui_agents_check": payload,
        "claude_agents": claude_agent_files,
        "cross_runtime_agents_missing": missing_cross,
    }


def run_self_evolution_pulse(check_only: bool = False) -> Tuple[bool, str, Dict[str, Any]]:
    pulse_steps: Dict[str, Any] = {}
    if not check_only:
        h_code, h_json, h_err = run_cli_json(["design-os", "harvest", "--dir", ".", "--emit-packet", "--json"])
        hb_code, hb_json, hb_err = run_cli_json(["design-os", "heartbeat", "--dir", ".", "--force", "--json"])
        mc_code, mc_out, mc_err = run_cli(["ui", "memory", "compile", "--dir", "."])
        pulse_steps = {
            "harvest": {"exit_code": h_code, "data": h_json, "stderr": h_err},
            "heartbeat": {"exit_code": hb_code, "data": hb_json, "stderr": hb_err},
            "memory_compile": {"exit_code": mc_code, "stdout": mc_out, "stderr": mc_err},
        }

    ev_code, ev_payload, ev_err = run_cli_json(["design-os", "evolution", "--dir", ".", "--json"])
    ev_data = ev_payload.get("data", {})
    verdict = ev_data.get("verdict")
    proof = ev_data.get("proof", {})
    proof_level = proof.get("level")
    proof_findings = proof.get("findings", [])
    ledger_total = ev_data.get("ledger", {}).get("total", 0)
    recurrent_insights = ev_data.get("graph", {}).get("insights_recurrent", 0)

    ok = (
        ev_code == 0
        and bool(ev_payload.get("ok"))
        and verdict == "ALIVE"
        and proof_level in ("APPLIED", "IMPROVING")
        and len(proof_findings) == 0
    )
    mode_label = "check-only" if check_only else "harvest+heartbeat+compile"
    if ok:
        summary = (
            f"verdict={verdict}, proof.level={proof_level}, findings=0, "
            f"ledger_events={ledger_total}, recurrent_insights={recurrent_insights} ({mode_label})"
        )
    else:
        summary = (
            f"verdict={verdict}, proof.level={proof_level}, findings={proof_findings} ({mode_label}; {ev_err})"
        )
    return ok, summary, {"pulse": pulse_steps, "evolution": ev_payload}


def check_ui_gate_specimen() -> Tuple[bool, str, Dict[str, Any]]:
    code, payload, err = run_cli_json(
        ["ui", "gate", "design/specimen.html", "--tokens", "design/design.tokens.json", "--json"]
    )
    data = payload.get("data", {})
    families = data.get("families", {})
    expected_families = ("layout", "a11y", "taste", "tell", "content", "autofix")
    family_errors: Dict[str, int] = {}
    family_warnings: Dict[str, int] = {}
    for fam in expected_families:
        fam_obj = families.get(fam, {})
        family_errors[fam] = int(fam_obj.get("errorCount", 0))
        family_warnings[fam] = int(fam_obj.get("warningCount", 0))

    total_errors = int(data.get("errorCount", sum(family_errors.values())))
    total_warnings = int(data.get("warningCount", sum(family_warnings.values())))
    gate_pass = bool(data.get("pass")) and (code == 0) and (total_errors == 0) and all(
        v == 0 for v in family_errors.values()
    )

    u_code, u_payload, u_err = run_cli_json(
        ["ui", "ds-usage-lint", "design/specimen.html", "--dir", ".", "--json"]
    )
    u_data = u_payload.get("data", {})
    u_errors = int(u_data.get("errorCount", 0))
    usage_ok = (u_code == 0) and bool(u_payload.get("ok")) and (u_errors == 0)

    ok = gate_pass and usage_ok
    if ok:
        summary = (
            f"6/6 families PASS (errors={total_errors}, warnings={total_warnings}); "
            f"ds-usage-lint errors={u_errors}"
        )
    else:
        summary = (
            f"gate_exit={code}, family_errors={family_errors}, warnings={total_warnings}, "
            f"ds_usage_errors={u_errors} ({err or u_err})"
        )
    return ok, summary, {"ui_gate": payload, "ds_usage_lint": u_payload}


def check_design_os_project_audit() -> Tuple[bool, str, Dict[str, Any]]:
    a_code, a_payload, a_err = run_cli_json(["design-os", "audit", ".", "--json"])
    a_data = a_payload.get("data", {})
    a_summary = a_data.get("summary", {})
    tools_run = int(a_summary.get("toolsRun", 0))
    tools_failed = int(a_summary.get("toolsFailed", 0))
    audit_errors = int(a_summary.get("errors", 0))
    audit_warnings = int(a_summary.get("warnings", 0))
    audit_ok = (
        (a_code == 0)
        and bool(a_payload.get("ok"))
        and (tools_run > 0)
        and (tools_failed == 0)
        and (audit_errors == 0)
    )

    nodes_ok = True
    nodes_total = 0
    nodes_payload: Dict[str, Any] = {}
    if STUDIO_UI_NODES_JSON.exists():
        n_code, nodes_payload, _ = run_cli_json(
            [
                "ui",
                "audit",
                "design/studio-ui-nodes.json",
                "--tokens",
                "design/design.tokens.json",
                "--registry",
                "design/component-registry.json",
                "--grid",
                "4",
                "--json",
            ]
        )
        n_data = nodes_payload.get("data", {})
        nodes_total = int(n_data.get("total", len(n_data.get("violations", []))))
        nodes_ok = (n_code == 0) and bool(nodes_payload.get("ok")) and (nodes_total == 0)

    flow_ok = True
    flow_errors = 0
    flow_warnings = 0
    flow_payload: Dict[str, Any] = {}
    if FLOW_JSON.exists():
        f_code, flow_payload, _ = run_cli_json(["ui", "flow", "lint", "design/flow.json", "--json"])
        f_data = flow_payload.get("data", {})
        flow_errors = int(f_data.get("errorCount", 0))
        flow_warnings = int(f_data.get("warningCount", 0))
        flow_ok = (f_code == 0) and bool(flow_payload.get("ok")) and (flow_errors == 0)

    ok = audit_ok and nodes_ok and flow_ok
    if ok:
        summary = (
            f"design-os audit . ({tools_run} tools, errors={audit_errors}, warnings={audit_warnings}); "
            f"nodes violations={nodes_total}; flow errors={flow_errors}"
        )
    else:
        summary = (
            f"audit_exit={a_code}, tools_failed={tools_failed}, audit_errors={audit_errors}, "
            f"nodes_violations={nodes_total}, flow_errors={flow_errors} ({a_err})"
        )
    return ok, summary, {
        "project_audit": a_payload,
        "studio_ui_nodes_audit": nodes_payload,
        "flow_lint": flow_payload,
    }


def main() -> int:
    parser = argparse.ArgumentParser(
        description="design:os studio — Automated Self-Evolution Engine & Rust <-> design:os Token Bridge"
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="Read-only check mode (skips state-mutating harvest, heartbeat --force, and memory compile)",
    )
    parser.add_argument(
        "--evolve",
        action="store_true",
        help="Run full harvest + heartbeat --force + memory compile pulse (default unless --check is passed)",
    )
    parser.add_argument(
        "--record-gaps",
        action="store_true",
        help="Automatically record any detected Rust <-> design.tokens.json drift via `ui memory record gap`",
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="Emit machine-readable JSON summary report",
    )
    args = parser.parse_args()

    check_only = bool(args.check) and not bool(args.evolve)

    # Execute G08 and G09 before G07 so that in evolve mode G07 harvests the latest gate/audit events
    execution_order = [
        ("G01", "check_rust_token_parity", "Rust theme.rs <-> design.tokens.json Parity & Purple Ban", lambda: check_rust_token_parity(record_gaps=args.record_gaps)),
        ("G02", "zero_fake_ui_gate", "Zero Fake UI Deterministic Gate (all invariants)", check_zero_fake_ui_gate),
        ("G03", "ui_ds_status", "design:os Compiled Manifest & Component Registry", check_ui_ds_status),
        ("G04", "ui_ds_a11y", "WCAG Contrast Audit (ui ds a11y)", check_ui_ds_a11y),
        ("G05", "ui_ds_soul_check", "Ratified Soul Integrity (ui ds soul check)", check_ui_ds_soul),
        ("G06", "ui_agents_check", "Roster & Cross-Runtime Design Agents Parity", check_ui_agents),
        ("G08", "ui_gate_specimen", "6-Family Specimen Gate & DS Usage Lint (design/specimen.html)", check_ui_gate_specimen),
        ("G09", "design_os_project_audit", "Full Repository Audit, Node Tree & Flow Lint (design-os audit .)", check_design_os_project_audit),
        ("G07", "design_os_evolution", "Self-Evolution Loop Proof (design-os evolution)", lambda: run_self_evolution_pulse(check_only=check_only)),
    ]

    results_by_id: Dict[str, Dict[str, Any]] = {}
    for gid, key, title, fn in execution_order:
        passed, summary, detail = fn()
        results_by_id[gid] = {
            "id": gid,
            "gate": key,
            "title": title,
            "passed": passed,
            "status": "PASS" if passed else "FAIL",
            "summary": summary,
            "detail": detail,
        }

    results: List[Dict[str, Any]] = [results_by_id[k] for k in sorted(results_by_id.keys())]

    passed_count = sum(1 for r in results if r["passed"])
    total_count = len(results)
    all_passed = passed_count == total_count

    if args.json:
        report = {
            "engine": "design_os_self_evolve",
            "mode": "check" if check_only else "evolve",
            "passed": passed_count,
            "total": total_count,
            "status": "PASS" if all_passed else "FAIL",
            "gates": results,
        }
        print(json.dumps(report, indent=2))
    else:
        print("=" * 116)
        print(
            f" design:os studio — Self-Evolution Engine & Live Token Parity Audit (mode={'CHECK' if check_only else 'EVOLVE'})"
        )
        print("=" * 116)
        print(f"{'ID':<4} | {'STATUS':<6} | {'GATE':<24} | {'EVIDENCE & VERDICT'}")
        print("-" * 116)
        for r in results:
            print(f"{r['id']:<4} | {r['status']:<6} | {r['gate']:<24} | {r['summary']}")
        print("-" * 116)
        verdict = "PASS" if all_passed else "FAIL"
        print(f" Overall Self-Evolution Verdict: {verdict} ({passed_count}/{total_count} gates passed)")
        print("=" * 116)

    return 0 if all_passed else 1


if __name__ == "__main__":
    sys.exit(main())
