# designos-gpu

GPU canvas renderer for designos documents. It draws a `designos_doc::Document`
page with wgpu, either straight into an egui paint callback (the editor canvas)
or into an offscreen texture (tests, benchmarks, `designos --screenshot --gpu`).

The CPU renderer in `designos-render` (tiny-skia) is the deterministic
reference: `--screenshot` and every pixel-diff baseline keep using it, and this
crate is held to it by `tests/gpu/fidelity.rs`, which renders scenes with both and
compares them with `designos-diff`.

## Backend decision: wgpu pipelines + lyon, not vello

Candidates: **vello** (GPU compute 2D renderer on wgpu) and a **custom wgpu
renderer** that tessellates paths with `lyon` and draws them with instanced
pipelines.

### wgpu version

`Cargo.lock` pins eframe/egui-wgpu 0.35 to **wgpu 29.0.4**. Checking
crates.io (October 2026): vello 0.11 depends on `wgpu ^30`, vello 0.10 and 0.9
on `wgpu ^29.0.3`, vello 0.8 on `wgpu ^28`. So vello 0.10 *does* share the
wgpu major with egui 0.35; the version match is not what rules it out. The
decision rests on feature coverage and complexity.

### Feature coverage

| Need (CPU renderer parity) | vello 0.10 | wgpu + lyon (this crate) |
|---|---|---|
| Solid, linear, radial gradients | yes | yes (shader, ramp texture) |
| Angular gradient | sweep gradient (needs mapping) | yes |
| Diamond gradient | **no** | yes |
| Image fills: fill / fit / crop / tile | images, no letterbox / pad semantics | yes |
| Stroke with inside / outside alignment | via clip layers | clip mask (same construction as the CPU path) |
| Dashes, caps, joins, miter limit | own stroker (results differ from tiny-skia) | the CPU renderer's own stroker output is tessellated, so strokes are the same geometry |
| Opacity layers, blend modes, clips, masks | layers; no LinearBurn / LinearDodge | layers, all 19 modes |
| Drop / inner shadow, layer blur, background blur | **only blurred rounded rects**; no general blur and no backdrop access | render-to-texture + separable Gaussian |
| Noise, Texture, Glass | **no** (would need a second pipeline anyway) | shaders |
| Retained per-node cache | none: the scene is re-encoded every frame | tessellation cached per node and zoom bucket |
| Needs | compute shaders + storage textures (fails on GL / old drivers) | plain render pipelines (runs on every wgpu backend) |

Shadows, blurs, background blur, noise, texture and glass are half of what the
CPU renderer does. All of them need render-to-texture with custom passes in
the middle of the draw order, which vello's single "encode scene, then render"
model does not offer: every such node would need its own full vello render plus
an image round trip. Building our own layer system for those effects also
gives us blend modes, masks and clips for free, so vello would only replace
the path rasteriser while we kept writing everything around it.

### Complexity

vello adds a large dependency tree (shader preprocessing, peniko, kurbo,
skrifa copies), a multi-second shader compile at start-up and a hard
requirement for compute support. The custom renderer adds `lyon_tessellation`
and about 2k lines of Rust + WGSL that we fully control and can debug with the
pixel-diff harness.

Trade-off accepted: MSAA 4x edges instead of vello's analytic coverage (small
edge differences, covered by the tolerance of the fidelity tests), and more
code to own.

## Architecture

```
Document ──► compile (CPU, per frame) ──► Frame ──► execute (GPU) ──► texture
              geometry cache (lyon)        programs     passes, pools
```

* `compile`: walks the visible part of the page exactly like the CPU renderer
  (`draw_children` / `draw_node` / `draw_body`) and emits a `Frame`: a list of
  render *programs* (the page, plus layers, masks, clip masks and effect
  textures), each a list of draw and composite ops. Geometry comes from
  `designos-render` (`outline_of`, `text_outlines`, `stroke_plan`), so shapes,
  glyph positions, stroke expansion and boolean operations are the CPU
  renderer's. Meshes are cached per node, keyed by the node's geometry-relevant
  fields and a zoom bucket, so pan and zoom never re-tessellate.
* `execute`: owns pipelines, a mesh arena, a texture pool and the passes. It
  runs programs (children first, in-order when they read the backdrop), blurs
  and composites.
* The compile half needs `&Document`; the execute half is `Send + Sync` and is
  what egui's paint callback runs, so the editor builds the `Frame` while
  painting the UI and hands it to the callback.

Noise, Texture and Glass run as shaders ports of `designos-render`'s `fx.rs`
(the same integer hash on the absolute pixel grid, so grain and displacement
line up with the CPU output). Solid rounded-rectangle shadows use an analytic
shader (one quad, no layers); every other shadow is a silhouette layer, a
separable Gaussian blur and a composite.

## Fidelity

`tests/gpu/fidelity.rs` renders scenes with both renderers headless and compares
them with `designos-diff` at tolerance 8. On an RTX 4070 (Vulkan):

| scene | strict mismatch | AA pixels forgiven |
|---|---|---|
| `render_demo` (1560x2000) | 0.99 % | 0.14 % |
| `render_demo` zoomed 3x and panned | 0.77 % | 0.19 % |
| effects + blend modes (glass, noise, texture, 6 blend modes, clip, image) | 0.24 % | 0.07 % |
| same scene with the effects forced through the CPU fallback | 0.24 % | 0.07 % |

Nearly all strict mismatches are edge pixels: tiny-skia computes analytic
coverage (a few levels per edge pixel) while the GPU path uses 4x MSAA, and
supersampling does not close the gap (tiny-skia's own coverage is quantised
too). Tests skip with a printed note when no adapter is available.

## Performance (`cargo run --release -p designos-gpu --example bench_gpu`)

`bench_big`: 5,051 nodes, 1505x1125 viewport, RTX 4070 Laptop (Vulkan).
"frame" includes waiting for the GPU to finish; the editor does not block on
it. CPU numbers are the tile cache (`TileCache`, 6 ms budget, as the editor
uses it): its pan frames show only what it manages to render inside the budget.

| case | zoom | GPU compile | GPU frame | CPU tiles |
|---|---|---|---|---|
| pan (40 px / frame) | 0.25 | 2.8 ms | 4.7 ms | 0.17 ms (p95 7.5, tiles pending) |
| | 1 | 0.9 ms | 1.4 ms | 2.1 ms |
| | 4 | 0.04 ms | 0.3 ms | 0.45 ms |
| zoom gesture (x1.03 / frame) | 0.25 | 6.8 ms | 8.2 ms | 0.00 ms, then 125 ms re-render when it settles |
| | 1 | 0.13 ms | 0.5 ms | 0.00 ms, then 34 ms |
| | 4 | 0.04 ms | 0.4 ms | 0.00 ms, then 29 ms |
| drag one node | 0.25 | 3.5 ms | 5.2 ms | 8.6 ms |
| | 1 | 0.9 ms | 1.4 ms | 5.7 ms |
| | 4 | 0.6 ms | 1.0 ms | 1.8 ms |
| Smart Animate frame (800x600, 100 nodes) | 1 | 10.9 ms | 12.5 ms | 44.6 ms (`render_node`) |

The GPU path always shows the exact frame (nothing is stale or pending); the
cost it pays is first-visit tessellation (0.25 zoom cold frame: 220 ms for
~4,000 nodes, 6 ms at zoom 1) and re-flattening curves when the zoom crosses a
power of two, which the editor spreads over frames with a 6 ms budget.

## Fallbacks

Anything the GPU path should not draw is rendered by the CPU renderer into a
texture for just that node (`render_node_into`, same grid anchoring as the GPU
effects) and composited at the right place in z-order, so content is never
dropped. Triggered by:

* `CompileOptions::force_fallback` (tests, escape hatch).

Nodes that read their backdrop (background blur, glass) never fall back: the
CPU image would not see what the GPU drew beneath them.

## Known gaps

* Image fills are sampled bilinearly with mipmaps; the CPU renderer uses
  bicubic when magnifying, so magnified images differ slightly.
* Hard-clip frames with axis-aligned rectangles use a scissor (edge pixels are
  not anti-aliased); other clips are anti-aliased masks.
* Present mode (including Smart Animate playback) still renders on the CPU;
  `SceneCompiler::compile_node` is ready for it.
* Blur radii beyond ~16 px of sigma are computed at reduced resolution.
