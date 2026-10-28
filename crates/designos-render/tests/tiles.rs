mod common;

use common::*;
use designos_doc::*;
use designos_render::tiny_skia::Pixmap;
use designos_render::{render_page, render_page_indexed, SceneIndex, TileBudget, TileCache, TileFrame, Viewport, TILE_SIZE};

const SIDE: u32 = 3 * TILE_SIZE;

/// Scene touching every feature that crosses tile borders: shadows, blurs,
/// background blur, clipping, rotation, opacity layers, masks and text.
fn scene() -> T {
    let mut t = T::new();
    let frame = t.add(None, |id| {
        let mut n = Node::frame(id, Rect::new(40.0, 30.0, 500.0, 380.0));
        n.kind.frame_props_mut().unwrap().clips_content = true;
        n
    });
    let r = t.rect(Some(frame), Rect::new(200.0, 120.0, 150.0, 90.0), RED);
    t.node(r).effects = vec![Effect::DropShadow {
        color: Color { r: 0.0, g: 0.0, b: 0.0, a: 0.5 },
        offset: Vec2::new(6.0, 10.0),
        radius: 24.0,
        spread: 2.0,
        visible: true,
        show_behind: false,
        blend: Default::default(),
    }];
    t.node(r).corner_radii = [12.0; 4];
    let b = t.rect(Some(frame), Rect::new(60.0, 200.0, 220.0, 140.0), BLUE);
    t.node(b).effects = vec![Effect::LayerBlur { radius: 10.0, visible: true }];
    let rot = t.add(None, |id| {
        let mut n = Node::frame(id, Rect::new(520.0, 300.0, 160.0, 90.0));
        n.fills = vec![Paint::solid(Color::rgb(0, 160, 80))];
        n
    });
    // A rotation-like matrix whose entries are exact binary fractions: float
    // rounding of edge coordinates is then identical in a tile and in a full
    // page. (With an irrational angle a few edge pixels differ by rounding.)
    t.node(rot).transform = Affine { a: 0.75, b: 0.5, c: -0.5, d: 0.75, tx: 560.0, ty: 420.0 };
    t.node(rot).opacity = 0.6;
    let inner = t.rect(Some(rot), Rect::new(20.0, 20.0, 60.0, 30.0), Color::rgb(250, 200, 0));
    t.node(inner).effects = vec![Effect::InnerShadow {
        color: Color { r: 0.0, g: 0.0, b: 0.0, a: 0.6 },
        offset: Vec2::new(2.0, 3.0),
        radius: 6.0,
        spread: 0.0,
        visible: true,
        blend: Default::default(),
    }];
    let glass = t.rect(None, Rect::new(180.0, 480.0, 300.0, 120.0), Color { r: 1.0, g: 1.0, b: 1.0, a: 0.2 });
    t.node(glass).effects = vec![Effect::BackgroundBlur { radius: 12.0, visible: true }];
    // Seeded effects crossing tile borders (before the mask below, which would clip them): grain, edge texture and refractive glass.
    let grain = t.rect(None, Rect::new(230.0, 20.0, 300.0, 70.0), Color::rgb(240, 240, 250));
    t.node(grain).effects = vec![Effect::Noise(NoiseEffect {
        mode: NoiseMode::Duo { color: Color::rgb(200, 0, 0), secondary: Color::BLACK },
        size: 1.5,
        density: 0.6,
        opacity: 0.7,
        blend: BlendMode::Normal,
        seed: 11,
        visible: true,
    })];
    // Corner arcs must not straddle a tile border: a curved edge rasterises with slightly
    // different AA in a tile than in a full page (true without effects too), and the
    // texture displacement would only make that visible.
    let rough = t.rect(None, Rect::new(540.0, 150.0, 200.0, 100.0), Color::rgb(30, 90, 200));
    t.node(rough).effects = vec![Effect::Texture(TextureEffect { size: 5.0, radius: 6.0, clip_to_shape: false, seed: 3, visible: true })];
    t.node(rough).corner_radii = [20.0; 4];
    let pane = t.rect(None, Rect::new(90.0, 560.0, 340.0, 150.0), Color { r: 1.0, g: 1.0, b: 1.0, a: 0.1 });
    t.node(pane).effects =
        vec![Effect::Glass(GlassEffect { light_angle: 315.0, light_intensity: 0.7, refraction: 0.6, depth: 14.0, dispersion: 0.5, frost: 5.0, visible: true })];
    t.node(pane).corner_radii = [28.0; 4];
    let m = t.rect(None, Rect::new(600.0, 40.0, 120.0, 120.0), Color::BLACK);
    t.node(m).is_mask = true;
    t.node(m).corner_radii = [60.0; 4];
    t.rect(None, Rect::new(580.0, 60.0, 200.0, 40.0), Color::rgb(200, 30, 120));
    let style = TextStyle { font_size: 22.0, fills: vec![Paint::solid(Color::rgb(0x10, 0x10, 0x10))], ..Default::default() };
    t.add(None, |id| Node::text(id, Rect::new(250.0, 440.0, 260.0, 30.0), "Tiled text crossing borders", style));
    t.add(Some(frame), |id| {
        let mut n = Node::rectangle(id, Rect::new(10.0, 300.0, 300.0, 60.0));
        n.fills = vec![Paint::solid(Color::rgb(30, 30, 30))];
        n.stroke.paints = vec![Paint::solid(Color::rgb(255, 255, 255))];
        n.stroke.weight = 4.0;
        n
    });
    t
}

/// The reference: a plain `render_page`. It is rendered with a margin and
/// cropped, because blurs near the edge of a pixmap see transparent pixels
/// there while a tile sees the real neighbouring content.
fn full(t: &T, vp: &Viewport) -> Pixmap {
    const M: u32 = 128;
    let s = vp.zoom * vp.dpr;
    let vp2 = Viewport { offset: Vec2::new(vp.offset.x - M as f32 / s, vp.offset.y - M as f32 / s), ..*vp };
    let mut big = Pixmap::new(SIDE + 2 * M, SIDE + 2 * M).unwrap();
    render_page(&t.doc, t.page, &vp2, &mut big);
    let mut out = Pixmap::new(SIDE, SIDE).unwrap();
    out.draw_pixmap(-(M as i32), -(M as i32), big.as_ref(), &Default::default(), Default::default(), None);
    out
}

fn stitch(frame: &TileFrame, vp: &Viewport) -> Pixmap {
    let mut out = Pixmap::new(SIDE, SIDE).unwrap();
    let s = vp.zoom * vp.dpr;
    let (ox, oy) = ((vp.offset.x * s).round() as i32, (vp.offset.y * s).round() as i32);
    for tile in &frame.tiles {
        let (x, y) = tile.key.device_origin();
        out.draw_pixmap(x - ox, y - oy, tile.pixmap.as_ref().as_ref(), &Default::default(), Default::default(), None);
    }
    out
}

/// (differing pixels, largest channel delta, bounding box x0,y0,x1,y1)
fn diff_stats(a: &Pixmap, b: &Pixmap) -> (usize, i32, (usize, usize, usize, usize)) {
    assert_eq!((a.width(), a.height()), (b.width(), b.height()));
    let w = a.width() as usize;
    let (mut n, mut max, mut bb) = (0, 0, (usize::MAX, usize::MAX, 0, 0));
    for (i, (p, q)) in a.data().chunks(4).zip(b.data().chunks(4)).enumerate() {
        let d = p.iter().zip(q).map(|(x, y)| (*x as i32 - *y as i32).abs()).max().unwrap();
        if d > 0 {
            n += 1;
            max = max.max(d);
            let (x, y) = (i % w, i / w);
            bb = (bb.0.min(x), bb.1.min(y), bb.2.max(x), bb.3.max(y));
        }
    }
    (n, max, bb)
}

fn assert_same(a: &Pixmap, b: &Pixmap, what: &str) {
    let (n, max, bb) = diff_stats(a, b);
    assert!(n == 0, "{what}: {n} pixels differ (max delta {max}) within {bb:?}");
}

#[test]
fn tiles_stitch_to_render_page_exactly() {
    let t = scene();
    for zoom in [1.0f32, 0.5, 2.0, 0.25] {
        // the viewport origin sits on the tile grid, as the tile cache lays it out
        let vp = Viewport { offset: Vec2::ZERO, zoom, dpr: 1.0 };
        let mut cache = TileCache::new();
        let f = cache.render_tiles(&t.doc, t.page, &vp, (SIDE, SIDE), 1, TileBudget::unlimited());
        assert_eq!(f.tiles.len(), 9, "zoom {zoom}");
        assert_eq!(f.pending(), 0);
        assert_same(&stitch(&f, &vp), &full(&t, &vp), &format!("zoom {zoom}"));
    }
}

/// With arbitrary float geometry (irrational rotation, fractional offsets) the
/// edge coverage of a handful of pixels can differ by float rounding because a
/// tile's transform has a smaller magnitude. It must stay imperceptible.
#[test]
fn tiles_match_within_rounding_for_arbitrary_geometry() {
    let mut t = scene();
    let rot = t.doc.get(t.page).unwrap().children[1];
    let a = Affine::rotate(0.5);
    t.node(rot).transform = Affine { tx: 563.3, ty: 417.7, ..a };
    let vp = Viewport { offset: Vec2::ZERO, zoom: 1.0, dpr: 1.0 };
    let mut cache = TileCache::new();
    let f = cache.render_tiles(&t.doc, t.page, &vp, (SIDE, SIDE), 1, TileBudget::unlimited());
    let (n, max, _) = diff_stats(&stitch(&f, &vp), &full(&t, &vp));
    assert!(n <= 32 && max <= 32, "{n} pixels differ, max delta {max}");
}

#[test]
fn tiles_match_when_panned_by_whole_tiles() {
    let t = scene();
    let vp = Viewport { offset: Vec2::new(256.0, 256.0), zoom: 1.0, dpr: 1.0 };
    let mut cache = TileCache::new();
    let f = cache.render_tiles(&t.doc, t.page, &vp, (SIDE, SIDE), 1, TileBudget::unlimited());
    assert_same(&stitch(&f, &vp), &full(&t, &vp), "panned");
}

#[test]
fn indexed_render_page_matches_plain() {
    let t = scene();
    let mut idx = SceneIndex::new();
    for (zoom, off) in [(1.0f32, Vec2::new(100.0, 50.0)), (0.5, Vec2::ZERO), (3.0, Vec2::new(200.0, 150.0))] {
        let vp = Viewport { offset: off, zoom, dpr: 1.0 };
        let mut a = Pixmap::new(640, 480).unwrap();
        let mut b = Pixmap::new(640, 480).unwrap();
        render_page(&t.doc, t.page, &vp, &mut a);
        render_page_indexed(&t.doc, t.page, &vp, &mut b, &mut idx, 1);
        assert_same(&a, &b, "indexed");
    }
}

#[test]
fn edit_reredraws_only_affected_tiles_and_stays_exact() {
    let mut t = scene();
    let vp = Viewport { offset: Vec2::ZERO, zoom: 1.0, dpr: 1.0 };
    let mut cache = TileCache::new();
    cache.prefetch_ring = 0;
    let f = cache.render_tiles(&t.doc, t.page, &vp, (SIDE, SIDE), 1, TileBudget::unlimited());
    assert_eq!(f.rendered, 9);

    // nothing changed: nothing re-rendered
    let f = cache.render_tiles(&t.doc, t.page, &vp, (SIDE, SIDE), 1, TileBudget::unlimited());
    assert_eq!(f.rendered, 0);
    let f = cache.render_tiles(&t.doc, t.page, &vp, (SIDE, SIDE), 2, TileBudget::unlimited());
    assert_eq!(f.rendered, 0, "revision bump without a change");

    // nudge the rotated, translucent node (bottom right)
    let victim = t.doc.get(t.page).unwrap().children[1];
    t.node(victim).transform.tx += 30.0;
    // with a zero budget the previous image stays up and is flagged stale
    let f = cache.render_tiles(&t.doc, t.page, &vp, (SIDE, SIDE), 3, TileBudget::tiles(0));
    assert_eq!(f.rendered, 0);
    assert!(f.stale > 0 && f.stale < 9, "stale = {}", f.stale);
    assert!(f.tiles.iter().filter(|t| t.stale).count() == f.stale);
    // then the stale tiles are redrawn
    let f = cache.render_tiles(&t.doc, t.page, &vp, (SIDE, SIDE), 3, TileBudget::unlimited());
    assert!(f.rendered > 0 && f.rendered < 9, "rendered = {}", f.rendered);
    assert_eq!(f.pending(), 0);
    assert_same(&stitch(&f, &vp), &full(&t, &vp), "after edit");

    // structural edits: delete a node, hide another, reorder
    let kids = t.doc.get(t.page).unwrap().children.clone();
    t.doc.remove(kids[2]).unwrap();
    t.node(kids[3]).visible = false;
    let f = cache.render_tiles(&t.doc, t.page, &vp, (SIDE, SIDE), 4, TileBudget::unlimited());
    assert_same(&stitch(&f, &vp), &full(&t, &vp), "after remove/hide");
    let last = *t.doc.get(t.page).unwrap().children.last().unwrap();
    t.doc.reparent(last, t.page, 0).unwrap();
    let f = cache.render_tiles(&t.doc, t.page, &vp, (SIDE, SIDE), 5, TileBudget::unlimited());
    assert_same(&stitch(&f, &vp), &full(&t, &vp), "after reorder");

    // a change below a blurred parent, and one beneath the background blur
    let frame = t.doc.get(t.page).unwrap().children.iter().copied().find(|c| matches!(t.doc.get(*c).unwrap().kind, NodeKind::Frame(_))).unwrap();
    let blurred = t.doc.get(frame).unwrap().children[1];
    t.node(blurred).fills = vec![Paint::solid(Color::rgb(255, 0, 255))];
    let f = cache.render_tiles(&t.doc, t.page, &vp, (SIDE, SIDE), 6, TileBudget::unlimited());
    assert_same(&stitch(&f, &vp), &full(&t, &vp), "blurred child");
    t.rect(None, Rect::new(190.0, 470.0, 40.0, 40.0), Color::rgb(0, 0, 0));
    let f = cache.render_tiles(&t.doc, t.page, &vp, (SIDE, SIDE), 7, TileBudget::unlimited());
    assert_same(&stitch(&f, &vp), &full(&t, &vp), "under bg blur");

    // background colour change invalidates everything
    t.node(t.page).kind = NodeKind::Page { background: Color::rgb(10, 20, 30) };
    let f = cache.render_tiles(&t.doc, t.page, &vp, (SIDE, SIDE), 8, TileBudget::unlimited());
    assert_eq!(f.rendered, 9);
    assert_same(&stitch(&f, &vp), &full(&t, &vp), "background");
}

#[test]
fn invalidate_rect_and_all() {
    let t = scene();
    let vp = Viewport { offset: Vec2::ZERO, zoom: 1.0, dpr: 1.0 };
    let mut cache = TileCache::new();
    cache.prefetch_ring = 0;
    cache.render_tiles(&t.doc, t.page, &vp, (SIDE, SIDE), 1, TileBudget::unlimited());
    cache.invalidate_rect(Rect::new(10.0, 10.0, 20.0, 20.0));
    let f = cache.render_tiles(&t.doc, t.page, &vp, (SIDE, SIDE), 1, TileBudget::tiles(0));
    assert_eq!(f.stale, 1);
    cache.invalidate_all();
    let f = cache.render_tiles(&t.doc, t.page, &vp, (SIDE, SIDE), 1, TileBudget::tiles(0));
    assert_eq!(f.stale, 9);
}

#[test]
fn zoom_change_falls_back_to_previous_scale() {
    let t = scene();
    let mut cache = TileCache::new();
    cache.prefetch_ring = 0;
    let a = Viewport { offset: Vec2::ZERO, zoom: 1.0, dpr: 1.0 };
    cache.render_tiles(&t.doc, t.page, &a, (SIDE, SIDE), 1, TileBudget::unlimited());
    let b = Viewport { offset: Vec2::ZERO, zoom: 2.0, dpr: 1.0 };
    let f = cache.render_tiles(&t.doc, t.page, &b, (SIDE, SIDE), 1, TileBudget::tiles(0));
    assert_eq!(f.missing, 9);
    assert!(!f.fallback.is_empty());
    let f = cache.render_tiles(&t.doc, t.page, &b, (SIDE, SIDE), 1, TileBudget::unlimited());
    assert_eq!(f.pending(), 0);
    assert!(f.fallback.is_empty());
}
