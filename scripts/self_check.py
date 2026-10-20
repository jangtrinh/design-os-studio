#!/usr/bin/env python3
"""self_check.py — Deterministic offline self-check for design-os-studio."""

from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

REQUIRED_JSON = (
    ".claude-plugin/plugin.json",
    ".claude-plugin/marketplace.json",
    ".codex-plugin/plugin.json",
    ".agents/plugins/marketplace.json",
    ".factory-plugin/plugin.json",
    ".factory-plugin/marketplace.json",
)

REQUIRED_FILES = (
    "Cargo.toml",
    "README.md",
    "LICENSE",
    "PRIVACY.md",
    "SECURITY.md",
    "CONTRIBUTING.md",
    "CODE_OF_CONDUCT.md",
    "CHANGELOG.md",
    "llms.txt",
    "docs/index.html",
    "docs/_config.yml",
    "docs/_layouts/default.html",
    "docs/assets/site.css",
    "docs/robots.txt",
    "docs/sitemap.xml",
    "docs/llms.txt",
    "docs/cookbook.md",
    "docs/install.md",
    "docs/releasing.md",
    "commands/doctor.md",
    "prompts/doctor.md",
)

REQUIRED_CRATES = (
    "crates/designos-doc/Cargo.toml",
    "crates/designos-fig/Cargo.toml",
    "crates/designos-render/Cargo.toml",
    "crates/designos-gpu/Cargo.toml",
    "crates/designos-app/Cargo.toml",
    "crates/designos-library/Cargo.toml",
    "crates/designos-fonts/Cargo.toml",
    "crates/designos-export/Cargo.toml",
    "crates/designos-svg/Cargo.toml",
    "crates/designos-diff/Cargo.toml",
    "crates/designos-mcp/Cargo.toml",
)

REQUIRED_IMAGES = (
    "docs/images/designos-hero.png",
    "docs/images/designos-agents-chat.png",
    "docs/images/designos-spec-reader.png",
    "docs/images/designos-assets-panel.png",
    "docs/images/designos-actions-cmdk.png",
    "docs/images/designos-devmode.png",
    "docs/images/designos-variables.png",
    "docs/images/designos-components.png",
    "docs/images/designos-prototype.png",
    "docs/images/designos-comments.png",
    "docs/images/designos-main-menu.png",
    "docs/images/designos-color-picker.png",
    "docs/images/designos-agent-spec.png",
    "docs/images/designos-overview.png",
    "docs/images/designos-home.png",
)


def main() -> int:
    issues: list[str] = []
    for rel in REQUIRED_JSON:
        p = ROOT / rel
        if not p.is_file():
            issues.append(f"missing JSON manifest: {rel}")
            continue
        try:
            data = json.loads(p.read_text(encoding="utf-8"))
            if not isinstance(data, dict):
                issues.append(f"manifest is not a JSON object: {rel}")
        except Exception as exc:
            issues.append(f"invalid JSON in {rel}: {exc}")

    for rel in REQUIRED_FILES:
        p = ROOT / rel
        if not p.is_file() or p.stat().st_size == 0:
            issues.append(f"missing or empty required file: {rel}")

    for rel in REQUIRED_CRATES:
        p = ROOT / rel
        if not p.is_file() or p.stat().st_size == 0:
            issues.append(f"missing or empty workspace crate manifest: {rel}")

    for rel in REQUIRED_IMAGES:
        p = ROOT / rel
        if not p.is_file() or p.stat().st_size == 0:
            issues.append(f"missing or empty visual proof image: {rel}")

    skills = list((ROOT / "skills").glob("*/SKILL.md")) if (ROOT / "skills").is_dir() else []
    if not skills:
        issues.append("missing skills/<slug>/SKILL.md")

    gate_script = ROOT / "scripts" / "zero_fake_ui_gate.py"
    if not gate_script.is_file():
        issues.append("missing scripts/zero_fake_ui_gate.py")
    else:
        import subprocess
        proc = subprocess.run(
            [sys.executable, str(gate_script), "--json"],
            cwd=str(ROOT),
            capture_output=True,
            text=True,
        )
        if proc.returncode != 0:
            issues.append(f"zero_fake_ui_gate.py failed (exit {proc.returncode}): {proc.stdout or proc.stderr}")

    if issues:
        for item in issues:
            print(f"FAIL: {item}", file=sys.stderr)
        return 1

    print("ok: design-os-studio self-check passed (design-os-studio)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
