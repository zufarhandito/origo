//! True permanent PDF redaction (rasterizing only redacted pages with opaque pixel burn-in
//! so underlying text/vector streams are physically destroyed) and metadata sanitization
//! (stripping `/Info`, XMP `/Metadata`, and `/PieceInfo` via QPDF JSON patching).
use pdf_application::{
    ExactPageRenderer, ExactRenderRequest, PageGeometry, PdfInspector,
    export::{ExportControl, ExportError, ExportPhase, ExportPlan, resolve_source},
};
use pdf_domain::annotation::rotate;
use pdf_platform::{StagedPdfOutput, compress::write_single_page_raster_pdf, fingerprint, run};
use serde_json::{Map, Value, json};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    path::{Path, PathBuf},
    process::Command,
    time::Instant,
};

const JSON_LIMIT: usize = 16 * 1024 * 1024;
const MAX_BITMAP_DIMENSION: u32 = 8192;
const MAX_BITMAP_PIXELS: u64 = 16 * 1024 * 1024;
const REDACT_JPEG_QUALITY: u8 = 92;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RedactDpi {
    #[default]
    Dpi200,
    Dpi300,
}

impl RedactDpi {
    pub const fn dpi(self) -> u32 {
        match self {
            Self::Dpi200 => 200,
            Self::Dpi300 => 300,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Dpi200 => "200 DPI (Standar)",
            Self::Dpi300 => "300 DPI (Tajam / Cetak)",
        }
    }
}

/// Normalized redaction box in unrotated page space `[0.0..=1.0]`, matching workspace annotation coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RedactRect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl RedactRect {
    pub fn from_corners(a: [f32; 2], b: [f32; 2]) -> Option<Self> {
        let left = a[0].min(b[0]).clamp(0.0, 1.0);
        let right = a[0].max(b[0]).clamp(0.0, 1.0);
        let top = a[1].min(b[1]).clamp(0.0, 1.0);
        let bottom = a[1].max(b[1]).clamp(0.0, 1.0);
        let rect = Self {
            left,
            top,
            right,
            bottom,
        };
        rect.valid().then_some(rect)
    }

    pub fn valid(self) -> bool {
        self.left.is_finite()
            && self.top.is_finite()
            && self.right.is_finite()
            && self.bottom.is_finite()
            && self.left >= 0.0
            && self.top >= 0.0
            && self.right <= 1.0
            && self.bottom <= 1.0
            && (self.right - self.left) >= 0.002
            && (self.bottom - self.top) >= 0.002
    }

    /// Transform this unrotated box into visual normalized coordinates for a page rotated by `quarter_turns`.
    pub fn visual_bounds(self, quarter_turns: u8) -> [f32; 4] {
        let p0 = rotate([self.left, self.top], quarter_turns);
        let p1 = rotate([self.right, self.bottom], quarter_turns);
        [
            p0[0].min(p1[0]).clamp(0.0, 1.0),
            p0[1].min(p1[1]).clamp(0.0, 1.0),
            p0[0].max(p1[0]).clamp(0.0, 1.0),
            p0[1].max(p1[1]).clamp(0.0, 1.0),
        ]
    }
}

#[derive(Clone, Debug)]
pub struct RedactOptions {
    /// Redaction rectangles keyed by 0-based page index in the `ExportPlan`.
    pub boxes_by_page: BTreeMap<usize, Vec<RedactRect>>,
    /// Opaque RGB fill color burned into pixels (default black `[0, 0, 0]`).
    pub fill_color: [u8; 3],
    pub dpi: RedactDpi,
    /// When true, strips `/Info` trailer dictionary and `/Metadata` / `/PieceInfo` streams & dicts.
    pub sanitize_metadata: bool,
}

impl Default for RedactOptions {
    fn default() -> Self {
        Self {
            boxes_by_page: BTreeMap::new(),
            fill_color: [0, 0, 0],
            dpi: RedactDpi::Dpi200,
            sanitize_metadata: true,
        }
    }
}

impl RedactOptions {
    pub fn total_boxes(&self) -> usize {
        self.boxes_by_page.values().map(Vec::len).sum()
    }

    pub fn redacted_page_count(&self) -> usize {
        self.boxes_by_page
            .values()
            .filter(|boxes| !boxes.is_empty())
            .count()
    }

    pub fn has_work(&self) -> bool {
        self.total_boxes() > 0 || self.sanitize_metadata
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RedactReport {
    pub output: PathBuf,
    pub bytes: u64,
    pub page_count: usize,
    pub redacted_pages: usize,
    pub redacted_boxes: usize,
    pub metadata_sanitized: bool,
}

#[derive(Clone)]
pub struct QpdfRedactor {
    executable: PathBuf,
}

impl QpdfRedactor {
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

    fn read_json(&self, path: &Path, control: &ExportControl) -> Result<Value, ExportError> {
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

    /// Execute permanent redaction and/or metadata sanitization into a brand-new PDF file.
    /// Pages with redaction boxes are rasterized with opaque pixel burn-in (destroying all
    /// underlying text/vector streams), while unredacted pages remain crisp vector pages.
    #[allow(clippy::too_many_arguments)]
    pub fn redact_pdf(
        &self,
        plan: &ExportPlan,
        destination: &Path,
        options: &RedactOptions,
        renderer: &impl ExactPageRenderer,
        inspector: &impl PdfInspector,
        control: &ExportControl,
        progress: &mut dyn FnMut(usize),
    ) -> Result<RedactReport, ExportError> {
        if !options.has_work() {
            return Err(ExportError::InvalidPlan);
        }
        for (&page_idx, boxes) in &options.boxes_by_page {
            if page_idx >= plan.pages().len() || boxes.iter().any(|b| !b.valid()) {
                return Err(ExportError::InvalidPlan);
            }
        }

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
        let mut rotations: [Vec<usize>; 4] = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
        let mut redacted_pages = 0usize;
        let mut redacted_boxes = 0usize;

        for (index, page) in plan.pages().iter().enumerate() {
            control.checkpoint()?;
            let boxes = options
                .boxes_by_page
                .get(&index)
                .map(Vec::as_slice)
                .unwrap_or(&[]);

            if !boxes.is_empty() {
                let source = resolve_source(plan.sources(), page.entry.source_id)?;
                let (width, height) = raster_dimensions(page.geometry, options.dpi.dpi());
                let started = Instant::now();
                let mut rendered = renderer
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

                burn_redaction_boxes(
                    &mut rendered.rgba,
                    rendered.width,
                    rendered.height,
                    boxes,
                    page.geometry.quarter_turns,
                    options.fill_color,
                );

                let single_pdf = work.join(format!("redacted-page-{index}.pdf"));
                write_single_page_raster_pdf(
                    &single_pdf,
                    &rendered.rgba,
                    rendered.width,
                    rendered.height,
                    page.geometry.width_points,
                    page.geometry.height_points,
                    REDACT_JPEG_QUALITY,
                    control,
                )?;
                let path_str = single_pdf.to_str().ok_or(ExportError::InvalidPlan)?;
                page_inputs.push(json!({"file": path_str, "range": "1"}));
                redacted_pages += 1;
                redacted_boxes += boxes.len();
            } else {
                let source = resolve_source(plan.sources(), page.entry.source_id)?;
                let path_str = source.path.to_str().ok_or(ExportError::InvalidPlan)?;
                let page_one_based = usize::from(page.entry.source_page_index) + 1;
                page_inputs.push(json!({
                    "file": path_str,
                    "range": page_one_based.to_string()
                }));
                let delta = usize::from(page.entry.rotation_delta % 4);
                if delta > 0 {
                    rotations[delta].push(index + 1);
                }
            }
            progress(index + 1);
        }

        let spliced = work.join("spliced.pdf");
        let rotate_args: Vec<String> = (1..4)
            .filter(|i| !rotations[*i].is_empty())
            .map(|i| format!("+{}:{}", i * 90, format_ranges(&rotations[i])))
            .collect();
        let splice_job = json!({
            "empty": "",
            "outputFile": spliced.to_str().ok_or(ExportError::InvalidPlan)?,
            "pages": page_inputs,
            "rotate": rotate_args
        });
        self.run_job(&splice_job, work, "redact-splice.json", control)?;

        let mut current = spliced;
        let mut metadata_sanitized = false;
        if options.sanitize_metadata {
            let doc_json = self.read_json(&current, control)?;
            if let Some(patch) = sanitize_metadata_patch(&doc_json)? {
                let patch_path = work.join("sanitize-patch.json");
                let patch_bytes = serde_json::to_vec(&patch).map_err(|_| ExportError::Engine)?;
                std::fs::write(&patch_path, patch_bytes).map_err(|_| ExportError::Io)?;
                let patched = work.join("sanitized-raw.pdf");
                let mut arg = OsString::from("--update-from-json=");
                arg.push(&patch_path);
                let out = self.invoke(
                    &[
                        current.as_os_str().to_owned(),
                        arg,
                        patched.as_os_str().to_owned(),
                    ],
                    65536,
                    control,
                )?;
                if !matches!(out.code, Some(0 | 3)) {
                    return Err(ExportError::Engine);
                }
                current = patched;
            }
            // Garbage-collect orphaned /Info and XMP /Metadata objects cleanly into the final staged file.
            let gc_job = json!({
                "inputFile": current.to_str().ok_or(ExportError::InvalidPlan)?,
                "outputFile": staged.output_path().to_str().ok_or(ExportError::InvalidPlan)?,
                "objectStreams": "generate",
                "streamData": "compress"
            });
            self.run_job(&gc_job, work, "redact-gc.json", control)?;
            metadata_sanitized = true;
        } else {
            std::fs::copy(&current, staged.output_path()).map_err(|_| ExportError::Io)?;
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
        Ok(RedactReport {
            output,
            bytes,
            page_count: plan.pages().len(),
            redacted_pages,
            redacted_boxes,
            metadata_sanitized,
        })
    }
}

/// Overwrite RGBA pixels inside every redaction rectangle with opaque `fill_color`.
pub fn burn_redaction_boxes(
    rgba: &mut [u8],
    width: u32,
    height: u32,
    boxes: &[RedactRect],
    quarter_turns: u8,
    fill_color: [u8; 3],
) {
    if width == 0 || height == 0 || rgba.len() != width as usize * height as usize * 4 {
        return;
    }
    let w = width as usize;
    let h = height as usize;
    let pixel = [fill_color[0], fill_color[1], fill_color[2], 255];

    for rect in boxes {
        let [left, top, right, bottom] = rect.visual_bounds(quarter_turns);
        // Slightly expand by 1px to avoid anti-aliased text fringes peeking at the edges.
        let x0 = ((left * width as f32).floor() as isize - 1).clamp(0, w as isize) as usize;
        let y0 = ((top * height as f32).floor() as isize - 1).clamp(0, h as isize) as usize;
        let x1 = ((right * width as f32).ceil() as isize + 1).clamp(0, w as isize) as usize;
        let y1 = ((bottom * height as f32).ceil() as isize + 1).clamp(0, h as isize) as usize;
        for y in y0..y1 {
            let row = y * w;
            for x in x0..x1 {
                let offset = (row + x) * 4;
                rgba[offset..offset + 4].copy_from_slice(&pixel);
            }
        }
    }
}

/// Compute bounded pixel dimensions for rasterizing a page at `dpi`.
pub fn raster_dimensions(geometry: PageGeometry, dpi: u32) -> (u32, u32) {
    let scale = dpi as f64 / 72.0;
    let mut w = (f64::from(geometry.width_points) * scale).round().max(1.0);
    let mut h = (f64::from(geometry.height_points) * scale).round().max(1.0);
    let max_dim = f64::from(MAX_BITMAP_DIMENSION);
    if w > max_dim || h > max_dim {
        let factor = max_dim / w.max(h);
        w = (w * factor).floor().max(1.0);
        h = (h * factor).floor().max(1.0);
    }
    let pixels = w * h;
    if pixels > MAX_BITMAP_PIXELS as f64 {
        let factor = (MAX_BITMAP_PIXELS as f64 / pixels).sqrt();
        w = (w * factor).floor().max(1.0);
        h = (h * factor).floor().max(1.0);
    }
    (w as u32, h as u32)
}

/// Produce a QPDF v2 `--update-from-json` patch that strips `/Info`, `/Metadata` (XMP),
/// `/PieceInfo`, and `/SpiderInfo` from the document trailer, catalog, pages, and streams.
pub fn sanitize_metadata_patch(document: &Value) -> Result<Option<Value>, ExportError> {
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
        if id == "trailer" {
            if let Some(dict) = changed.get_mut("value").and_then(Value::as_object_mut)
                && dict.remove("/Info").is_some()
            {
                updates.insert(id.clone(), changed);
            }
            continue;
        }
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
        let mut dirty = false;
        dirty |= dict.remove("/Metadata").is_some();
        dirty |= dict.remove("/PieceInfo").is_some();
        if dict.get("/Type").and_then(Value::as_str) == Some("/Catalog") {
            dirty |= dict.remove("/SpiderInfo").is_some();
            dirty |= dict.remove("/Legal").is_some();
        }
        if dirty {
            updates.insert(id.clone(), changed);
        }
    }

    Ok((!updates.is_empty()).then(|| json!({"qpdf": [document["qpdf"][0].clone(), updates]})))
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
        ExportSupport, InspectedDocument, RenderError, RenderedPage, SourceDocument, SourceId,
        Workspace,
    };

    #[test]
    fn burn_redaction_boxes_overwrites_rotated_pixels() {
        let mut rgba = vec![255u8; 100 * 100 * 4];
        let rect = RedactRect {
            left: 0.1,
            top: 0.2,
            right: 0.3,
            bottom: 0.4,
        };
        burn_redaction_boxes(&mut rgba, 100, 100, &[rect], 0, [10, 20, 30]);
        let inside = (30 * 100 + 20) * 4;
        assert_eq!(&rgba[inside..inside + 4], &[10, 20, 30, 255]);
        let outside = (80 * 100 + 80) * 4;
        assert_eq!(&rgba[outside..outside + 4], &[255, 255, 255, 255]);

        // 90 deg clockwise turns [0.1..0.3, 0.2..0.4] into [0.6..0.8, 0.1..0.3]
        let mut rot_rgba = vec![255u8; 100 * 100 * 4];
        burn_redaction_boxes(&mut rot_rgba, 100, 100, &[rect], 1, [0, 0, 0]);
        let rot_inside = (20 * 100 + 70) * 4;
        assert_eq!(&rot_rgba[rot_inside..rot_inside + 4], &[0, 0, 0, 255]);
        assert_eq!(&rot_rgba[inside..inside + 4], &[255, 255, 255, 255]);
    }

    #[test]
    fn sanitize_metadata_patch_strips_info_xmp_and_piece_info() -> Result<(), ExportError> {
        let doc = json!({"qpdf": [{"jsonversion": 2}, {
            "trailer": {"value": {"/Root": "1 0 R", "/Info": "5 0 R", "/Size": 6}},
            "obj:1 0 R": {"value": {"/Type": "/Catalog", "/Pages": "2 0 R", "/Metadata": "4 0 R", "/PieceInfo": {}}},
            "obj:2 0 R": {"value": {"/Type": "/Page", "/Contents": "3 0 R", "/Metadata": "4 0 R"}},
            "obj:4 0 R": {"stream": {"dict": {"/Type": "/Metadata", "/Subtype": "/XML", "/Length": 10}}}
        }]});
        let patch = sanitize_metadata_patch(&doc)?.ok_or(ExportError::Validation)?;
        let objs = &patch["qpdf"][1];
        assert!(objs["trailer"]["value"].get("/Info").is_none());
        assert_eq!(objs["trailer"]["value"]["/Root"], json!("1 0 R"));
        assert!(objs["obj:1 0 R"]["value"].get("/Metadata").is_none());
        assert!(objs["obj:1 0 R"]["value"].get("/PieceInfo").is_none());
        assert!(objs["obj:2 0 R"]["value"].get("/Metadata").is_none());
        // Applying again is a clean no-op.
        assert!(sanitize_metadata_patch(&patch)?.is_none());
        Ok(())
    }

    struct StubRenderer;
    impl ExactPageRenderer for StubRenderer {
        fn render_exact(&self, request: &ExactRenderRequest) -> Result<RenderedPage, RenderError> {
            Ok(RenderedPage {
                width: request.width,
                height: request.height,
                rgba: vec![240u8; request.width as usize * request.height as usize * 4],
                page_count: 2,
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

    fn two_page_pdf_with_secret_and_info() -> Vec<u8> {
        let mut out = Vec::new();
        let mut offsets = Vec::new();
        out.extend_from_slice(b"%PDF-1.4\n");
        let objs: &[&[u8]] = &[
            b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n",
            b"2 0 obj\n<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>\nendobj\n",
            b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 300] /Resources << /Font << /F1 7 0 R >> >> /Contents 5 0 R >>\nendobj\n",
            b"4 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 300] /Resources << /Font << /F1 7 0 R >> >> /Contents 6 0 R >>\nendobj\n",
            b"5 0 obj\n<< /Length 44 >>\nstream\nBT /F1 12 Tf 20 200 Td (SECRET_TOKEN_99) Tj ET\nendstream\nendobj\n",
            b"6 0 obj\n<< /Length 44 >>\nstream\nBT /F1 12 Tf 20 200 Td (PUBLIC_PAGE_TWO) Tj ET\nendstream\nendobj\n",
            b"7 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n",
            b"8 0 obj\n<< /Author (RAHASIA_PENULIS) /Title (DOKUMEN_SENSITIF) >>\nendobj\n",
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
                "trailer\n<< /Size {} /Root 1 0 R /Info 8 0 R >>\nstartxref\n{xref}\n%%EOF\n",
                objs.len() + 1
            )
            .as_bytes(),
        );
        out
    }

    #[test]
    fn true_redaction_destroys_page_one_text_strips_info_and_keeps_page_two()
    -> Result<(), Box<dyn std::error::Error>> {
        let sidecars = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sidecars");
        let redactor = QpdfRedactor::new(&sidecars);
        let dir = std::env::temp_dir().join(format!("bpdf-redact-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir)?;

        let src = dir.join("secret.pdf");
        let dest = dir.join("secret-redacted.pdf");
        std::fs::write(&src, two_page_pdf_with_secret_and_info())?;

        let fp = fingerprint(&src)?;
        let ws = Workspace::new(SourceDocument {
            id: SourceId(1),
            path: src.clone(),
            fingerprint: fp,
            geometries: vec![
                PageGeometry {
                    width_points: 200.0,
                    height_points: 300.0,
                    quarter_turns: 0,
                };
                2
            ],
            export_support: ExportSupport::NotPreflighted,
            annotations: vec![Vec::new(); 2],
        })
        .map_err(|_| "workspace init failed")?;

        let plan = ExportPlan::capture(&ws, 1, pdf_application::export::ExportScope::All)?;
        let mut boxes = BTreeMap::new();
        boxes.insert(
            0usize,
            vec![RedactRect {
                left: 0.05,
                top: 0.25,
                right: 0.95,
                bottom: 0.45,
            }],
        );
        let opts = RedactOptions {
            boxes_by_page: boxes,
            fill_color: [0, 0, 0],
            dpi: RedactDpi::Dpi200,
            sanitize_metadata: true,
        };
        let control = ExportControl::default();
        let mut last_done = 0usize;
        let report = redactor.redact_pdf(
            &plan,
            &dest,
            &opts,
            &StubRenderer,
            &StubInspector(2),
            &control,
            &mut |done| last_done = done,
        )?;
        assert_eq!(last_done, 2);
        assert_eq!(report.page_count, 2);
        assert_eq!(report.redacted_pages, 1);
        assert_eq!(report.redacted_boxes, 1);
        assert!(report.metadata_sanitized);
        assert!(report.bytes > 0);

        // Inspect output PDF via QPDF JSON & decompressed stream inspection:
        // - trailer has NO /Info
        // - SECRET_TOKEN_99 and RAHASIA_PENULIS are completely gone
        // - PUBLIC_PAGE_TWO on unredacted page 2 is still preserved!
        let json_doc = redactor.read_json(&dest, &control)?;
        assert!(
            json_doc.pointer("/qpdf/1/trailer/value/Info").is_none()
                && json_doc.pointer("/qpdf/1/trailer/value/~1Info").is_none()
        );
        let qdf_out = dir.join("inspect.qdf");
        let qdf_job = json!({
            "inputFile": dest.to_str().ok_or("utf8")?,
            "outputFile": qdf_out.to_str().ok_or("utf8")?,
            "qdf": ""
        });
        redactor.run_job(&qdf_job, &dir, "qdf.json", &control)?;
        let qdf_text = String::from_utf8_lossy(&std::fs::read(&qdf_out)?).into_owned();
        assert!(
            !qdf_text.contains("SECRET_TOKEN_99"),
            "redacted page text must be permanently destroyed"
        );
        assert!(
            !qdf_text.contains("RAHASIA_PENULIS"),
            "metadata Author must be stripped"
        );
        assert!(
            !qdf_text.contains("DOKUMEN_SENSITIF"),
            "metadata Title must be stripped"
        );
        assert!(
            qdf_text.contains("PUBLIC_PAGE_TWO"),
            "unredacted page vector text should remain intact"
        );

        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }
}
