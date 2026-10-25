# Component documentation

_12 component(s), regenerated from the registry._

## action

### Button/Primary

Brand orange primary CTA button (#E36323 fill, #09090B label, 28px min height, 6px radius)

- **Variants:** Size=28px, State=Active, State=Default, State=Disabled, State=Focus, State=Hover
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.primary` → #e36323
  - `colors.primary-active` → #ff9e66
  - `colors.primary-foreground` → #09090b
  - `colors.primary-hover` → #f07436
  - `colors.ring` → #e36323
  - `duration.duration-fast` → 83ms
  - `radius.radius-md` → 6px
  - `spacing.space-2` → 8px
  - `spacing.space-3` → 12px
  - `typography.font-family-sans` → Inter, -apple-system, BlinkMacSystemFont, sans-serif
  - `typography.font-size-body` → 11px

### Button/Secondary

Elevated zinc secondary action button (#27272A fill, #F5F5F7 label, 1px #27272A border)

- **Variants:** State=Active, State=Default, State=Disabled, State=Focus, State=Hover, Tone=Zinc
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.border` → #27272a
  - `colors.secondary` → #27272a
  - `colors.secondary-active` → #3f3f46
  - `colors.secondary-foreground` → #f5f5f7
  - `colors.secondary-hover` → #2e2e34
  - `duration.duration-fast` → 83ms
  - `radius.radius-md` → 6px
  - `spacing.space-2` → 8px
  - `spacing.space-3` → 12px
  - `typography.font-family-sans` → Inter, -apple-system, BlinkMacSystemFont, sans-serif
  - `typography.font-size-body` → 11px

## content

### Card/FileThumbnail

Home workspace file card with live node mini-map preview, concentric 8px outer / 4px inner radius, and context actions

- **Variants:** State=Active, State=Default, State=Disabled, State=Focus, State=Hover, View=Grid
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.border` → #27272a
  - `colors.card` → #1e1e22
  - `colors.card-foreground` → #f5f5f7
  - `colors.muted-foreground` → #a8a8b2
  - `colors.surface-hover` → #232329
  - `radius.radius-lg` → 8px
  - `spacing.space-3` → 12px
  - `spacing.space-4` → 16px
  - `typography.font-family-sans` → Inter, -apple-system, BlinkMacSystemFont, sans-serif
  - `typography.font-size-caption` → 10px
  - `typography.font-size-title` → 12px

### Card/StarterTemplate

Interactive starter template card in Home top tray with mini-canvas preview and one-click document seeding

- **Variants:** State=Active, State=Default, State=Disabled, State=Focus, State=Hover, Tray=Home
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.border` → #27272a
  - `colors.card` → #1e1e22
  - `colors.card-foreground` → #f5f5f7
  - `colors.primary` → #e36323
  - `colors.surface-hover` → #232329
  - `radius.radius-lg` → 8px
  - `spacing.space-3` → 12px
  - `spacing.space-4` → 16px
  - `typography.font-family-sans` → Inter, -apple-system, BlinkMacSystemFont, sans-serif
  - `typography.font-size-title` → 12px

## feedback

### Badge/StatusCount

Pill count and semantic status badge using monospace numerals separated from section headers (D85)

- **Variants:** State=Active, State=Default, State=Disabled, State=Focus, State=Hover, Tone=Neutral
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.component` → #06b6d4
  - `colors.muted-foreground` → #a8a8b2
  - `colors.secondary` → #27272a
  - `colors.success` → #14ae5c
  - `radius.radius-pill` → 13px
  - `spacing.space-1` → 4px
  - `spacing.space-2` → 8px
  - `typography.font-family-mono` → Roboto Mono, SFMono-Regular, Menlo, monospace
  - `typography.font-size-small` → 9px

## form

### Input/PropertyField

24px height numeric/text scrubbable inspector input with muted prefix label and #E36323 focus ring

- **Variants:** Axis=Scrub, State=Active, State=Default, State=Disabled, State=Focus, State=Hover
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.bg-foreground` → #f5f5f7
  - `colors.border-focus` → #e36323
  - `colors.border-subtle` → #222226
  - `colors.input` → #121215
  - `colors.muted-foreground` → #a8a8b2
  - `colors.ring` → #e36323
  - `radius.radius-sm` → 4px
  - `spacing.space-1` → 4px
  - `spacing.space-2` → 8px
  - `typography.font-family-mono` → Roboto Mono, SFMono-Regular, Menlo, monospace
  - `typography.font-size-body` → 11px

## layout

### Panel/AgentTerminal

400px resizable per-file embedded VTE PTY terminal panel with Orca Ghostty palette, procedural box-drawing, and 3-tier live status dots

- **Variants:** Pty=Live, State=Active, State=Default, State=Disabled, State=Focus, State=Hover
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.bg` → #18181b
  - `colors.border` → #27272a
  - `colors.canvas` → #121215
  - `colors.canvas-foreground` → #f5f5f7
  - `colors.primary` → #e36323
  - `colors.success` → #14ae5c
  - `colors.warning` → #f59e0b
  - `radius.radius-sm` → 4px
  - `spacing.ai-panel-width` → 400px
  - `spacing.space-2` → 8px
  - `typography.font-family-mono` → Roboto Mono, SFMono-Regular, Menlo, monospace
  - `typography.font-size-body` → 11px

### Panel/RightInspector

240px right properties and Dev Mode inspector with 28px header tabs, collapsible property sections, and dedicated 36px status footer

- **Variants:** State=Active, State=Default, State=Disabled, State=Focus, State=Hover, Tab=Design
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.bg` → #18181b
  - `colors.bg-foreground` → #f5f5f7
  - `colors.border` → #27272a
  - `colors.component` → #06b6d4
  - `colors.success` → #14ae5c
  - `colors.surface` → #1e1e22
  - `radius.radius-sm` → 4px
  - `spacing.panel-width` → 240px
  - `spacing.space-2` → 8px
  - `spacing.space-3` → 12px
  - `typography.font-family-mono` → Roboto Mono, SFMono-Regular, Menlo, monospace
  - `typography.font-family-sans` → Inter, -apple-system, BlinkMacSystemFont, sans-serif
  - `typography.font-size-body` → 11px

## navigation

### Navigation/BottomCanvasToolbar

Floating bottom canvas creation dock with tool triggers and #14AE5C Dev Mode toggle

- **Variants:** Dock=Canvas, State=Active, State=Default, State=Disabled, State=Focus, State=Hover
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.border` → #27272a
  - `colors.primary` → #e36323
  - `colors.success` → #14ae5c
  - `colors.surface` → #1e1e22
  - `colors.surface-foreground` → #f5f5f7
  - `colors.surface-hover` → #232329
  - `radius.radius-lg` → 8px
  - `spacing.space-2` → 8px
  - `spacing.space-3` → 12px
  - `typography.font-family-sans` → Inter, -apple-system, BlinkMacSystemFont, sans-serif
  - `typography.font-size-body` → 11px

### Navigation/LeftRail

Home sidebar and 56px editor activity rail with 1px right border and orange active indicator

- **Variants:** Rail=56px, State=Active, State=Default, State=Disabled, State=Focus, State=Hover
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.bg` → #18181b
  - `colors.bg-foreground` → #f5f5f7
  - `colors.border` → #27272a
  - `colors.primary` → #e36323
  - `colors.surface-active` → #2e2e36
  - `colors.surface-hover` → #232329
  - `radius.radius-md` → 6px
  - `spacing.rail-width` → 56px
  - `spacing.space-2` → 8px
  - `typography.font-family-sans` → Inter, -apple-system, BlinkMacSystemFont, sans-serif
  - `typography.font-size-body` → 11px

### Navigation/TopTabBar

36px multi-document workspace top tab bar with Home button, document tabs, and window controls

- **Variants:** Bar=36px, State=Active, State=Default, State=Disabled, State=Focus, State=Hover
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.bg` → #18181b
  - `colors.bg-foreground` → #f5f5f7
  - `colors.border` → #27272a
  - `colors.primary` → #e36323
  - `colors.surface` → #1e1e22
  - `radius.radius-sm` → 4px
  - `spacing.space-2` → 8px
  - `spacing.space-3` → 12px
  - `typography.font-family-sans` → Inter, -apple-system, BlinkMacSystemFont, sans-serif
  - `typography.font-size-body` → 11px

## overlay

### Dialog/CommandPalette

Modal Cmd+K command palette overlay with fuzzy action search, keyboard shortcut badges, and focus trap

- **Variants:** Mode=CmdK, State=Active, State=Default, State=Disabled, State=Focus, State=Hover
- **States:** —
- **Consider adding:** focus, disabled, loading, error, empty (common states this component does not declare)
- **Tokens used:**
  - `colors.border-strong` → #3f3f46
  - `colors.popover` → #18181b
  - `colors.popover-foreground` → #f5f5f7
  - `colors.primary` → #e36323
  - `colors.surface-hover` → #232329
  - `radius.radius-xl` → 10px
  - `spacing.space-3` → 12px
  - `spacing.space-4` → 16px
  - `typography.font-family-mono` → Roboto Mono, SFMono-Regular, Menlo, monospace
  - `typography.font-family-sans` → Inter, -apple-system, BlinkMacSystemFont, sans-serif
  - `typography.font-size-title` → 12px

