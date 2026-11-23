use crate::{
    Cached, Language, MAX_IMAGE_PIXELS, OcrError, PageKey, Raster, Recognition, Recognizer, Store,
};
use pdf_application::{
    ExactPageRenderer, ExactRenderRequest, PageGeometry, RenderError,
    export::ExportControl,
    text::{TextChar, TextExtractor, TextPage, TextRect, TextRequest},
};

#[derive(Clone)]
pub struct DocumentPage {
    pub request: TextRequest,
    /// Display geometry including any workspace rotation.
    pub geometry: PageGeometry,
    pub rotation: u8,
    pub language: Language,
}

#[derive(Debug, thiserror::Error)]
pub enum DocumentError {
    #[error(transparent)]
    Ocr(#[from] OcrError),
    #[error(transparent)]
    Render(#[from] RenderError),
    #[error(transparent)]
    Cache(#[from] crate::CacheError),
}

impl DocumentPage {
    pub fn key(&self) -> PageKey {
        PageKey::new(&self.request, self.language)
    }

    pub fn recognize(
        &self,
        renderer: &(impl TextExtractor + ExactPageRenderer),
        engine: &dyn Recognizer,
        store: &Store,
        control: &ExportControl,
        retry_empty: bool,
    ) -> Result<Cached, DocumentError> {
        control.checkpoint().map_err(OcrError::from)?;
        let key = self.key();
        if let Some(entry @ (Cached::Text(_) | Cached::Native)) = store.get(&key)?
            && !(retry_empty && matches!(&entry, Cached::Text(text) if text.page.words.is_empty()))
        {
            // Revalidate even on a cache hit. Source identity is also checked
            // on each native render and before a new result is committed.
            verify(&self.request)?;
            return Ok(entry);
        }
        let text = renderer.extract_text(&self.request)?;
        let entry = if has_text(&text) {
            Cached::Native
        } else {
            control.checkpoint().map_err(OcrError::from)?;
            let (width, height) = dimensions(self.geometry)?;
            let page = renderer.render_exact(&ExactRenderRequest {
                annotations: Some(vec![]),
                path: self.request.path.clone(),
                page_index: self.request.page_index,
                rotation_delta: self.rotation,
                fingerprint: self.request.fingerprint,
                width,
                height,
            })?;
            let result = engine.recognize(
                Raster {
                    width: page.width,
                    height: page.height,
                    rgba: page.rgba,
                },
                self.language,
                control,
            )?;
            Cached::Text(Recognition {
                page: result,
                rotation: self.rotation,
            })
        };
        control.checkpoint().map_err(OcrError::from)?;
        verify(&self.request)?;
        store.put(&key, &entry)?;
        Ok(entry)
    }
}

fn verify(request: &TextRequest) -> Result<(), RenderError> {
    if pdf_platform::fingerprint(&request.path).ok() != Some(request.fingerprint) {
        return Err(RenderError::SourceChanged);
    }
    Ok(())
}

pub fn has_text(page: &TextPage) -> bool {
    page.chars
        .iter()
        .any(|ch| !ch.unicode.is_whitespace() && !ch.unicode.is_control())
}

pub fn read_text(
    renderer: &impl TextExtractor,
    store: &Store,
    request: &TextRequest,
    language: Language,
) -> Result<(TextPage, bool), RenderError> {
    let native = renderer.extract_text(request)?;
    if has_text(&native) {
        return Ok((native, false));
    }
    let cached = store.get(&PageKey::new(request, language));
    if let Ok(Some(Cached::Text(text))) = cached {
        verify(request)?;
        return Ok((text.text_page(), true));
    }
    Ok((native, false))
}

impl Recognition {
    /// Source-normalized frame, matching PDFium's native text extraction.
    pub fn text_page(&self) -> TextPage {
        let mut chars = Vec::new();
        let mut last_line = None;
        for word in &self.page.words {
            if let Some(line) = last_line {
                chars.push(TextChar {
                    unicode: if line == word.line { ' ' } else { '\n' },
                    bounds: None,
                });
            }
            let [left, top, right, bottom] = word.rect;
            let length = word.text.chars().count() as f32;
            let step = (right - left) / length;
            for (i, unicode) in word.text.chars().enumerate() {
                chars.push(TextChar {
                    unicode,
                    bounds: Some(TextRect {
                        left: left + i as f32 * step,
                        top,
                        right: (left + (i + 1) as f32 * step).min(right),
                        bottom,
                    }),
                });
            }
            last_line = Some(word.line);
        }
        let mut page = TextPage { chars };
        page.rotate((4 - self.rotation) % 4);
        page
    }
}

pub fn dimensions(geometry: PageGeometry) -> Result<(u32, u32), OcrError> {
    let width = f64::from(geometry.width_points);
    let height = f64::from(geometry.height_points);
    if !width.is_finite() || !height.is_finite() || width <= 0. || height <= 0. {
        return Err(OcrError::Limit);
    }
    let scale = (300. / 72_f64)
        .min(4096. / width.max(height))
        .min((MAX_IMAGE_PIXELS as f64 / (width * height)).sqrt());
    let result = (
        (width * scale).floor() as u32,
        (height * scale).floor() as u32,
    );
    if result.0 < 32 || result.1 < 32 {
        return Err(OcrError::Limit);
    }
    Ok(result)
}
