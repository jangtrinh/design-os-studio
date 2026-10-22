# Privacy Policy (`design-os-studio`)

`design-os-studio` (**DESIGN:OS Studio**) is a local-first, source-available interface design studio, Figma `.fig` v106 binary engine, and AI agent MCP toolchain in the **DESIGN:OS** ecosystem.

## Zero Telemetry Guarantee

- **No network calls by the Studio or CLI**: `designos-studio`, `designos`, `designos-mcp`, and `designos-diff` do not phone home, collect analytics, or transmit `.fig` / `.designos` documents, prompts, or generated artifacts to any external server.
- **No third-party tracking**: There are no embedded telemetry beacons, crash reporters, or usage trackers in `crates/`, `skills/`, `scripts/`, or plugin manifests.
- **100% local execution**: All scene-graph rendering (`wgpu` / `tiny-skia`), `.fig` binary encoding/decoding, MCP tool invocations, and Agentic PTY Chat sessions (`claude`, `codex`, `agy`) happen strictly on your local workstation.
- **Auditable source**: Every crate, shader, script, and skill in this repository is source-available Rust, WGSL, Python, and Markdown under the [PolyForm Shield 1.0.0](LICENSE) license. Third-party files keep their own licenses (see [`THIRD_PARTY_LICENSES.md`](THIRD_PARTY_LICENSES.md)).

## Contact

For privacy or security questions, see [`SECURITY.md`](SECURITY.md) or open an issue on [`jangtrinh/design-os-studio`](https://github.com/jangtrinh/design-os-studio).
