//! Conservative structural preflight over QPDF inspection JSON v2.
use pdf_application::export::{ExportError, PreflightReport};
use serde_json::{Map, Value};
use std::collections::HashSet;
fn object<'a>(
    objects: &'a Map<String, Value>,
    reference: &Value,
) -> Result<&'a Value, ExportError> {
    let key = format!("obj:{}", reference.as_str().ok_or(ExportError::Inspection)?);
    objects
        .get(&key)
        .and_then(|v| v.get("value"))
        .ok_or(ExportError::Inspection)
}
fn forbidden(v: &Value) -> Result<(), ExportError> {
    match v {
        Value::Object(dict) => {
            for (key, value) in dict {
                if matches!(
                    key.as_str(),
                    "/ByteRange"
                        | "/XFA"
                        | "/JavaScript"
                        | "/JS"
                        | "/OpenAction"
                        | "/AA"
                        | "/EmbeddedFiles"
                        | "/EF"
                        | "/RichMedia"
                        | "/OC"
                        | "/OCProperties"
                        | "/StructTreeRoot"
                ) {
                    return Err(ExportError::Unsupported(
                        "signature, form, tag, aksi aktif, lampiran atau konten kompleks belum didukung.",
                    ));
                }
                if key == "/Annots" && !value.as_array().is_some_and(|a| a.is_empty()) {
                    return Err(ExportError::Unsupported(
                        "dokumen memiliki anotasi atau tautan halaman.",
                    ));
                }
                if (key == "/Type"
                    && matches!(value.as_str(), Some("/Sig" | "/Annot" | "/EmbeddedFile")))
                    || (key == "/FT" && value == "/Sig")
                {
                    return Err(ExportError::Unsupported(
                        "signature atau anotasi belum didukung.",
                    ));
                }
                if key == "/S"
                    && matches!(
                        value.as_str(),
                        Some(
                            "/JavaScript"
                                | "/Launch"
                                | "/URI"
                                | "/GoTo"
                                | "/GoToR"
                                | "/SubmitForm"
                                | "/ImportData"
                                | "/Rendition"
                                | "/Movie"
                                | "/Sound"
                        )
                    )
                {
                    return Err(ExportError::Unsupported(
                        "dokumen memiliki aksi atau tautan interaktif.",
                    ));
                }
                forbidden(value)?;
            }
        }
        Value::Array(array) => {
            for value in array {
                forbidden(value)?;
            }
        }
        _ => {}
    }
    Ok(())
}
fn walk_tree(
    objects: &Map<String, Value>,
    reference: &Value,
    seen: &mut HashSet<String>,
    pages: &mut Vec<String>,
    depth: usize,
) -> Result<usize, ExportError> {
    if depth > 64 {
        return Err(ExportError::ResourceLimit);
    }
    let id = reference
        .as_str()
        .ok_or(ExportError::Inspection)?
        .to_owned();
    if !seen.insert(id.clone()) {
        return Err(ExportError::Inspection);
    }
    let node = object(objects, reference)?
        .as_object()
        .ok_or(ExportError::Inspection)?;
    match node.get("/Type").and_then(Value::as_str) {
        Some("/Page") => {
            const KEYS: &[&str] = &[
                "/Type",
                "/Parent",
                "/Resources",
                "/MediaBox",
                "/CropBox",
                "/Rotate",
                "/Contents",
                "/Group",
                "/UserUnit",
                "/BleedBox",
                "/TrimBox",
                "/ArtBox",
                "/Annots",
            ];
            if node.keys().any(|k| !KEYS.contains(&k.as_str())) {
                return Err(ExportError::Unsupported(
                    "atribut halaman di luar kebijakan ekspor awal.",
                ));
            }
            pages.push(id);
            Ok(1)
        }
        Some("/Pages") => {
            const KEYS: &[&str] = &[
                "/Type",
                "/Parent",
                "/Resources",
                "/MediaBox",
                "/CropBox",
                "/Rotate",
                "/Count",
                "/Kids",
            ];
            if node.keys().any(|k| !KEYS.contains(&k.as_str())) {
                return Err(ExportError::Unsupported(
                    "struktur page tree kompleks belum didukung.",
                ));
            }
            let kids = node
                .get("/Kids")
                .and_then(Value::as_array)
                .ok_or(ExportError::Inspection)?;
            let mut count = 0;
            for kid in kids {
                count += walk_tree(objects, kid, seen, pages, depth + 1)?;
            }
            if node.get("/Count").and_then(Value::as_u64) != Some(count as u64) {
                return Err(ExportError::Inspection);
            }
            Ok(count)
        }
        _ => Err(ExportError::Inspection),
    }
}
pub(super) fn inspect_json(
    json: &Value,
    page_count: usize,
) -> Result<PreflightReport, ExportError> {
    if json.get("version").and_then(Value::as_u64) != Some(2)
        || json.pointer("/qpdf/0/jsonversion").and_then(Value::as_u64) != Some(2)
    {
        return Err(ExportError::Inspection);
    }
    match json.pointer("/encrypt/encrypted").and_then(Value::as_bool) {
        Some(false) => {}
        Some(true) => return Err(ExportError::Unsupported("PDF terenkripsi belum didukung.")),
        None => return Err(ExportError::Inspection),
    }
    match json
        .pointer("/acroform/hasacroform")
        .and_then(Value::as_bool)
    {
        Some(false) => {}
        Some(true) => return Err(ExportError::Unsupported("AcroForm/XFA belum didukung.")),
        None => return Err(ExportError::Inspection),
    }
    if !json
        .pointer("/acroform/fields")
        .and_then(Value::as_array)
        .is_some_and(|a| a.is_empty())
    {
        return Err(ExportError::Inspection);
    }
    if !json
        .get("outlines")
        .and_then(Value::as_array)
        .is_some_and(|a| a.is_empty())
    {
        return Err(ExportError::Unsupported(
            "bookmark/outlines belum didukung.",
        ));
    }
    let objects = json
        .pointer("/qpdf/1")
        .and_then(Value::as_object)
        .ok_or(ExportError::Inspection)?;
    let trailer = objects
        .get("trailer")
        .and_then(|v| v.get("value"))
        .and_then(Value::as_object)
        .ok_or(ExportError::Inspection)?;
    if trailer.contains_key("/Encrypt") {
        return Err(ExportError::Unsupported("PDF terenkripsi belum didukung."));
    }
    let catalog = object(
        objects,
        trailer.get("/Root").ok_or(ExportError::Inspection)?,
    )?
    .as_object()
    .ok_or(ExportError::Inspection)?;
    if catalog.get("/Type").and_then(Value::as_str) != Some("/Catalog") {
        return Err(ExportError::Inspection);
    }
    const CATALOG_KEYS: &[&str] = &["/Type", "/Pages", "/Metadata", "/Lang", "/Version"];
    if catalog.keys().any(|k| !CATALOG_KEYS.contains(&k.as_str())) {
        return Err(ExportError::Unsupported(
            "catalog memiliki bookmark, form, tag, label halaman atau fitur kompleks yang belum didukung.",
        ));
    }
    for value in objects.values() {
        forbidden(value)?;
    }
    let mut tree_pages = Vec::new();
    walk_tree(
        objects,
        catalog.get("/Pages").ok_or(ExportError::Inspection)?,
        &mut HashSet::new(),
        &mut tree_pages,
        0,
    )?;
    let pages = json
        .get("pages")
        .and_then(Value::as_array)
        .ok_or(ExportError::Inspection)?;
    if pages.len() != page_count
        || pages.len() != tree_pages.len()
        || pages
            .iter()
            .zip(tree_pages)
            .any(|(page, id)| page.get("object").and_then(Value::as_str) != Some(id.as_str()))
    {
        return Err(ExportError::Inspection);
    }
    if let Some(info) = trailer.get("/Info")
        && !object(objects, info)?.is_object()
    {
        return Err(ExportError::Inspection);
    }
    if let Some(metadata) = catalog.get("/Metadata") {
        let reference = metadata.as_str().ok_or(ExportError::Inspection)?;
        if !objects
            .get(&format!("obj:{reference}"))
            .and_then(|o| o.get("stream"))
            .and_then(|s| s.get("dict"))
            .is_some_and(|d| d.get("/Type").and_then(Value::as_str) == Some("/Metadata"))
        {
            return Err(ExportError::Inspection);
        }
    }
    Ok(PreflightReport {
        metadata_loss: trailer.contains_key("/Info") || catalog.contains_key("/Metadata"),
        policy_version: 1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn json() -> Value {
        serde_json::json!({"version":2,"encrypt":{"encrypted":false},"acroform":{"hasacroform":false,"fields":[]},"outlines":[],"pages":[{"object":"3 0 R"}],"qpdf":[{"jsonversion":2},{"trailer":{"value":{"/Root":"1 0 R"}},"obj:1 0 R":{"value":{"/Type":"/Catalog","/Pages":"2 0 R"}},"obj:2 0 R":{"value":{"/Type":"/Pages","/Count":1,"/Kids":["3 0 R"]}},"obj:3 0 R":{"value":{"/Type":"/Page","/Parent":"2 0 R"}}}]})
    }
    #[test]
    fn schema_cycles_counts_and_unsafe_objects_fail_closed() {
        assert!(inspect_json(&json(), 1).is_ok());
        let mut v = json();
        v["version"] = serde_json::json!(3);
        assert!(inspect_json(&v, 1).is_err());
        let mut v = json();
        v["acroform"] = Value::Null;
        assert!(inspect_json(&v, 1).is_err());
        let mut v = json();
        v["qpdf"][1]["obj:2 0 R"]["value"]["/Kids"] = serde_json::json!(["2 0 R"]);
        assert!(inspect_json(&v, 1).is_err());
        let mut v = json();
        v["qpdf"][1]["obj:2 0 R"]["value"]["/Count"] = serde_json::json!(2);
        assert!(inspect_json(&v, 1).is_err());
        let mut v = json();
        v["qpdf"][1]["obj:4 0 R"] = serde_json::json!({"value":{"/FT":"/Sig"}});
        assert!(inspect_json(&v, 1).is_err());
        let mut v = json();
        v["qpdf"][1]["trailer"]["value"]["/Info"] = serde_json::json!("4 0 R");
        assert!(inspect_json(&v, 1).is_err());
    }
}
