//! Write passive, editable annotation objects and deterministic appearance streams.
use pdf_application::export::{ExportError, ExportPlan};
use pdf_domain::annotation::Annotation;
use serde_json::{Map, Value, json};
use std::path::Path;

pub(crate) fn owned(dict: &Map<String, Value>) -> Option<Annotation> {
    Annotation::from_name(dict.get("/NM")?.as_str()?.strip_prefix("u:")?)
}
pub(crate) fn validate(dict: &Map<String, Value>) -> Result<(), ExportError> {
    let a = owned(dict).ok_or(ExportError::Inspection)?;
    let keys = [
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
    if dict.keys().any(|k| !keys.contains(&k.as_str()))
        || dict.get("/Type").and_then(Value::as_str) != Some("/Annot")
        || dict.get("/Subtype").and_then(Value::as_str)
            != Some(if a.rectangles { "/Highlight" } else { "/Ink" })
    {
        return Err(ExportError::Unsupported(
            "anotasi diubah di luar subset pasif yang didukung.",
        ));
    }
    for key in ["/Rect", "/C", "/QuadPoints"] {
        if let Some(v) = dict.get(key)
            && !v.as_array().is_some_and(|v| {
                v.len() <= 16384 && v.iter().all(|n| n.as_f64().is_some_and(f64::is_finite))
            })
        {
            return Err(ExportError::Inspection);
        }
    }
    if dict.get("/Rect").and_then(Value::as_array).map(Vec::len) != Some(4)
        || !dict.get("/C").and_then(Value::as_array).is_some_and(|v| {
            v.len() == 3
                && v.iter().zip(a.color).all(|(v, c)| {
                    v.as_f64()
                        .is_some_and(|v| (v - f64::from(c) / 255.).abs() < 0.00001)
                })
        })
        || dict.get("/F").and_then(Value::as_u64) != Some(4)
        || dict.get("/CA").and_then(Value::as_f64) != Some(if a.highlight { 0.5 } else { 1. })
    {
        return Err(ExportError::Inspection);
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
fn inherited<'a>(
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
