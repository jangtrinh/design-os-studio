---
description: Diagnose DESIGN:OS Studio installation, Cargo workspace, plugin manifests, skill files, and local verification gates
allowed-tools: Bash, Read, Glob
---

# `design-os-studio` `/doctor` Diagnostics

Run a read-only diagnostic check of the `design-os-studio` installation, Cargo workspace manifests, visual proof assets, and repository health. Do **not** modify, create, or delete any files during this check.

## Steps

1. **Run the deterministic self-check script** when inside a `design-os-studio` checkout:
   ```bash
   python3 scripts/self_check.py
   ```
2. **Check installed skill and binary locations** across active agent harnesses:
   - Claude Code: `~/.claude/skills/design-os-studio/SKILL.md` or `.claude-plugin/marketplace.json`
   - OpenAI Codex: `~/.codex/skills/design-os-studio/SKILL.md` or `.codex-plugin/plugin.json`
   - Gemini CLI / Antigravity: `~/.gemini/skills/design-os-studio/SKILL.md`
   - Cross-agent `.agents`: `~/.agents/skills/design-os-studio/SKILL.md`
   - Native binaries: `designos-studio`, `designos`, `designos-mcp`, `designos-diff` in `~/.local/bin`, `/Applications/designos.app`, or `target/release/`
3. **Report a concise health table** covering:
   - Cargo workspace integrity (`Cargo.toml` and 11 crates under `crates/`)
   - Plugin manifest integrity (`.claude-plugin`, `.codex-plugin`, `.agents/plugins`, `.factory-plugin`)
   - Canonical skill (`skills/design-os-studio/SKILL.md`)
   - Trust & privacy governance (`PRIVACY.md`, `SECURITY.md`, `LICENSE`)
   - Visual proof gallery (`docs/images/designos-*.png`)
