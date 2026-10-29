# Component documentation

_78 component(s), regenerated from the registry._

## component

### Badge/StatusCount

Component of Studio's chrome, specified in spec C.
tier=component; rust=kit/badge.rs::count_badge; rust_status=new-in-C; today=home/mod.rs:1111-1113; paints=BADGE_NEUTRAL_BORDER,BADGE_NEUTRAL_BG,BADGE_NEUTRAL_TEXT,BG_SUCCESS_PRIMARY,BADGE_SUCCESS_BORDER,BADGE_SUCCESS_TEXT,BG_WARNING_PRIMARY,BADGE_WARNING_BORDER,BADGE_WARNING_TEXT,BG_ERROR_PRIMARY,BADGE_ERROR_BORDER,BADGE_ERROR_TEXT; pending=BADGE_NEUTRAL_BORDER,BADGE_NEUTRAL_BG,BADGE_NEUTRAL_TEXT,BG_SUCCESS_PRIMARY,BADGE_SUCCESS_BORDER,BADGE_SUCCESS_TEXT,BG_WARNING_PRIMARY,BADGE_WARNING_BORDER,BADGE_WARNING_TEXT,BG_ERROR_PRIMARY,BADGE_ERROR_BORDER,BADGE_ERROR_TEXT; spec=C

- **Variants:** Size=Md, Size=Sm, State=Default, Tone=Error, Tone=Neutral, Tone=Success, Tone=Warning
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `radius.radius-pill` → 9999px
  - `typography.font-size-caption` → 12px

### Button/Close

Component of Studio's chrome, specified in spec C.
tier=component; rust=kit/button.rs::button; rust_status=new-in-C; today=widgets/popover.rs:40-43; paints=BG,BG_HOVER,BG_PRESSED,ICON_QUATERNARY,ICON_QUATERNARY_HOVER,FOCUS_RING; pending=ICON_QUATERNARY_HOVER; spec=C

- **Variants:** Size=Sm, Size=Xs, State=Active, State=Default, State=Focus, State=Hover
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.bg` → #0a0a0a
  - `colors.bg-hover` → #171717
  - `colors.neutral` → #737373
  - `colors.ring` → #f97316
  - `colors.secondary-active` → #404040
  - `radius.radius-md` → 8px

### Button/Destructive

Component of Studio's chrome, specified in spec C.
tier=component; rust=kit/button.rs::button; rust_status=new-in-C; today=none; paints=BG_ERROR_SOLID_PRESSED,BG_ERROR_SOLID,BG_ERROR_SOLID_HOVER,FOCUS_RING_ERROR,TEXT_WHITE; pending=BG_ERROR_SOLID_PRESSED,BG_ERROR_SOLID_HOVER,FOCUS_RING_ERROR; spec=C

- **Variants:** Size=Md, Size=Sm, Size=Xs, State=Active, State=Default, State=Disabled, State=Focus, State=Hover
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.destructive` → #dc2626
  - `colors.destructive-foreground` → #ffffff
  - `radius.radius-md` → 8px

### Button/Icon

Component of Studio's chrome, specified in spec C.
tier=component; rust=kit/button.rs::button; rust_status=new-in-C; today=widgets/mod.rs::icon_button; paints=BG,BG_HOVER,BG_PRESSED,BG_SELECTED,ICON_QUATERNARY,ICON_QUATERNARY_HOVER,ICON_SECONDARY_HOVER,FOCUS_RING; pending=ICON_QUATERNARY_HOVER,ICON_SECONDARY_HOVER; spec=C

- **Variants:** Size=Sm, Size=Xs, State=Active, State=Default, State=Disabled, State=Focus, State=Hover, State=Selected
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.bg` → #0a0a0a
  - `colors.bg-active` → #262626
  - `colors.bg-hover` → #171717
  - `colors.neutral` → #737373
  - `colors.ring` → #f97316
  - `colors.secondary-active` → #404040
  - `radius.radius-md` → 8px

### Button/Primary

Component of Studio's chrome, specified in spec C.
tier=component; rust=kit/button.rs::button; rust_status=new-in-C; today=home/dialog.rs:367-386 and four more (C 3.3); paints=TEXT_ON_BRAND,BG_BRAND_PRESSED,BG_BRAND,BG_BRAND_HOVER,FOCUS_RING; spec=C

- **Variants:** Size=Md, Size=Sm, Size=Xs, State=Active, State=Default, State=Disabled, State=Focus, State=Hover
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.primary` → #e36323
  - `colors.primary-active` → #ea580c
  - `colors.primary-foreground` → #0a0a0a
  - `colors.primary-hover` → #f97316
  - `colors.ring` → #f97316
  - `radius.radius-md` → 8px

### Button/Secondary

Component of Studio's chrome, specified in spec C.
tier=component; rust=kit/button.rs::button; rust_status=new-in-C; today=comments/ui.rs:92-108 and three more; paints=BG,BG_HOVER,BG_PRESSED,BORDER_STRONG,ICON_QUATERNARY,ICON_QUATERNARY_HOVER,FOCUS_RING,TEXT_SECONDARY,TEXT_SECONDARY_HOVER; pending=ICON_QUATERNARY_HOVER,TEXT_SECONDARY,TEXT_SECONDARY_HOVER; spec=C

- **Variants:** Size=Md, Size=Sm, Size=Xs, State=Active, State=Default, State=Disabled, State=Focus, State=Hover
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.bg` → #0a0a0a
  - `colors.bg-hover` → #171717
  - `colors.border-strong` → #404040
  - `colors.neutral` → #737373
  - `colors.ring` → #f97316
  - `colors.secondary-active` → #404040
  - `radius.radius-md` → 8px

### Button/SecondaryDestructive

Component of Studio's chrome, specified in spec C/H.
tier=component; rust=kit/button.rs::button; rust_status=new-in-C; today=home/mod.rs:1239; paints=BG_ERROR_PRIMARY,BG,BORDER_ERROR,BORDER_ERROR_SUBTLE,FOCUS_RING_ERROR,TEXT_ERROR,TEXT_ERROR_HOVER; pending=BG_ERROR_PRIMARY,BORDER_ERROR,BORDER_ERROR_SUBTLE,FOCUS_RING_ERROR,TEXT_ERROR,TEXT_ERROR_HOVER; spec=C/H

- **Variants:** Size=Md, Size=Sm, Size=Xs, State=Active, State=Default, State=Disabled, State=Focus, State=Hover
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.bg` → #0a0a0a
  - `radius.radius-md` → 8px

### Button/Tertiary

Component of Studio's chrome, specified in spec C.
tier=component; rust=kit/button.rs::button; rust_status=new-in-C; today=home/dialog.rs:401-410; paints=BG,BG_HOVER,BG_PRESSED,ICON_QUATERNARY,ICON_QUATERNARY_HOVER,FOCUS_RING,TEXT_TERTIARY,TEXT_TERTIARY_HOVER; pending=ICON_QUATERNARY_HOVER,TEXT_TERTIARY_HOVER; spec=C

- **Variants:** Size=Md, Size=Sm, Size=Xs, State=Active, State=Default, State=Disabled, State=Focus, State=Hover
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.bg` → #0a0a0a
  - `colors.bg-hover` → #171717
  - `colors.muted-foreground` → #a3a3a3
  - `colors.neutral` → #737373
  - `colors.ring` → #f97316
  - `colors.secondary-active` → #404040
  - `radius.radius-md` → 8px

### Button/Tool

Component of Studio's chrome, specified in spec C/D.
tier=component; rust=kit/button.rs::button; rust_status=new-in-C; today=panels/toolbar.rs:209-227; paints=BG,BG_HOVER,BG_PRESSED,BG_SELECTED,ICON,ICON_QUATERNARY_HOVER,ICON_SECONDARY_HOVER,FOCUS_RING; pending=ICON,ICON_QUATERNARY_HOVER,ICON_SECONDARY_HOVER; spec=C/D

- **Variants:** State=Active, State=Default, State=Disabled, State=Focus, State=Hover, State=Selected
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.bg` → #0a0a0a
  - `colors.bg-active` → #262626
  - `colors.bg-hover` → #171717
  - `colors.ring` → #f97316
  - `colors.secondary-active` → #404040
  - `radius.radius-md` → 8px

### Checkbox/Box

Component of Studio's chrome, specified in spec C.
tier=component; rust=kit/choice.rs::checkbox; rust_status=new-in-C; today=widgets/popover.rs::check; paints=TEXT_ON_BRAND,BG_BRAND,BG,BG_TERTIARY,BORDER_STRONG,FOCUS_RING; spec=C

- **Variants:** State=Default, State=Disabled, State=Focus, State=Hover, State=Selected
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.bg` → #0a0a0a
  - `colors.border-strong` → #404040
  - `colors.primary` → #e36323
  - `colors.primary-foreground` → #0a0a0a
  - `colors.ring` → #f97316
  - `colors.secondary` → #262626
  - `radius.radius-xs` → 4px

### Icon/Tile

Component of Studio's chrome, specified in spec C/H.
tier=component; rust=kit/feedback.rs::tile_icon; rust_status=new-in-C; today=home/mod.rs:2563-2577; paints=TILE_DESIGN_BG,TILE_DESIGN_GLYPH,TILE_BOARD_BG,TILE_BOARD_GLYPH,TILE_DECK_BG,TILE_DECK_GLYPH,TILE_SPEC_BG,TILE_SPEC_GLYPH,BORDER_STRONG,ICON_QUATERNARY; pending=TILE_DESIGN_BG,TILE_DESIGN_GLYPH,TILE_BOARD_BG,TILE_BOARD_GLYPH,TILE_DECK_BG,TILE_DECK_GLYPH,TILE_SPEC_GLYPH; spec=C/H

- **Variants:** Kind=Board, Kind=Deck, Kind=Design, Kind=Neutral, Kind=Spec, Size=Md, Size=Sm
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.accent` → #431407
  - `colors.border-strong` → #404040
  - `colors.neutral` → #737373
  - `radius.radius-md` → 8px

### Input/Field

Component of Studio's chrome, specified in spec C.
tier=component; rust=kit/field.rs::paint_field; rust_status=new-in-C; today=widgets/field.rs::field_bg; paints=BG,BORDER_SELECTED,BORDER_STRONG,ICON_QUATERNARY,TEXT,TEXT_PLACEHOLDER; pending=TEXT_PLACEHOLDER; spec=C

- **Variants:** Size=Sm, Size=Xs, State=Default, State=Disabled, State=Focus, State=Hover
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.bg-foreground` → #fafafa
  - `colors.border-focus` → #fb923c
  - `colors.border-strong` → #404040
  - `colors.input` → #0a0a0a
  - `colors.neutral` → #737373
  - `radius.radius-md` → 8px

### Input/PropertyField

Component of Studio's chrome, specified in spec C/F.
tier=component; rust=widgets/field.rs::num_field; rust_status=exists; paints=BG,BORDER_SELECTED,BORDER_STRONG,TEXT,TEXT_TERTIARY; spec=C/F

- **Variants:** Axis=Scrub, Size=Xs, State=Default, State=Disabled, State=Focus, State=Hover
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.bg-foreground` → #fafafa
  - `colors.border-focus` → #fb923c
  - `colors.border-strong` → #404040
  - `colors.input` → #0a0a0a
  - `colors.muted-foreground` → #a3a3a3
  - `radius.radius-md` → 8px
  - `typography.font-family-mono` → Roboto Mono, SFMono-Regular, Menlo, monospace

### Input/Search

Component of Studio's chrome, specified in spec C.
tier=component; rust=kit/field.rs::paint_field; rust_status=new-in-C; today=find/filter.rs::show_field; paints=BG,BORDER_SELECTED,BORDER_STRONG,ICON_QUATERNARY,TEXT_PLACEHOLDER,TEXT; pending=TEXT_PLACEHOLDER; spec=C

- **Variants:** Size=Md, Size=Sm, Size=Xs, State=Default, State=Disabled, State=Focus, State=Hover
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.bg-foreground` → #fafafa
  - `colors.border-focus` → #fb923c
  - `colors.border-strong` → #404040
  - `colors.input` → #0a0a0a
  - `colors.neutral` → #737373
  - `radius.radius-md` → 8px

### List/Row

Component of Studio's chrome, specified in spec C.
tier=component; rust=kit/rows.rs::list_row; rust_status=new-in-C; today=home/mod.rs:1171-1192; paints=BG,BG_HOVER,BG_SELECTED,ICON_QUATERNARY,ICON_QUATERNARY_HOVER,FOCUS_RING,TEXT_SECONDARY,TEXT_SECONDARY_HOVER; pending=ICON_QUATERNARY_HOVER,TEXT_SECONDARY,TEXT_SECONDARY_HOVER; spec=C

- **Variants:** Size=Sm, Size=Xs, State=Default, State=Focus, State=Hover, State=Selected
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.bg` → #0a0a0a
  - `colors.bg-active` → #262626
  - `colors.bg-hover` → #171717
  - `colors.neutral` → #737373
  - `colors.ring` → #f97316
  - `radius.radius-md` → 8px

### Loading/Inline

Component of Studio's chrome, specified in spec C/I.
tier=component; rust=kit/feedback.rs::loading_ring; rust_status=new-in-C; today=image_jobs.rs::layer_row_progress_chip; paints=BG_SECONDARY,BORDER,ICON_BRAND_PRIMARY,TEXT_SECONDARY,BG_TERTIARY; pending=ICON_BRAND_PRIMARY,TEXT_SECONDARY; spec=C/I

- **Variants:** State=Default
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.border` → #262626
  - `colors.secondary` → #262626
  - `colors.surface` → #171717
  - `radius.radius-md` → 8px

### Menu/Item

Component of Studio's chrome, specified in spec C.
tier=component; rust=widgets/menu.rs::popup_tree; rust_status=exists; today=widgets/menu.rs:430-505; paints=BG_HOVER,ICON_BRAND_PRIMARY,ICON_QUATERNARY,ICON_QUATERNARY_HOVER,TEXT_SECONDARY,TEXT_SECONDARY_HOVER; pending=ICON_BRAND_PRIMARY,ICON_QUATERNARY_HOVER,TEXT_SECONDARY,TEXT_SECONDARY_HOVER; spec=C

- **Variants:** Size=Sm, Size=Xs, State=Active, State=Default, State=Disabled, State=Focus, State=Hover, State=Selected
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.bg-hover` → #171717
  - `colors.neutral` → #737373
  - `radius.radius-md` → 8px

### Progress/Bar

Component of Studio's chrome, specified in spec C/K.
tier=component; rust=kit/feedback.rs::progress_bar; rust_status=new-in-C; today=panels/check_designs.rs::progress_row; paints=BG_PRESSED,ICON_BRAND_PRIMARY,TEXT_SECONDARY; pending=ICON_BRAND_PRIMARY,TEXT_SECONDARY; spec=C/K

- **Variants:** State=Default
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.secondary-active` → #404040
  - `radius.radius-pill` → 9999px

### Radio/Card

Component of Studio's chrome, specified in spec C/I.
tier=component; rust=kit/choice.rs::radio_card; rust_status=new-in-C; today=agent_approval_card.rs:256-286; paints=BG_BRAND,BG,BG_HOVER,BORDER_SELECTED,BORDER_STRONG,BORDER,FOCUS_RING,TEXT,TEXT_SECONDARY,TEXT_ON_BRAND; pending=TEXT_SECONDARY; spec=C/I

- **Variants:** State=Default, State=Disabled, State=Focus, State=Hover, State=Selected
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.bg` → #0a0a0a
  - `colors.bg-foreground` → #fafafa
  - `colors.bg-hover` → #171717
  - `colors.border` → #262626
  - `colors.border-focus` → #fb923c
  - `colors.border-strong` → #404040
  - `colors.primary` → #e36323
  - `colors.primary-foreground` → #0a0a0a
  - `colors.ring` → #f97316
  - `radius.radius-xl` → 12px

### Select/Trigger

Component of Studio's chrome, specified in spec C.
tier=component; rust=widgets/menu.rs::dropdown; rust_status=exists; today=widgets/menu.rs:571-606; paints=BG,BORDER_SELECTED,BORDER_STRONG,ICON_QUATERNARY,TEXT; spec=C

- **Variants:** Size=Sm, Size=Xs, State=Default, State=Disabled, State=Focus, State=Hover, State=Open
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.bg-foreground` → #fafafa
  - `colors.border-focus` → #fb923c
  - `colors.border-strong` → #404040
  - `colors.input` → #0a0a0a
  - `colors.neutral` → #737373
  - `radius.radius-md` → 8px

### Slider/Handle

Component of Studio's chrome, specified in spec C.
tier=component; rust=kit/choice.rs::slider_handle; rust_status=new-in-C; paints=BG,BG_PRESSED,ICON_BRAND_PRIMARY,FOCUS_RING; pending=ICON_BRAND_PRIMARY; spec=C

- **Variants:** State=Active, State=Default, State=Disabled, State=Focus, State=Hover
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.bg` → #0a0a0a
  - `colors.ring` → #f97316
  - `colors.secondary-active` → #404040
  - `radius.radius-pill` → 9999px

### Swatch/Color

Component of Studio's chrome, specified in spec C/F.
tier=component; rust=kit/choice.rs::swatch_ring; rust_status=new-in-C; today=widgets/mod.rs::swatch; paints=BORDER_TRANSLUCENT,FOCUS_RING; pending=BORDER_TRANSLUCENT; spec=C/F

- **Variants:** State=Default, State=Focus, State=Hover, State=Selected
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.ring` → #f97316

### Tabs/Pill

Component of Studio's chrome, specified in spec C.
tier=component; rust=kit/tabs.rs::tab; rust_status=new-in-C; today=panels/right.rs:103-127; paints=BG,BG_HOVER,BG_SELECTED,ICON_QUATERNARY,ICON_QUATERNARY_HOVER,FOCUS_RING,TEXT_QUATERNARY,TEXT_SECONDARY; pending=ICON_QUATERNARY_HOVER,TEXT_QUATERNARY,TEXT_SECONDARY; spec=C

- **Variants:** State=Default, State=Disabled, State=Focus, State=Hover, State=Selected
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.bg` → #0a0a0a
  - `colors.bg-active` → #262626
  - `colors.bg-hover` → #171717
  - `colors.neutral` → #737373
  - `colors.ring` → #f97316
  - `radius.radius-md` → 8px

### Tag/Chip

Component of Studio's chrome, specified in spec C.
tier=component; rust=kit/badge.rs::tag; rust_status=new-in-C; today=agent_selection_chip.rs:160-200; paints=BG,BORDER_STRONG,ICON_QUATERNARY,ICON_QUATERNARY_HOVER,FOCUS_RING,TEXT_SECONDARY; pending=ICON_QUATERNARY_HOVER,TEXT_SECONDARY; spec=C

- **Variants:** Size=Sm, State=Default, State=Focus, State=Hover
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.bg` → #0a0a0a
  - `colors.border-strong` → #404040
  - `colors.neutral` → #737373
  - `colors.ring` → #f97316
  - `radius.radius-md` → 8px

### Toggle/Switch

Component of Studio's chrome, specified in spec C.
tier=component; rust=kit/choice.rs::toggle; rust_status=new-in-C; today=panels/components.rs::toggle; paints=BG_BRAND,BG_TERTIARY,BORDER,ICON_WHITE,FOCUS_RING; pending=ICON_WHITE; spec=C

- **Variants:** State=Default, State=Disabled, State=Focus, State=Hover, State=Selected
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.border` → #262626
  - `colors.primary` → #e36323
  - `colors.ring` → #f97316
  - `colors.secondary` → #262626
  - `radius.radius-pill` → 9999px

### Tooltip/Default

Component of Studio's chrome, specified in spec C.
tier=component; rust=theme.rs::apply; rust_status=exists; paints=BG_PRIMARY_SOLID,TEXT,TEXT_SECONDARY; pending=BG_PRIMARY_SOLID,TEXT_SECONDARY; spec=C

- **Variants:** State=Default
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.bg-foreground` → #fafafa
  - `radius.radius-md` → 8px

### Tree/Row

Component of Studio's chrome, specified in spec C/E.
tier=component; rust=kit/rows.rs::tree_row_bg; rust_status=new-in-C; today=panels/layers.rs:516-596; paints=BG_HOVER,BG_SELECTED,ICON_QUATERNARY,TEXT_TERTIARY; spec=C/E

- **Variants:** Size=Xs, State=Default, State=Hover, State=Selected
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.bg-active` → #262626
  - `colors.bg-hover` → #171717
  - `colors.muted-foreground` → #a3a3a3
  - `colors.neutral` → #737373
  - `radius.radius-md` → 8px

## composite

### Banner/Inline

Composite of Studio's chrome, specified in spec J/I/K.
tier=composite; rust=history/banner.rs::show,libraries/review.rs::show_banner; rust_status=exists; paints=BG_SECONDARY,BG_SUCCESS_PRIMARY,BG_WARNING_PRIMARY,BG_ERROR_PRIMARY,TEXT,BORDER; pending=BG_SUCCESS_PRIMARY,BG_WARNING_PRIMARY,BG_ERROR_PRIMARY; spec=J/I/K; composes=Status/Dot,Button/*

- **Variants:** Tone=Error, Tone=Neutral, Tone=Success, Tone=Warning
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.bg-foreground` → #fafafa
  - `colors.border` → #262626
  - `colors.surface` → #171717

### Bar/Contextual

Composite of Studio's chrome, specified in spec D.
tier=composite; rust=panels/toolbar.rs::contextual_bar; rust_status=new-in-D; today=panels/toolbar.rs:160-189; paints=BG_TOOLBAR,INNER_HIGHLIGHT,BORDER; pending=INNER_HIGHLIGHT; spec=D; composes=Button/Icon,Button/Tertiary,Divider/Hairline

- **Variants:** —
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.bg` → #0a0a0a
  - `colors.border` → #262626

### Card/AgentTurn

Composite of Studio's chrome, specified in spec I.
tier=composite; rust=agent_turn_card.rs::show; rust_status=exists; paints=BG_SECONDARY,BORDER; spec=I; composes=Status/Dot,Badge/StatusCount

- **Variants:** —
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.border` → #262626
  - `colors.card` → #171717

### Card/Approval

Composite of Studio's chrome, specified in spec I.
tier=composite; rust=agent_approval_card.rs::paint_approval_card; rust_status=exists; paints=BG_SECONDARY,BORDER; spec=I; composes=Badge/StatusCount,Button/*,Radio/Card

- **Variants:** —
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.border` → #262626
  - `colors.card` → #171717
  - `radius.radius-xl` → 12px

### Card/FileThumbnail

Composite of Studio's chrome, specified in spec H.
tier=composite; rust=home/mod.rs::main_view::card; rust_status=exists; paints=BG_SECONDARY,TEXT,TEXT_TERTIARY,BG_SECONDARY_HOVER,BORDER,BORDER_SELECTED,HOME_THUMB_BG; spec=H; composes=Icon/Tile,Badge/StatusCount

- **Variants:** —
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.border` → #262626
  - `colors.border-focus` → #fb923c
  - `colors.card` → #171717
  - `colors.card-foreground` → #fafafa
  - `colors.muted-foreground` → #a3a3a3
  - `colors.secondary` → #262626
  - `colors.surface-hover` → #262626

### Card/Library

Composite of Studio's chrome, specified in spec K.
tier=composite; rust=panels/assets.rs::library_card; rust_status=new-in-K; today=panels/assets.rs:542-552; paints=BG_SECONDARY,BORDER; spec=K; composes=Tag/Chip,Toggle/Switch

- **Variants:** —
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.border` → #262626
  - `colors.card` → #171717

### Card/StarterTemplate

Composite of Studio's chrome, specified in spec H.
tier=composite; rust=home/mod.rs::main_view::templates_view; rust_status=exists; paints=BG_SECONDARY,TEXT,BORDER; spec=H; composes=Button/Secondary

- **Variants:** —
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.border` → #262626
  - `colors.card` → #171717
  - `colors.card-foreground` → #fafafa

### Code/Block

Composite of Studio's chrome, specified in spec G.
tier=composite; rust=dev/code_view.rs::code_block; rust_status=exists; paints=BG_SECONDARY,BORDER; spec=G

- **Variants:** —
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.border` → #262626
  - `colors.surface` → #171717
  - `typography.font-family-mono` → Roboto Mono, SFMono-Regular, Menlo, monospace

### Dialog/CommandPalette

Composite of Studio's chrome, specified in spec C/J/D.
tier=composite; rust=kit/surface.rs::command_bar_surface; rust_status=new-in-C; today=cmdk.rs::show; panels/menus.rs:907-1085; paints=BG_MENU,BORDER_MENU,ICON_QUATERNARY,TEXT,TEXT_TERTIARY; pending=BORDER_MENU; spec=C/J/D; flow=modal-cmdk; composes=Input/Search,Tabs/Segmented,Menu/Item,Badge/StatusCount,Menu/Shortcut

- **Variants:** Kind=Actions, Kind=CmdK, State=Default
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.muted-foreground` → #a3a3a3
  - `colors.neutral` → #737373
  - `colors.popover` → #0a0a0a
  - `colors.popover-foreground` → #fafafa
  - `radius.radius-xl` → 12px

### Dialog/Modal

Composite of Studio's chrome, specified in spec C/J.
tier=composite; rust=kit/surface.rs::modal_surface; rust_status=new-in-C; today=home/dialog.rs::show; paints=BG,ICON_QUATERNARY,TEXT,BG_OVERLAY,TEXT_TERTIARY,RADIUS_FRAME; pending=BG_OVERLAY,RADIUS_FRAME; spec=C/J; flow=modal-assets-libraries,modal-plugins; composes=Panel/Header,Button/Close,Button/*,Divider/Hairline

- **Variants:** Kind=Libraries, Kind=Plugins, State=Default
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.bg` → #0a0a0a
  - `colors.bg-foreground` → #fafafa
  - `colors.muted-foreground` → #a3a3a3
  - `colors.neutral` → #737373

### EmptyState/Default

Composite of Studio's chrome, specified in spec C.
tier=composite; rust=kit/feedback.rs::empty_state; rust_status=new-in-C; today=home/mod.rs::main_view::empty_state; paints=BORDER_STRONG,ICON_QUATERNARY,TEXT,TEXT_TERTIARY; spec=C; composes=Icon/Tile,Button/Secondary

- **Variants:** State=Default
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.bg-foreground` → #fafafa
  - `colors.border-strong` → #404040
  - `colors.muted-foreground` → #a3a3a3
  - `colors.neutral` → #737373
  - `radius.radius-md` → 8px

### Input/Composer

Composite of Studio's chrome, specified in spec I.
tier=composite; rust=agent_composer.rs::show_composer; rust_status=exists; paints=BG_SECONDARY,BORDER,FOCUS_RING; spec=I; composes=Tag/Chip,Button/Icon,Button/Primary

- **Variants:** —
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.border` → #262626
  - `colors.ring` → #f97316
  - `colors.surface` → #171717

### Menu/Context

Composite of Studio's chrome, specified in spec C/J.
tier=composite; rust=panels/context.rs::show; rust_status=exists; paints=BG_MENU,BORDER_MENU,ICON_QUATERNARY,TEXT_SECONDARY,RADIUS_FRAME; pending=BORDER_MENU,TEXT_SECONDARY,RADIUS_FRAME; spec=C/J; composes=Menu/Item,Menu/Separator,Menu/Shortcut

- **Variants:** State=Default
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.neutral` → #737373
  - `colors.popover` → #0a0a0a
  - `spacing.space-2` → 8px

### Menu/Panel

Composite of Studio's chrome, specified in spec C/J.
tier=composite; rust=kit/surface.rs::menu_surface; rust_status=new-in-C; today=widgets/mod.rs::menu_panel; paints=BG_MENU,BORDER_MENU,ICON_QUATERNARY,TEXT_SECONDARY,RADIUS_FRAME; pending=BORDER_MENU,TEXT_SECONDARY,RADIUS_FRAME; spec=C/J; composes=Menu/Item,Menu/Separator,Menu/Shortcut

- **Variants:** State=Default
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.neutral` → #737373
  - `colors.popover` → #0a0a0a
  - `spacing.space-2` → 8px

### Panel/Header

Composite of Studio's chrome, specified in spec C/E.
tier=composite; rust=kit/rows.rs::panel_header; rust_status=new-in-C; today=panels/left.rs::section_title; paints=ICON_QUATERNARY,TEXT; spec=C/E; composes=Button/Icon,Badge/StatusCount

- **Variants:** State=Default
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.bg-foreground` → #fafafa
  - `colors.neutral` → #737373
  - `typography.font-size-heading` → 14px

### Popover/ColorPicker

Composite of Studio's chrome, specified in spec F.
tier=composite; rust=widgets/picker.rs::show; rust_status=exists; paints=BG_MENU,BORDER_MENU,RADIUS_FRAME; pending=BORDER_MENU,RADIUS_FRAME; spec=F; composes=Swatch/Color,Slider/Handle,Input/PropertyField,Select/Trigger

- **Variants:** —
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.popover` → #0a0a0a

### Popover/Panel

Composite of Studio's chrome, specified in spec C.
tier=composite; rust=kit/surface.rs::popover_surface; rust_status=new-in-C; today=widgets/popover.rs::show; paints=BG_MENU,BORDER_MENU,ICON_QUATERNARY,TEXT,RADIUS_FRAME; pending=BORDER_MENU,RADIUS_FRAME; spec=C; composes=Panel/Header,Menu/Item,List/Row

- **Variants:** State=Default
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.neutral` → #737373
  - `colors.popover` → #0a0a0a
  - `colors.popover-foreground` → #fafafa

### Section/Header

Composite of Studio's chrome, specified in spec C/F.
tier=composite; rust=kit/rows.rs::section_header; rust_status=new-in-C; today=panels/properties.rs::title; paints=BORDER,ICON_QUATERNARY,TEXT,TEXT_TERTIARY; spec=C/F; composes=Button/Icon,Divider/Hairline

- **Variants:** State=Default, State=Disabled
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.bg-foreground` → #fafafa
  - `colors.border` → #262626
  - `colors.muted-foreground` → #a3a3a3
  - `colors.neutral` → #737373

### Table/Data

Composite of Studio's chrome, specified in spec K.
tier=composite; rust=variables/view.rs::table; rust_status=exists; paints=BG_PRIMARY,BG_HOVER,BORDER; spec=K; composes=List/Row,Input/PropertyField,Badge/StatusCount,Toggle/Switch

- **Variants:** —
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.bg` → #0a0a0a
  - `colors.bg-hover` → #171717
  - `colors.border` → #262626

### Tabs/Segmented

Composite of Studio's chrome, specified in spec C.
tier=composite; rust=kit/tabs.rs::segmented; rust_status=new-in-C; today=comments/ui.rs:133-152; paints=BG,BG_SELECTED,BORDER_STRONG,ICON_QUATERNARY,ICON_QUATERNARY_HOVER,ICON_SECONDARY,FOCUS_RING,TEXT_QUATERNARY,TEXT_SECONDARY; pending=ICON_QUATERNARY_HOVER,ICON_SECONDARY,TEXT_QUATERNARY,TEXT_SECONDARY; spec=C; composes=Tabs/Pill

- **Variants:** State=Default, State=Disabled, State=Focus, State=Hover, State=Selected
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.bg` → #0a0a0a
  - `colors.bg-active` → #262626
  - `colors.border-strong` → #404040
  - `colors.neutral` → #737373
  - `colors.ring` → #f97316
  - `radius.radius-md` → 8px
  - `radius.radius-sm` → 6px

### Toast/Default

Composite of Studio's chrome, specified in spec C/D/J.
tier=composite; rust=kit/surface.rs::toast_surface; rust_status=new-in-C; today=shell.rs::toast; paints=BG_PRIMARY_ALT,BORDER_MENU,ICON_BRAND_PRIMARY,TEXT,DANGER,SUCCESS,BG_SECONDARY_HOVER; pending=BG_PRIMARY_ALT,BORDER_MENU,ICON_BRAND_PRIMARY,DANGER; spec=C/D/J; composes=Status/Dot,Button/Tertiary

- **Variants:** State=Default
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.bg-foreground` → #fafafa
  - `colors.success` → #22c55e
  - `colors.surface-hover` → #262626
  - `radius.radius-xl` → 12px

## element

### Divider/Hairline

Element of Studio's chrome, specified in spec C.
tier=element; rust=widgets/mod.rs::hline,widgets/mod.rs::vline; rust_status=exists; paints=BORDER; spec=C

- **Variants:** State=Default
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.border` → #262626

### Focus/Ring

Element of Studio's chrome, specified in spec C.
tier=element; rust=kit/focus.rs::focus_ring; rust_status=new-in-C; paints=FOCUS_RING,FOCUS_RING_ERROR; pending=FOCUS_RING_ERROR; spec=C

- **Variants:** Tone=Brand, Tone=Error
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.ring` → #f97316

### Icon/Glyph

Element of Studio's chrome, specified in spec C.
tier=element; rust=icons.rs::paint; rust_status=exists; paints=ICON_QUATERNARY,ICON_QUATERNARY_HOVER,ICON_SECONDARY_HOVER; pending=ICON_QUATERNARY_HOVER,ICON_SECONDARY_HOVER; spec=C

- **Variants:** Size=Md, Size=Sm, State=Default, State=Hover, State=Selected
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.neutral` → #737373

### Menu/Separator

Element of Studio's chrome, specified in spec C.
tier=element; rust=widgets/menu.rs::popup_tree; rust_status=exists; today=widgets/menu.rs:441,504; paints=BORDER; spec=C

- **Variants:** State=Default
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.border` → #262626

### Menu/Shortcut

Element of Studio's chrome, specified in spec C.
tier=element; rust=widgets/menu.rs::popup_tree; rust_status=exists; today=widgets/menu.rs:492-501; paints=TEXT_QUATERNARY; pending=TEXT_QUATERNARY; spec=C

- **Variants:** State=Active, State=Default, State=Disabled, State=Hover
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `typography.font-size-caption` → 12px

### Status/Dot

Element of Studio's chrome, specified in spec C/J.
tier=element; rust=kit/badge.rs::status_dot; rust_status=new-in-C; today=shell.rs::toast (:1136); paints=ICON_BRAND_PRIMARY,DANGER,SUCCESS; pending=ICON_BRAND_PRIMARY,DANGER; spec=C/J

- **Variants:** Tone=Error, Tone=Info, Tone=Success
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.success` → #22c55e

## region

### Bar/Presentation

Region of Studio's chrome, specified in spec J.
tier=region; rust=present.rs::show; rust_status=exists; spec=J; composes=Button/*,Tooltip/Default,Divider/Hairline

- **Variants:** —
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)

### Navigation/BottomCanvasToolbar

Region of Studio's chrome, specified in spec D.
tier=region; rust=panels/toolbar.rs::show; rust_status=exists; spec=D; composes=Button/Tool,Tabs/Segmented,Bar/Contextual,Button/Secondary

- **Variants:** —
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)

### Navigation/LeftRail

Region of Studio's chrome, specified in spec D.
tier=region; rust=panels/rail.rs::show; rust_status=exists; spec=D; composes=Button/Icon,Button/Tool,Tooltip/Default

- **Variants:** Rail=48px
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)

### Navigation/Rulers

Region of Studio's chrome, specified in spec D.
tier=region; rust=canvas/rulers.rs::strips; rust_status=exists; spec=D; composes=Divider/Hairline

- **Variants:** —
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)

### Navigation/TitleBar

Region of Studio's chrome, specified in spec D.
tier=region; rust=shell.rs::title_bar; rust_status=new-in-D; today=shell.rs:1536-1562 (inside draw_window); spec=D; composes=Icon/Glyph,Divider/Hairline

- **Variants:** —
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)

### Navigation/TopTabBar

Region of Studio's chrome, specified in spec D.
tier=region; rust=tabs.rs::show_top_tab_bar; rust_status=exists; spec=D; composes=Tabs/Pill,Button/Icon,Menu/Panel

- **Variants:** Bar=32px
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)

### Panel/AgentTerminal

Region of Studio's chrome, specified in spec I.
tier=region; rust=terminal.rs::show_left_with_doc; rust_status=exists; spec=I; flow=panel-agents-terminal; composes=Tabs/Pill,Button/*,Tag/Chip,Card/Approval,Card/AgentTurn,Input/Composer,Input/Search,Banner/Inline

- **Variants:** —
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)

### Panel/Assets

Region of Studio's chrome, specified in spec K.
tier=region; rust=panels/assets.rs::show; rust_status=exists; spec=K; composes=Panel/Header,Input/Search,Card/Library,List/Row,Badge/StatusCount

- **Variants:** —
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)

### Panel/CheckDesigns

Region of Studio's chrome, specified in spec K.
tier=region; rust=panels/check_designs.rs::show; rust_status=exists; spec=K; composes=Panel/Header,Button/*,Badge/StatusCount,Tabs/Segmented,Progress/Bar,EmptyState/Default

- **Variants:** —
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)

### Panel/Comments

Region of Studio's chrome, specified in spec J.
tier=region; rust=comments/panel.rs::show; rust_status=exists; spec=J; composes=Tabs/Segmented,Input/Search,Popover/Panel

- **Variants:** —
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)

### Panel/DevInspect

Region of Studio's chrome, specified in spec F/G.
tier=region; rust=panels/inspect.rs::show,dev/list.rs::show; rust_status=exists; spec=F/G; composes=Section/Header,Badge/StatusCount,Code/Block,Button/Primary,Tabs/Pill,Tree/Row

- **Variants:** —
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)

### Panel/FindReplace

Region of Studio's chrome, specified in spec J.
tier=region; rust=find/panel.rs::show; rust_status=exists; spec=J; composes=Input/Search,Input/Field,Button/Secondary,Button/Icon

- **Variants:** —
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)

### Panel/HomeMain

Region of Studio's chrome, specified in spec H.
tier=region; rust=home/mod.rs::main_view::show; rust_status=exists; spec=H; composes=Button/*,Card/FileThumbnail,Card/StarterTemplate,List/Row,EmptyState/Default,Icon/Tile

- **Variants:** —
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)

### Panel/HomeSidebar

Region of Studio's chrome, specified in spec H.
tier=region; rust=home/mod.rs::sidebar::show; rust_status=exists; spec=H; composes=Input/Search,List/Row,Badge/StatusCount

- **Variants:** —
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)

### Panel/LeftLayers

Region of Studio's chrome, specified in spec E.
tier=region; rust=panels/left.rs::show,panels/layers.rs::show; rust_status=exists; spec=E; composes=Panel/Header,List/Row,Tree/Row,Input/Search,Menu/Panel,Badge/StatusCount

- **Variants:** —
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)

### Panel/RightInspector

Region of Studio's chrome, specified in spec F.
tier=region; rust=panels/right.rs::show; rust_status=exists; spec=F; composes=Tabs/Pill,Section/Header,Input/PropertyField,Select/Trigger,Tabs/Segmented,Checkbox/Box,Popover/ColorPicker,Button/*

- **Variants:** —
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)

### Panel/SpecReader

Region of Studio's chrome, specified in spec G.
tier=region; rust=specs.rs::show_reader; rust_status=exists; today=hidden: specs_reader_open = false; spec=G; flow=panel-specs-reader; composes=Section/Header

- **Variants:** —
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)

### Panel/VersionHistory

Region of Studio's chrome, specified in spec J.
tier=region; rust=history/panel.rs::show; rust_status=exists; spec=J; composes=List/Row,Banner/Inline

- **Variants:** —
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)

### Window/Variables

Region of Studio's chrome, specified in spec K.
tier=region; rust=variables/view.rs::show; rust_status=exists; spec=K; flow=modal-variables; composes=Table/Data,Popover/Panel,Menu/Item

- **Variants:** —
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)

## screen

### Screen/EditorDesign

Screen of Studio's chrome, specified in spec D.
tier=screen; rust=shell.rs::draw_window; rust_status=exists; spec=D; flow=editor-canvas; composes=Navigation/TitleBar,Navigation/TopTabBar,Navigation/LeftRail,Panel/LeftLayers,Navigation/Rulers,Navigation/BottomCanvasToolbar,Panel/RightInspector,Menu/Panel,Dialog/CommandPalette,Toast/Default

- **Variants:** —
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)

### Screen/EditorDev

Screen of Studio's chrome, specified in spec G.
tier=screen; rust=shell.rs::draw_window; rust_status=exists; spec=G; flow=editor-devmode; composes=Navigation/TitleBar,Navigation/TopTabBar,Navigation/LeftRail,Panel/LeftLayers,Navigation/Rulers,Navigation/BottomCanvasToolbar,Panel/RightInspector,Panel/DevInspect,Menu/Panel,Dialog/CommandPalette,Toast/Default

- **Variants:** —
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)

### Screen/EditorPrototype

Screen of Studio's chrome, specified in spec F.
tier=screen; rust=shell.rs::draw_window; rust_status=exists; spec=F; flow=editor-prototype; composes=Navigation/TitleBar,Navigation/TopTabBar,Navigation/LeftRail,Panel/LeftLayers,Navigation/Rulers,Navigation/BottomCanvasToolbar,Panel/RightInspector,Menu/Panel,Dialog/CommandPalette,Toast/Default

- **Variants:** —
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)

### Screen/Home

Screen of Studio's chrome, specified in spec H.
tier=screen; rust=shell.rs::home_frame; rust_status=exists; spec=H; flow=home-recents,home-templates,home-all-folders,home-resources,home-drafts,home-trash; composes=Navigation/TitleBar,Navigation/TopTabBar,Panel/HomeSidebar,Panel/HomeMain,Dialog/Modal,Toast/Default

- **Variants:** —
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)

### Screen/Present

Screen of Studio's chrome, specified in spec J.
tier=screen; rust=shell.rs::draw_window; rust_status=exists; spec=J; flow=editor-present; composes=Bar/Presentation

- **Variants:** —
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)

