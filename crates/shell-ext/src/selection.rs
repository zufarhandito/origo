//! Explorer selection classification and display filename derivation.
//! Designed to extend cleanly to future PDF actions under the same `Origo >` menu.
use crate::locale::ShellLanguage;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SelectionKind {
    Empty,
    Images(Vec<PathBuf>),
    Pdfs(Vec<PathBuf>),
    Unsupported,
}

pub fn is_supported_image(path: &Path) -> bool {
    path.extension().is_some_and(|ext| {
        ext.eq_ignore_ascii_case("png")
            || ext.eq_ignore_ascii_case("jpg")
            || ext.eq_ignore_ascii_case("jpeg")
    })
}

pub fn is_supported_pdf(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("pdf"))
}

pub fn classify_paths(paths: Vec<PathBuf>) -> SelectionKind {
    if paths.is_empty() {
        return SelectionKind::Empty;
    }
    if paths.iter().all(|p| is_supported_image(p)) {
        SelectionKind::Images(paths)
    } else if paths.iter().all(|p| is_supported_pdf(p)) {
        SelectionKind::Pdfs(paths)
    } else {
        SelectionKind::Unsupported
    }
}

fn sanitize_stem(raw: &str) -> String {
    let cleaned: String = raw
        .trim()
        .chars()
        .map(|c| {
            if matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || c.is_control() {
                '-'
            } else {
                c
            }
        })
        .collect();
    cleaned.trim_matches(|c| c == '.' || c == ' ').to_owned()
}

fn truncate_display(stem: &str, max_chars: usize) -> String {
    let count = stem.chars().count();
    if count <= max_chars {
        return stem.to_owned();
    }
    let prefix: String = stem.chars().take(max_chars.saturating_sub(1)).collect();
    format!("{prefix}…")
}

pub fn quick_pdf_display_name(images: &[PathBuf], lang: ShellLanguage) -> String {
    let raw = if images.len() == 1 {
        images[0].file_stem().and_then(|s| s.to_str()).unwrap_or("")
    } else {
        images
            .first()
            .and_then(|p| p.parent())
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("")
    };
    let stem = sanitize_stem(raw);
    if stem.is_empty() {
        return match lang {
            ShellLanguage::Indonesian => "gambar-ke-pdf.pdf".into(),
            ShellLanguage::English => "images.pdf".into(),
        };
    }
    format!("{}.pdf", truncate_display(&stem, 40))
}

pub fn quick_merge_pdf_display_name(pdfs: &[PathBuf], lang: ShellLanguage) -> String {
    let raw = pdfs
        .first()
        .and_then(|p| p.parent())
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("");
    let stem = sanitize_stem(raw);
    if stem.is_empty() {
        return match lang {
            ShellLanguage::Indonesian => "gabungan.pdf".into(),
            ShellLanguage::English => "combined.pdf".into(),
        };
    }
    format!("{}.pdf", truncate_display(&stem, 40))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_image_and_pdf_selections_strictly() {
        assert_eq!(classify_paths(vec![]), SelectionKind::Empty);
        let imgs = vec![
            PathBuf::from("C:\\Pictures\\a.PNG"),
            PathBuf::from("C:\\Pictures\\b.jpeg"),
            PathBuf::from("C:\\Pictures\\c.jpg"),
        ];
        assert_eq!(classify_paths(imgs.clone()), SelectionKind::Images(imgs));

        let pdfs = vec![
            PathBuf::from("C:\\Docs\\bab1.pdf"),
            PathBuf::from("C:\\Docs\\bab2.PDF"),
        ];
        assert_eq!(classify_paths(pdfs.clone()), SelectionKind::Pdfs(pdfs));

        assert_eq!(
            classify_paths(vec![
                PathBuf::from("C:\\Pictures\\a.png"),
                PathBuf::from("C:\\Pictures\\notes.txt"),
            ]),
            SelectionKind::Unsupported
        );
        assert_eq!(
            classify_paths(vec![
                PathBuf::from("C:\\Docs\\a.pdf"),
                PathBuf::from("C:\\Docs\\b.png"),
            ]),
            SelectionKind::Unsupported
        );
    }

    #[test]
    fn derives_single_and_folder_pdf_names() {
        let one = vec![PathBuf::from("C:\\Users\\demo\\Pictures\\scan_01.png")];
        assert_eq!(
            quick_pdf_display_name(&one, ShellLanguage::English),
            "scan_01.pdf"
        );
        let many = vec![
            PathBuf::from("C:\\Users\\demo\\Pictures\\a.png"),
            PathBuf::from("C:\\Users\\demo\\Pictures\\b.jpg"),
        ];
        assert_eq!(
            quick_pdf_display_name(&many, ShellLanguage::English),
            "Pictures.pdf"
        );
        let pdfs = vec![
            PathBuf::from("C:\\Users\\demo\\Kontrak\\01.pdf"),
            PathBuf::from("C:\\Users\\demo\\Kontrak\\02.pdf"),
        ];
        assert_eq!(
            quick_merge_pdf_display_name(&pdfs, ShellLanguage::Indonesian),
            "Kontrak.pdf"
        );
    }
}
