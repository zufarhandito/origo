use super::*;
use pdf_application::{
    PdfInspector,
    text::{TextExtractor, TextRequest},
};
use pdf_ocr::FrozenResults;
use std::io::Read;

/// Decorates the ordinary snapshot exporter with an immutable local text layer.
pub struct SearchableExporter<R> {
    base: QpdfExporter,
    results: FrozenResults,
    renderer: R,
}

impl<R: TextExtractor> SearchableExporter<R> {
    pub fn new(base: QpdfExporter, results: FrozenResults, renderer: R) -> Self {
        Self {
            base,
            results,
            renderer,
        }
    }

    fn validate_text(&self, output: &Path, control: &ExportControl) -> Result<(), ExportError> {
        let fingerprint = fingerprint(output)?;
        for index in 0..self.results.pages() {
            control.checkpoint()?;
            let Some(page) = self.results.get(index).map_err(|_| ExportError::Io)? else {
                continue;
            };
            let text = self
                .renderer
                .extract_text(&TextRequest {
                    path: output.to_path_buf(),
                    page_index: u16::try_from(index).map_err(|_| ExportError::Validation)?,
                    fingerprint,
                })
                .map_err(|_| ExportError::Validation)?;
            let actual = text
                .chars
                .iter()
                .map(|ch| ch.unicode)
                .filter(|c| !c.is_whitespace());
            let expected = page.page.words.iter().flat_map(|word| word.text.chars());
            if !actual.eq(expected) {
                return Err(ExportError::Validation);
            }
        }
        Ok(())
    }
}

impl<R: TextExtractor + PdfInspector> PdfExporter for SearchableExporter<R> {
    fn write(
        &self,
        plan: &ExportPlan,
        sources: &[SourceDocument],
        output: &Path,
        work: &Path,
        control: &ExportControl,
    ) -> Result<(), ExportError> {
        if plan.scope() != ExportScope::Searchable || self.results.recognized() == 0 {
            return Err(ExportError::InvalidPlan);
        }
        let mut font = Vec::new();
        std::fs::File::open(&self.base.ocr_font)
            .and_then(|file| file.take(573).read_to_end(&mut font))
            .map_err(|_| ExportError::Unsupported("Font OCR lokal tidak tersedia."))?;
        if font.len() != 572 {
            return Err(ExportError::Unsupported("Font OCR lokal rusak."));
        }
        self.base.write(plan, sources, output, work, control)?;
        let limit = annotation_json_limit(plan.pages().iter().flat_map(|page| &page.annotations));
        let doc = self.base.output_json(output, control, limit)?;
        let patch = crate::ocr_layer::patch(plan, &doc, &self.results, &font, work, control)?;
        let patch = serde_json::to_vec(&patch).map_err(|_| ExportError::Inspection)?;
        if patch.len() > limit {
            return Err(ExportError::ResourceLimit);
        }
        control.checkpoint()?;
        let input = work.join("ocr-patch.json");
        std::fs::write(&input, patch).map_err(|_| ExportError::Io)?;
        let revised = work.join("ocr-searchable.pdf");
        let mut option = OsString::from("--update-from-json=");
        option.push(&input);
        if self
            .base
            .invoke(
                &[
                    output.as_os_str().to_owned(),
                    option,
                    revised.as_os_str().to_owned(),
                ],
                65536,
                control,
            )?
            .code
            != Some(0)
        {
            return Err(ExportError::Engine);
        }
        control.checkpoint()?;
        let validation = self.validate_text(&revised, control);
        // Release the staging PDF's cached native handles on success or failure.
        self.renderer
            .inspect_output(&revised)
            .map_err(|_| ExportError::Validation)?;
        validation?;
        std::fs::copy(revised, output).map_err(|_| ExportError::Io)?;
        Ok(())
    }

    fn check(&self, path: &Path, control: &ExportControl) -> Result<(), ExportError> {
        self.base.check(path, control)
    }
}
