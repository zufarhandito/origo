//! Recognize passive Ink annotations; other annotation types/actions stay blocked.
use super::object;
use pdf_application::export::ExportError;
use serde_json::{Map, Value};
use std::collections::HashSet;

fn bad() -> ExportError {
    ExportError::Unsupported(
        "hanya anotasi tinta pasif tanpa aksi/appearance tambahan yang didukung untuk ekstraksi ini.",
    )
}
fn numbers(v: &Value) -> bool {
    v.as_array()
        .is_some_and(|a| a.iter().all(|n| n.as_f64().is_some_and(f64::is_finite)))
}
fn validate(dict: &Map<String, Value>, page: &str) -> Result<(), ExportError> {
    const KEYS: &[&str] = &[
        "/Type",
        "/Subtype",
        "/Rect",
        "/InkList",
        "/Border",
        "/C",
        "/CA",
        "/F",
        "/Contents",
        "/CreationDate",
        "/M",
        "/NM",
        "/T",
        "/P",
        "/StructParent",
    ];
    if dict.keys().any(|k| !KEYS.contains(&k.as_str()))
        || dict.get("/Subtype").and_then(Value::as_str) != Some("/Ink")
        || dict
            .get("/Type")
            .is_some_and(|t| t.as_str() != Some("/Annot"))
        || dict.get("/P").is_some_and(|p| p.as_str() != Some(page))
    {
        return Err(bad());
    }
    let rect = dict.get("/Rect").ok_or_else(bad)?;
    if !numbers(rect) || rect.as_array().map(Vec::len) != Some(4) {
        return Err(bad());
    }
    let ink = dict
        .get("/InkList")
        .and_then(Value::as_array)
        .ok_or_else(bad)?;
    if ink.is_empty()
        || ink.iter().any(|line| {
            !numbers(line)
                || !line
                    .as_array()
                    .is_some_and(|p| p.len() >= 4 && p.len() % 2 == 0)
        })
    {
        return Err(bad());
    }
    for key in ["/Contents", "/CreationDate", "/M", "/NM", "/T"] {
        if dict.get(key).is_some_and(|v| {
            !v.as_str()
                .is_some_and(|s| s.starts_with("u:") || s.starts_with("b:"))
        }) {
            return Err(bad());
        }
    }
    if dict
        .get("/CA")
        .is_some_and(|v| !v.as_f64().is_some_and(|n| (0.0..=1.0).contains(&n)))
        || dict
            .get("/F")
            .is_some_and(|v| !v.as_u64().is_some_and(|n| n <= 1023))
        || dict
            .get("/StructParent")
            .is_some_and(|v| v.as_u64().is_none())
    {
        return Err(bad());
    }
    if let Some(color) = dict.get("/C") {
        let values = color.as_array().ok_or_else(bad)?;
        if !matches!(values.len(), 0 | 1 | 3 | 4)
            || values
                .iter()
                .any(|v| !v.as_f64().is_some_and(|n| (0.0..=1.0).contains(&n)))
        {
            return Err(bad());
        }
    }
    if let Some(border) = dict.get("/Border")
        && (!numbers(border) || border.as_array().map(Vec::len) != Some(3))
    {
        return Err(bad());
    }
    Ok(())
}
/// The returned objects have been fully validated and can bypass generic Annot
/// rejection. Page dictionaries are scanned separately with only Annots omitted.
pub(super) fn inspect(
    objects: &Map<String, Value>,
    pages: &[String],
) -> Result<HashSet<String>, ExportError> {
    let mut checked = HashSet::new();
    for page in pages {
        let page_ref = Value::String(page.clone());
        let dict = object(objects, &page_ref)?
            .as_object()
            .ok_or(ExportError::Inspection)?;
        let Some(reference) = dict.get("/Annots") else {
            continue;
        };
        let annotations = if reference.is_array() {
            reference
        } else {
            checked.insert(format!(
                "obj:{}",
                reference.as_str().ok_or(ExportError::Inspection)?
            ));
            object(objects, reference)?
        };
        for reference in annotations.as_array().ok_or(ExportError::Inspection)? {
            let annotation = if reference.is_object() {
                reference
            } else {
                checked.insert(format!(
                    "obj:{}",
                    reference.as_str().ok_or(ExportError::Inspection)?
                ));
                object(objects, reference)?
            };
            validate(annotation.as_object().ok_or(ExportError::Inspection)?, page)?;
        }
    }
    Ok(checked)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn only_passive_well_formed_ink_is_accepted() -> Result<(), ExportError> {
        let objects = json!({"obj:1 0 R":{"value":{"/Type":"/Page","/Annots":"2 0 R"}},"obj:2 0 R":{"value":["3 0 R"]},"obj:3 0 R":{"value":{"/Type":"/Annot","/Subtype":"/Ink","/P":"1 0 R","/Rect":[0,0,20,20],"/InkList":[[1,1,10,10]],"/Contents":"u:Test"}}});
        let map = objects.as_object().ok_or(ExportError::Inspection)?;
        assert_eq!(inspect(map, &["1 0 R".into()])?.len(), 2);
        for (key, value) in [
            ("/Subtype", json!("/Widget")),
            ("/A", json!({"/S":"/JavaScript"})),
            ("/AP", json!({})),
            ("/InkList", json!([])),
            ("/P", json!("9 0 R")),
            ("/Rect", json!([0, 0])),
            ("/Contents", json!({})),
        ] {
            let mut bad = objects.clone();
            bad["obj:3 0 R"]["value"][key] = value;
            assert!(
                inspect(
                    bad.as_object().ok_or(ExportError::Inspection)?,
                    &["1 0 R".into()]
                )
                .is_err()
            );
        }
        Ok(())
    }
}
