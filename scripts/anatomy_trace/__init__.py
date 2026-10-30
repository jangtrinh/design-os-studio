"""Trace a Studio render into the six hairline drawings of the homepage "anatomy" scene.

  python3 scripts/anatomy_trace RENDER.png --out docs/images/v2/anatomy \\
      [--source TEXT] [--webp FILE] [--log FILE] [--evidence DIR --chrome PATH]

RENDER.png is a 2880x1800 `designos-studio --screenshot --scale 2` capture of a 1440x900 window.
Output: file.svg canvas.svg layers.svg inspector.svg rail.svg chrome.svg plus anatomy.json (the files
docs/assets/v2-anatomy.js loads). Needs Python 3 and Pillow; numpy is not used. The run is
deterministic: the same render gives byte-identical files.

Method
  1. Edges. Two detectors on the colour channels. Thick: the 3x3 range of each channel, so a step edge is
     2 px wide and a 2 px border 4 px. Thin: a pixel differs from its right or lower neighbour, so a step
     is 1 px and two features 2 px apart stay apart. Thick is used everywhere except inside the document
     (the viewport interior, minus the tool bar and the selection overlay), which is drawn at 17% zoom, where
     text, borders and fields are 1-2 px and thick edges would fuse a label to its field. An edge counts at
     >= EDGE_T (12 levels). A fainter edge (>= FAINT_T, 8: the 245 page against a 255 frame) counts only
     when its skeleton runs >= FAINT_MIN px, so frame outlines survive and antialiasing halos do not.
  2. Structure map. Each edge map is thinned to a 1 px, 8-connected skeleton (Zhang-Suen, then an L-corner
     pass, both 3x3 LUTs run by Pillow). A 2 px border thins to its centre line. A skeleton pixel (x, y)
     becomes the lattice vertex (x + 1, y + 1): the centre of an even-width band is a pixel boundary. Every
     vertex is an integer source px, i.e. a multiple of 0.5 logical px.
  3. Splits. Borders are measured, never hard-coded: a 2 px line whose two edges cross >= 85% of the window
     is a border. The four full-height ones are, left to right, the rail, the Layers panel, the vertical
     ruler and the inspector; the full-width ones are the tab bar bottom and the horizontal ruler bottom.
     A border belongs to the panel whose edge it is.
  4. Layers by vertex position: chrome (tab bar), rail, layers (Pages and Layers), inspector, canvas
     (rulers, tool bar, selection overlay) and file (the document inside the viewport). Inside the
     viewport a stroke is canvas when it lies within CYAN_R px of the overlay cyan (selection box, handles,
     frame names, size pill) or in the tool bar, else file. Every skeleton pixel is assigned once, so the
     six maps never overlap.
  5. Text. Per layer, straight runs >= LINE_MIN (200) px (borders, dividers) are set aside so a label touching a
     border is still found. A skeleton component is a text candidate when it fits a line of text (<= 34 px
     across, <= 200 along; <= 12 across in the document), is not an icon (a near-square blob >= 22 px, or
     >= 30 x 28), a tick (thin and >= 24 long) or an outlined field (skeleton on all four sides of its box).
     Candidates on one line, no further apart than 0.95 of the glyph height (0.6 in the document), join
     into a run. Two or more candidates, or one component at least 2.2x as long as it is tall, are a text
     run. The vertical ruler's rotated numbers are grouped along y. A run is ONE bar at its pixel-weighted
     centre line, from its first to its last ink. A lone glyph or icon stays an outline.
  6. Shapes. The remaining skeleton is traced into chains that meet at junctions (a pixel whose 8-ring
     has >= 3 runs), simplified with Douglas-Peucker (0.5 source px; 0.8 in the document) and axis snapped
     (a segment whose cross-axis step is <= 1 px becomes exactly horizontal or vertical). Components under
     3 px are dropped.
  7. Gate. Recall and precision of the rasterised drawing against the structure map (text runs collapsed
     to their bars), with a 2 px Chebyshev tolerance, as specified; the same with the drawing shifted 6 px,
     to show the metric reacts; and an independent check against a Sobel gradient of the render.
     Layers never overlap: the maps by construction, the rasterised strokes by measurement.

Coordinates in the SVGs are logical px (viewBox 0 0 1440 900), relative `l` segments, stroke 1.

Modules (steps in brackets): config (constants), skeleton and edges (1, 2), splits (3, 4), text (5),
tracing and layer (5, 6), svg, metrics (7), outputs (anatomy.json, evidence images), cli (entry point).
"""
