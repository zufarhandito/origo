//! Production adapter: snapshots and staging are supplied by the application.
use pdf_application::{
    SourceDocument,
    export::{
        ExportControl, ExportError, ExportPlan, ExportPreflight, PdfExporter, PreflightReport,
        resolve_source,
    },
};
use pdf_platform::{fingerprint, run};
use serde_json::{Value, json};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::Command,
};
const JSON_LIMIT: usize = 16 * 1024 * 1024;
#[derive(Clone)]
pub struct QpdfExporter {
    executable: PathBuf,
}
impl QpdfExporter {
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
    fn version(&self, control: &ExportControl) -> Result<(), ExportError> {
        let out = self.invoke(&["--version".into()], 65536, control)?;
        if out.code != Some(0)
            || String::from_utf8_lossy(&out.stdout).lines().next() != Some("qpdf version 12.4.2")
        {
            return Err(ExportError::Dependency);
        }
        Ok(())
    }
}
impl ExportPreflight for QpdfExporter {
    fn preflight(
        &self,
        sources: &[SourceDocument],
        control: &ExportControl,
    ) -> Result<PreflightReport, ExportError> {
        self.version(control)?;
        let mut metadata_loss = false;
        let mut bookmarks_loss = false;
        for source in sources {
            control.checkpoint()?;
            if fingerprint(&source.path)? != source.fingerprint {
                return Err(ExportError::SourceChanged);
            }
            // Encryption probe uses special exit codes: 0 encrypted, 2 unencrypted.
            let encryption = self.invoke(
                &["--is-encrypted".into(), source.path.as_os_str().to_owned()],
                65536,
                control,
            )?;
            if encryption.code == Some(0) {
                return Err(ExportError::Unsupported("PDF terenkripsi belum didukung."));
            }
            if encryption.code != Some(2) {
                return Err(ExportError::Structure);
            }
            self.check(&source.path, control)?;
            let out = self.invoke(
                &[
                    "--json=2".into(),
                    "--json-stream-data=none".into(),
                    "--json-key=encrypt".into(),
                    "--json-key=acroform".into(),
                    "--json-key=outlines".into(),
                    "--json-key=pages".into(),
                    "--json-key=qpdf".into(),
                    source.path.as_os_str().to_owned(),
                ],
                JSON_LIMIT,
                control,
            )?;
            if out.code != Some(0) {
                return Err(ExportError::Structure);
            }
            let parsed: Value =
                serde_json::from_slice(&out.stdout).map_err(|_| ExportError::Inspection)?;
            let report = crate::preflight::inspect_json(&parsed, source.geometries.len())?;
            metadata_loss |= report.metadata_loss;
            bookmarks_loss |= report.bookmarks_loss;
            if fingerprint(&source.path)? != source.fingerprint {
                return Err(ExportError::SourceChanged);
            }
        }
        Ok(PreflightReport {
            metadata_loss,
            bookmarks_loss,
            policy_version: pdf_application::export::EXPORT_POLICY_VERSION,
        })
    }
}
fn ranges(indices: &[usize]) -> String {
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
fn job(plan: &ExportPlan, sources: &[SourceDocument], output: &Path) -> Result<Value, ExportError> {
    let mut groups: Vec<(pdf_application::SourceId, Vec<usize>)> = Vec::new();
    let mut rotations = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
    for (index, page) in plan.pages().iter().enumerate() {
        rotations[usize::from(page.entry.rotation_delta)].push(index + 1);
        if let Some((id, indices)) = groups.last_mut()
            && *id == page.entry.source_id
        {
            indices.push(usize::from(page.entry.source_page_index) + 1);
        } else {
            groups.push((
                page.entry.source_id,
                vec![usize::from(page.entry.source_page_index) + 1],
            ));
        }
    }
    let mut pages = Vec::new();
    let mut estimated = 1024;
    for (id, indices) in groups {
        let source = resolve_source(sources, id)?;
        let path = source.path.to_str().ok_or(ExportError::InvalidPlan)?;
        estimated += path.len().saturating_mul(6) + indices.len() * 24;
        if estimated > JSON_LIMIT {
            return Err(ExportError::ResourceLimit);
        }
        pages.push(json!({"file":path,"range":ranges(&indices)}));
    }
    let rotate: Vec<_> = (1..4)
        .filter(|i| !rotations[*i].is_empty())
        .map(|i| format!("+{}:{}", i * 90, ranges(&rotations[i])))
        .collect();
    Ok(
        json!({"empty":"","outputFile":output.to_str().ok_or(ExportError::InvalidPlan)?,"pages":pages,"rotate":rotate}),
    )
}
impl PdfExporter for QpdfExporter {
    fn write(
        &self,
        plan: &ExportPlan,
        sources: &[SourceDocument],
        output: &Path,
        work: &Path,
        control: &ExportControl,
    ) -> Result<(), ExportError> {
        self.version(control)?;
        let bytes = serde_json::to_vec(&job(plan, sources, output)?)
            .map_err(|_| ExportError::InvalidPlan)?;
        if bytes.len() > JSON_LIMIT {
            return Err(ExportError::ResourceLimit);
        }
        let job_path = work.join("job.json");
        std::fs::write(&job_path, bytes).map_err(|_| ExportError::Io)?;
        let mut argument = OsString::from("--job-json-file=");
        argument.push(job_path);
        if self.invoke(&[argument], 65536, control)?.code != Some(0) {
            return Err(ExportError::Engine);
        }
        Ok(())
    }
    fn check(&self, path: &Path, control: &ExportControl) -> Result<(), ExportError> {
        if self
            .invoke(
                &["--check".into(), path.as_os_str().to_owned()],
                65536,
                control,
            )?
            .code
            != Some(0)
        {
            return Err(ExportError::Structure);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn version_mismatch_and_missing_sidecar_are_refused() -> Result<(), Box<dyn std::error::Error>>
    {
        let exporter = QpdfExporter {
            executable: std::env::current_exe()?,
        };
        assert!(matches!(
            exporter.version(&ExportControl::default()),
            Err(ExportError::Dependency)
        ));
        let exporter = QpdfExporter {
            executable: PathBuf::from("missing-sidecar-that-does-not-exist.exe"),
        };
        assert!(matches!(
            exporter.version(&ExportControl::default()),
            Err(ExportError::Dependency)
        ));
        Ok(())
    }
    #[test]
    fn range_compilation_never_sorts_or_deduplicates() {
        assert_eq!(ranges(&[2, 1, 4, 5, 6, 3]), "2,1,4-6,3");
    }
}
