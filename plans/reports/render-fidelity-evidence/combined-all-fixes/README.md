# Render fidelity of Untitled UI kit frames: all three fixes together

Before = Studio 0.3.0 on documents imported with the 0.3.0 importer. After = the three fix branches merged (#448 mask frame without fill, #449 GPU screenshot at rest, #450 property values in `varValue`) on documents re-imported with the fixed importer. Same kit frames, same camera (`--zoom 55`), 1440x810 at 2x, CPU and GPU renderer.

| Frame | CPU before | CPU after | GPU before | GPU after |
|---|---|---|---|---|
| Card mockup 07 | `card-mockup-07-cpu-before.png` | `card-mockup-07-cpu-after.png` | `card-mockup-07-gpu-before.png` | `card-mockup-07-gpu-after.png` |
| Geometric shapes 01 | `geometric-shapes-01-cpu-before.png` | `geometric-shapes-01-cpu-after.png` | `geometric-shapes-01-gpu-before.png` | `geometric-shapes-01-gpu-after.png` |

What changes: the dot grid and the guide lines fade out from the top as in Figma instead of covering the whole frame (#448); the GPU capture shows the same at-rest frame as the CPU render, group opacity included (#449); the "Log in" / "Sign up" buttons draw no placeholder circle over the label (#450).

```
designos-studio <doc> --select none --zoom 55 --size 1440x810 --scale 2 [--gpu] --no-toast --screenshot out.png
```
