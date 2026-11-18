//! Maps a `NodeChange` key to the feature row of the coverage matrix it belongs to (section 2 of the matrix).

pub fn feature_of(key: &str) -> &'static str {
    let k = key;
    let has = |p: &str| k.contains(p);
    let starts = |p: &str| k.starts_with(p);
    match k {
        "guid" | "phase" | "parentIndex" | "type" | "name" | "overrideKey" => "Identity and tree",
        "visible" | "locked" => "Visibility and lock",
        "opacity" => "Layer opacity",
        "blendMode" => "Layer blend modes",
        "size" | "transform" => "Size and transform (rotation and flip)",
        "mask" | "isMask" | "maskType" | "maskIsOutline" => "Masks",
        "frameMaskDisabled" => "Clip content",
        "horizontalConstraint" | "verticalConstraint" => "Constraints",
        "layoutGrids" => "Layout grids",
        "exportSettings" => "Export settings",
        "fillGeometry" | "strokeGeometry" => "Stored geometry (fillGeometry / strokeGeometry)",
        "vectorData" | "vectorNetwork" => "Vector / vector network",
        "arcData" => "Ellipse incl. arcs/rings",
        "count" | "pointCount" | "starInnerScale" | "starInnerRadius" => "Polygon / Star",
        "booleanOperation" => "Boolean operations",
        "effects" => "Effects",
        "derivedTextData" | "textData" => "Text node (basics)",
        "derivedSymbolData" | "symbolData" | "overriddenSymbolID" => "Instances",
        "cornerSmoothing" => "Corner smoothing",
        _ if starts("fillPaints") || starts("backgroundColor") => "Fills",
        _ if starts("stroke") || starts("border") || k == "dashPattern" || k == "miterLimit" => "Strokes",
        _ if has("CornerRadius") || k == "cornerRadius" || k == "rectangleCornerRadiiIndependent" => "Corner radius",
        _ if starts("stack") || starts("layoutSizing") || k == "bordersTakeSpace" || k == "minSize" || k == "maxSize" => "Auto layout",
        _ if starts("grid") => "Auto layout grid",
        _ if starts("text")
            || starts("font")
            || starts("letter")
            || starts("line")
            || starts("paragraph")
            || starts("list")
            || starts("leading")
            || k == "maxLines"
            || k == "hangingPunctuation"
            || k == "hangingList"
            || k == "autoRename" =>
        {
            "Text"
        }
        _ if starts("component")
            || starts("symbol")
            || starts("variant")
            || starts("stateGroup")
            || starts("isStateGroup")
            || k == "propsAreBubbled"
            || k == "overrideLevel"
            || starts("isSlot")
            || k == "detachedSymbolId"
            || starts("sharedSymbol")
            || k == "isSymbolPublishable" =>
        {
            "Components"
        }
        _ if starts("variable") || starts("parameter") || k == "codeSyntax" => "Variables",
        _ if starts("styleId") || starts("inherit") || starts("style") || k == "key" || starts("isSoftDeleted") || k == "sortPosition" => "Styles",
        _ if starts("prototype")
            || starts("transition")
            || has("Interaction")
            || starts("scroll")
            || starts("overflow")
            || starts("flowStartingPoints")
            || starts("overlay") =>
        {
            "Prototype"
        }
        _ if starts("plugin") || starts("editInfo") || starts("version") || starts("userFacingVersion") || starts("publish") || has("Version") => {
            "Provenance metadata"
        }
        _ if starts("dev") || starts("handoff") || starts("sectionStatus") || starts("annotation") => "Dev Mode",
        _ => "Other",
    }
}
