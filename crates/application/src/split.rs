//! Bounded split plans and all-or-nothing batch orchestration.
use crate::{PdfInspector, SourceDocument, export::*};
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum SplitError {
    #[error("Masukkan jumlah halaman per file antara 1 dan jumlah halaman workspace.")]
    Count,
    #[error(
        "Gunakan rentang naik seperti 1-3; 4-8; 9-12. Koma menggabungkan halaman dalam satu file. Halaman tidak boleh berulang atau melewati batas workspace."
    )]
    Ranges,
    #[error(
        "Gunakan nama folder baru, maksimal 100 karakter, tanpa tanda path atau karakter khusus Windows."
    )]
    Folder,
    #[error(
        "Folder hasil sudah ada. Pilih nama folder baru; folder yang ada tidak digabung atau ditimpa."
    )]
    Exists,
    #[error("Folder hasil belum diterbitkan. Periksa izin, ruang disk, dan folder induk.")]
    Output,
    #[error(transparent)]
    Export(#[from] ExportError),
}
pub enum SplitSpec<'a> {
    Every(usize),
    Ranges(&'a str),
}
#[derive(Clone)]
pub struct SplitPlan {
    full: ExportPlan,
    parts: Vec<ExportPlan>,
}
fn split_groups(count: usize, spec: SplitSpec<'_>) -> Result<Vec<Vec<usize>>, SplitError> {
    if count == 0 || count > 1024 {
        return Err(ExportError::ResourceLimit.into());
    }
    let groups: Vec<Vec<usize>> = match spec {
        SplitSpec::Every(n) => {
            if n == 0 || n > count {
                return Err(SplitError::Count);
            }
            (0..count)
                .collect::<Vec<_>>()
                .chunks(n)
                .map(<[usize]>::to_vec)
                .collect()
        }
        SplitSpec::Ranges(text) => {
            if text.len() > 16384 || text.trim().is_empty() {
                return Err(SplitError::Ranges);
            }
            let mut groups = Vec::new();
            let mut previous = 0;
            for group in text.split(';') {
                let mut pages = Vec::new();
                for range in group.split(',') {
                    let mut pair = range.trim().split('-');
                    let parse = |s: &str| -> Result<usize, SplitError> {
                        let s = s.trim();
                        if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
                            return Err(SplitError::Ranges);
                        }
                        s.parse().map_err(|_| SplitError::Ranges)
                    };
                    let start = parse(pair.next().ok_or(SplitError::Ranges)?)?;
                    let end = match pair.next() {
                        Some(s) => parse(s)?,
                        None => start,
                    };
                    if pair.next().is_some() || start <= previous || end < start || end > count {
                        return Err(SplitError::Ranges);
                    }
                    pages.extend((start..=end).map(|i| i - 1));
                    previous = end;
                }
                groups.push(pages);
            }
            groups
        }
    };
    Ok(groups)
}
impl SplitPlan {
    pub fn preview(count: usize, spec: SplitSpec<'_>) -> Result<(usize, usize), SplitError> {
        let groups = split_groups(count, spec)?;
        Ok((groups.len(), groups.iter().map(Vec::len).sum()))
    }

    pub fn new(full: ExportPlan, spec: SplitSpec<'_>) -> Result<Self, SplitError> {
        let count = full.pages().len();
        let groups = split_groups(count, spec)?;
        let parts = groups
            .iter()
            .map(|g| full.subset(g))
            .collect::<Result<_, _>>()?;
        let used = groups.into_iter().flatten().collect::<Vec<_>>();
        let full = full.subset(&used)?;
        Ok(Self { full, parts })
    }
    pub fn parts(&self) -> &[ExportPlan] {
        &self.parts
    }
    /// Union of referenced pages for shared snapshots; omitted sources are excluded.
    pub fn full(&self) -> &ExportPlan {
        &self.full
    }
    pub fn page_count(&self) -> usize {
        self.parts.iter().map(|p| p.pages().len()).sum()
    }
    pub fn filename(index: usize) -> String {
        format!("bagian-{:04}.pdf", index + 1)
    }
}
pub fn validate_folder(name: &str) -> Result<(), SplitError> {
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (1..=9).any(|n| stem == format!("COM{n}") || stem == format!("LPT{n}"));
    if name.is_empty()
        || name.chars().count() > 100
        || name.ends_with(['.', ' '])
        || reserved
        || name
            .chars()
            .any(|c| c.is_control() || "/\\:*?\"<>|".contains(c))
    {
        return Err(SplitError::Folder);
    }
    Ok(())
}
pub trait SplitTransaction: Send + Sync {
    fn sources(&self) -> &[SourceDocument];
    fn output_directory(&self) -> &Path;
    fn work_directory(&self) -> &Path;
    fn verify_sources(&self) -> Result<(), ExportError>;
    /// Flush all outputs and publish a new directory without replacing anything.
    fn commit(self, control: &ExportControl) -> Result<PathBuf, SplitError>;
}
pub trait SplitOutput {
    type Transaction: SplitTransaction;
    fn stage(
        &self,
        plan: &ExportPlan,
        destination: &Path,
        control: &ExportControl,
    ) -> Result<Self::Transaction, SplitError>;
}
pub struct PreparedSplit<T> {
    plan: SplitPlan,
    transaction: T,
    reports: Vec<PreflightReport>,
    pub report: PreflightReport,
}
pub struct SplitReceipt {
    pub destination: PathBuf,
    pub files: usize,
    pub pages: usize,
}

fn part_sources(plan: &ExportPlan, sources: &[SourceDocument]) -> Vec<SourceDocument> {
    sources
        .iter()
        .filter(|s| plan.sources().iter().any(|p| p.id == s.id))
        .cloned()
        .collect()
}
pub fn prepare_split<O: SplitOutput>(
    plan: SplitPlan,
    destination: &Path,
    output: &O,
    preflight: &impl ExportPreflight,
    control: &ExportControl,
) -> Result<PreparedSplit<O::Transaction>, SplitError> {
    control.checkpoint()?;
    control.set_phase(ExportPhase::Snapshot);
    let transaction = output.stage(plan.full(), destination, control)?;
    let mut reports = Vec::new();
    let mut report = PreflightReport {
        metadata_loss: false,
        bookmarks_loss: false,
        tags_loss: false,
        policy_version: EXPORT_POLICY_VERSION,
    };
    control.set_phase(ExportPhase::Preflight);
    for part in plan.parts() {
        control.checkpoint()?;
        let r =
            preflight.preflight_plan(part, &part_sources(part, transaction.sources()), control)?;
        if r.policy_version != EXPORT_POLICY_VERSION {
            return Err(ExportError::Inspection.into());
        }
        report.metadata_loss |= r.metadata_loss;
        report.bookmarks_loss |= r.bookmarks_loss;
        report.tags_loss |= r.tags_loss;
        reports.push(r);
    }
    transaction.verify_sources()?;
    control.checkpoint()?;
    Ok(PreparedSplit {
        plan,
        transaction,
        reports,
        report,
    })
}
struct Part<'a, T> {
    batch: &'a T,
    sources: Vec<SourceDocument>,
    path: PathBuf,
}
impl<T: SplitTransaction> OutputTransaction for Part<'_, T> {
    fn sources(&self) -> &[SourceDocument] {
        &self.sources
    }
    fn output(&self) -> &Path {
        &self.path
    }
    fn directory(&self) -> &Path {
        self.batch.work_directory()
    }
    fn verify_sources(&self) -> Result<(), ExportError> {
        self.batch.verify_sources()
    }
    // This is still inside the private batch directory. Publication is batch-only.
    fn commit(self, control: &ExportControl) -> Result<PathBuf, ExportError> {
        control.checkpoint()?;
        self.verify_sources()?;
        Ok(self.path)
    }
}
pub fn finish_split<T: SplitTransaction>(
    prepared: PreparedSplit<T>,
    acknowledged: bool,
    exporter: &impl PdfExporter,
    inspector: &impl PdfInspector,
    control: &ExportControl,
    mut progress: impl FnMut(usize),
) -> Result<SplitReceipt, SplitError> {
    control.checkpoint()?;
    if prepared.report.requires_acknowledgement() && !acknowledged {
        return Err(ExportError::Acknowledgement.into());
    }
    let PreparedSplit {
        plan,
        transaction,
        reports,
        ..
    } = prepared;
    for (index, (part, report)) in plan.parts.iter().zip(reports).enumerate() {
        let output = transaction
            .output_directory()
            .join(SplitPlan::filename(index));
        finish_export(
            PreparedExport {
                plan: part.clone(),
                report,
                transaction: Part {
                    batch: &transaction,
                    sources: part_sources(part, transaction.sources()),
                    path: output,
                },
            },
            acknowledged,
            exporter,
            inspector,
            control,
        )?;
        progress(index + 1);
    }
    transaction.verify_sources()?;
    control.set_phase(ExportPhase::Commit);
    let destination = transaction.commit(control)?;
    control.set_phase(ExportPhase::Finished);
    Ok(SplitReceipt {
        destination,
        files: plan.parts.len(),
        pages: plan.page_count(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ExportSupport, PageGeometry, SourceFingerprint, SourceId, Workspace};
    fn plan() -> Result<ExportPlan, ExportError> {
        let w = Workspace::new(SourceDocument {
            id: SourceId(1),
            path: "source.pdf".into(),
            fingerprint: SourceFingerprint {
                bytes: 1,
                modified_ns: 1,
            },
            export_support: ExportSupport::NotPreflighted,
            geometries: vec![
                PageGeometry {
                    width_points: 100.,
                    height_points: 200.,
                    quarter_turns: 0
                };
                23
            ],
        })
        .map_err(|_| ExportError::InvalidPlan)?;
        ExportPlan::capture(&w, 7, ExportScope::All)
    }
    #[test]
    fn every_n_and_manual_groups_follow_current_plan() -> Result<(), SplitError> {
        let p = SplitPlan::new(plan()?, SplitSpec::Every(10))?;
        assert_eq!(
            p.parts()
                .iter()
                .map(|p| p.pages().len())
                .collect::<Vec<_>>(),
            vec![10, 10, 3]
        );
        assert_eq!(p.parts()[2].pages()[0].entry.source_page_index, 20);
        assert_eq!(SplitPlan::filename(0), "bagian-0001.pdf");
        let p = SplitPlan::new(plan()?, SplitSpec::Ranges("1-3,5; 9-12; 23"))?;
        assert_eq!(p.page_count(), 9);
        assert_eq!(p.parts()[0].pages()[3].entry.source_page_index, 4);
        assert!(p.parts().iter().all(|p| !p.is_whole_source_extract()));
        Ok(())
    }
    #[test]
    fn bad_ranges_counts_and_folder_paths_are_rejected() -> Result<(), SplitError> {
        for text in [
            "",
            "0",
            "24",
            "2-1",
            "1;1",
            "1-3;3-5",
            "3;2",
            "1;;2",
            "1,",
            "1--2",
            "+1",
            "1-999999999999999999999",
        ] {
            assert!(
                SplitPlan::new(plan()?, SplitSpec::Ranges(text)).is_err(),
                "{text}"
            );
        }
        for n in [0, 24, usize::MAX] {
            assert!(SplitPlan::new(plan()?, SplitSpec::Every(n)).is_err());
        }
        for name in [
            "", ".", "..", "../x", "a/b", "a\\b", "NUL.pdf", "COM1", "end.", "end ", "a:b",
        ] {
            assert!(validate_folder(name).is_err(), "{name}");
        }
        assert!(validate_folder("hasil split_日本").is_ok());
        Ok(())
    }
}
