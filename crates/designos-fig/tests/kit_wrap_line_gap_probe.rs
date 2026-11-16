//! Reverse-engineering probe (ignored by default): measures how Figma resolves
//! the line gap of wrapped auto-layout stacks (`stackWrap == "WRAP"`, non-grid)
//! directly from the raw `NodeChange` records and stored child geometry in the
//! Untitled UI PRO v8.0 kit (`KIT_CLONE` or `DESIGNOS_KIT_PATH`).

use designos_doc::*;
use designos_fig::container;
use designos_fig::kiwi::{self, KiwiValue, Schema};
use designos_fig::{export_fig, export_rest_json, import_fig_node_changes, import_rest_json};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};

fn fnum(v: &Value, k: &str) -> Option<f32> {
    v.get(k).and_then(Value::as_f64).map(|x| x as f32)
}

fn fstr<'a>(v: &'a Value, k: &str) -> Option<&'a str> {
    v.get(k).and_then(Value::as_str)
}

fn fbool(v: &Value, k: &str) -> Option<bool> {
    v.get(k).and_then(Value::as_bool)
}

fn guid_str(v: &Value) -> Option<String> {
    let s = v.get("sessionID")?.as_u64()?;
    let l = v.get("localID")?.as_u64()?;
    if s == u64::from(u32::MAX) && l == u64::from(u32::MAX) {
        return None;
    }
    Some(format!("{s}:{l}"))
}

fn guid_path(o: &Value) -> Vec<String> {
    let p = o.get("guidPath");
    let list = p.and_then(|p| p.get("guids")).or(p).and_then(Value::as_array);
    list.map(|a| a.iter().filter_map(guid_str).collect()).unwrap_or_default()
}

fn kiwi_guid_str(g: &KiwiValue) -> Option<String> {
    let u64_of = |v: &KiwiValue| match v {
        KiwiValue::Uint(u) => Some(*u as u64),
        KiwiValue::Uint64(u) => Some(*u),
        KiwiValue::Int(i) if *i >= 0 => Some(*i as u64),
        KiwiValue::Int64(i) if *i >= 0 => Some(*i as u64),
        _ => None,
    };
    let (s, l) = (u64_of(g.get("sessionID")?)?, u64_of(g.get("localID")?)?);
    if s == u64::from(u32::MAX) && l == u64::from(u32::MAX) {
        return None;
    }
    Some(format!("{s}:{l}"))
}

fn kiwi_f32(v: &KiwiValue, k: &str) -> Option<f32> {
    match v.get(k)? {
        KiwiValue::Float(f) => f.is_finite().then_some(*f),
        KiwiValue::Uint(u) => Some(*u as f32),
        KiwiValue::Int(i) => Some(*i as f32),
        _ => None,
    }
}

fn kiwi_str(v: &KiwiValue) -> Option<&str> {
    match v {
        KiwiValue::String(s) => Some(s.as_str()),
        KiwiValue::Enum(s) => Some(s),
        _ => None,
    }
}

#[derive(Debug, Clone)]
struct DerivedEntry {
    path: Vec<String>,
    size: Option<Vec2>,
    transform: Option<Affine>,
}

#[derive(Debug, Clone)]
struct ChildBox {
    main_start: f32,
    main_end: f32,
    cross_start: f32,
    cross_end: f32,
}

#[derive(Debug, Clone)]
struct LineSpan {
    cross_start: f32,
    cross_end: f32,
    count: usize,
}

impl LineSpan {
    fn cross_extent(&self) -> f32 {
        self.cross_end - self.cross_start
    }
}

fn child_bbox(transform: Affine, size: Vec2) -> [f32; 4] {
    let corners = [
        transform.apply(Vec2::new(0.0, 0.0)),
        transform.apply(Vec2::new(size.x, 0.0)),
        transform.apply(Vec2::new(0.0, size.y)),
        transform.apply(Vec2::new(size.x, size.y)),
    ];
    let (mut x0, mut y0, mut x1, mut y1) = (f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
    for p in corners {
        x0 = x0.min(p.x);
        y0 = y0.min(p.y);
        x1 = x1.max(p.x);
        y1 = y1.max(p.y);
    }
    [x0, y0, x1, y1]
}

fn node_affine(nc: &Value) -> Affine {
    if let Some(t) = nc.get("transform") {
        let g = |k: &str, d: f32| fnum(t, k).unwrap_or(d);
        Affine { a: g("m00", 1.0), c: g("m01", 0.0), tx: g("m02", 0.0), b: g("m10", 0.0), d: g("m11", 1.0), ty: g("m12", 0.0) }
    } else {
        Affine::IDENTITY
    }
}

fn node_size(nc: &Value) -> Vec2 {
    nc.get("size").map(|s| Vec2::new(fnum(s, "x").unwrap_or(0.0), fnum(s, "y").unwrap_or(0.0))).unwrap_or(Vec2::ZERO)
}

/// Groups ordered flow children into lines based on stored geometry:
/// when a child wraps to a new line, its main-axis start resets to the left/top of the previous child's main-axis end
/// (or when item_spacing is negative, resets back to <= the previous child's main-axis start while advancing on the counter axis).
fn group_into_lines(children: &[ChildBox], item_spacing: f32) -> Vec<LineSpan> {
    let mut lines: Vec<LineSpan> = Vec::new();
    let mut prev: Option<&ChildBox> = None;
    for c in children {
        let starts_new_line = match prev {
            None => true,
            Some(p) => {
                if item_spacing >= 0.0 {
                    c.main_start < p.main_end - 0.5
                } else {
                    c.main_start < p.main_end + item_spacing - 0.5 || (c.main_start <= p.main_start + 0.5 && c.cross_start > p.cross_start + 0.5)
                }
            }
        };
        if starts_new_line {
            lines.push(LineSpan { cross_start: c.cross_start, cross_end: c.cross_end, count: 1 });
        } else if let Some(last) = lines.last_mut() {
            last.cross_start = last.cross_start.min(c.cross_start);
            last.cross_end = last.cross_end.max(c.cross_end);
            last.count += 1;
        }
        prev = Some(c);
    }
    lines
}

fn padding_of(nc: &Value) -> [f32; 4] {
    let p = fnum(nc, "stackPadding");
    let h = fnum(nc, "stackHorizontalPadding").or(p);
    let v = fnum(nc, "stackVerticalPadding").or(p);
    let uniform_if_absent = |side_field: &str, uniform: Option<f32>| {
        if fnum(nc, side_field).is_some() {
            None
        } else {
            uniform
        }
    };
    let left = fnum(nc, "stackPaddingLeft").or(h).unwrap_or(0.0);
    let top = fnum(nc, "stackPaddingTop").or(v).unwrap_or(0.0);
    let right = fnum(nc, "stackPaddingRight").or(uniform_if_absent("stackHorizontalPadding", p)).unwrap_or(0.0);
    let bottom = fnum(nc, "stackPaddingBottom").or(uniform_if_absent("stackVerticalPadding", p)).unwrap_or(0.0);
    [top, right, bottom, left]
}

#[test]
#[ignore = "reads KIT_CLONE or DESIGNOS_KIT_PATH env var"]
fn probe_kit_wrapped_auto_layout_line_gap() {
    let kit_path = std::env::var_os("KIT_CLONE").or_else(|| std::env::var_os("DESIGNOS_KIT_PATH")).expect("set KIT_CLONE or DESIGNOS_KIT_PATH");
    let bytes = std::fs::read(&kit_path).expect("read kit file");

    let mut archive = container::open(&bytes).expect("open container");
    let (schema_bin, msg_bin) = container::read_archive_chunks(&archive).expect("read chunks");
    archive.canvas = Vec::new();
    let schema = Schema::decode(&schema_bin).expect("decode schema");
    drop(schema_bin);

    // 5. Inspect Kiwi schema in the kit itself
    println!("=== 5. KIWI SCHEMA IN KIT ===");
    if let Some(nc_idx) = schema.find("NodeChange") {
        let def = &schema.defs[nc_idx];
        println!(
            "  NodeChange kind: {:?} (in Kiwi, Message fields are optional tag-value pairs with no schema default; absent = not encoded on wire)",
            def.kind
        );
        for f in &def.fields {
            let name = &*f.name;
            if name.to_ascii_lowercase().contains("stack")
                || name.to_ascii_lowercase().contains("counter")
                || name.to_ascii_lowercase().contains("wrap")
                || name.to_ascii_lowercase().contains("gap")
                || name.to_ascii_lowercase().contains("spacing")
            {
                println!("  NodeChange.{name} = tag {} : ty {:?} (array={})", f.value, f.ty, f.is_array);
            }
        }
    }
    for enum_name in ["StackWrap", "StackCounterAlignContent", "StackMode", "VariableField"] {
        if let Some(idx) = schema.find(enum_name) {
            let def = &schema.defs[idx];
            let fields: Vec<String> = def
                .fields
                .iter()
                .filter(|f| {
                    enum_name != "VariableField"
                        || f.name.contains("STACK")
                        || f.name.contains("COUNTER")
                        || f.name.contains("GAP")
                        || f.name.contains("SPACING")
                })
                .map(|f| format!("{}={}", f.name, f.value))
                .collect();
            println!("  Enum {enum_name}: {}", fields.join(", "));
        }
    }

    let root = schema.root_message().expect("root message");
    let mut message = schema.decode_message_for_import(root, &msg_bin).expect("decode message");
    drop(msg_bin);

    let raw_changes: Vec<KiwiValue> = match message.take_field("nodeChanges") {
        Some(KiwiValue::Array(arr)) => arr,
        _ => Vec::new(),
    };
    println!("Total raw NodeChange records in kit: {}", raw_changes.len());

    // Extract derivedSymbolData before converting each NodeChange to JSON via into_json_fast
    let mut derived_by_inst: HashMap<String, Vec<DerivedEntry>> = HashMap::new();
    let mut nodes: Vec<Value> = Vec::with_capacity(raw_changes.len());
    for mut nc in raw_changes {
        if nc.get("type").and_then(kiwi_str) == Some("INSTANCE") {
            if let Some(id) = nc.get("guid").and_then(kiwi_guid_str) {
                let derived_val = nc.take_field("derivedSymbolData").or_else(|| nc.get_mut("symbolData").and_then(|s| s.take_field("derivedSymbolData")));
                if let Some(KiwiValue::Array(arr)) = derived_val {
                    let mut entries = Vec::with_capacity(arr.len());
                    for o in arr {
                        let p = o.get("guidPath");
                        let list = p.and_then(|p| p.get("guids")).or(p).and_then(KiwiValue::as_array);
                        let path: Vec<String> = list.map(|a| a.iter().filter_map(kiwi_guid_str).collect()).unwrap_or_default();
                        if path.is_empty() {
                            continue;
                        }
                        let size = o.get("size").and_then(|s| Some(Vec2::new(kiwi_f32(s, "x")?, kiwi_f32(s, "y")?)));
                        let transform = o.get("transform").map(|t| {
                            let g = |k: &str, d: f32| kiwi_f32(t, k).unwrap_or(d);
                            Affine { a: g("m00", 1.0), c: g("m01", 0.0), tx: g("m02", 0.0), b: g("m10", 0.0), d: g("m11", 1.0), ty: g("m12", 0.0) }
                        });
                        entries.push(DerivedEntry { path, size, transform });
                    }
                    if !entries.is_empty() {
                        derived_by_inst.insert(id, entries);
                    }
                }
            }
        }
        nodes.push(kiwi::into_json_fast(nc));
    }

    // Index nodes by guid and overrideKey, and children by parent guid
    let mut by_guid: HashMap<String, usize> = HashMap::with_capacity(nodes.len());
    let mut by_override_key: HashMap<String, usize> = HashMap::new();
    let mut children_by_parent: HashMap<String, Vec<(String, usize)>> = HashMap::new();
    let mut any_align_content_in_kit = 0usize;

    for (idx, nc) in nodes.iter().enumerate() {
        if fstr(nc, "phase") == Some("REMOVED") || fbool(nc, "isSoftDeleted") == Some(true) {
            continue;
        }
        if nc.get("stackCounterAlignContent").is_some() {
            any_align_content_in_kit += 1;
        }
        let Some(g) = nc.get("guid").and_then(guid_str) else { continue };
        by_guid.insert(g.clone(), idx);
        if let Some(ok) = nc.get("overrideKey").and_then(guid_str) {
            by_override_key.insert(ok, idx);
        }
        if let Some(pi) = nc.get("parentIndex") {
            if let Some(pg) = pi.get("guid").and_then(guid_str) {
                let pos = fstr(pi, "position").unwrap_or("").to_string();
                children_by_parent.entry(pg).or_default().push((pos, idx));
            }
        }
    }
    for kids in children_by_parent.values_mut() {
        kids.sort_by(|a, b| a.0.cmp(&b.0));
    }
    println!("Any NodeChange in kit with stackCounterAlignContent present (wrapped or not): {any_align_content_in_kit}");

    // Helper to get flow children of a node:
    // 1. Direct NodeChange children (ordered by parentIndex.position)
    // 2. If it's an INSTANCE with 0 direct children, from derivedSymbolData + master SYMBOL children
    //    (only when derivedSymbolData actually carries entries for the direct children).
    let flow_children_of = |idx: usize, horiz: bool| -> (Vec<ChildBox>, &'static str) {
        let nc = &nodes[idx];
        let g = nc.get("guid").and_then(guid_str).unwrap_or_default();
        let mut out = Vec::new();
        if let Some(kids) = children_by_parent.get(&g) {
            for (_pos, cidx) in kids {
                let c = &nodes[*cidx];
                if fbool(c, "visible") == Some(false) || fstr(c, "stackPositioning") == Some("ABSOLUTE") {
                    continue;
                }
                let b = child_bbox(node_affine(c), node_size(c));
                let (main_start, main_end, cross_start, cross_end) = if horiz { (b[0], b[2], b[1], b[3]) } else { (b[1], b[3], b[0], b[2]) };
                out.push(ChildBox { main_start, main_end, cross_start, cross_end });
            }
        }
        if !out.is_empty() {
            return (out, "direct");
        }
        if fstr(nc, "type") == Some("INSTANCE") {
            let sym_guid = nc.get("symbolData").and_then(|s| s.get("symbolID")).and_then(guid_str);
            let ovs = nc.get("symbolData").and_then(|s| s.get("symbolOverrides")).and_then(Value::as_array);
            if let (Some(sg), Some(derived)) = (sym_guid, derived_by_inst.get(&g)) {
                if let Some(sym_kids) = children_by_parent.get(&sg) {
                    for (_pos, sc_idx) in sym_kids {
                        let sc = &nodes[*sc_idx];
                        let sc_guid = sc.get("guid").and_then(guid_str).unwrap_or_default();
                        let sc_ok = sc.get("overrideKey").and_then(guid_str);
                        let mut vis = fbool(sc, "visible").unwrap_or(true);
                        if let Some(ov_list) = ovs {
                            for ov in ov_list {
                                let p = guid_path(ov);
                                if p.len() == 1 && (p[0] == sc_guid || sc_ok.as_deref() == Some(p[0].as_str())) {
                                    if let Some(v) = fbool(ov, "visible") {
                                        vis = v;
                                    }
                                }
                            }
                        }
                        if !vis || fstr(sc, "stackPositioning") == Some("ABSOLUTE") {
                            continue;
                        }
                        // Only use direct NodeChange children for raw NodeChange census;
                        // instance derivedSymbolData is analyzed separately in Section 3.
                        let _ = (sc_guid, sc_ok, vis, ovs, derived);
                    }
                }
            }
        }
        (out, "none")
    };

    // Inspect 9291:552366, 9291:558693, 9292:559708
    for check_g in ["9291:552366", "9291:558693", "9292:559708"] {
        if let Some(&idx) = by_guid.get(check_g) {
            let nc = &nodes[idx];
            let kids = children_by_parent.get(check_g).map(|v| v.len()).unwrap_or(0);
            let d_len = derived_by_inst.get(check_g).map(|v| v.len()).unwrap_or(0);
            println!(
                "Inspect {check_g}: type={:?} name={:?} size={:?} sp={:?} csp={:?} direct_kids={kids} derived_entries={d_len}",
                fstr(nc, "type"),
                fstr(nc, "name"),
                nc.get("size"),
                fnum(nc, "stackSpacing"),
                fnum(nc, "stackCounterSpacing")
            );
            if let Some(klist) = children_by_parent.get(check_g) {
                for (pos, cidx) in klist {
                    let c = &nodes[*cidx];
                    println!(
                        "  direct child of {check_g}: pos={pos:?} guid={:?} ok={:?} name={:?} vis={:?} size={:?} transform={:?}",
                        c.get("guid").and_then(guid_str),
                        c.get("overrideKey").and_then(guid_str),
                        fstr(c, "name"),
                        fbool(c, "visible"),
                        c.get("size"),
                        c.get("transform")
                    );
                }
            }
            if let Some(dlist) = derived_by_inst.get(check_g) {
                for de in dlist.iter().take(6) {
                    println!("  derived entry of {check_g}: path={:?} size={:?} transform={:?}", de.path, de.size, de.transform);
                }
            }
        }
    }

    // 1. Census of wrapped stacks
    println!("\n=== 1. CENSUS OF WRAPPED STACKS ===");
    let mut wrap_with_grid_or_none = 0usize;
    let mut total_wrapped_stacks = 0usize;
    let mut count_a_present = 0usize;
    let mut count_b_absent = 0usize;
    let mut count_c_space_between = 0usize;
    let mut count_d_multi_line = 0usize;

    let mut count_a_and_c = 0usize;
    let mut count_b_and_c = 0usize;
    let mut count_a_align_auto = 0usize;
    let mut count_a_align_absent = 0usize;
    let mut count_b_align_auto = 0usize;
    let mut count_b_align_absent = 0usize;

    let mut count_a_multi_line = 0usize;
    let mut count_b_multi_line = 0usize;
    let mut count_c_multi_line = 0usize;
    let mut count_a_not_c_multi_line = 0usize;
    let mut count_b_not_c_multi_line = 0usize;
    let mut count_a_c_multi_line = 0usize;
    let mut count_b_c_multi_line = 0usize;

    let mut count_a_1_line = 0usize;
    let mut count_a_0_line = 0usize;
    let mut count_b_1_line = 0usize;
    let mut count_b_0_line = 0usize;

    let mut count_a_zero = 0usize;
    let mut count_a_eq_item = 0usize;
    let mut count_a_ne_item = 0usize;
    let mut count_b_item_zero_or_absent = 0usize;
    let mut count_b_item_nonzero = 0usize;
    let mut count_a_var_sp = 0usize;
    let mut count_a_var_csp = 0usize;
    let mut count_b_var_sp = 0usize;
    let mut count_b_var_csp = 0usize;
    let mut by_type_a: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_type_b: BTreeMap<String, usize> = BTreeMap::new();

    let has_var_field = |v: &Value, field_name: &str| -> bool {
        v.get("variableConsumptionMap")
            .and_then(|m| m.get("entries"))
            .and_then(Value::as_array)
            .is_some_and(|arr| arr.iter().any(|e| fstr(e, "variableField") == Some(field_name)))
    };

    #[derive(Debug)]
    struct MultiLineCase {
        guid: String,
        name: String,
        ty: String,
        mode: String,
        counter_sizing: String,
        child_source: &'static str,
        frame_size: Vec2,
        padding: [f32; 4],
        stack_spacing: Option<f32>,
        stack_counter_spacing: Option<f32>,
        align_content: Option<String>,
        lines: Vec<LineSpan>,
        gaps: Vec<f32>,
        pred_item: f32,
        pred_counter_or_zero: f32,
        pred_space_between: f32,
    }

    let mut multi_cases_a_not_c: Vec<MultiLineCase> = Vec::new();
    let mut multi_cases_b_not_c: Vec<MultiLineCase> = Vec::new();
    let mut multi_cases_c: Vec<MultiLineCase> = Vec::new();

    for (idx, nc) in nodes.iter().enumerate() {
        if fstr(nc, "phase") == Some("REMOVED") || fbool(nc, "isSoftDeleted") == Some(true) {
            continue;
        }
        let is_wrap = fstr(nc, "stackWrap") == Some("WRAP") || fbool(nc, "stackWrap") == Some(true);
        if !is_wrap {
            continue;
        }
        let mode = fstr(nc, "stackMode").unwrap_or("ABSENT");
        if mode != "HORIZONTAL" && mode != "VERTICAL" {
            wrap_with_grid_or_none += 1;
            continue;
        }

        total_wrapped_stacks += 1;
        let horiz = mode == "HORIZONTAL";
        let ty = fstr(nc, "type").unwrap_or("UNKNOWN").to_string();
        let g = nc.get("guid").and_then(guid_str).unwrap_or_default();
        let name = fstr(nc, "name").unwrap_or("").to_string();
        let sp = fnum(nc, "stackSpacing");
        let csp = fnum(nc, "stackCounterSpacing");
        let ac = fstr(nc, "stackCounterAlignContent").map(String::from);
        let csizing = fstr(nc, "stackCounterSizing").unwrap_or("FIXED").to_string();
        let is_space_between = ac.as_deref() == Some("SPACE_BETWEEN");

        let (kids, child_source) = flow_children_of(idx, horiz);
        let lines = group_into_lines(&kids, sp.unwrap_or(0.0));
        let multi = lines.len() >= 2;

        if let Some(cval) = csp {
            count_a_present += 1;
            if has_var_field(nc, "STACK_SPACING") {
                count_a_var_sp += 1;
            }
            if has_var_field(nc, "STACK_COUNTER_SPACING") {
                count_a_var_csp += 1;
            }
            *by_type_a.entry(ty.clone()).or_default() += 1;
            if cval == 0.0 {
                count_a_zero += 1;
            }
            if (cval - sp.unwrap_or(0.0)).abs() < 1e-4 {
                count_a_eq_item += 1;
            } else {
                count_a_ne_item += 1;
            }
            match ac.as_deref() {
                Some("SPACE_BETWEEN") => count_a_and_c += 1,
                Some("AUTO") => count_a_align_auto += 1,
                None => count_a_align_absent += 1,
                Some(_) => {}
            }
            match lines.len() {
                0 => count_a_0_line += 1,
                1 => count_a_1_line += 1,
                _ => {
                    count_a_multi_line += 1;
                    if is_space_between {
                        count_a_c_multi_line += 1;
                    } else {
                        count_a_not_c_multi_line += 1;
                    }
                }
            }
        } else {
            count_b_absent += 1;
            if has_var_field(nc, "STACK_SPACING") {
                count_b_var_sp += 1;
            }
            if has_var_field(nc, "STACK_COUNTER_SPACING") {
                count_b_var_csp += 1;
            }
            *by_type_b.entry(ty.clone()).or_default() += 1;
            if sp.unwrap_or(0.0) == 0.0 {
                count_b_item_zero_or_absent += 1;
            } else {
                count_b_item_nonzero += 1;
            }
            match ac.as_deref() {
                Some("SPACE_BETWEEN") => count_b_and_c += 1,
                Some("AUTO") => count_b_align_auto += 1,
                None => count_b_align_absent += 1,
                Some(_) => {}
            }
            match lines.len() {
                0 => count_b_0_line += 1,
                1 => count_b_1_line += 1,
                _ => {
                    count_b_multi_line += 1;
                    if is_space_between {
                        count_b_c_multi_line += 1;
                    } else {
                        count_b_not_c_multi_line += 1;
                    }
                }
            }
        }

        if is_space_between {
            count_c_space_between += 1;
            if multi {
                count_c_multi_line += 1;
            }
        }
        if multi {
            count_d_multi_line += 1;
            let gaps: Vec<f32> = lines.windows(2).map(|w| w[1].cross_start - w[0].cross_end).collect();
            let fsize = node_size(nc);
            let pad = padding_of(nc);
            let frame_cross = if horiz { fsize.y } else { fsize.x };
            let pad_cross = if horiz { pad[0] + pad[2] } else { pad[3] + pad[1] };
            let sum_line_cross: f32 = lines.iter().map(LineSpan::cross_extent).sum();
            let pred_item = sp.unwrap_or(0.0);
            let pred_counter_or_zero = csp.unwrap_or(0.0);
            let raw_sb = (frame_cross - pad_cross - sum_line_cross) / (lines.len() as f32 - 1.0);
            let pred_space_between = raw_sb.max(pred_counter_or_zero);

            let case = MultiLineCase {
                guid: g,
                name,
                ty,
                mode: mode.to_string(),
                counter_sizing: csizing,
                child_source,
                frame_size: fsize,
                padding: pad,
                stack_spacing: sp,
                stack_counter_spacing: csp,
                align_content: ac,
                lines,
                gaps,
                pred_item,
                pred_counter_or_zero,
                pred_space_between,
            };
            if is_space_between {
                multi_cases_c.push(case);
            } else if csp.is_some() {
                multi_cases_a_not_c.push(case);
            } else {
                multi_cases_b_not_c.push(case);
            }
        }
    }

    println!("  stackWrap=WRAP with non-stack mode (GRID/NONE/ABSENT): {wrap_with_grid_or_none}");
    println!("  Total wrapped stacks (HORIZONTAL | VERTICAL): {total_wrapped_stacks}");
    println!("  (a) stackCounterSpacing present: {count_a_present} (by type: {by_type_a:?})");
    println!("      - stackCounterSpacing == 0.0: {count_a_zero}");
    println!("      - stackCounterSpacing == stackSpacing: {count_a_eq_item}, != stackSpacing: {count_a_ne_item}");
    println!("      - variableConsumptionMap: STACK_SPACING={count_a_var_sp}, STACK_COUNTER_SPACING={count_a_var_csp}");
    println!("      - stackCounterAlignContent: SPACE_BETWEEN={count_a_and_c}, AUTO={count_a_align_auto}, absent={count_a_align_absent}");
    println!("      - lines: 0 lines={count_a_0_line}, 1 line={count_a_1_line}, >=2 lines={count_a_multi_line} (not SPACE_BETWEEN={count_a_not_c_multi_line}, SPACE_BETWEEN={count_a_c_multi_line})");
    println!("  (b) stackCounterSpacing absent:  {count_b_absent} (by type: {by_type_b:?})");
    println!("      - stackSpacing == 0 or absent: {count_b_item_zero_or_absent}, stackSpacing != 0: {count_b_item_nonzero}");
    println!("      - variableConsumptionMap: STACK_SPACING={count_b_var_sp}, STACK_COUNTER_SPACING={count_b_var_csp}");
    println!("      - stackCounterAlignContent: SPACE_BETWEEN={count_b_and_c}, AUTO={count_b_align_auto}, absent={count_b_align_absent}");
    println!("      - lines: 0 lines={count_b_0_line}, 1 line={count_b_1_line}, >=2 lines={count_b_multi_line} (not SPACE_BETWEEN={count_b_not_c_multi_line}, SPACE_BETWEEN={count_b_c_multi_line})");
    println!("  (c) stackCounterAlignContent == SPACE_BETWEEN: {count_c_space_between} (in a: {count_a_and_c}, in b: {count_b_and_c}; >=2 lines: {count_c_multi_line})");
    println!("  (d) >= 2 lines in stored geometry: {count_d_multi_line} (in a: {count_a_multi_line}, in b: {count_b_multi_line})");

    // 2. Gap Figma used, per case
    println!("\n=== 2. GAP FIGMA USED, PER CASE ===");
    let summarize_group = |label: &str, cases: &[MultiLineCase]| {
        let mut match_item = 0usize;
        let mut match_counter_or_zero = 0usize;
        let mut match_sb = 0usize;
        let mut match_none = 0usize;
        let mut fixed_cross_count = 0usize;
        let mut disc_total = 0usize;
        let mut disc_match_item = 0usize;
        let mut disc_match_counter_or_zero = 0usize;
        let mut disc_match_sb = 0usize;
        let mut disc_match_none = 0usize;

        for c in cases {
            if c.counter_sizing == "FIXED" {
                fixed_cross_count += 1;
            }
            let g0 = c.gaps[0];
            let m_item = c.gaps.iter().all(|&g| (g - c.pred_item).abs() <= 0.5);
            let m_cz = c.gaps.iter().all(|&g| (g - c.pred_counter_or_zero).abs() <= 0.5);
            let m_sb = c.gaps.iter().all(|&g| (g - c.pred_space_between).abs() <= 0.5);
            if m_item {
                match_item += 1;
            }
            if m_cz {
                match_counter_or_zero += 1;
            }
            if m_sb {
                match_sb += 1;
            }
            if !m_item && !m_cz && !m_sb {
                match_none += 1;
                println!(
                    "  [UNMATCHED in {label}] guid={} name={:?} ty={} src={} lines={} gap0={g0:.3} gaps={:?} sp={:?} csp={:?} ac={:?}",
                    c.guid,
                    c.name,
                    c.ty,
                    c.child_source,
                    c.lines.len(),
                    c.gaps,
                    c.stack_spacing,
                    c.stack_counter_spacing,
                    c.align_content
                );
            }
            if (c.pred_item - c.pred_counter_or_zero).abs() > 0.5 {
                disc_total += 1;
                if m_item {
                    disc_match_item += 1;
                }
                if m_cz {
                    disc_match_counter_or_zero += 1;
                }
                if m_sb {
                    disc_match_sb += 1;
                }
                if !m_item && !m_cz && !m_sb {
                    disc_match_none += 1;
                }
            }
        }
        println!(
            "Group {label}: total={} (counterSizing FIXED={fixed_cross_count}, HUG={}) | matches stackSpacing={} | matches stackCounterSpacing(or 0)={} | matches space_between={} | unmatched={}",
            cases.len(),
            cases.len() - fixed_cross_count,
            match_item,
            match_counter_or_zero,
            match_sb,
            match_none
        );
        println!(
            "  Discriminating subset (stackSpacing != stackCounterSpacing_or_0): total={disc_total} | matches stackSpacing={disc_match_item} | matches stackCounterSpacing(or 0)={disc_match_counter_or_zero} | matches space_between={disc_match_sb} | unmatched={disc_match_none}"
        );
        let mut shown = 0usize;
        for pass in [true, false] {
            for c in cases {
                let is_disc = (c.pred_item - c.pred_counter_or_zero).abs() > 0.5;
                if is_disc != pass || shown >= 10 {
                    continue;
                }
                shown += 1;
                let line_heights: Vec<f32> = c.lines.iter().map(LineSpan::cross_extent).collect();
                println!(
                    "  Example #{shown}: guid={} type={} mode={} counterSizing={} name={:?} size=({:.1}x{:.1}) pad={:?} stackSpacing={:?} stackCounterSpacing={:?} alignContent={:?} lines={} line_cross={:?} measured_gaps={:?} (pred_item={:.1}, pred_cz={:.1}, pred_sb={:.2})",
                    c.guid, c.ty, c.mode, c.counter_sizing, c.name, c.frame_size.x, c.frame_size.y, c.padding, c.stack_spacing, c.stack_counter_spacing, c.align_content, c.lines.len(), line_heights, c.gaps, c.pred_item, c.pred_counter_or_zero, c.pred_space_between
                );
            }
        }
    };

    summarize_group("(a) stackCounterSpacing present, not SPACE_BETWEEN", &multi_cases_a_not_c);
    summarize_group("(b) stackCounterSpacing absent, not SPACE_BETWEEN", &multi_cases_b_not_c);
    summarize_group("(c) stackCounterAlignContent == SPACE_BETWEEN", &multi_cases_c);

    // Check 11834:15048 specifically and print the 4 stackSpacing==0 frames and 5 FIXED frames in Group (b)
    if let Some(c) = multi_cases_b_not_c.iter().find(|c| c.guid == "11834:15048") {
        let line_heights: Vec<f32> = c.lines.iter().map(LineSpan::cross_extent).collect();
        println!(
            "\nSpecific check 11834:15048: guid={} type={} name={:?} size=({:.1}x{:.1}) stackSpacing={:?} stackCounterSpacing={:?} lines={} line_cross_first3={:?} measured_gaps_first3={:?}",
            c.guid, c.ty, c.name, c.frame_size.x, c.frame_size.y, c.stack_spacing, c.stack_counter_spacing, c.lines.len(), &line_heights[..line_heights.len().min(3)], &c.gaps[..c.gaps.len().min(3)]
        );
    }
    println!("Group (b) multi-line with stackSpacing == 0/absent:");
    for c in multi_cases_b_not_c.iter().filter(|c| c.pred_item == 0.0) {
        println!(
            "  guid={} type={} mode={} counterSizing={} name={:?} size=({:.1}x{:.1}) sp={:?} csp={:?} lines={} measured_gaps={:?}",
            c.guid,
            c.ty,
            c.mode,
            c.counter_sizing,
            c.name,
            c.frame_size.x,
            c.frame_size.y,
            c.stack_spacing,
            c.stack_counter_spacing,
            c.lines.len(),
            c.gaps
        );
    }
    println!("Group (b) multi-line with counterSizing == FIXED:");
    for c in multi_cases_b_not_c.iter().filter(|c| c.counter_sizing == "FIXED") {
        println!(
            "  guid={} type={} mode={} counterSizing={} name={:?} size=({:.1}x{:.1}) pad={:?} sp={:?} csp={:?} lines={} measured_gaps={:?} pred_sb={:.2}",
            c.guid,
            c.ty,
            c.mode,
            c.counter_sizing,
            c.name,
            c.frame_size.x,
            c.frame_size.y,
            c.padding,
            c.stack_spacing,
            c.stack_counter_spacing,
            c.lines.len(),
            c.gaps,
            c.pred_space_between
        );
    }
    println!("Group (a) multi-line where stackCounterSpacing == stackSpacing:");
    for c in multi_cases_a_not_c.iter().filter(|c| (c.pred_item - c.pred_counter_or_zero).abs() <= 0.5) {
        println!(
            "  guid={} type={} mode={} counterSizing={} name={:?} size=({:.1}x{:.1}) sp={:?} csp={:?} lines={} measured_gaps={:?}",
            c.guid,
            c.ty,
            c.mode,
            c.counter_sizing,
            c.name,
            c.frame_size.x,
            c.frame_size.y,
            c.stack_spacing,
            c.stack_counter_spacing,
            c.lines.len(),
            c.gaps
        );
    }

    // 3. Detailed Instance Overrides investigation
    println!("\n=== 3. INSTANCE OVERRIDES ON WRAPPED STACKS ===");
    let resolve_target = |g: &str| -> Option<usize> { by_guid.get(g).copied().or_else(|| by_override_key.get(g).copied()) };

    let mut total_instances = 0usize;
    let mut inst_with_overrides = 0usize;
    let mut ov_entries_total = 0usize;
    let mut ov_entries_setting_stack_spacing = 0usize;
    let mut ov_entries_setting_counter_spacing = 0usize;
    let mut ov_entries_setting_stack_wrap = 0usize;
    let mut ov_entries_on_wrapped_target = 0usize;
    let mut ov_wrapped_spacing_without_counter = 0usize;
    let mut inst_self_wrapped_total = 0usize;
    let mut inst_self_wrapped_differs_from_sym = 0usize;
    let mut inst_self_wrapped_multi_line = 0usize;

    for (idx, nc) in nodes.iter().enumerate() {
        if fstr(nc, "phase") == Some("REMOVED") || fbool(nc, "isSoftDeleted") == Some(true) {
            continue;
        }
        if fstr(nc, "type") != Some("INSTANCE") {
            continue;
        }
        total_instances += 1;
        let sym_guid = nc.get("symbolData").and_then(|s| s.get("symbolID")).and_then(guid_str);
        let sym_idx = sym_guid.as_deref().and_then(resolve_target);

        let sym_is_wrap = sym_idx.is_some_and(|si| fstr(&nodes[si], "stackWrap") == Some("WRAP"));
        let inst_is_wrap = fstr(nc, "stackWrap") == Some("WRAP") || (fstr(nc, "stackWrap").is_none() && sym_is_wrap);
        if inst_is_wrap && fnum(nc, "stackSpacing").is_some() && fnum(nc, "stackCounterSpacing").is_none() {
            inst_self_wrapped_total += 1;
            let sym_sp = sym_idx.and_then(|si| fnum(&nodes[si], "stackSpacing"));
            let inst_sp = fnum(nc, "stackSpacing");
            let differs = sym_sp != inst_sp;
            if differs {
                inst_self_wrapped_differs_from_sym += 1;
            }
            let (kids, _src) =
                flow_children_of(idx, fstr(nc, "stackMode").or_else(|| sym_idx.and_then(|si| fstr(&nodes[si], "stackMode"))) != Some("VERTICAL"));
            let lines = group_into_lines(&kids, inst_sp.unwrap_or(0.0));
            if lines.len() >= 2 {
                inst_self_wrapped_multi_line += 1;
            }
        }

        let Some(ovs) = nc.get("symbolData").and_then(|s| s.get("symbolOverrides")).and_then(Value::as_array) else {
            continue;
        };
        if ovs.is_empty() {
            continue;
        }
        inst_with_overrides += 1;
        for ov in ovs {
            ov_entries_total += 1;
            let has_sp = fnum(ov, "stackSpacing").is_some() || fnum(ov, "gridColumnGap").is_some() || fnum(ov, "gridColumnsGap").is_some();
            let has_csp = fnum(ov, "stackCounterSpacing").is_some() || fnum(ov, "gridRowGap").is_some() || fnum(ov, "gridRowsGap").is_some();
            let has_wrap = ov.get("stackWrap").is_some();
            let path = guid_path(ov);
            let target_idx = path.last().and_then(|g| resolve_target(g));
            let target_is_wrap = target_idx.is_some_and(|ti| fstr(&nodes[ti], "stackWrap") == Some("WRAP")) || fstr(ov, "stackWrap") == Some("WRAP");

            if has_sp {
                ov_entries_setting_stack_spacing += 1;
            }
            if has_csp {
                ov_entries_setting_counter_spacing += 1;
            }
            if has_wrap {
                ov_entries_setting_stack_wrap += 1;
            }
            if target_is_wrap {
                ov_entries_on_wrapped_target += 1;
            }
            if target_is_wrap && has_sp && !has_csp {
                ov_wrapped_spacing_without_counter += 1;
            }
        }
    }

    println!("  Total INSTANCE records: {total_instances}");
    println!("  Instances with non-empty symbolOverrides: {inst_with_overrides}");
    println!("  Total symbolOverrides entries: {ov_entries_total}");
    println!("  symbolOverrides entries setting stackSpacing/gridColumnGap: {ov_entries_setting_stack_spacing}");
    println!("  symbolOverrides entries setting stackCounterSpacing/gridRowGap: {ov_entries_setting_counter_spacing}");
    println!("  symbolOverrides entries setting stackWrap: {ov_entries_setting_stack_wrap}");
    println!("  symbolOverrides entries targeting a wrapped stack: {ov_entries_on_wrapped_target}");
    println!("  symbolOverrides entries on wrapped stack setting stackSpacing without stackCounterSpacing: {ov_wrapped_spacing_without_counter}");
    println!("  INSTANCE own-record fields on wrapped SYMBOL with stackSpacing and no stackCounterSpacing: {inst_self_wrapped_total} (where inst.stackSpacing != sym.stackSpacing: {inst_self_wrapped_differs_from_sym}; with >=2 lines in derivedSymbolData: {inst_self_wrapped_multi_line})");

    // 4. Check Export side behavior on each case
    println!("\n=== 4. EXPORT SIDE BEHAVIOR ===");
    for (case_name, extra_kiwi, extra_rest) in [
        ("wrap_absent_counter_gap", json!({"stackWrap": "WRAP"}), json!({"layoutWrap": "WRAP"})),
        ("wrap_explicit_zero_counter_gap", json!({"stackWrap": "WRAP", "stackCounterSpacing": 0.0}), json!({"layoutWrap": "WRAP", "counterAxisSpacing": 0})),
        ("wrap_explicit_12_counter_gap", json!({"stackWrap": "WRAP", "stackCounterSpacing": 12.0}), json!({"layoutWrap": "WRAP", "counterAxisSpacing": 12})),
        (
            "wrap_space_between_absent_counter_gap",
            json!({"stackWrap": "WRAP", "stackCounterAlignContent": "SPACE_BETWEEN"}),
            json!({"layoutWrap": "WRAP", "counterAxisAlignContent": "SPACE_BETWEEN"}),
        ),
        ("nowrap_absent_counter_gap", json!({}), json!({})),
    ] {
        let mut stack = json!({
            "guid": {"sessionID": 1, "localID": 2},
            "parentIndex": {"guid": {"sessionID": 0, "localID": 1}, "position": "!"},
            "type": "FRAME", "name": "Stack", "size": {"x": 300.0, "y": 200.0},
            "stackMode": "HORIZONTAL", "stackSpacing": 32.0
        });
        stack.as_object_mut().unwrap().extend(extra_kiwi.as_object().unwrap().clone());
        let changes = json!([
            {"guid": {"sessionID": 0, "localID": 0}, "type": "DOCUMENT", "name": "d"},
            {"guid": {"sessionID": 0, "localID": 1}, "parentIndex": {"guid": {"sessionID": 0, "localID": 0}, "position": "!"}, "type": "CANVAS", "name": "p"},
            stack
        ]);
        let doc = import_fig_node_changes(&changes, "d").unwrap();
        let fig_bytes = export_fig(&doc).unwrap();
        let decoded = designos_fig::decode_fig_to_json(&fig_bytes).unwrap();
        let exported_nc = decoded["message"]["nodeChanges"].as_array().unwrap().iter().find(|nc| fstr(nc, "name") == Some("Stack")).unwrap();
        let rest_out = export_rest_json(&doc);
        let rest_frame = &rest_out["document"]["children"][0]["children"][0];

        let mut rf = json!({
            "id": "1:2", "name": "Stack", "type": "FRAME", "layoutMode": "HORIZONTAL", "itemSpacing": 32,
            "absoluteBoundingBox": {"x": 0, "y": 0, "width": 300, "height": 200}, "children": []
        });
        rf.as_object_mut().unwrap().extend(extra_rest.as_object().unwrap().clone());
        let rdoc_json =
            json!({"name": "f", "document": {"id": "0:0", "type": "DOCUMENT", "children": [{"id": "0:1", "name": "p", "type": "CANVAS", "children": [rf]}]}});
        let rdoc = import_rest_json(&rdoc_json.to_string()).unwrap();
        let r_rest_out = export_rest_json(&rdoc);
        let r_rest_frame = &r_rest_out["document"]["children"][0]["children"][0];

        println!(
            "  Case {case_name}: .fig export stackSpacing={:?} stackCounterSpacing={:?} stackWrap={:?} stackCounterAlignContent={:?} | REST export itemSpacing={:?} counterAxisSpacing={:?} layoutWrap={:?} counterAxisAlignContent={:?} (from REST import: counterAxisSpacing={:?})",
            exported_nc.get("stackSpacing"),
            exported_nc.get("stackCounterSpacing"),
            exported_nc.get("stackWrap"),
            exported_nc.get("stackCounterAlignContent"),
            rest_frame.get("itemSpacing"),
            rest_frame.get("counterAxisSpacing"),
            rest_frame.get("layoutWrap"),
            rest_frame.get("counterAxisAlignContent"),
            r_rest_frame.get("counterAxisSpacing"),
        );
    }
}
