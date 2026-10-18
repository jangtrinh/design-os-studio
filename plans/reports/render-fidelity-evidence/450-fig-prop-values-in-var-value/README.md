# #450: hidden button icons are drawn

Untitled UI PRO v8.0 kit frames `Type=Card mockup 07, Breakpoint=Desktop` and `Type=Geometric shapes 01, Breakpoint=Desktop`, re-imported from the kit `.fig` with the old and the fixed importer (same camera, same renderer binary), 1440x810 at 2x.

The "Log in" / "Sign up" buttons nest two placeholder icon layers whose visibility is bound to the boolean properties `⬅️ Icon leading` / `➡️ Icon trailing`. The kit's instance sets both to false, in the `varValue.value.boolValue` form that the importer ignored.

| | before | after |
|---|---|---|
| Card mockup 07, nav buttons (4x crop) | `card-mockup-07-nav-buttons-before.png`: a circle over the "L" and the "S" | `card-mockup-07-nav-buttons-after.png`: label only |
| Geometric shapes 01, nav buttons (4x crop) | `geometric-shapes-01-nav-buttons-before.png` | `geometric-shapes-01-nav-buttons-after.png` |
| Full frames, CPU | `*-cpu-before.png` | `*-cpu-after.png` |
| Full frames, GPU | `*-gpu-before.png` | `*-gpu-after.png` |

The dot grid and the grid lines are the mask bug (#448) and the capped screenshot frame (#449); they are not part of this change.

Imported document, instance `Buttons/Button`: `component.values` was `{}` and both placeholders were visible; after the fix it holds both assignments (false) and both placeholders are hidden.

```
designos-studio <doc> --select none --zoom 55 --size 1440x810 --scale 2 [--gpu] --no-toast --screenshot out.png
```
