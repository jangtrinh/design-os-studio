"""Spec T: colour partition and registry record bindings (fix round 2).

Reads only: specs/token-contract.md (a) and (b), specs/C-widget-kit-specimen-spec.py (imported as a module,
its __main__ block is never run), evidence/t-ontology/contract-colour-classes.json (R26 classes).
Imported by emit-registry-tables.py; build() returns (records, partition).
"""
import collections, importlib.util, json, os, re, sys
sys.dont_write_bytecode = True  # imports here must not write caches into specs/ or scripts/

HERE = os.path.dirname(os.path.abspath(__file__))
PLAN = os.path.normpath(os.path.join(HERE, '..', '..', '..'))
CONTRACT = os.path.join(PLAN, 'specs', 'token-contract.md')
CSPEC = os.path.join(PLAN, 'specs', 'C-widget-kit-specimen-spec.py')
CLASSES = os.path.join(HERE, '..', 'contract-colour-classes.json')

V2 = {'UTILITY_NEUTRAL_50': 'BADGE_NEUTRAL_BG', 'UTILITY_NEUTRAL_200': 'BADGE_NEUTRAL_BORDER', 'UTILITY_NEUTRAL_700': 'BADGE_NEUTRAL_TEXT',
      'UTILITY_GREEN_200': 'BADGE_SUCCESS_BORDER', 'UTILITY_YELLOW_200': 'BADGE_WARNING_BORDER', 'UTILITY_RED_200': 'BADGE_ERROR_BORDER',
      'UTILITY_GREEN_700': 'BADGE_SUCCESS_TEXT', 'UTILITY_YELLOW_700': 'BADGE_WARNING_TEXT', 'UTILITY_RED_700': 'BADGE_ERROR_TEXT',
      'UTILITY_BLUE_50': 'TILE_DESIGN_BG', 'UTILITY_BLUE_600': 'TILE_DESIGN_GLYPH', 'UTILITY_AMBER_50': 'TILE_BOARD_BG', 'UTILITY_AMBER_600': 'TILE_BOARD_GLYPH',
      'UTILITY_GREEN_50': 'TILE_DECK_BG', 'UTILITY_GREEN_600': 'TILE_DECK_GLYPH', 'UTILITY_ORANGE_50': 'TILE_SPEC_BG', 'UTILITY_ORANGE_600': 'TILE_SPEC_GLYPH'}
G01 = {'BG_PRIMARY': 'bg', 'BG_SECONDARY': 'surface', 'BG_TERTIARY': 'secondary', 'BG_HOVER': 'bg-hover', 'BG_PRESSED': 'secondary-active', 'BORDER': 'border',
       'BORDER_SUBTLE': 'border-subtle', 'BORDER_STRONG': 'border-strong', 'BG_BRAND': 'primary', 'COMPONENT': 'component', 'CANVAS_COMPONENT': 'component',
       'DEV_COMPLETED': 'component', 'AGENT_1': 'agent-1', 'AGENT_2': 'agent-2', 'AGENT_3': 'agent-3', 'AGENT_4': 'agent-4', 'INFO': 'info', 'SUCCESS': 'success',
       'WARNING': 'warning', 'BG_SELECTED': 'bg-active', 'BG': 'canvas', 'TEXT': 'bg-foreground', 'TEXT_TERTIARY': 'muted-foreground', 'BG_BRAND_HOVER': 'primary-hover',
       'BG_BRAND_PRESSED': 'primary-active', 'TEXT_ON_BRAND': 'primary-foreground', 'FOCUS_RING': 'ring', 'BORDER_SELECTED': 'border-focus', 'BG_TEXT_SELECTION': 'accent',
       'TEXT_BRAND': 'accent-foreground', 'BG_MENU': 'popover', 'BG_ERROR_SOLID': 'destructive'}
# a constant whose value and role several keys share: the key that names its use (hover on bg-secondary, not the selected fill)
SHARED_PREF = {'BG_SECONDARY_HOVER': 'colors.surface-hover'}


def cells(line):
    return [x.strip() for x in line.strip().strip('|').split('|')]


def first_role(text):
    m = re.match(r'`?([A-Za-z][A-Za-z0-9_-]*)', text)
    return m.group(1) if m else None


def norm_value(raw):
    """Full contract value: #RRGGBB, #RRGGBBAA (rgba), a constant name (alias), or the literal text."""
    raw = raw.strip().strip('`')
    m = re.fullmatch(r'rgb\(0x([0-9A-Fa-f]{6})\)', raw)
    if m:
        return '#' + m.group(1).upper()
    m = re.fullmatch(r'rgba\(0x([0-9A-Fa-f]{6}),\s*0x([0-9A-Fa-f]{2})\)', raw)
    if m:
        return '#' + (m.group(1) + m.group(2)).upper()
    if re.fullmatch(r'[A-Z][A-Z0-9_]+', raw):
        return ('alias', raw)
    return raw


def parse_contract():
    tc = open(CONTRACT, encoding='utf-8').read().splitlines()
    raw, role, deleted = {}, {}, set()
    a_end = next(i for i, l in enumerate(tc) if l.startswith('## (b)'))
    for l in tc[:a_end]:
        m = re.match(r'^\| *(N?\d+) *\|', l)
        if not m:
            continue
        c = cells(l)
        name = re.search(r'([A-Z][A-Z0-9_]+)', c[1]).group(1)
        if m.group(1).startswith('N'):
            val, rl = c[2], c[3]
        else:
            act = c[3]
            if act == 'delete':
                deleted.add(name)
                continue
            val = c[4] if act == 'revalue' else c[2]
            rl = c[5]
        raw[name] = norm_value(val)
        role[name] = first_role(rl)
    value = {}

    def resolve(n, seen=()):
        v = raw[n]
        if isinstance(v, tuple):
            assert v[1] not in seen, n
            return resolve(v[1], seen + (n,))
        return v
    for n in raw:
        value[n] = resolve(n)
    for n, v in raw.items():  # an alias row carries its target's role (contract cell "= BG_SELECTED")
        t = n
        while isinstance(raw[t], tuple):
            t = raw[t][1]
        if t != n and (role[n] is None or role[n] == 'alias'):
            role[n] = role[t]
    for old, new in V2.items():
        value[new], role[new] = value[old], role[old]
    store = {}
    b_start = a_end
    for l in tc[b_start:]:
        if l.startswith('## (c)'):
            break
        c = cells(l)
        if len(c) >= 4 and c[0].startswith('`colors.'):
            key, new = c[0].strip('`'), c[2].strip('`')
            kept = new == 'unchanged'
            if kept:  # an unchanged key keeps its HEAD value (column 2)
                new = c[1].strip('`')
            rl = first_role(c[3])
            owners = [n for n, k in G01.items() if 'colors.' + k == key]
            if kept and owners:  # "Studio-only role kept": the key carries the role of the constant G01 maps to it
                rl = role[owners[0]]
            store[key] = (new.upper() if new.startswith('#') else None, rl)
    return value, role, deleted, store


VALUE, ROLE, DELETED, STORE = parse_contract()
KEY_OWNER = {'colors.' + k: n for n, k in G01.items()}


def shared_key(n):
    """A store key with the same 6-digit value and the same role, or None."""
    if n in SHARED_PREF:
        k = SHARED_PREF[n]
        assert STORE[k][0] == VALUE[n] and STORE[k][1] == ROLE[n], n
        return k
    hits = [k for k, (v, r) in STORE.items() if v and v == VALUE[n] and r == ROLE[n]]
    hits.sort(key=lambda k: (k not in KEY_OWNER, k))
    return hits[0] if hits else None


def partition():
    cls = {r[0]: r[1] for r in json.load(open(CLASSES))}
    for n in ('FOCUS_RING_ERROR', 'BORDER_ERROR_SUBTLE', 'BORDER_ERROR', 'TEXT_ERROR_HOVER'):
        cls[n] = 'untitled semantic'  # contract rows N74-N77, added after R26
    out = {}
    for n, c in sorted(cls.items()):
        if c == 'deleted':
            continue
        k = V2.get(n, n)
        rec = {'value': VALUE[n], 'role': ROLE[n], 'contract_name': n if k != n else None}
        if n in G01:
            rec.update({'class': 'mapped', 'key': 'colors.' + G01[n]})
        elif c.startswith('exempt') or c.startswith('alias'):
            rec.update({'class': 'exempt', 'reason': c})
        elif shared_key(n):
            rec.update({'class': 'shared-key', 'key': shared_key(n)})
        else:
            rec.update({'class': 'pending', 'path': 'colors.' + k.lower().replace('_', '-')})
        out[k] = rec
    return out


# ---- records -------------------------------------------------------------------------------
ROLE_CONST = {'bg-primary': 'BG', 'bg-primary_hover': 'BG_HOVER', 'bg-quaternary': 'BG_PRESSED', 'bg-secondary_hover': 'BG_SELECTED', 'bg-secondary': 'BG_SECONDARY',
              'bg-tertiary': 'BG_TERTIARY', 'bg-primary_alt': 'BG_PRIMARY_ALT', 'bg-primary-solid': 'BG_PRIMARY_SOLID', 'bg-overlay': 'BG_OVERLAY',
              'border-primary': 'BORDER_STRONG', 'border-secondary': 'BORDER', 'border-secondary_alt': 'BORDER_MENU', 'border-brand': 'BORDER_SELECTED',
              'fg-quaternary': 'ICON_QUATERNARY', 'fg-quaternary_hover': 'ICON_QUATERNARY_HOVER', 'fg-secondary': 'ICON_SECONDARY', 'fg-secondary_hover': 'ICON_SECONDARY_HOVER',
              'fg-primary': 'ICON', 'fg-white': 'ICON_WHITE', 'fg-brand-primary': 'ICON_BRAND_PRIMARY', 'fg-error-primary': 'DANGER', 'fg-success-primary': 'SUCCESS',
              'focus-ring': 'FOCUS_RING', 'focus-ring-error': 'FOCUS_RING_ERROR', 'text-primary': 'TEXT', 'text-secondary': 'TEXT_SECONDARY',
              'text-secondary_hover': 'TEXT_SECONDARY_HOVER', 'text-tertiary': 'TEXT_TERTIARY', 'text-tertiary_hover': 'TEXT_TERTIARY_HOVER', 'text-quaternary': 'TEXT_QUATERNARY',
              'text-placeholder': 'TEXT_PLACEHOLDER', 'text-white': 'TEXT_WHITE', 'Neutral-950': 'TEXT_ON_BRAND', 'bg-brand-solid': 'BG_BRAND',
              'bg-brand-solid_hover': 'BG_BRAND_HOVER', 'Orange-600': 'BG_BRAND_PRESSED', 'bg-error-solid': 'BG_ERROR_SOLID', 'bg-error-solid_hover': 'BG_ERROR_SOLID_HOVER',
              'Red-800': 'BG_ERROR_SOLID_PRESSED', 'bg-error-primary': 'BG_ERROR_PRIMARY', 'border-error': 'BORDER_ERROR', 'border-error_subtle': 'BORDER_ERROR_SUBTLE',
              'text-error-primary': 'TEXT_ERROR', 'text-error-primary_hover': 'TEXT_ERROR_HOVER', 'utility-neutral-50': 'BADGE_NEUTRAL_BG',
              'utility-neutral-200': 'BADGE_NEUTRAL_BORDER', 'utility-neutral-700': 'BADGE_NEUTRAL_TEXT', 'alpha-black-10': 'BORDER_TRANSLUCENT'}
# roles named in a parts or geometry line that the widget does not paint (a contrast note, or an alternative C rejects)
PARTS_SKIP = {('Button/SecondaryDestructive', 'bg-error-solid_hover'): 'conditional alternative, not the ruled look (C-1.65)',
              ('Button/SecondaryDestructive', 'text-white'): 'conditional alternative, not the ruled look (C-1.65)',
              ('Checkbox/Box', 'bg-secondary'): 'contrast note, not a paint',
              ('Checkbox/Box', 'bg-primary'): 'contrast note, not a paint'}
EXTRA = {'Badge/StatusCount': ['BG_SUCCESS_PRIMARY', 'BADGE_SUCCESS_BORDER', 'BADGE_SUCCESS_TEXT', 'BG_WARNING_PRIMARY', 'BADGE_WARNING_BORDER', 'BADGE_WARNING_TEXT',
                               'BG_ERROR_PRIMARY', 'BADGE_ERROR_BORDER', 'BADGE_ERROR_TEXT'],  # C 3.3 status colours (token-contract N24-N32)
         'Toast/Default': ['BG_SECONDARY_HOVER']}  # action hover on the toast (C rule S6, ruling C-R)
MANUAL = {'Focus/Ring': ['FOCUS_RING', 'FOCUS_RING_ERROR'], 'Icon/Glyph': ['ICON_QUATERNARY', 'ICON_QUATERNARY_HOVER', 'ICON_SECONDARY_HOVER'],
          'Status/Dot': ['ICON_BRAND_PRIMARY', 'DANGER', 'SUCCESS'],
          'Icon/Tile': ['TILE_DESIGN_BG', 'TILE_DESIGN_GLYPH', 'TILE_BOARD_BG', 'TILE_BOARD_GLYPH', 'TILE_DECK_BG', 'TILE_DECK_GLYPH', 'TILE_SPEC_BG', 'TILE_SPEC_GLYPH', 'BORDER_STRONG', 'ICON_QUATERNARY'],
          'Card/FileThumbnail': ['BG_SECONDARY', 'TEXT', 'TEXT_TERTIARY', 'BG_SECONDARY_HOVER', 'BORDER', 'BORDER_SELECTED', 'HOME_THUMB_BG'],
          'Card/StarterTemplate': ['BG_SECONDARY', 'TEXT', 'BORDER'], 'Card/Library': ['BG_SECONDARY', 'BORDER'], 'Card/Approval': ['BG_SECONDARY', 'BORDER'],
          'Card/AgentTurn': ['BG_SECONDARY', 'BORDER'], 'Banner/Inline': ['BG_SECONDARY', 'BG_SUCCESS_PRIMARY', 'BG_WARNING_PRIMARY', 'BG_ERROR_PRIMARY', 'TEXT', 'BORDER'],
          'Table/Data': ['BG_PRIMARY', 'BG_HOVER', 'BORDER'], 'Code/Block': ['BG_SECONDARY', 'BORDER'], 'Input/Composer': ['BG_SECONDARY', 'BORDER', 'FOCUS_RING'],
          'Popover/ColorPicker': ['BG_MENU', 'BORDER_MENU'], 'Bar/Contextual': ['BG_TOOLBAR', 'INNER_HIGHLIGHT', 'BORDER']}
RAD = {'radius.radius-md': ['Button/Primary', 'Button/Secondary', 'Button/Tertiary', 'Button/Destructive', 'Button/SecondaryDestructive', 'Button/Icon', 'Button/Tool',
                            'Button/Close', 'Input/Field', 'Input/PropertyField', 'Input/Search', 'Select/Trigger', 'Menu/Item', 'Tabs/Pill', 'Tag/Chip', 'Tooltip/Default',
                            'Tree/Row', 'List/Row', 'Loading/Inline', 'EmptyState/Default', 'Tabs/Segmented', 'Icon/Tile'],
       'radius.radius-sm': ['Tabs/Segmented'], 'radius.radius-pill': ['Toggle/Switch', 'Badge/StatusCount', 'Slider/Handle', 'Progress/Bar'],
       'radius.radius-xs': ['Checkbox/Box'], 'radius.radius-xl': ['Radio/Card', 'Dialog/CommandPalette', 'Toast/Default', 'Card/Approval'],
       'RADIUS_FRAME': ['Menu/Panel', 'Menu/Context', 'Popover/Panel', 'Dialog/Modal', 'Popover/ColorPicker']}
OTHER = {'Menu/Shortcut': ['typography.font-size-caption'], 'Badge/StatusCount': ['typography.font-size-caption'], 'Panel/Header': ['typography.font-size-heading'],
         'Input/PropertyField': ['typography.font-family-mono'], 'Code/Block': ['typography.font-family-mono'], 'Menu/Panel': ['spacing.space-2'], 'Menu/Context': ['spacing.space-2']}
FAMILY_KEY = {'input': {'bg-primary': 'colors.input'}, 'menu': {'bg-primary': 'colors.popover', 'text-primary': 'colors.popover-foreground'},
              'card': {'bg-secondary': 'colors.card', 'text-primary': 'colors.card-foreground'}, 'default': {'bg-primary': 'colors.bg'}}
CSTATE = {'Default': 'default', 'Hover': 'hover', 'Pressed': 'active', 'Focused': 'focus', 'Disabled': 'disabled'}


def family(n):
    f = n.split('/')[0]
    if f in ('Input', 'Select'):
        return 'input'
    if f in ('Menu', 'Popover') or n == 'Dialog/CommandPalette':
        return 'menu'
    return 'card' if f == 'Card' else 'default'


def load_c():
    spec = importlib.util.spec_from_file_location('c_specimen', CSPEC)
    mod = importlib.util.module_from_spec(spec)
    argv = sys.argv
    sys.argv = [CSPEC]
    try:
        spec.loader.exec_module(mod)  # defines W, TOK, resolve, look; the __main__ block does not run
    finally:
        sys.argv = argv
    return mod


def c_widgets(mod):
    roles_by_len = sorted(mod.TOK, key=len, reverse=True)
    out = {}
    for w in mod.W:
        n, painted, declared, selected = w['name'], set(), [], False
        for st in mod.STATES:
            v = mod.resolve(w, st)
            if v == 'n/a':
                continue
            if st == 'Selected':
                selected = True
            else:
                declared.append(CSTATE[st])
            kind = v[0] if isinstance(v, tuple) and v[0] in ('ring', 'half') else 'plain'
            fill, border, _bw, txt, ic = mod.look(v[1] if kind != 'plain' else v)
            painted |= {x for x in (fill, border, txt, ic) if x}
            if kind == 'ring':
                painted.add(v[2] if len(v) > 2 else 'focus-ring')
        text = (w.get('parts') or '') + ' ' + (w.get('geom') or '')
        parts = set()
        for r in roles_by_len:
            if re.search(r'(?<![\w-])' + re.escape(r) + r'(?![\w-])', text) and (n, r) not in PARTS_SKIP:
                parts.add(r)
        out[n] = {'states': declared, 'selected': selected, 'state_roles': sorted(painted), 'parts_roles': sorted(parts - painted)}
    return out


def bind(name, const):
    """(constant, store key or None): family key, then the G01 key, then a key with the same value and role."""
    if family(name) == 'menu' and const == 'BG':
        const = 'BG_MENU'
    val, role = VALUE[const], ROLE[const]
    k = FAMILY_KEY.get(family(name), {}).get(role)
    if k and STORE[k][0] == val:
        return const, k
    if const in G01:
        return const, 'colors.' + G01[const]
    return const, shared_key(const)


def build():
    part = partition()
    mod = load_c()
    cw = c_widgets(mod)
    recs = {}
    for n in list(cw) + list(MANUAL):
        consts = []
        if n in cw:
            consts += [ROLE_CONST[r] for r in cw[n]['state_roles'] + cw[n]['parts_roles']]
        consts += EXTRA.get(n, []) + MANUAL.get(n, [])
        tokens, pending, paints = [], [], []
        for c in dict.fromkeys(consts):
            c2, k = bind(n, c)
            if c2 not in paints:
                paints.append(c2)
            if k:
                assert STORE[k][0] in (VALUE[c2], None), (n, c2, k)
                tokens.append(k)
            else:
                assert part[c2]['class'] == 'pending', (n, c2, part[c2]['class'])
                pending.append(c2)
        for k, names in RAD.items():
            if n in names:
                (tokens if k.startswith('radius.') else pending).append(k)
                if not k.startswith('radius.'):
                    paints.append(k)
        tokens += OTHER.get(n, [])
        recs[n] = {'tokens': sorted(dict.fromkeys(tokens)), 'pending': pending, 'paints': paints,
                   'paints_from_parts_line': [ROLE_CONST[r] for r in cw[n]['parts_roles']] if n in cw else [],
                   'states': cw[n]['states'] if n in cw else None, 'selected': cw[n]['selected'] if n in cw else False}
    return recs, part
