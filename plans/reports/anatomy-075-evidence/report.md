# Anatomy scene re-trace on 0.7.5

Branch `jangtrinh/anatomy-075-retrace`. Not pushed. Commits: `cd0a988a` (re-trace), `591d68c3` (package split), `71a5e219` (merge of `origin/main` 8cf5dfd8, which includes #1394), then this report.

## What changed

| File | Change |
|---|---|
| `scripts/anatomy_trace/` | new package, Pillow only, deterministic, run with `python3 scripts/anatomy_trace RENDER.png --out ...`. Modules and sizes: `__init__` 54 (method docstring), `__main__` 14, `cli` 100, `config` 35, `skeleton` 80, `edges` 49, `splits` 131, `text` 98, `tracing` 130, `layer` 66, `svg` 30, `metrics` 92, `outputs` 99. Longest is 131 lines. |
| `docs/images/v2/anatomy/{file,canvas,layers,inspector,rail,chrome}.svg`, `anatomy.json` | regenerated from the 0.7.5 render, same format and keys |
| `docs/images/v2/anatomy/studio-window-075.webp` | 1600x1000, q80, 104,180 B (the 061 file was 103,246 B) |
| `docs/images/v2/anatomy/studio-window-061.webp` | deleted: `git grep` finds no reference outside `plans/` |
| `docs/index.html` | the anatomy `<img>` `src` only |
| `docs/assets/v2-anatomy.js` | header comment 0.6.1 -> 0.7.5 |

Render: `studio-window-075.png`, sha256 `d21f35d862efd4d2dd7ddd3127f9275383485ba1aa32e789e17acbfb73953f30`.

Reproduce:
`python3 scripts/anatomy_trace <render> --out docs/images/v2/anatomy --source 'DESIGN:OS Studio 0.7.5 (tag v0.7.5) --screenshot of the Untitled UI PRO v8.0 kit, "Input field" selected, zoom 17, 1440x900 at scale 2, --no-toast' --webp docs/images/v2/anatomy/studio-window-075.webp --log plans/reports/anatomy-075-evidence/trace-075.txt --evidence plans/reports/anatomy-075-evidence --chrome "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"`

## Package split: behaviour unchanged

The code moved verbatim (functions copied by AST; two cross-module helpers renamed `frame_off` and `comp_box`, no leading underscore). After the split the reproduce command above was rerun. The six SVGs, `anatomy.json` and the webp match the committed ones byte for byte; `git status` showed no change to the evidence PNGs or `trace-075.txt` either.

| file | sha256 before (commit cd0a988a) | sha256 after (package) |
|---|---|---|
| file.svg | a343987e479c5195a878ad74e5e1275ec34051331500c14815f9b24bb7397be9 | same |
| canvas.svg | 1fd5e2be56d0d6f72a290986eb23ca3b9fbf66b3a469efc861c66b9d137126cc | same |
| layers.svg | b9797bb298194803164910c65c7eb962aa62df1167cb17f9623903661aa21998 | same |
| inspector.svg | 2c13533da70db7020d0e05dd1b04709d85251afe3989e7b72206f39b85464e57 | same |
| rail.svg | 01270c0c43eef09818fa3ad2d35d0174da096457842a042ffefb9b58c4fd26ab | same |
| chrome.svg | 5c0314c7736dd0df77a923e6771e9c5daa29b5f0619fca51f5f0cc7405c489e7 | same |
| anatomy.json | 58d6fa4d92157f48b3fb69d648949ab69d1baff5f077ce89660fd8c7e2925540 | same |
| studio-window-075.webp | 9464779190a000e88f1af829faeea4edb7493beb617114bcc800de3250c13dab | same |

A static check of every module found no unused import and no undefined name.

## Merge of origin/main

`git merge --no-ff origin/main` (8cf5dfd8): auto-merged `docs/index.html` with no conflict. Merge commit `71a5e219`. Against `origin/main`, `docs/index.html` differs by exactly one line, the anatomy `<img>` `src` (061 -> 075); #1394's alt and caption changes are kept. `git grep` still finds no `studio-window-061` outside `plans/`.

## Measured splits (source px, from the render)

tab bar bottom 64, rail right 98 (was 114), Layers panel right 580 (was 596), vertical ruler right 620 (was 636), horizontal ruler bottom 104, inspector left 2398. The rail went from 57 to 49 logical px and is icon-only (0 text bars).

## Metrics (full printout: `trace-075.txt`)

Pairwise overlap between layers: 0 px on the structure maps (sum 203,180 = union 203,180) and 0 px on the rasterised 1 px strokes.

Per layer, as specified (drawing vs the structure map it was traced from, text runs collapsed to bars, 2 px Chebyshev tolerance):

| layer | map px | bytes | subpaths | bars | recall | precision |
|---|---|---|---|---|---|---|
| file | 144,283 | 63,502 | 2,575 | 960 | 100.00% | 100.00% |
| canvas | 18,809 | 14,438 | 729 | 34 | 100.00% | 100.00% |
| layers | 11,906 | 3,519 | 103 | 24 | 100.00% | 100.00% |
| inspector | 19,795 | 8,606 | 222 | 27 | 100.00% | 100.00% |
| rail | 2,920 | 1,749 | 39 | 0 | 100.00% | 100.00% |
| chrome | 5,467 | 1,180 | 29 | 2 | 100.00% | 100.00% |
| ALL | 203,180 | SVGs 92,994 B, with JSON 99,725 B = 97.4 KB | | | 100.00% | 100.00% |

Shifted 6 px (shows the metric reacts), ALL: x recall 83.1% / precision 83.7%; y 44.6% / 44.7%.

The as-specified number is close to circular: the drawing is built from that same map. The old log had the same property. The independent check is a Sobel gradient of the render:

| check | value |
|---|---|
| P_src, drawn shape px within 2 px of a gradient >= 6 | 99.91% |
| P_src, same at gradient >= 15 (like for like with the old log) | 94.46% |
| R_src_strong15, gradient >= 15 px within 2 px of any drawn px | 84.66% |
| same, outside the boxes of text runs | 96.48% |
| same, text-zone px counted as covered (the run is a bar) | 98.23% |

The old log had 97.1% (P_src), 97.08% (R_src_strong15) and 99.52% (text zones widened), so the independent recall is lower now. Why is an inference, not a measured split: this render has much more UI text, which is drawn as one bar per run by design. Spot-viewing the uncovered strong-edge px outside text zones showed filled-icon interiors, glyph residue and low-contrast detail. The coordinator accepted criterion 1 on the as-specified metric plus the visual evidence.

## Evidence

- `overlay-075.png`: six drawings (`#ff2d55`, inlined SVG) over the render at 0.6, Chrome headless 1440x900.
- `overlay-075-detail.png`: the same at 2x for the top bar and rulers, rail, Pages/Layers, inspector fields, canvas content and tool bar.
- `layers-colored-075.png`: one colour per layer on black.
- `trace-075.txt`: the printout.

## Misalignments found and fixed while viewing

- overlay stacked the six SVGs vertically (only `file` visible): each SVG now sits in its own absolutely positioned layer
- document fields fused to their labels and hints (3x3 edge band at 17% zoom): thin edges inside the document
- long bars across several columns: text candidates in the document limited to 12 px tall
- 245/255 frame edges missing on the left and right sides: faint edges use the thick band
- rounded corners dropped (diagonal 2 px steps split chains): junctions by crossing number
- text touching a border (ruler number, size pill caps) swallowed: straight runs >= 200 px set aside while looking for text

## Not done / known blemishes

- The tracer was tuned and checked on this one render. The 0.6.1 PNG is not available (only a 1600x1000 webp), so it was not re-run on it.
- `-1000` at the top of the vertical ruler stays a glyph outline (clipped and merged with the corner). The sparkles rail icon is drawn smoother than the render. Filled-icon interiors are not reproduced. Total drawing size is 97.4 KB against 72.2 KB before (the document layer grew 49 -> 63.5 KB).
- The page itself was not run: `.project-agent.md` has no `localhost_port`, so per the port contract no server was started. The JS-facing format was checked instead: same root and group attributes, same `data-kind`/`vector-effect`, same JSON keys per layer, every vertex on the 0.5 logical px grid.
- Anchors follow the same rules as before (window-edge or region centre, file = ink bbox right edge) but moved with the layout: canvas 744.5 (was 750), layers 169.5 (was 177.5).
- The first handback could not write this file (the runtime refused the Write tool, twice); it was written through a Bash heredoc on the coordinator's instruction.
