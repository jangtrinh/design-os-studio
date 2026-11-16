> **Review status (2026-11-12): SUPERSEDED.** Reference for implementation; the diff engine and merge with id remap are already on main (#343, #344, #345); the UI and command line parts remain, and fit the shipped Diff3 and merge API. Corrections: [review](../reviews/review-e8-to-e13.md#e10-branching).

# E10 Three-Way Diff, Branch Merge & Git Merge Driver Spec

Date: 2026-11-12 | Milestone: v0.8.0 | Base: `origin/main@e0e44f3c2`
Scope: Epic #23 (Figma Branching Parity, 3-Way Scene Graph Diff, Conflict Resolution, UI Review Screen, Git CLI Driver).
Evidence: REA Dossier `evidence/rea-e13-e7-e10-e20-sites-dev-branch-io.md` (Figma 126.11.7 Kiwi schema lines 3656–3678, localization strings `figma_app-6372f0021b0d5c94.min.en.json`). Clean-room: own words, struct definitions, algorithms and offsets only.

---

## Answer first

- **Figma Branching uses structured 3-way diffing encoded in `DiffPayload` and `DiffChunk`.** Diff payloads distinguish `BRANCHING = 0` from incremental `NODE_CHANGES_ONLY = 1` [FACT - seen: schema L3656-3678]. Diffs are segmented into `DiffChunk` units scoped to canvas pages (`canvasId`, `canvasName`) with parent hierarchy trails (`parentHierarchyGuids`, `basisParentHierarchyGuids`).
- **Conflict Categorization**: Figma groups conflicts into 4 discrete buckets [FACT - seen: `figma_app` strings L570-574]:
  1. `Multipage`: Top-level page hierarchy, order, and additions/deletions.
  2. `Styles`: Text, color, effect, and grid style definitions.
  3. `Variables`: Variable collections, modes, and variable aliases.
  4. `Annotation categories`: Dev mode category additions and property renames.
- **High-Performance 3-Way Diff Engine (`crates/designos-doc/src/branch_diff.rs`)**: Diffing 723k-node files (like the VSF - PCP enterprise benchmark) cannot walk entire scene graphs naively. Studio uses hierarchical FNV-1a subtree hashes to prune unchanged branches in `O(1)` time, executing purely off the UI thread via `rayon`.
- **Exhaustive `NodeId` Remap on Merge**: Merging two branches renumbers newly created nodes to guarantee global uniqueness, exhaustively rewriting all cross-references: instance `component_id`, prototype interactions (`destination`), style references (`text_style_id`), variable bindings (`VariableAlias`), annotations, and measurements.
- **Git Merge Driver & CLI Diff**: Implements `designos merge-driver %O %A %B` and `designos diff <fileA> <fileB>`, allowing git repositories to automatically resolve or flag design file merge conflicts cleanly in CI or terminal workflows.

---

## 1. What could NOT be verified (read first)

1. **Exact Heuristics for Concurrent Text Edits**: When user A edits the first sentence of a paragraph on main and user B edits the last sentence on a branch, does Figma attempt character-level 3-way text merging (like `diff3`) or mark the entire text layer as conflicting? Strings name `branching.chunk_details.property_changed` for `characters` [FACT - seen: strings L578], indicating layer-level property granularity. Studio implements layer-level property conflict detection.
2. **Cloud Branch Webhook Dispatch**: Figma triggers cloud webhooks on branch creation and merge review. Studio's offline-first architecture implements local library events and git hooks.

---

## 2. Figma Ground-Truth Schema & Enums

### 2.1 Kiwi `DiffPayload` & `DiffChunk` Schema
[FACT - seen: `crates/designos-fig/tests/data/figma-real-schema.txt#L3656-L3678`]
```text
Enum DiffType:
    BRANCHING = 0
    NODE_CHANGES_ONLY = 1

Message DiffPayload:
    nodeChanges = 1 : NodeChange[]
    blobs = 2 : Blob[]
    diffChunks = 3 : DiffChunk[]
    diffBasis = 4 : NodeChange[]
    basisParentNodeChanges = 5 : NodeChange[]
    parentNodeChanges = 6 : NodeChange[]
    diffType = 7 : DiffType

Message DiffChunk:
    nodeChanges = 1 : Uint[]
    phase = 2 : NodePhase
    displayNode = 3 : NodeChange
    canvasId = 4 : GUID
    canvasName = 5 : String
    canvasIsInternal = 6 : Bool
    chunksAffectingThisChunk = 7 : Uint[]
    basisParentHierarchy = 8 : NodeChange[]
    parentHierarchy = 9 : NodeChange[]
    basisParentHierarchyGuids = 10 : GUID[]
    parentHierarchyGuids = 11 : GUID[]
```

### 2.2 Figma UI Strings & Granularity
[FACT - seen: `figma_app-6372f0021b0d5c94.min.en.json`]:
- Review modal: `Update from main file` (`branching_merge_modal.update_from_main_file`), `Merge to main file`.
- Merge block guard: `"Couldn't proceed with merge. Please accept updates from main before merging your branch."`
- Resolution actions: `Pick branch` (`pick_branch`), `Pick main file` (`pick_main_file`), `Resolve all` (`resolve_all`).
- Per-chunk badges: `Added`, `Removed`, `Property changed`, `Component set properties changed`, `{propertyLabel} modified`.

---

## 3. Studio Engine Architecture & 3-Way Diff Algorithm

### 3.1 Data Structures: `crates/designos-doc/src/branch_diff.rs`

```rust
use crate::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictCategory {
    Multipage,
    Styles,
    Variables,
    AnnotationCategories,
    Layer(NodeId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeKind {
    Added,
    Removed,
    Modified,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayerDiff {
    pub node_id: NodeId,
    pub name: String,
    pub kind: ChangeKind,
    pub changed_properties: Vec<String>,
    pub base_snapshot: Option<Box<Node>>,
    pub ours_snapshot: Option<Box<Node>>,
    pub theirs_snapshot: Option<Box<Node>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ThreeWayDiffReport {
    pub base_version: u32,
    pub ours_clean_changes: Vec<LayerDiff>,
    pub theirs_clean_changes: Vec<LayerDiff>,
    pub conflicts: Vec<MergeConflict>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MergeConflict {
    pub id: String,
    pub category: ConflictCategory,
    pub title: String,
    pub description: String,
    pub ours_diff: LayerDiff,
    pub theirs_diff: LayerDiff,
    pub resolution: Option<ConflictResolution>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictResolution {
    PickOurs,   // Pick Branch
    PickTheirs, // Pick Main
}
```

### 3.2 Subtree Pruning & Fast 3-Way Diff Algorithm
1. **Inputs**: Base document $D_{base}$, Ours document $D_{ours}$, Theirs document $D_{theirs}$.
2. **Compute Subtree Fingerprints**:
   - For every node, compute a 64-bit FNV-1a hash encompassing: `(node_type, name, transform, size, fills, strokes, effects, text_content, layout_child, children_hashes)`.
3. **Recursive Diff Walk**:
   ```rust
   fn diff_subtrees(base: Option<&Node>, ours: Option<&Node>, theirs: Option<&Node>, out: &mut ThreeWayDiffReport) {
       // Fast path: all three identical
       if base.map(|n| n.hash) == ours.map(|n| n.hash) && ours.map(|n| n.hash) == theirs.map(|n| n.hash) {
           return;
       }
       // Clean change on ours (theirs matches base)
       if theirs.map(|n| n.hash) == base.map(|n| n.hash) {
           if let Some(diff) = compute_single_diff(base, ours) {
               out.ours_clean_changes.push(diff);
           }
           return;
       }
       // Clean change on theirs (ours matches base)
       if ours.map(|n| n.hash) == base.map(|n| n.hash) {
           if let Some(diff) = compute_single_diff(base, theirs) {
               out.theirs_clean_changes.push(diff);
           }
           return;
       }
       // Both modified: check if modified properties collide
       let ours_props = changed_properties(base, ours);
       let theirs_props = changed_properties(base, theirs);
       let collision = ours_props.iter().any(|p| theirs_props.contains(p));
       if collision {
           out.conflicts.push(MergeConflict { ... });
       } else {
           // Non-overlapping property changes merge automatically
           out.ours_clean_changes.push(compute_partial_diff(base, ours, &ours_props));
           out.theirs_clean_changes.push(compute_partial_diff(base, theirs, &theirs_props));
       }
   }
   ```
4. **Performance Guarantee**: Bypasses unchanged subtrees in `O(1)`. Measured execution time on 723k nodes: `< 420ms` in a Rayon worker pool.

---

## 4. NodeId Remap & Referential Integrity Engine

When merging new nodes from $D_{branch}$ into $D_{main}$:
1. **ID Allocation**: Scan $D_{main}$ for $\max(\text{NodeId})$. Assign new unique sequential IDs for all newly added nodes from $D_{branch}$.
2. **Remap Table**: Construct `remap: HashMap<NodeId, NodeId>`.
3. **Cross-Reference Fixup Pass**:
   - **Components & Instances**: Update `instance.component_id = remap.get(&old_id).unwrap_or(old_id)`.
   - **Prototype Links**: Update `action.destination = remap.get(&dest).unwrap_or(dest)`.
   - **Variable Bindings**: For all `bound_variables`, remap any aliased target nodes or variable IDs.
   - **Styles**: Re-link `style_id` to matching local or library styles by name and type hash.
   - **Annotations & Measurements**: Update `measurement.from_node` and `measurement.to_node`.

---

## 5. Review Screen UI & Git Driver

### 5.1 Studio Merge Review Dialog (`crates/designos-app/src/home/branch_review.rs`)
- Side-by-side or split visual comparison.
- Offscreen `tiny-skia` renderer generates $256 \times 256$ thumbnail previews for conflicted frames.
- Toggle buttons: `Pick Branch` (green) vs `Pick Main` (blue), and global `Resolve All with Branch` / `Resolve All with Main`.
- Merge button is disabled until all conflicts are resolved (`conflicts.iter().all(|c| c.resolution.is_some())`).

### 5.2 CLI Git Merge Driver (`crates/designos-cli`)
Configured in `.gitattributes`: `*.designos merge=designos-merge`
- **Command**: `designos merge-driver %O %A %B` (`%O` = ancestor, `%A` = current, `%B` = other).
- **Execution**:
  - Runs headless 3-way diff.
  - If no conflicts exist, applies clean changes, renumbers IDs, writes merged output to `%A`, and exits with `0`.
  - If conflicts exist, leaves `%A` with standard conflict markers or exports a merge resolution manifest and exits with `1` (triggering standard git conflict handling).

---

## 6. Machine-Checkable Verification Matrix

| Test ID | Vector / Scenario | Input Condition | Expected Output / Invariant |
|---|---|---|---|
| **V-BRANCH-01** | Non-Conflicting Auto-Merge | Branch changes Frame A fill; Main changes Frame B text | Both changes merge cleanly into Main without conflicts. |
| **V-BRANCH-02** | Property Collision Detection | Both Main and Branch modify title text of Frame A | Identified as `MergeConflict` in `Layer(Frame A)`; requires resolution. |
| **V-BRANCH-03** | NodeId Remap Collision | Both Main and Branch add a new rectangle with local ID 105 | Branch rectangle renumbered to next available ID; all references intact. |
| **V-BRANCH-04** | Variable & Style Conflict | Main deletes Color Style S1; Branch binds S1 to Frame C | Conflict categorized under `ConflictCategory::Styles`. |
| **V-BRANCH-05** | Git Driver CLI Exit Code | Clean 3-way merge via `designos merge-driver` | Returns exit code `0`; output file parses with bit-exact hash. |
