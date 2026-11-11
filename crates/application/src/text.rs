use crate::{RenderError, SourceFingerprint};
use std::{ops::Range, path::PathBuf};

#[derive(Clone)]
pub struct TextRequest {
    pub path: PathBuf,
    pub page_index: u16,
    pub fingerprint: SourceFingerprint,
}
pub trait TextExtractor {
    fn extract_text(&self, request: &TextRequest) -> Result<TextPage, RenderError>;
}
/// Top-left origin, normalized to the displayed page including its source rotation.
#[derive(Clone, Copy, Debug)]
pub struct TextRect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}
impl TextRect {
    fn distance(&self, x: f32, y: f32) -> f32 {
        (self.left - x).max(0.).max(x - self.right).powi(2)
            + (self.top - y).max(0.).max(y - self.bottom).powi(2)
    }
}
#[derive(Clone, Debug)]
pub struct TextChar {
    pub unicode: char,
    pub bounds: Option<TextRect>,
}
pub struct TextPage {
    pub chars: Vec<TextChar>,
}
impl TextPage {
    /// Compose virtual clockwise rotation with already source-normalized character boxes.
    pub fn rotate(&mut self, turns: u8) {
        for ch in &mut self.chars {
            if let Some(r) = &mut ch.bounds {
                for _ in 0..turns % 4 {
                    *r = TextRect {
                        left: 1. - r.bottom,
                        top: r.left,
                        right: 1. - r.top,
                        bottom: r.right,
                    };
                }
            }
        }
    }

    pub fn sentence_range(&self, index: usize) -> Range<usize> {
        if index >= self.chars.len() {
            return 0..0;
        }
        let boundary = |i: usize| {
            let ch = self.chars[i].unicode;
            if matches!(ch, '!' | '?' | '。' | '！' | '？') {
                return true;
            }
            if ch == '.' {
                let decimal = i > 0
                    && i + 1 < self.chars.len()
                    && self.chars[i - 1].unicode.is_ascii_digit()
                    && self.chars[i + 1].unicode.is_ascii_digit();
                return !decimal;
            }
            ch == '\n'
                && self.chars[..i]
                    .iter()
                    .rev()
                    .find(|c| c.unicode != '\r')
                    .is_some_and(|c| c.unicode == '\n')
        };
        let mut start = (0..index)
            .rev()
            .find(|i| boundary(*i))
            .map(|i| i + 1)
            .unwrap_or(0);
        let mut end = (index..self.chars.len())
            .find(|i| boundary(*i))
            .map(|i| i + 1)
            .unwrap_or(self.chars.len());
        // With no punctuation, use the current extracted line (titles/labels).
        if !self.chars[start..end]
            .iter()
            .any(|c| matches!(c.unicode, '.' | '!' | '?' | '。' | '！' | '？'))
        {
            start = (start..index)
                .rev()
                .find(|i| matches!(self.chars[*i].unicode, '\r' | '\n'))
                .map(|i| i + 1)
                .unwrap_or(start);
            end = (index..end)
                .find(|i| matches!(self.chars[*i].unicode, '\r' | '\n'))
                .unwrap_or(end);
        }
        while start < end && self.chars[start].unicode.is_whitespace() {
            start += 1;
        }
        while end > start && self.chars[end - 1].unicode.is_whitespace() {
            end -= 1;
        }
        start..end
    }
    pub fn highlights(&self, range: Range<usize>) -> Vec<TextRect> {
        let mut result: Vec<TextRect> = Vec::new();
        for rect in self
            .chars
            .get(range)
            .unwrap_or_default()
            .iter()
            .filter_map(|ch| ch.bounds)
        {
            if let Some(last) = result.last_mut() {
                let horizontal = (last.top - rect.top).abs() < 0.002
                    && (last.bottom - rect.bottom).abs() < 0.002
                    && rect.left <= last.right + 0.01
                    && rect.right >= last.left - 0.01;
                let vertical = (last.left - rect.left).abs() < 0.002
                    && (last.right - rect.right).abs() < 0.002
                    && rect.top <= last.bottom + 0.01
                    && rect.bottom >= last.top - 0.01;
                if horizontal || vertical {
                    last.left = last.left.min(rect.left);
                    last.right = last.right.max(rect.right);
                    last.top = last.top.min(rect.top);
                    last.bottom = last.bottom.max(rect.bottom);
                    continue;
                }
            }
            result.push(rect);
        }
        result
    }
    pub fn bytes(&self) -> usize {
        self.chars.capacity() * std::mem::size_of::<TextChar>()
    }
    pub fn hit(&self, x: f32, y: f32, near_only: bool) -> Option<usize> {
        let (index, distance) = self
            .chars
            .iter()
            .enumerate()
            .filter_map(|(index, ch)| ch.bounds.map(|bounds| (index, bounds.distance(x, y))))
            .min_by(|a, b| a.1.total_cmp(&b.1))?;
        (!near_only || distance <= 0.0004).then_some(index)
    }
    pub fn copy_range(&self, range: Range<usize>) -> String {
        self.chars
            .get(range)
            .unwrap_or_default()
            .iter()
            .map(|ch| ch.unicode)
            .collect()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct TextPosition {
    pub page: usize,
    pub character: usize,
}
#[derive(Clone, Copy)]
pub struct TextSelection {
    pub anchor: TextPosition,
    pub focus: TextPosition,
}
impl TextSelection {
    pub fn range(&self, page: usize, count: usize) -> Range<usize> {
        let start = self.anchor.min(self.focus);
        let end = self.anchor.max(self.focus);
        if page < start.page || page > end.page {
            return 0..0;
        }
        let first = if page == start.page {
            start.character.min(count)
        } else {
            0
        };
        let last = if page == end.page {
            end.character.saturating_add(1).min(count)
        } else {
            count
        };
        first..last
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sentence_selection_handles_wrapped_lines_decimals_and_unpunctuated_labels() {
        let make = |text: &str| TextPage {
            chars: text
                .chars()
                .map(|unicode| TextChar {
                    unicode,
                    bounds: None,
                })
                .collect(),
        };
        let page = make("Satu kalimat.  Dua\r\nbaris 3.14 selesai! Berikutnya?");
        assert_eq!(page.copy_range(page.sentence_range(2)), "Satu kalimat.");
        assert_eq!(
            page.copy_range(page.sentence_range(19)),
            "Dua\r\nbaris 3.14 selesai!"
        );
        assert_eq!(
            page.copy_range(page.sentence_range(page.chars.len() - 2)),
            "Berikutnya?"
        );
        let label = make("PAGE 1\r\nSecond line");
        assert_eq!(label.copy_range(label.sentence_range(2)), "PAGE 1");
        assert_eq!(label.copy_range(label.sentence_range(10)), "Second line");
        assert!(label.sentence_range(99).is_empty());
    }
    #[test]
    fn reverse_selection_spans_pages_and_preserves_unicode_and_line_breaks() {
        let page = TextPage {
            chars: "Aé\r\nZ"
                .chars()
                .map(|unicode| TextChar {
                    unicode,
                    bounds: None,
                })
                .collect(),
        };
        let selection = TextSelection {
            anchor: TextPosition {
                page: 2,
                character: 1,
            },
            focus: TextPosition {
                page: 0,
                character: 1,
            },
        };
        assert_eq!(selection.range(0, 5), 1..5);
        assert_eq!(selection.range(1, 5), 0..5);
        assert_eq!(selection.range(2, 5), 0..2);
        assert_eq!(selection.range(3, 5), 0..0);
        assert_eq!(page.copy_range(selection.range(0, 5)), "é\r\nZ");
    }
    #[test]
    fn highlight_rectangles_merge_adjacent_glyphs_without_joining_separate_lines() {
        let page = TextPage {
            chars: vec![
                TextChar {
                    unicode: 'A',
                    bounds: Some(TextRect {
                        left: 0.1,
                        top: 0.1,
                        right: 0.2,
                        bottom: 0.15,
                    }),
                },
                TextChar {
                    unicode: 'B',
                    bounds: Some(TextRect {
                        left: 0.2,
                        top: 0.1,
                        right: 0.3,
                        bottom: 0.15,
                    }),
                },
                TextChar {
                    unicode: 'C',
                    bounds: Some(TextRect {
                        left: 0.1,
                        top: 0.2,
                        right: 0.2,
                        bottom: 0.25,
                    }),
                },
            ],
        };
        let rects = page.highlights(0..3);
        assert_eq!(rects.len(), 2);
        assert_eq!(rects[0].right, 0.3);
        assert_eq!(page.highlights(1..2).len(), 1);
        assert!(page.highlights(0..0).is_empty());
    }
    #[test]
    fn hit_testing_ignores_non_spatial_characters_and_empty_scanned_pages() {
        let page = TextPage {
            chars: vec![
                TextChar {
                    unicode: 'A',
                    bounds: Some(TextRect {
                        left: 0.1,
                        top: 0.2,
                        right: 0.2,
                        bottom: 0.3,
                    }),
                },
                TextChar {
                    unicode: '\n',
                    bounds: None,
                },
            ],
        };
        assert_eq!(page.hit(0.15, 0.25, true), Some(0));
        assert_eq!(page.hit(0.9, 0.9, true), None);
        assert_eq!(page.hit(0.9, 0.9, false), Some(0));
        assert_eq!(TextPage { chars: vec![] }.hit(0., 0., false), None);
    }
}
