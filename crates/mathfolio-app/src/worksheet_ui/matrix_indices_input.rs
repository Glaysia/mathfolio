//! Single retained flat-matrix result only. No source editor/calculator calls.
use super::*;
impl WorksheetApp {
    pub(super) fn matrix_indices_context(&self) -> Option<ribbon::MatrixIndicesContext> {
        if self.matrix_view_drag.is_some() {
            return None;
        }
        let owner = self.selected?;
        // Bounded RESULT adapter. Prime's full command can instead select a
        // source matrix format; that separate source formatting is not offered.
        let region = self.document.regions.iter().find(|r| r.id == owner)?;
        if !matches!(&region.content, CanvasContent::Math(tree)
            if matches!(tree.kind, NodeKind::Binary(BinaryOp::Evaluate)))
        {
            return None;
        }
        let (_, show_indices) = self.result_matrix_indices(owner)?;
        self.display_cache.get(&owner)?.matrix.value()?;
        if let Some(editor) = self.editors.get(&owner) {
            if matches!(
                editor.selection(),
                mathfolio_editor::Selection::MatrixCells(_)
            ) {
                return None;
            }
            let mut current = Some(editor.caret().node);
            while let Some(id) = current {
                let node = editor.arena().get(id)?;
                if matches!(node.kind(), NodeKind::Matrix { .. }) {
                    return None;
                }
                current = node.parent();
            }
        }
        Some(ribbon::MatrixIndicesContext {
            owner,
            show_indices,
        })
    }
    pub(super) fn set_matrix_indices(&mut self, owner: u64, show: bool) -> Result<bool, String> {
        let current = self
            .matrix_indices_context()
            .filter(|c| c.owner == owner)
            .ok_or("Matrix indices target is unavailable or unresolved")?;
        if current.show_indices == show {
            return Ok(false);
        }
        let origin = self
            .result_matrix_indices(owner)
            .ok_or("Matrix result origin is unavailable")?
            .0;
        let target = MatrixResultTarget::math(owner);
        let before = self.document.matrix_result_views.get(&target).copied();
        let mut next = match before {
            Some(local) => local,
            None => {
                // Prime first-local field mutation clones the inherited
                // requested format, not the shape-clamped visible projection.
                let inherited = self
                    .matrix_view_sources
                    .effective(owner, None)
                    .ok_or("Inherited matrix result format is unresolved")?;
                if inherited.size.is_none()
                    || inherited.offset.is_none()
                    || inherited.show_indices.is_none()
                {
                    return Err("Inherited matrix result format is incomplete".into());
                }
                inherited
            }
        };
        next.show_indices = Some(show);
        let window = self
            .matrix_view_sources
            .window(owner, Some(next))
            .ok_or("Matrix view is unresolved")?;
        let format = self
            .document
            .regions
            .iter()
            .find(|r| r.id == owner)
            .ok_or("Missing matrix owner")?
            .format;
        self.display_cache
            .get_mut(&owner)
            .ok_or("Complete matrix result unavailable")?
            .matrix
            .preflight(window, format)?;
        // Turning off a persistent request must remain possible after a new
        // origin makes its old labels unsupported. Turning on is preflighted.
        if show {
            let projection = self
                .display_cache
                .get(&owner)
                .and_then(|cache| cache.matrix.value())
                .ok_or("Matrix result view is unavailable")?;
            formula::MatrixIndexLabels::for_projection(projection, origin, true)?;
        }
        let history = self.snapshot_with_document(DocumentHistory::MatrixResultView {
            target,
            viewport: before,
        });
        let changed = self.set_matrix_result_view(owner, Some(next))?;
        if changed {
            self.remember(history);
        }
        Ok(changed)
    }
}
