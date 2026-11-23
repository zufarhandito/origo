//! Add invisible, Unicode-mapped OCR text while preserving the page's original streams.
use pdf_application::export::{ExportControl, ExportError, ExportPlan, resolve_source};
use pdf_ocr::{FrozenResults, Recognition};
use serde_json::{Map, Value, json};
use std::{collections::HashMap, fmt::Write as _, path::Path};

pub(super) fn patch(
    plan: &ExportPlan,
    doc: &Value,
    results: &FrozenResults,
    font: &[u8],
    work: &Path,
    control: &ExportControl,
) -> Result<Value, ExportError> {
    let objects = doc
        .pointer("/qpdf/1")
        .and_then(Value::as_object)
        .ok_or(ExportError::Inspection)?;
    let pages = doc
        .get("pages")
        .and_then(Value::as_array)
        .ok_or(ExportError::Inspection)?;
    if pages.len() != plan.pages().len() || pages.len() != results.pages() {
        return Err(ExportError::Validation);
    }
    let mut writer = Objects {
        work,
        next: objects
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
            .checked_add(1)
            .ok_or(ExportError::Inspection)?,
        patch: Map::new(),
    };
    let font_id = writer.allocate()?;
    let mut charset = Charset::default();
    for (index, output_page) in pages.iter().enumerate() {
        control.checkpoint()?;
        let Some(recognition) = results.get(index).map_err(|_| ExportError::Io)? else {
            continue;
        };
        let reference = output_page
            .get("object")
            .and_then(Value::as_str)
            .ok_or(ExportError::Inspection)?;
        let key = format!("obj:{reference}");
        let original = objects
            .get(&key)
            .and_then(|o| o.get("value"))
            .ok_or(ExportError::Inspection)?;
        let mut page = original
            .as_object()
            .cloned()
            .ok_or(ExportError::Inspection)?;
        let mut resources = inherited_dict(objects, original, "/Resources")?;
        let mut fonts = dictionary(objects, resources.get("/Font"))?;
        let font_name = (0..=fonts.len())
            .map(|i| format!("/OrigoOcr{i}"))
            .find(|key| !fonts.contains_key(key))
            .ok_or(ExportError::Inspection)?;
        fonts.insert(font_name.clone(), json!(font_id));
        resources.insert("/Font".into(), Value::Object(fonts));
        page.insert("/Resources".into(), Value::Object(resources));

        let entry = plan.pages()[index].entry;
        let source = resolve_source(plan.sources(), entry.source_id)?;
        let intrinsic = source
            .geometries
            .get(usize::from(entry.source_page_index))
            .ok_or(ExportError::InvalidPlan)?
            .quarter_turns;
        let bounds = visible_bounds(objects, original)?;
        let bytes = commands(&recognition, intrinsic, bounds, &font_name, &mut charset)?;
        let stream = writer.stream(Map::new(), bytes.as_bytes())?;
        let mut contents = output_page
            .get("contents")
            .and_then(Value::as_array)
            .cloned()
            .ok_or(ExportError::Inspection)?;
        if contents.iter().any(|r| !r.is_string()) {
            return Err(ExportError::Inspection);
        }
        // Put the layer first so existing content cannot leak its clipping or
        // transformation state into our text. The layer paints no graphics.
        contents.insert(0, json!(stream));
        page.insert("/Contents".into(), Value::Array(contents));
        writer.patch.insert(key, json!({"value":page}));
    }
    write_font(&mut writer, &font_id, charset, font)?;
    Ok(json!({"qpdf":[{"jsonversion":2}, writer.patch]}))
}

fn resolve<'a>(objects: &'a Map<String, Value>, value: &'a Value) -> Option<&'a Value> {
    let mut value = value;
    for _ in 0..64 {
        let Some(reference) = value.as_str() else {
            return Some(value);
        };
        value = objects.get(&format!("obj:{reference}"))?.get("value")?;
    }
    None
}

fn dictionary(
    objects: &Map<String, Value>,
    value: Option<&Value>,
) -> Result<Map<String, Value>, ExportError> {
    value.map_or_else(
        || Ok(Map::new()),
        |value| {
            resolve(objects, value)
                .and_then(Value::as_object)
                .cloned()
                .ok_or(ExportError::Inspection)
        },
    )
}

fn inherited_dict(
    objects: &Map<String, Value>,
    page: &Value,
    key: &str,
) -> Result<Map<String, Value>, ExportError> {
    dictionary(objects, crate::annotations::inherited(objects, page, key))
}

fn visible_bounds(objects: &Map<String, Value>, page: &Value) -> Result<[f64; 4], ExportError> {
    let read = |key| -> Result<Option<[f64; 4]>, ExportError> {
        crate::annotations::inherited(objects, page, key)
            .map(|v| {
                let bounds = resolve(objects, v)
                    .and_then(Value::as_array)
                    .ok_or(ExportError::Inspection)?;
                let [a, b, c, d] = bounds.as_slice() else {
                    return Err(ExportError::Inspection);
                };
                let [a, b, c, d] = [a, b, c, d].map(|n| n.as_f64().filter(|v| v.is_finite()));
                if let [Some(a), Some(b), Some(c), Some(d)] = [a, b, c, d] {
                    Ok([a.min(c), b.min(d), a.max(c), b.max(d)])
                } else {
                    Err(ExportError::Inspection)
                }
            })
            .transpose()
    };
    let mut media = read("/MediaBox")?.ok_or(ExportError::Inspection)?;
    if let Some(crop) = read("/CropBox")? {
        media = [
            media[0].max(crop[0]),
            media[1].max(crop[1]),
            media[2].min(crop[2]),
            media[3].min(crop[3]),
        ];
    }
    if media[0] >= media[2] || media[1] >= media[3] {
        return Err(ExportError::Inspection);
    }
    Ok(media)
}

#[derive(Default)]
struct Charset {
    ids: HashMap<char, u16>,
    chars: Vec<char>,
}

impl Charset {
    fn encode(&mut self, ch: char) -> Result<u16, ExportError> {
        if let Some(cid) = self.ids.get(&ch) {
            return Ok(*cid);
        }
        let cid = u16::try_from(self.chars.len() + 1).map_err(|_| ExportError::ResourceLimit)?;
        self.chars.push(ch);
        self.ids.insert(ch, cid);
        Ok(cid)
    }
}

fn commands(
    recognition: &Recognition,
    intrinsic: u8,
    [x0, y0, x1, y1]: [f64; 4],
    font: &str,
    charset: &mut Charset,
) -> Result<String, ExportError> {
    let inverse = (4 - (intrinsic + recognition.rotation) % 4) % 4;
    let to_point = |p| {
        let [x, y] = pdf_domain::annotation::rotate(p, inverse);
        [x0 + (x1 - x0) * f64::from(x), y1 - (y1 - y0) * f64::from(y)]
    };
    let mut out = format!("q\nBT\n{font} 1 Tf\n3 Tr\n");
    for word in &recognition.page.words {
        let [l, t, r, b] = word.rect;
        let [x, y] = to_point([l, b]);
        let [right_x, right_y] = to_point([r, b]);
        let [top_x, top_y] = to_point([l, t]);
        let chars = word.text.chars().count() as f64;
        let [a, b, c, d] = [
            (right_x - x) / chars,
            (right_y - y) / chars,
            top_x - x,
            top_y - y,
        ];
        write!(out, "{a:.6} {b:.6} {c:.6} {d:.6} {x:.6} {y:.6} Tm <")
            .map_err(|_| ExportError::Engine)?;
        for ch in word.text.chars() {
            write!(out, "{:04X}", charset.encode(ch)?).map_err(|_| ExportError::Engine)?;
        }
        out.push_str("> Tj\n");
    }
    out.push_str("ET\nQ\n");
    Ok(out)
}

fn write_font(
    writer: &mut Objects<'_>,
    id: &str,
    charset: Charset,
    font: &[u8],
) -> Result<(), ExportError> {
    let mut cmap = String::from(
        "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n/CMapName /OrigoOCRUnicode def\n/CMapType 2 def\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
    );
    let mut code = 1;
    for chunk in charset.chars.chunks(100) {
        writeln!(cmap, "{} beginbfchar", chunk.len()).map_err(|_| ExportError::Engine)?;
        for ch in chunk {
            write!(cmap, "<{code:04X}> <").map_err(|_| ExportError::Engine)?;
            for unit in ch.encode_utf16(&mut [0; 2]) {
                write!(cmap, "{unit:04X}").map_err(|_| ExportError::Engine)?;
            }
            cmap.push_str(">\n");
            code += 1;
        }
        cmap.push_str("endbfchar\n");
    }
    cmap.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
    let cmap = writer.stream(Map::new(), cmap.as_bytes())?;
    let map = writer.stream(Map::new(), &[0, 1].repeat(charset.chars.len() + 1))?;
    let length = Map::from_iter([("/Length1".into(), json!(font.len()))]);
    let font_file = writer.stream(length, font)?;
    let descriptor = writer.value(json!({
        "/Type":"/FontDescriptor", "/FontName":"/GlyphLessFont", "/Flags":5,
        "/FontBBox":[0,0,1000,1000], "/ItalicAngle":0, "/Ascent":1000,
        "/CapHeight":1000, "/Descent":-1, "/StemV":80, "/FontFile2":font_file
    }))?;
    let cid = writer.value(json!({
        "/Type":"/Font", "/Subtype":"/CIDFontType2", "/BaseFont":"/GlyphLessFont",
        "/CIDSystemInfo":{"/Registry":"u:Adobe","/Ordering":"u:Identity","/Supplement":0},
        "/DW":1000, "/CIDToGIDMap": map, "/FontDescriptor":descriptor
    }))?;
    writer.patch.insert(
        format!("obj:{id}"),
        json!({"value":{
            "/Type":"/Font", "/Subtype":"/Type0", "/BaseFont":"/GlyphLessFont",
            "/Encoding":"/Identity-H", "/DescendantFonts":[cid], "/ToUnicode":cmap
        }}),
    );
    Ok(())
}

struct Objects<'a> {
    work: &'a Path,
    next: u64,
    patch: Map<String, Value>,
}

impl Objects<'_> {
    fn allocate(&mut self) -> Result<String, ExportError> {
        let id = format!("{} 0 R", self.next);
        self.next = self.next.checked_add(1).ok_or(ExportError::Inspection)?;
        Ok(id)
    }
    fn value(&mut self, value: Value) -> Result<String, ExportError> {
        let id = self.allocate()?;
        self.patch
            .insert(format!("obj:{id}"), json!({"value":value}));
        Ok(id)
    }
    fn stream(&mut self, dict: Map<String, Value>, bytes: &[u8]) -> Result<String, ExportError> {
        let file = self.work.join(format!("ocr-stream-{}", self.next));
        std::fs::write(&file, bytes).map_err(|_| ExportError::Io)?;
        let file = file.to_str().ok_or(ExportError::Io)?;
        let id = self.allocate()?;
        self.patch.insert(
            format!("obj:{id}"),
            json!({"stream":{"dict":dict,"datafile":file}}),
        );
        Ok(id)
    }
}
