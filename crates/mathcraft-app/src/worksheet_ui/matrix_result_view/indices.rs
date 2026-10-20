//! Result-only decoration; never consult current/final calculator Scope.
use super::*;

pub(in crate::worksheet_ui) fn indexed_formula_display<'a>(
    display: Option<formula::FormulaResult<'a>>,
    metadata: Option<(i32, bool)>,
) -> Result<Option<formula::FormulaResult<'a>>, String> {
    let projection = match display {
        Some(formula::FormulaResult::MatrixViewport(projection))
        | Some(formula::FormulaResult::MatrixViewportWithIndices { projection, .. }) => projection,
        _ => return Ok(display),
    };
    let Some((origin, show_indices)) = metadata else {
        return Ok(Some(formula::FormulaResult::MatrixViewport(projection)));
    };
    let indices = formula::MatrixIndexLabels::for_projection(projection, origin, show_indices)?;
    Ok(Some(formula::FormulaResult::MatrixViewportWithIndices {
        projection,
        indices,
    }))
}

impl WorksheetApp {
    /// A callback can lag behind App invalidation. Its old numeric cells stay
    /// visible, but old result provenance must not be published as current.
    pub(in crate::worksheet_ui) fn result_matrix_indices(&self, owner: u64) -> Option<(i32, bool)> {
        let region = self.document.regions.iter().find(|r| r.id == owner)?;
        if !matches!(region.content, CanvasContent::Math(_)) {
            return None;
        }
        result_indices_metadata(
            &self.matrix_view_sources,
            owner,
            region.calculation_disabled || self.pending_regions.contains(&owner),
            self.results.get(&owner),
            self.requested_matrix_view(owner),
        )
    }
}

pub(in crate::worksheet_ui) fn result_indices_metadata(
    sources: &SourceFormats,
    owner: u64,
    pending: bool,
    row: Option<&RegionEvaluation>,
    request: Option<MatrixResultViewport>,
) -> Option<(i32, bool)> {
    if pending {
        return None;
    }
    let row = row?;
    if row.error.is_some()
        || !matches!(
            row.display_value.as_ref().or(row.value.as_ref()),
            Some(mathcraft_compat::Value::Matrix { .. })
        )
    {
        return None;
    }
    Some((
        row.matrix_origin?,
        sources.effective(owner, request)?.show_indices?,
    ))
}
