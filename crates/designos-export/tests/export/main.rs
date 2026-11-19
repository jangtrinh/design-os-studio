//! One integration-test binary for the crate: one module per former test file.

mod pdf_probe;

mod board;
mod codegen_escapes_variable_code_syntax;
mod display_p3_png;
mod display_p3_png_images;
mod export_bounds_rules;
mod grid_chrome_fixtures;
mod grid_codegen_placement_normalisation;
mod grid_codegen_resolved_row_tracks;
mod grid_codegen_spacing_bindings;
mod grid_codegen_tailwind_scale;
mod grid_codegen_track_counts;
// `pub` keeps the scene struct's unread `items` field out of the dead-code lint, as it was at a binary root.
pub mod grid_native_fixtures;
mod grid_native_weights;
mod grid_rules_single_source;
mod grid_stroke_bound_variables;
mod kit_export_bounds;
mod live_line_export_band;
mod missing_font_glyph_exports;
mod native_compile_fixtures;
mod native_stack_export;
mod non_grid_stroke_export_unchanged;
mod non_grid_stroke_placement;
mod pdf_export;
mod pdf_vector_fidelity;
mod pixmap_size_cap;
mod slot_codegen;
mod svg_full_ring_stroke_alignment;
mod svg_polygon_star_points_match_canvas;
mod table_export;
mod variable_code_syntax_per_platform;
