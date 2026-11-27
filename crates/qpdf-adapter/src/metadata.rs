//! PDF Document Properties / Metadata editor (`/Info` + XMP cleanup) and
//! permanent Flatten PDF (baking annotations, signatures, watermarks, and forms into static pages).
use crate::redact::{RedactDpi, raster_dimensions, sanitize_metadata_patch};
use pdf_application::{
    ExactPageRenderer, ExactRenderRequest, PdfInspector,
    export::{ExportControl, ExportError, ExportPhase, ExportPlan, resolve_source},
};
use pdf_platform::{StagedPdfOutput, compress::write_single_page_raster_pdf, fingerprint, run};
use serde_json::{Map, Value, json};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::Command,
    time::Instant,
};

const JSON_LIMIT: usize = 16 * 1024 * 1024;
const FLATTEN_JPEG_QUALITY: u8 = 92;
const MAX_FIELD_CHARS: usize = 512;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PdfMetadata {
    pub title: String,
    pub author: String,
    pub subject: String,
    pub keywords: String,
    pub creator: String,
    pub producer: String,
}

impl PdfMetadata {
    pub fn is_empty(&self) -> bool {
        self.title.trim().is_empty()
            && self.author.trim().is_empty()
            && self.subject.trim().is_empty()
            && self.keywords.trim().is_empty()
            && self.creator.trim().is_empty()
            && self.producer.trim().is_empty()
    }

    pub fn sanitized(&self) -> Self {
        let clean = |s: &str| -> String {
            s.chars()
                .filter(|c| !c.is_control() || *c == ' ')
                .take(MAX_FIELD_CHARS)
                .collect::<String>()
                .trim()
                .to_owned()
        };
        Self {
            title: clean(&self.title),
            author: clean(&self.author),
            subject: clean(&self.subject),
            keywords: clean(&self.keywords),
            creator: clean(&self.creator),
            producer: clean(&self.producer),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetadataReport {
    pub output: PathBuf,
    pub bytes: u64,
    pub page_count: usize,
    pub cleared: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct FlattenOptions {
    pub dpi: RedactDpi,
    pub strip_metadata: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlattenReport {
    pub output: PathBuf,
    pub bytes: u64,
    pub page_count: usize,
}

#[derive(Clone)]
pub struct QpdfMetadata {
    executable: PathBuf,
}

impl QpdfMetadata {
    pub fn new(sidecars: &Path) -> Self {
        Self {
            executable: sidecars.join("qpdf/bin/qpdf.exe"),
        }
    }

    fn invoke(
        &self,
        args: &[OsString],
        limit: usize,
        control: &ExportControl,
    ) -> Result<pdf_platform::ProcessOutput, ExportError> {
        let mut command = Command::new(&self.executable);
        command.args(args);
        run(command, limit, control)
    }

    fn verify_version(&self, control: &ExportControl) -> Result<(), ExportError> {
        let out = self.invoke(&["--version".into()], 65536, control)?;
        if out.code != Some(0)
            || String::from_utf8_lossy(&out.stdout).lines().next() != Some("qpdf version 12.4.2")
        {
            return Err(ExportError::Dependency);
        }
        Ok(())
    }

    fn run_job(
        &self,
        job: &Value,
        work: &Path,
        name: &str,
        control: &ExportControl,
    ) -> Result<(), ExportError> {
        control.checkpoint()?;
        let bytes = serde_json::to_vec(job).map_err(|_| ExportError::Engine)?;
        let path = work.join(name);
        std::fs::write(&path, bytes).map_err(|_| ExportError::Io)?;
        let mut arg = OsString::from("--job-json-file=");
        arg.push(&path);
        let out = self.invoke(&[arg], 65536, control)?;
        if !matches!(out.code, Some(0 | 3)) {
            return Err(ExportError::Engine);
        }
        Ok(())
    }

    fn check(&self, path: &Path, control: &ExportControl) -> Result<(), ExportError> {
        let out = self.invoke(
            &["--check".into(), path.as_os_str().to_owned()],
            65536,
            control,
        )?;
        if !matches!(out.code, Some(0 | 3)) {
            return Err(ExportError::Validation);
        }
        Ok(())
    }

    fn read_qpdf_json(&self, path: &Path, control: &ExportControl) -> Result<Value, ExportError> {
        let out = self.invoke(
            &[
                "--json=2".into(),
                "--json-stream-data=none".into(),
                "--json-key=qpdf".into(),
                path.as_os_str().to_owned(),
            ],
            JSON_LIMIT,
            control,
        )?;
        if !matches!(out.code, Some(0 | 3)) {
            return Err(ExportError::Inspection);
        }
        serde_json::from_slice(&out.stdout).map_err(|_| ExportError::Inspection)
    }

    /// Read `/Info` metadata dictionary from an unencrypted PDF file.
    pub fn read_metadata(
        &self,
        source: &Path,
        control: &ExportControl,
    ) -> Result<PdfMetadata, ExportError> {
        self.verify_version(control)?;
        let canonical = dunce::canonicalize(source).map_err(|_| ExportError::SourceChanged)?;
        let doc = self.read_qpdf_json(&canonical, control)?;
        extract_metadata_from_json(&doc)
    }

    /// Write updated `/Info` metadata (or clear all metadata when all fields are empty)
    /// into a brand-new PDF file, optionally stripping XMP `/Metadata` so viewers never
    /// show conflicting hidden metadata.
    #[allow(clippy::too_many_arguments)]
    pub fn write_metadata(
        &self,
        plan: &ExportPlan,
        destination: &Path,
        metadata: &PdfMetadata,
        strip_xmp: bool,
        inspector: &impl PdfInspector,
        control: &ExportControl,
    ) -> Result<MetadataReport, ExportError> {
        control.checkpoint()?;
        control.set_phase(ExportPhase::Snapshot);
        self.verify_version(control)?;
        for source in plan.sources() {
            control.checkpoint()?;
            if fingerprint(&source.path)? != source.fingerprint {
                return Err(ExportError::SourceChanged);
            }
        }

        let clean = metadata.sanitized();
        let staged = StagedPdfOutput::stage(destination, control)?;
        let work = staged.work_dir();
        control.set_phase(ExportPhase::Writing);

        // If single-source whole-document unmodified, copy source directly so existing /Info & structure are preserved before patching.
        let baseline = work.join("meta-baseline.pdf");
        if plan.is_whole_source_extract() {
            let source = resolve_source(plan.sources(), plan.sources()[0].id)?;
            std::fs::copy(&source.path, &baseline).map_err(|_| ExportError::Io)?;
        } else {
            let splice_job = build_splice_job(plan, &baseline)?;
            self.run_job(&splice_job, work, "meta-splice.json", control)?;
        }

        let doc = self.read_qpdf_json(&baseline, control)?;
        let patch = build_metadata_patch(&doc, &clean, strip_xmp)?;
        let patch_path = work.join("meta-patch.json");
        let patch_bytes = serde_json::to_vec(&patch).map_err(|_| ExportError::Engine)?;
        std::fs::write(&patch_path, patch_bytes).map_err(|_| ExportError::Io)?;

        let patched = work.join("meta-patched.pdf");
        let mut opt = OsString::from("--update-from-json=");
        opt.push(&patch_path);
        let apply = self.invoke(
            &[
                baseline.as_os_str().to_owned(),
                opt,
                patched.as_os_str().to_owned(),
            ],
            65536,
            control,
        )?;
        if !matches!(apply.code, Some(0 | 3)) {
            return Err(ExportError::Engine);
        }

        // Clean pass to garbage-collect any removed /Info or /Metadata objects.
        let gc_job = json!({
            "inputFile": patched.to_str().ok_or(ExportError::InvalidPlan)?,
            "outputFile": staged.output_path().to_str().ok_or(ExportError::InvalidPlan)?,
            "objectStreams": "generate",
            "streamData": "compress"
        });
        self.run_job(&gc_job, work, "meta-gc.json", control)?;

        control.set_phase(ExportPhase::Validation);
        self.check(staged.output_path(), control)?;
        let inspected = inspector
            .inspect_output(staged.output_path())
            .map_err(|_| ExportError::Validation)?;
        if inspected.pages.len() != plan.pages().len() {
            return Err(ExportError::Validation);
        }

        let (output, bytes) = staged.commit(control)?;
        Ok(MetadataReport {
            output,
            bytes,
            page_count: plan.pages().len(),
            cleared: clean.is_empty(),
        })
    }

    /// Permanently flatten all annotations, signatures, stamps, watermarks, and form appearances
    /// into static non-editable pages in a brand-new PDF file.
    #[allow(clippy::too_many_arguments)]
    pub fn flatten_pdf(
        &self,
        plan: &ExportPlan,
        destination: &Path,
        options: FlattenOptions,
        renderer: &impl ExactPageRenderer,
        inspector: &impl PdfInspector,
        control: &ExportControl,
        progress: &mut dyn FnMut(usize),
    ) -> Result<FlattenReport, ExportError> {
        control.checkpoint()?;
        control.set_phase(ExportPhase::Snapshot);
        self.verify_version(control)?;
        for source in plan.sources() {
            control.checkpoint()?;
            if fingerprint(&source.path)? != source.fingerprint {
                return Err(ExportError::SourceChanged);
            }
        }

        let staged = StagedPdfOutput::stage(destination, control)?;
        let work = staged.work_dir();
        control.set_phase(ExportPhase::Writing);

        let mut page_inputs: Vec<Value> = Vec::with_capacity(plan.pages().len());
        for (index, page) in plan.pages().iter().enumerate() {
            control.checkpoint()?;
            let source = resolve_source(plan.sources(), page.entry.source_id)?;
            let (width, height) = raster_dimensions(page.geometry, options.dpi.dpi());
            let started = Instant::now();
            let rendered = renderer
                .render_exact(&ExactRenderRequest {
                    annotations: Some(page.annotations.clone()),
                    path: source.path.clone(),
                    page_index: page.entry.source_page_index,
                    rotation_delta: page.entry.rotation_delta,
                    fingerprint: source.fingerprint,
                    width,
                    height,
                })
                .map_err(|_| ExportError::Engine)?;
            control.charge(started.elapsed());
            control.remaining()?;

            if rendered.width != width
                || rendered.height != height
                || rendered.rgba.len() != width as usize * height as usize * 4
            {
                return Err(ExportError::Validation);
            }

            let single = work.join(format!("flatten-page-{index}.pdf"));
            write_single_page_raster_pdf(
                &single,
                &rendered.rgba,
                rendered.width,
                rendered.height,
                page.geometry.width_points,
                page.geometry.height_points,
                FLATTEN_JPEG_QUALITY,
                control,
            )?;
            let path_str = single.to_str().ok_or(ExportError::InvalidPlan)?;
            page_inputs.push(json!({"file": path_str, "range": "1"}));
            progress(index + 1);
        }

        let merged = work.join("flatten-merged.pdf");
        let merge_job = json!({
            "empty": "",
            "outputFile": merged.to_str().ok_or(ExportError::InvalidPlan)?,
            "pages": page_inputs
        });
        self.run_job(&merge_job, work, "flatten-merge.json", control)?;

        if options.strip_metadata {
            let doc = self.read_qpdf_json(&merged, control)?;
            let mut current = merged;
            if let Some(patch) = sanitize_metadata_patch(&doc)? {
                let patch_path = work.join("flatten-sanitize.json");
                let bytes = serde_json::to_vec(&patch).map_err(|_| ExportError::Engine)?;
                std::fs::write(&patch_path, bytes).map_err(|_| ExportError::Io)?;
                let sanitized = work.join("flatten-sanitized.pdf");
                let mut arg = OsString::from("--update-from-json=");
                arg.push(&patch_path);
                let out = self.invoke(
                    &[
                        current.as_os_str().to_owned(),
                        arg,
                        sanitized.as_os_str().to_owned(),
                    ],
                    65536,
                    control,
                )?;
                if !matches!(out.code, Some(0 | 3)) {
                    return Err(ExportError::Engine);
                }
                current = sanitized;
            }
            let gc_job = json!({
                "inputFile": current.to_str().ok_or(ExportError::InvalidPlan)?,
                "outputFile": staged.output_path().to_str().ok_or(ExportError::InvalidPlan)?,
                "objectStreams": "generate",
                "streamData": "compress"
            });
            self.run_job(&gc_job, work, "flatten-gc.json", control)?;
        } else {
            let gc_job = json!({
                "inputFile": merged.to_str().ok_or(ExportError::InvalidPlan)?,
                "outputFile": staged.output_path().to_str().ok_or(ExportError::InvalidPlan)?,
                "objectStreams": "generate",
                "streamData": "compress"
            });
            self.run_job(&gc_job, work, "flatten-gc.json", control)?;
        }

        control.set_phase(ExportPhase::Validation);
        self.check(staged.output_path(), control)?;
        let inspected = inspector
            .inspect_output(staged.output_path())
            .map_err(|_| ExportError::Validation)?;
        if inspected.pages.len() != plan.pages().len() {
            return Err(ExportError::Validation);
        }

        let (output, bytes) = staged.commit(control)?;
        Ok(FlattenReport {
            output,
            bytes,
            page_count: plan.pages().len(),
        })
    }
}

pub fn decode_qpdf_string(val: &Value) -> String {
    let Some(raw) = val.as_str() else {
        return String::new();
    };
    if let Some(unicode) = raw.strip_prefix("u:") {
        return unicode.to_owned();
    }
    if let Some(hex) = raw.strip_prefix("b:") {
        // Decode hex bytes: handle UTF-16BE BOM (feff) or fall back to Latin-1.
        let bytes: Option<Vec<u8>> = hex
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| {
                let s = std::str::from_utf8(pair).ok()?;
                u8::from_str_radix(s, 16).ok()
            })
            .collect();
        if let Some(bytes) = bytes {
            if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
                let units: Vec<u16> = bytes[2..]
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|c| u16::from_be_bytes(*c))
                    .collect();
                return String::from_utf16_lossy(&units);
            }
            return bytes.into_iter().map(char::from).collect();
        }
    }
    raw.to_owned()
}

pub fn extract_metadata_from_json(document: &Value) -> Result<PdfMetadata, ExportError> {
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
    let Some(info_entry) = objects
        .get("trailer")
        .and_then(|t| t.get("value"))
        .and_then(|v| v.get("/Info"))
    else {
        return Ok(PdfMetadata::default());
    };
    let dict = if let Some(inline) = info_entry.as_object() {
        inline
    } else if let Some(ref_str) = info_entry.as_str() {
        let key = format!("obj:{ref_str}");
        let Some(found) = objects
            .get(&key)
            .and_then(|o| o.get("value"))
            .and_then(Value::as_object)
        else {
            return Ok(PdfMetadata::default());
        };
        found
    } else {
        return Ok(PdfMetadata::default());
    };

    let field = |k: &str| dict.get(k).map(decode_qpdf_string).unwrap_or_default();
    Ok(PdfMetadata {
        title: field("/Title"),
        author: field("/Author"),
        subject: field("/Subject"),
        keywords: field("/Keywords"),
        creator: field("/Creator"),
        producer: field("/Producer"),
    })
}

pub fn build_metadata_patch(
    document: &Value,
    metadata: &PdfMetadata,
    strip_xmp: bool,
) -> Result<Value, ExportError> {
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

    if strip_xmp {
        for (id, original) in objects {
            if id == "trailer" {
                continue;
            }
            let mut changed = original.clone();
            let dict = if original.get("stream").is_some() {
                changed
                    .pointer_mut("/stream/dict")
                    .and_then(Value::as_object_mut)
            } else {
                changed.get_mut("value").and_then(Value::as_object_mut)
            };
            if let Some(dict) = dict {
                let mut dirty = dict.remove("/Metadata").is_some();
                dirty |= dict.remove("/PieceInfo").is_some();
                if dirty {
                    updates.insert(id.clone(), changed);
                }
            }
        }
    }

    let trailer = objects
        .get("trailer")
        .ok_or(ExportError::Inspection)?
        .clone();
    let existing_info_ref = trailer
        .get("value")
        .and_then(|v| v.get("/Info"))
        .and_then(Value::as_str)
        .map(str::to_owned);

    if metadata.is_empty() {
        let mut new_trailer = trailer;
        if let Some(dict) = new_trailer.get_mut("value").and_then(Value::as_object_mut) {
            dict.remove("/Info");
        }
        updates.insert("trailer".into(), new_trailer);
    } else {
        let mut info_dict = existing_info_ref
            .as_deref()
            .and_then(|r| objects.get(&format!("obj:{r}")))
            .and_then(|o| o.get("value"))
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();

        let apply_field = |dict: &mut Map<String, Value>, key: &str, value: &str| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                dict.remove(key);
            } else {
                dict.insert(key.into(), json!(format!("u:{trimmed}")));
            }
        };
        apply_field(&mut info_dict, "/Title", &metadata.title);
        apply_field(&mut info_dict, "/Author", &metadata.author);
        apply_field(&mut info_dict, "/Subject", &metadata.subject);
        apply_field(&mut info_dict, "/Keywords", &metadata.keywords);
        apply_field(&mut info_dict, "/Creator", &metadata.creator);
        apply_field(&mut info_dict, "/Producer", &metadata.producer);

        if let Some(ref_str) = existing_info_ref {
            updates.insert(format!("obj:{ref_str}"), json!({"value": info_dict}));
        } else {
            let next_id = objects
                .keys()
                .filter_map(|k| {
                    k.strip_prefix("obj:")?
                        .split_whitespace()
                        .next()?
                        .parse::<u64>()
                        .ok()
                })
                .max()
                .unwrap_or(0)
                + 1;
            let info_ref = format!("{next_id} 0 R");
            updates.insert(format!("obj:{info_ref}"), json!({"value": info_dict}));
            let mut new_trailer = trailer;
            if let Some(dict) = new_trailer.get_mut("value").and_then(Value::as_object_mut) {
                dict.insert("/Info".into(), json!(info_ref));
            }
            updates.insert("trailer".into(), new_trailer);
        }
    }

    Ok(json!({"qpdf": [document["qpdf"][0].clone(), updates]}))
}

fn build_splice_job(plan: &ExportPlan, output: &Path) -> Result<Value, ExportError> {
    let mut page_inputs = Vec::with_capacity(plan.pages().len());
    let mut rotations: [Vec<usize>; 4] = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
    for (index, page) in plan.pages().iter().enumerate() {
        let source = resolve_source(plan.sources(), page.entry.source_id)?;
        let path_str = source.path.to_str().ok_or(ExportError::InvalidPlan)?;
        let one_based = usize::from(page.entry.source_page_index) + 1;
        page_inputs.push(json!({"file": path_str, "range": one_based.to_string()}));
        let delta = usize::from(page.entry.rotation_delta % 4);
        if delta > 0 {
            rotations[delta].push(index + 1);
        }
    }
    let rotate_args: Vec<String> = (1..4)
        .filter(|i| !rotations[*i].is_empty())
        .map(|i| format!("+{}:{}", i * 90, format_ranges(&rotations[i])))
        .collect();
    Ok(json!({
        "empty": "",
        "outputFile": output.to_str().ok_or(ExportError::InvalidPlan)?,
        "pages": page_inputs,
        "rotate": rotate_args
    }))
}

fn format_ranges(indices: &[usize]) -> String {
    let mut result = Vec::new();
    let mut i = 0;
    while i < indices.len() {
        let start = indices[i];
        let mut end = start;
        i += 1;
        while i < indices.len() && indices[i] == end + 1 {
            end = indices[i];
            i += 1;
        }
        result.push(if start == end {
            start.to_string()
        } else {
            format!("{start}-{end}")
        });
    }
    result.join(",")
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdf_application::{
        ExportSupport, InspectedDocument, PageGeometry, RenderError, RenderedPage, SourceDocument,
        SourceId, Workspace,
    };

    struct StubRenderer;
    impl ExactPageRenderer for StubRenderer {
        fn render_exact(&self, request: &ExactRenderRequest) -> Result<RenderedPage, RenderError> {
            Ok(RenderedPage {
                width: request.width,
                height: request.height,
                rgba: vec![245u8; request.width as usize * request.height as usize * 4],
                page_count: 1,
                clockwise_quarter_turns: request.rotation_delta % 4,
            })
        }
    }

    struct StubInspector(usize);
    impl PdfInspector for StubInspector {
        fn inspect(&self, source: &Path) -> Result<InspectedDocument, RenderError> {
            Ok(InspectedDocument {
                annotations: vec![Vec::new(); self.0],
                path: source.to_path_buf(),
                fingerprint: fingerprint(source).map_err(|_| RenderError::InvalidPdf)?,
                pages: vec![
                    PageGeometry {
                        width_points: 200.0,
                        height_points: 300.0,
                        quarter_turns: 0,
                    };
                    self.0
                ],
            })
        }
    }

    fn pdf_with_annot_and_no_info() -> Vec<u8> {
        let mut out = Vec::new();
        let mut offsets = Vec::new();
        out.extend_from_slice(b"%PDF-1.4\n");
        let objs: &[&[u8]] = &[
            b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n",
            b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n",
            b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 300] /Annots [5 0 R] /Contents 4 0 R >>\nendobj\n",
            b"4 0 obj\n<< /Length 0 >>\nstream\nendstream\nendobj\n",
            b"5 0 obj\n<< /Type /Annot /Subtype /Text /Rect [10 10 30 30] /Contents (Catatan) >>\nendobj\n",
        ];
        for obj in objs {
            offsets.push(out.len());
            out.extend_from_slice(obj);
        }
        let xref = out.len();
        out.extend_from_slice(
            format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes(),
        );
        for off in offsets {
            out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
        }
        out.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
                objs.len() + 1
            )
            .as_bytes(),
        );
        out
    }

    #[test]
    fn metadata_create_update_clear_and_flatten_with_real_qpdf()
    -> Result<(), Box<dyn std::error::Error>> {
        let sidecars = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sidecars");
        let tool = QpdfMetadata::new(&sidecars);
        let dir = std::env::temp_dir().join(format!("bpdf-meta-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir)?;

        let src = dir.join("doc.pdf");
        std::fs::write(&src, pdf_with_annot_and_no_info())?;
        let control = ExportControl::default();

        // Initially empty metadata
        assert_eq!(tool.read_metadata(&src, &control)?, PdfMetadata::default());

        let fp = fingerprint(&src)?;
        let ws = Workspace::new(SourceDocument {
            id: SourceId(1),
            path: src.clone(),
            fingerprint: fp,
            geometries: vec![PageGeometry {
                width_points: 200.0,
                height_points: 300.0,
                quarter_turns: 0,
            }],
            export_support: ExportSupport::NotPreflighted,
            annotations: vec![Vec::new()],
        })
        .map_err(|_| "workspace init failed")?;
        let plan = ExportPlan::capture(&ws, 1, pdf_application::export::ExportScope::All)?;

        // 1) Write brand-new /Info metadata (with Unicode)
        let meta = PdfMetadata {
            title: "Laporan Keuangan — 2026".into(),
            author: "Origo PDF Studio".into(),
            subject: "Ringkasan Tahunan".into(),
            keywords: "keuangan, audit, 2026".into(),
            creator: "Origo".into(),
            producer: "Origo Offline".into(),
        };
        let with_meta = dir.join("doc-meta.pdf");
        let report =
            tool.write_metadata(&plan, &with_meta, &meta, true, &StubInspector(1), &control)?;
        assert!(!report.cleared);
        assert_eq!(tool.read_metadata(&with_meta, &control)?, meta);

        // 2) Clear all metadata cleanly from with_meta
        let ws2 = Workspace::new(SourceDocument {
            id: SourceId(2),
            path: with_meta.clone(),
            fingerprint: fingerprint(&with_meta)?,
            geometries: vec![PageGeometry {
                width_points: 200.0,
                height_points: 300.0,
                quarter_turns: 0,
            }],
            export_support: ExportSupport::NotPreflighted,
            annotations: vec![Vec::new()],
        })
        .map_err(|_| "ws2")?;
        let plan2 = ExportPlan::capture(&ws2, 2, pdf_application::export::ExportScope::All)?;
        let cleared_pdf = dir.join("doc-cleared.pdf");
        let clear_rep = tool.write_metadata(
            &plan2,
            &cleared_pdf,
            &PdfMetadata::default(),
            true,
            &StubInspector(1),
            &control,
        )?;
        assert!(clear_rep.cleared);
        assert_eq!(
            tool.read_metadata(&cleared_pdf, &control)?,
            PdfMetadata::default()
        );

        // 3) Flatten PDF removes /Annots completely
        let flattened = dir.join("doc-flattened.pdf");
        let flat_rep = tool.flatten_pdf(
            &plan,
            &flattened,
            FlattenOptions {
                dpi: RedactDpi::Dpi200,
                strip_metadata: true,
            },
            &StubRenderer,
            &StubInspector(1),
            &control,
            &mut |_| {},
        )?;
        assert_eq!(flat_rep.page_count, 1);
        let flat_json = tool.read_qpdf_json(&flattened, &control)?;
        let objs = flat_json["qpdf"][1].as_object().ok_or("objs")?;
        for obj in objs.values() {
            if obj.pointer("/value/~1Type").and_then(Value::as_str) == Some("/Page") {
                assert!(
                    obj.pointer("/value/~1Annots").is_none(),
                    "flattened page must have zero annotations"
                );
            }
        }

        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }
}
