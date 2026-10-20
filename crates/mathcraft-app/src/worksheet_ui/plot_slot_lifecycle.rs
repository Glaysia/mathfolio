//! Preserve historical curves only across positively retained equation pairs.
//! An ordinal, equal source text or equal trace count is not an identity proof.
use super::*;
fn pair_ids(spec: &PlotSpec) -> Option<Vec<(u64, u64)>> {
    let a = spec.axis_lists()?;
    Some(
        (0..a.x.len().max(a.y.len()))
            .map(|i| (a.x[i.min(a.x.len() - 1)].id, a.y[i.min(a.y.len() - 1)].id))
            .collect(),
    )
}
pub(super) fn remap(
    old: &PlotSpec,
    new: &PlotSpec,
    data: &crate::plots::PlotData,
) -> crate::plots::PlotData {
    let pending = || crate::plots::TraceOutcome::Pending { previous: None };
    let Some(old_ids) = pair_ids(old).filter(|ids| ids.len() == data.traces.len()) else {
        return crate::plots::PlotData {
            traces: (0..new.trace_count()).map(|_| pending()).collect(),
        };
    };
    let Some(new_ids) = pair_ids(new) else {
        return crate::plots::PlotData {
            traces: (0..new.trace_count()).map(|_| pending()).collect(),
        };
    };
    let traces = new_ids
        .into_iter()
        .map(|id| {
            old_ids
                .iter()
                .position(|old| *old == id)
                .map_or_else(pending, |i| data.traces[i].clone())
        })
        .collect();
    crate::plots::PlotData { traces }
}
impl WorksheetApp {
    pub(super) fn retire_plot_widgets(&mut self, owner: Option<u64>) {
        for (id, edit) in &mut self.inline_plots {
            if owner.is_none_or(|owner| owner == *id) {
                self.retired_plot_widgets.extend(edit.retire_widgets());
                let floor = self.plot_widget_generations.entry(*id).or_default();
                *floor = (*floor).max(edit.widget_generation());
            }
        }
    }
    pub(super) fn release_retired_plot_widgets(&mut self, ctx: &egui::Context) {
        for id in self.retired_plot_widgets.drain(..) {
            ctx.data_mut(|data| data.remove::<String>(id.with("ime-preedit")));
            ctx.memory_mut(|memory| memory.surrender_focus(id));
        }
    }
    // A deleted owner can still return through either history stack. Retain its
    // runtime equation-ID floor while that owner remains reachable; do not keep
    // arbitrary deleted owner records for the entire process lifetime.
    pub(super) fn retain_plot_equation_floors(&mut self) {
        let mut owners: HashSet<_> = self.document.regions.iter().map(|r| r.id).collect();
        for entry in self.undo.iter().chain(&self.redo) {
            owners.extend(entry.document.regions().iter().map(|r| r.id));
        }
        self.plot_equation_floors
            .retain(|id, _| owners.contains(id));
        self.plot_widget_generations
            .retain(|id, _| owners.contains(id));
    }
    pub(super) fn reconcile_plot_axis_pairs(&mut self, id: u64, old: &PlotSpec, new: &PlotSpec) {
        let data = self
            .plot_results
            .get(&id)
            .and_then(|d| d.as_ref().ok())
            .map(|data| remap(old, new, data));
        self.plot_results.insert(
            id,
            Ok(data.unwrap_or_else(|| crate::plots::PlotData {
                traces: (0..new.trace_count())
                    .map(|_| crate::plots::TraceOutcome::Pending { previous: None })
                    .collect(),
            })),
        );
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use mathcraft_document::canvas::{PlotAxis, PlotSource, PlotTraceAppearance};
    fn source() -> PlotSpec {
        PlotSpec {
            source: PlotSource::axis_lists(
                vec!["x".into()],
                vec!["x".into(), "x".into()],
                vec![Default::default(); 2],
            )
            .unwrap(),
            ..Default::default()
        }
    }
    fn curves() -> crate::plots::PlotData {
        crate::plots::PlotData {
            traces: (0..2)
                .map(|i| {
                    crate::plots::TraceOutcome::Ready(crate::plots::PlotTraceData {
                        points: vec![[1., i as f64]].into(),
                        label: String::new(),
                        style: Default::default(),
                        implicit_domain: false,
                        presentation: None,
                    })
                })
                .collect(),
        }
    }
    #[test]
    fn insertion_and_removal_keep_only_retained_pair_identities_even_with_equal_text() {
        let old = source();
        let data = curves();
        let pointer = data.traces[1].curve().unwrap().points.as_ptr();
        let mut new = old.clone();
        new.insert_axis_equation(PlotAxis::Y, 0, "", Some(PlotTraceAppearance::default()))
            .unwrap();
        let mut mapped = remap(&old, &new, &data);
        assert!(mapped.traces[0].curve().is_none());
        assert_eq!(mapped.traces[2].curve().unwrap().points.as_ptr(), pointer);
        mapped.invalidate();
        assert!(
            mapped
                .traces
                .iter()
                .all(crate::plots::TraceOutcome::is_pending)
        );
        let mut removed = new.clone();
        removed.remove_axis_equation(PlotAxis::Y, 1).unwrap();
        let mapped = remap(&new, &removed, &mapped);
        assert!(mapped.traces[0].curve().is_none());
        assert_eq!(mapped.traces[1].curve().unwrap().points.as_ptr(), pointer);
    }
    #[test]
    fn shorter_axis_insertion_rebinds_pairs_without_transplanting_a_neighbor_curve() {
        let old = source();
        let data = curves();
        let mut new = old.clone();
        new.insert_axis_equation(PlotAxis::X, 0, "x", None).unwrap();
        let mapped = remap(&old, &new, &data);
        assert!(mapped.traces[0].curve().is_none());
        assert_eq!(mapped.traces[1], data.traces[1]);
        let legacy = PlotSpec::default();
        assert!(
            remap(&legacy, &new, &data)
                .traces
                .iter()
                .all(|t| t.curve().is_none())
        );
    }
}
