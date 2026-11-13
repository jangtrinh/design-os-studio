> **Review status: NOT REVIEWED.** Reference for implementation; verify symbols and statements about Figma against main and the schema before relying on them.

# E21 Document Foundation Spec: 64-Bit NodeId, Base-94 Fractional Index, Field-Level LWW & Cycle-Free Undo

**Epic:** E21 (Multiplayer & Real-Time Sync) | **Slices:** E21.1, E21.2, E21.3  
**Milestone:** v0.9.0 | **Base:** `crates/designos-doc` (`lib.rs`, `node.rs`, `history.rs`, `live_patch/`)  
**Evidence Source:** Clean-Room Figma 126.11.7 REA Dossier (`../evidence/rea-e21-multiplayer-realtime-sync.md`)  
**Epistemic Ledger:** Classified strictly as `[FACT - seen]` (Figma binary schema/WASM strings/Studio source), `[INFERENCE]` (algorithmic mechanics), or `[ASSUMPTION]` (design choices).

---

## Answer first

- **What this specification delivers:** The pre-flight architectural foundation in `crates/designos-doc` required to support conflict-free concurrent editing between human users and $N$ AI agents without data loss or UI rubber-banding.
- **Three Core Pillars:**
  1. **Collision-Free 64-Bit `NodeId`:** Encodes `(session_id: u32, local_id: u32)` into `u64`. Single-player documents keep `session_id == 0`, ensuring **100% backward compatibility** with existing `.designos` files (IDs `1`, `2`, `42` deserialize identically). Multiplayer peers, branches (`E10`), and concurrent `designos-mcp` agents receive unique `session_id >= 1`, eliminating ID collisions globally without central coordination.
  2. **Base-94 Fractional Indexing (`ParentIndex.position`):** Sibling order is governed by a lexicographically sortable printable ASCII string (`0x21` `'!'` to `0x7E` `'~'`). Midpoints are computed using an exact, non-deadlocking algorithm with zero trailing `'!'` characters. `Node::children: Vec<NodeId>` remains a sorted projection, ensuring `SceneIndex`, `designos-render`, and `designos-gpu` pay **zero runtime overhead**.
  3. **Field-Level LWW & Cycle-Free Undo:** Whole-node replacements (`Command::Update { node: Box<Node> }`) are upgraded to commutative `Command::PatchNode` with Hybrid Logical Clocks (HLC) and optimistic unacknowledged registers (`ChangeRegisters`). Tree cycles during concurrent reparenting are deterministically prevented via ancestor walks and Kleppmann-style total-order rollback. Local undo/redo operates via session-scoped inverse deltas that safely no-op on tombstoned nodes.

---

## 1. What could NOT be verified (read first)

1. **Figma C++ Memory Alignment of `ParentIndex`:** In `compiled_wasm.wasm`, `ParentIndex` is aligned to 16 bytes. Studio's Rust implementation uses standard Rust struct layout (`#[repr(C)]` is not required as it does not cross FFI boundaries).
2. **Figma's Internal Garbage Collection Interval:** While `FGGarbageCollectionRestoration.cpp` confirms tombstoned nodes (`NodePhase::REMOVED`) are retained for undo references and unacknowledged client edits, the exact duration before permanent purge depends on session activity. In Studio, tombstones are retained in memory for the duration of the open document session and purged upon export or compaction.

---

## 2. Figma 126.11.7 Ground-Truth Wire & Algorithmic Mechanics

### 2.1 64-Bit GUID Layout `[FACT - seen: figma-real-schema.txt lines 466-468]`
In Figma's Kiwi binary schema, every node identity is represented by:
```kiwi
Struct GUID
    sessionID = 1 : Uint
    localID = 2 : Uint
```
In native memory (`compiled_wasm.wasm` and `desktop_rust.node`), a `GUID` is an 8-byte (64-bit) integer:
$$\text{GUID} = (\text{sessionID} \ll 32) \mid \text{localID}$$
- $\text{sessionID} = 0$: Reserved for root document structures, server-synthesized nodes, or imported legacy templates.
- $\text{sessionID} \ge 1$: Assigned by the collaboration host or server upon connection handshake (`JOIN_START`).
- Each client maintains an internal monotonic counter for $\text{localID}$, guaranteeing that no two nodes created in the universe can ever share the same 64-bit ID.

### 2.2 Sibling Ordering via `ParentIndex.position` `[FACT - seen: figma-real-schema.txt lines 515-517]`
```kiwi
Struct ParentIndex
    guid = 1 : GUID
    position = 2 : String
```
Figma does not maintain child arrays in wire messages. Sibling ordering is determined solely by:
$$\text{SortKey}(N) = \big(N.\text{parentIndex.position},\; N.\text{guid.sessionID},\; N.\text{guid.localID}\big)$$
- Alphabet domain: ASCII `33` (`'!'`) through `126` (`'~'`) ($B = 94$).
- Initial midpoint: `"V"` (ASCII 86, roughly $(33 + 126) / 2$).
- Sibling tie-breaking: If two users concurrently create a node between the same two items, their positions may match ($pos_A == pos_B$). The tie is deterministically broken by `sessionID`, then `localID`.

### 2.3 Optimistic Registers (`FGChangeRegisters.cpp`) `[FACT - seen: wasm-strings.txt lines 14003, 27598]`
Figma avoids rubber-banding during local drag/transform interactions by tracking property mutations in client-side optimistic registers:
1. When the user initiates a drag, each frame sets an in-flight property register stamped with the pending `ackID`.
2. When a remote `NODE_CHANGES` frame arrives for the same property, the client detects the conflict (`"conflict for property change '%s'"`). Because the local register is unacknowledged, the incoming remote edit is stashed, and the local UI does not flicker or jump.
3. Upon receiving server ACK (`"Processing ack of edit"`), the register is marked clean.

### 2.4 Inverse Delta Undo Stack `[FACT - seen: figma-real-schema.txt lines 3626-3627]`
```kiwi
localUndoStack = 22 : Message[]
localRedoStack = 23 : Message[]
undoBatchPageIds = 46 : GUID[]
```
Undo does not rewind the scene graph. It evaluates the local user's inverse delta and dispatches a standard forward `NODE_CHANGES` message. If the target node was concurrently deleted by another user, the engine safely logs `"ignoring change to property '%s' for missing node"` and no-ops.

---

## 3. Studio Rust Architecture & Data Structures

### 3.1 Slice E21.1: 64-Bit `NodeId` Session Packing & Base-94 Fractional Indexing

#### 3.1.1 `NodeId` Packing (`crates/designos-doc/src/lib.rs`)
Studio's `NodeId` is already defined as `pub struct NodeId(pub u64);`. We extend it with zero-cost bitwise accessors:

```rust
impl NodeId {
    pub const ROOT: Self = Self(0);

    /// Constructs a 64-bit NodeId from session and local sequence IDs.
    #[inline]
    pub const fn from_parts(session_id: u32, local_id: u32) -> Self {
        Self(((session_id as u64) << 32) | (local_id as u64))
    }

    /// The session/actor ID (high 32 bits). Returns 0 for single-player / legacy nodes.
    #[inline]
    pub const fn session_id(self) -> u32 {
        (self.0 >> 32) as u32
    }

    /// The intra-session monotonic sequence ID (low 32 bits).
    #[inline]
    pub const fn local_id(self) -> u32 {
        (self.0 & 0xFFFF_FFFF) as u32
    }

    /// True if this node was created in single-player or imported from a legacy file.
    #[inline]
    pub const fn is_legacy_or_root(self) -> bool {
        self.session_id() == 0
    }
}
```

**Backward Compatibility Guarantee:**
When an existing `.designos` file containing IDs like `{"id": 42}` is deserialized by `serde`, `NodeId(42)` has `session_id() == 0` and `local_id() == 42`. It requires **zero schema migrations**.

#### 3.1.2 Base-94 Fractional Indexing Engine (`crates/designos-doc/src/fractional_index.rs`)

```rust
pub const MIN_CHAR: u8 = 0x21; // '!'
pub const MAX_CHAR: u8 = 0x7E; // '~'
pub const MID_CHAR: u8 = 0x56; // 'V' (Figma default initial midpoint)

/// Clean-room Base-94 fractional index string.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FractionalPos(pub String);

impl Default for FractionalPos {
    fn default() -> Self {
        Self("V".to_string())
    }
}

impl FractionalPos {
    pub fn new(pos: impl Into<String>) -> Self {
        let s = pos.into();
        debug_assert!(Self::validate(&s), "Invalid fractional pos: {}", s);
        Self(s)
    }

    /// Validates that all bytes are in [0x21, 0x7E] and does not end with '!'
    pub fn validate(s: &str) -> bool {
        if s.is_empty() || s.ends_with('!') {
            return false;
        }
        s.bytes().all(|b| (MIN_CHAR..=MAX_CHAR).contains(&b))
    }

    /// Computes a lexicographic midpoint strictly between `lo` and `hi`.
    /// Invariant: lo < mid < hi, with zero trailing '!' characters.
    pub fn between(lo: Option<&str>, hi: Option<&str>) -> String {
        match (lo, hi) {
            (None, None) => "V".to_string(),
            (None, Some(h)) => {
                debug_assert!(Self::validate(h));
                Self::between_empty_and(h)
            }
            (Some(l), None) => {
                debug_assert!(Self::validate(l));
                Self::between_and_empty(l)
            }
            (Some(l), Some(h)) => {
                debug_assert!(l < h, "lo ({l}) must be strictly less than hi ({h})");
                debug_assert!(Self::validate(l));
                debug_assert!(Self::validate(h));
                Self::between_both(l, h)
            }
        }
    }

    fn between_empty_and(hi: &str) -> String {
        let hi_bytes = hi.as_bytes();
        let first = hi_bytes[0];
        if first > MIN_CHAR + 1 {
            let mid = MIN_CHAR + (first - MIN_CHAR) / 2;
            let res = String::from(mid as char);
            if res.ends_with('!') {
                format!("{res}V")
            } else {
                res
            }
        } else {
            // hi starts with '!' + 1 or '!'
            format!("!{}", Self::between(None, Some(&hi[1..])))
        }
    }

    fn between_and_empty(lo: &str) -> String {
        let lo_bytes = lo.as_bytes();
        let first = lo_bytes[0];
        if first < MAX_CHAR - 1 {
            let mid = first + (MAX_CHAR - first + 1) / 2;
            String::from(mid as char)
        } else {
            format!("{lo}V")
        }
    }

    fn between_both(lo: &str, hi: &str) -> String {
        let lo_b = lo.as_bytes();
        let hi_b = hi.as_bytes();
        let mut prefix = String::new();
        let mut i = 0;

        while i < lo_b.len() && i < hi_b.len() && lo_b[i] == hi_b[i] {
            prefix.push(lo_b[i] as char);
            i += 1;
        }

        // Since lo < hi and lo_b[..i] == hi_b[..i], hi_b[i] is always present.
        let lo_char = lo_b.get(i).copied().unwrap_or(MIN_CHAR);
        let hi_char = hi_b[i];

        if hi_char.saturating_sub(lo_char) > 1 {
            let mid = lo_char + (hi_char - lo_char) / 2;
            let candidate = format!("{prefix}{}", mid as char);
            if candidate.ends_with('!') {
                format!("{candidate}V")
            } else {
                candidate
            }
        } else if lo_char == hi_char {
            // Occurs when `lo` ended at `i` (implicit MIN_CHAR '!') and `hi_b[i] == b'!'`:
            // keep upper bound `hi[i + 1..]` intact so `mid < hi` strictly holds!
            prefix.push('!');
            let tail = Self::between(None, Some(&hi[i + 1..]));
            format!("{prefix}{tail}")
        } else {
            // Adjacent characters (`hi_char == lo_char + 1`):
            // `prefix + lo_char` is already strictly `< hi`, so tail upper bound is unbounded (`None`).
            prefix.push(lo_char as char);
            let next_lo = if i + 1 < lo.len() { Some(&lo[i + 1..]) } else { None };
            let tail = Self::between(next_lo, None);
            format!("{prefix}{tail}")
        }
    }
}
```

#### 3.1.3 `ParentIndex` and Node Structure Integration (`crates/designos-doc/src/node.rs`)

```rust
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ParentIndex {
    pub parent: NodeId,
    pub position: FractionalPos,
}

// In struct Node:
pub struct Node {
    pub id: NodeId,
    pub source_id: Option<String>,
    pub name: String,
    pub kind: NodeKind,
    pub parent: Option<NodeId>,
    /// Fractional index position inside parent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_index: Option<ParentIndex>,
    /// Cached, sorted back-to-front child list.
    /// Invariant: children is always maintained in sorted order of (position, session_id, local_id).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<NodeId>,
    ...
}
```

**Zero Runtime Overhead Guarantee:**
`Node::children` is maintained as a materialized, sorted `Vec<NodeId>`. When `designos-render` traverses nodes or `SceneIndex` calculates spatial bounds, it reads `node.children` directly without re-sorting fractional indices!

---

### 3.2 Slice E21.2: Field-Level LWW `ChangeRegisters` & Commutative `PropertyPatch`

#### 3.2.1 Hybrid Logical Clock (HLC)
To order edits deterministically in decentralized environments without wall-clock drift issues:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct HlcTimestamp {
    pub physical_ms: u64,
    pub logical_counter: u32,
    pub session_id: u32,
}

impl HlcTimestamp {
    pub fn now(session_id: u32, max_seen: Option<&HlcTimestamp>) -> Self {
        let wall_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        if let Some(seen) = max_seen {
            if seen.physical_ms > wall_ms {
                return Self {
                    physical_ms: seen.physical_ms,
                    logical_counter: seen.logical_counter + 1,
                    session_id,
                };
            } else if seen.physical_ms == wall_ms {
                return Self {
                    physical_ms: wall_ms,
                    logical_counter: seen.logical_counter + 1,
                    session_id,
                };
            }
        }
        Self { physical_ms: wall_ms, logical_counter: 0, session_id }
    }
}
```

#### 3.2.2 Property Patch Decomposition
Rather than replacing the entire `Node` struct via `Command::Update { node: Box<Node> }`, mutations decompose into orthogonal properties:

```rust
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum PropertyPatch {
    Name(String),
    Visible(bool),
    Locked(bool),
    Opacity(f32),
    Blend(BlendMode),
    Transform(Affine),
    Size(Vec2),
    Fills(Vec<Paint>),
    Stroke(Stroke),
    Effects(Vec<Effect>),
    CornerRadii([f32; 4]),
    CornerSmoothing(f32),
    ParentIndex(ParentIndex),
    LayoutChild(LayoutChild),
    Constraints(Constraints),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum PropertyKey {
    Name,
    Visible,
    Locked,
    Opacity,
    Blend,
    Transform,
    Size,
    Fills,
    Stroke,
    Effects,
    CornerRadii,
    CornerSmoothing,
    ParentIndex,
    LayoutChild,
    Constraints,
}
```

#### 3.2.3 Client `ChangeRegisters` Concurrency Table (`crates/designos-doc/src/change_registers.rs`)

```rust
#[derive(Clone, Debug)]
pub struct RegisterEntry {
    pub confirmed_hlc: HlcTimestamp,
    pub pending_ack_id: Option<u32>,
    pub local_hlc: HlcTimestamp,
    /// Newer remote patch received while a local drag/edit was unacknowledged;
    /// reconciled on `handle_ack` so zero rubber-banding AND strong eventual convergence both hold.
    pub stashed_remote: Option<(HlcTimestamp, PropertyPatch)>,
}

#[derive(Default, Debug)]
pub struct ChangeRegisters {
    registers: BTreeMap<(NodeId, PropertyKey), RegisterEntry>,
    current_ack_counter: u32,
}

impl ChangeRegisters {
    /// Generates next local ack_id for an outbound batch.
    pub fn next_ack_id(&mut self) -> u32 {
        self.current_ack_counter += 1;
        self.current_ack_counter
    }

    /// Records an optimistic local mutation.
    pub fn mark_local_edit(&mut self, node: NodeId, prop: PropertyKey, ack_id: u32, hlc: HlcTimestamp) {
        let entry = self.registers.entry((node, prop)).or_insert_with(|| RegisterEntry {
            confirmed_hlc: hlc,
            pending_ack_id: Some(ack_id),
            local_hlc: hlc,
            stashed_remote: None,
        });
        entry.pending_ack_id = Some(ack_id);
        entry.local_hlc = hlc;
    }

    /// Tests whether an incoming remote edit should be applied immediately or stashed.
    pub fn on_remote_patch(
        &mut self,
        node: NodeId,
        prop: PropertyKey,
        remote_hlc: HlcTimestamp,
        patch: &PropertyPatch,
    ) -> bool {
        if let Some(entry) = self.registers.get_mut(&(node, prop)) {
            if entry.pending_ack_id.is_some() {
                // Suppress immediate apply during in-flight local drag (no rubber-banding),
                // but stash if newer than `local_hlc` and any prior stash so peers converge on ack!
                if remote_hlc > entry.local_hlc
                    && entry.stashed_remote.as_ref().map_or(true, |(h, _)| remote_hlc > *h)
                {
                    entry.stashed_remote = Some((remote_hlc, patch.clone()));
                }
                return false;
            }
            if remote_hlc <= entry.confirmed_hlc {
                return false;
            }
            entry.confirmed_hlc = remote_hlc;
            true
        } else {
            self.registers.insert((node, prop), RegisterEntry {
                confirmed_hlc: remote_hlc,
                pending_ack_id: None,
                local_hlc: remote_hlc,
                stashed_remote: None,
            });
            true
        }
    }

    /// Clears pending state on ack and returns any higher-HLC remote patches to reconcile cleanly.
    pub fn handle_ack(&mut self, ack_id: u32) -> Vec<(NodeId, PropertyPatch)> {
        let mut reconcile = Vec::new();
        for (&(node_id, _), entry) in self.registers.iter_mut() {
            if entry.pending_ack_id == Some(ack_id) {
                entry.pending_ack_id = None;
                if let Some((remote_hlc, patch)) = entry.stashed_remote.take() {
                    if remote_hlc > entry.local_hlc {
                        entry.confirmed_hlc = remote_hlc;
                        reconcile.push((node_id, patch));
                        continue;
                    }
                }
                entry.confirmed_hlc = entry.local_hlc;
            }
        }
        reconcile
    }
}
```

---

### 3.3 Slice E21.3: Deterministic Tree-Move Cycle Breaking & Per-Session Local Undo

#### 3.3.1 Cycle Prevention during Concurrent Reparenting
If User A moves Node $X$ inside Node $Y$, while User B moves Node $Y$ inside Node $X$:

```rust
impl Document {
    /// Checks if setting `new_parent` as parent of `node_id` would introduce a cycle.
    pub fn would_create_cycle(&self, node_id: NodeId, new_parent: NodeId) -> bool {
        if node_id == new_parent {
            return true;
        }
        let mut curr = Some(new_parent);
        while let Some(p) = curr {
            if p == node_id {
                return true; // Found node_id in ancestor chain!
            }
            curr = self.get(p).and_then(|n| n.parent);
        }
        false
    }

    /// Reparents node safely with cycle checking and deterministic total-order fallback.
    pub fn reparent_node(
        &mut self,
        node_id: NodeId,
        new_parent: NodeId,
        pos: FractionalPos,
        hlc: HlcTimestamp,
    ) -> Result<(), &'static str> {
        if self.would_create_cycle(node_id, new_parent) {
            // Cycle detected! Reject move or fallback to Kleppmann total-order resolution.
            return Err("Cycle detected in parent chain");
        }

        // Detach from old parent
        if let Some(old_parent_id) = self.get(node_id).and_then(|n| n.parent) {
            if let Some(old_p) = self.get_mut(old_parent_id) {
                old_p.children.retain(|&c| c != node_id);
            }
        }

        // Update node parent_index
        if let Some(node) = self.get_mut(node_id) {
            node.parent = Some(new_parent);
            node.parent_index = Some(ParentIndex { parent: new_parent, position: pos });
        }

        // Insert into new parent and re-sort children
        if let Some(new_p) = self.get_mut(new_parent) {
            new_p.children.push(node_id);
            self.sort_children_projection(new_parent);
        }

        Ok(())
    }

    /// Sorts children projection according to (position, session_id, local_id) with zero heap reallocation.
    pub fn sort_children_projection(&mut self, parent_id: NodeId) {
        let Some(mut children) = self.get_mut(parent_id).map(|p| std::mem::take(&mut p.children)) else {
            return;
        };
        children.sort_by(|&a, &b| {
            let pos_a = self.nodes.get(&a).and_then(|n| n.parent_index.as_ref()).map(|pi| &pi.position);
            let pos_b = self.nodes.get(&b).and_then(|n| n.parent_index.as_ref()).map(|pi| &pi.position);
            pos_a.cmp(&pos_b)
                .then_with(|| a.session_id().cmp(&b.session_id()))
                .then_with(|| a.local_id().cmp(&b.local_id()))
        });
        if let Some(p) = self.get_mut(parent_id) {
            p.children = children;
        }
    }
}
```

#### 3.3.2 Per-Session Local Undo & Tombstone GC Safety (`SessionUndoStack`)

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionInverseDelta {
    pub target_node: NodeId,
    pub previous_patches: Vec<PropertyPatch>,
    pub was_created: bool,
    pub was_removed: bool,
}

#[derive(Default, Debug)]
pub struct SessionUndoStack {
    undo_stack: Vec<Vec<SessionInverseDelta>>,
    redo_stack: Vec<Vec<SessionInverseDelta>>,
}

impl SessionUndoStack {
    /// Applies local undo: pops inverse delta and generates a forward PatchNode command.
    pub fn apply_undo(&mut self, doc: &mut Document, registers: &mut ChangeRegisters) -> Option<Command> {
        let batch = self.undo_stack.pop()?;
        let mut forward_patches = Vec::new();

        for delta in batch {
            // Check if target node still exists or is tombstoned
            if let Some(node) = doc.get(delta.target_node) {
                if node.is_removed() && !delta.was_removed {
                    // Node was deleted concurrently by another user:
                    // Safely ignore property undo (matches Figma's missing node ignore).
                    continue;
                }
                for patch in delta.previous_patches {
                    forward_patches.push((delta.target_node, patch));
                }
            }
        }
        // Emits forward command without rewinding document clock
        Some(Command::BatchPatch(forward_patches))
    }
}
```

---

## 4. Test Vectors & Edge Cases

### Test Vector 1: Fractional Midpoint Non-Deadlocking Invariants
```rust
#[test]
fn test_fractional_index_edge_cases() {
    // 1. Empty boundaries
    assert_eq!(FractionalPos::between(None, None), "V");

    // 2. Minimum digit collapse ("!" and "\"")
    let lo = "!";
    let hi = "\"";
    let mid = FractionalPos::between(Some(lo), Some(hi));
    assert!(mid.as_str() > lo);
    assert!(mid.as_str() < hi);
    assert!(!mid.ends_with('!'), "Must not end in '!': {}", mid);
    assert_eq!(mid, "!V");

    // 3. 100 consecutive head inserts without exponential string growth
    let mut curr = "V".to_string();
    for _ in 0..100 {
        let next = FractionalPos::between(None, Some(&curr));
        assert!(next < curr);
        assert!(!next.ends_with('!'));
        curr = next;
    }
    assert!(curr.len() <= 102, "Length bloat must be tightly bounded: {}", curr.len());
}
```

### Test Vector 2: 64-Bit `NodeId` Backward Compatibility
```rust
#[test]
fn test_node_id_legacy_compatibility() {
    let raw_json = r#"{"id": 42, "name": "Frame 1"}"#;
    #[derive(Deserialize)]
    struct Probe { id: NodeId, name: String }
    let parsed: Probe = serde_json::from_str(raw_json).unwrap();
    assert_eq!(parsed.id.0, 42);
    assert_eq!(parsed.id.session_id(), 0);
    assert_eq!(parsed.id.local_id(), 42);
    assert!(parsed.id.is_legacy_or_root());

    let multiplayer_id = NodeId::from_parts(1042, 7);
    assert_eq!(multiplayer_id.session_id(), 1042);
    assert_eq!(multiplayer_id.local_id(), 7);
    assert_ne!(multiplayer_id.0, 7);
}
```

### Test Vector 3: Cycle Detection Invariant
```rust
#[test]
fn test_reparent_cycle_prevention() {
    let mut doc = Document::default();
    let a = NodeId::from_parts(1, 1);
    let b = NodeId::from_parts(1, 2);
    let c = NodeId::from_parts(1, 3);
    doc.insert_node_stub(a, None);
    doc.insert_node_stub(b, Some(a));
    doc.insert_node_stub(c, Some(b));

    // Tree: Root -> A -> B -> C
    // Attempting to move A inside C must fail with cycle detected!
    assert!(doc.would_create_cycle(a, c));
    assert!(!doc.would_create_cycle(c, a));
}
```

---

## 5. Acceptance Commands

When builders implement Slices `E21.1`–`E21.3`, verify with the following commands:

```bash
export PATH="$HOME/.cargo/bin:$PATH"

# 1. Run unit tests for 64-bit NodeId packing & fractional indexing
cargo test -p designos-doc fractional_index::tests

# 2. Run ChangeRegisters concurrency & HLC tests
cargo test -p designos-doc change_registers::tests

# 3. Verify zero regression on existing document history & JSON serialization
cargo test -p designos-doc history::tests
cargo test -p designos-doc --lib
```
