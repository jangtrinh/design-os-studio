# E15-40 Insert stock images with attribution metadata

- Issue: #629 (task E15-40, epic #585, M / P0)
- Spec base: `main` at `767acf9`.
- Files: `crates/designos-mcp/src/tools/assets.rs` (`kind: "stock"`), `crates/designos-doc/src/agent_meta.rs` (SC-4: `attribution`), `crates/designos-app/src/panels/inspect.rs` (Credit row), `crates/designos-export/src/{svg.rs,pdf.rs,raster.rs}` (credits in exports), `crates/designos-fig/src/fig_export.rs` and the `.fig` importer (plugin data round trip, one field; these are hot files that merge one at a time per `docs/releasing.md`).

## 1. Summary and user story

As an agent, I want to place a stock photo found and fetched by my own agent CLI, with the author, source URL and licence kept on the layer, so credits travel with the design into exports.

As a user, I want to see the credit in Inspect, and a miss to come back as a warning with nothing changed.

## 2. Current state

- FACT: E15-39 defines `designos_generate_asset`, agent jobs (SC-5), SHA-1 image keys and the pending overlay.
- FACT: nodes have `plugin_data: BTreeMap<String, String>` (Figma plugin data) and `fig_metadata` (`crates/designos-doc/src/node.rs:146,155`); no attribution field exists.
- FACT: the Inspect panel lives in `crates/designos-app/src/panels/inspect.rs`.
- FACT: exporters exist for SVG, PDF and PNG/JPEG (`crates/designos-export/src/{svg.rs,pdf.rs,raster.rs}`).

## 3. Behaviour rules

1. **Call.** `designos_generate_asset({ kind: "stock", query, target, orientation?, scale_mode?, wait? })`; `wait` defaults to `true` for stock (searches are short), with a 60 s job timeout.
2. **Job.** The agent CLI gets our own prompt: find one photo matching `query` and `orientation` from a source whose licence allows use in designs, download it into `{output_dir}`, and write `result.json` with `files[0]` and `meta: { author, author_url?, source_url, licence, licence_url?, source_name }`. If nothing suitable is found it writes `{ "status": "miss", "message": "…" }`.
3. **Validation.** `author`, `source_url` and `licence` are required non-empty strings (≤ 512 chars each); `source_url` and `licence_url` must be `https://` URLs. Missing fields → the job fails with `stock_attribution_missing` (the image is not placed without its credit).
4. **Placement.** Same as E15-39 rule 4 (SHA-1 key, one undo step), and in the same undo step `target.agent.attribution = Attribution { author, author_url, source_url, source_name, licence, licence_url, retrieved_at }`.
5. **Miss (AC2).** The tool returns a success result with `warnings: [{ "name": "stock_not_found", "message": "…" }]` (not an error), `agent.asset` is cleared, and the document is exactly as before: fills unchanged, no attribution written, no undo entry.
6. **Inspect.** A selected node with `attribution` shows a `Credit` section: `Photo by <author>` (link to `author_url` when present), `Source <source_name>` (link to `source_url`), `Licence <licence>` (link to `licence_url`), plus a `Copy credit` button that copies `Photo by <author> on <source_name> (<licence>) <source_url>`.
7. **Exports with credits.**
   - SVG: a `<metadata>` element at the root listing one `<designos:credit author=… source=… licence=… node=…/>` per attributed node in the exported subtree (namespace `urn:designos:credits:1`).
   - PDF: the document information dictionary gets a `Credits` entry (text string, one credit per line).
   - PNG: an `iTXt` chunk with keyword `Credits` (UTF-8, one per line). JPEG: a COM segment with the same text.
   - Code export: out of scope.
8. **Round trip.** `.designos` stores `agent.attribution` natively (AC1: survives save and reload). `.fig` export writes it to `plugin_data["designos:attribution"]` as JSON so a `.fig` round trip through Studio keeps it; `.fig` import reads that key back into `agent.attribution` and removes it from `plugin_data`.
9. **Replacing the image** (any later fill change that removes the stock image paint) clears `attribution` in the same undo step, so credits never outlive their image.

## 4. Data model / schema

```rust
// agent_meta.rs (SC-4; this task adds `attribution`)
#[serde(default, skip_serializing_if = "Option::is_none")] pub attribution: Option<Attribution>,
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Attribution { pub author: String, #[serde(default)] pub author_url: Option<String>, pub source_url: String,
    pub source_name: String, pub licence: String, #[serde(default)] pub licence_url: Option<String>, pub retrieved_at: String }
```

## 5. Contracts

- Input `{ "kind": "stock", "query": "team meeting in a bright office", "target": 88, "orientation": "landscape" }`.
- Output (hit) `{ "job_id": "a-12", "status": "done", "target": 88, "image_key": "9b…", "attribution": { "author": "…", "source_url": "https://…", "licence": "…" } }`.
- Output (miss) `{ "job_id": "a-12", "status": "miss", "target": 88, "warnings": [ { "name": "stock_not_found", "message": "No photo matched 'team meeting in a bright office'." } ] }`.
- Errors: E15-39's, plus `stock_attribution_missing` (`safe_to_retry: true`).

## 6. File-format mapping

| Field | `.designos` | `.fig` | SVG | PDF | PNG / JPEG |
|---|---|---|---|---|---|
| attribution | `agent.attribution` | `plugin_data["designos:attribution"]` JSON | `<metadata>` credits | Info `Credits` | `iTXt Credits` / COM |

## 7. UI

Inspect `Credit` section: 28 px rows, labels 11 px `TEXT_SECONDARY`, values 11 px `TEXT`, links in `TEXT_BRAND` with underline on hover, `Copy credit` secondary button (28 px). Section hidden when there is no attribution. Evidence: 1440 capture of Inspect with a credited image.

## 8. Edge cases

1. Attributed node duplicated: the copy keeps the attribution (the image is the same).
2. Instance of a component whose main has a stock image: Inspect shows the main's credit; exports list it once per rendered instance node.
3. URLs with query strings or non-ASCII: stored as given (validated as https URLs).
4. Export of a subtree without attributed nodes: no credits metadata is written.
5. The agent returns a licence string such as "unknown": accepted as text but the result adds a `stock_licence_unclear` warning; the owner decides whether to keep it.

## 9. Non-goals

- Choosing stock providers or calling their APIs from the app (the agent CLI does the search).
- Credits in generated code.

## 10. Test plan

Seam: shell test with a stub agent (writes a fixture JPEG and meta), plus a screenshot; export unit tests.

Fail-before: `kind: "stock"` is rejected.

| Test | Input | Expected |
|---|---|---|
| `attribution_survives_save_reload` (AC1) | stub hit, save, reload | `agent.attribution` equal; Inspect shows the Credit section |
| `miss_warns_and_changes_nothing` (AC2) | stub miss | `stock_not_found` warning; document hash unchanged; undo depth unchanged |
| `missing_author_fails` | meta without author | job fails `stock_attribution_missing`; fills unchanged |
| `svg_export_has_credits` | export attributed frame | `<metadata>` with one credit |
| `png_export_has_itxt` | export | `iTXt` `Credits` chunk present |
| `fig_round_trip_keeps_credit` | export `.fig`, import | attribution equal; `plugin_data` has no leftover key |
| `replacing_image_clears_credit` | set a solid fill on the node | attribution removed in the same undo step |

Visual evidence: Inspect capture at 1440 and terminal output of the export checks.

## 11. Performance budgets

Same as E15-39. Exports add credits by walking only the exported subtree (no whole-document scans).

## 12. Dependencies and build order

- Needs E15-39. Parallel with E15-41.

## 13. Owner rulings that apply

- R4.13: the agent CLI finds and fetches the photo; the app calls no stock service.
- Nothing vanishes silently: credits stay with the layer and reach exports.
