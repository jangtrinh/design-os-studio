# Spike #1527: Mermaid diagrams (flowchart + sequence), offline, off the UI thread

Epic #1522, Phase 05, es:feature-dev Phase 2.5. Branch `spike/1527-mermaid` from origin/main `53860a64`. Prototype: `spikes/mermaid/` (THROWAWAY, own `[workspace]`, no change to `crates/**`, root `Cargo.toml` or `Cargo.lock`). Evidence: `plans/reports/spike-1527-evidence/`. Machine: Apple M4 Pro, macOS 26.6.2, rustc 1.95.0 for the spike (repo pin is 1.93.1).

## NOT verified (read first)

- Option (a) native subset and option (c) JS engine were NOT prototyped. They are rejected on measured facts about the candidates plus estimates (INFERENCE).
- The design-studio reference capture shows only the card header "Feedback review flow"; the diagram body is below the fold. The source in `flow-1-...reconstructed.mmd` is RECONSTRUCTED from the page text ("New, Triaged, Planned, and Resolved"). No true visual comparison of that diagram exists yet. The real source is needed for the Phase 05 acceptance capture.
- Not wired into egui. The off-UI-thread pattern is proven with `std::thread` + channel (the worker did all parse, layout, SVG and raster; the polling thread stayed free). Texture upload, crossfade and accesskit labels are untested.
- Determinism checked only as byte-identical PNGs across two runs on this machine (3 pairs). Cross-machine and cross-OS golden stability is NOT verified.
- merman text-width layout uses its built-in deterministic metrics, not Inter. No clipping was seen in 5 diagrams, but the `system-fonts` real-font measurement mode was not tried.
- Zed using merman is a README claim (ASSUMPTION), not checked.
- Multi-line `accDescr { ... }` block syntax was not tested.

## Answer to (a) / (b) / (c)

**(b), an existing Rust crate, wins, specifically `merman` 0.9.3, with a toolchain bump to Rust 1.95+.**

| Option | Candidate | What was done | Verdict |
|---|---|---|---|
| (b) | **merman 0.9.3** (MIT OR Apache-2.0) | RAN. Built, rendered 5 diagrams + 4 error sources, offline, timed | **Recommended** |
| (b) | mermaid-rs-renderer 0.3.1 "mmdr" (MIT) | RAN. Same | Rejected: silently renders wrong diagrams from invalid source |
| (b) | merman 0.6.2 (last MSRV 1.87 line) | RAN on 1.93.1 as a hedge against the toolchain bump | Rejected: sequence diagrams ignore the theme and lose labels; errors are Debug dumps of parser internals |
| (b) | flowmaid 0.33.0 (GPL-3.0-or-later) | READ crates.io | Rejected on licence (INFERENCE: copyleft vs the repo's PolyForm Shield licence) |
| (b) | selkie-rs 0.3.0 (MIT), fermaid 0.5.1, ariel-rs 0.4.0, mermaid-little 11.14.0-6 | READ crates.io metadata only | Not run. selkie last release 2026-02-07 (9 months stale); the other three have 143 to 853 downloads |
| (a) | native subset + layered-graph crate (rust-sugiyama 0.4.0 MIT, dagre 0.1.2 Apache-2.0, layout-rs 0.1.3 MIT) | READ crates.io only | Rejected: INFERENCE 8 to 12 d. Parser, 7+ shapes, edge routing, labels, subgraphs, and sequence frames (alt/loop/note/activate/autonumber) all by hand. merman-render alone is 214,776 Rust lines (crates.io linecounts) |
| (c) | mermaid.js 12.2.0 in rquickjs 0.14.0 (MIT) / boa_engine 0.22.0 | FETCHED the bundle and counted DOM calls | Rejected: `mermaid.min.js` is 5,494,801 bytes and calls `getBBox` 98 times, `getComputedTextLength` 10 times, `createElementNS` 11 times. QuickJS and boa have no DOM or SVG text metrics, so we would write a DOM shim that re-implements text measurement, or ship a webview. Not prototyped |

### Comparison of the two candidates that ran (FACT unless marked)

Timings: release build, Apple M4 Pro, median of 20 warm runs; "cold" is the first call in a fresh process; raster = resvg 0.45 at 2x. `rss` is process peak (floor with font loaded: 4.8 MB). Noise between runs is roughly 10 to 30 percent (mmdr flow-2 cold read 25 ms once and 14 ms twice).

| Diagram | mmdr cold / warm / raster / peak RSS | merman cold / warm / raster / peak RSS |
|---|---|---|
| flow-1 feedback review (reconstructed) | 35.1 / 33.1 / 3.2 ms / 17.7 MB | 4.3 / 1.4 / 4.0 ms / 23.0 MB |
| flow-2 checkout payment | 14.3 / 13.0 / 2.3 ms / 14.9 MB | 2.9 / 1.3 / 2.9 ms / 19.4 MB |
| flow-3 subgraphs + shapes | 33.4 / 32.6 / 6.6 ms / 24.6 MB | 4.6 / 2.8 / 7.7 ms / 29.8 MB |
| seq-1 open doc (alt, autonumber) | 0.75 / 0.42 / 8.9 ms / 28.0 MB | 3.5 / 1.9 / 20.4 ms / 33.0 MB |
| seq-2 agent co-editing (loop, note, activate) | 0.59 / 0.29 / 8.9 ms / 28.2 MB | 3.2 / 1.6 / 18.6 ms / 31.8 MB |

Stress (one run each, not a benchmark): 200-node chain flowchart: mmdr 952 ms layout, 100 MB; merman 21 ms layout, 74 ms raster, 155 MB. 1000 nodes: merman 89 ms layout, 412 ms raster, 722 MB peak (PNG 447 x 186,118 px); mmdr did not finish within 10 s. Conclusion: any build needs source-size, node-count and raster-pixel caps plus cancellation.

| Criterion | mmdr 0.3.1 | merman 0.9.3 |
|---|---|---|
| Invalid source (4 samples) | Rendered a picture for 3 of 4 (FACT). The broken `--> -->` line produced a PNG byte-identical to the correct diagram (same md5). The unterminated `[Payment` label drew a node literally named `pay[Payment`. The bad sequence arrow `B=>>A` silently dropped a message. Only the unclosed subgraph errored (line 3). Its own validator source says coverage is "deliberately narrow, five detection paths" | Errored on 4 of 4 with line numbers via the byte span: line 4 for `--> -->` (the exact "Line 4: unexpected token" case in the mockup), line 5 for the `B=>>A` arrow. Unclosed constructs are reported at end of input (line 5 for a subgraph opened at line 3), as upstream Mermaid does |
| Flowchart look | Cleaner: rounded corners, orthogonal edges, labels on a chip | Straight and curved edges, edge-label background nearly invisible so labels sit on the line, subgraph title overlaps the inbound edge (mmdr has a similar title misplacement) |
| Sequence correctness | `loop` frame mis-scoped: it wraps the first message, `apply(edit)` and the note instead of only the loop body (seq-2 sheet) | Correct frame scoping, activation bars, notes, autonumber (after the font fix below) |
| Unsupported types | Renders 23 types, so Studio must gate (done in the prototype front door) | Same |
| accTitle / accDescr | Parses the lines, ignores them (no `<title>`/`<desc>` in the SVG) | Emits `<title>`, `<desc>`, `aria-labelledby`, `aria-describedby` natively (FACT, `RAW_ACC=1` run) |
| Cancel, resource limits | None seen | `OperationControl` cancellation and typed `ResourceLimitExceeded` errors (source reading, not exercised) |
| Offline | No network crate in tree; ran under `sandbox-exec (deny network*)`: ok | Same |
| Build on repo toolchain 1.93.1 | Builds (exit 0) | Fails: `merman`, `merman-core`, `merman-render`, `dugong`, `dugong-graphlib` require rustc 1.95; edition 2024 |
| Stripped binary delta (on a 3.4 MB base with resvg + Inter) | +3.8 MB | +8.2 MB |
| Crates added (unique, normal deps; base 61) | +25 (86 total) | +121 (182 total) |
| Licences of added crates | MIT, Apache-2.0, Unlicense, Unicode-3.0 only | MIT/Apache-2.0 plus 3 MPL-2.0 (`cssparser`, `cssparser-macros`, `dtoa-short`; not in the repo's `Cargo.lock` today), Unicode-3.0 (ICU), Zlib. ELK (EPL-2.0) is NOT pulled with default features off |
| Maintenance | 7 versions, last 2026-07-06, ~232k downloads, 24 open issues (README) | 27 versions, last 2026-10-30, ~210k downloads, 4 open issues (README), single maintainer |
| Determinism | Byte-identical across runs; `fast_text_metrics = true` is required (otherwise it loads system fonts and writes a metrics cache under `~/.cache`) | Byte-identical across runs; built-in text metrics by default |

## Recommendation

Use **merman 0.9.3, pinned with `=`**, behind a thin adapter in the diagram module (as the phase's Files section already allows):

- Features: `svg`, `diagram-flowchart`, `diagram-sequence` only. No ELK, no math, no analysis.
- Parse with `ParseOptions::strict()`; map `RenderError::Parse` to "Line N: message" by converting the byte span to a line; keep the engine message as is.
- Theme via the host site config (`theme: base` + `themeVariables` built from `theme.rs` constants, not copied hex) and `SvgPipeline::resvg_safe()`; set `htmlLabels:false` so labels are plain `<text>`.
- Rasterise on the worker with resvg/tiny-skia at card width x DPR, Inter bundled in the fontdb, generic families mapped to Inter. Cache by hash(source, theme, dpr). Hard caps on source bytes, node count and raster pixels. Honour the spec's 300 ms debounce and "keep last good render at 50 % opacity".
- Studio-owned front door decides support: `flowchart`/`graph` and `sequenceDiagram` only; everything else shows the source plus "Diagram type not supported yet · flowchart and sequence work" (verified path in the prototype, both engines would otherwise render it).

Why not mmdr: requirement 3 ("syntax errors show the error line, keep the last good render") and the repo law "a wrong fact is worse than an absent one" are violated by a renderer that draws a picture for invalid source. Fixing that means writing a second, strict parser in Studio (INFERENCE: +2 d, unbounded) and its sequence `loop` scoping bug stays. Its advantages (builds on 1.93.1, +4 MB less, cleaner flowcharts, all-permissive licences) do not outweigh that.

Fallback if the owner refuses the toolchain bump: mmdr + Studio-side strict validator, with the extra cost above.

## accTitle / accDescr to the card

- The source stays the single truth (round-trips through the MCP doc tools). Studio parses the two lines itself; the engine also accepts them untouched (verified), so the source can go to merman verbatim and error line numbers stay equal to the source pane.
- Header text (spec §7): `accTitle` else "Diagram". Mockup M09 shows "Checkout payment flow" (accTitle) and "Diagram" (gantt, none).
- Accessible label: `accTitle` + ". " + `accDescr` on the preview image widget (egui accesskit labelled Image: ASSUMPTION, not checked against egui 0.31). Without `accDescr`: title only.
- The prototype extracts single-line forms only; the block form `accDescr { ... }` needs handling in the build.

## Risks

1. Toolchain bump 1.93.1 to 1.95+ is repo-wide (`rust-toolchain.toml`, CI, new clippy lints). merman cannot be isolated from it. The 0.6.2 hedge failed (two observations: labels vanished with HTML labels; with `htmlLabels:false` the flowchart showed labels but sequence diagrams ignored the theme and lost labels).
2. Single-maintainer dependency with fast release cadence and an API break between 0.8 and 0.9 (upgrade guide exists). Pin exactly; budget a re-check on each bump.
3. +8.2 MB and +121 crates; 3 new MPL-2.0 crates need `THIRD_PARTY_LICENSES.md` handling (file-level copyleft, INFERENCE: fine unmodified, owner/legal call).
4. SVG is written for browsers. Seen under resvg 0.45: edge-label backgrounds nearly invisible, subgraph title overlapping the inbound edge. Autonumber digits looked missing at first; that was the prototype's raster setup (generic `sans-serif` unmapped in a system-font-free fontdb), fixed by mapping generics to Inter. A host-CSS pass (`ScopedCssPostprocessor`) exists for the rest; not tried for the label background.
5. Pathological input: raster of a 1000-node chain was 722 MB. Caps are mandatory.
6. Large legit diagrams do not fit a 420-high card (spec §6 uses max height 420 for embeds; §7 states none for diagrams).

## Estimate for the 3 d build: revise to 4 d (3.5 d if the bump is a separate PR)

| Item | d |
|---|---|
| Toolchain bump PR (CI, lints) | 0.25 |
| Adapter: front door, strict parse, error to line, acc* extraction | 0.5 |
| Worker: latest-wins cancel, 300 ms debounce, caps, cache | 0.5 |
| Theme from `theme.rs` + resvg SVG polish (label bg, subgraph title) | 0.75 |
| Card UI `project_tab/diagram.rs`: header, edit pane, error strip, 50 % last-good, unsupported notice, texture, a11y label | 0.75 |
| Tests: 5 golden PNGs with tolerance, error, unsupported, acc mapping; THIRD_PARTY_LICENSES | 0.5 |
| 1440 capture next to `ds-project-context-doc.jpg` | 0.25 |
| **Total** | **3.5 + 0.25** |

These are INFERENCE from the prototype's friction points, not measured.

## Evidence

- `plans/reports/spike-1527-evidence/compare-feedback-review-flow-vs-design-studio.png`: design-studio header crop | mmdr | merman, same reconstructed source.
- `compare-flowcharts-mmdr-vs-merman.png`, `compare-sequence-mmdr-vs-merman.png`: side-by-side per diagram.
- `individual/`: 13 PNGs (5 diagrams x 2 engines, plus 3 mmdr outputs for invalid source showing the silent wrong render). All under 300 KB. `results.jsonl`: raw per-run metrics.
- All were opened and looked at.

## Commands run (exit codes)

- `cd spikes/mermaid && cargo build --release` exit 0 (rustc 1.95.0). `cargo build` (dev) exit 0.
- `cargo +1.93.1 build --release --no-default-features --features mmdr` exit 0.
- `cargo +1.93.1 build --release --no-default-features --features merman`: fails, "rustc 1.93.1 is not supported by ... merman@0.9.3 requires rustc 1.95".
- `cargo metadata --no-deps` at the repo root: exit 0 (workspace unaffected).
- Offline: `./measure-footprint.sh` ran both engines under `sandbox-exec -p '(version 1)(allow default)(deny network*)'`: ok. The same sandbox makes `curl https://crates.io/` fail (exit 6) while unsandboxed it reaches the host.

## Unresolved questions for the owner

1. Is a repo-wide toolchain bump from 1.93.1 to 1.95+ acceptable for this feature? (Decides merman vs the mmdr fallback.)
2. Are three MPL-2.0 crates (`cssparser` family) and +8 MB installer size acceptable?
3. Error copy: show the engine message after "Line N:" (e.g. "Line 4: unexpected edge operator; expected node identifier"), instead of the mockup's generic "unexpected token"? Unclosed blocks are reported at the last line, not the opening line.
4. What happens when a diagram is taller than the card: cap and scroll, pan/zoom, or an "Open full size" action? Max height is not specified in §7.
5. v1 allow-list: confirm `flowchart`/`graph` and `sequenceDiagram` only, even though the engine could draw state, class, ER and gantt.
6. Please supply the real "Feedback review flow" Mermaid source (the capture hides the diagram body) for the acceptance capture.

Status: DONE_WITH_CONCERNS
