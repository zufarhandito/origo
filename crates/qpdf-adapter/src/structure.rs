//! Drop accessibility references only after the application has obtained consent.
//! Patches update the same staged PDF whose object numbers were inspected.
use pdf_application::export::ExportError;
use serde_json::{Map, Value, json};

pub(super) fn patch(document: &Value) -> Result<Option<Value>, ExportError> {
    if document
        .pointer("/qpdf/0/jsonversion")
        .and_then(Value::as_u64)
        != Some(2)
    {
        return Err(ExportError::Inspection);
    }
    let objects = document
        .pointer("/qpdf/1")
        .and_then(Value::as_object)
        .ok_or(ExportError::Inspection)?;
    let mut updates = Map::new();
    for (id, original) in objects {
        let mut changed = original.clone();
        let dict = if original.get("stream").is_some() {
            changed
                .pointer_mut("/stream/dict")
                .and_then(Value::as_object_mut)
        } else {
            changed.get_mut("value").and_then(Value::as_object_mut)
        };
        let Some(dict) = dict else {
            continue;
        };
        let mut dirty = dict.remove("/StructParent").is_some();
        dirty |= dict.remove("/StructParents").is_some();
        if dict.get("/Type").and_then(Value::as_str) == Some("/Catalog") {
            dirty |= dict.remove("/StructTreeRoot").is_some();
            dirty |= dict.remove("/MarkInfo").is_some();
        }
        if dict.get("/Type").and_then(Value::as_str) == Some("/Page")
            && dict.get("/Tabs").and_then(Value::as_str) == Some("/S")
        {
            dict.remove("/Tabs");
            dirty = true;
        }
        if dirty {
            updates.insert(id.clone(), changed);
        }
    }
    Ok((!updates.is_empty()).then(|| json!({"qpdf":[document["qpdf"][0].clone(), updates]})))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strips_only_structure_fields_and_keeps_streams_and_ink() -> Result<(), ExportError> {
        let doc = json!({"qpdf":[{"jsonversion":2},{
            "obj:1 0 R":{"value":{"/Type":"/Catalog","/StructTreeRoot":"4 0 R","/MarkInfo":{},"/Pages":"2 0 R"}},
            "obj:2 0 R":{"value":{"/Type":"/Page","/StructParents":0,"/Tabs":"/S","/Annots":["3 0 R"],"/Contents":"5 0 R"}},
            "obj:3 0 R":{"value":{"/Type":"/Annot","/Subtype":"/Ink","/InkList":[[1,2,3,4]],"/StructParent":1}},
            "obj:5 0 R":{"stream":{"dict":{"/Subtype":"/Form","/StructParents":2,"/Length":20}}}
        }]});
        let update = patch(&doc)?.ok_or(ExportError::Validation)?;
        let objects = &update["qpdf"][1];
        assert!(
            objects["obj:1 0 R"]["value"]
                .get("/StructTreeRoot")
                .is_none()
        );
        assert_eq!(objects["obj:2 0 R"]["value"]["/Annots"], json!(["3 0 R"]));
        assert_eq!(
            objects["obj:3 0 R"]["value"]["/InkList"],
            json!([[1, 2, 3, 4]])
        );
        assert_eq!(objects["obj:5 0 R"]["stream"]["dict"]["/Length"], 20);
        assert!(patch(&update)?.is_none());
        Ok(())
    }
}
