# #448: mask frame without its own fill

Untitled UI PRO v8.0 kit frames `Type=Card mockup 07, Breakpoint=Desktop` and `Type=Geometric shapes 01, Breakpoint=Desktop`, CPU renderer, 1440x810 at 2x.

| | before (0.3.0) | after (this branch) |
|---|---|---|
| Card mockup 07 | `card-mockup-07-cpu-before.png`: dots everywhere, over the headline and buttons | `card-mockup-07-cpu-after.png`: dots fade out from the top as in Figma |
| Geometric shapes 01 | `geometric-shapes-01-cpu-before.png`: grid lines across the whole hero | `geometric-shapes-01-cpu-after.png`: no grid lines over the content |

`card-mockup-07-cpu-after.png` is byte-identical (md5 `59014f04…`) to the render of the same document with a zero-alpha fill added by hand to the mask frame, so the fill fallback was the only difference.

The GPU view of these frames also needs the screenshot fix for #449 (the capped interactive frame replaces masks by a plain clip); see that PR for the GPU before/after.

```
designos-studio <doc> --select none --zoom 55 --size 1440x810 --scale 2 --no-toast --screenshot out.png     # card mockup 07
designos-studio <doc> --select none --zoom 40 --size 1440x810 --scale 2 --no-toast --screenshot out.png     # geometric shapes 01
```
