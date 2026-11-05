VERDICT: CONCERNS

1. [P2] `crates/designos-app/src/panels/components.rs:183-186` — `truncate_description_lines` appends `…` to the already width-wrapped second line without making room for it. When that line fills the 184px content width, the ellipsis can paint past the panel content edge, violating AC14. Measure the candidate line and remove trailing graphemes until the ellipsis fits; add a width-based truncation test.
2. [P3] `crates/designos-app/src/panels/components.rs:154` — `source_line` accepts `_comp` but never uses it. Remove this argument and its call-site value to avoid dead API surface.
