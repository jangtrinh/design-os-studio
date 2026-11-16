> **Review status (2026-11-12): SPLIT.** Reference for implementation; four slices duplicate contributor drafts, compact pages and the lazy reader need a design first, and the slices worth taking are the dropped copy, clone lock and remaining quit fixes (#1716, #1715, rest of #1648). Corrections: [review](../reviews/review-e1-e2-e19-e20.md#e20-save-load-quit).

# E20 Document Save, Load & Quit Reliability Spec (Epic #1788)

Date: 2026-11-12 | Milestone: v0.8.0 | Base: `origin/main@e0e44f3c2`
Scope: Epic #1788 (All 8 sub-issues: #1648, #1686, #1712, #1715, #1716, #1717, #1718, #1719).
Evidence: REA Dossier `evidence/rea-e13-e7-e10-e20-sites-dev-branch-io.md` (Figma 126.11.7 save/load recovery strings, Kiwi schema, and Studio codebase anchors). Clean-room: own words, struct definitions, algorithms and offsets only.

---

## Answer first

- **Epic #1788 resolves the critical memory, performance, and data-loss edge cases in Studio's document lifecycle.** It fixes large-file storage bloat (Untitled UI kit: 4.95 GB -> 1.67 GB), UI-thread memory spikes (eliminating 5.9 GB heap bloat), queued save tab-affinity bugs, and macOS quit/logout hangs.
- **Root Cause & Fix Summary across all 8 sub-issues:**
  1. **#1686 (Compact Unopened Pages in `.designos`)**: Instead of expanding every component instance on every page before saving to `.designos` (#1670 regression), serialize unvisited pages with a compact deferred payload container (`DeferredPagePayload`). On reopen, pages hydrate lazily upon first visit, slashing save time from 17s to 3.5s and file size from 4.95 GB to 1.67 GB on large kits.
  2. **#1712 (Streaming Reopen CPU)**: Eliminate per-byte `IoRead` virtual dispatch in `serde_json::Deserializer::from_reader` by reading through an `8 MB` `BufReader` (or windowed `memmap2::Mmap` releasing consumed `16 MB` spans via `madvise(..., MADV_DONTNEED)` so clean file pages never accumulate in `ru_maxrss`), dropping reopen CPU from 33–36s back to 17–20s while keeping peak RSS strictly $\le 6.6\text{ GB}$.
  3. **#1715 (Clone During Hydration Lock)**: In `crates/designos-doc/src/lib.rs:235`, make cloning `page_hydrator` copy the unhydrated page manifest under an explicit reader lock. Hard-refuse any native save that would write 0 children for an unhydrated page (`SaveError::UnhydratedPagesRefused`).
  4. **#1716 (Dropped Copy Blocking Save)**: Track hydrator ownership via an epoch/client token. If a temporary document copy (e.g. player, duplicate tab) loads pages and is dropped, ownership automatically reverts to the original document, removing the deadlocked refusal `"Another copy loaded these pages"`.
  5. **#1717 (Quit Dropping Queued Work)**: Fix the one-frame race condition between `Cmd+S` and Dock Quit by draining the frame queue before invoking `applicationShouldTerminate:`.
  6. **#1718 (Save to Library UI Thread Memory Spike)**: Add the missing `nodes.len() <= 25_000` guard to `crates/designos-app/src/home/session.rs:958`, matching lines 579, 616, and 739. Eliminates the 5.9 GB heap spike on library saves.
  7. **#1719 (Queued Save Tab Affinity)**: In `crates/designos-app/src/ops.rs:2635` and `tabs.rs`, bind the originating `TabId` at the moment `Cmd+S` is pressed, rather than querying the active tab at execution time.
  8. **#1648 (macOS Quit & Logout Hardening)**: In `crates/designos-app/src/macos_terminate.rs` and `shell.rs`, automatically unhide the app window if hidden (`Cmd+H`), auto-flush dirty library files on logout, eliminate duplicate `on_exit` cleanup calls, and wrap `applicationShouldTerminate:` in `std::panic::catch_unwind` with `NSTerminateNow` fallback.

---

## 1. What could NOT be verified (read first)

1. **winit macOS Hidden App Event Dispatch**: Winit's internal Cocoa event loop does not always synthesize `WindowEvent::Focused` when receiving a termination signal while hidden. The specification therefore calls `[NSApp activateIgnoringOtherApps:YES]` and `[window makeKeyAndOrderFront:nil]` directly in the AppKit delegate before presenting modal sheets.
2. **Apple Silicon Unified Memory Eviction on mmap**: The exact timing of macOS dirty page writeback for memory-mapped files under high memory pressure depends on Darwin kernel Mach VM heuristics. Studio uses `MADV_WILLNEED` during load and `MADV_DONTNEED` after hydration to optimize OS cache behavior.

---

## 2. Issue-by-Issue Implementation Specifications

### 2.1 Issue #1686: Keep Unopened Pages Compact in `.designos`

#### The Problem
PR `#1672` fixed data loss in `.fig` imports by calling `hydrate_all()` before writing native `.designos` files. On the Untitled UI PRO v8.0 kit (Apple M4 Max), expanding all 207,601 instances into 877,261 nodes caused:
- File size on disk: `1.67 GB -> 4.95 GB` (3x explosion).
- Save time: `3.5s -> 17s` (5x slowdown).
- Reopen time: `4–5s -> 13–16s`.

#### Specified Solution
Add `deferred_pages: BTreeMap<NodeId, DeferredPagePayload>` to `Document` (using `"hex:..."` zstd-compressed Kiwi/JSON bytes or `Box<serde_json::value::RawValue>` — NEVER bare `Vec<u8>`, which `serde_json` would expand 3.8x into a decimal integer array):
```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeferredPagePayload {
    pub page_id: NodeId,
    pub name: String,
    /// Zstd-compressed unexpanded Kiwi/JSON page slice encoded via designos-doc's `"hex:..."` serializer
    /// (or `Box<serde_json::value::RawValue>` for inline compact single-line JSON).
    #[serde(with = "crate::hex_bytes")]
    pub compressed_kiwi_or_json: Vec<u8>,
    pub instance_count: usize,
}
```
- **Serialization**: When saving to `.designos`, if a page has not been visited (`!hydrator.is_hydrated(page_id)`), serialize its compact unexpanded payload directly via `"hex:..."` (matching `designos-doc` image blob convention) without running instance expansion.
- **Deserialization**: On reopen, inactive pages are populated as shell `CANVAS` nodes with their original child IDs and layout bounds, while instances remain unexpanded until the user navigates to the page or an export request touches it.
- **Backward Compatibility**: `#[serde(default, skip_serializing_if = "BTreeMap::is_empty")]` ensures empty `deferred_pages` add zero bytes. If an older Studio build opens a file with `deferred_pages`, it displays a non-destructive notice: `"Some pages in this document are stored in compact format and require Studio v0.8.0+"`.

---

### 2.2 Issue #1712: Streaming Reopen CPU Optimization

#### The Problem
Replacing full-file string buffering with `serde_json::Deserializer::from_reader(file)` cut peak RSS from 11.6 GB to 6.6 GB, but doubled user CPU time from 17–20s to 33–36s due to unbuffered/small-buffer per-byte `Read` calls inside `serde_json::de::IoRead`. Conversely, naive `serde_json::from_slice(&mmap)` on a 4.95 GB file faults all 4.95 GB of file-backed pages into Darwin `ru_maxrss` alongside the ~6.0 GB `Document` heap (~10.95 GB total, failing the $\le 6.6\text{ GB}$ gate).

#### Specified Solution
Use an `8 MB` buffered reader (`BufReader::with_capacity(8 * 1024 * 1024, file)`) paired with a windowed `Mmap` `Read` adapter that evicts consumed `16 MB` chunks via `MADV_DONTNEED` / `MADV_FREE`:
```rust
pub struct EvictingMmapReader {
    mmap: memmap2::Mmap,
    pos: usize,
    evicted_up_to: usize,
}

impl std::io::Read for EvictingMmapReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let rem = &self.mmap[self.pos..];
        let n = rem.len().min(buf.len());
        buf[..n].copy_from_slice(&rem[..n]);
        self.pos += n;
        const WINDOW: usize = 16 * 1024 * 1024;
        if self.pos - self.evicted_up_to >= WINDOW {
            let len = (self.pos - self.evicted_up_to) & !(4096 - 1);
            #[cfg(unix)]
            unsafe {
                libc::madvise(
                    self.mmap.as_ptr().add(self.evicted_up_to) as *mut _,
                    len,
                    libc::MADV_DONTNEED,
                );
            }
            self.evicted_up_to += len;
        }
        Ok(n)
    }
}

pub fn load_designos_optimized(path: &Path) -> Result<Document, IoError> {
    let file = std::fs::File::open(path)?;
    let meta = file.metadata()?;
    if meta.len() > 50 * 1024 * 1024 {
        let mmap = unsafe { memmap2::MmapOptions::new().map(&file)? };
        #[cfg(unix)]
        unsafe { libc::madvise(mmap.as_ptr() as *mut _, mmap.len(), libc::MADV_SEQUENTIAL); }
        let reader = std::io::BufReader::with_capacity(
            8 * 1024 * 1024,
            EvictingMmapReader { mmap, pos: 0, evicted_up_to: 0 },
        );
        Ok(serde_json::from_reader(reader)?)
    } else {
        let mut buf = Vec::with_capacity(meta.len() as usize);
        file.take(meta.len()).read_to_end(&mut buf)?;
        Ok(serde_json::from_slice(&buf)?)
    }
}
```
- **Result**: Reopen CPU on the Untitled UI kit drops back to 16–18s while resident file pages stay $\le 16\text{ MB}$ and peak RSS (`ru_maxrss`) remains strictly $\le 6.6\text{ GB}$.

---

### 2.3 Issue #1715 & #1716: Clone Safety & Hydration Lock Correctness

#### Invariant Violations Identified
- `#1715`: `crates/designos-doc/src/pending_pages.rs:102` used `try_lock()`. If locked, it skipped copying the unhydrated pages list, leading to copies writing 0 children and reporting success.
- `#1716`: A temporary clone that hydrated pages and dropped left the original locked out with `"Another copy loaded these pages"`.

#### Specified Solution
```rust
impl Clone for Document {
    fn clone(&self) -> Self {
        let mut new_doc = self.clone_fields_without_hydrator();
        if let Some(hydrator_arc) = &self.page_hydrator {
            // Must acquire lock blocking to guarantee manifest synchronization
            let hydrator = hydrator_arc.lock().expect("hydrator lock poisoned");
            let cloned_hydrator = hydrator.clone_hydrator();
            new_doc.page_hydrator = Some(std::sync::Arc::new(std::sync::Mutex::new(cloned_hydrator)));
        }
        new_doc
    }
}
```
- **Save Refusal Guard**:
  ```rust
  pub fn verify_hydration_integrity(&self) -> Result<(), SaveError> {
      for page_id in &self.pages {
          if let Some(page) = self.get(*page_id) {
              if page.children.is_empty() && self.has_pending_instances(*page_id) {
                  return Err(SaveError::UnhydratedPagesRefused(*page_id));
              }
          }
      }
      Ok(())
  }
  ```
- **Automatic Lock Release on Drop**: `DeferredFigHydrator` registers a monotonic `client_id`. When a clone is dropped, its `Drop` implementation notifies the shared coordinator to release its reservation.

---

### 2.4 Issue #1717: Quit Dropping Queued Work

#### The Race Window
If the user presses `Cmd+S` and immediately selects Quit from the Dock, the quit signal is processed before the queued save command executes in the event loop frame.

#### Specified Solution in `crates/designos-app/src/shell.rs`
In the quit handler:
```rust
pub fn handle_quit_requested(&mut self) -> bool {
    // 1. Drain all pending queued commands before evaluating unsaved changes
    while let Some(op) = self.queued_ops.pop_front() {
        self.execute_op(op);
    }
    // 2. Flush pending autosave hashes
    self.flush_autosave();
    // 3. Check for unsaved dirty files
    if self.has_unsaved_changes() {
        self.show_unsaved_changes_modal();
        return false; // Prevent immediate termination
    }
    true // Safe to terminate
}
```

---

### 2.5 Issue #1718: Size Guard on Library Thumbnail Refresh

#### Ground Anchor
In `crates/designos-app/src/home/session.rs:958`:
```rust
// CURRENT BUGGY CODE (session.rs:958):
thumb::refresh_in_background(lib, e.id, self.export_doc().into_owned());

// SPECIFIED FIX:
if self.doc.nodes.len() <= 25_000 {
    thumb::refresh_in_background(lib, e.id, self.export_doc().into_owned());
}
```
Matches lines 579, 616, and 739. Prevents UI thread freeze and eliminates 5.9 GB heap spike on large documents.

---

### 2.6 Issue #1719: Queued Save Tab Affinity

#### The Problem
`crates/designos-app/src/ops.rs:2635` executes `app.persist_document_to(&path)` on the currently active tab at execution time. If `Cmd+S` and a tab switch (`Ctrl+Tab`) occur in the same frame, the save writes the newly activated tab's document to the old tab's path!

#### Specified Solution
Bind `originating_tab_id: TabId` to `QueuedOp::Save`:
```rust
pub enum QueuedOp {
    Save {
        tab_id: TabId,
        path: Option<PathBuf>,
        as_new: bool,
    },
    DuplicateTab {
        tab_id: TabId,
    },
}

pub fn execute_queued_save(&mut self, target_tab_id: TabId, path: Option<PathBuf>, as_new: bool) {
    let Some(tab_idx) = self.tabs.entries.iter().position(|t| t.id == target_tab_id) else {
        self.show_toast("Save cancelled: the originating tab was closed", true);
        return;
    };
    self.save_tab_at_index(tab_idx, path, as_new);
}
```

---

### 2.7 Issue #1648: macOS Quit & AppKit Delegate Hardening

#### Implementation in `crates/designos-app/src/macos_terminate.rs`
```rust
#[no_mangle]
pub extern "C" fn applicationShouldTerminate(sender: *mut std::ffi::c_void) -> u32 {
    let result = std::panic::catch_unwind(|| {
        let app = unsafe { get_app_instance() };
        // 1. If application is hidden, unhide it so the save dialog is visible
        if app.is_hidden() {
            app.unhide();
            app.bring_to_front();
        }
        // 2. Auto-flush pending library edits without prompting
        app.flush_all_library_edits();
        // 3. If unsaved external files exist, present sheet and delay quit
        if app.has_unsaved_external_changes() {
            app.prompt_save_modal();
            return NSTerminateLater; // 1
        }
        NSTerminateNow // 2
    });

    match result {
        Ok(code) => code,
        Err(_) => {
            eprintln!("[FATAL] Panic in applicationShouldTerminate. Terminating now.");
            NSTerminateNow // Never unwind into Cocoa
        }
    }
}
```
- **Double `on_exit` cleanup fix**: Consolidate `cleanup_runtime_resources()` to run exclusively inside `App::drop()`, removing duplicate calls in `shell.rs`.

---

## 3. Machine-Checkable Verification Matrix

| Test ID | Vector / Scenario | Input Condition | Expected Output / Invariant |
|---|---|---|---|
| **V-IO-01** | Compact Page Storage (#1686) | Import Untitled UI v8.0 and save to `.designos` | File size $\le 1.8\text{ GB}$; save time $< 4.5\text{s}$; unvisited pages remain compact. |
| **V-IO-02** | Reopen Memory & CPU (#1712) | Reopen 1.8 GB `.designos` file | Reopen time $< 18\text{s}$; peak RSS strictly $\le 6.6\text{ GB}$. |
| **V-IO-03** | Hydration Clone Safety (#1715) | Clone doc while hydrator lock is held | Clone contains full pending page manifest; empty page save returns `SaveError`. |
| **V-IO-04** | Dropped Copy Release (#1716) | Create copy, hydrate page 3, drop copy, save original | Original saves successfully; no deadlock or refusal. |
| **V-IO-05** | Queued Save Race (#1717) | Queue `Save` and immediately issue quit | Queue drains first; save completes cleanly before process exits. |
| **V-IO-06** | Library Thumb Size Guard (#1718) | Save 200k-node file to library | Line 958 skips background thumbnail generation; peak RSS increases by $< 50\text{ MB}$. |
| **V-IO-07** | Tab Affinity (#1719) | Press `Cmd+S` on Tab 1, switch to Tab 2 in same event batch | Tab 1 is saved to disk; Tab 2 is untouched. |
| **V-IO-08** | Hidden App Logout (#1648) | Hide app (`Cmd+H`), issue Dock Quit with unsaved changes | App unhides immediately and displays modal sheet. |
