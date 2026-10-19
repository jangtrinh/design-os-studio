# E15-1 Place component instances and variants from apply_spec (`kind: "instance"`)

| Field | Value |
|---|---|
| Epic / pack | E15 AI design / v0.6 AI design |
| Size / priority | L / P0 |
| Depends on | none |
| Unblocks | E15-2, T7.9 extension (instance-ready results), E15-16, part-2 style and variant tasks |
| Touches | `crates/designos-doc/src/spec.rs`, new `crates/designos-doc/src/spec_instance.rs`, `crates/designos-mcp/src/engine.rs` (dispatch only), `crates/designos-mcp/src/tools/library_source.rs` (new) |

## 1. Summary and user story

As an agent building a screen in a file that already has a design system, I place the file's own
components (local or from an enabled library) as real instances, pick their variants, and override
text, fills, visibility, nested instances and component properties, so the result stays linked to
the system instead of being a redrawn copy.

## 2. Current state (FACT)

- `NodeSpec` (`crates/designos-doc/src/spec.rs:46-124`) has no component reference, variant map or
  override list. Its doc comment lists the kinds `frame, rectangle, ellipse, text, section, component`
  (`spec.rs:49`).
- `build_spec_node` maps kinds at `spec.rs:389-414`; any other kind fails with the free-text error
  `Unsupported spec node kind: {other}` (`spec.rs:413`). `"component"` creates an empty
  `NodeKind::Component` (`spec.rs:391-396`), never an instance.
- `NodeSpec` has no `#[serde(deny_unknown_fields)]` (`spec.rs:45`), so an agent sending
  `"component": "Button"` today gets a silent 200x100 frame.
- The instance model exists: `NodeKind::Instance { frame, component, component_key }`
  (`crates/designos-doc/src/node.rs:432-436`); every node of the copy carries
  `component.source` (`node.rs:247-251`); overridable fields are the 16 `OverrideField` values
  (`node.rs:168-190`); property definitions and values are `ComponentPropDef` / `PropValue`
  (`node.rs:193-219`).
- Pure, undoable operations exist and return `Command`s: `create_instance`
  (`crates/designos-doc/src/components.rs:387`), `swap_instance` (`components.rs:958`),
  `set_instance_prop` (`components.rs:977`), `parse_variant_name` (`components.rs:271`) and the
  variant switch `variant_target` (`components.rs:580`).
- Library copies: `LibrarySnapshot` (`crates/designos-doc/src/library_ops.rs:45`),
  `import_asset(doc, snap, key, place)` (`library_ops.rs:1034`), `library_component_copies`
  (`library_ops.rs:478`), `local_component_roots` (`library_ops.rs:462`). Enabled libraries are
  `Document.library.enabled` (`crates/designos-doc/src/library.rs:57`); snapshots load through
  `Library::load_snapshot` (`crates/designos-library/src/publish.rs:199`).
- MCP `designos_apply_spec` clones the document, builds on the clone and commits one
  `Command::Insert` (`crates/designos-mcp/src/engine.rs:251-270`). Errors are plain strings
  (`engine.rs:548-550`).

## 3. Behaviour rules

### 3.1 Component reference resolution

`component` is a string. Resolution order; the first rule that matches wins, and a rule that
matches more than one component is an error, never a guess:

1. `"node:<id>"`: a local node id. Must be `Component` or `ComponentSet`.
2. A component key: equal to a key in `Document.library.external` whose asset is a component or
   set (local copy node), or to a key published by an enabled library snapshot.
3. A name: exact, case-sensitive match of the whole string on component or set names (names may
   contain `/`, e.g. `Icon/Arrow`). Local components are searched first; only when none match are
   all enabled libraries searched together (a hit in two libraries is ambiguous). Only when the whole string matches nothing and the part before the first
   `/` equals an enabled library name is the rest matched inside that library
   (`"Untitled UI/Button"`).

Disabled libraries are never searched. A matched library component with no local copy is imported
with `import_asset` inside the same trial document, so the import and the placement are one undo
step.

### 3.2 Variant selection

- `variant` is a map `{property: value}`. When the resolved node is a `ComponentSet`, the target is
  the child `Component` whose `parse_variant_name` pairs equal the set default pairs overwritten by
  `variant`. The set default is the first variant in child order (the Figma default variant).
- A property name that no variant uses fails with `UNKNOWN_VARIANT_PROPERTY`; a value that no variant
  has for that property fails with `UNKNOWN_VARIANT_VALUE`. Both list the valid options.
- A combination where every value exists but no variant matches fails with `NO_SUCH_VARIANT` and
  lists the nearest 5 existing combinations (count of differing pairs, then document order).
- `variant` on a plain `Component` is an error unless every pair already matches its own name.

### 3.3 Overrides

`overrides` is an ordered list; later entries win on the same field. Each entry addresses exactly
one descendant of the new instance:

- `path`: `/`-separated layer names from the instance root, e.g. `"Header/Title"`. Each segment must
  match exactly one child name at that level; `Name[2]` selects the second sibling with that name
  (1-based). A segment that matches zero or several children fails with `UNKNOWN_OVERRIDE_PATH` or
  `AMBIGUOUS_OVERRIDE_PATH`, listing the child names at the failing level.
- `id`: the main-component node id the descendant mirrors (`component.source`), for agents that
  read the component first. Exactly one of `path` or `id`.

Supported fields per entry, each recorded in the descendant's `component.overrides`:

| Field | Type | Effect | OverrideField |
|---|---|---|---|
| `text` | string | Replaces characters of a text layer; clears runs | `Characters` |
| `fill` | colour string (hex, `var:` per E15-3) | Single solid or bound fill | `Fills` |
| `stroke` | colour string | Single solid or bound stroke | `Stroke` |
| `visible` | bool | Show or hide | `Visible` |
| `opacity` | number 0..1 | Layer opacity | `Opacity` |
| `swap` | component reference (3.1) | Instance swap on a nested instance | `MainComponent` |
| `variant` | map | Variant switch on a nested instance | via nested `set_instance_prop` |
| `props` | map | Component properties of a nested instance | via `set_instance_prop` |

`props` on the instance itself sets Boolean, Text and InstanceSwap properties by definition name,
through `set_instance_prop`. Unknown property names fail with `UNKNOWN_COMPONENT_PROPERTY`; a value
of the wrong type fails with `PROPERTY_TYPE_MISMATCH` (expected type named).

### 3.4 Geometry and layout

- `x`, `y`, `width`, `height`, `width_sizing`, `height_sizing` and min/max apply to the instance
  root. Size set from the spec records `OverrideField::Size`; otherwise the main component size is
  kept (`size_comes_from_main`, `components.rs:701`).
- `children`, `preset`, `layout`, `text` and `fill` on the instance root itself are rejected with
  `FIELD_NOT_ALLOWED_ON_INSTANCE` (children belong to slots, E15-2; content changes go in
  `overrides`).
- Instances are never detached; `component.source` links survive.

### 3.5 Atomicity

All resolution happens before any node is allocated. Any error leaves the document and history
unchanged (the existing trial clone at `engine.rs:262` already guarantees this; the resolver must not
write outside the trial).

## 4. Data model and schema changes

New `NodeSpec` fields (all `#[serde(default, skip_serializing_if = ...)]`, so existing specs and
`export_spec` output stay byte-identical):

```rust
pub component: Option<String>,
pub variant: BTreeMap<String, String>,
pub props: BTreeMap<String, serde_json::Value>,
pub overrides: Vec<OverrideSpec>,

pub struct OverrideSpec {
    pub path: Option<String>,
    pub id: Option<u64>,
    pub text: Option<String>,
    pub fill: Option<String>,
    pub stroke: Option<String>,
    pub visible: Option<bool>,
    pub opacity: Option<f32>,
    pub swap: Option<String>,
    pub variant: BTreeMap<String, String>,
    pub props: BTreeMap<String, serde_json::Value>,
}
```

- `OverrideSpec` uses `#[serde(deny_unknown_fields)]`. `NodeSpec` gains `deny_unknown_fields` in
  E15-5 (unknown keys become a named error there), not here.
- `export_spec` of an instance emits `kind:"instance"`, `component` (key if present, else
  `node:<id>`), `variant` (from the variant name) and `overrides` for every field in
  `component.overrides`, so export then apply round-trips.
- New module `spec_instance.rs` holds `resolve_component`, `resolve_override_path` and
  `apply_overrides`. It takes `&[LibrarySnapshot]` so designos-doc does not depend on
  designos-library; designos-mcp loads the snapshots of `library.enabled`.

## 5. Contracts

Tool: `designos_apply_spec` (existing name, schema extended). Input unchanged at the top level
(`parent_id`, `spec`); `spec.kind` gains `"instance"`. Output adds:

```json
{
  "root_node_id": 812,
  "parent_id": 1,
  "changed": true,
  "name_to_id": {"Buy button": 812, "Buy button/Label": 813},
  "instances": [{"node_id": 812, "component_id": 40, "component_key": "a1b2", "variant": {"Size": "lg"}}],
  "imported_assets": ["a1b2"]
}
```

`name_to_id` keys are slash paths from the spec root, matching E15-4.

Errors (envelope defined in E15-5; all `safe_to_retry: true` because nothing is written):

| Code | When | `valid_options` |
|---|---|---|
| `UNKNOWN_COMPONENT` | no match | 5 closest names (Levenshtein, then library order) |
| `AMBIGUOUS_COMPONENT` | name path matches several | each match as `node:<id>` with library |
| `UNKNOWN_VARIANT_PROPERTY` | property not in set | all property names |
| `UNKNOWN_VARIANT_VALUE` | value not in property | all values of that property |
| `NO_SUCH_VARIANT` | combination missing | 5 nearest combinations |
| `UNKNOWN_OVERRIDE_PATH` / `AMBIGUOUS_OVERRIDE_PATH` | path segment fails | child names at that level |
| `UNKNOWN_COMPONENT_PROPERTY` / `PROPERTY_TYPE_MISMATCH` | props | property names / expected type |
| `FIELD_NOT_ALLOWED_ON_INSTANCE` | children, preset, layout on root | allowed fields |
| `OVERRIDE_FIELD_NOT_APPLICABLE` | `text` on a non-text layer, `swap` on a non-instance | none |

Corrected-example rule: the error's `corrected_example` is the offending spec fragment with the
failing value replaced by the first valid option, e.g. for `variant:{Size:"xl"}` on a set with
`sm, md, lg` the example is `{"kind":"instance","component":"Button","variant":{"Size":"sm"}}`.

Idempotency and undo: inherited from the call (E15-4, E15-6). One call is one undo step including
any library import.

## 6. File-format mapping

- `.designos`: instances are ordinary `Node`s; nothing new is persisted.
- `.fig` export/import: instances already map to `INSTANCE` with `symbolData`; the acceptance test
  verifies the link survives `export_fig` then re-import (component id or key equal).
- JSON scene format (#570): `ref` + `descendants` overrides; covered by #570, verified here only by a
  round-trip test once #570 lands.

## 7. UI

No new Studio surface. Instances created by the agent appear in the Layers panel with the existing
instance icon and in the canvas with the existing cyan instance chrome (`theme::COMPONENT`). The live
patch path (`server.rs:252-267`) carries them like any other insert.

## 8. Edge cases

1. Component set with zero variants: `UNKNOWN_VARIANT_PROPERTY` with an empty option list and a
   message that the set is empty.
2. Two enabled libraries publish a component named `Button`: unprefixed `"Button"` with no local
   match fails `AMBIGUOUS_COMPONENT`; `"Lib A/Button"` succeeds.
3. Override path through a nested instance (`"Card/Avatar/Initials"`): resolves into the nested
   instance's mirrored subtree and records the override on that descendant.
4. `swap` to a component whose root size differs: swap applies and keeps the nested instance's
   overridden size only when `Size` was already overridden.
5. Placing an instance of the component the spec is being built inside (cycle): rejected with
   `INSTANCE_CYCLE` (mirrors the guard in `create_instance`, `components.rs:389`).
6. Library component whose local copy is stale versus the enabled version: placement uses the local
   copy and the reply lists it under `stale_library_assets`; no silent update.
7. 500-instance spec: resolution caches component lookups per call (one name index build).

## 9. Non-goals

- Slot filling (E15-2), variable and style references (E15-3).
- Updating a stale library copy (existing library update flow).
- Creating new components or variants from a spec.

## 10. Test plan

Seam: `designos-doc` unit tests in `spec_instance.rs` plus `cargo test -p designos-mcp` dispatch
tests. Fixture `fixtures::button_library()` (built in code): a `ComponentSet` "Button" with variants
`Size=sm|md|lg` x `State=default|hover`, each holding a text "Label" and an instance "Icon", plus a
second component "Icon/Arrow".

| Case | Input | Expected |
|---|---|---|
| `places_linked_variant` | `{"kind":"instance","component":"Button","variant":{"Size":"lg"}}` | root is `Instance` whose `component` is the `Size=lg, State=default` variant id; no detached copy |
| `name_path_override_only_touches_target` | plus `"overrides":[{"path":"Label","text":"Buy"}]` | Label characters `Buy`, `Characters` in its overrides; Icon has no overrides |
| `invalid_variant_value_is_named_and_atomic` | `"variant":{"Size":"xl"}` | error `UNKNOWN_VARIANT_VALUE`, `valid_options == ["sm","md","lg"]`, `doc_hash` before == after, history depth unchanged |
| `swap_nested_instance` | `[{"path":"Icon","swap":"Icon/Arrow"}]` | nested instance points at Icon/Arrow, `MainComponent` override |
| `library_component_imports_once` | enabled snapshot with key `k1`, two instances | one `import_asset`, both instances link to the same copy, one undo removes all |
| `round_trip_designos_and_fig` | place, `to_json`/`from_json`, `export_fig`/import | instance link and variant preserved |
| `export_then_apply_round_trips` | `export_spec` of the instance then `apply_spec` | equal override sets |

Fail-before: `places_linked_variant` fails on main with `Unsupported spec node kind: instance`.

## 11. Performance budgets

- Resolution builds one name index per call: O(components) with a `HashMap`; budget 5 ms for a
  document with 5,000 components.
- No full-document walk per instance. On the large fixture (112,474 scene nodes / 723,136 records;
  CI uses the synthetic generator, see README) a 50-instance spec applies in under 150 ms
  release on the reference machine; recorded in the PR.
- The trial clone at `engine.rs:262` is a known O(document) cost. E15-4 keeps one whole-document
  trial clone per batch and owns its measured budget (E15-4 §11, test `trial_clone_budget_on_large_synthetic`);
  a scoped trial is not part of v0.6. This task adds no second clone.

## 12. Dependencies and build order

None upstream. Build first in the grounding track; E15-2 and E15-3 extend the same resolver module.

## 13. Owner rulings

- Full scope, never MVP (2026-10-17): every override type in 3.3 ships together.
- All AI through the user's agent CLI (R4.13): this is engine-side resolution only; no model.
- Parallel agents hard-locked to frames (RD-3): placement under a parent outside the caller's lease
  is refused by RD-3's guard, not here.
- Visual evidence: PR includes a terminal capture of the tests and a CPU render of the fixture
  instance.
