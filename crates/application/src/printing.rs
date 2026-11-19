//! Print ranges and immutable page snapshots; printing never marks a revision clean.
use crate::export::{ExportError, ExportPlan};
#[derive(Clone)]
pub struct PrintPlan {
    pub snapshot: ExportPlan,
    pub indices: Vec<usize>,
}
impl PrintPlan {
    pub fn new(snapshot: ExportPlan, range: &str) -> Result<Self, ExportError> {
        let count = snapshot.pages().len();
        if count == 0 || range.len() > 4096 {
            return Err(ExportError::InvalidPlan);
        }
        let mut indices = Vec::new();
        if range.trim().is_empty() {
            indices.extend(0..count);
        } else {
            for part in range.split(',') {
                let mut numbers = part.trim().split('-');
                let first = numbers
                    .next()
                    .and_then(|n| n.trim().parse::<usize>().ok())
                    .filter(|n| *n > 0 && *n <= count)
                    .ok_or(ExportError::InvalidPlan)?;
                let last = match numbers.next() {
                    Some(n) => n
                        .trim()
                        .parse::<usize>()
                        .ok()
                        .filter(|n| *n >= first && *n <= count)
                        .ok_or(ExportError::InvalidPlan)?,
                    None => first,
                };
                if numbers.next().is_some() {
                    return Err(ExportError::InvalidPlan);
                }
                for index in first - 1..last {
                    if !indices.contains(&index) {
                        indices.push(index);
                    }
                }
            }
        }
        // Print ranges select pages in workspace order, not comma-entry order.
        indices.sort_unstable();
        Ok(Self { snapshot, indices })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    // Range grammar tested through its independent parser with a captured synthetic workspace.
    #[test]
    fn ranges_reject_bad_input_and_keep_workspace_order() -> Result<(), Box<dyn std::error::Error>>
    {
        use crate::{
            ExportSupport, PageGeometry, SourceDocument, SourceFingerprint, SourceId, Workspace,
        };
        let w = Workspace::new(SourceDocument {
            id: SourceId(1),
            path: "fixture.pdf".into(),
            fingerprint: SourceFingerprint {
                bytes: 1,
                modified_ns: 0,
            },
            geometries: vec![
                PageGeometry {
                    width_points: 300.,
                    height_points: 400.,
                    quarter_turns: 0
                };
                5
            ],
            annotations: vec![],
            export_support: ExportSupport::NotPreflighted,
        })?;
        let capture = || ExportPlan::capture(&w, 1, crate::export::ExportScope::All);
        assert_eq!(
            PrintPlan::new(capture()?, "5,1-3,2")?.indices,
            vec![0, 1, 2, 4]
        );
        for text in ["0", "6", "4-2", "1-2-3", "1,", "x"] {
            assert!(PrintPlan::new(capture()?, text).is_err());
        }
        assert_eq!(PrintPlan::new(capture()?, "")?.indices.len(), 5);
        Ok(())
    }
}
