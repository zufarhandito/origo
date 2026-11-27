//! Lossless page margin cropping (rotation-aware `/CropBox` patching via QPDF JSON)
//! and proportional paper-size standardization (A4 / Letter / F4 / Legal).
use crate::redact::raster_dimensions;
use pdf_application::{
    ExactPageRenderer, ExactRenderRequest, PdfInspector,
    export::{ExportControl, ExportError, ExportPhase, ExportPlan, resolve_source},
};
use pdf_platform::{
    StagedPdfOutput, compress::write_single_page_raster_pdf_placed, fingerprint, run,
};
use serde_json::{Map, Value, json};
use std::{
    collections::BTreeSet,
    ffi::OsString,
    path::{Path, PathBuf},
    process::Command,
    time::Instant,
};

const JSON_LIMIT: usize = 16 * 1024 * 1024;
const MIN_REMAINING_POINTS: f32 = 36.0;
const MM_TO_POINTS: f32 = 72.0 / 25.4;
const STANDARDIZE_DPI: u32 = 250;
const STANDARDIZE_JPEG_QUALITY: u8 = 92;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PaperSize {
    #[default]
    A4,
    Letter,
    F4,
    Legal,
}

impl PaperSize {
    /// Portrait dimensions in PDF points (1/72 inch).
    pub const fn portrait_points(self) -> (f32, f32) {
        match self {
            Self::A4 => (595.28, 841.89),
            Self::Letter => (612.0, 792.0),
            Self::F4 => (612.0, 936.0),
            Self::Legal => (612.0, 1008.0),
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::A4 => "A4 (210 × 297 mm)",
            Self::Letter => "Letter (216 × 279 mm)",
            Self::F4 => "F4 / Folio (216 × 330 mm)",
            Self::Legal => "Legal (216 × 356 mm)",
        }
    }

    pub const fn suffix(self) -> &'static str {
        match self {
            Self::A4 => "a4",
            Self::Letter => "letter",
            Self::F4 => "f4",
            Self::Legal => "legal",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PaperOrientation {
    #[default]
    Auto,
    Portrait,
    Landscape,
}

impl PaperOrientation {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Auto => "Otomatis (Ikuti Tiap Halaman)",
            Self::Portrait => "Paksa Portrait (Tegak)",
            Self::Landscape => "Paksa Landscape (Mendatar)",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CropMargins {
    pub top_pt: f32,
    pub bottom_pt: f32,
    pub left_pt: f32,
    pub right_pt: f32,
}

impl CropMargins {
    pub fn from_mm(top: f32, bottom: f32, left: f32, right: f32) -> Self {
        Self {
            top_pt: top * MM_TO_POINTS,
            bottom_pt: bottom * MM_TO_POINTS,
            left_pt: left * MM_TO_POINTS,
            right_pt: right * MM_TO_POINTS,
        }
    }

    pub fn uniform_mm(mm: f32) -> Self {
        Self::from_mm(mm, mm, mm, mm)
    }

    pub fn valid(self) -> bool {
        [self.top_pt, self.bottom_pt, self.left_pt, self.right_pt]
            .into_iter()
            .all(|v| v.is_finite() && (0.0..=5000.0).contains(&v))
            && (self.top_pt > 0.0
                || self.bottom_pt > 0.0
                || self.left_pt > 0.0
                || self.right_pt > 0.0)
    }
}

#[derive(Clone, Debug)]
pub enum PageBoxOperation {
    Crop {
        margins: CropMargins,
        /// 0-based page indices inside `ExportPlan`. Empty means all pages.
        pages: Vec<usize>,
    },
    StandardizePaper {
        paper: PaperSize,
        orientation: PaperOrientation,
        /// 0-based page indices inside `ExportPlan`. Empty means all pages.
        pages: Vec<usize>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageBoxReport {
    pub output: PathBuf,
    pub bytes: u64,
    pub page_count: usize,
    pub modified_pages: usize,
}

/// Parse 1-based page range string such as `"1-3, 5, 8-10"` into sorted unique 0-based indices.
pub fn parse_page_selection(input: &str, total_pages: usize) -> Result<Vec<usize>, &'static str> {
    if total_pages == 0 {
        return Err("Dokumen tidak memiliki halaman.");
    }
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("Masukkan rentang halaman, contoh: 1-3, 5.");
    }
    let mut set = BTreeSet::new();
    for part in trimmed.split(',') {
        let token = part.trim();
        if token.is_empty() {
            return Err("Format rentang halaman tidak valid (koma kosong).");
        }
        if let Some((a, b)) = token.split_once('-') {
            let start: usize = a
                .trim()
                .parse()
                .map_err(|_| "Nomor halaman awal harus berupa angka.")?;
            let end: usize = b
                .trim()
                .parse()
                .map_err(|_| "Nomor halaman akhir harus berupa angka.")?;
            if start == 0 || end == 0 || start > end || end > total_pages {
                return Err("Rentang halaman berada di luar jumlah halaman dokumen.");
            }
            for p in start..=end {
                set.insert(p - 1);
            }
        } else {
            let p: usize = token
                .parse()
                .map_err(|_| "Nomor halaman harus berupa angka atau rentang seperti 1-3.")?;
            if p == 0 || p > total_pages {
                return Err("Nomor halaman berada di luar jumlah halaman dokumen.");
            }
            set.insert(p - 1);
        }
    }
    if set.is_empty() {
        return Err("Tidak ada halaman yang dipilih.");
    }
    Ok(set.into_iter().collect())
}

/// Compute proportional centered placement `[x, y, w, h]` of `(src_w, src_h)` inside `(dst_w, dst_h)`.
pub fn centered_fit_rect(src_w: f32, src_h: f32, dst_w: f32, dst_h: f32) -> [f32; 4] {
    if src_w <= 0.0 || src_h <= 0.0 || dst_w <= 0.0 || dst_h <= 0.0 {
        return [0.0, 0.0, dst_w.max(1.0), dst_h.max(1.0)];
    }
    let scale = (dst_w / src_w).min(dst_h / src_h);
    let draw_w = src_w * scale;
    let draw_h = src_h * scale;
    let dx = (dst_w - draw_w) * 0.5;
    let dy = (dst_h - draw_h) * 0.5;
    [dx, dy, draw_w, draw_h]
}

/// Apply visual `(top, bottom, left, right)` margin trims to an unrotated PDF box `[llx, lly, urx, ury]`
/// taking the page's clockwise rotation (`quarter_turns` in 0..4) into account.
pub fn crop_pdf_box(
    box_rect: [f32; 4],
    quarter_turns: u8,
    margins: CropMargins,
) -> Result<[f32; 4], ExportError> {
    let [mut llx, mut lly, mut urx, mut ury] = box_rect;
    if llx > urx {
        std::mem::swap(&mut llx, &mut urx);
    }
    if lly > ury {
        std::mem::swap(&mut lly, &mut ury);
    }
    match quarter_turns % 4 {
        0 => {
            llx += margins.left_pt;
            urx -= margins.right_pt;
            lly += margins.bottom_pt;
            ury -= margins.top_pt;
        }
        1 => {
            // 90 deg CW: unrotated left=visual top, right=bottom, bottom=left, top=right
            llx += margins.top_pt;
            urx -= margins.bottom_pt;
            lly += margins.left_pt;
            ury -= margins.right_pt;
        }
        2 => {
            llx += margins.right_pt;
            urx -= margins.left_pt;
            lly += margins.top_pt;
            ury -= margins.bottom_pt;
        }
        _ => {
            // 270 deg CW: unrotated left=visual bottom, right=top, bottom=right, top=left
            llx += margins.bottom_pt;
            urx -= margins.top_pt;
            lly += margins.right_pt;
            ury -= margins.left_pt;
        }
    }
    if (urx - llx) < MIN_REMAINING_POINTS || (ury - lly) < MIN_REMAINING_POINTS {
        return Err(ExportError::InvalidPlan);
    }
    Ok([llx, lly, urx, ury])
}

#[derive(Clone)]
pub struct QpdfPageBox {
    executable: PathBuf,
}

impl QpdfPageBox {
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

    #[allow(clippy::too_many_arguments)]
    pub fn apply(
        &self,
        plan: &ExportPlan,
        destination: &Path,
        operation: &PageBoxOperation,
        renderer: &impl ExactPageRenderer,
        inspector: &impl PdfInspector,
        control: &ExportControl,
        progress: &mut dyn FnMut(usize),
    ) -> Result<PageBoxReport, ExportError> {
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

        let modified_pages = match operation {
            PageBoxOperation::Crop { margins, pages } => {
                if !margins.valid() {
                    return Err(ExportError::InvalidPlan);
                }
                let targets = resolve_targets(pages, plan.pages().len())?;
                self.run_crop(
                    plan,
                    &targets,
                    *margins,
                    work,
                    staged.output_path(),
                    control,
                    progress,
                )?
            }
            PageBoxOperation::StandardizePaper {
                paper,
                orientation,
                pages,
            } => {
                let targets = resolve_targets(pages, plan.pages().len())?;
                self.run_standardize(
                    plan,
                    &targets,
                    *paper,
                    *orientation,
                    work,
                    staged.output_path(),
                    renderer,
                    control,
                    progress,
                )?
            }
        };

        control.set_phase(ExportPhase::Validation);
        self.check(staged.output_path(), control)?;
        let inspected = inspector
            .inspect_output(staged.output_path())
            .map_err(|_| ExportError::Validation)?;
        if inspected.pages.len() != plan.pages().len() {
            return Err(ExportError::Validation);
        }

        let (output, bytes) = staged.commit(control)?;
        Ok(PageBoxReport {
            output,
            bytes,
            page_count: plan.pages().len(),
            modified_pages,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn run_crop(
        &self,
        plan: &ExportPlan,
        targets: &BTreeSet<usize>,
        margins: CropMargins,
        work: &Path,
        final_staged: &Path,
        control: &ExportControl,
        progress: &mut dyn FnMut(usize),
    ) -> Result<usize, ExportError> {
        // First splice current workspace page plan (order + rotations) into a baseline PDF.
        let baseline = work.join("crop-baseline.pdf");
        let splice_job = build_plan_splice_job(plan, &baseline)?;
        self.run_job(&splice_job, work, "crop-splice.json", control)?;

        // Inspect pages + object graph from QPDF JSON v2.
        let out = self.invoke(
            &[
                "--json=2".into(),
                "--json-stream-data=none".into(),
                "--json-key=pages".into(),
                "--json-key=qpdf".into(),
                baseline.as_os_str().to_owned(),
            ],
            JSON_LIMIT,
            control,
        )?;
        if !matches!(out.code, Some(0 | 3)) {
            return Err(ExportError::Inspection);
        }
        let doc: Value =
            serde_json::from_slice(&out.stdout).map_err(|_| ExportError::Inspection)?;
        let (patch, modified) = build_crop_patch(&doc, targets, margins, progress)?;
        let patch_path = work.join("crop-patch.json");
        let bytes = serde_json::to_vec(&patch).map_err(|_| ExportError::Engine)?;
        std::fs::write(&patch_path, bytes).map_err(|_| ExportError::Io)?;

        let mut opt = OsString::from("--update-from-json=");
        opt.push(&patch_path);
        let apply_out = self.invoke(
            &[
                baseline.as_os_str().to_owned(),
                opt,
                final_staged.as_os_str().to_owned(),
            ],
            65536,
            control,
        )?;
        if !matches!(apply_out.code, Some(0 | 3)) {
            return Err(ExportError::Engine);
        }
        Ok(modified)
    }

    #[allow(clippy::too_many_arguments)]
    fn run_standardize(
        &self,
        plan: &ExportPlan,
        targets: &BTreeSet<usize>,
        paper: PaperSize,
        orientation: PaperOrientation,
        work: &Path,
        final_staged: &Path,
        renderer: &impl ExactPageRenderer,
        control: &ExportControl,
        progress: &mut dyn FnMut(usize),
    ) -> Result<usize, ExportError> {
        let mut page_inputs: Vec<Value> = Vec::with_capacity(plan.pages().len());
        let mut rotations: [Vec<usize>; 4] = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
        let mut modified = 0usize;

        for (index, page) in plan.pages().iter().enumerate() {
            control.checkpoint()?;
            let source = resolve_source(plan.sources(), page.entry.source_id)?;
            let src_w = page.geometry.width_points;
            let src_h = page.geometry.height_points;
            let (base_w, base_h) = paper.portrait_points();
            let (dst_w, dst_h) = match orientation {
                PaperOrientation::Auto => {
                    if src_w > src_h {
                        (base_h, base_w)
                    } else {
                        (base_w, base_h)
                    }
                }
                PaperOrientation::Portrait => (base_w, base_h),
                PaperOrientation::Landscape => (base_h, base_w),
            };
            let already_matches = (src_w - dst_w).abs() <= 1.5 && (src_h - dst_h).abs() <= 1.5;

            if targets.contains(&index) && !already_matches {
                let (px_w, px_h) = raster_dimensions(page.geometry, STANDARDIZE_DPI);
                let started = Instant::now();
                let rendered = renderer
                    .render_exact(&ExactRenderRequest {
                        annotations: Some(page.annotations.clone()),
                        path: source.path.clone(),
                        page_index: page.entry.source_page_index,
                        rotation_delta: page.entry.rotation_delta,
                        fingerprint: source.fingerprint,
                        width: px_w,
                        height: px_h,
                    })
                    .map_err(|_| ExportError::Engine)?;
                control.charge(started.elapsed());
                control.remaining()?;

                if rendered.width != px_w
                    || rendered.height != px_h
                    || rendered.rgba.len() != px_w as usize * px_h as usize * 4
                {
                    return Err(ExportError::Validation);
                }

                let draw_rect = centered_fit_rect(src_w, src_h, dst_w, dst_h);
                let single_pdf = work.join(format!("paper-page-{index}.pdf"));
                write_single_page_raster_pdf_placed(
                    &single_pdf,
                    &rendered.rgba,
                    rendered.width,
                    rendered.height,
                    dst_w,
                    dst_h,
                    draw_rect,
                    STANDARDIZE_JPEG_QUALITY,
                    control,
                )?;
                let path_str = single_pdf.to_str().ok_or(ExportError::InvalidPlan)?;
                page_inputs.push(json!({"file": path_str, "range": "1"}));
                modified += 1;
            } else {
                let path_str = source.path.to_str().ok_or(ExportError::InvalidPlan)?;
                let one_based = usize::from(page.entry.source_page_index) + 1;
                page_inputs.push(json!({"file": path_str, "range": one_based.to_string()}));
                let delta = usize::from(page.entry.rotation_delta % 4);
                if delta > 0 {
                    rotations[delta].push(index + 1);
                }
            }
            progress(index + 1);
        }

        let rotate_args: Vec<String> = (1..4)
            .filter(|i| !rotations[*i].is_empty())
            .map(|i| format!("+{}:{}", i * 90, format_ranges(&rotations[i])))
            .collect();
        let splice_job = json!({
            "empty": "",
            "outputFile": final_staged.to_str().ok_or(ExportError::InvalidPlan)?,
            "pages": page_inputs,
            "rotate": rotate_args
        });
        self.run_job(&splice_job, work, "standardize-splice.json", control)?;
        Ok(modified)
    }
}

fn resolve_targets(pages: &[usize], total: usize) -> Result<BTreeSet<usize>, ExportError> {
    if total == 0 {
        return Err(ExportError::Empty);
    }
    if pages.is_empty() {
        return Ok((0..total).collect());
    }
    let mut set = BTreeSet::new();
    for &p in pages {
        if p >= total {
            return Err(ExportError::InvalidPlan);
        }
        set.insert(p);
    }
    Ok(set)
}

fn build_plan_splice_job(plan: &ExportPlan, output: &Path) -> Result<Value, ExportError> {
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

fn parse_box_array(val: &Value) -> Option<[f32; 4]> {
    let arr = val.as_array()?;
    if arr.len() != 4 {
        return None;
    }
    let a = arr[0].as_f64()? as f32;
    let b = arr[1].as_f64()? as f32;
    let c = arr[2].as_f64()? as f32;
    let d = arr[3].as_f64()? as f32;
    if [a, b, c, d].into_iter().all(f32::is_finite) {
        Some([a, b, c, d])
    } else {
        None
    }
}

fn inherited_value<'a>(
    objects: &'a Map<String, Value>,
    start_id: &str,
    key: &str,
) -> Option<&'a Value> {
    let mut current = start_id.to_owned();
    for _ in 0..32 {
        let obj_key = if current.starts_with("obj:") {
            current.clone()
        } else {
            format!("obj:{current}")
        };
        let dict = objects.get(&obj_key)?.get("value")?.as_object()?;
        if let Some(found) = dict.get(key) {
            return Some(found);
        }
        let parent = dict.get("/Parent")?.as_str()?;
        current = parent.to_owned();
    }
    None
}

fn build_crop_patch(
    document: &Value,
    targets: &BTreeSet<usize>,
    margins: CropMargins,
    progress: &mut dyn FnMut(usize),
) -> Result<(Value, usize), ExportError> {
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
    let pages = document
        .get("pages")
        .and_then(Value::as_array)
        .ok_or(ExportError::Inspection)?;

    let mut updates = Map::new();
    let mut modified = 0usize;

    for (index, page_entry) in pages.iter().enumerate() {
        if targets.contains(&index) {
            let obj_ref = page_entry
                .get("object")
                .and_then(Value::as_str)
                .ok_or(ExportError::Inspection)?;
            let obj_key = format!("obj:{obj_ref}");
            let original = objects.get(&obj_key).ok_or(ExportError::Inspection)?;
            let mut changed = original.clone();
            let base_box = inherited_value(objects, obj_ref, "/CropBox")
                .and_then(parse_box_array)
                .or_else(|| {
                    inherited_value(objects, obj_ref, "/MediaBox").and_then(parse_box_array)
                })
                .ok_or(ExportError::Inspection)?;
            let rotate_deg = inherited_value(objects, obj_ref, "/Rotate")
                .and_then(Value::as_i64)
                .unwrap_or(0);
            let turns = ((rotate_deg.rem_euclid(360)) / 90) as u8;
            let cropped = crop_pdf_box(base_box, turns, margins)?;
            let dict = changed
                .get_mut("value")
                .and_then(Value::as_object_mut)
                .ok_or(ExportError::Inspection)?;
            let box_json = json!([
                round3(cropped[0]),
                round3(cropped[1]),
                round3(cropped[2]),
                round3(cropped[3])
            ]);
            dict.insert("/CropBox".into(), box_json.clone());
            // Keep TrimBox/ArtBox consistent if present so viewers don't disagree.
            if dict.contains_key("/TrimBox") {
                dict.insert("/TrimBox".into(), box_json.clone());
            }
            if dict.contains_key("/ArtBox") {
                dict.insert("/ArtBox".into(), box_json);
            }
            updates.insert(obj_key, changed);
            modified += 1;
        }
        progress(index + 1);
    }

    Ok((
        json!({"qpdf": [document["qpdf"][0].clone(), updates]}),
        modified,
    ))
}

fn round3(v: f32) -> f64 {
    (f64::from(v) * 1000.0).round() / 1000.0
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

    #[test]
    fn parse_page_selection_handles_ranges_singles_and_errors() {
        assert_eq!(parse_page_selection("1-3, 5, 2", 6), Ok(vec![0, 1, 2, 4]));
        assert!(parse_page_selection("", 5).is_err());
        assert!(parse_page_selection("0-2", 5).is_err());
        assert!(parse_page_selection("4-2", 5).is_err());
        assert!(parse_page_selection("1-7", 5).is_err());
    }

    #[test]
    fn crop_pdf_box_respects_rotation_and_min_size() -> Result<(), ExportError> {
        let margins = CropMargins {
            top_pt: 40.0,
            bottom_pt: 20.0,
            left_pt: 10.0,
            right_pt: 30.0,
        };
        assert_eq!(
            crop_pdf_box([0.0, 0.0, 600.0, 800.0], 0, margins)?,
            [10.0, 20.0, 570.0, 760.0]
        );
        // 90 deg CW: visual top trims llx, visual bottom trims urx, visual left trims lly, visual right trims ury
        assert_eq!(
            crop_pdf_box([0.0, 0.0, 600.0, 800.0], 1, margins)?,
            [40.0, 10.0, 580.0, 770.0]
        );
        // Over-cropping below 36pt fails cleanly.
        assert!(crop_pdf_box([0.0, 0.0, 100.0, 100.0], 0, CropMargins::uniform_mm(30.0)).is_err());
        Ok(())
    }

    #[test]
    fn centered_fit_preserves_aspect_ratio() {
        let [x, y, w, h] = centered_fit_rect(612.0, 792.0, 595.28, 841.89);
        assert!(x.abs() < 0.01);
        assert!(y > 20.0);
        assert!((w - 595.28).abs() < 0.05);
        assert!((w / h - 612.0 / 792.0).abs() < 0.001);
    }

    struct StubRenderer;
    impl ExactPageRenderer for StubRenderer {
        fn render_exact(&self, request: &ExactRenderRequest) -> Result<RenderedPage, RenderError> {
            Ok(RenderedPage {
                width: request.width,
                height: request.height,
                rgba: vec![250u8; request.width as usize * request.height as usize * 4],
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
                        width_points: 595.28,
                        height_points: 841.89,
                        quarter_turns: 0,
                    };
                    self.0
                ],
            })
        }
    }

    fn sample_letter_pdf() -> Vec<u8> {
        let mut out = Vec::new();
        let mut offsets = Vec::new();
        out.extend_from_slice(b"%PDF-1.4\n");
        let objs: &[&[u8]] = &[
            b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n",
            b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n",
            b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R >>\nendobj\n",
            b"4 0 obj\n<< /Length 0 >>\nstream\nendstream\nendobj\n",
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
    fn crop_and_standardize_a4_with_real_qpdf() -> Result<(), Box<dyn std::error::Error>> {
        let sidecars = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sidecars");
        let tool = QpdfPageBox::new(&sidecars);
        let dir = std::env::temp_dir().join(format!("bpdf-pagebox-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir)?;

        let src = dir.join("letter.pdf");
        std::fs::write(&src, sample_letter_pdf())?;
        let fp = fingerprint(&src)?;
        let ws = Workspace::new(SourceDocument {
            id: SourceId(1),
            path: src.clone(),
            fingerprint: fp,
            geometries: vec![PageGeometry {
                width_points: 612.0,
                height_points: 792.0,
                quarter_turns: 0,
            }],
            export_support: ExportSupport::NotPreflighted,
            annotations: vec![Vec::new()],
        })
        .map_err(|_| "workspace init failed")?;
        let plan = ExportPlan::capture(&ws, 1, pdf_application::export::ExportScope::All)?;
        let control = ExportControl::default();

        // 1) Crop margins (50pt all sides) -> CropBox [50, 50, 562, 742]
        let cropped = dir.join("letter-cropped.pdf");
        let crop_report = tool.apply(
            &plan,
            &cropped,
            &PageBoxOperation::Crop {
                margins: CropMargins {
                    top_pt: 50.0,
                    bottom_pt: 50.0,
                    left_pt: 50.0,
                    right_pt: 50.0,
                },
                pages: vec![],
            },
            &StubRenderer,
            &StubInspector(1),
            &control,
            &mut |_| {},
        )?;
        assert_eq!(crop_report.modified_pages, 1);
        let crop_json_out = tool.invoke(
            &[
                "--json=2".into(),
                "--json-stream-data=none".into(),
                "--json-key=pages".into(),
                "--json-key=qpdf".into(),
                cropped.as_os_str().to_owned(),
            ],
            JSON_LIMIT,
            &control,
        )?;
        let crop_json: Value = serde_json::from_slice(&crop_json_out.stdout)?;
        let page_ref = crop_json["pages"][0]["object"].as_str().ok_or("page ref")?;
        let crop_box = crop_json["qpdf"][1][format!("obj:{page_ref}")]["value"]["/CropBox"]
            .as_array()
            .ok_or("missing CropBox")?;
        assert_eq!(crop_box[0].as_f64(), Some(50.0));
        assert_eq!(crop_box[1].as_f64(), Some(50.0));
        assert_eq!(crop_box[2].as_f64(), Some(562.0));
        assert_eq!(crop_box[3].as_f64(), Some(742.0));

        // 2) Standardize Letter -> A4 (595.28 x 841.89)
        let a4 = dir.join("letter-a4.pdf");
        let a4_report = tool.apply(
            &plan,
            &a4,
            &PageBoxOperation::StandardizePaper {
                paper: PaperSize::A4,
                orientation: PaperOrientation::Auto,
                pages: vec![],
            },
            &StubRenderer,
            &StubInspector(1),
            &control,
            &mut |_| {},
        )?;
        assert_eq!(a4_report.modified_pages, 1);
        let a4_json_out = tool.invoke(
            &[
                "--json=2".into(),
                "--json-stream-data=none".into(),
                "--json-key=pages".into(),
                "--json-key=qpdf".into(),
                a4.as_os_str().to_owned(),
            ],
            JSON_LIMIT,
            &control,
        )?;
        let a4_json: Value = serde_json::from_slice(&a4_json_out.stdout)?;
        let a4_ref = a4_json["pages"][0]["object"].as_str().ok_or("a4 ref")?;
        let media_box = a4_json["qpdf"][1][format!("obj:{a4_ref}")]["value"]["/MediaBox"]
            .as_array()
            .ok_or("missing MediaBox")?;
        let mw = media_box[2].as_f64().ok_or("w")?;
        let mh = media_box[3].as_f64().ok_or("h")?;
        assert!((mw - 595.28).abs() < 0.05);
        assert!((mh - 841.89).abs() < 0.05);

        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }
}
