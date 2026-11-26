//! Write passive, editable annotation objects and deterministic appearance streams.
use pdf_application::export::{ExportError, ExportPlan};
use pdf_domain::annotation::{
    Annotation, AnnotationExtra, AnnotationKind, FillSymbol, MAX_SIGNATURE_PLACEMENTS,
    MAX_SIGNATURES, PREFIX, ParsedName, ShapeKind, SignatureImage,
};
use serde_json::{Map, Value, json};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::Arc,
};

pub(crate) struct SignatureBudget {
    full_left: usize,
    placements_left: usize,
    full_signatures: HashMap<u64, (u32, u32)>,
    refs: Vec<(u64, u32, u32)>,
}

impl SignatureBudget {
    pub(crate) fn new() -> Self {
        Self {
            full_left: MAX_SIGNATURES,
            placements_left: MAX_SIGNATURE_PLACEMENTS,
            full_signatures: HashMap::new(),
            refs: Vec::new(),
        }
    }

    pub(crate) fn verify_refs(&self) -> Result<(), ExportError> {
        for &(ref_id, width, height) in &self.refs {
            if self.full_signatures.get(&ref_id) != Some(&(width, height)) {
                return Err(ExportError::Inspection);
            }
        }
        Ok(())
    }
}

pub(crate) fn owned_name(dict: &Map<String, Value>) -> Option<&str> {
    let name = dict.get("/NM")?.as_str()?.strip_prefix("u:")?;
    name.starts_with(PREFIX).then_some(name)
}

fn subtype_kind(dict: &Map<String, Value>) -> Option<AnnotationKind> {
    match dict.get("/Subtype").and_then(Value::as_str)? {
        "/Stamp" => Some(AnnotationKind::Stamp),
        "/FreeText" => Some(AnnotationKind::FreeText),
        "/Highlight" => Some(AnnotationKind::Highlight),
        "/Underline" => Some(AnnotationKind::Underline),
        "/StrikeOut" => Some(AnnotationKind::StrikeOut),
        "/Square" => Some(AnnotationKind::Square),
        "/Circle" => Some(AnnotationKind::Circle),
        "/Line" => Some(AnnotationKind::Line),
        "/Text" => Some(AnnotationKind::Text),
        "/Ink" => Some(AnnotationKind::Ink),
        _ => None,
    }
}

fn owned(dict: &Map<String, Value>) -> Option<ParsedName> {
    let kind = subtype_kind(dict)?;
    Annotation::parse_with_kind(owned_name(dict)?, Some(kind))
}

pub(crate) fn validate(
    dict: &Map<String, Value>,
    budget: &mut SignatureBudget,
) -> Result<(), ExportError> {
    let subtype = dict.get("/Subtype").and_then(Value::as_str);
    let base_keys = [
        "/Type",
        "/Subtype",
        "/Rect",
        "/NM",
        "/C",
        "/CA",
        "/F",
        "/BS",
        "/Border",
        "/InkList",
        "/QuadPoints",
        "/AP",
        "/P",
    ];
    let allowed_key = |k: &str| {
        base_keys.contains(&k)
            || (subtype == Some("/FreeText") && matches!(k, "/Contents" | "/DA"))
            || (subtype == Some("/Text") && matches!(k, "/Contents" | "/Name" | "/Open"))
            || (subtype == Some("/Line") && matches!(k, "/L" | "/LE"))
    };
    if dict.keys().any(|k| !allowed_key(k.as_str()))
        || dict.get("/Type").and_then(Value::as_str) != Some("/Annot")
    {
        return Err(ExportError::Unsupported(
            "anotasi diubah di luar subset pasif yang didukung.",
        ));
    }
    let kind = subtype_kind(dict).ok_or(ExportError::Unsupported(
        "anotasi diubah di luar subset pasif yang didukung.",
    ))?;
    let parsed =
        Annotation::parse_with_kind(owned_name(dict).ok_or(ExportError::Inspection)?, Some(kind))
            .ok_or(ExportError::Inspection)?;
    if matches!(
        &parsed,
        ParsedName::SignatureRef { .. }
            | ParsedName::Complete(Annotation { signature: Some(_), .. })
    ) {
        budget.placements_left = budget
            .placements_left
            .checked_sub(1)
            .ok_or(ExportError::ResourceLimit)?;
    }
    for key in ["/Rect", "/C", "/QuadPoints", "/L"] {
        if let Some(v) = dict.get(key)
            && !v.as_array().is_some_and(|v| {
                v.len() <= 16384 && v.iter().all(|n| n.as_f64().is_some_and(f64::is_finite))
            })
        {
            return Err(ExportError::Inspection);
        }
    }
    let (color, highlight) = match &parsed {
        ParsedName::Complete(a) => (a.color, a.highlight),
        ParsedName::SignatureRef { color, .. } => (*color, false),
    };
    if dict.get("/Rect").and_then(Value::as_array).map(Vec::len) != Some(4)
        || !dict.get("/C").and_then(Value::as_array).is_some_and(|v| {
            v.len() == 3
                && v.iter().zip(color).all(|(v, c)| {
                    v.as_f64()
                        .is_some_and(|v| (v - f64::from(c) / 255.).abs() < 0.00001)
                })
        })
        || dict.get("/F").and_then(Value::as_u64) != Some(4)
        || dict.get("/CA").and_then(Value::as_f64) != Some(if highlight { 0.5 } else { 1. })
    {
        return Err(ExportError::Inspection);
    }
    match parsed {
        ParsedName::SignatureRef {
            image_width,
            image_height,
            ref_id,
            ..
        } => {
            budget.refs.push((ref_id, image_width, image_height));
            Ok(())
        }
        ParsedName::Complete(a) => {
            if let Some(image) = &a.signature {
                budget.full_left = budget
                    .full_left
                    .checked_sub(1)
                    .ok_or(ExportError::ResourceLimit)?;
                budget
                    .full_signatures
                    .insert(a.id, (image.width, image.height));
                return Ok(());
            }
            if let Some(text) = &a.text {
                let expected_contents = format!("u:{}", text.content);
                if dict.get("/Contents").and_then(Value::as_str) != Some(expected_contents.as_str())
                {
                    return Err(ExportError::Inspection);
                }
                let da = dict
                    .get("/DA")
                    .and_then(Value::as_str)
                    .ok_or(ExportError::Inspection)?;
                if da.len() > 64 || !da.starts_with("u:/Helv ") || !da.ends_with(" rg") {
                    return Err(ExportError::Inspection);
                }
                return Ok(());
            }
            if let Some(extra) = &a.extra {
                match extra {
                    AnnotationExtra::Underline | AnnotationExtra::StrikeOut => {
                        if dict
                            .get("/QuadPoints")
                            .and_then(Value::as_array)
                            .map(Vec::len)
                            != Some(a.points.len() * 4)
                        {
                            return Err(ExportError::Inspection);
                        }
                    }
                    AnnotationExtra::Shape(ShapeKind::Line | ShapeKind::Arrow) => {
                        if dict.get("/L").and_then(Value::as_array).map(Vec::len) != Some(4) {
                            return Err(ExportError::Inspection);
                        }
                    }
                    AnnotationExtra::Note(content) => {
                        let expected_contents = format!("u:{content}");
                        if dict.get("/Contents").and_then(Value::as_str)
                            != Some(expected_contents.as_str())
                        {
                            return Err(ExportError::Inspection);
                        }
                    }
                    AnnotationExtra::Shape(ShapeKind::Rectangle | ShapeKind::Ellipse)
                    | AnnotationExtra::Symbol(_) => {}
                }
                return Ok(());
            }
            if a.rectangles {
                if dict
                    .get("/QuadPoints")
                    .and_then(Value::as_array)
                    .map(Vec::len)
                    != Some(a.points.len() * 4)
                {
                    return Err(ExportError::Inspection);
                }
            } else {
                let strokes = dict
                    .get("/InkList")
                    .and_then(Value::as_array)
                    .ok_or(ExportError::Inspection)?;
                if strokes.len() != 1
                    || !strokes[0].as_array().is_some_and(|p| {
                        p.len() == a.points.len() * 2
                            && p.iter().all(|n| n.as_f64().is_some_and(f64::is_finite))
                    })
                {
                    return Err(ExportError::Inspection);
                }
            }
            Ok(())
        }
    }
}

pub(crate) fn inherited<'a>(
    objects: &'a Map<String, Value>,
    mut page: &'a Value,
    key: &str,
) -> Option<&'a Value> {
    for _ in 0..64 {
        if let Some(v) = page.get(key) {
            return Some(v);
        }
        page = objects
            .get(&format!("obj:{}", page.get("/Parent")?.as_str()?))?
            .get("value")?;
    }
    None
}

fn write_rgba_xobject(
    image: &SignatureImage,
    next: &mut u64,
    work: &Path,
    updates: &mut Map<String, Value>,
) -> Result<String, ExportError> {
    let image_ref = format!("{} 0 R", *next);
    *next += 1;
    let mask_ref = format!("{} 0 R", *next);
    *next += 1;
    let rgb = work.join(format!("annot-img-{}.rgb", *next));
    let alpha = work.join(format!("annot-img-{}.alpha", *next));
    std::fs::write(
        &rgb,
        image
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|p| p[..3].iter().copied())
            .collect::<Vec<_>>(),
    )
    .map_err(|_| ExportError::Io)?;
    std::fs::write(
        &alpha,
        image
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| p[3])
            .collect::<Vec<_>>(),
    )
    .map_err(|_| ExportError::Io)?;
    updates.insert(
        format!("obj:{image_ref}"),
        json!({"stream":{"dict":{"/Type":"/XObject","/Subtype":"/Image","/Width":image.width,"/Height":image.height,"/ColorSpace":"/DeviceRGB","/BitsPerComponent":8,"/Interpolate":true,"/SMask":mask_ref},"datafile":rgb.to_str().ok_or(ExportError::Io)?}}),
    );
    updates.insert(
        format!("obj:{mask_ref}"),
        json!({"stream":{"dict":{"/Type":"/XObject","/Subtype":"/Image","/Width":image.width,"/Height":image.height,"/ColorSpace":"/DeviceGray","/BitsPerComponent":8,"/Interpolate":true},"datafile":alpha.to_str().ok_or(ExportError::Io)?}}),
    );
    Ok(image_ref)
}

pub(crate) fn patch(doc: &Value, plan: &ExportPlan, work: &Path) -> Result<Value, ExportError> {
    let objects = doc
        .pointer("/qpdf/1")
        .and_then(Value::as_object)
        .ok_or(ExportError::Inspection)?;
    let pages = doc
        .get("pages")
        .and_then(Value::as_array)
        .ok_or(ExportError::Inspection)?;
    if pages.len() != plan.pages().len() {
        return Err(ExportError::Validation);
    }
    let mut next = objects
        .keys()
        .filter_map(|s| {
            s.strip_prefix("obj:")?
                .split(' ')
                .next()?
                .parse::<u64>()
                .ok()
        })
        .max()
        .unwrap_or(0)
        + 1;

    // Count stamp ID occurrences across the output plan so `ref:<id>` only targets unique IDs.
    let mut stamp_id_counts: HashMap<u64, usize> = HashMap::new();
    for page in plan.pages() {
        for a in &page.annotations {
            if a.signature.is_some() {
                *stamp_id_counts.entry(a.id).or_insert(0) += 1;
            }
        }
    }

    let mut written_signatures: Vec<(Arc<SignatureImage>, Option<u64>, String)> = Vec::new();
    let mut written_text_images: HashMap<(String, u32, [u8; 3]), String> = HashMap::new();
    let mut used_ref_targets: HashSet<u64> = HashSet::new();
    let mut updates = Map::new();
    for (index, p) in pages.iter().enumerate() {
        let page_ref = p["object"].as_str().ok_or(ExportError::Inspection)?;
        let key = format!("obj:{page_ref}");
        let original = &objects[&key]["value"];
        let mut page = original.clone();
        let mut annots = Vec::new();
        if let Some(existing) = page.get("/Annots") {
            let list = if let Some(r) = existing.as_str() {
                &objects[&format!("obj:{r}")]["value"]
            } else {
                existing
            };
            for reference in list.as_array().ok_or(ExportError::Inspection)? {
                let value = if let Some(r) = reference.as_str() {
                    &objects[&format!("obj:{r}")]["value"]
                } else {
                    reference
                };
                if !value.as_object().is_some_and(|v| owned(v).is_some()) {
                    annots.push(reference.clone());
                }
            }
        }
        let bounds = inherited(objects, original, "/CropBox")
            .or_else(|| inherited(objects, original, "/MediaBox"))
            .and_then(Value::as_array)
            .ok_or(ExportError::Inspection)?;
        if bounds.len() != 4 {
            return Err(ExportError::Inspection);
        }
        let b: Vec<f64> = bounds
            .iter()
            .map(|v| v.as_f64().ok_or(ExportError::Inspection))
            .collect::<Result<_, _>>()?;
        let (x, y, w, h) = (b[0], b[1], b[2] - b[0], b[3] - b[1]);
        if !w.is_finite() || !h.is_finite() || w <= 0. || h <= 0. {
            return Err(ExportError::Inspection);
        }
        for a in &plan.pages()[index].annotations {
            if !a.valid() {
                return Err(ExportError::InvalidPlan);
            }
            let points: Vec<_> = a
                .points
                .iter()
                .map(|p| [x + f64::from(p[0]) * w, y + (1. - f64::from(p[1])) * h])
                .collect();
            if let Some(image) = &a.signature {
                let rect = [points[0][0], points[1][1], points[1][0], points[0][1]];
                let annot_ref = format!("{next} 0 R");
                next += 1;
                let ap_ref = format!("{next} 0 R");
                next += 1;
                let (image_ref, nm) = if let Some((_, first_id, existing_ref)) = written_signatures
                    .iter()
                    .find(|(existing, _, _)| Arc::ptr_eq(existing, image) || **existing == **image)
                {
                    let nm = first_id
                        .filter(|&target| target != a.id)
                        .and_then(|target| a.name_with_signature_ref(target))
                        .unwrap_or_else(|| a.name());
                    (existing_ref.clone(), nm)
                } else {
                    let image_ref = write_rgba_xobject(image, &mut next, work, &mut updates)?;
                    let unique_id = (stamp_id_counts.get(&a.id).copied() == Some(1)
                        && used_ref_targets.insert(a.id))
                    .then_some(a.id);
                    written_signatures.push((Arc::clone(image), unique_id, image_ref.clone()));
                    (image_ref, a.name())
                };
                let ap = work.join(format!("signature-{next}.ap"));
                std::fs::write(
                    &ap,
                    format!(
                        "q {} 0 0 {} {} {} cm /Sig Do Q",
                        rect[2] - rect[0],
                        rect[3] - rect[1],
                        rect[0],
                        rect[1]
                    ),
                )
                .map_err(|_| ExportError::Io)?;
                updates.insert(
                    format!("obj:{annot_ref}"),
                    json!({"value":{"/Type":"/Annot","/Subtype":"/Stamp","/Rect":rect,"/NM":format!("u:{nm}"),"/C":[0,0,0],"/CA":1,"/F":4,"/P":page_ref,"/AP":{"/N":ap_ref}}}),
                );
                updates.insert(
                    format!("obj:{ap_ref}"),
                    json!({"stream":{"dict":{"/Type":"/XObject","/Subtype":"/Form","/BBox":rect,"/Resources":{"/XObject":{"/Sig":image_ref}}},"datafile":ap.to_str().ok_or(ExportError::Io)?}}),
                );
                annots.push(json!(annot_ref));
                continue;
            }
            if let Some(text) = &a.text {
                let rect = [points[0][0], points[1][1], points[1][0], points[0][1]];
                let annot_ref = format!("{next} 0 R");
                next += 1;
                let ap_ref = format!("{next} 0 R");
                next += 1;
                let cache_key = (text.content.clone(), text.font_size.to_bits(), a.color);
                let image_ref = if let Some(existing_ref) = written_text_images.get(&cache_key) {
                    existing_ref.clone()
                } else {
                    let bitmap = pdf_application::annotation::rasterize_text(
                        &text.content,
                        text.font_size,
                        a.color,
                    )
                    .ok_or(ExportError::InvalidPlan)?;
                    let image_ref = write_rgba_xobject(&bitmap, &mut next, work, &mut updates)?;
                    written_text_images.insert(cache_key, image_ref.clone());
                    image_ref
                };
                let ap = work.join(format!("freetext-{next}.ap"));
                std::fs::write(
                    &ap,
                    format!(
                        "q {} 0 0 {} {} {} cm /Sig Do Q",
                        rect[2] - rect[0],
                        rect[3] - rect[1],
                        rect[0],
                        rect[1]
                    ),
                )
                .map_err(|_| ExportError::Io)?;
                let color_norm = a.color.map(|c| f64::from(c) / 255.);
                let da = format!(
                    "u:/Helv {:.2} Tf {:.4} {:.4} {:.4} rg",
                    text.font_size, color_norm[0], color_norm[1], color_norm[2]
                );
                updates.insert(
                    format!("obj:{annot_ref}"),
                    json!({"value":{
                        "/Type":"/Annot",
                        "/Subtype":"/FreeText",
                        "/Rect":rect,
                        "/NM":format!("u:{}", a.name()),
                        "/Contents":format!("u:{}", text.content),
                        "/DA":da,
                        "/C":color_norm,
                        "/CA":1,
                        "/F":4,
                        "/P":page_ref,
                        "/AP":{"/N":ap_ref}
                    }}),
                );
                updates.insert(
                    format!("obj:{ap_ref}"),
                    json!({"stream":{"dict":{"/Type":"/XObject","/Subtype":"/Form","/BBox":rect,"/Resources":{"/XObject":{"/Sig":image_ref}}},"datafile":ap.to_str().ok_or(ExportError::Io)?}}),
                );
                annots.push(json!(annot_ref));
                continue;
            }
            if let Some(extra) = &a.extra {
                let annot_ref = emit_extra_annotation(
                    a,
                    extra,
                    &points,
                    page_ref,
                    &mut next,
                    work,
                    &mut updates,
                )?;
                annots.push(json!(annot_ref));
                continue;
            }
            let margin = if a.rectangles {
                0.
            } else {
                f64::from(a.width) / 2.
            };
            let rect = [
                points.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min) - margin,
                points.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min) - margin,
                points
                    .iter()
                    .map(|p| p[0])
                    .fold(f64::NEG_INFINITY, f64::max)
                    + margin,
                points
                    .iter()
                    .map(|p| p[1])
                    .fold(f64::NEG_INFINITY, f64::max)
                    + margin,
            ];
            let mut appearance = format!(
                "q /GS gs {} {} {} RG {} {} {} rg {} w 1 J 1 j\n",
                f64::from(a.color[0]) / 255.,
                f64::from(a.color[1]) / 255.,
                f64::from(a.color[2]) / 255.,
                f64::from(a.color[0]) / 255.,
                f64::from(a.color[1]) / 255.,
                f64::from(a.color[2]) / 255.,
                a.width
            );
            let mut annotation = json!({"/Type":"/Annot","/Subtype":if a.rectangles {"/Highlight"} else {"/Ink"},"/Rect":rect,"/NM":format!("u:{}",a.name()),"/C":a.color.map(|c|f64::from(c)/255.),"/CA":if a.highlight {0.5} else {1.0},"/F":4,"/P":page_ref,"/BS":{"/W":a.width,"/S":"/S"}});
            if a.rectangles {
                let mut quads = Vec::new();
                for pair in points.as_chunks::<2>().0.iter() {
                    let left = pair[0][0].min(pair[1][0]);
                    let right = pair[0][0].max(pair[1][0]);
                    let bottom = pair[0][1].min(pair[1][1]);
                    let top = pair[0][1].max(pair[1][1]);
                    quads.extend([left, top, right, top, left, bottom, right, bottom]);
                    appearance.push_str(&format!(
                        "{left} {bottom} {} {} re f\n",
                        right - left,
                        top - bottom
                    ));
                }
                annotation["/QuadPoints"] = json!(quads);
            } else {
                annotation["/InkList"] =
                    json!([points.iter().flatten().copied().collect::<Vec<_>>()]);
                appearance.push_str(&format!("{} {} m\n", points[0][0], points[0][1]));
                for p in &points[1..] {
                    appearance.push_str(&format!("{} {} l\n", p[0], p[1]));
                }
                appearance.push_str("S\n");
            }
            appearance.push_str("Q\n");
            let annot_ref = format!("{next} 0 R");
            next += 1;
            let ap_ref = format!("{next} 0 R");
            next += 1;
            annotation["/AP"] = json!({"/N":ap_ref});
            let file = work.join(format!("annotation-{next}.txt"));
            std::fs::write(&file, appearance).map_err(|_| ExportError::Io)?;
            updates.insert(format!("obj:{annot_ref}"), json!({"value":annotation}));
            updates.insert(format!("obj:{ap_ref}"),json!({"stream":{"dict":{"/Type":"/XObject","/Subtype":"/Form","/BBox":rect,"/Resources":{"/ExtGState":{"/GS":{"/Type":"/ExtGState","/CA":if a.highlight {0.5} else {1.0},"/ca":if a.highlight {0.5} else {1.0},"/BM":if a.highlight {"/Multiply"} else {"/Normal"}}}}},"datafile":file.to_str().ok_or(ExportError::Io)?}}));
            annots.push(json!(annot_ref));
        }
        if annots.is_empty() {
            page.as_object_mut()
                .ok_or(ExportError::Inspection)?
                .remove("/Annots");
        } else {
            page["/Annots"] = json!(annots);
        }
        updates.insert(key, json!({"value":page}));
    }
    Ok(json!({"qpdf":[doc["qpdf"][0].clone(),updates]}))
}

fn emit_extra_annotation(
    a: &Annotation,
    extra: &AnnotationExtra,
    points: &[[f64; 2]],
    page_ref: &str,
    next: &mut u64,
    work: &Path,
    updates: &mut Map<String, Value>,
) -> Result<String, ExportError> {
    let color_norm = a.color.map(|c| f64::from(c) / 255.);
    let [r, g, b] = color_norm;
    let w = f64::from(a.width);
    let mut appearance = format!("q {r:.5} {g:.5} {b:.5} RG {r:.5} {g:.5} {b:.5} rg {w:.3} w 1 J 1 j\n");
    let subtype = match extra {
        AnnotationExtra::Underline => "/Underline",
        AnnotationExtra::StrikeOut => "/StrikeOut",
        AnnotationExtra::Shape(ShapeKind::Rectangle) => "/Square",
        AnnotationExtra::Shape(ShapeKind::Ellipse) => "/Circle",
        AnnotationExtra::Shape(ShapeKind::Line | ShapeKind::Arrow) => "/Line",
        AnnotationExtra::Note(_) => "/Text",
        AnnotationExtra::Symbol(_) => "/Stamp",
    };

    let (rect, mut annotation) = match extra {
        AnnotationExtra::Underline | AnnotationExtra::StrikeOut => {
            let strike = matches!(extra, AnnotationExtra::StrikeOut);
            let mut quads = Vec::new();
            let mut min_x = f64::INFINITY;
            let mut min_y = f64::INFINITY;
            let mut max_x = f64::NEG_INFINITY;
            let mut max_y = f64::NEG_INFINITY;
            for pair in points.as_chunks::<2>().0.iter() {
                let left = pair[0][0].min(pair[1][0]);
                let right = pair[0][0].max(pair[1][0]);
                let bottom = pair[0][1].min(pair[1][1]);
                let top = pair[0][1].max(pair[1][1]);
                min_x = min_x.min(left);
                min_y = min_y.min(bottom);
                max_x = max_x.max(right);
                max_y = max_y.max(top);
                quads.extend([left, top, right, top, left, bottom, right, bottom]);
                let line_y = if strike {
                    (bottom + top) * 0.5
                } else {
                    bottom + (top - bottom) * 0.08
                };
                let lw = ((top - bottom) * 0.11).max(w).max(1.0);
                appearance.push_str(&format!("{lw:.3} w {left:.3} {line_y:.3} m {right:.3} {line_y:.3} l S\n"));
            }
            let rect = [min_x, min_y, max_x, max_y];
            let annot = json!({
                "/Type": "/Annot",
                "/Subtype": subtype,
                "/Rect": rect,
                "/NM": format!("u:{}", a.name()),
                "/C": color_norm,
                "/CA": 1.0,
                "/F": 4,
                "/P": page_ref,
                "/BS": {"/W": a.width, "/S": "/S"},
                "/QuadPoints": quads,
            });
            (rect, annot)
        }
        AnnotationExtra::Shape(ShapeKind::Rectangle) => {
            let x0 = points[0][0].min(points[1][0]);
            let x1 = points[0][0].max(points[1][0]);
            let y0 = points[0][1].min(points[1][1]);
            let y1 = points[0][1].max(points[1][1]);
            let pad = w * 0.5 + 1.0;
            let rect = [x0 - pad, y0 - pad, x1 + pad, y1 + pad];
            appearance.push_str(&format!("{x0:.3} {y0:.3} {:.3} {:.3} re S\n", x1 - x0, y1 - y0));
            let annot = json!({
                "/Type": "/Annot",
                "/Subtype": subtype,
                "/Rect": rect,
                "/NM": format!("u:{}", a.name()),
                "/C": color_norm,
                "/CA": 1.0,
                "/F": 4,
                "/P": page_ref,
                "/BS": {"/W": a.width, "/S": "/S"},
            });
            (rect, annot)
        }
        AnnotationExtra::Shape(ShapeKind::Ellipse) => {
            let x0 = points[0][0].min(points[1][0]);
            let x1 = points[0][0].max(points[1][0]);
            let y0 = points[0][1].min(points[1][1]);
            let y1 = points[0][1].max(points[1][1]);
            let pad = w * 0.5 + 1.0;
            let rect = [x0 - pad, y0 - pad, x1 + pad, y1 + pad];
            let cx = (x0 + x1) * 0.5;
            let cy = (y0 + y1) * 0.5;
            let rx = (x1 - x0) * 0.5;
            let ry = (y1 - y0) * 0.5;
            const KAPPA: f64 = 0.5522847498;
            let kx = rx * KAPPA;
            let ky = ry * KAPPA;
            appearance.push_str(&format!(
                "{:.3} {:.3} m\n\
                 {:.3} {:.3} {:.3} {:.3} {:.3} {:.3} c\n\
                 {:.3} {:.3} {:.3} {:.3} {:.3} {:.3} c\n\
                 {:.3} {:.3} {:.3} {:.3} {:.3} {:.3} c\n\
                 {:.3} {:.3} {:.3} {:.3} {:.3} {:.3} c S\n",
                cx + rx, cy,
                cx + rx, cy + ky, cx + kx, cy + ry, cx, cy + ry,
                cx - kx, cy + ry, cx - rx, cy + ky, cx - rx, cy,
                cx - rx, cy - ky, cx - kx, cy - ry, cx, cy - ry,
                cx + kx, cy - ry, cx + rx, cy - ky, cx + rx, cy,
            ));
            let annot = json!({
                "/Type": "/Annot",
                "/Subtype": subtype,
                "/Rect": rect,
                "/NM": format!("u:{}", a.name()),
                "/C": color_norm,
                "/CA": 1.0,
                "/F": 4,
                "/P": page_ref,
                "/BS": {"/W": a.width, "/S": "/S"},
            });
            (rect, annot)
        }
        AnnotationExtra::Shape(ShapeKind::Line | ShapeKind::Arrow) => {
            let p0 = points[0];
            let p1 = points[1];
            let is_arrow = matches!(extra, AnnotationExtra::Shape(ShapeKind::Arrow));
            let head_len = (w * 4.5).max(10.0);
            let mut min_x = p0[0].min(p1[0]);
            let mut max_x = p0[0].max(p1[0]);
            let mut min_y = p0[1].min(p1[1]);
            let mut max_y = p0[1].max(p1[1]);
            appearance.push_str(&format!("{:.3} {:.3} m {:.3} {:.3} l S\n", p0[0], p0[1], p1[0], p1[1]));
            if is_arrow
                && let Some([left, right]) = pdf_application::annotation::arrow_head_wings(
                    [p0[0] as f32, p0[1] as f32],
                    [p1[0] as f32, p1[1] as f32],
                    head_len as f32,
                )
            {
                let l = [f64::from(left[0]), f64::from(left[1])];
                let r_pt = [f64::from(right[0]), f64::from(right[1])];
                for wing in [l, r_pt] {
                    min_x = min_x.min(wing[0]);
                    max_x = max_x.max(wing[0]);
                    min_y = min_y.min(wing[1]);
                    max_y = max_y.max(wing[1]);
                    appearance.push_str(&format!(
                        "{:.3} {:.3} m {:.3} {:.3} l S\n",
                        p1[0], p1[1], wing[0], wing[1]
                    ));
                }
            }
            let pad = w * 0.5 + 2.0;
            let rect = [min_x - pad, min_y - pad, max_x + pad, max_y + pad];
            let mut annot = json!({
                "/Type": "/Annot",
                "/Subtype": subtype,
                "/Rect": rect,
                "/NM": format!("u:{}", a.name()),
                "/C": color_norm,
                "/CA": 1.0,
                "/F": 4,
                "/P": page_ref,
                "/BS": {"/W": a.width, "/S": "/S"},
                "/L": [p0[0], p0[1], p1[0], p1[1]],
            });
            if is_arrow {
                annot["/LE"] = json!(["/None", "/OpenArrow"]);
            }
            (rect, annot)
        }
        AnnotationExtra::Note(content) => {
            let x0 = points[0][0].min(points[1][0]);
            let x1 = points[0][0].max(points[1][0]);
            let y0 = points[0][1].min(points[1][1]);
            let y1 = points[0][1].max(points[1][1]);
            let bw = x1 - x0;
            let bh = y1 - y0;
            let rect = [x0, y0, x1, y1];
            let px = |u: f64| x0 + bw * u;
            let py = |v: f64| y1 - bh * v;
            let border_w = (bw.min(bh) * 0.06).max(0.8);
            let fold = [r * 0.70, g * 0.70, b * 0.70];
            let border = [r * 0.38, g * 0.38, b * 0.38];
            appearance.push_str(&format!(
                "{r:.5} {g:.5} {b:.5} rg {:.5} {:.5} {:.5} RG {border_w:.3} w\n\
                 {:.3} {:.3} m {:.3} {:.3} l {:.3} {:.3} l {:.3} {:.3} l {:.3} {:.3} l h B\n\
                 {:.5} {:.5} {:.5} rg\n\
                 {:.3} {:.3} m {:.3} {:.3} l {:.3} {:.3} l h B\n\
                 {:.3} {:.3} m {:.3} {:.3} l S\n\
                 {:.3} {:.3} m {:.3} {:.3} l S\n\
                 {:.3} {:.3} m {:.3} {:.3} l S\n",
                border[0], border[1], border[2],
                px(0.04), py(0.04), px(0.96), py(0.04), px(0.96), py(0.74), px(0.74), py(0.96), px(0.04), py(0.96),
                fold[0], fold[1], fold[2],
                px(0.74), py(0.74), px(0.96), py(0.74), px(0.74), py(0.96),
                px(0.20), py(0.30), px(0.78), py(0.30),
                px(0.20), py(0.50), px(0.78), py(0.50),
                px(0.20), py(0.69), px(0.60), py(0.69),
            ));
            let annot = json!({
                "/Type": "/Annot",
                "/Subtype": subtype,
                "/Rect": rect,
                "/NM": format!("u:{}", a.name()),
                "/Contents": format!("u:{content}"),
                "/Name": "/Comment",
                "/Open": false,
                "/C": color_norm,
                "/CA": 1.0,
                "/F": 4,
                "/P": page_ref,
            });
            (rect, annot)
        }
        AnnotationExtra::Symbol(symbol) => {
            let x0 = points[0][0].min(points[1][0]);
            let x1 = points[0][0].max(points[1][0]);
            let y0 = points[0][1].min(points[1][1]);
            let y1 = points[0][1].max(points[1][1]);
            let bw = x1 - x0;
            let bh = y1 - y0;
            let rect = [x0, y0, x1, y1];
            let px = |u: f64| x0 + bw * u;
            let py = |v: f64| y1 - bh * v;
            let stroke_w = (bw.min(bh) * 0.17).max(1.5);
            match symbol {
                FillSymbol::Check => {
                    appearance.push_str(&format!(
                        "{stroke_w:.3} w {:.3} {:.3} m {:.3} {:.3} l {:.3} {:.3} l S\n",
                        px(0.16), py(0.54), px(0.40), py(0.80), px(0.85), py(0.22),
                    ));
                }
                FillSymbol::Cross => {
                    appearance.push_str(&format!(
                        "{stroke_w:.3} w {:.3} {:.3} m {:.3} {:.3} l S\n{:.3} {:.3} m {:.3} {:.3} l S\n",
                        px(0.20), py(0.20), px(0.80), py(0.80),
                        px(0.80), py(0.20), px(0.20), py(0.80),
                    ));
                }
                FillSymbol::Dot => {
                    let cx = px(0.50);
                    let cy = py(0.50);
                    let rad = bw.min(bh) * 0.34;
                    let k = rad * 0.5522847498;
                    appearance.push_str(&format!(
                        "{:.3} {:.3} m\n\
                         {:.3} {:.3} {:.3} {:.3} {:.3} {:.3} c\n\
                         {:.3} {:.3} {:.3} {:.3} {:.3} {:.3} c\n\
                         {:.3} {:.3} {:.3} {:.3} {:.3} {:.3} c\n\
                         {:.3} {:.3} {:.3} {:.3} {:.3} {:.3} c f\n",
                        cx + rad, cy,
                        cx + rad, cy + k, cx + k, cy + rad, cx, cy + rad,
                        cx - k, cy + rad, cx - rad, cy + k, cx - rad, cy,
                        cx - rad, cy - k, cx - k, cy - rad, cx, cy - rad,
                        cx + k, cy - rad, cx + rad, cy - k, cx + rad, cy,
                    ));
                }
            }
            let annot = json!({
                "/Type": "/Annot",
                "/Subtype": subtype,
                "/Rect": rect,
                "/NM": format!("u:{}", a.name()),
                "/C": color_norm,
                "/CA": 1.0,
                "/F": 4,
                "/P": page_ref,
            });
            (rect, annot)
        }
    };

    appearance.push_str("Q\n");
    let annot_ref = format!("{next} 0 R");
    *next += 1;
    let ap_ref = format!("{next} 0 R");
    *next += 1;
    annotation["/AP"] = json!({"/N": ap_ref});
    let file = work.join(format!("annotation-extra-{next}.txt"));
    std::fs::write(&file, appearance).map_err(|_| ExportError::Io)?;
    updates.insert(format!("obj:{annot_ref}"), json!({"value": annotation}));
    updates.insert(
        format!("obj:{ap_ref}"),
        json!({
            "stream": {
                "dict": {
                    "/Type": "/XObject",
                    "/Subtype": "/Form",
                    "/BBox": rect,
                    "/Resources": {}
                },
                "datafile": file.to_str().ok_or(ExportError::Io)?
            }
        }),
    );
    Ok(annot_ref)
}
