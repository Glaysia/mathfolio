//! Presentation-only result resize history, source-format receipts and caching.
use super::*;
use crate::result_display::MatrixWindow;
use mathfolio_document::canvas::{
    MatrixResultTarget, MatrixResultViewport, MatrixViewOffset, MatrixViewSize,
};

mod indices;
pub(super) use indices::{indexed_formula_display, result_indices_metadata};
mod cache;
pub(super) use cache::MatrixCache;

pub(super) struct SourceFormats {
    defaults: Option<MatrixResultViewport>,
    owners: HashMap<u64, Option<MatrixResultViewport>>,
}

#[cfg(test)]
mod tests;
impl Default for SourceFormats {
    fn default() -> Self {
        Self {
            defaults: Some(MatrixResultViewport {
                size: Some(MatrixViewSize { rows: 12, cols: 12 }),
                offset: Some(MatrixViewOffset { row: 0, col: 0 }),
                show_indices: Some(false),
            }),
            owners: HashMap::new(),
        }
    }
}
impl SourceFormats {
    pub(super) fn from_document(
        document: &CanvasDocument,
        package: Option<&NativePackage>,
    ) -> Self {
        if document.native_archive.is_none() {
            return Self::default();
        }
        let Some(package) = package else {
            return Self {
                defaults: None,
                owners: HashMap::new(),
            };
        };
        let defaults = mathfolio_document::matrix_result_format::read_defaults(package).ok();
        let mut state = Self {
            defaults,
            owners: HashMap::new(),
        };
        let Ok(xml) = package.worksheet_xml() else {
            state.defaults = None;
            return state;
        };
        let Ok(source) = roxmltree::Document::parse(&xml) else {
            state.defaults = None;
            return state;
        };
        const WS: &str = "http://schemas.mathsoft.com/worksheet50";
        let mut source_owners = HashMap::new();
        let mut duplicates = HashSet::new();
        for region in source
            .root_element()
            .children()
            .filter(|n| n.has_tag_name((WS, "regions")))
            .flat_map(|n| n.children().filter(|n| n.has_tag_name((WS, "region"))))
        {
            if let Some(id) = region.attribute("region-id")
                && source_owners.insert(id, region).is_some()
            {
                duplicates.insert(id);
            }
        }
        for owner in &document.regions {
            if !matches!(owner.content, CanvasContent::Math(_)) {
                continue;
            }
            let Some(id) = owner.source_region.as_deref() else {
                continue;
            };
            let read = || -> Option<MatrixResultViewport> {
                if duplicates.contains(id) {
                    return None;
                }
                let region = source_owners.get(id)?;
                let mut maths = region.children().filter(|n| n.has_tag_name((WS, "math")));
                let math = maths.next()?;
                if maths.next().is_some() {
                    return None;
                }
                let local = mathfolio_document::matrix_result_format::read_local(math).ok()?;
                // A complete local Size is sufficient even if no worksheet
                // settings part exists. Missing size remains unresolved.
                Some(mathfolio_document::matrix_result_format::inherit(
                    &local,
                    &state.defaults.unwrap_or_default(),
                ))
            };
            state.owners.insert(owner.id, read());
        }
        state
    }
    pub(super) fn effective(
        &self,
        owner: u64,
        request: Option<MatrixResultViewport>,
    ) -> Option<MatrixResultViewport> {
        let base = self.owners.get(&owner).copied().unwrap_or(self.defaults)?;
        Some(mathfolio_document::matrix_result_format::inherit(
            &request.unwrap_or_default(),
            &base,
        ))
    }
    pub(super) fn window(
        &self,
        owner: u64,
        request: Option<MatrixResultViewport>,
    ) -> Option<MatrixWindow> {
        let requested = self.effective(owner, request)?;
        let size = requested.size?;
        let offset = requested
            .offset
            .unwrap_or(MatrixViewOffset { row: 0, col: 0 });
        Some(MatrixWindow {
            rows: size.rows as usize,
            cols: size.cols as usize,
            row_offset: offset.row as usize,
            col_offset: offset.col as usize,
        })
    }
}

pub(super) fn cached_formula_display<'a>(
    cache: &'a mut HashMap<u64, CachedDisplay>,
    owner: u64,
    format: MathFormat,
    result: Option<&RegionEvaluation>,
    saved: Option<&SavedCalculationRow>,
    window: Option<MatrixWindow>,
) -> Option<formula::FormulaResult<'a>> {
    let entry = cache
        .entry(owner)
        .or_insert_with(|| CachedDisplay::new(format));
    entry.matrix.prepare(result, saved);
    if entry.matrix.is_matrix() && window.is_none() {
        return Some(formula::FormulaResult::Text(
            "행렬 결과의 표시 형식을 확인할 수 없습니다",
        ));
    }
    if let Some(window) = window {
        entry.matrix.project(window, format, result, saved);
        if entry.matrix.is_matrix() {
            // A rejected requested view must not display plausible top-left
            // legacy cells under that request or offset.
            return entry
                .matrix
                .value()
                .map(formula::FormulaResult::MatrixViewport)
                .or_else(|| entry.matrix.error().map(formula::FormulaResult::Text));
        }
    }
    entry.prepare_legacy(format, result, saved);
    entry.value.as_ref().map(formula::FormulaResult::Formatted)
}

pub(super) struct Drag {
    owner: u64,
    before: Option<MatrixResultViewport>,
    initial_size: [usize; 2],
    initial_offset: [usize; 2],
    preview: MatrixResultViewport,
    inner_resize_base: Option<MatrixResultViewport>,
}
impl Drag {
    pub(super) fn owner(&self) -> u64 {
        self.owner
    }
}

pub(super) fn requested_view(
    views: &mathfolio_document::canvas::MatrixResultViews,
    drag: Option<&Drag>,
    owner: u64,
) -> Option<MatrixResultViewport> {
    drag.filter(|d| d.owner == owner)
        .map(|d| d.preview)
        .or_else(|| views.get(&MatrixResultTarget::math(owner)).copied())
}

impl WorksheetApp {
    /// Resolve a matrix view before a Math clone loses its native source owner.
    /// No document/history/cache source changes; preflight may refresh derived
    /// cells for the same current request. Scalar duplication remains unchanged.
    pub(super) fn matrix_view_for_duplicate(
        &mut self,
        owner: u64,
    ) -> Result<Option<MatrixResultViewport>, String> {
        let region = self
            .document
            .regions
            .iter()
            .find(|r| r.id == owner)
            .ok_or("Missing matrix duplicate source owner")?;
        let native = region.source_region.is_some();
        let format = region.format;
        let requested = self
            .document
            .matrix_result_views
            .get(&MatrixResultTarget::math(owner))
            .copied();
        if requested.is_none() && !native {
            return Ok(None);
        }
        let effective = self.matrix_view_sources.effective(owner, requested);
        let window = self.matrix_view_sources.window(owner, requested);
        let shown = cached_formula_display(
            &mut self.display_cache,
            owner,
            format,
            self.results.get(&owner),
            self.saved_results.get(&owner),
            window,
        );
        if let Some(formula::FormulaResult::Text(error)) = shown {
            return Err(error.to_owned());
        }
        let entry = self
            .display_cache
            .get_mut(&owner)
            .ok_or("Complete matrix result is unavailable")?;
        if entry.matrix.shape().is_none() {
            return if requested.is_some() {
                Err("Cannot duplicate a matrix view without its complete matrix result".into())
            } else {
                Ok(None)
            };
        }
        let window = window.ok_or("Native matrix result format is unresolved")?;
        entry.matrix.preflight(window, format)?;
        // Own requested None properties keep their prior inheritance. A native
        // source-local format must become an explicit known effective request,
        // because the cloned region intentionally has no native source_region.
        Ok(if native { effective } else { requested })
    }
    pub(super) fn requested_matrix_view(&self, owner: u64) -> Option<MatrixResultViewport> {
        if let Some(drag) = &self.matrix_view_drag
            && drag.owner == owner
        {
            return Some(drag.preview);
        }
        self.document
            .matrix_result_views
            .get(&MatrixResultTarget::math(owner))
            .copied()
    }

    pub(super) fn set_matrix_result_view(
        &mut self,
        owner: u64,
        viewport: Option<MatrixResultViewport>,
    ) -> Result<bool, String> {
        let target = MatrixResultTarget::math(owner);
        if !self
            .document
            .regions
            .iter()
            .any(|region| region.id == owner && matches!(region.content, CanvasContent::Math(_)))
        {
            return Err("Matrix result view requires an existing Math owner".into());
        }
        if self.document.matrix_result_views.get(&target) == viewport.as_ref() {
            return Ok(false);
        }
        let mut next = self.document.matrix_result_views.clone();
        if let Some(viewport) = viewport {
            next.set(target, viewport)?;
        } else {
            next.remove(&target);
        }
        next.validate_for_regions(&self.document.regions)?;
        self.document.matrix_result_views = next;
        self.invalidate_matrix_projection(owner);
        Ok(true)
    }

    pub(super) fn invalidate_matrix_projection(&mut self, owner: u64) {
        // Retain the fully admitted source/value. Only derived cells/geometry
        // change; no calculator, source commit, pending state or worker call.
        // The projection key includes requested window and numeric style.
        // Keep a successful preflight's cells; a cancel/history restore changes
        // that key naturally. Complete value admission remains untouched.
        self.region_heights.remove(&owner);
        self.region_widths.remove(&owner);
    }

    pub(super) fn matrix_view_gesture(&mut self, owner: u64, gesture: formula::MatrixViewGesture) {
        use formula::MatrixViewGesture;
        match gesture {
            MatrixViewGesture::Begin { size, offset }
            | MatrixViewGesture::BeginInnerResize { size, offset } => {
                let inner_resize = matches!(gesture, MatrixViewGesture::BeginInnerResize { .. });
                self.cancel_matrix_view_drag();
                if !self.document.regions.iter().any(|region| {
                    region.id == owner && matches!(region.content, CanvasContent::Math(_))
                }) {
                    return;
                }
                let before = self
                    .document
                    .matrix_result_views
                    .get(&MatrixResultTarget::math(owner))
                    .copied();
                let inner_resize_base = if inner_resize {
                    let inherited =
                        before.or_else(|| self.matrix_view_sources.effective(owner, None));
                    let Some(inherited) = inherited else {
                        return;
                    };
                    if before.is_none()
                        && (inherited.size.is_none()
                            || inherited.offset.is_none()
                            || inherited.show_indices.is_none())
                    {
                        // New explicit-pair mode refuses an unknown inherited
                        // base; the old sparse pan/bracket profile is untouched.
                        return;
                    }
                    Some(inherited)
                } else {
                    None
                };
                self.selected = Some(owner);
                self.insertion = None;
                self.matrix_view_drag = Some(Drag {
                    owner,
                    preview: before.unwrap_or_default(),
                    before,
                    initial_size: size,
                    initial_offset: offset,
                    inner_resize_base,
                });
            }
            MatrixViewGesture::Preview { size, offset }
            | MatrixViewGesture::Commit { size, offset }
            | MatrixViewGesture::PreviewInnerResize { size, offset }
            | MatrixViewGesture::CommitInnerResize { size, offset } => {
                let inner_resize = matches!(
                    gesture,
                    MatrixViewGesture::PreviewInnerResize { .. }
                        | MatrixViewGesture::CommitInnerResize { .. }
                );
                let commit = matches!(
                    gesture,
                    MatrixViewGesture::Commit { .. } | MatrixViewGesture::CommitInnerResize { .. }
                );
                let Some(drag) = self.matrix_view_drag.as_ref().filter(|d| d.owner == owner) else {
                    return;
                };
                if inner_resize != drag.inner_resize_base.is_some() {
                    self.cancel_matrix_view_drag();
                    return;
                }
                let before = drag.before;
                let old_preview = drag.preview;
                let changed = size != drag.initial_size || offset != drag.initial_offset;
                if inner_resize && !changed && commit {
                    // A captured clone is not itself a user change. Returning
                    // to the initial window also restores original inheritance.
                    self.cancel_matrix_view_drag();
                    return;
                }
                let mut next = if inner_resize && changed {
                    drag.inner_resize_base.unwrap()
                } else {
                    before.unwrap_or_default()
                };
                if size != drag.initial_size || (inner_resize && changed) {
                    let Ok(rows) = u32::try_from(size[0]) else {
                        return;
                    };
                    let Ok(cols) = u32::try_from(size[1]) else {
                        return;
                    };
                    next.size = Some(MatrixViewSize { rows, cols });
                }
                if offset != drag.initial_offset || (inner_resize && changed) {
                    let Ok(row) = u32::try_from(offset[0]) else {
                        return;
                    };
                    let Ok(col) = u32::try_from(offset[1]) else {
                        return;
                    };
                    next.offset = Some(MatrixViewOffset { row, col });
                }
                // Validate before exposing even an ephemeral preview.
                let target = MatrixResultTarget::math(owner);
                let mut probe = self.document.matrix_result_views.clone();
                if let Err(error) = probe.set(target.clone(), next) {
                    self.status = error;
                    self.cancel_matrix_view_drag();
                    return;
                }
                let Some(window) = self.matrix_view_sources.window(owner, Some(next)) else {
                    self.cancel_matrix_view_drag();
                    return;
                };
                let format = self
                    .document
                    .regions
                    .iter()
                    .find(|r| r.id == owner)
                    .map(|r| r.format);
                let admission = self
                    .display_cache
                    .get_mut(&owner)
                    .zip(format)
                    .ok_or_else(|| "Complete matrix result is unavailable".to_owned())
                    .and_then(|(entry, format)| entry.matrix.preflight(window, format));
                if let Err(error) = admission {
                    self.status = error;
                    self.cancel_matrix_view_drag();
                    return;
                }
                if !commit {
                    if old_preview != next {
                        self.matrix_view_drag.as_mut().unwrap().preview = next;
                        self.invalidate_matrix_projection(owner);
                    }
                    return;
                }
                let value = if before.is_none() && next == MatrixResultViewport::default() {
                    None
                } else {
                    Some(next)
                };
                self.matrix_view_drag = None;
                let history = self.snapshot_with_document(DocumentHistory::MatrixResultView {
                    target,
                    viewport: before,
                });
                match self.set_matrix_result_view(owner, value) {
                    Ok(true) => self.remember(history),
                    Ok(false) => self.invalidate_matrix_projection(owner),
                    Err(error) => self.status = error,
                }
            }
            MatrixViewGesture::Cancel => self.cancel_matrix_view_drag(),
        }
    }

    pub(super) fn cancel_matrix_view_drag(&mut self) {
        if let Some(drag) = self.matrix_view_drag.take() {
            self.invalidate_matrix_projection(drag.owner);
        }
    }

    pub(super) fn finish_matrix_view_frame(&mut self, painted: Option<u64>) {
        if self
            .matrix_view_drag
            .as_ref()
            .is_some_and(|drag| painted != Some(drag.owner) || self.selected != Some(drag.owner))
        {
            self.cancel_matrix_view_drag();
        }
    }
}
