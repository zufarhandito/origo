//! Whole-document UTF-8 plain text extraction in `ExportPlan` page order.
use crate::export::{ExportControl, ExportError, ExportPlan, resolve_source};
use crate::text::{TextExtractor, TextRequest};

pub const MAX_TEXT_EXPORT_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextExportOptions {
    pub include_page_headers: bool,
}

impl Default for TextExportOptions {
    fn default() -> Self {
        Self {
            include_page_headers: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtractedDocumentText {
    pub content: String,
    pub pages: usize,
    pub pages_with_text: usize,
    pub chars: usize,
}

/// Normalize extracted PDF text line endings and trailing whitespace without destroying paragraphs.
pub fn normalize_page_text(raw: &str) -> String {
    let mut lines: Vec<&str> = Vec::new();
    let unified = raw.replace("\r\n", "\n").replace('\r', "\n");
    let mut blank_run = 0usize;
    for line in unified.split('\n') {
        let trimmed = line.trim_end_matches([' ', '\t']);
        if trimmed.is_empty() {
            if !lines.is_empty() && blank_run < 1 {
                lines.push("");
                blank_run += 1;
            }
        } else {
            blank_run = 0;
            lines.push(trimmed);
        }
    }
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

/// Join normalized per-page strings into one UTF-8 document string.
pub fn assemble_document_text(
    page_texts: &[String],
    include_page_headers: bool,
) -> (String, usize) {
    let mut out = String::new();
    let mut pages_with_text = 0usize;
    let total = page_texts.len();
    for (idx, page_text) in page_texts.iter().enumerate() {
        let normalized = normalize_page_text(page_text);
        if !normalized.is_empty() {
            pages_with_text += 1;
        }
        if include_page_headers && total > 1 {
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            out.push_str(&format!("=== Halaman {} ===\n", idx + 1));
            if !normalized.is_empty() {
                out.push_str(&normalized);
            }
        } else if !normalized.is_empty() {
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            out.push_str(&normalized);
        }
    }
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    (out, pages_with_text)
}

/// Extract UTF-8 text from every page in `plan` in current document order.
pub fn extract_plan_text<E: TextExtractor>(
    plan: &ExportPlan,
    extractor: &E,
    options: TextExportOptions,
    control: &ExportControl,
    mut progress: impl FnMut(usize, usize),
) -> Result<ExtractedDocumentText, ExportError> {
    control.checkpoint()?;
    let pages = plan.pages();
    if pages.is_empty() {
        return Err(ExportError::Empty);
    }
    let total = pages.len();
    let mut per_page = Vec::with_capacity(total);
    let mut approx_bytes = 0usize;
    for (idx, page) in pages.iter().enumerate() {
        control.checkpoint()?;
        let source = resolve_source(plan.sources(), page.entry.source_id)?;
        let extracted = extractor
            .extract_text(&TextRequest {
                path: source.path.clone(),
                page_index: page.entry.source_page_index,
                fingerprint: source.fingerprint,
            })
            .map_err(|_| ExportError::Io)?;
        let raw: String = extracted.chars.iter().map(|ch| ch.unicode).collect();
        approx_bytes = approx_bytes.saturating_add(raw.len());
        if approx_bytes > MAX_TEXT_EXPORT_BYTES {
            return Err(ExportError::ResourceLimit);
        }
        per_page.push(raw);
        progress(idx + 1, total);
    }
    control.checkpoint()?;
    let (content, pages_with_text) =
        assemble_document_text(&per_page, options.include_page_headers);
    if content.len() > MAX_TEXT_EXPORT_BYTES {
        return Err(ExportError::ResourceLimit);
    }
    let chars = content.chars().count();
    Ok(ExtractedDocumentText {
        content,
        pages: total,
        pages_with_text,
        chars,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::export::ExportScope;
    use crate::text::{TextChar, TextPage};
    use crate::{
        ExportSupport, PageGeometry, RenderError, SourceDocument, SourceFingerprint, SourceId,
        Workspace,
    };
    use pdf_domain::Command;

    struct FakeExtractor {
        pages: Vec<String>,
    }
    impl TextExtractor for FakeExtractor {
        fn extract_text(&self, request: &TextRequest) -> Result<TextPage, RenderError> {
            let text = self
                .pages
                .get(usize::from(request.page_index))
                .cloned()
                .ok_or(RenderError::InvalidPage)?;
            Ok(TextPage {
                chars: text
                    .chars()
                    .map(|unicode| TextChar {
                        unicode,
                        bounds: None,
                    })
                    .collect(),
            })
        }
    }

    #[test]
    fn normalize_and_assemble_respects_plan_order_and_headers()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut ws = Workspace::new(SourceDocument {
            id: SourceId(1),
            path: "sample.pdf".into(),
            fingerprint: SourceFingerprint {
                bytes: 10,
                modified_ns: 1,
            },
            export_support: ExportSupport::NotPreflighted,
            annotations: vec![],
            geometries: vec![
                PageGeometry {
                    width_points: 595.,
                    height_points: 842.,
                    quarter_turns: 0,
                };
                3
            ],
        })?;
        // Reorder page 3 to front and delete page 2
        let ids: Vec<_> = ws.pages().iter().map(|p| p.id).collect();
        ws.execute(Command::Move {
            ids: vec![ids[2]],
            target: 0,
        })?;
        ws.execute(Command::Delete { ids: vec![ids[1]] })?;

        let plan = ExportPlan::capture(&ws, 1, ExportScope::All)?;
        let extractor = FakeExtractor {
            pages: vec![
                "Halo dunia   \r\n\r\n\r\nBaris dua\r".into(),
                "Halaman dihapus".into(),
                "  Judul Bab 3  \r\nIsi UTF-8: café €".into(),
            ],
        };
        let control = ExportControl::with_budget(std::time::Duration::from_secs(30));
        let mut seen = Vec::new();
        let with_headers = extract_plan_text(
            &plan,
            &extractor,
            TextExportOptions {
                include_page_headers: true,
            },
            &control,
            |done, total| seen.push((done, total)),
        )?;
        assert_eq!(seen, vec![(1, 2), (2, 2)]);
        assert_eq!(with_headers.pages, 2);
        assert_eq!(with_headers.pages_with_text, 2);
        assert_eq!(
            with_headers.content,
            "=== Halaman 1 ===\n  Judul Bab 3\nIsi UTF-8: café €\n\n=== Halaman 2 ===\nHalo dunia\n\nBaris dua\n"
        );

        let no_headers = extract_plan_text(
            &plan,
            &extractor,
            TextExportOptions {
                include_page_headers: false,
            },
            &control,
            |_, _| {},
        )?;
        assert_eq!(
            no_headers.content,
            "  Judul Bab 3\nIsi UTF-8: café €\n\nHalo dunia\n\nBaris dua\n"
        );
        Ok(())
    }
}
