//! Immutable export snapshots and technology-independent output orchestration.
use crate::{PageGeometry, PdfInspector, SourceDocument, Workspace};
use pdf_domain::{PageEntry, SourceId};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering},
    },
    time::Duration,
};

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("Cetak Windows gagal: {0}")]
    PrintBackend(String),
    #[error("Tidak ada halaman untuk diekspor.")]
    Empty,
    #[error("Rencana ekspor tidak valid.")]
    InvalidPlan,
    #[error("File sumber berubah atau tidak tersedia. Buka ulang PDF.")]
    SourceChanged,
    #[error("Tujuan sudah ada. Pilih nama PDF baru; file yang ada tidak ditimpa.")]
    DestinationExists,
    #[error("Tujuan harus berupa file PDF baru di folder yang dapat ditulis.")]
    Destination,
    #[error("Tidak dapat menulis atau membaca berkas ekspor.")]
    Io,
    #[error("QPDF 12.4.2 tidak tersedia atau versinya tidak sesuai.")]
    Dependency,
    #[error("PDF memiliki kesalahan atau peringatan struktur; ekspor diblokir.")]
    Structure,
    #[error("Ekspor diblokir: {0}")]
    Unsupported(&'static str),
    #[error(
        "Ekspor diblokir: atribut catalog {0} belum didukung. Dokumen perlu diperiksa sebelum dapat diekspor."
    )]
    CatalogAttribute(String),
    #[error("Struktur inspeksi PDF tidak dikenali; ekspor diblokir.")]
    Inspection,
    #[error(
        "Tag aksesibilitas, bookmark atau metadata dapat hilang; diperlukan persetujuan untuk ekspor ini."
    )]
    Acknowledgement,
    #[error("Ekspor melebihi batas sumber daya.")]
    ResourceLimit,
    #[error("Proses ekspor gagal. Tidak ada file pengguna yang ditimpa.")]
    Engine,
    #[error("Hasil ekspor tidak sesuai rencana; file tidak diterbitkan.")]
    Validation,
    #[error("Ekspor dibatalkan.")]
    Cancelled,
    #[error("Proses QPDF melewati batas waktu.")]
    Timeout,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportScope {
    All,
    Selected,
    /// All pages as a derived optimized PDF; never advances the clean baseline.
    Optimized,
}
#[derive(Clone)]
pub struct ExportPage {
    pub annotations: Vec<pdf_domain::annotation::Annotation>,
    pub entry: PageEntry,
    pub geometry: PageGeometry,
}
#[derive(Clone)]
pub struct ExportPlan {
    instance: u64,
    revision: u64,
    scope: ExportScope,
    pages: Vec<ExportPage>,
    sources: Vec<SourceDocument>,
}
impl ExportPlan {
    pub fn capture(
        workspace: &Workspace,
        instance: u64,
        scope: ExportScope,
    ) -> Result<Self, ExportError> {
        let mut pages = Vec::new();
        let mut sources = Vec::new();
        for (index, p) in workspace.pages().iter().enumerate() {
            if scope == ExportScope::Selected && !workspace.selected().contains(&p.id) {
                continue;
            }
            let source = workspace
                .resolve(p.source_id)
                .ok_or(ExportError::InvalidPlan)?;
            if p.rotation_delta > 3 || usize::from(p.source_page_index) >= source.geometries.len() {
                return Err(ExportError::InvalidPlan);
            }
            if !sources.iter().any(|s: &SourceDocument| s.id == source.id) {
                sources.push(source.clone());
            }
            pages.push(ExportPage {
                annotations: workspace.annotations(p.id).to_vec(),
                entry: *p,
                geometry: workspace
                    .geometry(index)
                    .map_err(|_| ExportError::InvalidPlan)?,
            });
        }
        if pages.len() > 1024 {
            return Err(ExportError::ResourceLimit);
        }
        if pages.is_empty() {
            return Err(ExportError::Empty);
        }
        Ok(Self {
            instance,
            revision: workspace.revision(),
            scope,
            pages,
            sources,
        })
    }
    pub fn pages(&self) -> &[ExportPage] {
        &self.pages
    }
    pub fn sources(&self) -> &[SourceDocument] {
        &self.sources
    }
    pub fn scope(&self) -> ExportScope {
        self.scope
    }
    pub fn instance(&self) -> u64 {
        self.instance
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub(crate) fn subset(&self, indices: &[usize]) -> Result<Self, ExportError> {
        if indices.is_empty() {
            return Err(ExportError::Empty);
        }
        let pages: Vec<_> = indices
            .iter()
            .map(|i| self.pages.get(*i).cloned().ok_or(ExportError::InvalidPlan))
            .collect::<Result<_, _>>()?;
        let sources = self
            .sources
            .iter()
            .filter(|s| pages.iter().any(|p| p.entry.source_id == s.id))
            .cloned()
            .collect();
        Ok(Self {
            instance: self.instance,
            revision: self.revision,
            scope: ExportScope::Selected,
            pages,
            sources,
        })
    }
    /// Extracting every page of one source, in original order and orientation,
    /// can preserve that PDF byte-for-byte instead of rebuilding its catalog.
    pub fn is_whole_source_extract(&self) -> bool {
        self.scope == ExportScope::Selected
            && self.sources.len() == 1
            && self.pages.len() == self.sources[0].geometries.len()
            && self.pages.iter().enumerate().all(|(index, page)| {
                page.entry.source_id == self.sources[0].id
                    && usize::from(page.entry.source_page_index) == index
                    && page.entry.rotation_delta == 0
                    && page.annotations
                        == self.sources[0]
                            .annotations
                            .get(index)
                            .cloned()
                            .unwrap_or_default()
            })
    }
    pub fn is_single_source_extract(&self) -> bool {
        self.scope == ExportScope::Selected && self.sources.len() == 1
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ExportPhase {
    Choosing = 0,
    Snapshot = 1,
    Preflight = 2,
    Confirmation = 3,
    Writing = 4,
    Validation = 5,
    Commit = 6,
    Finished = 7,
}
impl ExportPhase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Choosing => "Pilih tujuan ekspor",
            Self::Snapshot => "Menyiapkan salinan sumber",
            Self::Preflight => "Memeriksa keamanan ekspor",
            Self::Confirmation => "Menunggu persetujuan ekspor",
            Self::Writing => "Menyusun PDF",
            Self::Validation => "Memvalidasi hasil PDF",
            Self::Commit => "Menyimpan PDF baru",
            Self::Finished => "Ekspor selesai",
        }
    }
}
#[derive(Clone)]
pub struct ExportControl {
    cancelled: Arc<AtomicBool>,
    phase: Arc<AtomicU8>,
    spent: Arc<AtomicU64>,
    budget: Duration,
}
impl Default for ExportControl {
    fn default() -> Self {
        Self::with_budget(Duration::from_secs(120))
    }
}
impl ExportControl {
    pub fn with_budget(budget: Duration) -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
            phase: Arc::new(AtomicU8::new(0)),
            spent: Arc::new(AtomicU64::new(0)),
            budget,
        }
    }
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }
    pub fn checkpoint(&self) -> Result<(), ExportError> {
        if self.cancelled.load(Ordering::SeqCst) {
            Err(ExportError::Cancelled)
        } else {
            Ok(())
        }
    }
    pub fn set_phase(&self, p: ExportPhase) {
        self.phase.store(p as u8, Ordering::SeqCst);
    }
    pub fn phase(&self) -> ExportPhase {
        match self.phase.load(Ordering::SeqCst) {
            1 => ExportPhase::Snapshot,
            2 => ExportPhase::Preflight,
            3 => ExportPhase::Confirmation,
            4 => ExportPhase::Writing,
            5 => ExportPhase::Validation,
            6 => ExportPhase::Commit,
            7 => ExportPhase::Finished,
            _ => ExportPhase::Choosing,
        }
    }
    pub fn remaining(&self) -> Result<Duration, ExportError> {
        self.checkpoint()?;
        self.budget
            .checked_sub(Duration::from_millis(self.spent.load(Ordering::SeqCst)))
            .filter(|v| !v.is_zero())
            .ok_or(ExportError::Timeout)
    }
    pub fn charge(&self, duration: Duration) {
        self.spent.fetch_add(
            duration.as_millis().min(u128::from(u64::MAX)) as u64,
            Ordering::SeqCst,
        );
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreflightReport {
    pub metadata_loss: bool,
    pub bookmarks_loss: bool,
    pub tags_loss: bool,
    pub policy_version: u32,
}
pub const EXPORT_POLICY_VERSION: u32 = 6;
impl PreflightReport {
    pub fn requires_acknowledgement(&self) -> bool {
        self.metadata_loss || self.bookmarks_loss || self.tags_loss
    }
    pub fn confirmation_title(&self) -> &'static str {
        if self.tags_loss {
            return "Simpan PDF tanpa tag aksesibilitas?";
        }
        if self.bookmarks_loss {
            "Simpan PDF tanpa bookmark?"
        } else {
            "Metadata dokumen dapat tidak terbawa"
        }
    }
    pub fn confirmation_detail(&self) -> &'static str {
        if self.tags_loss {
            return match (self.bookmarks_loss, self.metadata_loss) {
                (false, false) => {
                    "Tag aksesibilitas dan informasi urutan baca tidak disertakan pada hasil ekstrak. Pembaca layar dapat kehilangan struktur navigasi. Tampilan halaman dan anotasi tinta tetap dipertahankan. File sumber tetap utuh."
                }
                (false, true) => {
                    "Tag aksesibilitas dan informasi urutan baca tidak disertakan pada hasil ekstrak. Pembaca layar dapat kehilangan struktur navigasi. Metadata dan pengaturan tampilan juga dapat hilang. Tampilan halaman dan anotasi tinta tetap dipertahankan. File sumber tetap utuh."
                }
                (true, false) => {
                    "Tag aksesibilitas, informasi urutan baca, dan bookmark tidak disertakan pada hasil ekstrak. Pembaca layar dapat kehilangan struktur navigasi. Tampilan halaman dan anotasi tinta tetap dipertahankan. File sumber tetap utuh."
                }
                (true, true) => {
                    "Tag aksesibilitas, informasi urutan baca, dan bookmark tidak disertakan pada hasil ekstrak. Pembaca layar dapat kehilangan struktur navigasi. Metadata dan pengaturan tampilan juga dapat hilang. Tampilan halaman dan anotasi tinta tetap dipertahankan. File sumber tetap utuh."
                }
            };
        }
        match (self.bookmarks_loss, self.metadata_loss) {
            (true, true) => {
                "Bookmark/daftar isi navigasi tidak disertakan pada PDF hasil ekspor. Metadata seperti judul, penulis, atau informasi XML juga dapat hilang. Halaman diekspor sesuai susunan; file sumber tetap utuh."
            }
            (true, false) => {
                "Bookmark/daftar isi navigasi tidak disertakan pada PDF hasil ekspor. Halaman diekspor sesuai susunan; file sumber tetap utuh."
            }
            _ => {
                "Ekspor ini menyusun PDF baru. Metadata seperti judul, penulis, atau informasi XML dapat hilang. Lanjutkan ekspor snapshot ini?"
            }
        }
    }
    pub fn confirmation_action(&self) -> &'static str {
        if self.tags_loss {
            return "Simpan tanpa tag";
        }
        if self.bookmarks_loss {
            "Simpan tanpa bookmark"
        } else {
            "Lanjutkan"
        }
    }
}
pub trait ExportPreflight {
    fn preflight_plan(
        &self,
        _plan: &ExportPlan,
        sources: &[SourceDocument],
        control: &ExportControl,
    ) -> Result<PreflightReport, ExportError> {
        self.preflight(sources, control)
    }
    fn preflight(
        &self,
        sources: &[SourceDocument],
        control: &ExportControl,
    ) -> Result<PreflightReport, ExportError>;
}
pub trait PdfExporter {
    fn write(
        &self,
        plan: &ExportPlan,
        sources: &[SourceDocument],
        path: &Path,
        work: &Path,
        control: &ExportControl,
    ) -> Result<(), ExportError>;
    fn check(&self, path: &Path, control: &ExportControl) -> Result<(), ExportError>;
}
pub trait OutputTransaction: Send {
    fn sources(&self) -> &[SourceDocument];
    fn output(&self) -> &Path;
    fn directory(&self) -> &Path;
    fn verify_sources(&self) -> Result<(), ExportError>;
    /// Byte size of an artifact owned by this transaction, before publication.
    fn artifact_size(&self, _path: &Path) -> Result<u64, ExportError> {
        Err(ExportError::Io)
    }
    fn commit(self, control: &ExportControl) -> Result<PathBuf, ExportError>;
}
pub trait AtomicOutput {
    type Transaction: OutputTransaction;
    fn stage(
        &self,
        plan: &ExportPlan,
        destination: &Path,
        control: &ExportControl,
    ) -> Result<Self::Transaction, ExportError>;
}
pub struct PreparedExport<T> {
    pub(crate) plan: ExportPlan,
    pub report: PreflightReport,
    pub(crate) transaction: T,
}
pub const OPTIMIZE_BASELINE_FILE: &str = "before-optimize.pdf";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OptimizationSizes {
    pub baseline_bytes: u64,
    pub output_bytes: u64,
}
pub struct ExportReceipt {
    pub instance: u64,
    pub revision: u64,
    pub scope: ExportScope,
    pub destination: PathBuf,
    pub page_count: usize,
    pub optimization: Option<OptimizationSizes>,
}
impl ExportReceipt {
    pub fn apply(&self, instance: u64, workspace: &mut Workspace) -> bool {
        self.scope == ExportScope::All
            && self.instance == instance
            && workspace.mark_exported(self.revision)
    }
}
pub fn prepare_export<O: AtomicOutput>(
    plan: ExportPlan,
    destination: &Path,
    output: &O,
    preflight: &impl ExportPreflight,
    control: &ExportControl,
) -> Result<PreparedExport<O::Transaction>, ExportError> {
    control.checkpoint()?;
    control.set_phase(ExportPhase::Snapshot);
    let transaction = output.stage(&plan, destination, control)?;
    control.set_phase(ExportPhase::Preflight);
    let report = preflight.preflight_plan(&plan, transaction.sources(), control)?;
    control.checkpoint()?;
    Ok(PreparedExport {
        plan,
        report,
        transaction,
    })
}
pub fn finish_export<T: OutputTransaction>(
    prepared: PreparedExport<T>,
    acknowledged: bool,
    exporter: &impl PdfExporter,
    inspector: &impl PdfInspector,
    control: &ExportControl,
) -> Result<ExportReceipt, ExportError> {
    control.checkpoint()?;
    if prepared.report.policy_version != EXPORT_POLICY_VERSION {
        return Err(ExportError::Inspection);
    }
    if prepared.report.requires_acknowledgement() && !acknowledged {
        return Err(ExportError::Acknowledgement);
    }
    let PreparedExport {
        plan, transaction, ..
    } = prepared;
    control.set_phase(ExportPhase::Writing);
    exporter.write(
        &plan,
        transaction.sources(),
        transaction.output(),
        transaction.directory(),
        control,
    )?;
    control.checkpoint()?;
    control.set_phase(ExportPhase::Validation);
    exporter.check(transaction.output(), control)?;
    let info = inspector
        .inspect_output(transaction.output())
        .map_err(|_| ExportError::Validation)?;
    control.checkpoint()?;
    if plan.pages.iter().enumerate().any(|(i, p)| {
        info.annotations.get(i).map(Vec::as_slice).unwrap_or(&[]) != p.annotations.as_slice()
    }) || info.pages.len() != plan.pages.len()
        || info.pages.iter().zip(&plan.pages).any(|(actual, p)| {
            actual.quarter_turns != p.geometry.quarter_turns
                || (actual.width_points - p.geometry.width_points).abs() > 0.1
                || (actual.height_points - p.geometry.height_points).abs() > 0.1
        })
    {
        return Err(ExportError::Validation);
    }
    transaction.verify_sources()?;
    let optimization = if plan.scope == ExportScope::Optimized {
        let baseline_bytes =
            transaction.artifact_size(&transaction.directory().join(OPTIMIZE_BASELINE_FILE))?;
        let output_bytes = transaction.artifact_size(transaction.output())?;
        if baseline_bytes == 0 || output_bytes == 0 {
            return Err(ExportError::Validation);
        }
        Some(OptimizationSizes {
            baseline_bytes,
            output_bytes,
        })
    } else {
        None
    };
    control.set_phase(ExportPhase::Commit);
    let destination = transaction.commit(control)?;
    control.set_phase(ExportPhase::Finished);
    Ok(ExportReceipt {
        instance: plan.instance,
        revision: plan.revision,
        scope: plan.scope,
        destination,
        page_count: plan.pages.len(),
        optimization,
    })
}
/// Resolve sources by identity, never by a workspace position or PageId serial.
pub fn resolve_source(
    sources: &[SourceDocument],
    id: SourceId,
) -> Result<&SourceDocument, ExportError> {
    sources
        .iter()
        .find(|s| s.id == id)
        .ok_or(ExportError::InvalidPlan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ExportSupport, InspectedDocument, RenderError, SourceFingerprint, SourceId};
    use pdf_domain::Command;
    use std::sync::atomic::AtomicUsize;
    fn workspace() -> Result<Workspace, Box<dyn std::error::Error>> {
        Ok(Workspace::new(SourceDocument {
            id: SourceId(1),
            path: "a.pdf".into(),
            fingerprint: SourceFingerprint {
                bytes: 1,
                modified_ns: 1,
            },
            geometries: vec![
                PageGeometry {
                    width_points: 300.,
                    height_points: 420.,
                    quarter_turns: 0
                };
                3
            ],
            export_support: ExportSupport::NotPreflighted,
            annotations: vec![],
        })?)
    }
    #[test]
    fn capture_tracks_source_mapping_selection_order_and_clean_revision()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut w = workspace()?;
        let ids = w.pages().iter().map(|p| p.id).collect::<Vec<_>>();
        w.select([ids[2], ids[0]]);
        w.execute(Command::Move {
            ids: vec![ids[2]],
            target: 0,
        })?;
        let plan = ExportPlan::capture(&w, 7, ExportScope::Selected)?;
        assert_eq!(
            plan.pages.iter().map(|p| p.entry.id).collect::<Vec<_>>(),
            vec![ids[2]]
        ); // Move selects moved pages.
        w.select([ids[0], ids[2]]);
        let plan = ExportPlan::capture(&w, 7, ExportScope::Selected)?;
        assert_eq!(
            plan.pages
                .iter()
                .map(|p| p.entry.source_page_index)
                .collect::<Vec<_>>(),
            vec![2, 0]
        );
        let receipt = ExportReceipt {
            instance: 7,
            revision: w.revision(),
            scope: ExportScope::Selected,
            destination: "out.pdf".into(),
            page_count: 2,
            optimization: None,
        };
        assert!(!receipt.apply(7, &mut w));
        assert!(w.is_dirty());
        let receipt = ExportReceipt {
            scope: ExportScope::All,
            ..receipt
        };
        assert!(!receipt.apply(8, &mut w));
        assert!(receipt.apply(7, &mut w));
        assert!(!w.is_dirty());
        assert!(w.undo());
        assert!(w.is_dirty());
        assert!(w.redo());
        assert!(!w.is_dirty());
        w.execute(Command::Rotate {
            ids: vec![ids[0]],
            turns: 1,
        })?;
        assert!(!receipt.apply(7, &mut w));
        assert!(w.is_dirty());
        let all = w.pages().iter().map(|p| p.id).collect();
        w.execute(Command::Delete { ids: all })?;
        assert!(matches!(
            ExportPlan::capture(&w, 7, ExportScope::All),
            Err(ExportError::Empty)
        ));
        Ok(())
    }
    #[test]
    fn source_copy_requires_complete_selected_original_order_and_orientation()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut w = workspace()?;
        let ids = w.pages().iter().map(|p| p.id).collect::<Vec<_>>();
        w.select(ids.clone());
        assert!(ExportPlan::capture(&w, 1, ExportScope::Selected)?.is_whole_source_extract());
        assert!(!ExportPlan::capture(&w, 1, ExportScope::All)?.is_whole_source_extract());
        w.select([ids[0]]);
        assert!(!ExportPlan::capture(&w, 1, ExportScope::Selected)?.is_whole_source_extract());
        w.select(ids.clone());
        w.execute(Command::Rotate {
            ids: vec![ids[0]],
            turns: 1,
        })?;
        assert!(!ExportPlan::capture(&w, 1, ExportScope::Selected)?.is_whole_source_extract());
        w.undo();
        w.execute(Command::Move {
            ids: vec![ids[2]],
            target: 0,
        })?;
        w.select(ids);
        assert!(!ExportPlan::capture(&w, 1, ExportScope::Selected)?.is_whole_source_extract());
        Ok(())
    }
    #[test]
    fn optimized_plan_captures_all_pages_but_cannot_mark_workspace_saved()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut w = workspace()?;
        let id = w.pages()[0].id;
        w.select([id]);
        w.execute(pdf_domain::Command::Rotate {
            ids: vec![id],
            turns: 1,
        })?;
        let plan = ExportPlan::capture(&w, 9, ExportScope::Optimized)?;
        assert_eq!(plan.pages().len(), w.pages().len());
        assert!(!plan.is_whole_source_extract() && !plan.is_single_source_extract());
        let receipt = ExportReceipt {
            instance: 9,
            revision: w.revision(),
            scope: ExportScope::Optimized,
            destination: "optimized.pdf".into(),
            page_count: plan.pages().len(),
            optimization: Some(OptimizationSizes {
                baseline_bytes: 100,
                output_bytes: 150,
            }),
        };
        assert!(!receipt.apply(9, &mut w));
        assert!(w.is_dirty());
        Ok(())
    }
    struct Transaction {
        sources: Vec<SourceDocument>,
        commits: Arc<AtomicUsize>,
        sizes: (u64, u64),
    }
    impl OutputTransaction for Transaction {
        fn sources(&self) -> &[SourceDocument] {
            &self.sources
        }
        fn output(&self) -> &Path {
            Path::new("staging.pdf")
        }
        fn directory(&self) -> &Path {
            Path::new("stage")
        }
        fn verify_sources(&self) -> Result<(), ExportError> {
            Ok(())
        }
        fn artifact_size(&self, path: &Path) -> Result<u64, ExportError> {
            if path == self.directory().join(OPTIMIZE_BASELINE_FILE) {
                Ok(self.sizes.0)
            } else if path == self.output() {
                Ok(self.sizes.1)
            } else {
                Err(ExportError::InvalidPlan)
            }
        }
        fn commit(self, control: &ExportControl) -> Result<PathBuf, ExportError> {
            control.checkpoint()?;
            self.commits.fetch_add(1, Ordering::SeqCst);
            Ok("out.pdf".into())
        }
    }
    struct Output(Arc<AtomicUsize>);
    impl AtomicOutput for Output {
        type Transaction = Transaction;
        fn stage(
            &self,
            plan: &ExportPlan,
            _: &Path,
            c: &ExportControl,
        ) -> Result<Transaction, ExportError> {
            c.checkpoint()?;
            Ok(Transaction {
                sources: plan.sources.clone(),
                commits: self.0.clone(),
                sizes: (100, 150),
            })
        }
    }
    struct Engine {
        metadata: bool,
        bookmarks: bool,
        fail: bool,
        cancel_on_write: bool,
    }
    impl ExportPreflight for Engine {
        fn preflight(
            &self,
            _: &[SourceDocument],
            _: &ExportControl,
        ) -> Result<PreflightReport, ExportError> {
            Ok(PreflightReport {
                metadata_loss: self.metadata,
                bookmarks_loss: self.bookmarks,
                tags_loss: false,
                policy_version: EXPORT_POLICY_VERSION,
            })
        }
    }
    impl PdfExporter for Engine {
        fn write(
            &self,
            _: &ExportPlan,
            _: &[SourceDocument],
            _: &Path,
            _: &Path,
            c: &ExportControl,
        ) -> Result<(), ExportError> {
            if self.cancel_on_write {
                c.cancel();
            }
            if self.fail {
                Err(ExportError::Engine)
            } else {
                Ok(())
            }
        }
        fn check(&self, _: &Path, _: &ExportControl) -> Result<(), ExportError> {
            Ok(())
        }
    }
    struct Inspector(bool);
    impl PdfInspector for Inspector {
        fn inspect(&self, path: &Path) -> Result<InspectedDocument, RenderError> {
            Ok(InspectedDocument {
                annotations: vec![],
                path: path.into(),
                fingerprint: SourceFingerprint {
                    bytes: 1,
                    modified_ns: 1,
                },
                pages: vec![
                    PageGeometry {
                        width_points: 300.,
                        height_points: 420.,
                        quarter_turns: 0
                    };
                    if self.0 { 3 } else { 2 }
                ],
            })
        }
    }
    #[test]
    fn optimization_reports_smaller_equal_larger_and_rejects_empty_artifacts()
    -> Result<(), Box<dyn std::error::Error>> {
        let plan = ExportPlan::capture(&workspace()?, 1, ExportScope::Optimized)?;
        let engine = Engine {
            metadata: false,
            bookmarks: false,
            fail: false,
            cancel_on_write: false,
        };
        for (before, after, success) in [
            (100, 50, true),
            (100, 100, true),
            (100, 150, true),
            (0, 50, false),
            (100, 0, false),
        ] {
            let commits = Arc::new(AtomicUsize::new(0));
            let control = ExportControl::default();
            let prepared = PreparedExport {
                plan: plan.clone(),
                report: engine.preflight(&[], &control)?,
                transaction: Transaction {
                    sources: plan.sources.clone(),
                    commits: commits.clone(),
                    sizes: (before, after),
                },
            };
            let result = finish_export(prepared, false, &engine, &Inspector(true), &control);
            assert_eq!(result.is_ok(), success);
            assert_eq!(commits.load(Ordering::SeqCst), usize::from(success));
            if let Ok(receipt) = result {
                assert_eq!(
                    receipt.optimization,
                    Some(OptimizationSizes {
                        baseline_bytes: before,
                        output_bytes: after
                    })
                );
            }
        }
        Ok(())
    }
    #[test]
    fn commit_is_after_acknowledgement_engine_validation_and_cancellation()
    -> Result<(), Box<dyn std::error::Error>> {
        let w = workspace()?;
        let plan = ExportPlan::capture(&w, 1, ExportScope::All)?;
        for (metadata, bookmarks, ack, fail, cancel, valid, success) in [
            (true, false, false, false, false, true, false),
            (true, false, true, false, false, true, true),
            (false, true, false, false, false, true, false),
            (false, true, true, false, false, true, true),
            (true, true, false, false, false, true, false),
            (true, true, true, false, false, true, true),
            (false, false, false, true, false, true, false),
            (false, false, false, false, true, true, false),
            (false, false, false, false, false, false, false),
            (false, false, false, false, false, true, true),
        ] {
            let count = Arc::new(AtomicUsize::new(0));
            let engine = Engine {
                metadata,
                bookmarks,
                fail,
                cancel_on_write: cancel,
            };
            let c = ExportControl::default();
            let prepared = prepare_export(
                plan.clone(),
                Path::new("out.pdf"),
                &Output(count.clone()),
                &engine,
                &c,
            )?;
            let result = finish_export(prepared, ack, &engine, &Inspector(valid), &c);
            assert_eq!(result.is_ok(), success);
            assert_eq!(count.load(Ordering::SeqCst), usize::from(success));
        }
        let c = ExportControl::default();
        c.cancel();
        assert!(matches!(
            prepare_export(
                plan,
                Path::new("out.pdf"),
                &Output(Arc::new(AtomicUsize::new(0))),
                &Engine {
                    metadata: false,
                    bookmarks: false,
                    fail: false,
                    cancel_on_write: false
                },
                &c
            ),
            Err(ExportError::Cancelled)
        ));
        Ok(())
    }
}
