//! Bounded literal phrase matching with source-character offsets for highlights.
use crate::text::TextPage;
use std::ops::Range;

pub const QUERY_LIMIT: usize = 1024;
pub const MATCH_LIMIT: usize = 10_000;

#[derive(Clone, Copy, Default)]
pub struct SearchOptions {
    pub case_sensitive: bool,
    pub whole_words: bool,
}

#[derive(Clone)]
pub struct SearchQuery {
    needle: Vec<char>,
    prefix: Vec<usize>,
    options: SearchOptions,
}

// Collapse adjacent whitespace (including PDF line breaks), keeping offsets into
// TextPage rather than UTF-8 bytes. Lowercase expansions keep their source index.
fn normalized(chars: impl Iterator<Item = char>, case_sensitive: bool) -> Vec<(char, usize)> {
    let mut result = Vec::new();
    for (index, ch) in chars.enumerate() {
        if ch.is_whitespace() {
            if result.last().is_none_or(|&(ch, _)| ch != ' ') {
                result.push((' ', index));
            }
        } else if case_sensitive {
            result.push((ch, index));
        } else {
            result.extend(ch.to_lowercase().map(|ch| (ch, index)));
        }
    }
    result
}

impl SearchQuery {
    /// Empty/whitespace and oversized queries do not start native work.
    pub fn new(text: &str, options: SearchOptions) -> Option<Self> {
        if text.chars().count() > QUERY_LIMIT {
            return None;
        }
        let needle: Vec<_> = normalized(text.trim().chars(), options.case_sensitive)
            .into_iter()
            .map(|(ch, _)| ch)
            .collect();
        if needle.is_empty() {
            return None;
        }
        let mut prefix = vec![0; needle.len()];
        let mut matched = 0;
        for i in 1..needle.len() {
            while matched > 0 && needle[i] != needle[matched] {
                matched = prefix[matched - 1];
            }
            if needle[i] == needle[matched] {
                matched += 1;
            }
            prefix[i] = matched;
        }
        Some(Self {
            needle,
            prefix,
            options,
        })
    }

    /// Non-overlapping, ordered occurrences of the entire phrase within one page.
    /// No regex, stemming, fuzzy matching, dehyphenation or cross-page matches.
    pub fn find(&self, page: &TextPage, limit: usize) -> Vec<Range<usize>> {
        let text = normalized(
            page.chars.iter().map(|c| c.unicode),
            self.options.case_sensitive,
        );
        let word = |ch: char| ch.is_alphanumeric() || ch == '_';
        let mut result = Vec::new();
        let mut matched = 0;
        for (i, &(ch, _)) in text.iter().enumerate() {
            if result.len() >= limit {
                break;
            }
            while matched > 0 && ch != self.needle[matched] {
                matched = self.prefix[matched - 1];
            }
            if ch == self.needle[matched] {
                matched += 1;
            }
            if matched == self.needle.len() {
                let start = text[i + 1 - matched].1;
                let end = text[i].1 + 1;
                if !self.options.whole_words
                    || (start == 0 || !word(page.chars[start - 1].unicode))
                        && (end == page.chars.len() || !word(page.chars[end].unicode))
                {
                    result.push(start..end);
                    matched = 0;
                } else {
                    matched = self.prefix[matched - 1];
                }
            }
        }
        result
    }
}

/// Wrap navigation; no active occurrence until results have actually arrived.
pub fn next_match(current: Option<usize>, count: usize, previous: bool) -> Option<usize> {
    if count == 0 {
        return None;
    }
    Some(match current.filter(|&i| i < count) {
        None => {
            if previous {
                count - 1
            } else {
                0
            }
        }
        Some(0) if previous => count - 1,
        Some(i) if previous => i - 1,
        Some(i) => (i + 1) % count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::TextChar;
    fn page(text: &str) -> TextPage {
        TextPage {
            chars: text
                .chars()
                .map(|unicode| TextChar {
                    unicode,
                    bounds: None,
                })
                .collect(),
        }
    }
    fn find(query: &str, text: &str, options: SearchOptions) -> Vec<Range<usize>> {
        SearchQuery::new(query, options).map_or_else(Vec::new, |q| q.find(&page(text), MATCH_LIMIT))
    }
    #[test]
    fn default_matches_entire_phrase_case_insensitively_and_maps_line_breaks() {
        let text = "Hasil PENELITIAN; hasil lain penelitian; hasil\r\n penelitian";
        let hits = find("hasil penelitian", text, SearchOptions::default());
        assert_eq!(hits.len(), 2);
        assert_eq!(page(text).copy_range(hits[0].clone()), "Hasil PENELITIAN");
        assert_eq!(
            page(text).copy_range(hits[1].clone()),
            "hasil\r\n penelitian"
        );
        assert!(find("penelitian hasil", text, SearchOptions::default()).is_empty());
    }
    #[test]
    fn case_and_whole_word_options_are_independent() {
        let text = "Data database DATA metadata data_1 data";
        assert_eq!(find("data", text, SearchOptions::default()).len(), 6);
        assert_eq!(
            find(
                "data",
                text,
                SearchOptions {
                    case_sensitive: true,
                    whole_words: false
                }
            )
            .len(),
            4
        );
        assert_eq!(
            find(
                "data",
                text,
                SearchOptions {
                    whole_words: true,
                    ..Default::default()
                }
            )
            .len(),
            3
        );
        assert_eq!(
            find(
                "data",
                text,
                SearchOptions {
                    case_sensitive: true,
                    whole_words: true
                }
            )
            .len(),
            1
        );
    }
    #[test]
    fn unicode_indices_limits_and_empty_queries() {
        let text = "猫 ÉTUDE étude";
        assert_eq!(
            find("étude", text, SearchOptions::default()),
            vec![2..7, 8..13]
        );
        assert!(SearchQuery::new(" \r\n", SearchOptions::default()).is_none());
        assert!(SearchQuery::new(&"x".repeat(QUERY_LIMIT + 1), SearchOptions::default()).is_none());
        if let Some(q) = SearchQuery::new("a", SearchOptions::default()) {
            assert_eq!(q.find(&page(&"a".repeat(20_000)), 3).len(), 3);
            assert!(q.find(&page("aaa"), 0).is_empty());
        }
        assert_eq!(
            find("aa", "aaaaa", SearchOptions::default()),
            vec![0..2, 2..4]
        );
    }
    #[test]
    fn navigation_wraps_both_ways() {
        assert_eq!(next_match(None, 0, false), None);
        assert_eq!(next_match(None, 3, false), Some(0));
        assert_eq!(next_match(None, 3, true), Some(2));
        assert_eq!(next_match(Some(2), 3, false), Some(0));
        assert_eq!(next_match(Some(0), 3, true), Some(2));
    }
}
