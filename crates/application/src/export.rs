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
    #[error("Struktur inspeksi PDF tidak dikenali; ekspor diblokir.")]
    Inspection,
    #[error("Bookmark atau metadata dapat hilang; diperlukan persetujuan untuk ekspor ini.")]
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
}
#[derive(Clone)]
pub struct ExportPage {
    pub entry: PageEntry,
    pub geometry: PageGeometry,
}
#[derive(Clone)]
pub struct ExportPlan {
    pub instance: u64,
    pub revision: u64,
    pub scope: ExportScope,
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
    pub policy_version: u32,
}
pub const EXPORT_POLICY_VERSION: u32 = 2;
impl PreflightReport {
    pub fn requires_acknowledgement(&self) -> bool {
        self.metadata_loss || self.bookmarks_loss
    }
    pub fn confirmation_title(&self) -> &'static str {
        if self.bookmarks_loss {
            "Simpan PDF tanpa bookmark?"
        } else {
            "Metadata dokumen dapat tidak terbawa"
        }
    }
    pub fn confirmation_detail(&self) -> &'static str {
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
        if self.bookmarks_loss {
            "Simpan tanpa bookmark"
        } else {
            "Lanjutkan"
        }
    }
}
pub trait ExportPreflight {
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
    pub plan: ExportPlan,
    pub report: PreflightReport,
    transaction: T,
}
pub struct ExportReceipt {
    pub instance: u64,
    pub revision: u64,
    pub scope: ExportScope,
    pub destination: PathBuf,
    pub page_count: usize,
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
    let report = preflight.preflight(transaction.sources(), control)?;
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
        .inspect(transaction.output())
        .map_err(|_| ExportError::Validation)?;
    control.checkpoint()?;
    if info.pages.len() != plan.pages.len()
        || info.pages.iter().zip(&plan.pages).any(|(actual, p)| {
            actual.quarter_turns != p.geometry.quarter_turns
                || (actual.width_points - p.geometry.width_points).abs() > 0.1
                || (actual.height_points - p.geometry.height_points).abs() > 0.1
        })
    {
        return Err(ExportError::Validation);
    }
    transaction.verify_sources()?;
    control.set_phase(ExportPhase::Commit);
    let destination = transaction.commit(control)?;
    control.set_phase(ExportPhase::Finished);
    Ok(ExportReceipt {
        instance: plan.instance,
        revision: plan.revision,
        scope: plan.scope,
        destination,
        page_count: plan.pages.len(),
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
    struct Transaction {
        sources: Vec<SourceDocument>,
        commits: Arc<AtomicUsize>,
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
