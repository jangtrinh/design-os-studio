"""Spec T 5.1: writes the registry tables (markdown, to argv[1]), registry-records.json and contract-colour-partition.json.
Usage: python3 -I evidence/t-ontology/scripts/emit-registry-tables.py <tables.md>
"""
import importlib.util, json, os, sys, collections
sys.dont_write_bytecode = True  # imports here must not write caches into specs/ or scripts/
HERE = os.path.dirname(os.path.abspath(__file__))
_sp = importlib.util.spec_from_file_location('gen_record_bindings', os.path.join(HERE, 'gen-record-bindings.py'))
gen = importlib.util.module_from_spec(_sp); _sp.loader.exec_module(gen)
recs, part = gen.build()
NC='new-in-C'
# name: (tier, composes, variants, kit, rust list, status, today, owner, flow)
M=[
('Divider/Hairline','element',[],'—','`_Modal header` Divider (81903)',['widgets/mod.rs::hline','widgets/mod.rs::vline'],'exists','','C',''),
('Menu/Separator','element',[],'—','`_Dropdown menu list item` Divider=True (27767)',['widgets/menu.rs::popup_tree'],'exists','widgets/menu.rs:441,504','C',''),
('Menu/Shortcut','element',[],'—','`_Command shortcut` (3307:405262)',['widgets/menu.rs::popup_tree'],'exists','widgets/menu.rs:492-501','C',''),
('Focus/Ring','element',[],'`Tone=Brand/Error`','effect style `Focus rings/focus-ring`',['kit/focus.rs::focus_ring'],NC,'','C',''),
('Icon/Glyph','element',[],'`Size=Sm`(16), `Size=Md`(20); `State=Default/Hover/Selected` (T variants)','↳ Icons (3463:407484); R6 keeps Studio\'s set',['icons.rs::paint'],'exists','','C',''),
('Status/Dot','element',[],'`Tone=Info/Error/Success`','`Notification` featured icon, replaced by a dot (C)',['kit/badge.rs::status_dot'],NC,'shell.rs::toast (:1136)','C/J',''),
('Button/Primary','component',[],'`Size=Xs/Sm/Md` (`Size=28px` dropped)','C 3.3',['kit/button.rs::button'],NC,'home/dialog.rs:367-386 and four more (C 3.3)','C',''),
('Button/Secondary','component',[],'`Size=Xs/Sm/Md` (`Tone=Zinc` dropped)','C 3.3',['kit/button.rs::button'],NC,'comments/ui.rs:92-108 and three more','C',''),
('Button/Tertiary','component',[],'`Size=Xs/Sm/Md`','C 3.3',['kit/button.rs::button'],NC,'home/dialog.rs:401-410','C',''),
('Button/Destructive','component',[],'`Size=Xs/Sm/Md`','C 3.3',['kit/button.rs::button'],NC,'home/mod.rs:1412-1417','C',''),
('Button/SecondaryDestructive','component',[],'`Size=Xs/Sm/Md`','C 3.3',['kit/button.rs::button'],NC,'home/mod.rs:1241','C/H',''),
('Button/Icon','component',[],'`Size=Xs/Sm`','C 3.3',['kit/button.rs::button'],NC,'widgets/mod.rs::icon_button','C',''),
('Button/Tool','component',[],'—','C 3.3',['kit/button.rs::button'],NC,'panels/toolbar.rs:209-227','C/D',''),
('Button/Close','component',[],'`Size=Xs/Sm`','C 3.3',['kit/button.rs::button'],NC,'widgets/popover.rs:40-43','C',''),
('Input/Field','component',[],'`Size=Xs/Sm`','C 3.3',['kit/field.rs::paint_field'],NC,'widgets/field.rs::field_bg','C',''),
('Input/PropertyField','component',[],'`Size=Xs`; `Axis=Scrub` (kept)','C 3.3',['widgets/field.rs::num_field'],'exists','','C/F',''),
('Input/Search','component',[],'`Size=Xs/Sm/Md`','C 3.3',['kit/field.rs::paint_field'],NC,'find/filter.rs::show_field','C',''),
('Select/Trigger','component',[],'`Size=Xs/Sm`; `State=Open`','C 3.3',['widgets/menu.rs::dropdown'],'exists','widgets/menu.rs:571-606','C',''),
('Menu/Item','component',[],'`Size=Xs/Sm`','C 3.3',['widgets/menu.rs::popup_tree'],'exists','widgets/menu.rs:430-505','C',''),
('Tabs/Pill','component',[],'heights per R10.1 (no `Size=`)','C 3.3',['kit/tabs.rs::tab'],NC,'panels/right.rs:103-127','C',''),
('Toggle/Switch','component',[],'—','C 3.3',['kit/choice.rs::toggle'],NC,'panels/components.rs::toggle','C',''),
('Checkbox/Box','component',[],'—','C 3.3',['kit/choice.rs::checkbox'],NC,'widgets/popover.rs::check','C',''),
('Radio/Card','component',[],'—','C 3.3',['kit/choice.rs::radio_card'],NC,'agent_approval_card.rs:256-286','C/I',''),
('Badge/StatusCount','component',[],'`Size=Sm`(22), `Size=Md`(24, K); `Tone=Neutral/Success/Warning/Error`','C 3.3',['kit/badge.rs::count_badge'],NC,'home/mod.rs:1113-1115','C',''),
('Tag/Chip','component',[],'`Size=Sm`','C 3.3',['kit/badge.rs::tag'],NC,'agent_selection_chip.rs:160-200','C',''),
('Tooltip/Default','component',[],'—','C 3.3',['theme.rs::apply'],'exists','','C',''),
('Tree/Row','component',[],'`Size=Xs`','C 3.3',['kit/rows.rs::tree_row_bg'],NC,'panels/layers.rs:516-596','C/E',''),
('List/Row','component',[],'`Size=Xs/Sm`','C 3.3',['kit/rows.rs::list_row'],NC,'home/mod.rs:1173-1190','C',''),
('Swatch/Color','component',[],'—','C 3.3',['kit/choice.rs::swatch_ring'],NC,'widgets/mod.rs::swatch','C/F',''),
('Slider/Handle','component',[],'—','C 3.3',['kit/choice.rs::slider_handle'],NC,'','C',''),
('Progress/Bar','component',[],'—','C 3.3',['kit/feedback.rs::progress_bar'],NC,'panels/check_designs.rs::progress_row','C/K',''),
('Loading/Inline','component',[],'—','C 3.3',['kit/feedback.rs::loading_ring'],NC,'image_jobs.rs::layer_row_progress_chip','C/I',''),
('Icon/Tile','component',[],'`Kind=Design/Board/Deck/Spec/Neutral`; `Size=Sm`(20), `Size=Md`(32)','Featured icon (kit `Empty state`, `File type icon`)',['kit/feedback.rs::tile_icon'],NC,'home/mod.rs:2583-2601','C/H',''),
('Menu/Panel','composite',['Menu/Item','Menu/Separator','Menu/Shortcut'],'—','`Dropdown menu` (3281:383083)',['kit/surface.rs::menu_surface'],NC,'widgets/mod.rs::menu_panel','C/J',''),
('Menu/Context','composite',['Menu/Item','Menu/Separator','Menu/Shortcut'],'—','`Context menu` (11547:540792)',['panels/context.rs::show'],'exists','','C/J',''),
('Tabs/Segmented','composite',['Tabs/Pill'],'—','`Horizontal tabs` Button minimal (1118:69893)',['kit/tabs.rs::segmented'],NC,'comments/ui.rs:133-152','C',''),
('Panel/Header','composite',['Button/Icon','Badge/StatusCount'],'—','derived `_Modal header` title role',['kit/rows.rs::panel_header'],NC,'panels/left.rs::section_title','C/E',''),
('Section/Header','composite',['Button/Icon','Divider/Hairline'],'—','derived `_Modal header`',['kit/rows.rs::section_header'],NC,'panels/properties.rs::title','C/F',''),
('Dialog/Modal','composite',['Panel/Header','Button/Close','Button/*','Divider/Hairline'],'`Kind=Libraries/Plugins` (flow ids, 5.1 note)','`Modal` (4057:415205) + `_Modal actions` (4057:414352)',['kit/surface.rs::modal_surface'],NC,'home/dialog.rs::show','C/J','modal-assets-libraries,modal-plugins'),
('Popover/Panel','composite',['Panel/Header','Menu/Item','List/Row'],'—','derived `Dropdown menu` frame',['kit/surface.rs::popover_surface'],NC,'widgets/popover.rs::show','C',''),
('Dialog/CommandPalette','composite',['Input/Search','Tabs/Segmented','Menu/Item','Badge/StatusCount','Menu/Shortcut'],'`Kind=CmdK/Actions`','`Command bar` (3307:407570; frame 3307:407031)',['kit/surface.rs::command_bar_surface'],NC,'cmdk.rs::show; panels/menus.rs:907-1085','C/J/D','modal-cmdk'),
('Toast/Default','composite',['Status/Dot','Button/Tertiary'],'—','`Notification` (1135:618)',['kit/surface.rs::toast_surface'],NC,'shell.rs::toast','C/D/J',''),
('EmptyState/Default','composite',['Icon/Tile','Button/Secondary'],'—','`Empty state` (1182:317)',['kit/feedback.rs::empty_state'],NC,'home/mod.rs::main_view::empty_state','C',''),
('Card/FileThumbnail','composite',['Icon/Tile','Badge/StatusCount'],'—','none (Studio\'s own)',['home/mod.rs::main_view::card'],'exists','','H',''),
('Card/StarterTemplate','composite',['Button/Secondary'],'—','none',['home/mod.rs::main_view::templates_view'],'exists','','H',''),
('Card/Library','composite',['Tag/Chip','Toggle/Switch'],'—','none',['panels/assets.rs::library_card'],'new-in-K','panels/assets.rs:542-552','K',''),
('Card/Approval','composite',['Badge/StatusCount','Button/*','Radio/Card'],'—','none',['agent_approval_card.rs::paint_approval_card'],'exists','','I',''),
('Card/AgentTurn','composite',['Status/Dot','Badge/StatusCount'],'—','`_Feed item base` (Activity feeds)',['agent_turn_card.rs::show'],'exists','','I',''),
('Banner/Inline','composite',['Status/Dot','Button/*'],'`Tone=Neutral/Success/Warning/Error`','`Alert` (1130:81134)',['history/banner.rs::show'],'exists','','J/I/K',''),
('Table/Data','composite',['List/Row','Input/PropertyField','Badge/StatusCount','Toggle/Switch'],'—','`Table` (Tables page)',['variables/view.rs::table'],'exists','','K',''),
('Code/Block','composite',[],'—','`Code snippet` (Code snippets page)',['dev/code_view.rs::code_block'],'exists','','G',''),
('Input/Composer','composite',['Tag/Chip','Button/Icon','Button/Primary'],'—','`Textarea input field` (1238:278)',['agent_composer.rs::show_composer'],'exists','','I',''),
('Popover/ColorPicker','composite',['Swatch/Color','Slider/Handle','Input/PropertyField','Select/Trigger'],'—','`Color picker` (Color pickers page)',['widgets/picker.rs::show'],'exists','','F',''),
('Bar/Contextual','composite',['Button/Icon','Button/Tertiary','Divider/Hairline'],'—','none',['panels/toolbar.rs::contextual_bar'],'new-in-D','panels/toolbar.rs:160-189','D',''),
]
REG=[
('Navigation/TitleBar',['Icon/Glyph','Divider/Hairline'],['shell.rs::title_bar'],'new-in-D','shell.rs:1536-1562 (inside draw_window)','D',''),
('Navigation/TopTabBar',['Tabs/Pill','Button/Icon','Menu/Panel'],['tabs.rs::show_top_tab_bar'],'exists','','D',''),
('Navigation/LeftRail',['Button/Icon','Button/Tool','Tooltip/Default'],['panels/rail.rs::show'],'exists','','D',''),
('Navigation/BottomCanvasToolbar',['Button/Tool','Tabs/Segmented','Bar/Contextual','Button/Secondary'],['panels/toolbar.rs::show'],'exists','','D',''),
('Navigation/Rulers',['Divider/Hairline'],['canvas/rulers.rs::strips'],'exists','','D',''),
('Panel/LeftLayers',['Panel/Header','List/Row','Tree/Row','Input/Search','Menu/Panel','Badge/StatusCount'],['panels/left.rs::show','panels/layers.rs::show'],'exists','','E',''),
('Panel/RightInspector',['Tabs/Pill','Section/Header','Input/PropertyField','Select/Trigger','Tabs/Segmented','Checkbox/Box','Popover/ColorPicker','Button/*'],['panels/right.rs::show'],'exists','','F',''),
('Panel/DevInspect',['Section/Header','Badge/StatusCount','Code/Block','Button/Primary','Tabs/Pill','Tree/Row'],['panels/inspect.rs::show','dev/list.rs::show'],'exists','','F/G',''),
('Panel/AgentTerminal',['Tabs/Pill','Button/*','Tag/Chip','Card/Approval','Card/AgentTurn','Input/Composer','Input/Search','Banner/Inline'],['terminal.rs::show_left_with_doc'],'exists','','I','panel-agents-terminal'),
('Panel/SpecReader',['Section/Header'],['specs.rs::show_reader'],'exists','hidden: specs_reader_open = false','G','panel-specs-reader'),
('Panel/HomeSidebar',['Input/Search','List/Row','Badge/StatusCount'],['home/mod.rs::sidebar::show'],'exists','','H',''),
('Panel/HomeMain',['Button/*','Card/FileThumbnail','Card/StarterTemplate','List/Row','EmptyState/Default','Icon/Tile'],['home/mod.rs::main_view::show'],'exists','','H',''),
('Panel/Assets',['Panel/Header','Input/Search','Card/Library','List/Row','Badge/StatusCount'],['panels/assets.rs::show'],'exists','','K',''),
('Panel/CheckDesigns',['Panel/Header','Button/*','Badge/StatusCount','Tabs/Segmented','Progress/Bar','EmptyState/Default'],['panels/check_designs.rs::show'],'exists','','K',''),
('Panel/Comments',['Tabs/Segmented','Input/Search','Popover/Panel'],['comments/panel.rs::show'],'exists','','J',''),
('Panel/VersionHistory',['List/Row','Banner/Inline'],['history/panel.rs::show'],'exists','','J',''),
('Panel/FindReplace',['Input/Search','Input/Field','Button/Secondary','Button/Icon'],['find/panel.rs::show'],'exists','','J',''),
('Window/Variables',['Table/Data','Popover/Panel','Menu/Item'],['variables/view.rs::show'],'exists','','K','modal-variables'),
('Bar/Presentation',['Button/*','Tooltip/Default','Divider/Hairline'],['present.rs::show'],'exists','','J',''),
]
SCR=[
('Screen/Home',['Navigation/TitleBar','Navigation/TopTabBar','Panel/HomeSidebar','Panel/HomeMain'],['Dialog/Modal','Toast/Default'],['shell.rs::home_frame'],'H','home-recents,home-templates,home-all-folders,home-resources,home-drafts,home-trash'),
('Screen/EditorDesign',['Navigation/TitleBar','Navigation/TopTabBar','Navigation/LeftRail','Panel/LeftLayers','Navigation/Rulers','Navigation/BottomCanvasToolbar','Panel/RightInspector'],['Menu/Panel','Dialog/CommandPalette','Toast/Default'],['shell.rs::draw_window'],'D','editor-canvas'),
('Screen/EditorPrototype',['as EditorDesign'],['as EditorDesign'],['shell.rs::draw_window'],'F','editor-prototype'),
('Screen/EditorDev',['as EditorDesign, plus Panel/DevInspect'],['as EditorDesign'],['shell.rs::draw_window'],'G','editor-devmode'),
('Screen/Present',['Bar/Presentation'],[],['shell.rs::draw_window'],'J','editor-present'),
]
def c(x): return '`'+x+'`'
def toks(n): r=recs.get(n); return ', '.join(c(t) for t in r['tokens']) if r and r['tokens'] else '—'
def pend(n): r=recs.get(n); return ', '.join(c(t) for t in r['pending']) if r and r['pending'] else '—'
def states(n):
    r=recs.get(n)
    if not r or r['states'] is None: return '— (no C table)'
    s=','.join(r['states']); return c(s)+(' + `State=Selected`' if r['selected'] else '')
def rust(rl,st,today):
    s=', '.join(c(x) for x in rl)+(' ('+st+')' if st!='exists' else '')
    return s+('; today '+c(today) if today else '')
out=[]
out.append('Elements (6) and components (27):\n')
out.append('| Name | Tier | Tokens (`--tokens`) | `pending=` (Rust constant, no store key yet) | Variants | States (`--states`, C 3.4) | Kit reference | `rust=` (`rust_status`); `today=` | Owner |')
out.append('|---|---|---|---|---|---|---|---|---|')
for n,t,comp,v,k,rl,st,td,o,f in M:
    if t=='composite': continue
    out.append(f'| {c(n)} | {t} | {toks(n)} | {pend(n)} | {v} | {states(n)} | {k} | {rust(rl,st,td)} | {o} |')
out.append('\nComposites (21):\n')
out.append('| Name | Composes (`data-ds`) | Tokens | `pending=` | Variants | States (C 3.4) | Kit reference | `rust=` (`rust_status`); `today=` | Owner | `flow=` |')
out.append('|---|---|---|---|---|---|---|---|---|---|')
for n,t,comp,v,k,rl,st,td,o,f in M:
    if t!='composite': continue
    out.append(f'| {c(n)} | {", ".join(c(x) for x in comp) or "text only"} | {toks(n)} | {pend(n)} | {v} | {states(n)} | {k} | {rust(rl,st,td)} | {o} | {c(f) if f else "—"} |')
out.append('\nRegions (19):\n')
out.append('| Name | Composes | `rust=` (`rust_status`); `today=` | Owner | `flow=` |')
out.append('|---|---|---|---|---|')
for n,comp,rl,st,td,o,f in REG:
    out.append(f'| {c(n)} | {", ".join(c(x) for x in comp)} | {rust(rl,st,td)} | {o} | {c(f) if f else "—"} |')
out.append('\nScreens (5):\n')
out.append('| Name | Composes: regions; floating composites | `rust=` | Owner | `flow=` (list) |')
out.append('|---|---|---|---|---|')
def screen_parts(regs, fl):
    r, f2 = [], []
    for x in regs:
        if x.startswith('as EditorDesign'):
            r += SCR[1][1]
            if 'plus' in x: r.append(x.split('plus ')[1])
        else: r.append(x)
    for x in fl:
        f2 += SCR[1][2] if x.startswith('as EditorDesign') else [x]
    return list(dict.fromkeys(r)), list(dict.fromkeys(f2))
for n,regs,fl,rl,o,f in SCR:
    r2,f2=screen_parts(regs,fl)
    a=', '.join(c(x) for x in r2); b=', '.join(c(x) for x in f2) or '—'
    out.append(f'| {c(n)} | {a}; {b} | {", ".join(c(x) for x in rl)} | {o} | {c(f)} |')
open(sys.argv[1],'w').write('\n'.join(out)+'\n')
# machine form
mach=[]
for n,t,comp,v,k,rl,st,td,o,f in M:
    r=recs[n]; mach.append(dict(name=n,tier=t,composes=comp,tokens=r['tokens'],pending=r['pending'],paints=r['paints'],paints_from_parts_line=r['paints_from_parts_line'],states=r['states'],selected=r['selected'],rust=rl,rust_status=st,today=td,spec=o,flow=f.split(',') if f else []))
for n,comp,rl,st,td,o,f in REG: mach.append(dict(name=n,tier='region',composes=comp,rust=rl,rust_status=st,today=td,spec=o,flow=f.split(',') if f else []))
for n,regs,fl,rl,o,f in SCR:
    r2,f2=screen_parts(regs,fl)
    mach.append(dict(name=n,tier='screen',composes=r2+f2,rust=rl,rust_status='exists',spec=o,flow=f.split(',')))
json.dump(mach,open(os.path.join(HERE,'..','registry-records.json'),'w'),indent=1)
json.dump({'source':'specs/token-contract.md (a) and (b) at fix round 2; classes from contract-colour-classes.json (R26) plus N74-N77; mapped = G01 map after A; shared-key = an existing store key with the same value and role; pending = no such key (G1 adds one); values are the full contract value (#RRGGBB, #RRGGBBAA for rgba; aliases resolved); keyed by the shipped (V2) name','counts':collections.Counter(v['class'] for v in part.values()),'names':part},open(os.path.join(HERE,'..','contract-colour-partition.json'),'w'),indent=1)
import collections; print(len(mach), collections.Counter(m['tier'] for m in mach))
