//! An edit that lands while earlier edits are still being drawn: tiles render synchronously from the
//! document passed to each `render_tiles` call, so a tile left stale by a short budget must end up showing
//! the LATEST document, never an intermediate one (#928: a canvas that kept the old picture after an agent
//! deleted a layer and after Revert, on a machine slow enough to draw one tile per frame).

mod common;

use common::*;
use designos_doc::*;
use designos_render::{TileBudget, TileCache, TileFrame, Viewport, TILE_SIZE};

const SIDE: u32 = 3 * TILE_SIZE;
const VP: Viewport = Viewport { offset: Vec2 { x: 0.0, y: 0.0 }, zoom: 1.0, dpr: 1.0 };

struct Card {
    t: T,
    card: NodeId,
    rows: Vec<NodeId>,
}

/// An auto-layout card (Hug height, drop shadow that crosses tile borders) of four rows, like a login form.
fn card() -> Card {
    let mut t = T::new();
    let card = t.add(None, |id| {
        let mut n = Node::frame(id, Rect::new(180.0, 130.0, 300.0, 10.0));
        n.fills = vec![Paint::solid(Color::WHITE)];
        n.corner_radii = [12.0; 4];
        n.effects = vec![Effect::DropShadow {
            color: Color { r: 0.0, g: 0.0, b: 0.0, a: 0.4 },
            offset: Vec2::new(0.0, 6.0),
            radius: 30.0,
            spread: 0.0,
            visible: true,
            show_behind: false,
            blend: Default::default(),
        }];
        n.kind.frame_props_mut().unwrap().auto_layout = Some(AutoLayout {
            direction: LayoutDirection::Vertical,
            item_spacing: 14.0,
            padding: [20.0; 4],
            primary_sizing: Sizing::Hug,
            counter_sizing: Sizing::Fixed,
            ..AutoLayout::v1()
        });
        n
    });
    let colours = [Color::rgb(220, 30, 30), Color::rgb(30, 160, 60), Color::rgb(30, 60, 220), Color::rgb(240, 190, 20)];
    let rows = colours.iter().map(|c| t.rect(Some(card), Rect::new(0.0, 0.0, 260.0, 70.0), *c)).collect();
    t.doc.relayout_subtree(card);
    Card { t, card, rows }
}

/// Renders everything from scratch for the document as it is now.
fn fresh(t: &T) -> TileFrame {
    let mut cache = TileCache::new();
    cache.prefetch_ring = 0;
    cache.render_tiles(&t.doc, t.page, &VP, (SIDE, SIDE), 1, TileBudget::unlimited())
}

fn assert_shows_document(cache: &mut TileCache, t: &T, rev: u64, what: &str) {
    // The cache hands out what it holds when no tile may render.
    let held = cache.render_tiles(&t.doc, t.page, &VP, (SIDE, SIDE), rev, TileBudget::tiles(0));
    assert_eq!(held.pending(), 0, "{what}: {} stale and {} missing tiles are left", held.stale, held.missing);
    let want = fresh(t);
    for w in &want.tiles {
        let s = held.tiles.iter().find(|s| s.key == w.key).unwrap_or_else(|| panic!("{what}: tile {:?} missing", w.key));
        assert!(s.pixmap.data() == w.pixmap.data(), "{what}: tile {:?} shows an older document", w.key);
    }
}

/// Draws with `budget` until nothing is pending, as many frames as it takes.
fn converge(cache: &mut TileCache, t: &T, rev: u64, budget: TileBudget) -> usize {
    for frames in 1..=64 {
        let f = cache.render_tiles(&t.doc, t.page, &VP, (SIDE, SIDE), rev, budget);
        if f.pending() == 0 {
            return frames;
        }
    }
    panic!("the tiles never converged");
}

#[test]
fn a_removed_row_and_the_reflow_below_it_are_redrawn_one_tile_at_a_time() {
    let mut c = card();
    let mut cache = TileCache::new();
    cache.prefetch_ring = 0;
    converge(&mut cache, &c.t, 1, TileBudget::tiles(1));
    assert_shows_document(&mut cache, &c.t, 1, "first picture");

    c.t.doc.remove(c.rows[1]).unwrap();
    c.t.doc.relayout_subtree(c.card);
    let one = cache.render_tiles(&c.t.doc, c.t.page, &VP, (SIDE, SIDE), 2, TileBudget::tiles(1));
    assert!(one.stale > 0, "the delete marks the tiles under the card stale");
    let frames = converge(&mut cache, &c.t, 2, TileBudget::tiles(1));
    assert!(frames >= 1);
    assert_shows_document(&mut cache, &c.t, 2, "after the delete");
}

/// The delete, then Revert (the same layer back at the same place), each arriving before the previous
/// picture finished drawing.
#[test]
fn edits_that_arrive_before_the_last_picture_is_finished_end_on_the_latest_document() {
    let mut c = card();
    let mut cache = TileCache::new();
    cache.prefetch_ring = 0;
    converge(&mut cache, &c.t, 1, TileBudget::tiles(1));

    // agent deletes a row; only one tile is drawn before the next edit
    let removed = c.t.doc.get(c.rows[1]).unwrap().clone();
    c.t.doc.remove(c.rows[1]).unwrap();
    c.t.doc.relayout_subtree(c.card);
    let f = cache.render_tiles(&c.t.doc, c.t.page, &VP, (SIDE, SIDE), 2, TileBudget::tiles(1));
    assert!(f.pending() > 0, "the picture is only partly drawn");

    // Revert puts it back before the delete finished drawing
    c.t.doc.insert(c.card, removed).unwrap();
    c.t.doc.reparent(c.rows[1], c.card, 1).unwrap();
    c.t.doc.relayout_subtree(c.card);
    let f = cache.render_tiles(&c.t.doc, c.t.page, &VP, (SIDE, SIDE), 3, TileBudget::tiles(1));
    assert!(f.pending() > 0, "the picture is still only partly drawn");

    // and another delete, of a different row, while that is still drawing
    c.t.doc.remove(c.rows[2]).unwrap();
    c.t.doc.relayout_subtree(c.card);
    cache.render_tiles(&c.t.doc, c.t.page, &VP, (SIDE, SIDE), 4, TileBudget::tiles(1));
    converge(&mut cache, &c.t, 4, TileBudget::tiles(1));
    assert_shows_document(&mut cache, &c.t, 4, "after delete, Revert and a second delete");
}
