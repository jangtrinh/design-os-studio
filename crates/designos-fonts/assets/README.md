# Bundled fonts

## Inter (`inter/`)

| File | Bytes | Notes |
| --- | --- | --- |
| `Inter[opsz,wght].ttf` | 876576 | variable, axes `opsz` 14-32 and `wght` 100-900 |
| `Inter-Italic[opsz,wght].ttf` | 906596 | variable italic, same axes |
| `OFL.txt` | 4377 | license text |

- Source: the official Google Fonts repository, `https://github.com/google/fonts/raw/main/ofl/inter/` (branch `main`, downloaded 2026-10-10).
- Font version: 4.001 (`Version 4.001;git-66647c0bb`, Inter by The Inter Project Authors, https://github.com/rsms/inter). Fontsource lists the same family as `v20`, last modified 2025-09-10.
- License: SIL Open Font License 1.1 (`inter/OFL.txt`). Copyright 2020 The Inter Project Authors.

## Roboto Mono (`roboto-mono/`)

| File | Bytes | Notes |
| --- | --- | --- |
| `RobotoMono[wght].ttf` | 183700 | variable, axis `wght` 100-700 |
| `OFL.txt` | 4395 | license text |

- Source: the official Google Fonts repository, `https://github.com/google/fonts/raw/main/ofl/robotomono/` (branch `main`, downloaded 2026-10-18). The file is byte-identical to `fonts/variable/RobotoMono[wght].ttf` in `https://github.com/googlefonts/RobotoMono`.
- Font version: 3.001. The UI monospace role (numbers, hex codes, token paths) renders in it at `wght` 400.
- License: SIL Open Font License 1.1 (`roboto-mono/OFL.txt`). Copyright 2015 The Roboto Mono Project Authors. Older releases were Apache-2.0; the current upstream, the Google Fonts `METADATA.pb` (`license: "OFL"`) and the font's own license string all say OFL-1.1.

Embedded into the binary with `include_bytes!` in `src/assets.rs`.
