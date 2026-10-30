//! Actual source/context invalidation changes each saved curve status explicitly.
use super::*;
impl WorksheetApp {
    pub(super) fn mark_regions_pending(&mut self, ids: Vec<u64>) {
        if ids.is_empty() {
            return;
        }
        let requested: HashSet<_> = ids.iter().copied().collect();
        let counts: HashMap<_, _> = self
            .document
            .regions
            .iter()
            .filter_map(|region| {
                if !requested.contains(&region.id) {
                    return None;
                }
                let CanvasContent::Plot(spec) = &region.content else {
                    return None;
                };
                Some((region.id, spec.trace_count()))
            })
            .collect();
        for (id, count) in counts {
            if let Some(Ok(data)) = self.plot_results.get_mut(&id) {
                if data.traces.len() == count {
                    data.invalidate();
                } else {
                    data.traces = (0..count)
                        .map(|_| crate::plots::TraceOutcome::Pending { previous: None })
                        .collect();
                }
                // A native-origin bundle may keep its original rows only until
                // an actual source/context invalidation, not just cold Pending.
                if let Some(row) = self.saved_results.get_mut(&id) {
                    row.numeric_source = None;
                    row.status = SavedCalculationStatus::Pending;
                }
            }
        }
        self.pending_regions.extend(ids);
    }
}

#[cfg(test)]
#[path = "plot_lifecycle_tests.rs"]
mod tests;

pub(super) fn same_plot_topology(before: &PlotSpec, after: &PlotSpec) -> bool {
    match (before.axis_lists(), after.axis_lists()) {
        (Some(a), Some(b)) => {
            a.x.iter().map(|e| e.id).eq(b.x.iter().map(|e| e.id))
                && a.y.iter().map(|e| e.id).eq(b.y.iter().map(|e| e.id))
        }
        (None, None) => before.trace_count() == after.trace_count(),
        _ => false,
    }
}
impl WorksheetApp {
    pub(super) fn discard_plot_previous_curves(&mut self, id: u64) {
        let Some(count) = self.document.regions.iter().find_map(|region| {
            if region.id == id {
                if let CanvasContent::Plot(spec) = &region.content {
                    Some(spec.trace_count())
                } else {
                    None
                }
            } else {
                None
            }
        }) else {
            return;
        };
        if let Some(Ok(data)) = self.plot_results.get_mut(&id) {
            data.traces = (0..count)
                .map(|_| crate::plots::TraceOutcome::Pending { previous: None })
                .collect();
        }
    }
}
