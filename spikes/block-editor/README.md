# Block editor spike (THROWAWAY)

**THROWAWAY.** Answers one question for issue #1526 / epic #1522: can the Notion-style block editor be
built in egui, and how? It is not production code and must not be imported by `crates/`. Delete the folder
once the build starts. Findings live in `plans/reports/spike-1526-block-editor.md`.

It is not a member of the Studio workspace: its `Cargo.toml` has an empty `[workspace]` table, so no change to
the root `Cargo.toml` is needed. `Cargo.lock` was seeded from the workspace lock so egui/eframe are the same
0.35.0 as Studio. `designos-app` is bin-only, so the spike copies the few theme constants it needs and
takes fonts from the real `designos-fonts` crate.

## Run

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd spikes/block-editor
cargo run --release -- --mode custom      # own input over egui galleys (recommended path)
cargo run --release -- --mode textedit    # one egui TextEdit per block
```

Flags: `--script` (scripted scenario through `raw_input_hook`, prints PASS/FAIL, exits), `--shots DIR`
(PNG output for `--script` and for Cmd+Shift+P), `--log-ime` (print raw Ime/Text/Key events).

```bash
cargo test                                                  # model, history, slash unit tests
cargo run -- --mode custom --script --shots /tmp/shots      # 22 checks (a)-(f)
cargo run -- --mode textedit --script --shots /tmp/shots    # 22 checks, d1 fails by design (see report)
```

## Keys to try

Arrows across blocks, Enter (split), Backspace at start (merge), `/` (menu: arrows, Enter, Esc), Cmd+B on a
selection (bold), Cmd+Z / Cmd+Shift+Z, click a checkbox, hover a block for the `+` button.

## Not built

Block handle drag and menu, inline toolbar and link popover, Markdown shortcuts, list indent, tables,
embeds, Mermaid, selection across blocks, multi-click selection, accessibility, save and conflict handling.
