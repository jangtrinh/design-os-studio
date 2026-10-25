# Privacy Policy (`design-os-studio`)

`design-os-studio` (**DESIGN:OS Studio**) is a local-first, source-available interface design studio that opens and saves Figma `.fig` files and ships an MCP server for coding agents in the **DESIGN:OS** ecosystem.

## No telemetry

- **No telemetry or tracking.** `designos-studio`, `designos`, `designos-mcp` and `designos-diff` do not collect analytics, and they never send your `.fig` or `.designos` files, prompts or generated files to a server. They contain no telemetry beacons, crash reporters or usage trackers.
- **The network requests Studio makes:**
  - font downloads from the Fontsource API when a file uses fonts you do not have
  - a check of a public status page after an agent provider error
  - a connection check when you click Retry connection after an agent connection error
  - plugin downloads from GitHub that you start
- **Agent CLIs are yours.** The Agents panel starts the agent CLI you choose (Claude Code, Codex, Grok, Antigravity or Gemini CLI). Each one signs in with your own account and sends what you ask it to its own provider, under that provider's terms. Studio does not proxy or record that traffic.
- **Local work.** Rendering (`wgpu` and `tiny-skia`), `.fig` reading and writing, and MCP tool calls run on your machine.
- **Auditable source.** Every crate, shader, script and skill in this repository is source-available under the [PolyForm Shield 1.0.0](LICENSE) license. Third-party files keep their own licenses (see [`THIRD_PARTY_LICENSES.md`](THIRD_PARTY_LICENSES.md)).

## Contact

For privacy or security questions, see [`SECURITY.md`](SECURITY.md) or open an issue on [`jangtrinh/design-os-studio`](https://github.com/jangtrinh/design-os-studio).
