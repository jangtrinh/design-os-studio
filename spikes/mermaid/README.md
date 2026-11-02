# THROWAWAY: Mermaid renderer spike (issue #1527)

Not production code. Not a workspace member (its own empty `[workspace]` table detaches it;
the repo root `Cargo.toml` is untouched). Delete this folder when the build phase starts.
Findings and recommendation: `plans/reports/spike-1527-mermaid.md`.

Question answered: how do we render Mermaid flowchart + sequence offline, off the UI thread,
theme-matched to Studio's neutral tokens (no purple)?

## Run

```sh
export PATH="$HOME/.cargo/bin:$PATH"
cd spikes/mermaid                      # rust-toolchain.toml pins 1.95.0 here (merman needs it)
cargo build --release                  # exit 0
./run-all.sh out                       # 7 fixtures x 2 engines -> out/*.svg, out/*.png, out/results.jsonl
./run-errors.sh out-err                # error path + unsupported-type path, one line each
./measure-footprint.sh out-footprint   # stripped binary size per engine, crate counts, offline run under a network-deny sandbox (macOS)
./licence-audit.sh                     # licences of the crates each engine adds
python3 -I compose-evidence.py out <reference.jpg> <dest-dir>   # side-by-side sheets
```

Build only `mermaid-rs-renderer` on the repo's pinned 1.93.1 (merman cannot build there):

```sh
cargo +1.93.1 build --release --no-default-features --features mmdr
```

## Layout

| Path | Role |
|---|---|
| `src/front_door.rs` | Studio-owned type allow-list (flowchart, sequence), `accTitle`/`accDescr` extraction |
| `src/engines.rs` | Adapters: source to themed SVG (`mmdr`, `merman`, `merman06`), error + line mapping |
| `src/studio_theme.rs` | Studio neutral dark tokens copied from `crates/designos-app/src/theme.rs` |
| `src/raster.rs` | SVG to PNG via resvg 0.45 with the bundled Inter (same usvg major as `crates/designos-svg`) |
| `src/main.rs` | One process per (engine, fixture): worker thread, timings, peak RSS, one JSON line |
| `fixtures/` | 3 flowcharts, 2 sequence diagrams, 4 syntax-error sources, 1 unsupported type |

`flow-1-feedback-review-flow-reconstructed.mmd` is RECONSTRUCTED from the design-studio
capture's page text ("New, Triaged, Planned, and Resolved"); the capture shows only the card
header, not the diagram body.

Set `RAW_ACC=1` to hand the engine the untouched source (with `accTitle`/`accDescr` lines).
