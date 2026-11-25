//! One integration-test binary for the crate: one module per former test file.
//! The font-registering test and the two wall-clock benchmarks keep their own process beside this directory.

#[path = "../common/mod.rs"]
mod common;

mod adapter;
mod board_dashes;
mod command_buffer_limit;
mod dashed_endpoint_caps;
mod display_p3_parity;
mod fidelity;
mod full_detail_at_rest;
mod group_effect_masks;
mod imported_basic_shapes;
mod live_line_stroke_band;
mod mask_without_own_fill;
mod program_cap;
mod refine_chunks;
mod refine_vram_budget;
mod resized_imported_line;
mod resized_shapes;
mod shadow_blend_parity;
mod shadow_fillless_fixture;
mod shadow_silhouette_source;
mod stored_glyph_parity;
mod table_parity;
mod text_stroke_alignment;
