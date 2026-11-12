//! Recognize navigation-only outlines before explicitly discarding them.
//! This does not remap or preserve bookmarks. Only validated outline/action objects
//! may bypass the general action blocker; external/chained actions remain blocked.
use super::object;
use pdf_application::export::ExportError;
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};

const NODE_LIMIT: usize = 16384;
fn unsupported() -> ExportError {
    ExportError::Unsupported("bookmark dengan aksi atau struktur kompleks belum didukung.")
}
fn keys(dict: &Map<String, Value>, allowed: &[&str]) -> Result<(), ExportError> {
    if dict.keys().any(|k| !allowed.contains(&k.as_str())) {
        return Err(unsupported());
    }
    Ok(())
}
fn name(value: &Value) -> Result<&str, ExportError> {
    let s = value.as_str().ok_or(ExportError::Inspection)?;
    s.strip_prefix("u:")
        .or_else(|| s.strip_prefix("/"))
        .or_else(|| s.strip_prefix("b:"))
        .ok_or(ExportError::Inspection)
}
struct Navigation<'a> {
    objects: &'a Map<String, Value>,
    pages: &'a [String],
    destinations: HashMap<String, &'a Value>,
    checked: HashSet<String>,
    items: HashSet<String>,
    name_nodes: HashSet<String>,
}
impl<'a> Navigation<'a> {
    fn resolve(&self, value: &'a Value) -> Result<&'a Value, ExportError> {
        if value.is_string() {
            object(self.objects, value)
        } else {
            Ok(value)
        }
    }
    fn name_tree(&mut self, reference: &'a Value, depth: usize) -> Result<(), ExportError> {
        if depth > 64 || self.name_nodes.len() >= NODE_LIMIT {
            return Err(ExportError::ResourceLimit);
        }
        if let Some(id) = reference.as_str()
            && !self.name_nodes.insert(id.to_owned())
        {
            return Err(ExportError::Inspection);
        }
        let node = self
            .resolve(reference)?
            .as_object()
            .ok_or(ExportError::Inspection)?;
        keys(node, &["/Kids", "/Names", "/Limits"])?;
        if let Some(limits) = node.get("/Limits") {
            let limits = limits.as_array().ok_or(ExportError::Inspection)?;
            if limits.len() != 2 {
                return Err(ExportError::Inspection);
            }
            for limit in limits {
                name(limit)?;
            }
        }
        match (node.get("/Kids"), node.get("/Names")) {
            (Some(kids), None) => {
                for child in kids.as_array().ok_or(ExportError::Inspection)? {
                    self.name_tree(child, depth + 1)?;
                }
            }
            (None, Some(names)) => {
                let names = names.as_array().ok_or(ExportError::Inspection)?;
                if names.len() % 2 != 0 {
                    return Err(ExportError::Inspection);
                }
                for pair in names.as_chunks::<2>().0 {
                    if self.destinations.len() >= NODE_LIMIT {
                        return Err(ExportError::ResourceLimit);
                    }
                    if self
                        .destinations
                        .insert(name(&pair[0])?.to_owned(), &pair[1])
                        .is_some()
                    {
                        return Err(ExportError::Inspection);
                    }
                }
            }
            (None, None) => {}
            _ => return Err(ExportError::Inspection),
        }
        Ok(())
    }
    fn destination(&self, value: &'a Value, depth: usize) -> Result<(), ExportError> {
        if depth > 64 {
            return Err(ExportError::ResourceLimit);
        }
        if let Some(s) = value.as_str() {
            if self.objects.contains_key(&format!("obj:{s}")) {
                return self.destination(object(self.objects, value)?, depth + 1);
            }
            return self.destination(
                self.destinations
                    .get(name(value)?)
                    .ok_or(ExportError::Inspection)?,
                depth + 1,
            );
        }
        if let Some(dict) = value.as_object() {
            keys(dict, &["/D"])?;
            return self.destination(dict.get("/D").ok_or(ExportError::Inspection)?, depth + 1);
        }
        let array = value.as_array().ok_or(ExportError::Inspection)?;
        if !array
            .first()
            .and_then(Value::as_str)
            .is_some_and(|p| self.pages.iter().any(|id| id == p))
        {
            return Err(ExportError::Inspection);
        }
        let (arguments, nullable) = match array.get(1).and_then(Value::as_str) {
            Some("/Fit" | "/FitB") => (0, false),
            Some("/FitH" | "/FitV" | "/FitBH" | "/FitBV") => (1, true),
            Some("/XYZ") => (3, true),
            Some("/FitR") => (4, false),
            _ => return Err(unsupported()),
        };
        if array.len() != 2 + arguments
            || array[2..]
                .iter()
                .any(|v| !(nullable && v.is_null()) && !v.as_f64().is_some_and(f64::is_finite))
        {
            return Err(ExportError::Inspection);
        }
        Ok(())
    }
    fn action(&mut self, value: &'a Value) -> Result<(), ExportError> {
        let dict = self
            .resolve(value)?
            .as_object()
            .ok_or(ExportError::Inspection)?;
        keys(dict, &["/Type", "/S", "/D"])?;
        if dict.get("/S").and_then(Value::as_str) != Some("/GoTo")
            || dict
                .get("/Type")
                .is_some_and(|t| t.as_str() != Some("/Action"))
        {
            return Err(unsupported());
        }
        self.destination(dict.get("/D").ok_or(ExportError::Inspection)?, 0)?;
        if let Some(id) = value.as_str() {
            self.checked.insert(format!("obj:{id}"));
        }
        Ok(())
    }
    fn chain(
        &mut self,
        parent: &'a Value,
        dict: &'a Map<String, Value>,
        depth: usize,
    ) -> Result<(), ExportError> {
        if depth > 64 {
            return Err(ExportError::ResourceLimit);
        }
        if dict.get("/Count").is_some_and(|n| n.as_i64().is_none()) {
            return Err(ExportError::Inspection);
        }
        let (Some(first), Some(last)) = (dict.get("/First"), dict.get("/Last")) else {
            return if dict.contains_key("/First") || dict.contains_key("/Last") {
                Err(ExportError::Inspection)
            } else {
                Ok(())
            };
        };
        let mut current = first;
        let mut previous: Option<&Value> = None;
        loop {
            let id = current.as_str().ok_or(ExportError::Inspection)?;
            if self.items.len() >= NODE_LIMIT {
                return Err(ExportError::ResourceLimit);
            }
            if !self.items.insert(id.to_owned()) {
                return Err(ExportError::Inspection);
            }
            let node = object(self.objects, current)?
                .as_object()
                .ok_or(ExportError::Inspection)?;
            keys(
                node,
                &[
                    "/Title", "/Parent", "/Prev", "/Next", "/First", "/Last", "/Count", "/Dest",
                    "/A", "/C", "/F",
                ],
            )?;
            name(node.get("/Title").ok_or(ExportError::Inspection)?)?;
            if node.get("/Parent") != Some(parent) || node.get("/Prev") != previous {
                return Err(ExportError::Inspection);
            }
            if let Some(color) = node.get("/C") {
                let color = color.as_array().ok_or(ExportError::Inspection)?;
                if color.len() != 3
                    || color
                        .iter()
                        .any(|v| !v.as_f64().is_some_and(|n| (0.0..=1.0).contains(&n)))
                {
                    return Err(ExportError::Inspection);
                }
            }
            if node
                .get("/F")
                .is_some_and(|n| !n.as_u64().is_some_and(|n| n <= 3))
            {
                return Err(ExportError::Inspection);
            }
            match (node.get("/Dest"), node.get("/A")) {
                (Some(dest), None) => self.destination(dest, 0)?,
                (None, Some(action)) => self.action(action)?,
                (None, None) => {}
                _ => return Err(ExportError::Inspection),
            }
            self.chain(current, node, depth + 1)?;
            self.checked.insert(format!("obj:{id}"));
            if let Some(next) = node.get("/Next") {
                previous = Some(current);
                current = next;
            } else {
                if current != last {
                    return Err(ExportError::Inspection);
                }
                return Ok(());
            }
        }
    }
}
fn summary_ids(
    outlines: &[Value],
    ids: &mut HashSet<String>,
    depth: usize,
) -> Result<(), ExportError> {
    if depth > 64 || ids.len() > NODE_LIMIT {
        return Err(ExportError::ResourceLimit);
    }
    for outline in outlines {
        let id = outline
            .get("object")
            .and_then(Value::as_str)
            .ok_or(ExportError::Inspection)?;
        if !ids.insert(id.to_owned()) {
            return Err(ExportError::Inspection);
        }
        summary_ids(
            outline
                .get("kids")
                .and_then(Value::as_array)
                .ok_or(ExportError::Inspection)?,
            ids,
            depth + 1,
        )?;
    }
    Ok(())
}
pub(super) fn inspect(
    catalog: &Map<String, Value>,
    objects: &Map<String, Value>,
    pages: &[String],
    summary: &[Value],
) -> Result<(bool, HashSet<String>), ExportError> {
    let mut navigation = Navigation {
        objects,
        pages,
        destinations: HashMap::new(),
        checked: HashSet::new(),
        items: HashSet::new(),
        name_nodes: HashSet::new(),
    };
    if let Some(mode) = catalog.get("/PageMode")
        && !matches!(mode.as_str(), Some("/UseOutlines" | "/UseNone"))
    {
        return Err(unsupported());
    }
    if let Some(dests) = catalog.get("/Dests") {
        let dests = navigation
            .resolve(dests)?
            .as_object()
            .ok_or(ExportError::Inspection)?;
        if dests.len() > NODE_LIMIT {
            return Err(ExportError::ResourceLimit);
        }
        for (key, value) in dests {
            navigation
                .destinations
                .insert(name(&Value::String(key.clone()))?.to_owned(), value);
        }
    }
    if let Some(names) = catalog.get("/Names") {
        let names = navigation
            .resolve(names)?
            .as_object()
            .ok_or(ExportError::Inspection)?;
        keys(names, &["/Dests"])?;
        if let Some(dests) = names.get("/Dests") {
            navigation.name_tree(dests, 0)?;
        }
    }
    for value in navigation.destinations.values() {
        navigation.destination(value, 0)?;
    }
    if let Some(root) = catalog.get("/Outlines") {
        let dict = object(objects, root)?
            .as_object()
            .ok_or(ExportError::Inspection)?;
        keys(dict, &["/Type", "/First", "/Last", "/Count"])?;
        if dict
            .get("/Type")
            .is_some_and(|t| t.as_str() != Some("/Outlines"))
        {
            return Err(ExportError::Inspection);
        }
        navigation.chain(root, dict, 0)?;
        navigation.checked.insert(format!(
            "obj:{}",
            root.as_str().ok_or(ExportError::Inspection)?
        ));
    }
    let mut ids = HashSet::new();
    summary_ids(summary, &mut ids, 0)?;
    if ids != navigation.items {
        return Err(ExportError::Inspection);
    }
    let loss = catalog.contains_key("/Outlines")
        || catalog.contains_key("/Dests")
        || catalog.contains_key("/Names");
    Ok((loss, navigation.checked))
}
