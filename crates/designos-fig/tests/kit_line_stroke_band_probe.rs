//! Reverse-engineering probe (ignored by default): measures where Figma puts the stored stroke band of
//! LINE layers (and flat VECTORs), and the outline of each end cap, directly from the raw `NodeChange`
//! records of the Untitled UI PRO v8.0 kit (`KIT_CLONE` or `DESIGNOS_KIT_PATH`).
//!
//! Two populations are read: the LINE nodes that the kit stores itself, and the outlines Figma derived for
//! LINE nodes inside instances (`derivedSymbolData`), which carry the instance's own size.
//!
//! Output is a plain-text census on stdout; run with `--ignored --nocapture`.

use designos_doc::{Path, PathCmd, Vec2};
use designos_fig::commands_blob_to_path;
use designos_fig::container;
use designos_fig::kiwi::{KiwiValue, Schema};
use std::collections::{BTreeMap, HashMap};

fn u64_of(v: &KiwiValue) -> Option<u64> {
    match v {
        KiwiValue::Uint(u) => Some(*u as u64),
        KiwiValue::Uint64(u) => Some(*u),
        KiwiValue::Int(i) if *i >= 0 => Some(*i as u64),
        KiwiValue::Int64(i) if *i >= 0 => Some(*i as u64),
        _ => None,
    }
}

fn guid_of(g: &KiwiValue) -> Option<String> {
    let (s, l) = (u64_of(g.get("sessionID")?)?, u64_of(g.get("localID")?)?);
    if s == u64::from(u32::MAX) && l == u64::from(u32::MAX) {
        return None;
    }
    Some(format!("{s}:{l}"))
}

fn f32_of(v: &KiwiValue, k: &str) -> Option<f32> {
    match v.get(k)? {
        KiwiValue::Float(f) => f.is_finite().then_some(*f),
        KiwiValue::Uint(u) => Some(*u as f32),
        KiwiValue::Int(i) => Some(*i as f32),
        _ => None,
    }
}

fn str_of(v: &KiwiValue) -> Option<&str> {
    match v {
        KiwiValue::String(s) => Some(s.as_str()),
        KiwiValue::Enum(s) => Some(s),
        _ => None,
    }
}

fn field_str<'a>(v: &'a KiwiValue, k: &str) -> Option<&'a str> {
    v.get(k).and_then(str_of)
}

fn size_of(v: &KiwiValue) -> Option<(f32, f32)> {
    let s = v.get("size")?;
    Some((f32_of(s, "x").unwrap_or(0.0), f32_of(s, "y").unwrap_or(0.0)))
}

fn guid_path(o: &KiwiValue) -> Vec<String> {
    let p = o.get("guidPath");
    let list = p.and_then(|p| p.get("guids")).or(p).and_then(KiwiValue::as_array);
    list.map(|a| a.iter().filter_map(guid_of).collect()).unwrap_or_default()
}

/// Geometry entries of a `fillGeometry` / `strokeGeometry` array: (blob index, even-odd).
fn geometry_of(v: &KiwiValue, key: &str) -> Vec<(usize, bool)> {
    let Some(arr) = v.get(key).and_then(KiwiValue::as_array) else { return Vec::new() };
    arr.iter()
        .filter_map(|g| {
            let idx = u64_of(g.get("commandsBlob")?)? as usize;
            let odd = matches!(field_str(g, "windingRule"), Some("ODD" | "EVENODD"));
            Some((idx, odd))
        })
        .collect()
}

fn dashes_of(v: &KiwiValue) -> Vec<f32> {
    v.get("dashPattern")
        .and_then(KiwiValue::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|x| match x {
                    KiwiValue::Float(f) => Some(*f),
                    KiwiValue::Uint(u) => Some(*u as f32),
                    KiwiValue::Int(i) => Some(*i as f32),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Per-vertex caps stored in `vectorData.styleOverrideTable`.
fn vector_caps_of(v: &KiwiValue) -> Vec<String> {
    v.get("vectorData")
        .and_then(|vd| vd.get("styleOverrideTable"))
        .and_then(KiwiValue::as_array)
        .map(|t| t.iter().filter_map(|e| field_str(e, "strokeCap").map(str::to_string)).collect())
        .unwrap_or_default()
}

#[derive(Debug, Clone, Default)]
struct StrokeProps {
    weight: Option<f32>,
    align: Option<String>,
    cap: Option<String>,
    join: Option<String>,
    dashes: Option<Vec<f32>>,
}

fn stroke_props_of(v: &KiwiValue) -> StrokeProps {
    StrokeProps {
        weight: f32_of(v, "strokeWeight"),
        align: field_str(v, "strokeAlign").map(str::to_string),
        cap: field_str(v, "strokeCap").map(str::to_string),
        join: field_str(v, "strokeJoin").map(str::to_string),
        dashes: v.get("dashPattern").map(|_| dashes_of(v)),
    }
}

#[derive(Debug, Clone)]
struct LineRec {
    guid: String,
    name: String,
    ty: String,
    size: (f32, f32),
    props: StrokeProps,
    stroke_geo: Vec<(usize, bool)>,
    vector_caps: Vec<String>,
    /// The band was derived for an instance (guid is the instance, `path` the node inside it).
    derived_in: Option<(String, Vec<String>)>,
    overridden: bool,
    /// Size of the node as the main component (or the page) stores it, and the instance's own size.
    main_size: (f32, f32),
    instance_size: Option<(f32, f32)>,
    transform: Option<[f32; 6]>,
    /// The derived entry states its own `size` (otherwise the size above is the main component's).
    own_size: bool,
    /// Size of the instance's main component (`symbolData.symbolID`).
    symbol_size: Option<(f32, f32)>,
    /// Weight stored on the main node itself, before any override merge.
    main_weight: Option<f32>,
    /// The enclosing nested instance (the derived entry for the path minus its last element): its derived size and
    /// the size of its own main component.
    nested: Option<((f32, f32), (f32, f32))>,
    /// Every `strokeWeight` the instance's override table states, by the path it names.
    weight_overrides: Vec<(Vec<String>, f32)>,
}

fn transform_of(v: &KiwiValue) -> Option<[f32; 6]> {
    let t = v.get("transform")?;
    let g = |k: &str, d: f32| f32_of(t, k).unwrap_or(d);
    Some([g("m00", 1.0), g("m01", 0.0), g("m02", 0.0), g("m10", 0.0), g("m11", 1.0), g("m12", 0.0)])
}

/// Axis-aligned extents of a path in its own space, curves sampled.
#[derive(Debug, Clone, Copy)]
struct Extent {
    x0: f32,
    x1: f32,
    y0: f32,
    y1: f32,
}

fn cubic(p0: Vec2, c1: Vec2, c2: Vec2, p1: Vec2, t: f32) -> Vec2 {
    let u = 1.0 - t;
    let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
    Vec2::new(a * p0.x + b * c1.x + c * c2.x + d * p1.x, a * p0.y + b * c1.y + c * c2.y + d * p1.y)
}

fn quad(p0: Vec2, c: Vec2, p1: Vec2, t: f32) -> Vec2 {
    let u = 1.0 - t;
    Vec2::new(u * u * p0.x + 2.0 * u * t * c.x + t * t * p1.x, u * u * p0.y + 2.0 * u * t * c.y + t * t * p1.y)
}

fn flatten(path: &Path, out: &mut Vec<Vec2>) {
    let mut cur = Vec2::new(0.0, 0.0);
    for c in &path.cmds {
        match *c {
            PathCmd::MoveTo(p) | PathCmd::LineTo(p) => {
                out.push(p);
                cur = p;
            }
            PathCmd::QuadTo(c1, p) => {
                for i in 1..=32 {
                    out.push(quad(cur, c1, p, i as f32 / 32.0));
                }
                cur = p;
            }
            PathCmd::CubicTo(c1, c2, p) => {
                for i in 1..=32 {
                    out.push(cubic(cur, c1, c2, p, i as f32 / 32.0));
                }
                cur = p;
            }
            PathCmd::Close => {}
        }
    }
}

fn extent_of(blobs: &[Vec<u8>], geo: &[(usize, bool)]) -> Option<Extent> {
    let mut pts = Vec::new();
    for &(idx, odd) in geo {
        let b = blobs.get(idx)?;
        flatten(&commands_blob_to_path(b, odd), &mut pts);
    }
    if pts.is_empty() {
        return None;
    }
    let mut e = Extent { x0: f32::MAX, x1: f32::MIN, y0: f32::MAX, y1: f32::MIN };
    for p in pts {
        e.x0 = e.x0.min(p.x);
        e.x1 = e.x1.max(p.x);
        e.y0 = e.y0.min(p.y);
        e.y1 = e.y1.max(p.y);
    }
    Some(e)
}

fn r3(x: f32) -> String {
    format!("{:.3}", (x * 1000.0).round() / 1000.0 + 0.0)
}

fn flat_label(r: &LineRec) -> String {
    let p = &r.props;
    format!(
        "align={} cap={} dash={}",
        p.align.as_deref().unwrap_or("(absent)"),
        p.cap.as_deref().unwrap_or("(absent)"),
        match &p.dashes {
            None => "(absent)".to_string(),
            Some(d) if d.is_empty() => "[]".to_string(),
            Some(d) => format!("{d:?}"),
        }
    )
}

#[test]
#[ignore = "reads KIT_CLONE or DESIGNOS_KIT_PATH env var"]
fn probe_kit_line_stroke_band_and_caps() {
    let kit_path = std::env::var_os("KIT_CLONE").or_else(|| std::env::var_os("DESIGNOS_KIT_PATH")).expect("set KIT_CLONE or DESIGNOS_KIT_PATH");
    let bytes = std::fs::read(&kit_path).expect("read kit file");
    let mut archive = container::open(&bytes).expect("open container");
    let (schema_bin, msg_bin) = container::read_archive_chunks(&archive).expect("read chunks");
    archive.canvas = Vec::new();
    let schema = Schema::decode(&schema_bin).expect("decode schema");
    drop(schema_bin);
    let root = schema.root_message().expect("root message");
    let mut message = schema.decode_message_for_import(root, &msg_bin).expect("decode message");
    drop(msg_bin);

    let blobs: Vec<Vec<u8>> = match message.take_field("blobs") {
        Some(KiwiValue::Array(arr)) => arr
            .into_iter()
            .map(|mut b| match b.take_field("bytes") {
                Some(KiwiValue::Bytes(bytes)) => bytes,
                _ => Vec::new(),
            })
            .collect(),
        _ => Vec::new(),
    };
    let raw: Vec<KiwiValue> = match message.take_field("nodeChanges") {
        Some(KiwiValue::Array(arr)) => arr,
        _ => Vec::new(),
    };
    println!("raw NodeChange records: {}  blobs: {}", raw.len(), blobs.len());

    // Index every live node: line-like ones keep their stroke fields.
    let mut by_guid: HashMap<String, usize> = HashMap::new();
    let mut by_override_key: HashMap<String, usize> = HashMap::new();
    let mut ty_count: BTreeMap<String, usize> = BTreeMap::new();
    let mut cap_by_type: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut vcap_by_type: BTreeMap<(String, String), usize> = BTreeMap::new();
    for (i, nc) in raw.iter().enumerate() {
        if field_str(nc, "phase") == Some("REMOVED") {
            continue;
        }
        let ty = field_str(nc, "type").unwrap_or("?").to_string();
        *ty_count.entry(ty.clone()).or_default() += 1;
        if let Some(c) = field_str(nc, "strokeCap") {
            *cap_by_type.entry((ty.clone(), c.to_string())).or_default() += 1;
        }
        for c in vector_caps_of(nc) {
            *vcap_by_type.entry((ty.clone(), c)).or_default() += 1;
        }
        if let Some(g) = nc.get("guid").and_then(guid_of) {
            by_guid.insert(g, i);
        }
        if let Some(g) = nc.get("overrideKey").and_then(guid_of) {
            by_override_key.insert(g, i);
        }
    }
    println!("\n== 0. strokeCap spelled on any node type (node-level), and per-vertex caps in vectorData.styleOverrideTable ==");
    for ((t, c), n) in &cap_by_type {
        println!("  node-level  {t:<18} cap={c:<18} {n}");
    }
    for ((t, c), n) in &vcap_by_type {
        println!("  per-vertex  {t:<18} cap={c:<18} {n}");
    }
    println!("  LINE nodes: {}   VECTOR nodes: {}", ty_count.get("LINE").copied().unwrap_or(0), ty_count.get("VECTOR").copied().unwrap_or(0));

    // Direct LINE nodes and flat VECTORs.
    let mut recs: Vec<LineRec> = Vec::new();
    let mut flat_vectors = 0usize;
    let mut flat_vector_with_geo = 0usize;
    for nc in &raw {
        if field_str(nc, "phase") == Some("REMOVED") {
            continue;
        }
        let ty = field_str(nc, "type").unwrap_or("?");
        let size = size_of(nc).unwrap_or((0.0, 0.0));
        let flat_vector = ty == "VECTOR" && (size.1.abs() < 1e-4 || size.0.abs() < 1e-4);
        if ty == "VECTOR" && flat_vector {
            flat_vectors += 1;
            if !geometry_of(nc, "strokeGeometry").is_empty() {
                flat_vector_with_geo += 1;
            }
        }
        if ty != "LINE" && !flat_vector {
            continue;
        }
        recs.push(LineRec {
            guid: nc.get("guid").and_then(guid_of).unwrap_or_default(),
            name: field_str(nc, "name").unwrap_or("").to_string(),
            ty: ty.to_string(),
            size,
            props: stroke_props_of(nc),
            stroke_geo: geometry_of(nc, "strokeGeometry"),
            vector_caps: vector_caps_of(nc),
            derived_in: None,
            overridden: false,
            main_size: size,
            instance_size: None,
            transform: transform_of(nc),
            own_size: true,
            symbol_size: None,
            main_weight: f32_of(nc, "strokeWeight"),
            nested: None,
            weight_overrides: Vec::new(),
        });
    }
    println!("  flat VECTORs (size.x or size.y == 0): {flat_vectors}, with stroke geometry: {flat_vector_with_geo}");

    // Derived outlines of LINE nodes inside instances.
    let mut derived_total = 0usize;
    let mut derived_unresolved = 0usize;
    for nc in &raw {
        if field_str(nc, "type") != Some("INSTANCE") || field_str(nc, "phase") == Some("REMOVED") {
            continue;
        }
        let Some(inst) = nc.get("guid").and_then(guid_of) else { continue };
        let sd = nc.get("symbolData");
        let derived = nc.get("derivedSymbolData").or_else(|| sd.and_then(|s| s.get("derivedSymbolData"))).and_then(KiwiValue::as_array);
        let Some(derived) = derived else { continue };
        let mut overrides: HashMap<Vec<String>, &KiwiValue> = HashMap::new();
        if let Some(arr) = sd.and_then(|s| s.get("symbolOverrides")).and_then(KiwiValue::as_array) {
            for o in arr {
                overrides.insert(guid_path(o), o);
            }
        }
        let derived_sizes: HashMap<Vec<String>, (f32, f32)> = derived.iter().filter_map(|d| Some((guid_path(d), size_of(d)?))).collect();
        for d in derived {
            let path = guid_path(d);
            let Some(leaf) = path.last() else { continue };
            let Some(&idx) = by_guid.get(leaf).or_else(|| by_override_key.get(leaf)) else {
                derived_unresolved += 1;
                continue;
            };
            let main = &raw[idx];
            let ty = field_str(main, "type").unwrap_or("?");
            let geo = geometry_of(d, "strokeGeometry");
            let size = size_of(d).or_else(|| size_of(main)).unwrap_or((0.0, 0.0));
            let flat_vector = ty == "VECTOR" && (size.1.abs() < 1e-4 || size.0.abs() < 1e-4);
            if ty != "LINE" && !flat_vector {
                continue;
            }
            derived_total += 1;
            let mut props = stroke_props_of(main);
            let mut overridden = false;
            if let Some(o) = overrides.get(&path) {
                let op = stroke_props_of(o);
                for (dst, src) in [(&mut props.weight, op.weight)] {
                    if src.is_some() {
                        *dst = src;
                        overridden = true;
                    }
                }
                for (dst, src) in [(&mut props.align, op.align), (&mut props.cap, op.cap), (&mut props.join, op.join)] {
                    if src.is_some() {
                        *dst = src;
                        overridden = true;
                    }
                }
                if op.dashes.is_some() {
                    props.dashes = op.dashes;
                    overridden = true;
                }
            }
            recs.push(LineRec {
                guid: leaf.clone(),
                name: field_str(main, "name").unwrap_or("").to_string(),
                ty: ty.to_string(),
                size,
                props,
                stroke_geo: geo,
                vector_caps: vector_caps_of(main),
                derived_in: Some((inst.clone(), path.clone())),
                overridden,
                main_size: size_of(main).unwrap_or((0.0, 0.0)),
                instance_size: size_of(nc),
                transform: transform_of(d).or_else(|| transform_of(main)),
                own_size: size_of(d).is_some(),
                symbol_size: sd.and_then(|s| s.get("symbolID")).and_then(guid_of).and_then(|g| by_guid.get(&g)).and_then(|&i| size_of(&raw[i])),
                main_weight: f32_of(main, "strokeWeight"),
                nested: (path.len() >= 2)
                    .then(|| {
                        let parent_path = path[..path.len() - 1].to_vec();
                        let parent_size = *derived_sizes.get(&parent_path)?;
                        let parent_leaf = parent_path.last()?;
                        let &pi = by_guid.get(parent_leaf).or_else(|| by_override_key.get(parent_leaf))?;
                        let sym = raw[pi].get("symbolData")?.get("symbolID").and_then(guid_of)?;
                        let sym_size = size_of(&raw[*by_guid.get(&sym)?])?;
                        Some((parent_size, sym_size))
                    })
                    .flatten(),
                weight_overrides: overrides.iter().filter_map(|(p, o)| Some((p.clone(), f32_of(o, "strokeWeight")?))).collect(),
            });
        }
    }
    println!("  derived entries naming a LINE / flat VECTOR: {derived_total}   (derived entries whose node did not resolve: {derived_unresolved})");

    {
        let mut distinct: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
        let mut per_main: HashMap<&str, usize> = HashMap::new();
        for r in recs.iter().filter(|r| r.ty == "LINE" && r.derived_in.is_some()) {
            distinct.insert(&r.guid);
            *per_main.entry(&r.guid).or_default() += 1;
        }
        let mut top: Vec<_> = per_main.into_iter().collect();
        top.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        println!("  derived LINE entries come from {} distinct main LINE nodes; most-copied: {:?}", distinct.len(), &top[..top.len().min(5)]);
    }

    // 1. Census
    let direct: Vec<&LineRec> = recs.iter().filter(|r| r.derived_in.is_none()).collect();
    let derived: Vec<&LineRec> = recs.iter().filter(|r| r.derived_in.is_some()).collect();
    for (title, set) in [("DIRECT nodes", &direct), ("DERIVED (inside instances)", &derived)] {
        println!("\n== 1. CENSUS {title} ==");
        for ty in ["LINE", "VECTOR"] {
            let s: Vec<&&LineRec> = set.iter().filter(|r| r.ty == ty).collect();
            if s.is_empty() {
                continue;
            }
            println!(
                "  {ty}: {}  with stroke geometry: {}  with overrides: {}",
                s.len(),
                s.iter().filter(|r| !r.stroke_geo.is_empty()).count(),
                s.iter().filter(|r| r.overridden).count()
            );
            let mut by: BTreeMap<String, usize> = BTreeMap::new();
            let mut by_geo: BTreeMap<String, usize> = BTreeMap::new();
            let mut weights: BTreeMap<String, usize> = BTreeMap::new();
            let mut vcaps: BTreeMap<String, usize> = BTreeMap::new();
            for r in &s {
                *by.entry(flat_label(r)).or_default() += 1;
                *by_geo.entry(format!("geo={}", if r.stroke_geo.is_empty() { "absent" } else { "present" })).or_default() += 1;
                *weights.entry(r3(r.props.weight.unwrap_or(f32::NAN))).or_default() += 1;
                *vcaps.entry(format!("{:?}", r.vector_caps)).or_default() += 1;
            }
            for (k, n) in &by {
                println!("    {k}: {n}");
            }
            for (k, n) in &by_geo {
                println!("    {k}: {n}");
            }
            println!("    weights: {weights:?}");
            println!("    per-vertex caps in vectorData: {vcaps:?}");
        }
    }

    // 2/3. Band geometry per (align, cap)
    #[derive(Default)]
    struct Group {
        n: usize,
        examples: Vec<String>,
        // (y0/w, y1/w, left overhang / w, right overhang / w)
        shapes: BTreeMap<(String, String, String, String), (usize, String)>,
    }
    let mut groups: BTreeMap<(String, String, String), Group> = BTreeMap::new();
    let mut weights_seen: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    let mut no_extent = 0usize;
    for r in &recs {
        if r.stroke_geo.is_empty() {
            continue;
        }
        let Some(e) = extent_of(&blobs, &r.stroke_geo) else {
            no_extent += 1;
            continue;
        };
        let w = r.props.weight.unwrap_or(0.0);
        if w <= 0.0 {
            continue;
        }
        let key = (r.ty.clone(), r.props.align.clone().unwrap_or_default(), r.props.cap.clone().unwrap_or_default());
        let g = groups.entry(key.clone()).or_default();
        g.n += 1;
        let label = format!("{}{}", r.guid, r.derived_in.as_ref().map(|(i, _)| format!(" in instance {i}")).unwrap_or_default());
        let left = -e.x0;
        let right = e.x1 - r.size.0;
        let k = (r3(e.y0 / w), r3(e.y1 / w), r3(left / w), r3(right / w));
        let slot = g.shapes.entry(k).or_insert((0, label.clone()));
        slot.0 += 1;
        if g.examples.len() < 3 {
            g.examples.push(format!("{label} w={} L={} y=[{},{}] x=[{},{}]", r3(w), r3(r.size.0), r3(e.y0), r3(e.y1), r3(e.x0), r3(e.x1)));
        }
        *weights_seen.entry(format!("{}/{}/{}", key.0, key.1, key.2)).or_default().entry(r3(w)).or_default() += 1;
    }
    println!("\n== 2/3. STORED BAND per (type, align, cap): shapes are (y0/w, y1/w, left overhang/w, right overhang/w); overhang = how far the outline sticks out past x=0 / x=size.x ==");
    println!("  stroke geometry that decoded to no points: {no_extent}");
    for ((ty, al, cap), g) in &groups {
        println!("  [{ty} align={al} cap={cap}] n={}", g.n);
        for (k, (n, ex)) in &g.shapes {
            println!("      y0/w={} y1/w={} overhangL/w={} overhangR/w={}  count={n}  e.g. {ex}", k.0, k.1, k.2, k.3);
        }
        for e in &g.examples {
            println!("      example {e}");
        }
        if let Some(ws) = weights_seen.get(&format!("{ty}/{al}/{cap}")) {
            println!("      weights: {ws:?}");
        }
    }

    // 5. A LINE whose stored band disagrees with its own size or weight.
    println!("\n== 5. LINE bands against the record's own size and weight ==");
    println!("  LINE size.y values: {:?}", {
        let mut m: BTreeMap<String, usize> = BTreeMap::new();
        for r in recs.iter().filter(|r| r.ty == "LINE") {
            *m.entry(r3(r.size.1)).or_default() += 1;
        }
        m
    });
    for (title, derived_pass) in [("direct", false), ("derived", true)] {
        let (mut fit, mut misfit, mut thick_bad) = (0usize, 0usize, 0usize);
        let mut classes: BTreeMap<String, usize> = BTreeMap::new();
        let mut shown = 0usize;
        for r in recs.iter().filter(|r| r.ty == "LINE" && r.derived_in.is_some() == derived_pass) {
            let (Some(e), Some(w)) = (extent_of(&blobs, &r.stroke_geo), r.props.weight) else { continue };
            let cap = r.props.cap.as_deref().unwrap_or("NONE");
            let _ = cap;
            let span = e.x1 - e.x0;
            if ((e.y1 - e.y0) - w).abs() > 0.01 {
                thick_bad += 1;
            }
            if (span - r.size.0).abs() <= 0.02 + 0.001 * r.size.0 && e.x0.abs() < 0.02 {
                fit += 1;
                continue;
            }
            misfit += 1;
            let rel_main = if (span - r.main_size.0).abs() <= 0.02 + 0.001 * r.main_size.0 {
                "band == main component's line length (not rescaled)"
            } else {
                "band matches neither"
            };
            *classes.entry(rel_main.to_string()).or_default() += 1;
            if shown < 8 {
                shown += 1;
                println!(
                    "  MISFIT({title}) {} w={} derived/own size.x={} main size.x={} band x=[{},{}] span={} band y=[{},{}] instance_size={:?} {}",
                    r.guid,
                    r3(w),
                    r3(r.size.0),
                    r3(r.main_size.0),
                    r3(e.x0),
                    r3(e.x1),
                    r3(span),
                    r3(e.y0),
                    r3(e.y1),
                    r.instance_size,
                    r.derived_in.as_ref().map(|(i, p)| format!("instance={i} path={p:?}")).unwrap_or_default()
                );
            }
        }
        println!("  {title} LINE: band x-span == size.x and starts at 0: {fit}; do not: {misfit}; thickness != weight: {thick_bad}; classes of the misfits: {classes:?}");
    }

    // 5b. Which ratio does a mismatched derived LINE band have to its size?
    {
        let mut ratios: BTreeMap<String, usize> = BTreeMap::new();
        for r in recs.iter().filter(|r| r.ty == "LINE" && r.derived_in.is_some()) {
            let Some(e) = extent_of(&blobs, &r.stroke_geo) else { continue };
            let span = e.x1 - e.x0;
            if (span - r.size.0).abs() > 0.02 + 0.001 * r.size.0 && r.size.0 > 0.0 {
                *ratios.entry(format!("size.x={} main={} span={}", r3(r.size.0), r3(r.main_size.0), r3(span))).or_default() += 1;
            }
        }
        let mut by_own: BTreeMap<String, usize> = BTreeMap::new();
        for r in recs.iter().filter(|r| r.ty == "LINE" && r.derived_in.is_some()) {
            let Some(e) = extent_of(&blobs, &r.stroke_geo) else { continue };
            let span = e.x1 - e.x0;
            let fits_size = (span - r.size.0).abs() <= 0.02 + 0.001 * r.size.0;
            let fits_instance = r.instance_size.is_some_and(|s| (span - s.0).abs() <= 0.02);
            *by_own
                .entry(format!(
                    "derived entry states own size: {}; band span == that size: {}; band span == instance width: {}",
                    r.own_size, fits_size, fits_instance
                ))
                .or_default() += 1;
        }
        println!("  derived LINE entries by whether they state a size:");
        for (k, n) in &by_own {
            println!("      {k}: {n}");
        }
        let mut v: Vec<_> = ratios.into_iter().collect();
        v.sort_by(|a, b| b.1.cmp(&a.1));
        println!("  most common mismatching (derived size.x, main size.x, band span): {:?}", &v[..v.len().min(8)]);
    }

    // 5a. Orientation of the LINE nodes themselves: the band is in local space, so rotation and flips must not matter.
    {
        let mut by: BTreeMap<String, (usize, String)> = BTreeMap::new();
        for r in recs.iter().filter(|r| r.ty == "LINE" && r.derived_in.is_none()) {
            let Some(t) = r.transform else { continue };
            let det = t[0] * t[4] - t[1] * t[3];
            let rotated = t[1].abs() > 1e-3 || t[3].abs() > 1e-3;
            let key = format!("rotated={rotated} mirrored(det<0)={}", det < 0.0);
            by.entry(key).or_insert((0, r.guid.clone())).0 += 1;
        }
        println!("  direct LINE nodes by orientation of their own transform: {by:?} (all bands y in [-w,0], see 2b)");
    }

    // 5b2. Round-cap LINE outlines: is the end shape the same wherever the band ends (resized or not)?
    {
        let signature = |r: &LineRec| -> Option<Vec<String>> {
            let e = extent_of(&blobs, &r.stroke_geo)?;
            let w = r.props.weight?;
            let span = e.x1 - e.x0;
            let mut set: Vec<String> = Vec::new();
            for &(idx, odd) in &r.stroke_geo {
                for cmd in &commands_blob_to_path(&blobs[idx], odd).cmds {
                    let pts: Vec<Vec2> = match *cmd {
                        PathCmd::MoveTo(p) | PathCmd::LineTo(p) => vec![p],
                        PathCmd::QuadTo(c, p) => vec![c, p],
                        PathCmd::CubicTo(a, b, p) => vec![a, b, p],
                        PathCmd::Close => vec![],
                    };
                    for p in pts {
                        let dx = if p.x < span / 2.0 { p.x } else { p.x - span };
                        set.push(format!("{},{}", r3(dx / w), r3(p.y / w)));
                    }
                }
            }
            set.sort();
            set.dedup();
            Some(set)
        };
        let reference = recs.iter().filter(|r| r.ty == "LINE" && r.derived_in.is_some() && r.props.cap.as_deref() == Some("ROUND")).find_map(&signature);
        let mut sigs: BTreeMap<String, (usize, String, f32, f32)> = BTreeMap::new();
        for r in recs.iter().filter(|r| r.ty == "LINE" && r.props.cap.as_deref() == Some("ROUND")) {
            if let Some(sig) = signature(r) {
                let e = sigs.entry(sig.join(" ")).or_insert((0, r.guid.clone(), r.props.weight.unwrap_or(0.0), r.size.0));
                e.0 += 1;
            }
        }
        for (sig, (n, g, w, l)) in &sigs {
            println!("  round LINE signature x{n} e.g. {g} w={w} L={l}: {sig}");
        }
        let mut shown_paths = 0usize;
        for r in recs.iter().filter(|r| r.ty == "LINE" && r.props.cap.as_deref() == Some("ROUND") && r.derived_in.is_some()) {
            let resized = !r.own_size && extent_of(&blobs, &r.stroke_geo).is_some_and(|e| ((e.x1 - e.x0) - r.size.0).abs() > 0.02);
            if shown_paths >= 4 || (shown_paths % 2 == 1) != resized {
                continue;
            }
            shown_paths += 1;
            let w = r.props.weight.unwrap_or(1.0);
            println!("  derived round LINE {} resized={resized} size.x={} instance={:?}:", r.guid, r3(r.size.0), r.derived_in.as_ref().map(|(i, _)| i));
            for &(idx, odd) in &r.stroke_geo {
                println!("      {:?}", commands_blob_to_path(&blobs[idx], odd).cmds.iter().map(|c| format!("{c:?}")).collect::<Vec<_>>());
                let _ = w;
            }
        }
        let (mut same, mut other) = (0usize, 0usize);
        let (mut same_resized, mut other_resized) = (0usize, 0usize);
        for r in recs.iter().filter(|r| r.ty == "LINE" && r.props.cap.as_deref() == Some("ROUND")) {
            let sig = signature(r);
            let resized = r.derived_in.is_some() && !r.own_size && extent_of(&blobs, &r.stroke_geo).is_some_and(|e| ((e.x1 - e.x0) - r.size.0).abs() > 0.02);
            if sig == reference {
                if resized {
                    same_resized += 1
                } else {
                    same += 1
                }
            } else if resized {
                other_resized += 1
            } else {
                other += 1
            }
        }
        println!("  ROUND-cap LINE end shape (points relative to the nearest end, / w) equals the reference 1076:1145: {same} unresized match, {other} differ; resized to the instance width: {same_resized} match, {other_resized} differ");
    }

    // 5c. Flat VECTOR bands against size, by orientation (horizontal: size.y == 0; vertical: size.x == 0).
    println!("\n== 5c. Flat VECTORs: band against size, by orientation ==");
    for derived_pass in [false, true] {
        let mut by: BTreeMap<String, usize> = BTreeMap::new();
        for r in recs.iter().filter(|r| r.ty == "VECTOR" && r.derived_in.is_some() == derived_pass) {
            let (Some(e), Some(w)) = (extent_of(&blobs, &r.stroke_geo), r.props.weight) else { continue };
            let cap = r.props.cap.as_deref().unwrap_or("NONE");
            let horizontal = r.size.1.abs() < 1e-4 && r.size.0.abs() >= 1e-4;
            let (along, across) = if horizontal { (e.x1 - e.x0, e.y1 - e.y0) } else { (e.y1 - e.y0, e.x1 - e.x0) };
            let len = if horizontal { r.size.0 } else { r.size.1 };
            let key = format!(
                "{} cap={cap} weight={}: across/w={} (along - len)/w={}",
                if horizontal { "horizontal" } else { "vertical" },
                r3(w),
                r3(across / w),
                r3((along - len) / w)
            );
            *by.entry(key).or_default() += 1;
        }
        println!("  {}:", if derived_pass { "derived" } else { "direct" });
        for (k, n) in by {
            if n > 0 {
                println!("      {k}: {n}");
            }
        }
    }

    // 3b. Arrow caps: weights seen and the outline of each direct example, normalised by w.
    println!("\n== 3b. ARROW_LINES (the only arrow kind in the kit): weights and outlines ==");
    let mut aw: BTreeMap<String, usize> = BTreeMap::new();
    for r in recs.iter().filter(|r| r.props.cap.as_deref() == Some("ARROW_LINES")) {
        *aw.entry(r3(r.props.weight.unwrap_or(f32::NAN))).or_default() += 1;
    }
    println!("  weights of arrow-cap lines (direct + derived): {aw:?}");
    for r in recs.iter().filter(|r| r.props.cap.as_deref() == Some("ARROW_LINES") && r.derived_in.is_none() && !r.stroke_geo.is_empty()).take(3) {
        let w = r.props.weight.unwrap_or(1.0);
        println!("  {} w={} size=({},{}) transform={:?} vertex caps={:?}", r.guid, r3(w), r3(r.size.0), r3(r.size.1), r.transform, r.vector_caps);
        for &(idx, odd) in &r.stroke_geo {
            let path = commands_blob_to_path(&blobs[idx], odd);
            let mut line = String::new();
            for cmd in &path.cmds {
                use std::fmt::Write;
                let _ = match *cmd {
                    PathCmd::MoveTo(p) => write!(line, " M({},{})", r3(p.x / w), r3(p.y / w)),
                    PathCmd::LineTo(p) => write!(line, " L({},{})", r3(p.x / w), r3(p.y / w)),
                    PathCmd::QuadTo(c, p) => write!(line, " Q({},{} {},{})", r3(c.x / w), r3(c.y / w), r3(p.x / w), r3(p.y / w)),
                    PathCmd::CubicTo(c1, c2, p) => {
                        write!(line, " C({},{} {},{} {},{})", r3(c1.x / w), r3(c1.y / w), r3(c2.x / w), r3(c2.y / w), r3(p.x / w), r3(p.y / w))
                    }
                    PathCmd::Close => write!(line, " Z"),
                };
            }
            println!("      (/w, line starts at x=0, far end x=L/w={}){line}", r3(r.size.0 / w));
        }
    }

    // 3c. One outline per (type, align, cap), normalised by w, for the shapes that are not arrows.
    println!("\n== 3c. Outlines per (type, align, cap, weight), points / w ==");
    let mut seen: Vec<(String, String, String, String)> = Vec::new();
    for r in &recs {
        if r.stroke_geo.is_empty() || r.props.cap.as_deref() == Some("ARROW_LINES") || r.derived_in.is_some() {
            continue;
        }
        let Some(w) = r.props.weight else { continue };
        let horizontal = r.size.1.abs() < 1e-4 && r.size.0.abs() >= 1e-4;
        if !horizontal {
            continue;
        }
        let tag = (r.ty.clone(), r.props.align.clone().unwrap_or_default(), r.props.cap.clone().unwrap_or_default(), r3(w));
        if seen.contains(&tag) || r.props.dashes.as_ref().is_some_and(|d| !d.is_empty()) {
            continue;
        }
        seen.push(tag.clone());
        println!("  [{} {} cap={} w={}] {} L/w={}", tag.0, tag.1, tag.2, tag.3, r.guid, r3(r.size.0 / w));
        for &(idx, odd) in &r.stroke_geo {
            let path = commands_blob_to_path(&blobs[idx], odd);
            let mut line = String::new();
            for cmd in &path.cmds {
                use std::fmt::Write;
                let _ = match *cmd {
                    PathCmd::MoveTo(p) => write!(line, " M({},{})", r3(p.x / w), r3(p.y / w)),
                    PathCmd::LineTo(p) => write!(line, " L({},{})", r3(p.x / w), r3(p.y / w)),
                    PathCmd::QuadTo(c, p) => write!(line, " Q({},{} {},{})", r3(c.x / w), r3(c.y / w), r3(p.x / w), r3(p.y / w)),
                    PathCmd::CubicTo(c1, c2, p) => {
                        write!(line, " C({},{} {},{} {},{})", r3(c1.x / w), r3(c1.y / w), r3(c2.x / w), r3(c2.y / w), r3(p.x / w), r3(p.y / w))
                    }
                    PathCmd::Close => write!(line, " Z"),
                };
            }
            println!("     {line}");
        }
    }

    // The two LINE nodes that store a ROUND cap.
    println!("\n== LINE nodes with strokeCap ROUND ==");
    for r in recs.iter().filter(|r| r.ty == "LINE" && r.derived_in.is_none() && r.props.cap.as_deref() == Some("ROUND")) {
        println!("  {} name={:?} w={:?} size={:?} transform={:?}", r.guid, r.name, r.props.weight, r.size, r.transform);
    }

    // 2b. Formula checks.
    println!("\n== 2b. Formula checks against every record with stroke geometry ==");
    let tol = 0.01f32;
    let (mut line_ok, mut line_bad) = (0usize, 0usize);
    let mut line_bad_examples: Vec<String> = Vec::new();
    let (mut hv_ok, mut hv_bad) = (0usize, 0usize);
    let (mut cap_ok, mut cap_bad) = (0usize, 0usize);
    for r in &recs {
        let (Some(e), Some(w)) = (extent_of(&blobs, &r.stroke_geo), r.props.weight) else { continue };
        let cap = r.props.cap.as_deref().unwrap_or("NONE");
        if r.ty == "LINE" {
            // y in [-w, 0]
            if (e.y0 + w).abs() < tol && e.y1.abs() < tol {
                line_ok += 1;
            } else {
                line_bad += 1;
                if line_bad_examples.len() < 5 {
                    line_bad_examples.push(format!("{} w={} y=[{},{}]", r.guid, r3(w), r3(e.y0), r3(e.y1)));
                }
            }
            // caps: ROUND stays inside [0, band length]; NONE is the plain rectangle
            let span = e.x1 - e.x0;
            let ok = e.x0.abs() < tol && (cap == "NONE" || cap == "ROUND" || cap == "SQUARE") && span > 0.0;
            if ok {
                cap_ok += 1;
            } else {
                cap_bad += 1;
            }
        } else if r.props.align.as_deref() == Some("CENTER") && !matches!(cap, "ARROW_LINES") {
            let horizontal = r.size.1.abs() < 1e-4 && r.size.0.abs() >= 1e-4;
            let across = if horizontal { (e.y0, e.y1) } else { (e.x0, e.x1) };
            // across the line the band is centered when its thickness equals the effective weight, which is
            // measured from the band itself for entries whose weight comes from a nested override
            let thick = across.1 - across.0;
            if (across.0 + thick / 2.0).abs() < tol && (across.1 - thick / 2.0).abs() < tol && !(horizontal && e.y1 - e.y0 > 20.0) {
                hv_ok += 1;
            } else if (cap == "ROUND" || cap == "NONE") && !(horizontal) {
                // vertical flat vector: across is x, the run is y
                hv_bad += 1;
            } else {
                hv_bad += 1;
            }
        }
    }
    println!("  LINE band y == [-w, 0] (+-{tol}): {line_ok} match, {line_bad} mismatch {line_bad_examples:?}");
    println!("  LINE band starts at x = 0 (caps inside the stored length): {cap_ok} match, {cap_bad} mismatch");
    println!("  flat VECTOR band centred across the line: {hv_ok} match, {hv_bad} not (the mismatches are vertical vectors whose y-run starts below 0 or round caps, see 5c)");

    // 5d. Flat VECTORs whose band thickness is not the main node's weight: does an override explain it?
    println!("\n== 5d. Flat VECTOR bands thinner/thicker than the main node's weight ==");
    let mut explained = 0usize;
    let mut unexplained = 0usize;
    let mut shown = 0usize;
    for r in recs.iter().filter(|r| r.ty == "VECTOR" && r.derived_in.is_some()) {
        let (Some(e), Some(w)) = (extent_of(&blobs, &r.stroke_geo), r.props.weight) else { continue };
        let horizontal = r.size.1.abs() < 1e-4 && r.size.0.abs() >= 1e-4;
        let across = if horizontal { e.y1 - e.y0 } else { e.x1 - e.x0 };
        if r.props.cap.as_deref() == Some("ARROW_LINES") || (across - w).abs() < 0.01 || across > 4.0 * w {
            continue;
        }
        println!(
            "      stored weight on main node {:?}, effective weight after the instance's override at this path {:?}, band thickness {}, nested instance (derived size, its main component size) {:?}",
            r.main_weight,
            r.props.weight,
            r3(across),
            r.nested
        );
        let hit = r.weight_overrides.iter().any(|(_, v)| (v - across).abs() < 0.01);
        if hit {
            explained += 1;
        } else {
            unexplained += 1;
        }
        if shown < 6 {
            shown += 1;
            println!(
                "  {} main weight {} band thickness {} size={:?} overrides naming that weight in the instance: {} (instance {:?}, instance size {:?}, its main component size {:?}, band/weight = {})",
                r.guid,
                r3(w),
                r3(across),
                r.size,
                hit,
                r.derived_in.as_ref().map(|(i, _)| i),
                r.instance_size,
                r.symbol_size,
                r3(across / w)
            );
        }
    }
    let mut scale_hits = 0usize;
    let mut scale_total = 0usize;
    for r in recs.iter().filter(|r| r.ty == "VECTOR" && r.derived_in.is_some()) {
        let (Some(e), Some(w), Some(is), Some(ss)) = (extent_of(&blobs, &r.stroke_geo), r.props.weight, r.instance_size, r.symbol_size) else { continue };
        let horizontal = r.size.1.abs() < 1e-4 && r.size.0.abs() >= 1e-4;
        let across = if horizontal { e.y1 - e.y0 } else { e.x1 - e.x0 };
        if r.props.cap.as_deref() == Some("ARROW_LINES") || (across - w).abs() < 0.01 || across > 4.0 * w || ss.0 <= 0.0 {
            continue;
        }
        scale_total += 1;
        if (across / w - is.0 / ss.0).abs() < 0.01 || (across / w - is.1 / ss.1.max(1e-6)).abs() < 0.01 {
            scale_hits += 1;
        }
    }
    println!("  band/weight equals instance size / main component size (either axis): {scale_hits} of {scale_total}");
    println!("  band thickness explained by a strokeWeight override of the instance: {explained}; not explained: {unexplained}");
}
