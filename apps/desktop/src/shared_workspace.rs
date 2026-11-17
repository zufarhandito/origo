//! One UI-thread document model shared by its reading views.
use crate::Workspace;
use pdf_domain::SourceId;
use std::{
    cell::{Cell, Ref, RefCell, RefMut},
    rc::Rc,
};

#[derive(Clone, Default)]
pub(super) struct SharedWorkspace(Rc<Model>);
#[derive(Default)]
struct Model {
    workspace: RefCell<Option<Workspace>>,
    next_source: Cell<u64>,
    importing: Cell<bool>,
}
#[derive(Clone, Default, PartialEq, Eq)]
pub(super) struct ModelSignature {
    pub revision: Option<u64>,
    dirty: bool,
    selected: Vec<pdf_domain::PageId>,
    importing: bool,
}

impl SharedWorkspace {
    pub fn signature(&self) -> ModelSignature {
        let mut signature = ModelSignature {
            importing: self.importing(),
            ..Default::default()
        };
        if let Some(w) = self.as_ref() {
            signature.revision = Some(w.revision());
            signature.dirty = w.is_dirty();
            signature.selected = w.selected().iter().copied().collect();
            signature
                .selected
                .sort_by_key(|id| (id.source.0, id.serial));
        }
        signature
    }
    pub fn as_ref(&self) -> Option<Ref<'_, Workspace>> {
        Ref::filter_map(self.0.workspace.borrow(), Option::as_ref).ok()
    }
    pub fn as_mut(&self) -> Option<RefMut<'_, Workspace>> {
        RefMut::filter_map(self.0.workspace.borrow_mut(), Option::as_mut).ok()
    }
    pub fn is_none(&self) -> bool {
        self.0.workspace.borrow().is_none()
    }
    pub fn is_some(&self) -> bool {
        !self.is_none()
    }
    pub fn install(&self, workspace: Workspace) {
        self.0.next_source.set(
            workspace
                .sources()
                .iter()
                .map(|s| s.id.0)
                .max()
                .unwrap_or(0),
        );
        *self.0.workspace.borrow_mut() = Some(workspace);
    }
    pub fn clear(&self) {
        *self.0.workspace.borrow_mut() = None;
    }
    pub fn next_source(&self) -> Option<SourceId> {
        let id = self.0.next_source.get().checked_add(1)?;
        self.0.next_source.set(id);
        Some(SourceId(id))
    }
    pub fn importing(&self) -> bool {
        self.0.importing.get()
    }
    pub fn set_importing(&self, busy: bool) {
        self.0.importing.set(busy);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdf_domain::{Command, ExportSupport, PageGeometry, SourceDocument, SourceFingerprint};
    #[test]
    fn two_views_share_history_selection_allocator_and_invalidation() -> anyhow::Result<()> {
        let source = SourceDocument {
            id: SourceId(100),
            path: "C:/fixture.pdf".into(),
            fingerprint: SourceFingerprint {
                bytes: 10,
                modified_ns: 0,
            },
            geometries: vec![
                PageGeometry {
                    width_points: 600.,
                    height_points: 800.,
                    quarter_turns: 0
                };
                3
            ],
            export_support: ExportSupport::NotPreflighted,
        };
        let left = SharedWorkspace::default();
        left.install(Workspace::new(source)?);
        let right = left.clone();
        let id = left
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("model missing"))?
            .pages()[1]
            .id;
        {
            let mut w = left
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("model missing"))?;
            w.select([id]);
            w.execute(Command::Delete { ids: vec![id] })?;
        }
        assert_eq!(right.as_ref().map(|w| w.pages().len()), Some(2));
        assert!(right.as_ref().is_some_and(|w| w.is_dirty()));
        assert!(right.as_mut().is_some_and(|mut w| w.undo()));
        assert_eq!(left.as_ref().map(|w| w.pages().len()), Some(3));
        assert!(left.as_ref().is_some_and(|w| w.selected().contains(&id)));
        assert!(!left.as_ref().is_some_and(|w| w.is_dirty()));
        assert_eq!(left.next_source(), Some(SourceId(101)));
        assert_eq!(right.next_source(), Some(SourceId(102)));
        right.set_importing(true);
        assert!(left.importing());
        left.set_importing(false);
        assert!(!right.importing());
        right.clear();
        assert!(left.is_none());
        Ok(())
    }
}
