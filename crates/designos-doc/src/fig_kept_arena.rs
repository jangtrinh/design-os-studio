//! The bytes a `.fig` open keeps of Figma's records: one buffer, in memory or in a spill file that is memory-mapped.
//!
//! The buffer is the inflated message payload of the file. The records a store keeps are ranges into it, so nothing is
//! copied per record and a value is read where Figma wrote it. A payload that does not fit the RAM threshold is written to
//! a file once and mapped, so its pages are the operating system's to evict and its cost is disk, not memory.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// The kept payload of one open file.
pub struct KeptArena {
    backing: Backing,
}

enum Backing {
    Memory(Vec<u8>),
    File(MappedFile),
}

/// A spill file and its mapping. The file is private to this process: it is created new, written once, and never
/// written again while the mapping lives, which is what makes reading the map sound.
struct MappedFile {
    map: Option<memmap2::Mmap>,
    /// The path to delete when the mapping is dropped. `None` where the file is already unlinked (a unix file stays
    /// readable through its mapping after it is removed, so nothing is left behind even if the process dies).
    remove_on_drop: Option<PathBuf>,
}

impl Drop for MappedFile {
    fn drop(&mut self) {
        self.map.take();
        if let Some(path) = self.remove_on_drop.take() {
            let _ = std::fs::remove_file(path);
        }
    }
}

static SPILL_FILES: AtomicU64 = AtomicU64::new(0);

impl KeptArena {
    /// Keeps `bytes` in memory.
    pub fn in_memory(bytes: Vec<u8>) -> Self {
        Self { backing: Backing::Memory(bytes) }
    }

    /// Writes `bytes` to a new file in `dir` (created when missing) and maps it. The caller drops its own copy after, so the
    /// memory it held is released; on an error it still has the bytes.
    pub fn on_disk(bytes: &[u8], dir: &Path) -> std::io::Result<Self> {
        if bytes.is_empty() {
            return Ok(Self::in_memory(Vec::new()));
        }
        std::fs::create_dir_all(dir)?;
        let n = SPILL_FILES.fetch_add(1, Ordering::Relaxed);
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
        let path = dir.join(format!("kept-{}-{n}-{stamp}.bin", std::process::id()));
        let mut file = OpenOptions::new().read(true).write(true).create_new(true).open(&path)?;
        let written = file.write_all(bytes).and_then(|()| file.flush());
        if let Err(e) = written {
            let _ = std::fs::remove_file(&path);
            return Err(e);
        }
        // SAFETY: the file was created by this call under a name no other process uses, is written in full above, and is
        // never written, truncated or resized afterwards, so the mapped bytes cannot change or shrink while the map lives.
        let map = match unsafe { memmap2::Mmap::map(&file) } {
            Ok(map) => map,
            Err(e) => {
                let _ = std::fs::remove_file(&path);
                return Err(e);
            }
        };
        let remove_on_drop = if cfg!(unix) {
            std::fs::remove_file(&path)?;
            None
        } else {
            Some(path)
        };
        Ok(Self { backing: Backing::File(MappedFile { map: Some(map), remove_on_drop }) })
    }

    pub fn bytes(&self) -> &[u8] {
        match &self.backing {
            Backing::Memory(v) => v,
            Backing::File(f) => f.map.as_deref().unwrap_or(&[]),
        }
    }

    pub fn len(&self) -> usize {
        self.bytes().len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes().is_empty()
    }

    /// Whether the bytes are a mapped file rather than memory.
    pub fn is_file_backed(&self) -> bool {
        matches!(self.backing, Backing::File(_))
    }

    /// Bytes held in memory by this arena: its length when it is a buffer, none when it is mapped.
    pub fn memory_bytes(&self) -> usize {
        match &self.backing {
            Backing::Memory(v) => v.capacity(),
            Backing::File(_) => 0,
        }
    }
}

impl std::fmt::Debug for KeptArena {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeptArena").field("len", &self.len()).field("file_backed", &self.is_file_backed()).finish()
    }
}

impl Default for KeptArena {
    fn default() -> Self {
        Self::in_memory(Vec::new())
    }
}
