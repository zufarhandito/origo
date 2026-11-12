//! Conservative structural preflight over QPDF inspection JSON v2.
use pdf_application::export::{ExportError, PreflightReport};
use serde_json::{Map, Value};
use std::collections::HashSet;
mod bookmarks;
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
    let outlines = json
        .get("outlines")
        .and_then(Value::as_array)
        .ok_or(ExportError::Inspection)?;
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
    const CATALOG_KEYS: &[&str] = &[
        "/Type",
        "/Pages",
        "/Metadata",
        "/Lang",
        "/Version",
        "/Outlines",
        "/PageMode",
        "/Names",
        "/Dests",
    ];
    if catalog.keys().any(|k| !CATALOG_KEYS.contains(&k.as_str())) {
        return Err(ExportError::Unsupported(
            "catalog memiliki form, tag, label halaman atau fitur kompleks yang belum didukung.",
        ));
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
            .zip(&tree_pages)
            .any(|(page, id)| page.get("object").and_then(Value::as_str) != Some(id.as_str()))
    {
        return Err(ExportError::Inspection);
    }
    let (bookmarks_loss, checked_bookmarks) =
        bookmarks::inspect(catalog, objects, &tree_pages, outlines)?;
    for (id, value) in objects {
        if !checked_bookmarks.contains(id) {
            forbidden(value)?;
        }
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
        bookmarks_loss,
        policy_version: pdf_application::export::EXPORT_POLICY_VERSION,
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
    fn with_bookmark() -> Value {
        let mut v = json();
        v["qpdf"][1]["obj:1 0 R"]["value"]["/Outlines"] = serde_json::json!("4 0 R");
        v["qpdf"][1]["obj:1 0 R"]["value"]["/PageMode"] = serde_json::json!("/UseOutlines");
        v["qpdf"][1]["obj:4 0 R"] = serde_json::json!({"value":{"/Type":"/Outlines","/First":"5 0 R","/Last":"5 0 R","/Count":1}});
        v["qpdf"][1]["obj:5 0 R"] = serde_json::json!({"value":{"/Title":"u:Part","/Parent":"4 0 R","/Dest":["3 0 R","/Fit"]}});
        v["outlines"] = serde_json::json!([{"object":"5 0 R","kids":[]}]);
        v
    }
    #[test]
    fn ordinary_bookmarks_and_local_actions_require_specific_consent() -> Result<(), ExportError> {
        let report = inspect_json(&with_bookmark(), 1)?;
        assert!(report.bookmarks_loss && report.requires_acknowledgement());
        assert!(!report.metadata_loss);
        let mut v = with_bookmark();
        v["qpdf"][1]["obj:5 0 R"]["value"]
            .as_object_mut()
            .ok_or(ExportError::Inspection)?
            .remove("/Dest");
        v["qpdf"][1]["obj:5 0 R"]["value"]["/A"] = serde_json::json!("6 0 R");
        v["qpdf"][1]["obj:6 0 R"] =
            serde_json::json!({"value":{"/S":"/GoTo","/D":["3 0 R","/XYZ",null,100,0]}});
        assert!(inspect_json(&v, 1)?.bookmarks_loss);
        v["qpdf"][1]["obj:6 0 R"]["value"]["/D"] = serde_json::json!("u:chapter");
        v["qpdf"][1]["obj:1 0 R"]["value"]["/Names"] = serde_json::json!({"/Dests":"7 0 R"});
        v["qpdf"][1]["obj:7 0 R"] = serde_json::json!({"value":{"/Kids":["8 0 R"]}});
        v["qpdf"][1]["obj:8 0 R"] = serde_json::json!({"value":{"/Limits":["u:chapter","u:chapter"],"/Names":["u:chapter",{"/D":["3 0 R","/Fit"]}]}});
        assert!(inspect_json(&v, 1)?.bookmarks_loss);
        v["qpdf"][1]["obj:1 0 R"]["value"]
            .as_object_mut()
            .ok_or(ExportError::Inspection)?
            .remove("/Names");
        v["qpdf"][1]["obj:1 0 R"]["value"]["/Dests"] =
            serde_json::json!({"/chapter":["3 0 R","/Fit"]});
        assert!(inspect_json(&v, 1)?.bookmarks_loss);
        // A nested item and a following sibling exercise Parent/Prev/Next/Last.
        let mut v = with_bookmark();
        v["qpdf"][1]["obj:5 0 R"]["value"]["/First"] = serde_json::json!("6 0 R");
        v["qpdf"][1]["obj:5 0 R"]["value"]["/Last"] = serde_json::json!("6 0 R");
        v["qpdf"][1]["obj:5 0 R"]["value"]["/Next"] = serde_json::json!("7 0 R");
        v["qpdf"][1]["obj:4 0 R"]["value"]["/Last"] = serde_json::json!("7 0 R");
        v["qpdf"][1]["obj:6 0 R"] = serde_json::json!({"value":{"/Title":"u:Nested","/Parent":"5 0 R","/Dest":["3 0 R","/Fit"]}});
        v["qpdf"][1]["obj:7 0 R"] = serde_json::json!({"value":{"/Title":"u:Sibling","/Parent":"4 0 R","/Prev":"5 0 R","/Dest":["3 0 R","/Fit"]}});
        v["outlines"] = serde_json::json!([{"object":"5 0 R","kids":[{"object":"6 0 R","kids":[]}]},{"object":"7 0 R","kids":[]}]);
        assert!(inspect_json(&v, 1)?.bookmarks_loss);
        Ok(())
    }
    #[test]
    fn complex_or_malformed_bookmarks_still_fail_closed() -> Result<(), ExportError> {
        for action in [
            serde_json::json!({"/S":"/JavaScript","/JS":"u:void 0"}),
            serde_json::json!({"/S":"/GoToR","/D":[0,"/Fit"],"/F":"u:file.pdf"}),
            serde_json::json!({"/S":"/URI","/URI":"u:https://example.com"}),
            serde_json::json!({"/S":"/GoTo","/D":["3 0 R","/Fit"],"/Next":{"/S":"/JavaScript","/JS":"u:void 0"}}),
            serde_json::json!({"/S":"/GoTo","/D":["99 0 R","/Fit"]}),
        ] {
            let mut v = with_bookmark();
            let node = v["qpdf"][1]["obj:5 0 R"]["value"]
                .as_object_mut()
                .ok_or(ExportError::Inspection)?;
            node.remove("/Dest");
            node.insert("/A".into(), action);
            assert!(inspect_json(&v, 1).is_err());
        }
        for (key, value) in [
            ("/Next", serde_json::json!("5 0 R")),
            ("/Parent", serde_json::json!("99 0 R")),
            ("/Dest", serde_json::json!("u:missing")),
            ("/FutureFeature", serde_json::json!(true)),
        ] {
            let mut v = with_bookmark();
            v["qpdf"][1]["obj:5 0 R"]["value"][key] = value;
            assert!(inspect_json(&v, 1).is_err());
        }
        let mut v = with_bookmark();
        v["qpdf"][1]["obj:1 0 R"]["value"]["/Names"] = serde_json::json!({"/EmbeddedFiles":{}});
        assert!(inspect_json(&v, 1).is_err());
        let mut v = with_bookmark();
        v["qpdf"][1]["obj:3 0 R"]["value"]["/Annots"] = serde_json::json!(["9 0 R"]);
        assert!(inspect_json(&v, 1).is_err());
        let mut v = with_bookmark();
        v["qpdf"][1]["obj:9 0 R"] =
            serde_json::json!({"value":{"/S":"/GoTo","/D":["3 0 R","/Fit"]}});
        assert!(
            inspect_json(&v, 1).is_err(),
            "Unvalidated actions must still be blocked"
        );
        let mut v = with_bookmark();
        v["outlines"] = serde_json::json!([]);
        assert!(inspect_json(&v, 1).is_err());
        Ok(())
    }
}
