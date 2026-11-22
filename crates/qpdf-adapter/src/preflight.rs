//! Conservative structural preflight over QPDF inspection JSON v2.
use pdf_application::export::{ExportError, PreflightReport};
use pdf_domain::annotation::MAX_SIGNATURES;
use serde_json::{Map, Value};
use std::collections::HashSet;
mod bookmarks;
mod ink;
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
fn forbidden(
    v: &Value,
    source_copy: bool,
    remaining_signatures: &mut usize,
) -> Result<(), ExportError> {
    match v {
        Value::Object(dict) => {
            if crate::annotations::owned_name(dict).is_some() {
                crate::annotations::validate(dict, remaining_signatures)?;
                for (key, value) in dict {
                    if key != "/Type" {
                        forbidden(value, source_copy, remaining_signatures)?;
                    }
                }
                return Ok(());
            }
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
                ) {
                    return Err(ExportError::Unsupported(
                        "signature, form, tag, aksi aktif, lampiran atau konten kompleks belum didukung.",
                    ));
                }
                if key == "/StructTreeRoot" && !source_copy {
                    return Err(ExportError::Unsupported(
                        "struktur aksesibilitas belum didukung untuk penyusunan ulang PDF.",
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
                forbidden(value, source_copy, remaining_signatures)?;
            }
        }
        Value::Array(array) => {
            for value in array {
                forbidden(value, source_copy, remaining_signatures)?;
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
    source_copy: bool,
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
            if node.keys().any(|k| {
                !KEYS.contains(&k.as_str())
                    && !(source_copy && matches!(k.as_str(), "/StructParents" | "/Tabs"))
            }) {
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
                count += walk_tree(objects, kid, seen, pages, depth + 1, source_copy)?;
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
    inspect(json, page_count, false, false)
}
pub(super) fn inspect_source_copy(
    json: &Value,
    page_count: usize,
) -> Result<PreflightReport, ExportError> {
    inspect(json, page_count, true, false)
}
pub(super) fn inspect_extract(
    json: &Value,
    page_count: usize,
) -> Result<PreflightReport, ExportError> {
    inspect(json, page_count, false, true)
}
fn inspect(
    json: &Value,
    page_count: usize,
    source_copy: bool,
    tagged_extract: bool,
) -> Result<PreflightReport, ExportError> {
    let allow_structure = source_copy || tagged_extract;
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
        Some(true) if allow_structure => {}
        Some(true) => return Err(ExportError::Unsupported("AcroForm/XFA belum didukung.")),
        None => return Err(ExportError::Inspection),
    }
    if !json
        .pointer("/acroform/fields")
        .and_then(Value::as_array)
        .is_some_and(|a| a.is_empty())
    {
        return Err(ExportError::Unsupported(
            "PDF memiliki field form aktif; ekspor AcroForm/XFA belum didukung.",
        ));
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
    if allow_structure && let Some(form) = catalog.get("/AcroForm") {
        let form = if form.is_object() {
            form
        } else {
            object(objects, form)?
        };
        let dict = form.as_object().ok_or(ExportError::Inspection)?;
        let fields = dict.get("/Fields").ok_or(ExportError::Inspection)?;
        let fields = if fields.is_array() {
            fields
        } else {
            object(objects, fields)?
        };
        if dict.keys().any(|k| k != "/Fields") || !fields.as_array().is_some_and(|f| f.is_empty()) {
            return Err(ExportError::Unsupported(
                "hanya AcroForm kosong tanpa XFA yang dapat disalin utuh.",
            ));
        }
    }
    if allow_structure
        && json
            .pointer("/acroform/hasacroform")
            .and_then(Value::as_bool)
            == Some(true)
        && !catalog.contains_key("/AcroForm")
    {
        return Err(ExportError::Inspection);
    }
    // Report the actual unsupported feature instead of suggesting that every
    // unknown catalog key implies forms, tags and page labels simultaneously.
    for (key, reason) in [
        (
            "/AcroForm",
            "PDF memiliki form AcroForm/XFA (/AcroForm); ekspor form belum didukung.",
        ),
        (
            "/StructTreeRoot",
            "PDF memiliki tag/struktur aksesibilitas (/StructTreeRoot); penyimpanan struktur ini belum didukung.",
        ),
        (
            "/MarkInfo",
            "PDF memiliki informasi penandaan aksesibilitas (/MarkInfo); dukungan ekspornya perlu diperiksa.",
        ),
        (
            "/PageLabels",
            "PDF memiliki label penomoran halaman (/PageLabels); penyusunan ulang label belum didukung.",
        ),
        (
            "/Perms",
            "PDF memiliki izin atau sertifikasi dokumen (/Perms); ekspor dokumen ini belum didukung.",
        ),
        (
            "/OpenAction",
            "PDF memiliki aksi saat dokumen dibuka (/OpenAction); ekspor aksi ini belum didukung.",
        ),
        (
            "/AA",
            "PDF memiliki aksi dokumen (/AA); ekspor aksi ini belum didukung.",
        ),
        (
            "/OutputIntents",
            "PDF memiliki profil warna dokumen (/OutputIntents); pelestarian profil ini belum didukung.",
        ),
        (
            "/ViewerPreferences",
            "PDF memiliki pengaturan tampilan/cetak (/ViewerPreferences); dukungan ekspornya perlu diperiksa.",
        ),
        (
            "/PageLayout",
            "PDF memiliki pengaturan tata letak pembaca (/PageLayout); dukungan ekspornya perlu diperiksa.",
        ),
    ] {
        if catalog.contains_key(key) {
            if allow_structure
                && matches!(
                    key,
                    "/AcroForm" | "/StructTreeRoot" | "/MarkInfo" | "/ViewerPreferences"
                )
            {
                continue;
            }
            return Err(ExportError::Unsupported(reason));
        }
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
    if let Some(key) = catalog.keys().find(|k| {
        !CATALOG_KEYS.contains(&k.as_str())
            && !(allow_structure
                && matches!(
                    k.as_str(),
                    "/AcroForm" | "/StructTreeRoot" | "/MarkInfo" | "/ViewerPreferences"
                ))
    }) {
        // Only a short PDF attribute name reaches the UI, never QPDF diagnostics
        // or the object's value. Nothing is added to production metrics/logs.
        let attribute: String = key
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '_' | '-'))
            .take(48)
            .collect();
        return Err(ExportError::CatalogAttribute(attribute));
    }
    let mut tree_pages = Vec::new();
    walk_tree(
        objects,
        catalog.get("/Pages").ok_or(ExportError::Inspection)?,
        &mut HashSet::new(),
        &mut tree_pages,
        0,
        allow_structure,
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
    let mut owned_pages = HashSet::new();
    let mut remaining_signatures = MAX_SIGNATURES;
    for page in &tree_pages {
        let dict = object(objects, &Value::String(page.clone()))?
            .as_object()
            .ok_or(ExportError::Inspection)?;
        if let Some(list) = dict.get("/Annots") {
            let inline_list = list.is_array();
            let list = if inline_list {
                list
            } else {
                object(objects, list)?
            };
            let list = list.as_array().ok_or(ExportError::Inspection)?;
            if list.iter().all(|r| {
                let v = if r.is_object() {
                    Ok(r)
                } else {
                    object(objects, r)
                };
                v.ok()
                    .and_then(Value::as_object)
                    .is_some_and(|d| crate::annotations::owned_name(d).is_some())
            }) {
                // Indirect annotations are checked in the object walk below.
                // Inline dictionaries must be checked before removing /Annots.
                if inline_list {
                    for v in list.iter().filter(|v| v.is_object()) {
                        forbidden(v, allow_structure, &mut remaining_signatures)?;
                    }
                }
                owned_pages.insert(page.clone());
            }
        }
    }
    let checked_ink = if allow_structure {
        let other_pages: Vec<_> = tree_pages
            .iter()
            .filter(|p| !owned_pages.contains(*p))
            .cloned()
            .collect();
        ink::inspect(objects, &other_pages)?
    } else {
        HashSet::new()
    };
    for (id, value) in objects {
        if checked_bookmarks.contains(id) || checked_ink.contains(id) {
            continue;
        }
        if tree_pages
            .iter()
            .any(|p| id == &format!("obj:{p}") && (allow_structure || owned_pages.contains(p)))
        {
            let mut page = value
                .get("value")
                .and_then(Value::as_object)
                .ok_or(ExportError::Inspection)?
                .clone();
            page.remove("/Annots");
            forbidden(
                &Value::Object(page),
                allow_structure,
                &mut remaining_signatures,
            )?;
        } else {
            forbidden(value, allow_structure, &mut remaining_signatures)?;
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
        metadata_loss: !source_copy
            && (trailer.contains_key("/Info")
                || catalog.contains_key("/Metadata")
                || (tagged_extract && catalog.contains_key("/ViewerPreferences"))),
        bookmarks_loss: !source_copy && bookmarks_loss,
        tags_loss: !source_copy
            && tagged_extract
            && (catalog.contains_key("/StructTreeRoot") || catalog.contains_key("/MarkInfo")),
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
    fn annotation_marker_never_bypasses_inline_or_indirect_active_content() {
        let a = pdf_domain::annotation::Annotation {
            id: 1,
            highlight: false,
            color: [0, 0, 0],
            width: 2.,
            points: vec![[0.1, 0.2], [0.3, 0.4]],
            rectangles: false,
            signature: None,
        };
        let annotation = serde_json::json!({"/Type":"/Annot","/Subtype":"/Ink","/NM":format!("u:{}",a.name()),"/Rect":[0,0,100,100],"/C":[0,0,0],"/CA":1,"/F":4,"/InkList":[[10,20,30,40]]});
        for indirect in [false, true] {
            let mut v = json();
            if indirect {
                v["qpdf"][1]["obj:4 0 R"] = serde_json::json!({"value":annotation});
                v["qpdf"][1]["obj:3 0 R"]["value"]["/Annots"] = serde_json::json!(["4 0 R"]);
            } else {
                v["qpdf"][1]["obj:3 0 R"]["value"]["/Annots"] = serde_json::json!([annotation]);
            }
            assert!(inspect_json(&v, 1).is_ok());
            let node = if indirect {
                &mut v["qpdf"][1]["obj:4 0 R"]["value"]
            } else {
                &mut v["qpdf"][1]["obj:3 0 R"]["value"]["/Annots"][0]
            };
            node["/A"] = serde_json::json!({"/S":"/JavaScript","/JS":"u:unsafe"});
            assert!(inspect_json(&v, 1).is_err());
            assert!(inspect_extract(&v, 1).is_err());
            assert!(inspect_source_copy(&v, 1).is_err());
        }
    }
    #[test]
    fn signature_stamp_rejects_active_keys_and_forged_subtype() {
        let a = pdf_domain::annotation::Annotation {
            id: 1,
            highlight: false,
            color: [0, 0, 0],
            width: 1.,
            points: vec![[0.1, 0.2], [0.3, 0.4]],
            rectangles: false,
            signature: Some(std::sync::Arc::new(
                pdf_domain::annotation::SignatureImage {
                    width: 1,
                    height: 1,
                    rgba: vec![0, 0, 0, 255],
                },
            )),
        };
        let annotation = serde_json::json!({"/Type":"/Annot","/Subtype":"/Stamp","/NM":format!("u:{}",a.name()),"/Rect":[0,0,100,100],"/C":[0,0,0],"/CA":1,"/F":4});
        let mut v = json();
        v["qpdf"][1]["obj:3 0 R"]["value"]["/Annots"] = serde_json::json!([annotation]);
        assert!(inspect_json(&v, 1).is_ok());
        let mut many = json();
        many["qpdf"][1]["obj:3 0 R"]["value"]["/Annots"] = serde_json::json!("4 0 R");
        many["qpdf"][1]["obj:4 0 R"] =
            serde_json::json!({"value":vec![annotation.clone(); MAX_SIGNATURES]});
        assert!(inspect_json(&many, 1).is_ok());
        many["qpdf"][1]["obj:5 0 R"] = serde_json::json!({"value":annotation});
        assert!(matches!(
            inspect_json(&many, 1),
            Err(ExportError::ResourceLimit)
        ));
        let mut active = v.clone();
        active["qpdf"][1]["obj:3 0 R"]["value"]["/Annots"][0]["/A"] =
            serde_json::json!({"/S":"/URI","/URI":"u:example"});
        assert!(inspect_json(&active, 1).is_err());
        assert!(inspect_source_copy(&active, 1).is_err());
        v["qpdf"][1]["obj:3 0 R"]["value"]["/Annots"][0]["/Subtype"] = serde_json::json!("/Widget");
        assert!(inspect_json(&v, 1).is_err());
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
    #[test]
    fn whole_source_copy_preserves_tags_but_does_not_allow_active_forms_or_actions()
    -> Result<(), ExportError> {
        let mut v = json();
        v["acroform"]["hasacroform"] = serde_json::json!(true);
        v["qpdf"][1]["obj:1 0 R"]["value"]["/AcroForm"] = serde_json::json!("4 0 R");
        v["qpdf"][1]["obj:4 0 R"] = serde_json::json!({"value":{"/Fields":[]}});
        v["qpdf"][1]["obj:1 0 R"]["value"]["/StructTreeRoot"] = serde_json::json!("5 0 R");
        v["qpdf"][1]["obj:5 0 R"] =
            serde_json::json!({"value":{"/Type":"/StructTreeRoot","/K":[]}});
        v["qpdf"][1]["obj:1 0 R"]["value"]["/MarkInfo"] = serde_json::json!({"/Marked":true});
        v["qpdf"][1]["obj:1 0 R"]["value"]["/ViewerPreferences"] =
            serde_json::json!({"/DisplayDocTitle":true});
        v["qpdf"][1]["obj:3 0 R"]["value"]["/StructParents"] = serde_json::json!(0);
        v["qpdf"][1]["obj:3 0 R"]["value"]["/Tabs"] = serde_json::json!("/S");
        assert!(!inspect_source_copy(&v, 1)?.requires_acknowledgement());
        assert!(
            inspect_json(&v, 1).is_err(),
            "Transforms still require strict policy"
        );
        for (key, value) in [
            ("/Fields", serde_json::json!(["6 0 R"])),
            ("/XFA", serde_json::json!("6 0 R")),
            ("/SigFlags", serde_json::json!(3)),
        ] {
            let mut bad = v.clone();
            bad["qpdf"][1]["obj:4 0 R"]["value"][key] = value;
            assert!(inspect_source_copy(&bad, 1).is_err());
        }
        for (key, value) in [
            (
                "/OpenAction",
                serde_json::json!({"/S":"/JavaScript","/JS":"u:void 0"}),
            ),
            ("/Perms", serde_json::json!({})),
        ] {
            let mut bad = v.clone();
            bad["qpdf"][1]["obj:1 0 R"]["value"][key] = value;
            assert!(inspect_source_copy(&bad, 1).is_err());
        }
        let mut bad = v.clone();
        bad["encrypt"]["encrypted"] = serde_json::json!(true);
        assert!(inspect_source_copy(&bad, 1).is_err());
        let mut bad = v;
        bad["acroform"]["fields"] = serde_json::json!([{}]);
        assert!(inspect_source_copy(&bad, 1).is_err());
        Ok(())
    }
    #[test]
    fn catalog_rejections_identify_the_actual_attribute() {
        for (attribute, detail) in [
            ("/StructTreeRoot", "aksesibilitas (/StructTreeRoot)"),
            ("/MarkInfo", "(/MarkInfo)"),
            ("/PageLabels", "(/PageLabels)"),
            ("/ViewerPreferences", "(/ViewerPreferences)"),
            ("/PageLayout", "(/PageLayout)"),
            ("/OutputIntents", "(/OutputIntents)"),
        ] {
            let mut v = json();
            v["qpdf"][1]["obj:1 0 R"]["value"][attribute] = serde_json::json!({});
            let error = inspect_json(&v, 1)
                .err()
                .map(|e| e.to_string())
                .unwrap_or_default();
            assert!(error.contains(detail), "Wrong catalog diagnostic: {error}");
        }
        let mut v = json();
        v["qpdf"][1]["obj:1 0 R"]["value"]["/FutureFeature"] = serde_json::json!(true);
        assert!(
            matches!(inspect_json(&v, 1), Err(ExportError::CatalogAttribute(key)) if key == "/FutureFeature")
        );
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
