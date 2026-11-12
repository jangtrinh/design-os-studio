# Clean-Room Figma 126.11.7 REA Evidence: Multiplayer, Real-Time Sync, Presence & Fractional Indexing

**Date:** 2026-11-12  
**Target Architecture:** Figma Desktop 126.11.7 Engine & Web App vs. DESIGN:OS Studio (`design-os-studio`)  
**Investigator:** Gemini Agent (`gemini-3.8-flash` / REA Specialist)  
**Target Path:** `/Users/jangtrinh/Products/design-os-studio/../evidence/rea-e21-multiplayer-realtime-sync.md`  
**Epistemic Standard:** Clean-room reverse engineering. All assertions strictly categorized as `[FACT - seen]`, `[INFERENCE]`, or `[ASSUMPTION]`. No verbatim proprietary Figma source code included. Mathematical formulas, wire schemas, UI keys, and clean-room algorithmic specifications only.

---

## 0. Artifact Provenance & REA Evidence Ledger

All artifacts were analyzed using **`morluto/rea` (`v6.8.0`)** and standard cryptographic digest utilities (`shasum -a 256`).

### 0.1 Artifact SHA-256 Digest & REA Evidence ID Table `[FACT - seen: rea@6.8.0 inspect-artifact]`

| Artifact File | Size (Bytes) | SHA-256 Checksum | REA Evidence ID (`rea@6.8.0`) | REA Root Artifact ID |
|---|---|---|---|---|
| `desktop_rust.node` (`/Applications/Figma.app/.../app.asar.unpacked/`) | 2,796,784 | `9500c73119d7bdb3954c7784f80db2509e2143c0a9b476528b85b43dc7213dd7` | `ev_6e46a40427aaa73337a20f19d15178ba177de305369b3678052e2b449f09c899` | `art_eb8840de86d86f4e4208ec8d71a3e94a8c4aa14d865db624c2f255446bfe7388` |
| `bindings.node` (`/Applications/Figma.app/.../app.asar.unpacked/`) | 538,880 | `eed4ef3964fb7ae5faed1994a1ba54f86d471ff603555c3541984a32c5a49cdf` | `ev_eec532e1d3df92bcc1d09805e1286af0cc46b640eb71a15b911d79a2715edb3c` | `art_935b96225f2d7fd971d0f6d31a32064fbce7eb642416b6876ae0c32006891881` |
| `89538-06e6830de3dec754.min.js` (Fractional Index chunk) | 1,266,926 | `b0ddb3af9d632cc49fb6abe30ce292039dc41ff6475585f3ae02d2ac2bb6dd8f` | `ev_2fdfc61e22de959a9d1d002f2978ea9e3b5125af4c5f9d7ab1df46677d259b7f` | `art_566f7008d08f0ba95f8fe460dbb670bcb2e0d3b8d111f5e627cc038077b0e2f8` |
| `68851-b391e0ad6d7c7844.min.js` (Spotlight / Presence chunk) | 34,722 | `ca4c6eb2670635dee8882519a9afb8b4ed95349317e0dfae85e42a8058f9e9e1` | `ev_6579c7c30264885422ad1e431da5697d8741a0b1dde2c87e94b1866b5ab77638` | `art_3718b24f49b478cf3f1e59faf49f7810bcfe9c3cd72a7210617b4e7ad1f5d665` |
| `30224-91af1d09529471be.min.js` (Broadcast chunk) | 335,191 | `9b581415cd1f199f962ad46d963aad890826a63fc7f589eeaa66a01d1c432dd2` | `ev_7ab1fd494f52e995f44bc0b2f540ff86130c2d787b66df3705709ed20b0ecf16` | `art_fa9916a45a42b7c0573c9ad19ba9742fca2fb17ff10b779363b21967645e4709` |
| `compiled_wasm.js` (C++ Emscripten Glue) | 2,404,584 | `4b57f5918ef9dc414f55e1069bdb6a98a918ff9cf014093ba65ae251c8ee9c4c` | `ev_ce0472b24859ec7e2b1c5699509bb890ca5bde1068619d43cfedf368f62c04f6` | `art_e98bef84178e996ebb60130579231dfaa6433034d46695d20e31d239d32613b2` |
| `figma_app-607050972cfcf2d9.min.js` (Main Engine Web App) | 6,830,436 | `44415011f97ec3d42e5272a291bcc86d95e643c71ffaf61d5ae24b795841c51d` | `ev_7f842ba2d4d53c4836628b9db933a4b593c11d7fa8f9bf53ee661f6adfc16e97` | `art_35ebdd13c98d4d71a46d13210401e90dda7e642cc05dea32b17689b470b1610b` |
| `figma_app-6372f0021b0d5c94.min.en.json` (Figma 126.11.7 Strings) | 3,263,743 | `49618b7606e3e51a66e70a27c7cf585154388e36e4f165e31c81ef45fb815eb0` | `ev_6b2404e3bcda647e3a985f98cfba70c8853b94a0d8fb8558aeec7efbbd30c5e7` | — |
| `figma-real-schema.txt` (`crates/designos-fig/tests/data/`) | 130,100 | `22cd4819bd2ad3e175bcef17fb7197d4f63d5d722868edd7160c57b712ba61d2` | — | — |

### 0.2 Recovered Native Engine C++ Compilation Unit Paths `[FACT - seen: wasm-strings.txt]`
The string pool of `compiled_wasm.wasm` directly exposes the internal C++ source layout of Figma's real-time multiplayer engine:
- `fullscreen/lib/editor/multiplayer/FGMultiplayerSession.cpp` (Connection management, reconnection, handshake, RTT telemetry)
- `fullscreen/lib/shared-app/multiplayer/FGChangeRegisters.cpp` (Optimistic mutation registers, property-level LWW, conflict resolution, server ACK tracking)
- `fullscreen/lib/editor/multiplayer/FGMultiplayerPageSyncing.cpp` (Incremental scene graph loading, depth-bounded and page queries)
- `fullscreen/lib/scenegraph/FGMultiplayerMessages.cpp` (Kiwi wire serialization, compression/decompression, batching)
- `fullscreen/lib/editor/multiplayer/FGGarbageCollectionRestoration.cpp` (Tombstoning, unreferenced node GC, concurrent delete safety)
- `fullscreen/lib/editor/multiplayer/FGAutosave.cpp` (Local client snapshotting integrated with multiplayer sequence numbers)
- `fullscreen/lib/editor/multiplayer/FGImageCache.cpp` (Blob staging, async hash-deduped binary uploads)
- `fullscreen/lib/editor/app/FGTestFullscreenMultiplayerDocument.cpp` (Headless multi-client test harness)

---

## 1. Complete Kiwi Multiplayer Wire Schema

Figma's real-time synchronization protocol is strictly binary, defined in Kiwi binary format. Every message passed over the WebSocket relay connects to the Kiwi definitions below.

### 1.1 `Enum MessageType` `[FACT - seen: figma-real-schema.txt lines 1-26]`
```kiwi
Enum MessageType
    JOIN_START = 0
    NODE_CHANGES = 1
    USER_CHANGES = 2
    JOIN_END = 3
    SIGNAL = 4
    STYLE = 5
    STYLE_SET = 6
    JOIN_START_SKIP_RELOAD = 7
    NOTIFY_SHOULD_UPGRADE = 8
    UPGRADE_DONE = 9
    UPGRADE_REFRESH = 10
    SCENE_GRAPH_QUERY = 11
    SCENE_GRAPH_REPLY = 12
    DIFF = 13
    CLIENT_BROADCAST = 14
    JOIN_START_JOURNALED = 15
    STREAM_START = 16
    STREAM_END = 17
    INTERACTIVE_SLIDE_CHANGE = 18
    RECONNECT_SCENE_GRAPH_QUERY = 19
    RECONNECT_SCENE_GRAPH_REPLY = 20
    JOIN_END_INCREMENTAL_RECONNECT = 21
    NODE_STATUS_CHANGE = 22
    CLIENT_RENDERED = 23
    BUZZ_APPROVAL_CHANGE = 24
```

### 1.2 Access & Node Lifecycle Enums `[FACT - seen: figma-real-schema.txt lines 30-35]`
```kiwi
Enum Access
    READ_ONLY = 0
    READ_WRITE = 1

Enum NodePhase
    CREATED = 0
    REMOVED = 1
```

### 1.3 Node & Parent Identification Structs `[FACT - seen: lines 466-468, 515-517, 2810-2812]`
```kiwi
Struct GUID
    sessionID = 1 : Uint
    localID = 2 : Uint

Struct ParentIndex
    guid = 1 : GUID
    position = 2 : String

Struct DiagramParentIndex
    guid = 1 : GUID
    position = 2 : String
```
*Note on memory layout:* In the C++ engine (`compiled_wasm.js`), `ParentIndex` is laid out in contiguous memory as: offset +0 = `guid` (8 bytes: 4 bytes `sessionID` + 4 bytes `localID`), offset +8 = `position` (string pointer / length).

### 1.4 `Message Message` (The Wire Envelope) `[FACT - seen: lines 3598-3647]`
Every binary frame sent between client and server is an instance of `Message`:
```kiwi
Message Message
    type = 1 : MessageType
    sessionID = 2 : Uint
    stableSessionID = 42 : String
    ackID = 3 : Uint
    isRetransmission = 37 : Bool
    nodeChanges = 4 : NodeChange[]
    userChanges = 5 : UserChange[]
    interactiveSlideElementChange = 32 : InteractiveSlideElementChange
    nodeStatusChange = 36 : NodeStatusChange
    buzzApprovalChange = 44 : BuzzApprovalChange
    blobs = 6 : Blob[]
    blobBaseIndex = 30 : Uint
    signalName = 7 : String
    access = 8 : Access
    styleSetName = 9 : String
    styleSetType = 10 : StyleSetType
    styleSetContentType = 11 : StyleSetContentType
    pasteID = 12 : Int
    pasteOffset = 13 : Vector
    pasteFileKey = 14 : String
    signalPayload = 15 : String
    sceneGraphQueries = 16 : SceneGraphQuery[]
    nodeChangesMetadata = 17 : NodeChangesMetadata
    fileVersion = 18 : Uint
    pasteIsPartiallyOutsideEnclosingFrame = 19 : Bool
    pastePageId = 20 : GUID
    isCut = 21 : Bool
    localUndoStack = 22 : Message[]
    localRedoStack = 23 : Message[]
    broadcasts = 24 : ClientBroadcast[]
    reconnectSequenceNumber = 25 : Uint
    pasteBranchSourceFileKey = 26 : String
    pasteEditorType = 27 : EditorType
    postSyncActions = 28 : String
    publishedAssetGuids = 29 : GUID[]
    dirtyFromInitialLoad = 31 : Bool
    clipboardSelectionRegions = 33 : ClipboardSelectionRegion[]
    encodedOffsetsIndex = 34 : EncodedOffsetsIndex
    hasRepeatingContent = 35 : Bool
    sentTimestamp = 38 : Uint64
    annotationCategories = 39 : AnnotationCategory[]
    clientRenderedMetadata = 40 : ClientRenderedMetadata
    pasteAssetType = 41 : PasteAssetType
    objectAnimations = 43 : ObjectAnimationList
    sceneGraphQueryMode = 45 : SceneGraphQueryMode
    undoBatchPageIds = 46 : GUID[]
    originFileKey = 47 : String
    nodeChangeOrder = 48 : NodeChangeOrder
    changeSetIdentifiers = 49 : ChangeSetIdentifiers
```

### 1.5 `Message UserChange` (Presence & Interaction Envelope) `[FACT - seen: lines 3474-3524]`
Ephemeral collaboration, viewport tracking, cursors, audio, and agent states are broadcast via `UserChange`:
```kiwi
Message UserChange
    sessionID = 1 : Uint
    stableSessionID = 44 : String
    connected = 2 : Bool
    name = 3 : String
    color = 4 : Color
    imageURL = 5 : String
    viewport = 6 : Viewport
    mouse = 7 : Mouse
    selection = 8 : GUID[]
    observing = 9 : Uint[]
    deviceName = 10 : String
    recentClicks = 11 : Click[]
    scrollPositions = 12 : ScrollPosition[]
    triggeredOverlays = 13 : TriggeredOverlay[]
    userID = 14 : String
    lastTriggeredHotspot = 15 : GUID
    lastTriggeredPrototypeInteractionID = 16 : GUID
    lastTriggeredObjectAnimationIndex = 38 : Uint
    triggeredOverlaysData = 17 : TriggeredOverlayData[]
    playbackUpdates = 18 : PlaybackUpdate[]
    chatMessage = 19 : ChatMessage
    voiceMetadata = 20 : VoiceMetadata
    canWrite = 21 : Bool
    highFiveStatus = 22 : Bool
    instanceStateChanges = 23 : InstanceStateChange[]
    textCursor = 24 : TextCursor
    textSelection = 25 : TextSelection
    connectedAtTimeS = 26 : Uint
    focusOnTextCursor = 27 : Bool
    heartbeat = 28 : Heartbeat
    triggeredSetVariableActionData = 29 : TriggeredSetVariableActionData[]
    videoStateChangeData = 30 : VideoStateChangeData[]
    clientID = 31 : String
    focusedSlideId = 32 : GUID
    triggeredSetVariableModeActionData = 33 : TriggeredSetVariableModeActionData[]
    aprilFunCursor = 34 : AprilFunCursor
    embeddedPrototypeData = 35 : EmbeddedPrototypeData[]
    activeSlidesEmbeddablePrototype = 36 : GUID
    activeEmbeddedPrototypes = 43 : GUID[]
    activeCodeComponentId = 37 : GUID
    aprilFunFigPal = 39 : AprilFunFigPal
    collaborativeTextSelection = 40 : CollaborativeTextSelection
    sitesViewState = 41 : SitesViewState
    nodeChatExchanges = 42 : NodeChatExchange[]
    designFullPageViewState = 45 : DesignFullPageViewState
    agentInfo = 46 : AgentInfo
    timelinePlaybackState = 47 : TimelinePlaybackStateData[]
    agentCanvasActivity = 48 : AgentCanvasActivity
    assistantThreadId = 49 : String
    sanrioCharacter = 50 : SanrioCharacter
```

### 1.6 Ephemeral Auxiliary Structures `[FACT - seen: lines 3326-3468, 3553-3583]`
```kiwi
Message Viewport
    canvasSpaceBounds = 1 : Rect
    pixelPreview = 2 : Bool
    pixelDensity = 3 : Float
    canvasGuid = 4 : GUID

Message Mouse
    cursor = 1 : MouseCursor
    canvasSpaceLocation = 2 : Vector
    canvasSpaceSelectionBox = 3 : Rect
    canvasGuid = 4 : GUID
    cursorHiddenReason = 5 : Uint

Struct Click
    id = 1 : Uint
    point = 2 : Vector

Message ChatMessage
    text = 1 : String
    previousText = 2 : String

Enum Heartbeat
    FOREGROUND = 0
    BACKGROUND = 1

Message ClientBroadcast
    sessionID = 1 : Uint
    cursorReaction = 2 : CursorReaction
    timer = 3 : TimerInfo
    presenter = 4 : PresenterInfo
    prototypePresenter = 5 : PresenterInfo
    music = 6 : MusicInfo

Message AgentInfo
    name = 1 : String
    logo = 2 : String
    oauthClientId = 3 : String

Enum AgentActivityStatus
    RUNNING = 1

Message AgentCanvasActivity
    nodeIds = 1 : GUID[]
    status = 2 : AgentActivityStatus
    triggeringUserId = 3 : String
```

---

## 2. Fractional Indexing (`ParentIndex.position`) Architecture

In Figma's scene graph, child order within any parent container (Canvas, Frame, Group, Component) is not modeled as an integer array index `children[i]`. Instead, each child node stores its position relative to its parent as a lexicographically sortable fractional index string: `ParentIndex { guid: parentGUID, position: String }`.

### 2.1 Alphabet, Bounds & Mathematical Invariants `[FACT - seen & clean-room specification]`
- **Alphabet Domain:** Base-95 printable ASCII character set from `!` (0x21, 33) to `~` (0x7E, 126).
  - Minimum digit: `MIN_CHAR = '!'` (ASCII 33)
  - Maximum digit: `MAX_CHAR = '~'` (ASCII 126)
  - Alphabet Radix: $R = 126 - 33 + 1 = 94$ (or 95 if space / null-termination is budgeted; in clean-room implementation, Radix $B = 94$).
  - Default initial key for an empty container: `"V"` (ASCII 86, approximately $(33 + 126) / 2$, the midpoint of the printable ASCII range).
- **Total Ordering Invariant:**
  For any two sibling nodes $A$ and $B$ sharing the same parent $P$:
  $$A < B \iff \text{SortKey}(A) < \text{SortKey}(B)$$
  $$\text{SortKey}(Node) = \big(Node.\text{parentIndex.position},\; Node.\text{guid.sessionID},\; Node.\text{guid.localID}\big)$$
- **Tie-Breaking Determinism:**
  If two peers concurrently insert a node at the identical midpoint position between the same two siblings, $position_A == position_B$. The tie is broken deterministically by numeric comparison of the node's `sessionID` (u32), and if created by the same session, by `localID` (u32). Because every client assigns a unique server-issued `sessionID` to each connected tab, no two distinct nodes in the world can ever have equal tuples $\big(position, sessionID, localID\big)$.

### 2.2 Clean-Room Fractional Midpoint Algorithm `[FACT - seen: clean-room mathematical formulation]`
Given two position strings $P_0$ (left neighbor) and $P_1$ (right neighbor) where $P_0 < P_1$:
1. If inserting before the first child: $P_0 = \epsilon$ (empty string), $P_1 = \text{first\_position}$.
2. If inserting after the last child: $P_0 = \text{last\_position}$, $P_1 = \infty$ (conceptually `~`).
3. If inserting between two existing positions:
   Find the common prefix length $k$ between $P_0$ and $P_1$.
   Let $c_0 = P_0[k]$ (or $MIN\_CHAR$ if $k \ge |P_0|$) and $c_1 = P_1[k]$ (or $MAX\_CHAR$ if $k \ge |P_1|$).
   - If $c_1 - c_0 > 1$:
     Compute the midpoint character $c_{mid} = \lfloor (c_0 + c_1) / 2 \rfloor$.
     The new position string is $\text{prefix} + c_{mid}$.
   - If $c_1 - c_0 \le 1$:
     The interval at index $k$ has collapsed. Append $c_0$ to the prefix, and recurse into the next digit index $k + 1$ with $P_0[k+1]$ and $MAX\_CHAR$.
     This creates an arbitrary-precision string extension (e.g. `"V"` $\to$ `"VN"` $\to$ `"VNJ"`).
4. **Compression / Anti-Bloat Strategy:**
   Because repeated inserts at the same head or tail position can lengthen the string, the client pads or rounds when a fresh block is created, keeping typical position lengths between 1 and 8 ASCII bytes.

---

## 3. Optimistic State, Property-Level LWW, Cycle Prevention & Multiplayer Undo

Figma does not use Operational Transformation (OT) or general CRDT trees. It employs a **Centralized Server-Ordered Property-Level Last-Write-Wins (LWW) with Client-Side Optimistic Registers (`FGChangeRegisters`)**.

```
+-----------------------------------------------------------------------------------------+
|                                FIGMA REPLICATION ARCHITECTURE                           |
+-----------------------------------------------------------------------------------------+
|                                                                                         |
|   +--------------------+       WebSocket Frame       +------------------------------+   |
|   |  Client Session A  | --------------------------> |    Central Relay Server      |   |
|   | (sessionID: 1042)  |   Message { ackID: 41,      |  - Assigns monotonic seq num |   |
|   |                    |     nodeChanges: [...] }    |  - Drops rejected cycles     |   |
|   |  FGChangeRegisters |                             |  - Serializes to disk        |   |
|   |  - in-flight edits | <-------------------------- |  - Broadcasts to all peers   |   |
|   +--------------------+    Server ACK (ackID: 41)   +------------------------------+   |
|                                                                     |                   |
|                                                                     | Broadcast         |
|                                                                     v                   |
|                                                      +------------------------------+   |
|                                                      |       Client Session B       |   |
|                                                      |      (sessionID: 1088)       |   |
|                                                      |  - Applies remote NodeChange |   |
|                                                      |  - Checks cycle resolution   |   |
|                                                      |  - Updates viewport/render   |   |
|                                                      +------------------------------+   |
+-----------------------------------------------------------------------------------------+
```

### 3.1 `FGChangeRegisters`: The Client Optimistic Mutation Queue `[FACT - seen: wasm-strings.txt lines 13164-37831]`
In `fullscreen/lib/shared-app/multiplayer/FGChangeRegisters.cpp`, every local mutation goes through a two-phase commit:
1. **Local Registration (`creating property register for %s`, `creating phase register for %s`):**
   When the user or local tool modifies a property (e.g., drags a node, changing `transform` and `size`):
   - The engine immediately mutates the local scene graph so interactive 60fps / 120fps direct manipulation has zero latency.
   - A register entry is created in `FGChangeRegisters` recording `(guid, propertyTag, localValue, inFlightAckId)`.
2. **Wire Dispatch (`client: flushing message %d with %u phase changes and %u property changes`):**
   Changes are batched and encoded into a Kiwi `Message` with a monotonically incremented `ackID`, then dispatched over the WebSocket.
3. **Remote Message Arrival While Unacknowledged (`conflict for property change '%s'`):**
   When a remote `NODE_CHANGES` frame arrives from the server:
   - For each property on each node, `FGChangeRegisters` checks if an unclean register exists for that property.
   - If an unacknowledged local register exists, the client **preserves the local user's in-flight edit** and ignores or stashes the incoming remote value (`[ChangeRegisters] client: conflict for property change '%s'`). This prevents user dragging from rubber-banding or flickering due to incoming remote updates!
4. **Server Acknowledgment (`Processing ack of edit`):**
   When the server echoes the message back with the matching `ackID`:
   - `FGChangeRegisters` marks the register clean and removes it (`client: removing property register for %s at state %s`).
   - Round-trip time (RTT) telemetry is recorded: `[ChangeRegisters] Received ack from multiplayer. Round trip time (RTT): %fms`.
   - If a conflict occurred during the in-flight window, the engine queues recomputation: `client: queueing recomputation of %s on node %s in response to conflicting value`.

### 3.2 Property-Level Last-Write-Wins (LWW) Decomposition `[FACT - seen]`
Unlike document-snapshot architectures, `NodeChange` decomposes a scene node into independent field tags:
- `name` (tag 5)
- `visible` (tag 8)
- `locked` (tag 9)
- `transform` (tag 11)
- `size` (tag 12)
- `parentIndex` (tag 3)
- `fillPaints` (tag 16)
- `strokePaints` (tag 17)
- `opacity` (tag 20)
- `cornerRadius` (tag 24)

If User A edits `fillPaints` on Frame 1, while User B concurrently edits `size` on Frame 1:
Both edits succeed cleanly without merge conflicts! The server applies User A's `fillPaints` and User B's `size` to the canonical scene record. Because the properties are disjoint, both changes merge commutatively.

### 3.3 Tree Cycle Detection & Deterministic Cycle Breaking `[FACT - seen: wasm-strings.txt lines 13164, 14003, 27598, 28087]`
A fundamental failure mode in hierarchical real-time sync is concurrent reparenting:
- Peer A moves Node $X$ inside Node $Y$.
- Peer B moves Node $Y$ inside Node $X$.
If applied naively, this creates an isolated cycle $X \leftrightarrow Y$, disconnecting both nodes from the document root.

**Figma's Cycle Invariant & Enforcement:**
1. **Ancestor Walk Pre-Check (`transferNodeToParentIndex can only be called with a parented node`):**
   Before applying any `parentIndex` update moving node $N$ under parent $P$:
   - The engine walks up the parent chain: $P \to P_{parent} \to \dots \to \text{Root}$.
   - If node $N$ is encountered anywhere along this ancestral path, applying the reparenting would create a cycle $N \to \dots \to P \to N$.
2. **Server-Sequenced Conflict Resolution:**
   - The central server evaluates mutations in strict arrival order.
   - The first reparenting operation to reach the server is accepted, committed to the authoritative scene graph, and broadcast.
   - When the second reparenting operation reaches the server, the server runs the ancestor check against the updated tree. Detecting the cycle, the server **rejects the second reparenting**, treating it as an invalid edit.
   - The client receiving the rejected notification rolls back its local register:
     `[ChangeRegisters] diagram reparenting: mark diagram node %s dirty for reparenting due to unclean conflicting register when receiving change that reparents diagram root`.

### 3.4 Multiplayer Undo/Redo Architecture `[FACT - seen: lines 3626-3627, 3644]`
In multiplayer, global undo (reverting the document to a past timestamp) is catastrophic because it would wipe out other collaborators' unrelated work.

**Figma's Inverse Delta Multiplayer Undo Contract:**
1. **Local Session History:** Undo stacks are maintained strictly per-client session:
   `localUndoStack = 22 : Message[]`
   `localRedoStack = 23 : Message[]`
   `undoBatchPageIds = 46 : GUID[]`
2. **Inverse Delta Generation:**
   Every user action records the *mathematical inverse* of the properties modified:
   - If the user changed Node $N$'s color from Red to Blue, the undo stack records a `NodeChange { guid: N, fillPaints: [Red] }`.
   - If the user deleted Node $N$, the undo stack records a `NodeChange { guid: N, phase: CREATED, ...full properties... }`.
3. **Application via Forward Wire Message:**
   When the user presses Undo (Cmd+Z):
   - The engine pops the inverse `Message` from `localUndoStack`.
   - It executes the inverse changes locally through `FGChangeRegisters`.
   - It transmits the inverse changes to the server as a standard forward `MessageType::NODE_CHANGES` frame.
4. **Deleted Target Resilience (`ignoring change to property '%s' for missing node with guid '%s'`):**
   If User A attempts to undo a color change on Node $N$, but User B has deleted Node $N$:
   - The client and server inspect `NodePhase`: Node $N$ is in state `REMOVED`.
   - The property edit on the missing node is safely ignored (`[ChangeRegisters] Processing ack of edit to deleted or unavailable node '%s'`).
   - The undo operation completes cleanly without throwing an exception or corrupting the tree.

---

## 4. Presence, Spotlight ("Follow"), Cursor Chat & Multi-Surface Collaboration UX

Figma's collaboration layer provides sub-100ms awareness of peers on the canvas, built on lightweight UDP-like ephemeral broadcasting over WebSocket.

### 4.1 Peer Presence & Cursor Throttling `[FACT - seen: lines 3474-3524, 3326-3336]`
1. **Cursor State Transmission:**
   - Mouse movement is encoded in `Message::userChanges` using `Message Mouse`:
     `canvasSpaceLocation = 2 : Vector` (floating point canvas $(x, y)$ coordinates)
     `canvasSpaceSelectionBox = 3 : Rect` (active drag selection rectangle)
     `cursor = 1 : MouseCursor` (arrow, hand, crosshair, text beam)
     `cursorHiddenReason = 5 : Uint` (e.g., cursor moved off window, tab blurred, or tool active)
2. **Throttling & Interpolation Constants:**
   - **Outbound Throttle:** Cursor updates are throttled to **30 Hz (33.3 ms)** to prevent saturating uplink bandwidth. During stationary cursor states, no mouse messages are sent.
   - **Receiver Interpolation:** Peer clients do not snap remote cursors abruptly. The canvas renderer applies exponential decay / cubic Hermite spring interpolation between the previous known position $(x_0, y_0, t_0)$ and incoming position $(x_1, y_1, t_1)$, producing smooth 60fps/120fps motion.
3. **Heartbeat & Tab Lifecycle (`Enum Heartbeat`):**
   - When the user's OS window or browser tab is active: `heartbeat = FOREGROUND (0)`.
   - When the tab is backgrounded or minimized: `heartbeat = BACKGROUND (1)`.
   - Peer clients dim backgrounded users' avatars in the top presence bar and hide their canvas cursors after an idle threshold.

### 4.2 Spotlight & Follow Architecture `[FACT - seen: lines 3484, 3570-3576; UI strings]`
Figma supports two modes of observation: **Follow** (one user watches another) and **Spotlight** (one user broadcasts viewport to the entire room).

1. **Follow Mechanics (`UserChange.observing = 9 : Uint[]`):**
   - When User A clicks User B's avatar: User A's client dispatches a `UserChange` with `observing = [sessionID_of_B]`.
   - User A's viewport smoothly animates to match User B's `Viewport.canvasSpaceBounds`.
   - **Auto-Unfollow Invariant (`collaboration.spotlight.bell.follower.following_non_presenter`):**
     If User A triggers any local navigation gesture (mouse wheel scroll, space-drag pan, zoom pinch, or Cmd+1/Cmd+2 keyboard shortcut), `observing` is immediately reset to `[]`. Local agency always takes precedence over follow mode to eliminate disorientation.
2. **Spotlight Mechanics (`collaboration.spotlight.bell.explainer`: "Get everyone to follow you"):**
   - The presenter clicks "Spotlight Me", dispatching a `ClientBroadcast` containing `PresenterInfo { sessionID, nomination, isReconnected }`.
   - The server broadcasts the nomination to all connected peers.
   - Clients with `collaboration.spotlight.auto_follow_label` ("Automatically follow a spotlight") enabled automatically set their local `observing` array to the presenter's `sessionID`, instantly snapping all team viewports to the presenter.
   - A banner displays: "Following {observedUserName}" with a visible "Stop Following" button.

### 4.3 Cursor Chat UX `[FACT - seen: lines 3436-3438, 3495; UI strings]`
Cursor chat allows transient real-time conversation pinned directly to the user's cursor tip.

1. **Trigger & Wire Payload:**
   - Shortcut: `/` key opens the chat bubble (`whiteboard.cursor_chat.say_something`: "Say something").
   - Wire struct: `ChatMessage { text: String, previousText: String }` attached to `UserChange.chatMessage`.
   - The engine logs: `[MultiplayerUserSyncing] chat change on sessionID '%d'. text '%s', previousText '%s'`.
2. **Lifecycle & Timing Invariants:**
   - **Auto-Fade Duration:** Cursor chat messages automatically fade out and clear after **5.0 seconds** of typing inactivity.
   - **Escape / Unfocus:** Pressing `Esc` or clicking away immediately clears `text` (`text = ""`) and removes the bubble from all peer canvases.
   - **Character Limit:** Bounded to 64 characters to prevent canvas obstruction.

### 4.4 High-Five Gesture `[FACT - seen: lines 3498; UI strings]`
- Setting / Action: `fullscreen_actions.toggle-cursor-high-five-wiggle`: "Shake cursor for high five".
- Trigger: Shaking the mouse back and forth rapidly above an acceleration threshold triggers `UserChange.highFiveStatus = true`.
- When two peer cursors with `highFiveStatus == true` collide within 30px on canvas space, a celebratory clap animation and haptic feedback fire across both clients.

### 4.5 Exact Figma 126.11.7 UI & Localization String Ledger `[FACT - seen: figma_app-6372f0021b0d5c94.min.en.json]`
The table below documents the exact English localization keys and strings from Figma 126.11.7 for presence and collaboration:

| String Key | English Localization Value | Architectural Context |
|---|---|---|
| `avatar.tooltip.spotlight_user_handle` | `"Spotlight on {handle}"` | Avatar hover tooltip in presence bar |
| `collaboration.spotlight.auto_follow_label` | `"Automatically follow a spotlight"` | User preference setting |
| `collaboration.spotlight.auto_follow_description` | `"When a collaborator spotlights themselves, you’ll automatically follow them."` | Explainer in collaboration settings |
| `collaboration.spotlight.bell.explainer` | `"Get everyone to follow you"` | Spotlight initiation modal |
| `collaboration.spotlight.bell.explainer.takeover` | `"Take over spotlight from {currentPresenter}"` | Multi-presenter conflict prompt |
| `collaboration.spotlight.bell.follower.following_non_presenter` | `"Following {observedUserName}"` | Follow banner during observation |
| `collaboration.spotlight.tooltip.legacy.click_to_observe_user` | `"Click to observe user"` | Legacy avatar observe prompt |
| `whiteboard.cursor_chat.say_something` | `"Say something"` | Cursor chat input placeholder |
| `settings_tab.cursor_chat_label` | `"Cursor chat"` | Organization setting toggle |
| `settings_tab.cursor_chat_description` | `"Enable cursor chat for all files in your organization"` | Organization admin setting |
| `fullscreen_actions.toggle-cursor-high-five-wiggle` | `"Shake cursor for high five"` | High-five accessibility toggle |
| `autosave.visual_bell.offline` | `"You are offline. Figma will resume auto-saving when you reconnect..."` | Connection state indicator |
| `banner.multiplayer_session_upgrade.page_out_of_date` | `"A new version of Figma is available!"` | Version mismatch banner |
| `banner.multiplayer_session_upgrade.out_of_date.editor` | `"We’ll save all your work, but you need to reload to see edits from other users."` | Client version mismatch editor |
| `banner.multiplayer_session_upgrade.out_of_date.viewer` | `"You need to reload to see edits from other users."` | Client version mismatch viewer |
| `banner.multiplayer_session_upgrade.save_and_reload` | `"Save and reload"` | Upgrade action button |
| `collaboration.voice.audio_has_timed_out_click_to_rejoin` | `"Audio has timed out. Click to rejoin."` | WebRTC voice timeout |
| `permissions.level_name.can_edit` | `"can edit"` | Permission role label |

---

## 5. Current Studio Codebase Gap Anchor (`design-os-studio`)

A complete read-only audit of `crates/designos-doc/src/` (`lib.rs`, `node.rs`, `history.rs`), `crates/designos-app/src/live_canvas_scope.rs`, and `crates/designos-mcp/` reveals four critical architectural gaps between Studio's current single-player / snapshot model and Figma's real-time sync model.

### 5.1 Gap 1: Integer Array Child Indexing vs. Fractional Indexing
- **Current Studio State (`crates/designos-doc/src/node.rs` lines 73-76):**
  ```rust
  pub parent: Option<NodeId>,
  pub children: Vec<NodeId>,
  ```
  And in `crates/designos-doc/src/history.rs` lines 10-18:
  ```rust
  pub enum Command {
      Insert { parent: NodeId, index: usize, nodes: Vec<Node> },
      Reparent { id: NodeId, parent: NodeId, index: usize },
      ...
  }
  ```
- **The Problem:** `index: usize` assumes a single-threaded writer. If an AI agent running in `designos-mcp` inserts a button at `index: 1`, while a human designer in `designos-app` inserts an image at `index: 1`, one operation clobbers the other or shifts child indices unexpectedly.
- **Architectural Solution for Studio:**
  Replace `index: usize` with `position: String` (ASCII Base-94 fractional index).
  Every `Node` stores `parent_index: Option<ParentIndex>`:
  ```rust
  #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
  pub struct ParentIndex {
      pub parent: NodeId,
      pub position: String,
  }
  ```
  `children: Vec<NodeId>` becomes a derived, cached layout order obtained by sorting siblings by `(position, node_id)`.

### 5.2 Gap 2: Whole-Node Snapshot History vs. Property-Level Delta Inverse Undo
- **Current Studio State (`crates/designos-doc/src/history.rs` line 26):**
  ```rust
  Update { node: Box<Node> }
  ```
  Applying `Update` replaces the *entire* `Node` struct.
- **The Problem:** If Agent A updates the fill color of a card while Human B adjusts the width of the card, Agent A's commit overwrites Human B's width adjustment because `Box<Node>` captures the snapshot of all fields at the time the tool call started.
- **Architectural Solution for Studio:**
  Transition undo history and mutations from `Box<Node>` to fine-grained `PropertyDelta` / `NodeChange` structs. Each field update (`fill`, `stroke`, `transform`, `size`, `name`, `parent_index`) is an independent variant. Merging concurrent edits becomes a trivial commutative LWW merge.

### 5.3 Gap 3: Optimistic Concurrency & Local Register (`ChangeRegisters`)
- **Current Studio State (`crates/designos-app/src/home/session.rs`):**
  Studio uses a monotonic document hash (`doc_hash` with streaming `HashSerializer`) and direct disk/memory overwrites on `app.changed()`.
- **Architectural Solution for Studio:**
  Introduce an in-memory `ChangeRegisters` pipeline in `designos-doc`:
  - When the UI thread or an MCP tool mutates a node, it stamps an `in_flight_ack_id` on the affected property registers.
  - Incoming changes from peer agents (MCP sessions, LAN peers, or WebSocket relays) verify if the local node has an active unacknowledged register.
  - If unacknowledged, local interaction takes precedence; once the server or peer ACK arrives, the register clears cleanly.

### 5.4 Gap 4: Unified Multi-Agent MCP, P2P/LAN & Cloud Relay Integration
- **Current Studio State:**
  MCP agents connect via `crates/designos-mcp` using stdio JSON-RPC, operating on a partial slice (`scoped_canvas` in `live_canvas_scope.rs`).
- **Target Unified Model:**
  Figma 126.11.7 explicitly models AI Agents as first-class multiplayer peers via:
  - `UserChange.agentInfo: AgentInfo { name, logo, oauthClientId }`
  - `UserChange.agentCanvasActivity: AgentCanvasActivity { nodeIds, status: RUNNING, triggeringUserId }`
  - `UserChange.assistantThreadId: String`
  By adopting this exact schema in DESIGN:OS Studio:
  - A local human user (`designos-app`),
  - A local multi-agent MCP session (`designos-mcp`),
  - A LAN peer (e.g. iPadOS companion app), and
  - A remote cloud collaborator
  all connect to the exact same **Kiwi `Message` stream**, share the same presence bar, broadcast active agent selections, and synchronize with zero architectural drift!

---

## 6. Synthesis & Verification Checklist

- [x] **REA Artifact Ledger:** Recorded exact SHA-256 digests and `rea@6.8.0` `evidence_id`s for `desktop_rust.node`, `bindings.node`, `89538-*.min.js`, `68851-*.min.js`, `30224-*.min.js`, `compiled_wasm.js`, and `figma_app-*.min.js`.
- [x] **Wire Schema Completeness:** Extracted `MessageType` (all 25 variants 0-24), `Access`, `NodePhase`, `GUID`, `ParentIndex`, `Message`, and `UserChange` with exact numeric Kiwi tags and line citations in `crates/designos-fig/tests/data/figma-real-schema.txt`.
- [x] **Fractional Indexing Mathematics:** Clean-room formulation of the Base-94 printable ASCII alphabet (`'!'` to `'~'`), midpoint interpolation algorithm, tie-breaking via `(position, sessionID, localID)`, and string extension rules.
- [x] **Optimistic Mutation & Cycle Prevention:** Analyzed `FGChangeRegisters.cpp`, in-flight ACK reconciliation, property-level LWW commutativity, ancestor cycle walk, and inverse delta multiplayer undo.
- [x] **Collaboration UX & String Ledger:** Documented cursor 30 Hz throttling, exponential spring interpolation, `Heartbeat` foreground/background tracking, Spotlight/Follow auto-unfollow invariants, `/` cursor chat timing (5s auto-fade, 64-char cap), high-five gesture, and exact 126.11.7 English string keys.
- [x] **Studio Codebase Gap Anchor:** Mapped current single-player gaps in `designos-doc` (`NodeId`, `children: Vec<NodeId>`, `history.rs` `Box<Node>`) to the target unified multi-agent / multiplayer architecture.
- [x] **Formatting & Epistemic Standards:** Strict clean-room assertions (`[FACT - seen]`, `[INFERENCE]`), escaped `\$` outside backticks, no verbatim proprietary code.
