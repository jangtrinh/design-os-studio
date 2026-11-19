//! One integration-test binary for the crate: one module per former test file.
//! Files that register fonts or otherwise need their own process stay separate beside this directory.

#[path = "../common/mod.rs"]
mod common;

mod backdrop;
mod backdrop_index;
mod basic_shapes_match_figma;
mod blend;
mod board;
mod board_gallery;
mod boolean_with_zero_width_ring;
mod color_profile;
mod color_profile_all_paints;
mod color_profile_image_cost;
mod color_profile_images;
mod corners;
mod dashed_endpoint_caps;
mod decoded_image_mean;
mod fx;
mod geometry;
mod gradients;
mod group_effect_masks;
mod image_adjust_sampling;
mod inside_stroke_stored_outline;
mod lint_contrast;
mod lint_rules;
mod live_line_stroke_band;
mod mask_without_own_fill;
mod outline_stroke_of_dashed_rectangles;
mod pixmap_size_cap;
mod render;
mod resized_imported_line;
mod resized_vector_stroke_weight;
mod shadow_blend;
mod shadow_fillless;
mod table_render;
mod text_fill_source;
mod text_lists;
mod text_stroke_alignment;
mod tiles;
mod tiles_edit_while_converging;
mod variables;
mod vector_network;
mod vectorops;
