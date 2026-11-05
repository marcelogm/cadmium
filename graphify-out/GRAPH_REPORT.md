# Graph Report - cadmium  (2026-11-04)

## Corpus Check
- 266 files · ~79,370 words
- Verdict: corpus is large enough that graph structure adds value.
- Unclassified: 35 file(s) not represented in the graph (top: .wgsl 23, .toml 2, (none) 2)

## Summary
- 3856 nodes · 9338 edges · 186 communities (151 shown, 35 thin omitted)
- Extraction: 88% EXTRACTED · 12% INFERRED · 0% AMBIGUOUS · INFERRED: 1077 edges (avg confidence: 0.83)
- Token cost: 131,740 input · 2,600 output

## Community Hubs (Navigation)
- Knife & Spreading Tests
- Swatch Drawing & Logo Example
- Canvas Widget & Input
- GPU Compute Passes
- Document Save Tests
- Pigment Slot Selection
- Engine Tool Definitions
- Dip Behaviour Tests
- Frame Presenter Modes
- Cursor Outline & Tray Ring
- Gesture Hold/Contact Tests
- Big Brush Engine Tests
- Smoke Test Config
- Engine Core Constants
- Document Writing
- Document Read Tests
- GPU Readback Helpers
- Tray View Widget
- Brush Panel Layout
- Canvas Spec Validation
- Spectral Colour Science
- Tray Draw-Up Tests
- App Document State
- Stylus Input Samples
- App Tool Definitions
- Dab Placement Records
- Paint Tube & Hold
- Brush Loading Tests
- Stroke Ring Artifact Tests
- Quality CI Workflow
- Main Window Shell
- Brush Panel Presentation
- New Painting Dialog Layout
- Paint Mixture Readout
- Compute Bind Layouts
- Draw-Up Pass
- Document Reading & Errors
- Stroke Fading Tests
- Canvas Zoom & Frames
- DMA-BUF Modifier Policy Tests
- Canvas State Textures
- Stroke Geometry
- GPU Adapter Setup
- Session Stroke Encoding
- Smoke Test Steps
- Canvas Texel Layout
- Canvas Weave Textures
- New Painting Dialog Impl
- Tray View Geometry
- Pen Gesture State
- Knife Reservoir Loading
- Spread Pass Bindings
- DRM Format Modifiers
- File Error Reporting
- New Painting Dialog Signals
- PNG Export & Present
- Session Clear & History Budget
- DMA-BUF Pool Core
- File Write Feedback
- Painting Presets & Formats
- Brush Shapes & Footprint
- Canvas Renderer
- Brush Panel Piles
- File Dialogs
- Window Document Actions
- Lab Panel Sliders
- App State & Stroke Start
- Tool Keypad Bindings
- History Tiles
- DMA-BUF Pool Textures
- Lab Panel Layout
- Pick Pass Bindings
- Application & Accelerators
- Dirty Tile Tracking
- Engine Finish Tests
- Stroke Spacing Tests
- About Dialog & Shortcuts
- Contact Surface State
- Brush Reservoir Tests
- Brush Shape Tests
- Brush Panel Wording
- DMA-BUF Sync Fences
- Brush Dip & Clean State
- Tray View API
- Brush Settings & Dab Upload
- Compute Stage Pipeline
- Flatpak Manifest
- History Atlas
- Canvas Picking & Points
- DMA-BUF Vulkan Export
- DMA-BUF Selftest Pattern
- View Zoom & Pan
- Session Brush Ops
- Release Workflow Steps
- DMA-BUF Selftest Check
- Hold Fill Levels
- Pigment Dip Recipes
- Engine Brush Tests
- History Tests
- Canvas Widget Handlers
- DMA-BUF Fallback Stub
- Lab Param Presentation
- Paint Engine & Params
- README & App Icons
- Brush Panel Widget
- Brush Panel Signals
- DMA-BUF Selftest Run
- Lab View Modes
- DMA-BUF Presenting
- Dip Pass
- History Slot Copies
- Pigment Mixture Colour
- Canvas Widget Signals
- Lab Panel Widget
- Paint Surface Input Trait
- Input Sample Tests
- Hand & History State
- Document State Tests
- History Undo Steps
- Window File Paths
- Pen & Pool Resize Tests
- Smoke Step Model
- Unsaved Writes Tests
- Rag & Dry Tests
- Tray Cord Tests
- Release Workflow Root
- Close & Unsaved Prompt
- Hold Gesture Thresholds
- Swatch Widget Impl
- Release Build Matrix
- App Screenshot UI
- Painting Write Flow
- Painting Format Model
- Smoke Runner
- Tray View Frames
- Document Entry Fixtures
- Canvas Frame Loop
- Selftest Run Limits
- Lab Presentation Tests
- Lab State
- Render Tests
- Python Packaging Scripts
- Release Shell Steps
- Window Action Table
- Brush Panel Snapshot
- Accelerator Tests
- Alert Dialogs
- DMA-BUF Errors
- Window Title
- Frame Encoding State
- Tool Toggles
- Texel Row Padding
- AppImage Builder
- Screenshot Errors
- Overwrite Prompt Tests
- Lab Panel Signals
- Readback Presenting
- Preset Tests
- Smoke Dialogs
- Brush Defaults
- Event Handler Registry
- Release Container Jobs
- Windows Release Job
- File Descriptor Census
- Smoke Env Lookup
- Pigment Click Actions
- Selftest Pattern Tests
- README Painting Tools
- Tool Icon Set
- Modifier Policy Enum
- Release Version Job
- Windows Installer Job
- App Build Script
- Release Notes Script
- File Size Check Script
- Set Version Script
- Windows Bundle Script
- Crate Manifests
- README Packaging
- Move Tool Icon
- Paint Tube Icon
- README Blender Tool

## God Nodes (most connected - your core abstractions)
1. `GpuContext` - 301 edges
2. `PaintSession` - 126 edges
3. `Canvas` - 96 edges
4. `painting_state()` - 75 edges
5. `StrokeSample` - 74 edges
6. `Surface` - 68 edges
7. `StateTexture` - 66 edges
8. `AppState` - 65 edges
9. `CanvasSpec` - 61 edges
10. `GpuLost` - 59 edges

## Surprising Connections (you probably didn't know these)
- `Cadmium (wet oil painting app)` --references--> `Cadmium App Icon`  [EXTRACTED]
  README.md → crates/cadmium-app/data/app-icons/io.github.marcelogm.Cadmium.svg
- `CadmiumCanvas` --references--> `CanvasHandler`  [INFERRED]
  crates/cadmium-app/src/canvas_widget/imp.rs → crates/cadmium-app/src/canvas_widget.rs
- `every_preset_is_a_valid_palette()` --calls--> `starting_canvas_spec()`  [INFERRED]
  crates/cadmium-app/src/presets/tests.rs → crates/cadmium-app/src/presets.rs
- `starting_painting_is_valid_and_fills_every_slot_with_the_primaries()` --calls--> `starting_canvas_spec()`  [INFERRED]
  crates/cadmium-app/src/presets/tests.rs → crates/cadmium-app/src/presets.rs
- `knife_colour_is_the_mean_of_what_it_carries()` --calls--> `mean_mixture()`  [INFERRED]
  crates/cadmium-app/src/state/dip/tests.rs → crates/cadmium-app/src/state/dip.rs

## Import Cycles
- 2-file cycle: `crates/cadmium-app/src/dmabuf_selftest.rs -> crates/cadmium-app/src/dmabuf_selftest/check.rs -> crates/cadmium-app/src/dmabuf_selftest.rs`
- 2-file cycle: `crates/cadmium-engine/src/document.rs -> crates/cadmium-engine/src/document/export.rs -> crates/cadmium-engine/src/document.rs`
- 2-file cycle: `crates/cadmium-app/src/presenter.rs -> crates/cadmium-app/src/presenter/by_dmabuf.rs -> crates/cadmium-app/src/presenter.rs`
- 2-file cycle: `crates/cadmium-app/src/presenter.rs -> crates/cadmium-app/src/presenter/by_readback.rs -> crates/cadmium-app/src/presenter.rs`
- 2-file cycle: `crates/cadmium-app/src/lab_panel.rs -> crates/cadmium-app/src/lab_panel/snapshot.rs -> crates/cadmium-app/src/lab_panel.rs`
- 2-file cycle: `crates/cadmium-app/src/input.rs -> crates/cadmium-app/src/input/stylus.rs -> crates/cadmium-app/src/input.rs`
- 2-file cycle: `crates/cadmium-app/src/input.rs -> crates/cadmium-app/src/input/navigation.rs -> crates/cadmium-app/src/input.rs`
- 2-file cycle: `crates/cadmium-app/src/state.rs -> crates/cadmium-app/src/state/history.rs -> crates/cadmium-app/src/state.rs`
- 2-file cycle: `crates/cadmium-app/src/state.rs -> crates/cadmium-app/src/state/pen.rs -> crates/cadmium-app/src/state.rs`
- 2-file cycle: `crates/cadmium-engine/src/session.rs -> crates/cadmium-engine/src/session/hold.rs -> crates/cadmium-engine/src/session.rs`
- 2-file cycle: `crates/cadmium-engine/src/history.rs -> crates/cadmium-engine/src/history/atlas.rs -> crates/cadmium-engine/src/history.rs`
- 2-file cycle: `crates/cadmium-engine/src/engine.rs -> crates/cadmium-engine/src/engine/dab_record.rs -> crates/cadmium-engine/src/engine.rs`
- 2-file cycle: `crates/cadmium-engine/src/engine.rs -> crates/cadmium-engine/src/engine/footprint.rs -> crates/cadmium-engine/src/engine.rs`
- 2-file cycle: `crates/cadmium-engine/src/engine.rs -> crates/cadmium-engine/src/engine/pass.rs -> crates/cadmium-engine/src/engine.rs`
- 2-file cycle: `crates/cadmium-engine/src/engine.rs -> crates/cadmium-engine/src/engine/pass/dip.rs -> crates/cadmium-engine/src/engine.rs`
- 2-file cycle: `crates/cadmium-engine/src/engine.rs -> crates/cadmium-engine/src/engine/pass/dry.rs -> crates/cadmium-engine/src/engine.rs`
- 2-file cycle: `crates/cadmium-engine/src/engine.rs -> crates/cadmium-engine/src/engine/pass/pick.rs -> crates/cadmium-engine/src/engine.rs`
- 2-file cycle: `crates/cadmium-engine/src/engine.rs -> crates/cadmium-engine/src/engine/placement.rs -> crates/cadmium-engine/src/engine.rs`
- 2-file cycle: `crates/cadmium-engine/src/engine.rs -> crates/cadmium-engine/src/engine/readout.rs -> crates/cadmium-engine/src/engine.rs`
- 2-file cycle: `crates/cadmium-engine/src/engine/knife_load.rs -> crates/cadmium-engine/src/engine/readout.rs -> crates/cadmium-engine/src/engine/knife_load.rs`

## Hyperedges (group relationships)
- **Painting Tools (brush, knife, blender, rag)** — readme_brush_tool, readme_palette_knife_tool, readme_blender_tool, readme_rag_tool [EXTRACTED 1.00]
- **Cadmium painting tool icon set (shared 16px symbolic style, #2e3436)** — crates_cadmium_app_data_resources_icons_scalable_actions_cadmium_blender_symbolic, crates_cadmium_app_data_resources_icons_scalable_actions_cadmium_brush_symbolic, crates_cadmium_app_data_resources_icons_scalable_actions_cadmium_knife_symbolic, crates_cadmium_app_data_resources_icons_scalable_actions_cadmium_move_symbolic, crates_cadmium_app_data_resources_icons_scalable_actions_cadmium_rag_symbolic, crates_cadmium_app_data_resources_icons_scalable_actions_cadmium_tube_symbolic [INFERRED 0.85]
- **Physical paint loading flow: palette -> brush reservoir -> canvas** — _github_images_screenshot_pigments_palette, _github_images_screenshot_brush_reservoir, _github_images_screenshot_reload_every_stroke, _github_images_screenshot_canvas_impasto_painting [INFERRED 0.85]

## Communities (186 total, 35 thin omitted)

### Community 0 - "Knife & Spreading Tests"
Cohesion: 0.06
Nodes (64): a_dipped_laid_knife_builds_a_plate_as_thick_as_its_gap_over_bare_canvas(), a_full_dip_lays_on_a_clean_knife_the_volume_the_engine_predicts(), a_full_knife_dip_lays_a_plate_most_of_a_canvas_width_long(), a_knife_plate_ends_at_its_sides_in_a_sharp_edge(), a_knife_tap_without_motion_leaves_the_canvas_untouched(), an_undipped_knife_lays_none_of_the_brush_load_on_bare_canvas(), an_upright_knife_ploughs_a_furrow_and_leaves_ridges_taller_than_the_paint_at_both_sides(), band_volume() (+56 more)

### Community 1 - "Swatch Drawing & Logo Example"
Cohesion: 0.06
Nodes (39): BORDER_ALPHA, BORDER_WIDTH, CHANNEL_MAX, CORNER_RADIUS, RING_GAP, RING_WIDTH, set_source(), Swatch (+31 more)

### Community 2 - "Canvas Widget & Input"
Cohesion: 0.06
Nodes (26): CadmiumCanvas, .NAME, attach(), attach_stylus(), attach_stylus_sharing(), Contact, event_belongs_to_stroke(), begin_pan() (+18 more)

### Community 3 - "GPU Compute Passes"
Cohesion: 0.08
Nodes (15): ApplyPass, bind_at_dab(), BladePass, CAPILLARY_POOL_GROUP_SIZE, CAPILLARY_POOL_GROUPS, CapillaryPass, DabBindGroups, DabPasses (+7 more)

### Community 4 - "Document Save Tests"
Cohesion: 0.08
Nodes (36): Primer, SiennaImprimatura, Tray, WhiteGesso, TRAY_PREFIX, export_ignores_the_tray(), export_png_restores_the_view_mode_and_reshades_it(), BLANK_HEIGHT (+28 more)

### Community 5 - "Pigment Slot Selection"
Cohesion: 0.08
Nodes (34): counter_text(), counter_reads_n_of_eight(), PigmentSlotsUniform, SLOT_UNIFORM_VEC4S, write_bands(), IMPRIMATURA_FRACTION, IMPRIMATURA_PIGMENT_ID, IMPRIMATURA_THICKNESS_UM (+26 more)

### Community 6 - "Engine Tool Definitions"
Cohesion: 0.07
Nodes (34): BLENDER, BRUSH, CONTACT_REST_FOLLOW, DabStage, Apply, Blade, Capillary, Exchange (+26 more)

### Community 7 - "Dip Behaviour Tests"
Cohesion: 0.09
Nodes (42): a_swatch_click_with_the_knife_loads_the_knife_and_leaves_the_brush_alone(), another_pigment_on_the_knife_cleans_it_first_and_ctrl_click_adds_a_little(), BARE_CANVAS_POINT, brush_readout_follows_loading_painting_and_cleaning(), choosing_another_pigment_cleans_the_brush_before_loading_it(), cleaning_the_knife_keeps_the_palette_pile_and_the_brush(), clicking_the_loaded_pigment_again_dips_more_paint_on_top(), ctrl_click_mixes_a_little_of_the_pigment_into_what_the_brush_holds() (+34 more)

### Community 8 - "Frame Presenter Modes"
Cohesion: 0.07
Nodes (22): DEBUG_ENABLED, DEBUG_VARIABLE, FramePresenter, MILLISECONDS_PER_SECOND, choose_mode(), describe_modifier(), mode_reason(), PRESENT_MODE_VARIABLE (+14 more)

### Community 9 - "Cursor Outline & Tray Ring"
Cohesion: 0.08
Nodes (31): append_ring(), CursorOutline, OUTLINE_DARK, OUTLINE_LIGHT, OUTLINE_MIN_RADIUS, OUTLINE_WIDTH, RingStroke, LoadRing (+23 more)

### Community 10 - "Gesture Hold/Contact Tests"
Cohesion: 0.13
Nodes (40): an_eraser_stroke_on_the_tray_puts_the_rag_down_after_the_tray_frame(), tray_frames_report_the_pen_samples_they_drew(), a_contact_is_on_the_surface_it_began_on(), a_hold_counts_as_a_contact_until_the_pen_lifts(), a_hold_is_on_the_surface_it_began_on(), a_stroke_is_not_a_hold_on_any_surface(), touching_the_tray_during_a_painting_stroke_leaves_only_the_tray_contact(), a_full_brush_closes_the_ring_at_once_on_a_clean_tray() (+32 more)

### Community 11 - "Big Brush Engine Tests"
Cohesion: 0.08
Nodes (38): PaintSession, paint_long_stroke(), a_heavily_loaded_wide_brush_keeps_its_width_along_a_light_stroke(), ACROSS_REACH_PX, along(), brightness(), colour_view(), FRAME_EVERY_SAMPLES (+30 more)

### Community 12 - "Smoke Test Config"
Cohesion: 0.09
Nodes (37): DEFAULT_EXIT_AFTER_FRAMES, DIALOG, DMABUF_SELFTEST, EXIT_AFTER_FRAMES, EXPORT, lookup_policy(), OPEN, PAGE (+29 more)

### Community 13 - "Engine Core Constants"
Cohesion: 0.07
Nodes (36): FULL_RESERVOIR_VOLUME_UM, BLADE_CELL_GROUP_SIZE, BLADE_CELLS, BLADE_DELTA_BYTES, BLADE_DELTA_CHANNELS, BLADE_ENDS, BLADE_LOAD_BYTES, BLADE_SPILL_BAND_PX (+28 more)

### Community 14 - "Document Writing"
Cohesion: 0.09
Nodes (24): ABSOLUTE_DRY_HEIGHT_FORMAT, DocumentSnapshot, DRY_RELIEF_NAME, export_png(), FORMAT_VERSION, Manifest, MANIFEST_ENTRY, mark_wet_tiles() (+16 more)

### Community 15 - "Document Read Tests"
Cohesion: 0.13
Nodes (34): tray_spec(), format_reason(), read_entries(), set_first_thickness(), a_dry_relief_that_sinks_below_the_bare_canvas_is_a_format_error(), a_well_formed_format_2_document_reads(), future_format_version_is_unsupported(), MAX_MANIFEST_BYTES (+26 more)

### Community 16 - "GPU Readback Helpers"
Cohesion: 0.11
Nodes (15): Canvas, init_params_buffer(), capture_canvas(), save(), GpuContext, GpuLost, map_and_wait(), read_all_mapped() (+7 more)

### Community 17 - "Tray View Widget"
Cohesion: 0.08
Nodes (10): SharedState, append_tray_image(), TrayView, .NAME, CadmiumWindow, COPY_PRESENTATION_TOAST, HISTORY_DISABLED_TOAST, NOTHING_TO_PICK_TOAST (+2 more)

### Community 18 - "Brush Panel Layout"
Cohesion: 0.09
Nodes (25): CLEAN_EACH_STROKE, hint_label(), loaded_row(), LOADED_SPACING, LOADED_SWATCH_SIZE, MEDIUM_SLIDER_MAX, MIN_PIGMENTS_PER_LINE, PIGMENT_BOX_MARGIN (+17 more)

### Community 19 - "Canvas Spec Validation"
Cohesion: 0.09
Nodes (26): validate_slot(), spec(), cotton_weave_stays_within_its_amplitude(), gesso_swatch_is_near_white_and_imprimatura_is_warm(), new_canvas_clears_wet_dry_top_and_display_textures(), new_canvas_has_no_wet_paint(), new_canvas_starts_fully_dirty_with_no_wet_tiles(), sienna_imprimatura_is_warm_and_darker_than_gesso() (+18 more)

### Community 20 - "Spectral Colour Science"
Cohesion: 0.07
Nodes (32): BAND_OF_SAMPLE, CIE1931_2DEG_CMF, composite_over(), compress_gamut(), D65_SPD, D65_WHITE_XYZ, INTERNAL_REFLECTION_K2, km_infinite() (+24 more)

### Community 21 - "Tray Draw-Up Tests"
Cohesion: 0.16
Nodes (34): a_blade_full_at_its_middle_closes_the_loading_ring(), blob(), drawing_up_beside_the_paint_leaves_no_undo_step(), drawing_up_from_a_clean_tray_takes_nothing(), drawing_up_into_a_clean_brush_gives_it_the_default_load_thickness(), drawing_up_into_a_loaded_brush_keeps_its_load_thickness(), drawing_up_moves_paint_from_the_tray_into_the_reservoir(), drawing_up_stops_when_the_reservoir_is_full() (+26 more)

### Community 22 - "App Document State"
Cohesion: 0.11
Nodes (14): AppState, recipe_for_palette(), recipe_follows_its_pigments_into_the_new_palette(), CanvasError, OutOfVideoMemory, CanvasSpec, ExportedImage, history_budget() (+6 more)

### Community 23 - "Stylus Input Samples"
Cohesion: 0.14
Nodes (22): comes_from_a_pen(), event_sample(), is_eraser(), is_primary_contact(), motion_samples(), MOUSE_PRESSURE, TILT_AXIS_TO_DEGREES, tilt_degrees_or() (+14 more)

### Community 24 - "App Tool Definitions"
Cohesion: 0.15
Nodes (21): BLENDER, BRUSH, BRUSH_WORDING, EYEDROPPER, KNIFE, KNIFE_WORDING, MOVE, CLEAN_LABEL (+13 more)

### Community 25 - "Dab Placement Records"
Cohesion: 0.08
Nodes (29): blade_capacity_um(), blade_gap_um(), CAPILLARY_POOL_BUDGET, capillary_scale(), DAB_RECORD_SIZE, DAB_RECORD_STRIDE, DabRecord, FAN_BRISTLE_CONTRAST (+21 more)

### Community 26 - "Paint Tube & Hold"
Cohesion: 0.14
Nodes (25): Hold, BLOB_SETTLE_SECONDS, Dwell, dwell_after(), Nozzle, NOZZLE_APRON_PX, squeeze_region(), SQUEEZE_SNIPPETS (+17 more)

### Community 27 - "Brush Loading Tests"
Cohesion: 0.14
Nodes (29): a_dip_of_nothing_leaves_the_brush_untouched(), assert_close(), BLUE, clean(), DEFAULT_TIP_RADIUS_PX, dip(), dip_adds_its_share_of_a_full_load_to_tip_and_reserve(), dipping_a_second_colour_mixes_with_what_the_brush_holds_by_volume() (+21 more)

### Community 28 - "Stroke Ring Artifact Tests"
Cohesion: 0.08
Nodes (32): write_png(), a_thick_stroke_has_no_dab_spaced_ridges_inside(), ALONG_REACH_PX, along_stroke_high_pass(), ARC_CENTRE, ARC_FROM_RAD, ARC_RADIUS, ARC_SAMPLES (+24 more)

### Community 29 - "Quality CI Workflow"
Cohesion: 0.07
Nodes (32): CADMIUM_SOFTWARE_GPU, LLVM_COV, LLVM_PROFDATA, VK_DRIVER_FILES, jobs, lint, test, container (+24 more)

### Community 30 - "Main Window Shell"
Cohesion: 0.06
Nodes (3): SmokeConfig, CadmiumWindow, .NAME

### Community 31 - "Brush Panel Presentation"
Cohesion: 0.12
Nodes (24): describe_recipe(), panel_tool(), PanelTool, Paint, Tube, pile_is_highlighted(), brush_shows_every_control_and_eyedropper_only_the_hint(), empty_recipe_reads_as_a_clean_brush() (+16 more)

### Community 32 - "New Painting Dialog Layout"
Cohesion: 0.12
Nodes (20): CHIP_SPACING, CHIP_SWATCH_SIZE, CHIPS_PER_LINE, combo_row(), ground_section(), GroundRows, header_bar(), NewPaintingDialog (+12 more)

### Community 33 - "Paint Mixture Readout"
Cohesion: 0.12
Nodes (16): KNIFE_TOP_UP_TOLERANCE, mean_mixture(), EMPTY_MIXTURE, PaintTotals, read_buffer(), BrushOutline, knife_from_load(), accumulate() (+8 more)

### Community 34 - "Compute Bind Layouts"
Cohesion: 0.16
Nodes (12): storage_buffer(), storage_entry(), texture_entry(), uniform_entry(), BLADE_CELL_GROUPS, BLADE_STATE_BYTES, BLADE_STATE_VALUES, dab_record_entry() (+4 more)

### Community 35 - "Draw-Up Pass"
Cohesion: 0.11
Nodes (20): DRAW_UP_INVOCATIONS, draw_up_region(), DRAWN_BYTES, DRAWN_GRANTED_SHARE, DRAWN_PIGMENT, drawn_share(), drawn_totals(), DRAWN_VOLUME (+12 more)

### Community 36 - "Document Reading & Errors"
Cohesion: 0.11
Nodes (18): DocumentError, Format, GpuLost, Io, OutOfVideoMemory, UnsupportedVersion, DocumentSnapshot, entry() (+10 more)

### Community 37 - "Stroke Fading Tests"
Cohesion: 0.09
Nodes (30): a_fading_stroke_has_no_dab_spaced_rings(), ALONG_REACH_PX, BAND_HALF_WIDTH_PX, CONSERVATION_TOLERANCE, FADED_SHARE, FIRST_WINDOW_X, FRAME_EVERY_SAMPLES, FROM_X (+22 more)

### Community 38 - "Canvas Zoom & Frames"
Cohesion: 0.10
Nodes (14): FIT_MARGIN, ZOOM_STEP, OUT_OF_VIDEO_MEMORY, CANNOT_OPEN, APP_ID, LOG_DOMAIN, main(), ACTION_GROUP (+6 more)

### Community 39 - "DMA-BUF Modifier Policy Tests"
Cohesion: 0.09
Nodes (21): is_compressed(), ModifierPolicy, shared_modifiers(), single_plane_storage_modifiers(), DMA_BUF_IOCTL_EXPORT_SYNC_FILE, DMA_BUF_IOCTL_IMPORT_SYNC_FILE, AMD_DCC_MODIFIER, AMD_GFX10_DCC_MODIFIER (+13 more)

### Community 40 - "Canvas State Textures"
Cohesion: 0.08
Nodes (14): create_surface(), StateTexture, .ALL, Display, DryHeight, DryRefl0, DryRefl1, DryTopPig0 (+6 more)

### Community 41 - "Stroke Geometry"
Cohesion: 0.12
Nodes (17): ARC_LENGTH_TOLERANCE_PX, BrushGeometry, is_finite(), lerp_f32(), MAX_DABS_PER_SEGMENT, MAX_TILT_DEG, MIN_DAB_STEP_PX, MIN_SPEED_INTERVAL_MS (+9 more)

### Community 42 - "GPU Adapter Setup"
Cohesion: 0.13
Nodes (16): device_type_preference(), DMABUF_EXPORT_FEATURES, DMABUF_SYNC_EXTENSIONS, GpuError, DeviceCreation, MissingFeature, NoVulkanAdapter, is_usable_vulkan() (+8 more)

### Community 43 - "Session Stroke Encoding"
Cohesion: 0.11
Nodes (4): PaintSession, PaintSession, SAVE_MARGIN_PX, surface_canvas()

### Community 44 - "Smoke Test Steps"
Cohesion: 0.09
Nodes (20): choose_tool(), FileWork, finish_file_work(), paint_stroke(), quit(), SCREENSHOT_RETRY_FRAMES, smoke_stroke(), STROKE_AMPLITUDE (+12 more)

### Community 45 - "Canvas Texel Layout"
Cohesion: 0.09
Nodes (27): BANDS_PER_VEC4, CHANNELS_PER_PIGMENT_TEXTURE, BYTES_PER_FLOAT, BYTES_PER_HALF, decode_texels(), encode_texels(), f32_to_half(), FLOAT_EXPONENT_BIAS (+19 more)

### Community 46 - "Canvas Weave Textures"
Cohesion: 0.09
Nodes (27): Weave, Cotton, Linen, Smooth, cell_jitter(), COTTON_AMPLITUDE_UM, COTTON_PERIOD_PX, LINEN_AMPLITUDE_UM (+19 more)

### Community 47 - "New Painting Dialog Impl"
Cohesion: 0.08
Nodes (8): CONTENT_WIDTH, CreateHandler, NewPaintingDialog, .NAME, PRIMERS, PigmentSelection, WEAVES, Widgets

### Community 48 - "Tray View Geometry"
Cohesion: 0.12
Nodes (20): tray_view_fits_the_whole_tray_in_the_widget(), tray_view(), is_on_tray(), a_short_widget_leaves_room_beside_the_tray(), a_tall_widget_leaves_room_below_the_tray(), a_widget_pixel_covers_one_over_the_screen_zoom_tray_pixels(), only_points_on_the_tray_image_are_on_the_tray(), the_tray_image_fills_the_width_its_height_was_measured_for() (+12 more)

### Community 49 - "Pen Gesture State"
Cohesion: 0.15
Nodes (12): Loading, Squeezing, Stroke, Loading, Contact, Squeezing, AppState, AppState (+4 more)

### Community 50 - "Knife Reservoir Loading"
Cohesion: 0.13
Nodes (13): BLADE_LOAD_VALUES, add_to_blade(), add_to_cell(), blade_cell_cover(), blade_room(), blade_shares(), a_nearly_full_blade_cell_limits_what_the_knife_draws_up(), WetOilEngine (+5 more)

### Community 51 - "Spread Pass Bindings"
Cohesion: 0.12
Nodes (9): dab_record_binding(), DabResources, Scratch, scratch_surface(), ScratchSurface, SpreadBindGroups, spreading_bind_group(), SpreadPass (+1 more)

### Community 52 - "DRM Format Modifiers"
Cohesion: 0.11
Nodes (12): AMD_FMT_MOD_DCC_BIT, candidate_modifiers(), DRM_FORMAT_MOD_LINEAR, DRM_FORMAT_MOD_VENDOR_AMD, DRM_FORMAT_MOD_VENDOR_SHIFT, format_modifiers(), is_exportable_at_size(), vulkan_exportable_storage_modifiers() (+4 more)

### Community 53 - "File Error Reporting"
Cohesion: 0.09
Nodes (20): ALREADY_EXISTS, BACKGROUND_TASK_FAILED, CadmiumWindow, describe_document_error(), describe_io_error(), Failure, FILE_NOT_FOUND, finished() (+12 more)

### Community 55 - "PNG Export & Present"
Cohesion: 0.11
Nodes (14): ALPHA_CHANNEL, OPAQUE_ALPHA, RGBA_CHANNELS, new_encoder(), encode_rgba8_copy(), PRESENT_FORMAT, PRESENT_WORKGROUP_SIZE, Presenter (+6 more)

### Community 56 - "Session Clear & History Budget"
Cohesion: 0.14
Nodes (23): ClearOutcome, Cleared, NothingWet, HISTORY_BUDGET_BYTES, HISTORY_MAX_STEPS, HISTORY_PAINTING_COPIES, HISTORY_TRAY_COPIES, PICK_RADIUS_PX (+15 more)

### Community 57 - "DMA-BUF Pool Core"
Cohesion: 0.10
Nodes (7): DmabufPool, DmabufSlot, HAL_TEXTURE_USES, PRESENT_WGPU_FORMAT, TEXTURE_USAGE, IMAGE_USAGE, PRESENT_VK_FORMAT

### Community 58 - "File Write Feedback"
Cohesion: 0.10
Nodes (13): CANNOT_EXPORT, CANNOT_SAVE, EXPORTED_TOAST, EXPORTING_TOAST, failure_wording(), SAVED_TOAST, SAVING_TOAST, EditCounter (+5 more)

### Community 59 - "Painting Presets & Formats"
Cohesion: 0.10
Nodes (15): starting_painting_size_is_recognised_as_landscape(), applying_a_preset_replaces_the_selection(), dropping_a_pigment_frees_a_slot(), every_preset_yields_a_valid_canvas_spec(), ninth_pigment_is_refused(), rechoosing_a_pigment_in_a_full_selection_is_accepted(), EARTHS, PIGMENT_PRESETS (+7 more)

### Community 60 - "Brush Shapes & Footprint"
Cohesion: 0.10
Nodes (21): BrushShape, Fan, Flat, Round, BLADE_COVERAGE_MARGIN_PX, bristle_spread(), blade_orientation(), FAN_THICKNESS_SHARE (+13 more)

### Community 61 - "Canvas Renderer"
Cohesion: 0.10
Nodes (17): BANDS_PER_VEC4, Renderer, SHADE_INPUTS, shade_params_buffer(), SHADE_SOURCE, shade_tile_buffer(), SHADE_WORKGROUP_SIZE, ShadeParams (+9 more)

### Community 62 - "Brush Panel Piles"
Cohesion: 0.12
Nodes (11): add_chosen_pile(), ADD_LIST_ROW_MARGIN, ADD_LIST_SPACING, ADD_LIST_SWATCH_SIZE, addable_row(), BrushPanel, click_pile_with_current_modifiers(), current_modifiers() (+3 more)

### Community 63 - "File Dialogs"
Cohesion: 0.17
Nodes (17): CadmiumWindow, chosen_write_target(), export_chosen_image(), EXPORT_TITLE, file_dialog(), file_filter(), image_filter(), IMAGE_FILTER_NAME (+9 more)

### Community 64 - "Window Document Actions"
Cohesion: 0.16
Nodes (3): CadmiumWindow, CadmiumWindow, CadmiumWindow

### Community 65 - "Lab Panel Sliders"
Cohesion: 0.13
Nodes (16): param_groups(), ParamControl, decimals_for_step(), format_param(), is_whole_number_param(), PARAM_PAGES, param_row(), param_step() (+8 more)

### Community 66 - "App State & Stroke Start"
Cohesion: 0.11
Nodes (17): STARTING_LOAD, AppState, stroke_preparation(), brush_and_knife_reload_or_clean_at_pen_down_and_other_tools_do_not(), DryOutcome, Dried, NothingWet, reserve_volume() (+9 more)

### Community 67 - "Tool Keypad Bindings"
Cohesion: 0.15
Nodes (14): bound_keys(), key_count(), KEYPAD_TOOL_KEYS, KeyRow, Keypad, Main, keys_on(), TOOL_KEYS (+6 more)

### Community 68 - "History Tiles"
Cohesion: 0.21
Nodes (9): Canvas, TileCoord, mark_dried(), SavedTile, forget_wet(), mark_restored(), mark_wet(), tile_extent() (+1 more)

### Community 69 - "DMA-BUF Pool Textures"
Cohesion: 0.14
Nodes (4): DmabufPool, DmabufSlot, ensure_supported(), hal_queue()

### Community 70 - "Lab Panel Layout"
Cohesion: 0.12
Nodes (15): lab_swatch(), pen_group(), PIGMENT_SWATCH_SIZE, PIGMENT_SWATCH_SPACING, pigments_group(), PROVISIONAL_NOTICE, provisional_notice(), reset_button() (+7 more)

### Community 71 - "Pick Pass Bindings"
Cohesion: 0.12
Nodes (11): buffer_binding(), texture_binding(), MAX_PICK_RADIUS_PX, MIN_PICK_RADIUS_PX, pick_record(), PICKED_BYTES, PICKED_FRACTIONS, PICKED_PAINT_FOUND (+3 more)

### Community 72 - "Application & Accelerators"
Cohesion: 0.17
Nodes (16): ACCELERATORS, activate(), active_window_shows_a_dialog(), CadmiumApplication, close_every_window(), ICON_RESOURCE_PATH, MISSING_GPU_BODY, MISSING_GPU_HEADING (+8 more)

### Community 73 - "Dirty Tile Tracking"
Cohesion: 0.13
Nodes (8): DirtyTiles, tile_grid(), tile_span(), tiles_in_rect(), point_rect_marks_its_tile(), rect_ending_inside_the_last_pixel_of_a_tile_stays_in_that_tile(), rect_ending_on_a_tile_boundary_marks_the_next_tile(), rect_starting_past_the_last_pixel_marks_nothing()

### Community 74 - "Engine Finish Tests"
Cohesion: 0.17
Nodes (20): a_brush_loaded_with_the_picked_mixture_paints_the_picked_colour(), a_pick_off_the_canvas_finds_nothing_even_right_after_a_pick_that_found_paint(), a_queued_stroke_tail_is_painted_with_the_tool_it_was_drawn_with(), assert_brush_untouched_by_pick(), CANVAS_WIDTH, dabs_with_non_finite_geometry_are_skipped(), dry(), drying_a_large_canvas_in_chunks_dries_every_wet_tile() (+12 more)

### Community 75 - "Stroke Spacing Tests"
Cohesion: 0.20
Nodes (21): a_wide_brush_keeps_its_spacing(), a_wide_knife_steps_finely_so_its_oblique_ends_stay_straight(), dab_carries_segment_direction_and_speed(), dab_pressure_follows_the_pressure_curve(), dabs_are_spaced_by_at_least_one_pixel_along_the_arc(), dabs_never_land_behind_the_segment_start_when_pressure_drops(), default_geometry(), diameter_spans_min_ratio_to_max_through_the_curve() (+13 more)

### Community 76 - "About Dialog & Shortcuts"
Cohesion: 0.10
Nodes (18): about_dialog(), action_shortcuts(), ACTION_SHORTCUTS, APPLICATION_ICON, CANCEL_RESPONSE, CLOSE_RESPONSE, DISCARD_PAINTING_HEADING, DISCARD_RESPONSE (+10 more)

### Community 77 - "Contact Surface State"
Cohesion: 0.16
Nodes (5): AppState, AppState, Surface, Painting, Tray

### Community 78 - "Brush Reservoir Tests"
Cohesion: 0.11
Nodes (21): a_full_load_with_the_default_reserve_still_paints_440_px_into_the_stroke(), a_low_load_with_the_default_reserve_runs_dry_within_440_px(), blue_along_a_long_stroke(), blue_along_the_row(), BLUE_SLOT, DEFAULT_RESERVE_RUN_PX, FULL_LOAD, HEIGHT (+13 more)

### Community 79 - "Brush Shape Tests"
Cohesion: 0.15
Nodes (21): a_fan_brush_outline_is_thinner_than_a_flat_one(), a_flat_brush_outline_is_a_horizontal_bar_when_the_pen_is_upright(), a_flat_brush_paints_broadside_wide_and_edgewise_thin(), a_flat_brush_turns_its_thin_side_towards_the_lean_of_the_pen(), a_round_brush_outline_is_a_circle_when_the_pen_is_upright(), ANGLE_TOLERANCE_RAD, axial_difference(), CENTRE (+13 more)

### Community 80 - "Brush Panel Wording"
Cohesion: 0.13
Nodes (18): EYEDROPPER_HINT, MAX_SIZE_PX, MIN_LISTED_SHARE, MIN_SIZE_PX, PERCENT, percent_text(), PLATE_GAP_KEY, plate_gap_spec() (+10 more)

### Community 81 - "DMA-BUF Sync Fences"
Cohesion: 0.16
Nodes (12): dma_buf_ioctl(), DMA_BUF_IOCTL_TYPE, DMA_BUF_SYNC_WRITE, DmabufPool, DmaBufSyncFile, export_sync_file(), import_sync_file(), IOC_DIRECTION_SHIFT (+4 more)

### Community 82 - "Brush Dip & Clean State"
Cohesion: 0.21
Nodes (4): AppState, knife_top_up(), knife_tops_up_only_the_share_it_is_missing(), AppState

### Community 84 - "Brush Settings & Dab Upload"
Cohesion: 0.19
Nodes (9): BrushSettings, upload_dab_records(), DabBatch, DabStep, heading_known(), is_finite(), mark_dab_regions(), WetOilEngine (+1 more)

### Community 85 - "Compute Stage Pipeline"
Cohesion: 0.15
Nodes (5): AutoLayoutStage, compute_pipeline(), ComputeStage, shader_module(), BladePipelines

### Community 86 - "Flatpak Manifest"
Cohesion: 0.10
Nodes (20): build-options, append-path, env, command, CARGO_HOME, finish-args, id, modules (+12 more)

### Community 87 - "History Atlas"
Cohesion: 0.15
Nodes (8): Atlas, bytes_per_tile_all_textures(), halved_capacity(), ATLAS_SLOTS, slots_for_budget(), ATLAS_TILES_PER_SIDE, History, MIN_SLOTS_AFTER_OUT_OF_MEMORY

### Community 89 - "DMA-BUF Vulkan Export"
Cohesion: 0.24
Nodes (4): ExportedImage, SlotExporter, vulkan_error(), Image

### Community 90 - "DMA-BUF Selftest Pattern"
Cohesion: 0.15
Nodes (10): DmabufSelftest, bind_group_layout(), BUSY_ITERATIONS, compute_pipeline(), PATTERN_SHADER, PatternPass, WORKGROUP_SIZE, Phase (+2 more)

### Community 91 - "View Zoom & Pan"
Cohesion: 0.23
Nodes (17): clamp_zoom(), fit_view(), MAX_ZOOM, MIN_ZOOM, pan(), close(), fitted_canvas_is_centered_with_the_margin_on_the_tight_axis(), panning_moves_the_canvas_with_the_pointer() (+9 more)

### Community 92 - "Session Brush Ops"
Cohesion: 0.19
Nodes (3): DrawnUp, PaintSession, PaintSession

### Community 93 - "Release Workflow Steps"
Cohesion: 0.11
Nodes (19): with, arch, artifact-name, body_path, bundle, fetch-depth, fetch-tags, files (+11 more)

### Community 94 - "DMA-BUF Selftest Check"
Cohesion: 0.18
Nodes (11): CHANNEL_TOLERANCE, check_pixels(), DMABUF_LINK_PREFIX, download(), pixel_error(), Pixels, render_through_gsk(), SYNC_FILE_LINK (+3 more)

### Community 95 - "Hold Fill Levels"
Cohesion: 0.17
Nodes (4): fill_level(), reserve_level_is_a_share_of_the_full_reserve(), AppState, held_on()

### Community 96 - "Pigment Dip Recipes"
Cohesion: 0.18
Nodes (16): is_only_pigment(), NEGLIGIBLE_PICKED_FRACTION, PARTIAL_DIP, PigmentDip, AddOnTop, CleanAndLoad, DipAgain, plan_pigment_dip() (+8 more)

### Community 97 - "Engine Brush Tests"
Cohesion: 0.20
Nodes (17): a_dab_marks_its_tiles_wet_and_dirty_including_the_apron(), a_finished_frame_leaves_no_pending_work(), a_stroke_deposits_paint_on_the_canvas(), BROAD_BRUSH_DIAMETER_PX, brush_and_canvas_together_conserve_volume_and_each_pigment(), canvas_never_gets_negative_volume(), clean_brush_empties_tip_and_body(), DEFAULT_SPACING (+9 more)

### Community 98 - "History Tests"
Cohesion: 0.24
Nodes (17): a_budget_below_one_tile_disables_history(), a_small_canvas_keeps_a_few_copies_of_itself_and_one_of_the_tray_for_undo(), a_step_larger_than_the_budget_is_dropped(), a_step_too_large_for_the_history_clears_earlier_steps(), after_an_unrecorded_step_the_full_capacity_is_free_again(), an_action_that_saves_no_tiles_keeps_redo(), assert_a_blank_canvas_with_an_empty_history(), clearing_the_history_forgets_undo_and_redo() (+9 more)

### Community 99 - "Canvas Widget Handlers"
Cohesion: 0.15
Nodes (5): CANVAS_CURSOR, CanvasHandler, PickHandler, notify_scale_change(), SurfaceScaleWatch

### Community 100 - "DMA-BUF Fallback Stub"
Cohesion: 0.14
Nodes (7): DmabufError, Unsupported, DmabufPool, ModifierPolicy, Any, LinearOnly, NoCompression

### Community 101 - "Lab Param Presentation"
Cohesion: 0.12
Nodes (15): PARAM_GROUPS, VIEW_COLOR, VIEW_HEIGHT, VIEW_PIGMENT, VIEW_TOGGLES, VIEW_WETNESS, PARAM_COUNT, ParamGroup (+7 more)

### Community 102 - "Paint Engine & Params"
Cohesion: 0.15
Nodes (5): PaintEngine, DryPass, WetOilEngine, index_of(), SimParams

### Community 103 - "README & App Icons"
Cohesion: 0.13
Nodes (13): Cadmium App Icon, Cadmium Symbolic Icon, Wurtzite Lattice of Cadmium Sulfide (icon motif), Build Requirements (GTK 4.22+, libadwaita 1.9+, Rust 1.92+), Cadmium (wet oil painting app), Canvas Types (linen, cotton, smooth), Lab Panel (simulation inspector), MIT License (+5 more)

### Community 105 - "Brush Panel Signals"
Cohesion: 0.28
Nodes (3): BrushPanel, refresh_if_alive(), with_panel()

### Community 106 - "DMA-BUF Selftest Run"
Cohesion: 0.23
Nodes (5): DmabufSelftest, SelftestProgress, Failed, Passed, Running

### Community 107 - "Lab View Modes"
Cohesion: 0.17
Nodes (12): every_view_mode_round_trips_through_its_toggle_name(), view_from_name(), view_name(), slot_infos(), SlotInfo, Snapshot, TINT_WHITE_PARTS, ViewMode (+4 more)

### Community 108 - "DMA-BUF Presenting"
Cohesion: 0.22
Nodes (3): FramePresenter, RESIZE_SETTLE, ResizeDebounce

### Community 109 - "Dip Pass"
Cohesion: 0.18
Nodes (6): WORKGROUP_SIZE, Buffers, DipPass, TIP_GROUPS, DipRecord, EYEDROPPER_SNIPPETS

### Community 110 - "History Slot Copies"
Cohesion: 0.21
Nodes (4): History, texel_copy(), TileCopy, Slot

### Community 111 - "Pigment Mixture Colour"
Cohesion: 0.15
Nodes (6): mixture_colour(), mixture_srgb8(), empty_mixture_swatch_is_black(), pure_mixture_swatch_matches_the_pigment_masstone(), ultramarine_and_cadmium_yellow_mix_to_green(), ultramarine_and_cadmium_yellow_swatch_is_green()

### Community 113 - "Lab Panel Widget"
Cohesion: 0.18
Nodes (3): LabPanel, dropped_swatches_leave_no_accent_colour_handler_behind(), style_manager_has_accent_colour_handler()

### Community 115 - "Input Sample Tests"
Cohesion: 0.23
Nodes (10): axis_index(), history_sample(), pressure_or(), axes_with(), AXIS_COUNT, backlog_entries_are_widget_points_mapped_like_the_motion_event(), backlog_entry_reads_pressure_and_tilt_in_degrees(), backlog_entry_without_pen_axes_keeps_the_current_pressure_and_tilt() (+2 more)

### Community 117 - "Document State Tests"
Cohesion: 0.21
Nodes (11): a_new_painting_starts_with_a_clean_tray(), opened_painting_keeps_the_brush_and_lab_and_remembers_its_file(), the_tube_falls_back_to_the_first_pigment_on_a_smaller_palette(), load_and_medium_wait_for_the_next_dip(), tool_chosen_mid_stroke_waits_for_the_stroke_to_end(), drying_a_canvas_without_wet_paint_does_nothing(), undo_lifts_a_hold_that_is_still_down(), PAINT_ROW_Y (+3 more)

### Community 121 - "Pen & Pool Resize Tests"
Cohesion: 0.18
Nodes (4): clock_going_backwards_does_not_spike_the_rate(), pen_rate_follows_the_sample_interval(), sample_timed(), samples_with_the_same_timestamp_leave_the_rate_alone()

### Community 122 - "Smoke Step Model"
Cohesion: 0.14
Nodes (12): SmokeConfig, SmokeStep, ChooseTool, Export, Open, OpenDialog, PaintStroke, Quit (+4 more)

### Community 124 - "Rag & Dry Tests"
Cohesion: 0.19
Nodes (14): stroke(), rag_keeps_the_wet_mixture_proportions(), rag_only_removes_paint(), a_dry_too_large_for_the_history_clears_earlier_steps_and_is_not_undoable(), a_new_stroke_after_undo_clears_redo(), dry_is_one_undoable_step(), redo_reapplies_the_stroke(), redoing_a_dry_leaves_its_tiles_dry() (+6 more)

### Community 125 - "Tray Cord Tests"
Cohesion: 0.44
Nodes (12): a_cord_starts_and_ends_round(), a_fast_and_a_slow_tube_raise_the_same_cord(), a_harder_squeeze_raises_a_taller_cord(), a_moving_tube_lays_an_even_cord_without_beads_at_the_frame_joints(), a_tube_crossing_another_pigment_lays_its_own_colour_there(), a_tube_crossing_its_own_cord_does_not_double_it(), a_very_slow_drag_lays_the_same_even_cord_as_a_brisk_one(), drag_tube() (+4 more)

### Community 126 - "Release Workflow Root"
Cohesion: 0.15
Nodes (13): jobs, release, name, on, push, workflow_dispatch, permissions, contents (+5 more)

### Community 127 - "Close & Unsaved Prompt"
Cohesion: 0.32
Nodes (7): unsaved_changes_discard_confirmed(), abandon_writes_and_close(), CadmiumWindow, confirm_close(), forget_write_wait_alert(), offer_to_quit_now(), WRITE_HANG_SECONDS

### Community 128 - "Hold Gesture Thresholds"
Cohesion: 0.15
Nodes (10): FULL_LEVEL, LOAD_CANCEL_SCREEN_PX, load_cancelled(), MAX_HOLD_STEP_SECONDS, STILL_TUBE_SCREEN_PX, load_is_cancelled_past_the_threshold(), StrokePreparation, CleanAndLoad (+2 more)

### Community 129 - "Swatch Widget Impl"
Cohesion: 0.15
Nodes (3): draw_swatch(), Swatch, .NAME

### Community 130 - "Release Build Matrix"
Cohesion: 0.15
Nodes (13): container, needs, runs-on, strategy, strategy, arch, cc, msystem (+5 more)

### Community 131 - "App Screenshot UI"
Cohesion: 0.21
Nodes (11): Cadmium App Screenshot, Brush / Lab Sidebar Tabs, Brush Settings Panel (Size, Load, Medium, Shape), Brush Paint Reservoir (On the Brush), Canvas with Impasto Paint Strokes, Clean Every Stroke, Pigments Palette, Reload Every Stroke (+3 more)

### Community 132 - "Painting Write Flow"
Cohesion: 0.38
Nodes (4): CadmiumWindow, write_image(), write_in_background(), write_painting()

### Community 133 - "Painting Format Model"
Cohesion: 0.20
Nodes (7): PaintingFormat, .ALL, Custom, Landscape16x9, Landscape4x3, Portrait3x4, Square

### Community 134 - "Smoke Runner"
Cohesion: 0.30
Nodes (4): run_dmabuf_selftest(), SETTLE_FRAMES_AFTER_FILE_WORK, SmokeConfig, SmokeRun

### Community 138 - "Selftest Run Limits"
Cohesion: 0.20
Nodes (7): check_handed_out(), EXHAUST_POOL_EVERY, FD_SETTLE_TICKS, HELD_TEXTURES, MAX_CONSECUTIVE_STALLS, MAX_STEADY_FD_GROWTH, present_unless_busy()

### Community 139 - "Lab Presentation Tests"
Cohesion: 0.24
Nodes (5): pen_texts(), pen_readout_reads_as_pressure_tilt_samples_and_rate(), bristle_count_moves_in_whole_bristles(), param_values_show_as_many_decimals_as_their_slider_step(), spec_named()

### Community 140 - "Lab State"
Cohesion: 0.27
Nodes (5): AppState, clamp_view_mode(), is_render_param(), only_render_params_repaint_the_whole_canvas(), pigment_view_beyond_the_palette_shows_the_last_slot()

### Community 142 - "Render Tests"
Cohesion: 0.42
Nodes (7): area_outside_the_canvas_is_background(), blank(), blank_white_gesso_presents_as_titanium_white_masstone(), impasto_edge_facing_the_light_is_brighter_than_the_edge_in_shadow(), shade_and_present(), shading_consumes_dirty_tiles(), wet_ultramarine_over_white_looks_blue()

### Community 144 - "Release Shell Steps"
Cohesion: 0.38
Nodes (10): steps, steps, id, name, run, shell, uses, steps (+2 more)

### Community 145 - "Window Action Table"
Cohesion: 0.33
Nodes (6): ACTIONS, Activate, DRIED_TOAST, simple_action(), tool_action(), UNDO_LABEL

### Community 146 - "Brush Panel Snapshot"
Cohesion: 0.39
Nodes (5): addable_pigments(), held_paint(), pigment_tooltips(), PigmentChoice, Snapshot

### Community 147 - "Accelerator Tests"
Cohesion: 0.31
Nodes (7): action_accelerator(), every_listed_action_has_an_accelerator(), every_tool_key_is_listed(), rag_lists_both_of_its_keys(), the_tube_answers_to_six(), zoom_in_lists_both_of_its_accelerators(), tool_accelerator()

### Community 148 - "Alert Dialogs"
Cohesion: 0.39
Nodes (6): announce_gpu_lost(), ask_to_quit_while_writing(), confirm_discard(), discard_alert(), overwrite_confirmed(), painting_discard_confirmed()

### Community 149 - "DMA-BUF Errors"
Cohesion: 0.25
Nodes (6): DmabufError, Gdk, Io, Poisoned, Unsupported, Vulkan

### Community 150 - "Window Title"
Cohesion: 0.33
Nodes (6): APPLICATION_NAME, document_name(), saved_painting_titles_the_window_with_its_name(), untitled_painting_is_named_sem_titulo(), UNTITLED, window_title()

### Community 154 - "AppImage Builder"
Cohesion: 0.22
Nodes (8): DEPLOY_VULKAN, DESKTOP, GTK_FIX_NONSENSE, ICON, OUTNAME, OUTPATH, make-appimage.sh script, STARTUPWMCLASS

### Community 155 - "Screenshot Errors"
Cohesion: 0.25
Nodes (5): save_screenshot(), ScreenshotError, NoRenderer, NotRenderedYet, Save

### Community 156 - "Overwrite Prompt Tests"
Cohesion: 0.46
Nodes (7): overwrite_needs_confirmation(), appended_extension_landing_on_an_existing_file_asks_before_replacing_it(), appended_extension_on_a_new_name_writes_without_asking(), chosen_name_with_the_extension_in_any_case_is_kept(), chosen_name_without_the_extension_gets_it(), name_already_confirmed_by_the_file_chooser_is_not_asked_again(), with_extension()

### Community 157 - "Lab Panel Signals"
Cohesion: 0.39
Nodes (3): LabPanel, show_pen_if_alive(), with_panel()

### Community 159 - "Preset Tests"
Cohesion: 0.39
Nodes (7): starting_recipe(), an_all_white_palette_starts_with_the_first_white(), every_preset_is_a_valid_palette(), first_brush_carries_ultramarine_when_the_palette_has_it(), first_brush_skips_the_whites_when_there_is_no_ultramarine(), slots(), starting_painting_is_valid_and_fills_every_slot_with_the_primaries()

### Community 160 - "Smoke Dialogs"
Cohesion: 0.29
Nodes (4): SmokeDialog, About, NewPainting, Shortcuts

### Community 161 - "Brush Defaults"
Cohesion: 0.25
Nodes (7): DEFAULT_LOAD, DEFAULT_MAX_DIAMETER_PX, DEFAULT_MEDIUM, DEFAULT_MIN_DIAMETER_RATIO, DEFAULT_SPACING, KNIFE_MAX_STEP_PX, MAX_MEDIUM

### Community 163 - "Release Container Jobs"
Cohesion: 0.29
Nodes (7): image, options, container, needs, runs-on, steps, flatpak

### Community 164 - "Windows Release Job"
Cohesion: 0.29
Nodes (7): run, windows, shell, defaults, needs, runs-on, strategy

### Community 166 - "Smoke Env Lookup"
Cohesion: 0.47
Nodes (3): lookup_flag(), lookup_path(), SmokeConfig

### Community 167 - "Pigment Click Actions"
Cohesion: 0.40
Nodes (4): PigmentClick, Choose, CleanAndLoad, Mix

### Community 169 - "README Painting Tools"
Cohesion: 0.40
Nodes (4): Brush Tool, Palette Knife Tool, Pen Pressure and Tilt Input, Rag Tool

### Community 170 - "Tool Icon Set"
Cohesion: 0.50
Nodes (4): Blender Tool Icon (cadmium-blender-symbolic), Brush Tool Icon (cadmium-brush-symbolic), Palette Knife Tool Icon (cadmium-knife-symbolic), Rag Tool Icon (cadmium-rag-symbolic)

### Community 171 - "Modifier Policy Enum"
Cohesion: 0.50
Nodes (4): ModifierPolicy, Any, LinearOnly, NoCompression

### Community 173 - "Release Version Job"
Cohesion: 0.50
Nodes (4): version, version, outputs, runs-on

### Community 174 - "Windows Installer Job"
Cohesion: 0.50
Nodes (4): windows-installer, needs, runs-on, strategy

## Knowledge Gaps
- **163 isolated node(s):** `cadmium-app`, `Tube`, `Any`, `NoCompression`, `LinearOnly` (+158 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 1273 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **35 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `GpuContext` connect `GPU Readback Helpers` to `Knife & Spreading Tests`, `Swatch Drawing & Logo Example`, `GPU Compute Passes`, `Document Save Tests`, `Pigment Slot Selection`, `Dip Behaviour Tests`, `Frame Presenter Modes`, `Selftest Run Limits`, `Big Brush Engine Tests`, `Engine Core Constants`, `Document Writing`, `Document Read Tests`, `Render Tests`, `Canvas Spec Validation`, `Tray Draw-Up Tests`, `App Document State`, `Dab Placement Records`, `Paint Tube & Hold`, `Brush Loading Tests`, `Stroke Ring Artifact Tests`, `Readback Presenting`, `Paint Mixture Readout`, `Compute Bind Layouts`, `Draw-Up Pass`, `Stroke Fading Tests`, `Canvas State Textures`, `GPU Adapter Setup`, `Session Stroke Encoding`, `Selftest Uniform Encode`, `Canvas Texel Layout`, `Knife Reservoir Loading`, `Spread Pass Bindings`, `DRM Format Modifiers`, `PNG Export & Present`, `Session Clear & History Budget`, `DMA-BUF Pool Core`, `Canvas Renderer`, `App State & Stroke Start`, `History Tiles`, `DMA-BUF Pool Textures`, `Pick Pass Bindings`, `Engine Finish Tests`, `Brush Reservoir Tests`, `Brush Shape Tests`, `Brush Settings & Dab Upload`, `Compute Stage Pipeline`, `History Atlas`, `DMA-BUF Vulkan Export`, `DMA-BUF Selftest Pattern`, `Session Brush Ops`, `Engine Brush Tests`, `History Tests`, `DMA-BUF Fallback Stub`, `DMA-BUF Presenting`, `Dip Pass`, `History Undo Steps`, `Rag & Dry Tests`, `Tray Cord Tests`?**
  _High betweenness centrality (0.285) - this node is a cross-community bridge._
- **Are the 7 inferred relationships involving `Canvas` (e.g. with `canvas/slots.rs` and `read_slot_table()`) actually correct?**
  _`Canvas` has 7 INFERRED edges - model-reasoned connections that need verification._
- **What connects `cadmium-app`, `Tube`, `Any` to the rest of the system?**
  _163 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Knife & Spreading Tests` be split into smaller, more focused modules?**
  _Cohesion score 0.05835010060362173 - nodes in this community are weakly interconnected._
- **Why does `AppState` connect `App State & Stroke Start` to `Hold Gesture Thresholds`, `Dip Behaviour Tests`, `Frame Presenter Modes`, `Canvas Frame Loop`, `Cursor Outline & Tray Ring`, `Big Brush Engine Tests`, `Gesture Hold/Contact Tests`, `Lab State`, `Tray View Frames`, `GPU Readback Helpers`, `Brush Panel Snapshot`, `App Document State`, `Readback Presenting`, `Brush Panel Presentation`, `Paint Mixture Readout`, `Canvas Zoom & Frames`, `Tray View Geometry`, `Pen Gesture State`, `File Write Feedback`, `Tool Keypad Bindings`, `Application & Accelerators`, `Contact Surface State`, `View Zoom & Pan`, `Brush Panel Signals`, `Lab View Modes`, `DMA-BUF Presenting`, `Hand & History State`, `Document State Tests`?**
  _High betweenness centrality (0.090) - this node is a cross-community bridge._
- **Are the 61 inferred relationships involving `painting_state()` (e.g. with `brush_readout_follows_loading_painting_and_cleaning()` and `choosing_another_pigment_cleans_the_brush_before_loading_it()`) actually correct?**
  _`painting_state()` has 61 INFERRED edges - model-reasoned connections that need verification._
- **Should `Swatch Drawing & Logo Example` be split into smaller, more focused modules?**
  _Cohesion score 0.056261343012704176 - nodes in this community are weakly interconnected._