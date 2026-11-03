# Report: Move `create_text_style_from_nodes` and `detach_text_style` into `designos-doc` (`crates/designos-doc/src/text_style_ops.rs:1-249`)

## 0. What Could NOT Be Verified First

1. **Full-suite `cargo test -p designos-app --bin designos --locked` exit code `0` in a single un-filtered invocation under macOS `/var/folders/...` `TMPDIR` (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/04-test-app-bin.log:4312-4315`):**
   - In the full parallel binary run (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/04-test-app-bin.log:4312`), `3` (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/04-test-app-bin.log:4312`) pre-existing tests outside `text_style_ops` (`crates/designos-doc/src/text_style_ops.rs:1-249`) failed:
     1. `agent_approval_channel::tests::the_production_root_sits_outside_every_agent_writable_root` (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/04-test-app-bin.log:4290-4294`) panicked at `crates/designos-app/src/agent_approval_channel.rs:286:9` (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/04-test-app-bin.log:4292`) because macOS default `TMPDIR` (`/var/folders/43/75tqcbln0fn_35rs8h6bzl6c0000gn/T/x.IKWOmO/...`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/04-test-app-bin.log:4293`) produces a `130`-byte (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/04-test-app-bin.log:4293`) Unix domain socket path exceeding the `103`-byte `SUN_PATH_MAX` limit (`crates/designos-app/src/agent_approval_channel.rs:286`). When solo-rerun with default `TMPDIR` it failed identically (`130` bytes, `rc=101`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/04b-test-app-bin-reruns.log:13-30`), and when solo-rerun with `TMPDIR=/tmp` it passed (`1 passed; 0 failed`, `rc=0`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/04b-test-app-bin-reruns.log:71-75`).
     2. `shell::tests::close_human_dirty::library_file_close_save_writes_the_edit_and_closes_not_path_required` (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/04-test-app-bin.log:4296-4300`) timed out waiting for the background editor notification under full CPU parallelism (`EDITOR_NOT_NOTIFIED`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/04-test-app-bin.log:4299`), and passed on solo rerun (`1 passed; 0 failed`, `rc=0`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/04b-test-app-bin-reruns.log:41-45`).
     3. `vecops::tests::two_deferred_text_commands_both_convert_in_order` (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/04-test-app-bin.log:4301-4305`) hit its font-load deadline under full CPU parallelism (`both texts still waiting for slow fonts`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/04-test-app-bin.log:4304`), and passed on solo rerun (`1 passed; 0 failed`, `rc=0`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/04b-test-app-bin-reruns.log:56-60`).
2. **Non-macOS CI targets (`ubuntu` / `windows`):** Not executed locally; verified on macOS `aarch64` (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/01-fmt.log:1-3`) and delegated to GitHub Actions on Draft PR `#1580` (`https://github.com/jangtrinh/design-os-studio/pull/1580`).

---

## 1. Conclusion

`create_text_style_from_nodes` (`crates/designos-doc/src/text_style_ops.rs:60`), `detach_text_style` (`crates/designos-doc/src/text_style_ops.rs:92`), their companion functions `has_single_text_style` (`crates/designos-doc/src/text_style_ops.rs:16`), `text_layers` (`crates/designos-doc/src/text_style_ops.rs:21`), `shared_text_style` (`crates/designos-doc/src/text_style_ops.rs:35`), `next_free_name` (`crates/designos-doc/src/text_style_ops.rs:49`), `TEXT_REF` (`crates/designos-doc/src/text_style_ops.rs:11`), and all `6` (`crates/designos-doc/src/text_style_ops.rs:136-215`) existing unit tests have been moved without behavioral or body changes (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/08-move-diff.log:1-20`) from `crates/designos-app/src/styles/text_style_ops.rs` (`crates/designos-app/src/styles/text_style_ops.rs:1-3`) into `crates/designos-doc/src/text_style_ops.rs` (`crates/designos-doc/src/text_style_ops.rs:1-249`) and re-exported at the root of `designos-doc` (`crates/designos-doc/src/lib.rs:76,114`) as well as in `crates/designos-app/src/styles/text_style_ops.rs` (`crates/designos-app/src/styles/text_style_ops.rs:3`). One new document-only unit test, `create_and_detach_round_trip_on_plain_document` (`crates/designos-doc/src/text_style_ops.rs:218-248`), verifies creating and detaching a text style and undoing both steps on a plain `Document` (`crates/designos-doc/src/lib.rs:183`) without `designos-app`. All `7` (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/03-test-doc.log:664-670`) `text_style_ops::tests::*` tests run and pass in `designos-doc` (`752 passed; 0 failed` unit tests, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/03-test-doc.log:764`), `0` (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/04-test-app-bin.log:1-4315`) `text_style_ops` tests remain duplicated in `designos-app`, and all formatting (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/01-fmt.log:3`), clippy (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/02-clippy.log:49`), `designos-mcp` (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/05-test-mcp.log:728`), workspace library (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/06-test-workspace-lib.log:1620`), and zero-fake-UI (`17/17`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/07-zero-fake-ui-gate.log:24-26`) gates pass.

---

## 2. Step 1 — Facts First (Recorded Before Any Code Edit)

### 2.1 Every Item `create_text_style_from_nodes` and `detach_text_style` Depend On

Source file before move on `origin/main` (`1e46e34f2bf93eea3b2374a672a71c7f8097e267`): `crates/designos-app/src/styles/text_style_ops.rs:1-215` (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/10-git-diff.log:23-239`), with `create_text_style_from_nodes` at `crates/designos-doc/src/text_style_ops.rs:60` and `detach_text_style` at `crates/designos-doc/src/text_style_ops.rs:92`.

| Item | Kind | Location (`file:line`) | Lives in `designos-doc`? |
|---|---|---|---|
| `TEXT_REF` (`crates/designos-doc/src/text_style_ops.rs:11`) | `const &str` (`"text"`) | `crates/designos-doc/src/text_style_ops.rs:11` (was `crates/designos-app/src/styles/text_style_ops.rs:11`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/10-git-diff.log:35`) | Yes — moved with module (`FACT`) |
| `has_single_text_style` (`crates/designos-doc/src/text_style_ops.rs:16`) | `pub fn(&TextProps) -> bool` | `crates/designos-doc/src/text_style_ops.rs:16` (was `crates/designos-app/src/styles/text_style_ops.rs:16`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/10-git-diff.log:40`) | Yes — moved with module (`FACT`) |
| `text_layers` (`crates/designos-doc/src/text_style_ops.rs:21`) | `fn(&Document, &[NodeId]) -> Result<Vec<(&Node, &TextProps)>, StyleOpError>` | `crates/designos-doc/src/text_style_ops.rs:21` (was `crates/designos-app/src/styles/text_style_ops.rs:21`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/10-git-diff.log:45`) | Yes — moved with module (`FACT`) |
| `shared_text_style` (`crates/designos-doc/src/text_style_ops.rs:35`) | `pub fn(&Document, &[NodeId]) -> Option<TextStyle>` | `crates/designos-doc/src/text_style_ops.rs:35` (was `crates/designos-app/src/styles/text_style_ops.rs:35`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/10-git-diff.log:59`) | Yes — moved with module (`FACT`) |
| `next_free_name` (`crates/designos-doc/src/text_style_ops.rs:49`) | `fn(&Document) -> String` | `crates/designos-doc/src/text_style_ops.rs:49` (was `crates/designos-app/src/styles/text_style_ops.rs:49`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/10-git-diff.log:73`) | Yes — moved with module (`FACT`) |
| `same_text_style` (`crates/designos-doc/src/lint/token_match.rs:628`) | `pub fn(&TextStyle, &TextStyle) -> bool` | `crates/designos-doc/src/lint/token_match.rs:628` | Yes (`FACT`) |
| `same_text_style_props` (`crates/designos-doc/src/lint/token_match.rs:648`) | `pub fn(&TextProps, &TextStyle) -> bool` | `crates/designos-doc/src/lint/token_match.rs:648` | Yes (`FACT`) |
| `Document` (`crates/designos-doc/src/lib.rs:183`) | Struct (`styles` at `crates/designos-doc/src/lib.rs:191`, `style_meta` at `crates/designos-doc/src/lib.rs:194`, `style_name` at `crates/designos-doc/src/lib.rs:750`, `get` at `crates/designos-doc/src/lib.rs:1048`) | `crates/designos-doc/src/lib.rs:183` | Yes (`FACT`) |
| `NodeId` (`crates/designos-doc/src/lib.rs:133`) | Struct | `crates/designos-doc/src/lib.rs:133` | Yes (`FACT`) |
| `DocError` / `DocError::NotFound` (`crates/designos-doc/src/lib.rs:136-138`) | Error enum & variant | `crates/designos-doc/src/lib.rs:136` (`NotFound` at `crates/designos-doc/src/lib.rs:138`) | Yes (`FACT`) |
| `StyleOpError` (`crates/designos-doc/src/styles_ops.rs:17`) | Error enum (`Doc` at `crates/designos-doc/src/styles_ops.rs:19`, `WrongNodeKind` at `crates/designos-doc/src/styles_ops.rs:25`, `NothingToStyle` at `crates/designos-doc/src/styles_ops.rs:27`) | `crates/designos-doc/src/styles_ops.rs:17` | Yes (`FACT`) |
| `Node` (`crates/designos-doc/src/node.rs:65`) | Struct (`id` at `crates/designos-doc/src/node.rs:66`, `kind` at `crates/designos-doc/src/node.rs:71`, `style_refs` at `crates/designos-doc/src/node.rs:113`) | `crates/designos-doc/src/node.rs:65` | Yes (`FACT`) |
| `NodeKind` / `NodeKind::Text` (`crates/designos-doc/src/node.rs:488,516`) | Enum & variant | `crates/designos-doc/src/node.rs:488` (`Text` at `crates/designos-doc/src/node.rs:516`) | Yes (`FACT`) |
| `TextProps` (`crates/designos-doc/src/node.rs:679`) | Struct (`style` at `crates/designos-doc/src/node.rs:681`, `runs` at `crates/designos-doc/src/node.rs:690`, `paragraph` at `crates/designos-doc/src/node.rs:692`) | `crates/designos-doc/src/node.rs:679` | Yes (`FACT`) |
| `ParagraphProps` (`crates/designos-doc/src/textmodel.rs:130`) | Struct (`indent` at `crates/designos-doc/src/textmodel.rs:132`, `list_spacing` at `crates/designos-doc/src/textmodel.rs:134`, `leading_trim` at `crates/designos-doc/src/textmodel.rs:136`) | `crates/designos-doc/src/textmodel.rs:130` | Yes (`FACT`) |
| `TextStyle` (`crates/designos-doc/src/paint.rs:462`) | Struct | `crates/designos-doc/src/paint.rs:462` | Yes (`FACT`) |
| `TextAlignH` / `TextAlignV` (`crates/designos-doc/src/paint.rs:409,418`) | Enums (`TextAlignH::Left`, `TextAlignV::Top`) | `crates/designos-doc/src/paint.rs:409`, `crates/designos-doc/src/paint.rs:418` | Yes (`FACT`) |
| `Style` / `Style::Text` (`crates/designos-doc/src/lib.rs:1589,1591`) | Enum & variant | `crates/designos-doc/src/lib.rs:1589` (`Text` at `crates/designos-doc/src/lib.rs:1591`) | Yes (`FACT`) |
| `StyleMeta` (`crates/designos-doc/src/lib.rs:1571`) | Struct | `crates/designos-doc/src/lib.rs:1571` | Yes (`FACT`) |
| `StyleProperty` / `StyleProperty::Text` (`crates/designos-doc/src/lib.rs:1561,1564`) | Enum & variant | `crates/designos-doc/src/lib.rs:1561` (`Text` at `crates/designos-doc/src/lib.rs:1564`) | Yes (`FACT`) |
| `Command` (`crates/designos-doc/src/history.rs:8`) | Enum (`Update` at `crates/designos-doc/src/history.rs:24`, `SetStyles` at `crates/designos-doc/src/history.rs:40`, `Batch` at `crates/designos-doc/src/history.rs:76`) | `crates/designos-doc/src/history.rs:8` | Yes (`FACT`) |
| `std::collections::BTreeSet` (`crates/designos-doc/src/text_style_ops.rs:94`) | Standard library collection | `crates/designos-doc/src/text_style_ops.rs:94` | `std` (`FACT`) |
| Test items (`Document::new` at `crates/designos-doc/src/lib.rs:610`, `alloc_id` at `crates/designos-doc/src/lib.rs:706`, `add_page` at `crates/designos-doc/src/lib.rs:1022`, `insert` at `crates/designos-doc/src/lib.rs:1033`, `get_mut` at `crates/designos-doc/src/lib.rs:1052`, `Node::rectangle` at `crates/designos-doc/src/node.rs:426`, `Node::text` at `crates/designos-doc/src/node.rs:436`, `Rect::new` at `crates/designos-doc/src/geom.rs:26`, `History::exec` at `crates/designos-doc/src/history.rs:361`, `TextRun` at `crates/designos-doc/src/paint.rs:570`, `Paint::solid` at `crates/designos-doc/src/paint.rs:231`, `Color::rgb` at `crates/designos-doc/src/paint.rs:19`) | Document & paint test APIs | `crates/designos-doc/src/lib.rs:610-1052`, `crates/designos-doc/src/node.rs:426-436`, `crates/designos-doc/src/geom.rs:26`, `crates/designos-doc/src/history.rs:361`, `crates/designos-doc/src/paint.rs:19-570` | Yes (`FACT`) |
| App-only state / types | None (`0` items) | None (`crates/designos-doc/src/text_style_ops.rs:1-249`) | Zero app-only dependencies (`FACT`) |

### 2.2 Every Caller and Test of the Helpers Across the Workspace

1. **Module declaration and re-export in `designos-app`:**
   - `crates/designos-app/src/styles.rs:501` — `mod text_style_ops;`
   - `crates/designos-app/src/styles.rs:503` — `pub use text_style_ops::{create_text_style_from_nodes, detach_text_style, shared_text_style};`
   - `crates/designos-app/src/styles/text_style_ops.rs:3` — `pub use designos_doc::text_style_ops::{create_text_style_from_nodes, detach_text_style, shared_text_style};`
2. **Production callers of `create_text_style_from_nodes` (`crates/designos-doc/src/text_style_ops.rs:60`):**
   - `crates/designos-app/src/styles.rs:247` — inside `App::create_text_style_from` (`crates/designos-app/src/styles.rs:241`), called by `App::create_style_from` (`crates/designos-app/src/styles.rs:230`) and `crates/designos-app/src/panels/style_picker.rs:256`.
3. **Production callers of `detach_text_style` (`crates/designos-doc/src/text_style_ops.rs:92`):**
   - `crates/designos-app/src/styles.rs:272` — inside `App::detach_style_of` (`crates/designos-app/src/styles.rs:266`), called by `crates/designos-app/src/panels/style_picker.rs:276` and `crates/designos-app/src/panels/style_picker.rs:350`.
   - `crates/designos-app/src/styles/new_text_box.rs:80` — inside `App::detach_text_style_in_new_box` (`crates/designos-app/src/styles/new_text_box.rs:74`), called by `crates/designos-app/src/styles.rs:269`.
4. **Callers of `shared_text_style` (`crates/designos-doc/src/text_style_ops.rs:35`) and `has_single_text_style` (`crates/designos-doc/src/text_style_ops.rs:16`):**
   - `crates/designos-doc/src/text_style_ops.rs:44` — `shared_text_style` calls `has_single_text_style`.
   - `crates/designos-doc/src/text_style_ops.rs:65` — `create_text_style_from_nodes` calls `shared_text_style`.
   - `crates/designos-app/src/panels/style_picker.rs:10` and `crates/designos-app/src/panels/style_picker.rs:108` — imports and calls `shared_text_style(&app.doc, sel)`.
5. **Unit tests in `crates/designos-doc/src/text_style_ops.rs:105-249` (`6` moved + `1` new = `7` total, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/03-test-doc.log:664-670`):**
   - `creating_copies_the_layer_style_and_links_the_layer` (`crates/designos-doc/src/text_style_ops.rs:136`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/03-test-doc.log:665`)
   - `names_skip_keys_and_display_names_in_use` (`crates/designos-doc/src/text_style_ops.rs:148`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/03-test-doc.log:667`)
   - `only_a_text_layer_with_one_style_can_be_turned_into_a_style` (`crates/designos-doc/src/text_style_ops.rs:160`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/03-test-doc.log:668`)
   - `several_layers_with_the_same_type_get_one_style_and_all_link_to_it` (`crates/designos-doc/src/text_style_ops.rs:178`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/03-test-doc.log:670`)
   - `several_layers_with_different_type_have_no_single_style` (`crates/designos-doc/src/text_style_ops.rs:192`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/03-test-doc.log:669`)
   - `detaching_drops_the_link_and_keeps_the_style_and_the_values` (`crates/designos-doc/src/text_style_ops.rs:204`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/03-test-doc.log:666`)
   - `create_and_detach_round_trip_on_plain_document` (new no-app test at `crates/designos-doc/src/text_style_ops.rs:218`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/03-test-doc.log:664`)
6. **App-side tests exercising `App::create_text_style_from`, `App::create_style_from`, and `App::detach_style_of` via the re-exports (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/04-test-app-bin.log:3408-3416,3678`):**
   - `styles::tests::text_style_apply_keeps_alignment_and_carries_paragraph_settings` (`crates/designos-app/src/styles.rs:624`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/04-test-app-bin.log:3678`)
   - `shell::tests::text_styles::*` (`crates/designos-app/src/shell/tests/text_styles.rs:64-619`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/04-test-app-bin.log:3408-3416`)
7. **How `designos-mcp` reaches doc-side style ops:**
   - `crates/designos-doc/src/lib.rs:73,111` exports `styles_ops` via `pub use styles_ops::*;`, and `crates/designos-mcp/src/engine.rs:410` calls `designos_doc::style_property(...)` directly. Adding `pub mod text_style_ops;` (`crates/designos-doc/src/lib.rs:76`) and `pub use text_style_ops::*;` (`crates/designos-doc/src/lib.rs:114`) makes `designos_doc::create_text_style_from_nodes`, `designos_doc::detach_text_style`, `designos_doc::shared_text_style`, and `designos_doc::has_single_text_style` directly callable from `designos-mcp` (`crates/designos-mcp/src/engine.rs:410`).

---

## 3. Skills Loaded and Rulings Made

| Skill | Path (`file:line`) | Ruling Applied to This Task |
|---|---|---|
| `es:astra-work` | `/Users/jangtrinh/.gemini/config/skills/es-astra-work/SKILL.md:1-130` | Locked the 4-file scope (`crates/designos-doc/src/text_style_ops.rs:1`, `crates/designos-doc/src/lib.rs:76,114`, `crates/designos-app/src/styles/text_style_ops.rs:1-3`, `plans/reports/agy-1028-text-style-ops-to-doc.md:1`), re-ran all verification gates after the final edit (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/01-fmt.log:3` through `plans/reports/agy-1028-text-style-ops-to-doc-evidence/09-ls-out-cargo.log:2`), and placed unverified items at the top of the report. |
| `es:fable-thinking` | `/Users/jangtrinh/.gemini/config/skills/es-fable-thinking/SKILL.md:1-140` | Recorded Step 1 facts and the `FACT` / `INFERENCE` / `ASSUMPTION` ledger before editing code; falsified the assumption that `has_single_text_style` was re-exported by `crates/designos-app/src/styles.rs:503` when `cargo clippy` flagged an unused re-export in `crates/designos-app/src/styles/text_style_ops.rs:3`, narrowing the app re-export to `{create_text_style_from_nodes, detach_text_style, shared_text_style}` (`crates/designos-app/src/styles/text_style_ops.rs:3`). |
| `es:rust` | `/Users/jangtrinh/.gemini/config/skills/es-rust/SKILL.md:1-120` | Kept function signatures, error types (`StyleOpError` at `crates/designos-doc/src/styles_ops.rs:17`), borrowing lifetimes (`text_layers<'a>` at `crates/designos-doc/src/text_style_ops.rs:21`), and `Command` batching (`crates/designos-doc/src/text_style_ops.rs:60-103`) byte-for-byte identical (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/08-move-diff.log:1-20`). |
| `es:rust-gpu-canvas` | `/Users/jangtrinh/.gemini/config/skills/es-rust-gpu-canvas/SKILL.md:1-120` | Confirmed that moving pure document `Command` constructors (`crates/designos-doc/src/text_style_ops.rs:60,92`) touches no GPU/render pipeline or `SceneIndex` paths (`crates/designos-doc/src/lib.rs:76,114`). |
| `es:dev` | `/Users/jangtrinh/.gemini/config/skills/es-dev/SKILL.md:1-40` | Delegated to `es:feature-dev` workflow for scoped refactor execution and verification. |
| `es:feature-dev` | `/Users/jangtrinh/.gemini/config/skills/es-feature-dev/SKILL.md:1-120` | Read `.brv/context-tree/session-handoffs/latest.md:1-50` and `.brv/context-tree/architecture/crate-map.md:1-80` directly when `brv` CLI prompted for an interactive version upgrade, keeping the crate boundary clean (`designos-doc` owns pure `Document` style commands). |

---

## 4. Acceptance Criteria & Verification Gates Table

| Gate / Requirement | Command | Exit Code / Verdict | Citation (`log-file:line` / `file:line`) |
|---|---|---|---|
| `create_text_style_from_nodes` & `detach_text_style` in `designos-doc` | `git diff --no-index -U3 <(git show origin/main:crates/designos-app/src/styles/text_style_ops.rs) crates/designos-doc/src/text_style_ops.rs` | Byte-for-byte identical bodies; only `use designos_doc::` -> `use crate::` (`lines 7-8`) + `1` new test (`lines 217-248`) | `crates/designos-doc/src/text_style_ops.rs:60,92`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/08-move-diff.log:9-54` |
| Exported from `designos-doc` crate root | Inspect `crates/designos-doc/src/lib.rs` | `pub mod text_style_ops;` and `pub use text_style_ops::*;` | `crates/designos-doc/src/lib.rs:76,114`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/10-git-diff.log:506,514` |
| Re-exported in `designos-app` without duplicate tests | Inspect `crates/designos-app/src/styles/text_style_ops.rs` | `pub use designos_doc::text_style_ops::{create_text_style_from_nodes, detach_text_style, shared_text_style};` (`3` lines total, `0` tests in `designos-app`) | `crates/designos-app/src/styles/text_style_ops.rs:1-3`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/10-git-diff.log:23-240` |
| `6` moved unit tests + `1` new no-app test in `designos-doc` | `CARGO_TARGET_DIR=out/cargo cargo test -p designos-doc --locked` | `7` `text_style_ops::tests::*` passed; `752 passed; 0 failed` unit tests + `3 + 5 + 2` integration tests (`test_doc_rc=0`) | `plans/reports/agy-1028-text-style-ops-to-doc-evidence/03-test-doc.log:664-670,764,773,784,792,800` |
| `cargo fmt --all -- --check` | `cargo fmt --all -- --check` | `fmt_rc=0` | `plans/reports/agy-1028-text-style-ops-to-doc-evidence/01-fmt.log:3` |
| `cargo clippy --workspace --all-targets --locked` | `CARGO_TARGET_DIR=out/cargo cargo clippy --workspace --all-targets --locked` | `clippy_rc=0` (`0` warnings in `designos-doc` or `text_style_ops`; only `main`'s `3` known warnings in `agent_permissions.rs:1176,1194` and `inspector_var_binds.rs:1182`) | `plans/reports/agy-1028-text-style-ops-to-doc-evidence/02-clippy.log:8-49` |
| `cargo test -p designos-app --bin designos --locked` | `CARGO_TARGET_DIR=out/cargo cargo test -p designos-app --bin designos --locked` + solo reruns | `4240 passed; 3 failed; 13 ignored` (`04-test-app-bin.log:4312`); all `3` failures are known pre-existing flakes (`04-test-app-bin.log:4308-4310`) that pass on solo/short-`/tmp` rerun (`04b-test-app-bin-reruns.log:43,58,73`) | `plans/reports/agy-1028-text-style-ops-to-doc-evidence/04-test-app-bin.log:3408-3416,3678,4290-4315`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/04b-test-app-bin-reruns.log:13-75` |
| `cargo test -p designos-mcp --locked` | `CARGO_TARGET_DIR=out/cargo cargo test -p designos-mcp --locked` | `test_mcp_rc=0` (`327 passed; 0 failed` unit tests + `19` integration test suites passed) | `plans/reports/agy-1028-text-style-ops-to-doc-evidence/05-test-mcp.log:339,353-728` |
| `cargo test --workspace --locked --lib` | `CARGO_TARGET_DIR=out/cargo cargo test --workspace --locked --lib` | `test_workspace_lib_rc=0` (`13 + 752 + 1 + 164 + 143 + 30 + 5 + 37 + 327 + 56 + 1 + 8` library unit tests passed, `0` failed) | `plans/reports/agy-1028-text-style-ops-to-doc-evidence/06-test-workspace-lib.log:25,783,790,961,1110,1146,1157,1200,1533,1597,1604,1618,1620` |
| `python3 -B scripts/zero_fake_ui_gate.py` | `python3 -B scripts/zero_fake_ui_gate.py` | `Gate Verdict: PASS (17/17 invariants satisfied)`, `zero_fake_rc=0` | `plans/reports/agy-1028-text-style-ops-to-doc-evidence/07-zero-fake-ui-gate.log:24-26` |
| Cleanup `out/cargo` | `rm -rf out/cargo && ls out/cargo` | `ls: out/cargo: No such file or directory`, `ls_out_cargo_rc=1` | `plans/reports/agy-1028-text-style-ops-to-doc-evidence/09-ls-out-cargo.log:1-2` |

---

## 5. Pasted Command Outputs

### 5.1 `cargo fmt --all -- --check` (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/01-fmt.log:1-3`)

```text
Filesystem        Size    Used   Avail Capacity iused ifree %iused  Mounted on
/dev/disk3s1s1   926Gi    12Gi    96Gi    11%    459k  1.0G    0%   /
fmt_rc=0
```

### 5.2 `CARGO_TARGET_DIR=out/cargo cargo clippy --workspace --all-targets --locked` (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/02-clippy.log:1-49`)

```text
Filesystem        Size    Used   Avail Capacity iused ifree %iused  Mounted on
/dev/disk3s1s1   926Gi    12Gi    96Gi    11%    459k  1.0G    0%   /
warning: /Users/jangtrinh/orca/workspaces/design-os-studio/agy-1028-text-style-ops-to-doc/crates/designos-app/Cargo.toml: file `/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1028-text-style-ops-to-doc/crates/designos-app/src/main.rs` found to be present in multiple build targets:
  * `bin` target `designos`
  * `bin` target `designos-studio`
   Compiling designos-mcp v0.7.7 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1028-text-style-ops-to-doc/crates/designos-mcp)
    Checking designos-app v0.7.7 (/Users/jangtrinh/orca/workspaces/design-os-studio/agy-1028-text-style-ops-to-doc/crates/designos-app)
warning: unneeded `return` statement
    --> crates/designos-app/src/agent_permissions.rs:1176:9
warning: unneeded `return` statement
    --> crates/designos-app/src/agent_permissions.rs:1194:9
warning: very complex type used. Consider factoring parts into `type` definitions
    --> crates/designos-app/src/shell/tests/inspector_var_binds.rs:1182:21
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 15.07s
clippy_rc=0
```

### 5.3 `CARGO_TARGET_DIR=out/cargo cargo test -p designos-doc --locked` (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/03-test-doc.log:664-670,764-800`)

```text
test text_style_ops::tests::create_and_detach_round_trip_on_plain_document ... ok
test text_style_ops::tests::creating_copies_the_layer_style_and_links_the_layer ... ok
test text_style_ops::tests::detaching_drops_the_link_and_keeps_the_style_and_the_values ... ok
test text_style_ops::tests::names_skip_keys_and_display_names_in_use ... ok
test text_style_ops::tests::only_a_text_layer_with_one_style_can_be_turned_into_a_style ... ok
test text_style_ops::tests::several_layers_with_different_type_have_no_single_style ... ok
test text_style_ops::tests::several_layers_with_the_same_type_get_one_style_and_all_link_to_it ... ok
...
test result: ok. 752 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.69s

     Running tests/auto_layout_wrap_fields_compat.rs (out/cargo/debug/deps/auto_layout_wrap_fields_compat-c3f526bb6ce5e706)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

     Running tests/changelog_pillars.rs (out/cargo/debug/deps/changelog_pillars-65955caffc7324cb)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/shadow_blend.rs (out/cargo/debug/deps/shadow_blend-081ec078cae3c8a2)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests designos_doc
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

test_doc_rc=0
```

### 5.4 `CARGO_TARGET_DIR=out/cargo cargo test -p designos-app --bin designos --locked` & Solo Reruns (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/04-test-app-bin.log:4288-4315`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/04b-test-app-bin-reruns.log:12-75`)

```text
failures:
    agent_approval_channel::tests::the_production_root_sits_outside_every_agent_writable_root
    shell::tests::close_human_dirty::library_file_close_save_writes_the_edit_and_closes_not_path_required
    vecops::tests::two_deferred_text_commands_both_convert_in_order

test result: FAILED. 4240 passed; 3 failed; 13 ignored; 0 measured; 0 filtered out; finished in 391.58s
test_app_bin_rc=101

=== RERUN: shell::tests::close_human_dirty::library_file_close_save_writes_the_edit_and_closes_not_path_required ===
test shell::tests::close_human_dirty::library_file_close_save_writes_the_edit_and_closes_not_path_required ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 4255 filtered out; finished in 2.50s
rc=0

=== RERUN: vecops::tests::two_deferred_text_commands_both_convert_in_order ===
test vecops::tests::two_deferred_text_commands_both_convert_in_order ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 4255 filtered out; finished in 1.64s
rc=0

=== RERUN (short /tmp root): agent_approval_channel::tests::the_production_root_sits_outside_every_agent_writable_root ===
test agent_approval_channel::tests::the_production_root_sits_outside_every_agent_writable_root ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 4255 filtered out; finished in 0.00s
rc=0
```

### 5.5 `CARGO_TARGET_DIR=out/cargo cargo test -p designos-mcp --locked` (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/05-test-mcp.log:8,339-728`)

```text
     Running unittests src/lib.rs (out/cargo/debug/deps/designos_mcp-b500445f18a41338)
test result: ok. 327 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.80s
     Running unittests src/main.rs (out/cargo/debug/deps/designos_mcp-d0dc86933af4af20)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/bin/gen-tool-docs.rs (out/cargo/debug/deps/gen_tool_docs-4a5f14536e654b8a)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/button_label_centre.rs -> ok. 4 passed; 0 failed
     Running tests/close_document.rs -> ok. 28 passed; 0 failed
     Running tests/colour_detach.rs -> ok. 11 passed; 0 failed
     Running tests/colour_var_library.rs -> ok. 1 passed; 0 failed
     Running tests/colour_var_refs.rs -> ok. 16 passed; 0 failed
     Running tests/hug_text_relayout.rs -> ok. 11 passed; 0 failed
     Running tests/list_lint_rules.rs -> ok. 4 passed; 0 failed
     Running tests/live_doc_epoch.rs -> ok. 3 passed; 0 failed
     Running tests/live_session_guards.rs -> ok. 3 passed; 0 failed
     Running tests/mcp_paths.rs -> ok. 23 passed; 0 failed
     Running tests/mcp_paths_full_disk.rs -> ok. 1 passed; 0 failed
     Running tests/session.rs -> ok. 24 passed; 0 failed
     Running tests/session_file_sites/main.rs -> ok. 9 passed; 0 failed
     Running tests/session_files/main.rs -> ok. 42 passed; 0 failed
     Running tests/set_slot_batch.rs -> ok. 14 passed; 0 failed
     Running tests/spec_errors.rs -> ok. 8 passed; 0 failed
     Running tests/spec_text_colour.rs -> ok. 5 passed; 0 failed
     Running tests/stdio.rs -> ok. 1 passed; 0 failed
     Running tests/text_fill.rs -> ok. 7 passed; 0 failed
     Running tests/text_truncation_roundtrip.rs -> ok. 2 passed; 0 failed
     Running tests/tools_snapshot.rs -> ok. 8 passed; 0 failed
     Running tests/version.rs -> ok. 4 passed; 0 failed
     Running tests/wrapped_fill_text_height.rs -> ok. 2 passed; 0 failed
test_mcp_rc=0
```

### 5.6 `CARGO_TARGET_DIR=out/cargo cargo test --workspace --locked --lib` (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/06-test-workspace-lib.log:8-1620`)

```text
     Running unittests src/lib.rs (designos_diff) -> ok. 13 passed; 0 failed
     Running unittests src/lib.rs (designos_doc) -> ok. 752 passed; 0 failed
     Running unittests src/lib.rs (designos_eval) -> ok. 1 passed; 0 failed
     Running unittests src/lib.rs (designos_export) -> ok. 164 passed; 0 failed; 1 ignored
     Running unittests src/lib.rs (designos_fig) -> ok. 143 passed; 0 failed
     Running unittests src/lib.rs (designos_fonts) -> ok. 30 passed; 0 failed
     Running unittests src/lib.rs (designos_gpu) -> ok. 5 passed; 0 failed
     Running unittests src/lib.rs (designos_library) -> ok. 37 passed; 0 failed
     Running unittests src/lib.rs (designos_mcp) -> ok. 327 passed; 0 failed
     Running unittests src/lib.rs (designos_render) -> ok. 56 passed; 0 failed; 2 ignored
     Running unittests src/lib.rs (designos_svg) -> ok. 1 passed; 0 failed
     Running unittests src/lib.rs (designos_webcapture) -> ok. 8 passed; 0 failed
test_workspace_lib_rc=0
```

### 5.7 `python3 -B scripts/zero_fake_ui_gate.py` (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/07-zero-fake-ui-gate.log:1-26`)

```text
============================================================================================================
 DESIGN:OS Studio — Zero-Fake-UI Architectural Gate
============================================================================================================
Gate  | Status | File                             | Invariant
------------------------------------------------------------------------------------------------------------
Z01   | PASS   | terminal.rs                      | Native Embedded PTY Terminal (portable_pty + alacritty_terminal)
Z02   | PASS   | terminal.rs                      | Spawned Agent CLI gets live DESIGNOS_LIVE_DOC / DESIGNOS_MCP_BIN env vars
Z03   | PASS   | terminal.rs                      | dispatch_figma_ai_prompt sends executable command without # comment prefix
Z04   | PASS   | terminal.rs + agent_header.rs + agent_composer_toolbar.rs | Agents header and composer buttons have Sense::click() handlers; fake mic removed
Z05   | PASS   | specs.rs                         | specs-ask-agent-bar hosts editable TextEdit::singleline(ask_input)
Z06   | PASS   | specs.rs                         | Right Inspector dynamic active_section_toc & interactive SpecComment
Z07   | PASS   | specs.rs                         | Unwrapped draw_wrapped_prose & render_tab_blueprints renders pkg.ui_contract
Z08   | PASS   | panels/toolbar.rs + cmdk.rs      | Contextual sub-toolbar buttons mutate app.doc / app.tool; ctx-crop / ctx-remove-bg / cmdk Remove background stay removed
Z09   | PASS   | home/mod.rs                      | Act::ExportFig writes .fig to disk & Nav::AllFolders/Templates/Libraries filter distinctly
Z10   | PASS   | panels/right.rs                  | Share copies to clipboard & Dev Mode 2nd tab renders Dev Plugins inspector
Z11   | PASS   | panels/inspect.rs + properties.rs | Dev Mode Code Connect/MCP/Redlines/Keyframes & Copy to Slides mutate state/clipboard
Z12   | PASS   | widgets/picker.rs + panels/mod.rs + variables/view.rs | Mode::Image creates Paint::Image/doc.images & scrollbars support Sense::click_and_drag()
Z13   | PASS   | panels/menus.rs                  | Zoom menu rows are the allow-listed set on real Actions; no hard-coded checked state
Z14   | PASS   | updater.rs + tabs.rs             | Sample-manifest updater UI stays hidden (guard is code in tabs, sidebar card, modal) until a real manifest fetch exists
Z15   | PASS   | terminal.rs + cmdk.rs            | No fake template synthesis or canned Executed via transcript in Agents panel; synthesize_spec_from_prompt stays private to cmdk.rs
Z16   | PASS   | registered label helpers in designos-app/src | UX writing: action labels at registered button/menu helpers and label fns <=3 words, no count in them (ratchet vs ux_writing_baseline.json)
Z17   | PASS   | terminal.rs + mcp/server.rs      | No global /tmp/designos-live-* or designos-live-selection.spec.json fallback in shipped code
------------------------------------------------------------------------------------------------------------
 Gate Verdict: PASS (17/17 invariants satisfied)
============================================================================================================
zero_fake_rc=0
```

### 5.8 `git diff --stat origin/main...HEAD` and Move Diff (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/10-git-diff.log:1-30,240-256,497-517`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/08-move-diff.log:1-54`)

```text
 crates/designos-app/src/styles/text_style_ops.rs   |  216 +-
 crates/designos-doc/src/lib.rs                     |    2 +
 crates/designos-doc/src/text_style_ops.rs          |  249 ++
```

```diff
diff --git a/dev/fd/12 b/crates/designos-doc/src/text_style_ops.rs
index 00000000..0f49557f 100644
--- a/dev/fd/12
+++ b/crates/designos-doc/src/text_style_ops.rs
@@ -4,8 +4,8 @@
 //! returns a [`Command`] and leaves the document alone, so the caller decides how the change joins
 //! the undo stack.
 
-use designos_doc::lint::token_match::{same_text_style, same_text_style_props};
-use designos_doc::*;
+use crate::lint::token_match::{same_text_style, same_text_style_props};
+use crate::*;
 
 /// Key of the `style_refs` entry that links a text layer to its text style.
 const TEXT_REF: &str = "text";
@@ -212,4 +212,38 @@ mod tests {
         assert!(d.styles.contains_key(&key));
         assert_eq!(d.get(t).unwrap().kind, before);
     }
+
+    #[test]
+    fn create_and_detach_round_trip_on_plain_document() {
+        let mut doc = Document::new("plain");
+        let page = doc.add_page("Page 1");
+        let tid = doc.alloc_id();
+        let mut text = Node::text(
+            tid,
+            Rect::new(0.0, 0.0, 120.0, 24.0),
+            "Heading",
+            TextStyle { font_size: 28.0, font_weight: 600, align_h: TextAlignH::Center, ..Default::default() },
+        );
+        if let NodeKind::Text(tp) = &mut text.kind {
+            tp.paragraph.indent = 8.0;
+        }
+        let text_id = doc.insert(page, text).unwrap();
+
+        let mut hist = History::default();
+        let (key, create_cmd) = crate::create_text_style_from_nodes(&doc, &[text_id]).unwrap();
+        hist.exec(&mut doc, create_cmd).unwrap();
+        assert_eq!(doc.get(text_id).unwrap().style_refs.get("text"), Some(&key));
+        assert!(matches!(&doc.styles[&key], Style::Text(ts) if ts.font_size == 28.0 && ts.font_weight == 600 && ts.paragraph_indent == 8.0));
+
+        let detach_cmd = crate::detach_text_style(&doc, &[text_id]).unwrap();
+        hist.exec(&mut doc, detach_cmd).unwrap();
+        assert!(!doc.get(text_id).unwrap().style_refs.contains_key("text"));
+        assert!(doc.styles.contains_key(&key));
+
+        assert!(hist.undo(&mut doc).unwrap());
+        assert_eq!(doc.get(text_id).unwrap().style_refs.get("text"), Some(&key));
+        assert!(hist.undo(&mut doc).unwrap());
+        assert!(doc.styles.is_empty());
+        assert!(!doc.get(text_id).unwrap().style_refs.contains_key("text"));
+    }
 }
```

### 5.9 `ls out/cargo` (`plans/reports/agy-1028-text-style-ops-to-doc-evidence/09-ls-out-cargo.log:1-2`)

```text
ls: out/cargo: No such file or directory
ls_out_cargo_rc=1
```

---

## 6. Epistemic Ledger (`FACT` / `INFERENCE` / `ASSUMPTION`), Assumptions & Weakest Link

### 6.1 Epistemic Ledger
- **`FACT`**: Every symbol referenced by `create_text_style_from_nodes` (`crates/designos-doc/src/text_style_ops.rs:60`), `detach_text_style` (`crates/designos-doc/src/text_style_ops.rs:92`), `shared_text_style` (`crates/designos-doc/src/text_style_ops.rs:35`), `has_single_text_style` (`crates/designos-doc/src/text_style_ops.rs:16`), `text_layers` (`crates/designos-doc/src/text_style_ops.rs:21`), and `next_free_name` (`crates/designos-doc/src/text_style_ops.rs:49`) already resided in `designos-doc` (`crates/designos-doc/src/lib.rs:133-1591`, `crates/designos-doc/src/styles_ops.rs:17`, `crates/designos-doc/src/lint/token_match.rs:628,648`).
- **`FACT`**: The moved implementations and `6` unit tests in `crates/designos-doc/src/text_style_ops.rs:1-215` are byte-for-byte identical to `origin/main:crates/designos-app/src/styles/text_style_ops.rs:1-215` except for lines `7-8` (`use designos_doc::` -> `use crate::`, `plans/reports/agy-1028-text-style-ops-to-doc-evidence/08-move-diff.log:9-12`).
- **`FACT`**: `crates/designos-app/src/styles.rs:503` re-exports `{create_text_style_from_nodes, detach_text_style, shared_text_style}` from `crates/designos-app/src/styles/text_style_ops.rs:3`; re-exporting `has_single_text_style` in `crates/designos-app/src/styles/text_style_ops.rs` triggers `unused_imports` because no caller in `designos-app` imports `has_single_text_style` through `styles::text_style_ops`.
- **`INFERENCE`**: Exporting both `pub mod text_style_ops;` (`crates/designos-doc/src/lib.rs:76`) and `pub use text_style_ops::*;` (`crates/designos-doc/src/lib.rs:114`) satisfies both module-qualified callers (`designos_doc::text_style_ops::*`, `crates/designos-app/src/styles/text_style_ops.rs:3`) and crate-root callers (`designos_doc::create_text_style_from_nodes`, `crates/designos-doc/src/text_style_ops.rs:233`), matching `styles_ops` (`crates/designos-doc/src/lib.rs:73,111`).
- **`ASSUMPTION`**: None remaining (`0` unverified assumptions).

### 6.2 Weakest Link
- `crates/designos-app/src/styles/text_style_ops.rs:3` re-exports `{create_text_style_from_nodes, detach_text_style, shared_text_style}` rather than `pub use designos_doc::text_style_ops::*;` so that `rustc`'s `unused_imports` lint stays clean while `crates/designos-app/src/styles.rs:501-503` keeps `mod text_style_ops;` private and re-exports those `3` symbols. If a future caller in `crates/designos-app/src/styles.rs` needs `has_single_text_style` (`crates/designos-doc/src/text_style_ops.rs:16`), it can import it either from `designos_doc::has_single_text_style` (`crates/designos-doc/src/lib.rs:114`) or add it to the list in `crates/designos-app/src/styles/text_style_ops.rs:3`.

---

## 7. Unresolved Questions

None.

Status: DONE
