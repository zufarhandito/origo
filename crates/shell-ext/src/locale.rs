//! OS UI language detection and localized Explorer context menu labels.
//!
//! Follows the host OS display language automatically:
//! - Windows: `GetUserDefaultUILanguage()` (primary language `0x21` = Indonesian)
//! - macOS / Linux: `LC_ALL` / `LC_MESSAGES` / `LANG`
//! - Testing override: `ORIGO_SHELL_LANG=id|en`

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShellLanguage {
    Indonesian,
    English,
}

#[cfg(windows)]
fn detect_os_language() -> ShellLanguage {
    // SAFETY: GetUserDefaultUILanguage takes no pointers and returns a LANGID u16.
    let lang_id = unsafe { windows::Win32::Globalization::GetUserDefaultUILanguage() };
    let primary = lang_id & 0x03FF;
    // LANG_INDONESIAN = 0x21
    if primary == 0x21 {
        ShellLanguage::Indonesian
    } else {
        ShellLanguage::English
    }
}

#[cfg(not(windows))]
fn detect_os_language() -> ShellLanguage {
    for key in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(value) = std::env::var(key) {
            let lower = value.to_ascii_lowercase();
            if lower == "id" || lower.starts_with("id_") || lower.starts_with("id-") {
                return ShellLanguage::Indonesian;
            }
            if !lower.is_empty() && lower != "c" && lower != "posix" {
                return ShellLanguage::English;
            }
        }
    }
    ShellLanguage::English
}

pub fn detect_language() -> ShellLanguage {
    if let Ok(override_lang) = std::env::var("ORIGO_SHELL_LANG") {
        let lower = override_lang.trim().to_ascii_lowercase();
        if lower == "id" || lower.starts_with("id-") || lower.starts_with("id_") {
            return ShellLanguage::Indonesian;
        }
        if lower == "en" || lower.starts_with("en-") || lower.starts_with("en_") {
            return ShellLanguage::English;
        }
    }
    detect_os_language()
}

pub fn root_title() -> &'static str {
    "Origo"
}

pub fn dialog_title(lang: ShellLanguage, image_count: usize) -> &'static str {
    match lang {
        ShellLanguage::Indonesian => "Buat PDF dari gambar ini…",
        ShellLanguage::English => {
            if image_count == 1 {
                "Create PDF from image…"
            } else {
                "Create PDF from images…"
            }
        }
    }
}

pub fn quick_title(
    lang: ShellLanguage,
    target_pdf_name: Option<&str>,
    image_count: usize,
) -> String {
    match (lang, target_pdf_name, image_count) {
        (ShellLanguage::Indonesian, Some(name), 1) => format!("Buat \"{name}\" di sini"),
        (ShellLanguage::Indonesian, Some(name), _) => format!("Gabungkan ke \"{name}\""),
        (ShellLanguage::Indonesian, None, _) => "Gabungkan langsung ke PDF".into(),
        (ShellLanguage::English, Some(name), 1) => format!("Create \"{name}\" here"),
        (ShellLanguage::English, Some(name), _) => format!("Combine into \"{name}\""),
        (ShellLanguage::English, None, _) => "Combine directly to PDF".into(),
    }
}

pub fn pdf_open_title(lang: ShellLanguage) -> &'static str {
    match lang {
        ShellLanguage::Indonesian => "Buka di Origo",
        ShellLanguage::English => "Open in Origo",
    }
}

pub fn pdf_compress_title(lang: ShellLanguage) -> &'static str {
    match lang {
        ShellLanguage::Indonesian => "Kompres PDF (lossless)…",
        ShellLanguage::English => "Compress PDF (lossless)…",
    }
}

pub fn pdf_split_title(lang: ShellLanguage) -> &'static str {
    match lang {
        ShellLanguage::Indonesian => "Pisahkan PDF…",
        ShellLanguage::English => "Split PDF…",
    }
}

pub fn pdf_png_title(lang: ShellLanguage) -> &'static str {
    match lang {
        ShellLanguage::Indonesian => "Ekspor ke PNG…",
        ShellLanguage::English => "Export to PNG…",
    }
}

pub fn pdf_ocr_title(lang: ShellLanguage) -> &'static str {
    match lang {
        ShellLanguage::Indonesian => "OCR seluruh dokumen",
        ShellLanguage::English => "OCR document",
    }
}

pub fn pdf_combine_organize_title(lang: ShellLanguage) -> &'static str {
    match lang {
        ShellLanguage::Indonesian => "Gabungkan PDF di Origo…",
        ShellLanguage::English => "Combine PDFs in Origo…",
    }
}

pub fn pdf_combine_quick_title(lang: ShellLanguage, target_pdf_name: Option<&str>) -> String {
    match (lang, target_pdf_name) {
        (ShellLanguage::Indonesian, Some(name)) => format!("Gabungkan ke \"{name}\""),
        (ShellLanguage::Indonesian, None) => "Gabungkan langsung ke PDF".into(),
        (ShellLanguage::English, Some(name)) => format!("Combine into \"{name}\""),
        (ShellLanguage::English, None) => "Combine directly into PDF".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn localized_labels_match_language_and_count() {
        assert_eq!(
            dialog_title(ShellLanguage::English, 1),
            "Create PDF from image…"
        );
        assert_eq!(
            dialog_title(ShellLanguage::English, 3),
            "Create PDF from images…"
        );
        assert_eq!(
            dialog_title(ShellLanguage::Indonesian, 3),
            "Buat PDF dari gambar ini…"
        );
        assert_eq!(
            quick_title(ShellLanguage::English, Some("Pictures.pdf"), 4),
            "Combine into \"Pictures.pdf\""
        );
        assert_eq!(
            quick_title(ShellLanguage::English, Some("scan.pdf"), 1),
            "Create \"scan.pdf\" here"
        );
        assert_eq!(
            quick_title(ShellLanguage::Indonesian, Some("Pictures.pdf"), 4),
            "Gabungkan ke \"Pictures.pdf\""
        );
        assert_eq!(
            quick_title(ShellLanguage::Indonesian, Some("scan.pdf"), 1),
            "Buat \"scan.pdf\" di sini"
        );
        assert_eq!(pdf_open_title(ShellLanguage::Indonesian), "Buka di Origo");
        assert_eq!(pdf_open_title(ShellLanguage::English), "Open in Origo");
        assert_eq!(
            pdf_compress_title(ShellLanguage::English),
            "Compress PDF (lossless)…"
        );
        assert_eq!(
            pdf_combine_organize_title(ShellLanguage::Indonesian),
            "Gabungkan PDF di Origo…"
        );
        assert_eq!(
            pdf_combine_quick_title(ShellLanguage::English, Some("Kontrak.pdf")),
            "Combine into \"Kontrak.pdf\""
        );
    }
}
