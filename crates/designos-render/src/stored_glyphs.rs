//! Outlines of the glyphs Figma stored with a text, built when the text is drawn.
//!
//! A stored text keeps a table of 24 bytes per glyph and the command blobs of the file (see
//! [`designos_doc::FigTextGeometry`]); nothing is decoded when a file opens. Drawing assembles the text's path from the
//! table and the unit path of each blob it names. The unit paths live in one cache for the whole process, bounded in
//! bytes (see [`UNIT_CACHE_BYTES`]). There is no cache of assembled texts: assembly is a transform and append of each
//! glyph's commands, and the GPU keeps the mesh built from it.

use designos_doc as d;
use designos_doc::{PathCmd, Vec2};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};

/// What the unit paths of every thread together may keep, in bytes: the decoded commands, the blob each entry holds on
/// to, and a fixed charge per entry. Two generations of half this size each are kept, so the cache never holds more
/// than this and an entry larger than half of it is not kept at all.
pub const UNIT_CACHE_BYTES: usize = 32 << 20;
/// What an entry costs besides its commands and its blob: the map slot and the two `Arc` headers.
const ENTRY_OVERHEAD: usize = 96;

/// How many blobs have been decoded into unit paths in this process, cache misses only. For tests and diagnostics.
static DECODES: AtomicUsize = AtomicUsize::new(0);

#[doc(hidden)]
pub fn stored_glyph_blob_decodes() -> usize {
    DECODES.load(Ordering::Relaxed)
}

/// The decoded commands of one glyph blob, in font-em space.
type Unit = Arc<[PathCmd]>;

struct Entry {
    /// Held so the key (the blob's address) cannot be reused by another blob while the entry lives.
    _blob: Arc<[u8]>,
    unit: Unit,
    bytes: usize,
}

/// Unit paths by the address of the blob they were decoded from, in a young and an old generation.
struct UnitCache {
    /// Bytes each generation may hold: half the bound.
    generation_bytes: usize,
    young: HashMap<usize, Entry>,
    old: HashMap<usize, Entry>,
    young_bytes: usize,
    old_bytes: usize,
}

fn key(blob: &Arc<[u8]>) -> usize {
    blob.as_ptr() as usize
}

impl UnitCache {
    fn with_bound(bound: usize) -> Self {
        Self { generation_bytes: bound / 2, young: HashMap::new(), old: HashMap::new(), young_bytes: 0, old_bytes: 0 }
    }

    /// Bytes held now; never more than the bound.
    fn bytes(&self) -> usize {
        self.young_bytes + self.old_bytes
    }

    fn get(&mut self, blob: &Arc<[u8]>) -> Option<Unit> {
        let k = key(blob);
        if let Some(entry) = self.young.get(&k) {
            return Some(Arc::clone(&entry.unit));
        }
        let entry = self.old.remove(&k)?;
        self.old_bytes -= entry.bytes;
        let unit = Arc::clone(&entry.unit);
        self.put(k, entry);
        Some(unit)
    }

    fn insert(&mut self, blob: &Arc<[u8]>, unit: &Unit) {
        let bytes = unit.len() * std::mem::size_of::<PathCmd>() + blob.len() + ENTRY_OVERHEAD;
        self.put(key(blob), Entry { _blob: Arc::clone(blob), unit: Arc::clone(unit), bytes });
    }

    fn put(&mut self, k: usize, entry: Entry) {
        if entry.bytes > self.generation_bytes {
            return;
        }
        if self.young_bytes + entry.bytes > self.generation_bytes {
            self.old = std::mem::take(&mut self.young);
            self.old_bytes = self.young_bytes;
            self.young_bytes = 0;
        }
        self.young_bytes += entry.bytes;
        if let Some(replaced) = self.young.insert(k, entry) {
            self.young_bytes -= replaced.bytes;
        }
    }
}

static UNITS: LazyLock<Mutex<UnitCache>> = LazyLock::new(|| Mutex::new(UnitCache::with_bound(UNIT_CACHE_BYTES)));

fn units() -> MutexGuard<'static, UnitCache> {
    UNITS.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Bytes the unit-path cache holds now. For tests and diagnostics.
#[doc(hidden)]
pub fn stored_glyph_cache_bytes() -> usize {
    units().bytes()
}

/// The unit path of every blob slot of `blobs` that some glyph draws, in one lock for the lookups and one for the
/// inserts. A slot no glyph names, or one outside `blobs`, is `None`.
fn unit_paths(saved: &d::FigTextGeometry) -> Vec<Option<Unit>> {
    let mut used = vec![false; saved.blobs.len()];
    for slot in saved.glyphs.iter().filter_map(d::StoredGlyph::blob_slot) {
        if let Some(u) = used.get_mut(slot as usize) {
            *u = true;
        }
    }
    let mut out: Vec<Option<Unit>> = vec![None; saved.blobs.len()];
    {
        let mut cache = units();
        for (slot, blob) in saved.blobs.iter().enumerate() {
            if used[slot] {
                out[slot] = cache.get(blob);
            }
        }
    }
    let mut decoded = Vec::new();
    for (slot, blob) in saved.blobs.iter().enumerate() {
        if used[slot] && out[slot].is_none() {
            DECODES.fetch_add(1, Ordering::Relaxed);
            let unit: Unit = d::decode_glyph_commands(blob).into();
            out[slot] = Some(Arc::clone(&unit));
            decoded.push((blob, unit));
        }
    }
    if !decoded.is_empty() {
        let mut cache = units();
        for (blob, unit) in decoded {
            cache.insert(blob, &unit);
        }
    }
    out
}

/// A unit point placed for a glyph at (`x`, `y`) and `scale`: the same arithmetic the importer used to run once per file.
fn placed(cmd: PathCmd, x: f32, y: f32, scale: f32) -> PathCmd {
    let at = |p: Vec2| Vec2::new(x + p.x * scale, y - p.y * scale);
    match cmd {
        PathCmd::MoveTo(p) => PathCmd::MoveTo(at(p)),
        PathCmd::LineTo(p) => PathCmd::LineTo(at(p)),
        PathCmd::QuadTo(a, b) => PathCmd::QuadTo(at(a), at(b)),
        PathCmd::CubicTo(a, b, c) => PathCmd::CubicTo(at(a), at(b), at(c)),
        PathCmd::Close => PathCmd::Close,
    }
}

/// Whether every point of `cmd` is finite.
fn is_finite(cmd: &PathCmd) -> bool {
    let ok = |p: &Vec2| p.x.is_finite() && p.y.is_finite();
    match cmd {
        PathCmd::MoveTo(p) | PathCmd::LineTo(p) => ok(p),
        PathCmd::QuadTo(a, b) => ok(a) && ok(b),
        PathCmd::CubicTo(a, b, c) => ok(a) && ok(b) && ok(c),
        PathCmd::Close => true,
    }
}

/// The commands of a bar Figma stored, as a closed rectangle in the direction the shaped text draws its own bars.
fn bar(rect: &d::StoredDecoration, out: &mut Vec<PathCmd>) {
    let (x, y, right, bottom) = (rect.x, rect.y, rect.x + rect.w, rect.y + rect.h);
    out.extend([
        PathCmd::MoveTo(Vec2::new(x, y)),
        PathCmd::LineTo(Vec2::new(right, y)),
        PathCmd::LineTo(Vec2::new(right, bottom)),
        PathCmd::LineTo(Vec2::new(x, bottom)),
        PathCmd::Close,
    ]);
}

/// Commands one stored bar adds to its style's path.
const BAR_COMMANDS: usize = 5;

/// The outline of `saved` for each of its `styles` (the text's own style, then one per run) in node-local space: the
/// glyphs of a style in Figma's order, then the bars Figma stored for it. A geometry from a document file written
/// before the glyph table existed has no glyphs and already holds its one path, which is the text's own style.
///
/// `None` when the outline cannot be assembled within bounds: a glyph or bar names a style past `styles`, more than
/// [`d::MAX_ASSEMBLED_COMMANDS`] commands (counted before anything is allocated), or a point that is not finite once the
/// glyphs are placed. The caller shapes the text. Callers have already asked [`d::FigTextGeometry::draw_refusal`], which
/// bounds the glyphs and the blob bytes.
pub(crate) fn stored_paths(saved: &d::FigTextGeometry, styles: usize) -> Option<Vec<Cow<'_, d::Path>>> {
    if saved.glyphs.is_empty() {
        return Some(vec![Cow::Borrowed(&saved.path)]);
    }
    if saved.glyphs.len() > d::MAX_DRAWN_GLYPHS {
        return None;
    }
    let units = unit_paths(saved);
    let slot_unit = |g: &d::StoredGlyph| g.blob_slot().and_then(|slot| units.get(slot as usize)).and_then(Option::as_ref);
    let mut counts = vec![0usize; styles];
    for glyph in &saved.glyphs {
        if let Some(unit) = slot_unit(glyph) {
            let count = counts.get_mut(usize::from(glyph.style))?;
            *count = count.checked_add(unit.len())?;
        }
    }
    for bar in saved.decorations.iter() {
        let count = counts.get_mut(usize::from(bar.style))?;
        *count = count.checked_add(BAR_COMMANDS)?;
    }
    if counts.iter().try_fold(0usize, |sum, &n| sum.checked_add(n))? > d::MAX_ASSEMBLED_COMMANDS {
        return None;
    }
    let mut cmds: Vec<Vec<PathCmd>> = counts.into_iter().map(Vec::with_capacity).collect();
    for glyph in &saved.glyphs {
        if let Some(unit) = slot_unit(glyph) {
            cmds[usize::from(glyph.style)].extend(unit.iter().map(|&cmd| placed(cmd, glyph.x, glyph.y, glyph.font_size)));
        }
    }
    for rect in saved.decorations.iter() {
        bar(rect, &mut cmds[usize::from(rect.style)]);
    }
    if !cmds.iter().flatten().all(is_finite) {
        return None;
    }
    Some(cmds.into_iter().map(|cmds| Cow::Owned(d::Path { cmds, fill_rule: d::FillRule::NonZero })).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The one path of a text without runs.
    fn stored_path(saved: &d::FigTextGeometry) -> Option<Cow<'_, d::Path>> {
        stored_paths(saved, 1).map(|mut paths| paths.remove(0))
    }

    fn blob_of(len_cmds: usize) -> Arc<[u8]> {
        let mut b = Vec::new();
        for i in 0..len_cmds {
            b.push(2u8);
            b.extend((i as f32).to_le_bytes());
            b.extend(1.0f32.to_le_bytes());
        }
        b.into()
    }

    fn unit_of(blob: &Arc<[u8]>) -> Unit {
        d::decode_glyph_commands(blob).into()
    }

    #[test]
    fn the_cache_never_holds_more_than_its_bound() {
        let bound = 64 * 1024;
        let mut cache = UnitCache::with_bound(bound);
        // the blobs stay alive: an address reused by a later blob would turn a miss into a hit
        let blobs: Vec<Arc<[u8]>> = (0..2_000).map(|i| blob_of(8 + i % 40)).collect();
        for blob in &blobs {
            cache.insert(blob, &unit_of(blob));
            assert!(cache.bytes() <= bound, "{} bytes held past the {bound} byte bound", cache.bytes());
        }
        assert!(cache.bytes() > bound / 4, "the cache keeps what fits ({} bytes)", cache.bytes());
        let hits = blobs.iter().filter(|b| cache.get(b).is_some()).count();
        assert!(hits > 0 && hits < blobs.len(), "the newest entries are kept and the oldest are gone ({hits} of {})", blobs.len());
        assert!(cache.bytes() <= bound);
    }

    #[test]
    fn an_entry_larger_than_half_the_bound_is_not_kept() {
        let mut cache = UnitCache::with_bound(10_000);
        let big = blob_of(500);
        cache.insert(&big, &unit_of(&big));
        assert_eq!((cache.bytes(), cache.get(&big).is_none()), (0, true));
    }

    #[test]
    fn a_hit_in_the_old_generation_moves_the_entry_forward() {
        let mut cache = UnitCache::with_bound(20 * (ENTRY_OVERHEAD + 36 + 28));
        let first = blob_of(1);
        cache.insert(&first, &unit_of(&first));
        let filler: Vec<Arc<[u8]>> = (0..12).map(|_| blob_of(1)).collect();
        for blob in &filler {
            cache.insert(blob, &unit_of(blob));
        }
        assert!(cache.get(&first).is_some(), "still held, in the old generation");
        for blob in &filler {
            cache.insert(blob, &unit_of(blob));
        }
        assert!(cache.get(&first).is_some(), "a use keeps an entry across a rotation");
    }

    fn blob_of_commands(cmds: &[(u8, &[f32])]) -> Arc<[u8]> {
        cmds.iter().flat_map(|(op, pts)| std::iter::once(*op).chain(pts.iter().flat_map(|v| v.to_le_bytes()))).collect::<Vec<u8>>().into()
    }

    #[test]
    fn assembly_places_each_glyph_like_the_outline_the_importer_used_to_build_and_skips_what_it_cannot_draw() {
        let triangle = blob_of_commands(&[(1, &[0.0, 0.0]), (2, &[1.0, 0.0]), (2, &[0.5, 0.8]), (0, &[])]);
        let curve = blob_of_commands(&[(1, &[0.1, 0.0]), (3, &[0.5, 0.9, 0.9, 0.0]), (4, &[0.7, 0.3, 0.3, 0.3, 0.1, 0.0]), (0, &[])]);
        let glyph = |x: f32, y: f32, font_size: f32, blob: u32| d::StoredGlyph { x, y, font_size, blob, first_char: 0, style: 0, flags: 0 };
        let glyphs = vec![
            glyph(0.0, 20.0, 20.0, 0),
            glyph(9.5, 20.0, 14.0, 1),
            glyph(17.0, 20.0, 20.0, d::StoredGlyph::NO_BLOB),
            glyph(20.25, 18.5, 14.0, 0),
            glyph(30.0, 18.5, 14.0, 9), // a slot the geometry does not have: skipped, not a panic
        ];
        let saved = d::FigTextGeometry {
            text: d::TextProps::default(),
            size: Vec2::new(40.0, 24.0),
            path: d::Path::default(),
            blobs: vec![triangle, curve],
            glyph_blobs: Vec::new(),
            glyphs: glyphs.clone().into(),
            ..Default::default()
        };
        let mut expected = Vec::new();
        for g in glyphs.iter().filter(|g| g.blob < 2) {
            let at = |p: Vec2| Vec2::new(g.x + p.x * g.font_size, g.y - p.y * g.font_size);
            for cmd in d::decode_glyph_commands(&saved.blobs[g.blob as usize]) {
                expected.push(match cmd {
                    PathCmd::MoveTo(p) => PathCmd::MoveTo(at(p)),
                    PathCmd::LineTo(p) => PathCmd::LineTo(at(p)),
                    PathCmd::QuadTo(a, b) => PathCmd::QuadTo(at(a), at(b)),
                    PathCmd::CubicTo(a, b, c) => PathCmd::CubicTo(at(a), at(b), at(c)),
                    PathCmd::Close => PathCmd::Close,
                });
            }
        }
        let path = stored_path(&saved).expect("within bounds");
        assert_eq!(path.cmds, expected);
        assert_eq!(path.cmds.len(), 4 + 4 + 4);
        assert!(matches!(path, Cow::Owned(_)));
        // a geometry from before the table is its own path
        let legacy = d::FigTextGeometry { path: d::Path { cmds: expected.clone(), ..Default::default() }, glyphs: Default::default(), ..saved };
        assert!(matches!(stored_path(&legacy), Some(Cow::Borrowed(p)) if p.cmds == expected));
    }

    #[test]
    fn a_text_whose_outline_would_pass_the_command_bound_is_refused_before_it_is_allocated() {
        // 3,000 close commands in one blob, named by 4,096 glyphs: 12.3 million commands, counted and not built
        let blob: Arc<[u8]> = vec![0u8; 3000].into();
        let saved = d::FigTextGeometry {
            text: d::TextProps::default(),
            size: Vec2::new(10.0, 10.0),
            path: d::Path::default(),
            blobs: vec![blob],
            glyph_blobs: Vec::new(),
            glyphs: (0..4096)
                .map(|i| d::StoredGlyph { x: i as f32, y: 0.0, font_size: 10.0, blob: 0, first_char: 0, style: 0, flags: 0 })
                .collect::<Vec<_>>()
                .into(),
            ..Default::default()
        };
        assert!(stored_path(&saved).is_none());
        // and a table over the glyph limit is refused without a lookup
        let many = d::FigTextGeometry {
            glyphs: (0..4097)
                .map(|_| d::StoredGlyph { x: 0.0, y: 0.0, font_size: 10.0, blob: 0, first_char: 0, style: 0, flags: 0 })
                .collect::<Vec<_>>()
                .into(),
            ..saved
        };
        assert!(stored_path(&many).is_none());
    }

    #[test]
    fn a_placed_point_that_is_not_finite_refuses_the_whole_outline() {
        let blob = blob_of_commands(&[(1, &[0.0, 0.0]), (2, &[1000.0, 1000.0])]);
        let saved = d::FigTextGeometry {
            text: d::TextProps::default(),
            size: Vec2::new(10.0, 10.0),
            path: d::Path::default(),
            blobs: vec![blob],
            glyph_blobs: Vec::new(),
            glyphs: vec![d::StoredGlyph { x: 0.0, y: 0.0, font_size: 1.0e38, blob: 0, first_char: 0, style: 0, flags: 0 }].into(),
            ..Default::default()
        };
        assert!(stored_path(&saved).is_none());
    }

    #[test]
    fn the_process_wide_bound_is_32_mib() {
        assert_eq!(UNIT_CACHE_BYTES, 32 * 1024 * 1024);
        assert!(stored_glyph_cache_bytes() <= UNIT_CACHE_BYTES);
    }
}
