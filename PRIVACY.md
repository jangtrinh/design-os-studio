# Privacy Policy (`design-os-studio`)

`design-os-studio` (**DESIGN:OS Studio**) is a local-first open-source interface design studio, Figma `.fig` v106 binary engine, and AI agent MCP toolchain in the **DESIGN:OS** ecosystem.

## Zero Telemetry Guarantee

- **No telemetry, no document uploads**: `designos-studio`, `designos`, `designos-mcp`, and `designos-diff` do not phone home, collect analytics, or transmit `.fig` / `.designos` documents, prompts, or generated artifacts to any external server.
- **Two on-demand network fetches, both user-triggered by content or action**:
  - **Fonts**: when a document uses a font family that is not bundled (only Inter is bundled), the Studio downloads it from Fontsource (`api.fontsource.org`) and caches it on disk. Only the font family is looked up; no document content is sent. Offline, or if the fetch fails, text falls back to Inter.
  - **Plugins**: installing a plugin downloads it from GitHub. Nothing is fetched unless you install one.
  - Agent CLIs you launch in the Agents rail (`claude`, `codex`, `agy`, `grok`) use their own network connections under their own terms.
- **No third-party tracking**: There are no embedded telemetry beacons, crash reporters, or usage trackers in `crates/`, `skills/`, `scripts/`, or plugin manifests.
- **Local execution**: All scene-graph rendering (`wgpu` / `tiny-skia`), `.fig` binary encoding/decoding, MCP tool invocations, and Agentic PTY Chat sessions (`claude`, `codex`, `agy`) happen strictly on your local workstation.
- **Auditable source**: Every crate, shader, script, and skill in this repository is open-source Rust, WGSL, Python, and Markdown under the dual [MIT](LICENSE-MIT) / [Apache-2.0](LICENSE-APACHE) ([`LICENSE`](LICENSE)) license.

## Contact

For privacy or security questions, see [`SECURITY.md`](SECURITY.md) or open an issue on [`jangtrinh/design-os-studio`](https://github.com/jangtrinh/design-os-studio).
