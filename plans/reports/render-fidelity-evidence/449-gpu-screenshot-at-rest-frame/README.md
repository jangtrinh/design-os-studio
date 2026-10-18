# #449: GPU screenshot captures the at-rest frame

Untitled UI PRO v8.0 kit frames, GPU renderer (`--gpu`), 1440x810 at 2x. The mask fix of #448 is **not** in this branch, so the dot grid still covers the hero here; what changes is that the capture now shows the frame the app presents at rest.

| | before (0.3.0) | after (this branch) |
|---|---|---|
| Card mockup 07 | `card-mockup-07-gpu-before.png`: group opacity ignored, the dot grid at full strength (md5 `c5eaaa41…`) | `card-mockup-07-gpu-after.png`: the dot grid at its 20% group opacity, like the CPU render |
| Geometric shapes 01 | `geometric-shapes-01-gpu-before.png` | `geometric-shapes-01-gpu-after.png`: the hero's guide lines are already faint, so the difference is small here |

Trace of the old behaviour (temporary log in `gpu::paint`, Card mockup 07):

```
plan=Interactive t=0.104 ... compiled programs=64      (the 64-program cap)
plan=Interactive t=0.388 stale=19 ... programs=64
plan=Wait(234ms)  t=0.404 stale=0                      first stable frame
plan=Wait(48ms)   t=0.543                              screenshot requested here
                                                       (the refine was due at t=0.65)
```

```
designos-studio <doc> --select none --zoom 55 --size 1440x810 --scale 2 --gpu --no-toast --screenshot out.png   # card mockup 07
designos-studio <doc> --select none --zoom 40 --size 1440x810 --scale 2 --gpu --no-toast --screenshot out.png   # geometric shapes 01
```

With #448 and this change together the GPU capture of Card mockup 07 matches the CPU render (see the combined evidence in the PR for #450).
