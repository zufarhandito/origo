//! Production adapter: snapshots and staging are supplied by the application.
use pdf_application::{
    SourceDocument,
    export::{
        ExportControl, ExportError, ExportPlan, ExportPreflight, ExportScope,
        OPTIMIZE_BASELINE_FILE, PdfExporter, PreflightReport, resolve_source,
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
    fn preflight_plan(
        &self,
        plan: &ExportPlan,
        sources: &[SourceDocument],
        control: &ExportControl,
    ) -> Result<PreflightReport, ExportError> {
        self.preflight_mode(
            sources,
            control,
            plan.is_whole_source_extract(),
            plan.is_single_source_extract(),
        )
    }
    fn preflight(
        &self,
        sources: &[SourceDocument],
        control: &ExportControl,
    ) -> Result<PreflightReport, ExportError> {
        self.preflight_mode(sources, control, false, false)
    }
}
impl QpdfExporter {
    fn preflight_mode(
        &self,
        sources: &[SourceDocument],
        control: &ExportControl,
        source_copy: bool,
        tagged_extract: bool,
    ) -> Result<PreflightReport, ExportError> {
        self.version(control)?;
        let mut metadata_loss = false;
        let mut bookmarks_loss = false;
        let mut tags_loss = false;
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
            let report = if source_copy {
                crate::preflight::inspect_source_copy(&parsed, source.geometries.len())?
            } else if tagged_extract {
                crate::preflight::inspect_extract(&parsed, source.geometries.len())?
            } else {
                crate::preflight::inspect_json(&parsed, source.geometries.len())?
            };
            metadata_loss |= report.metadata_loss;
            bookmarks_loss |= report.bookmarks_loss;
            tags_loss |= report.tags_loss;
            if fingerprint(&source.path)? != source.fingerprint {
                return Err(ExportError::SourceChanged);
            }
        }
        Ok(PreflightReport {
            metadata_loss,
            bookmarks_loss,
            tags_loss,
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
    let mut job = json!({"empty":"","outputFile":output.to_str().ok_or(ExportError::InvalidPlan)?,"pages":pages,"rotate":rotate});
    if plan.is_single_source_extract() {
        job["removeStructure"] = json!("");
    }
    Ok(job)
}
fn optimization_job(input: &Path, output: &Path) -> Result<Value, ExportError> {
    Ok(json!({
        "inputFile": input.to_str().ok_or(ExportError::InvalidPlan)?,
        "outputFile": output.to_str().ok_or(ExportError::InvalidPlan)?,
        "objectStreams": "generate", "streamData": "compress",
        "recompressFlate": "", "compressionLevel": "9"
    }))
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
        if plan.is_whole_source_extract() {
            use std::io::{Read, Write};
            let source = resolve_source(sources, plan.sources()[0].id)?;
            let mut input =
                std::fs::File::open(&source.path).map_err(|_| ExportError::SourceChanged)?;
            let mut target = std::fs::File::create(output).map_err(|_| ExportError::Io)?;
            let mut buffer = [0; 65536];
            let mut copied = 0;
            loop {
                control.checkpoint()?;
                let size = input.read(&mut buffer).map_err(|_| ExportError::Io)?;
                if size == 0 {
                    break;
                }
                copied += size as u64;
                if copied > source.fingerprint.bytes {
                    return Err(ExportError::SourceChanged);
                }
                target
                    .write_all(&buffer[..size])
                    .map_err(|_| ExportError::Io)?;
            }
            if copied != source.fingerprint.bytes {
                return Err(ExportError::SourceChanged);
            }
            return Ok(());
        }
        self.version(control)?;
        let baseline = work.join(OPTIMIZE_BASELINE_FILE);
        let target = if plan.scope() == ExportScope::Optimized {
            baseline.as_path()
        } else {
            output
        };
        self.write_job(&job(plan, sources, target)?, work, "job.json", control)?;
        if plan.scope() == ExportScope::Optimized {
            self.check(&baseline, control)?;
            self.write_job(
                &optimization_job(&baseline, output)?,
                work,
                "optimize-job.json",
                control,
            )?;
        }
        if plan.is_single_source_extract() {
            self.remove_structure_references(output, work, control)?;
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
impl QpdfExporter {
    fn write_job(
        &self,
        job: &Value,
        work: &Path,
        name: &str,
        control: &ExportControl,
    ) -> Result<(), ExportError> {
        control.checkpoint()?;
        let bytes = serde_json::to_vec(job).map_err(|_| ExportError::InvalidPlan)?;
        if bytes.len() > JSON_LIMIT {
            return Err(ExportError::ResourceLimit);
        }
        let path = work.join(name);
        std::fs::write(&path, bytes).map_err(|_| ExportError::Io)?;
        let mut argument = OsString::from("--job-json-file=");
        argument.push(path);
        if self.invoke(&[argument], 65536, control)?.code != Some(0) {
            return Err(ExportError::Engine);
        }
        control.checkpoint()
    }
    fn output_json(&self, path: &Path, control: &ExportControl) -> Result<Value, ExportError> {
        let result = self.invoke(
            &[
                "--json=2".into(),
                "--json-stream-data=none".into(),
                "--json-key=qpdf".into(),
                path.as_os_str().to_owned(),
            ],
            JSON_LIMIT,
            control,
        )?;
        if result.code != Some(0) {
            return Err(ExportError::Validation);
        }
        serde_json::from_slice(&result.stdout).map_err(|_| ExportError::Inspection)
    }
    fn remove_structure_references(
        &self,
        output: &Path,
        work: &Path,
        control: &ExportControl,
    ) -> Result<(), ExportError> {
        let json = self.output_json(output, control)?;
        if let Some(patch) = crate::structure::patch(&json)? {
            let bytes = serde_json::to_vec(&patch).map_err(|_| ExportError::Inspection)?;
            if bytes.len() > JSON_LIMIT {
                return Err(ExportError::ResourceLimit);
            }
            let patch_path = work.join("structure-patch.json");
            std::fs::write(&patch_path, bytes).map_err(|_| ExportError::Io)?;
            let clean = work.join("untagged.pdf");
            let mut option = OsString::from("--update-from-json=");
            option.push(&patch_path);
            let result = self.invoke(
                &[
                    output.as_os_str().to_owned(),
                    option,
                    clean.as_os_str().to_owned(),
                ],
                65536,
                control,
            )?;
            if result.code != Some(0) {
                return Err(ExportError::Validation);
            }
            control.checkpoint()?;
            if crate::structure::patch(&self.output_json(&clean, control)?)?.is_some() {
                return Err(ExportError::Validation);
            }
            std::fs::copy(&clean, output).map_err(|_| ExportError::Io)?;
        }
        control.checkpoint()
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
    #[test]
    fn optimization_uses_only_lossless_compression_options() -> Result<(), ExportError> {
        let job = optimization_job(Path::new("baseline.pdf"), Path::new("optimized.pdf"))?;
        assert_eq!(
            job,
            json!({"inputFile":"baseline.pdf", "outputFile":"optimized.pdf", "objectStreams":"generate", "streamData":"compress", "recompressFlate":"", "compressionLevel":"9"})
        );
        Ok(())
    }
}
