//! Atomic primary-axis structural edits for the canonical subset in analysis/0158.
//! No evaluation or style selector. Source IDs and existing appearance payloads
//! survive min-index membership changes; native runtime identities are not stored.
use super::{PlotAxisEquation, PlotAxisLists, PlotExpression, PlotSpec, PlotTraceAppearance};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlotAxis {
    X,
    Y,
}
#[derive(Clone, Debug, PartialEq)]
pub enum PlotAxisEquationChange {
    Inserted,
    Removed { equation: PlotAxisEquation },
    Cleared { previous_source: PlotExpression },
}
#[derive(Clone, Debug, PartialEq)]
pub enum PlotAxisTraceChange {
    Unchanged,
    Inserted {
        index: usize,
        appearance: PlotTraceAppearance,
    },
    Removed {
        index: usize,
        appearance: PlotTraceAppearance,
    },
}
#[derive(Clone, Debug, PartialEq)]
pub struct PlotAxisEditOutcome {
    pub axis: PlotAxis,
    pub index: usize,
    pub equation_id: u64,
    pub change: PlotAxisEquationChange,
    pub trace_change: PlotAxisTraceChange,
    pub migrated: bool,
}
impl PlotAxisEditOutcome {
    pub fn trace_count_delta(&self) -> i8 {
        match self.trace_change {
            PlotAxisTraceChange::Unchanged => 0,
            PlotAxisTraceChange::Inserted { .. } => 1,
            PlotAxisTraceChange::Removed { .. } => -1,
        }
    }
}
fn equations(lists: &PlotAxisLists, axis: PlotAxis) -> &[PlotAxisEquation] {
    match axis {
        PlotAxis::X => &lists.x,
        PlotAxis::Y => &lists.y,
    }
}
fn equations_mut(lists: &mut PlotAxisLists, axis: PlotAxis) -> &mut Vec<PlotAxisEquation> {
    match axis {
        PlotAxis::X => &mut lists.x,
        PlotAxis::Y => &mut lists.y,
    }
}
impl PlotSpec {
    pub fn axis_equation_count(&self, axis: PlotAxis) -> usize {
        self.axis_lists()
            .map_or_else(|| self.trace_count(), |lists| equations(lists, axis).len())
    }
    /// Callers may consult this before reserving their own appearance. The
    /// mutation checks it again, so a stale optional appearance cannot be used.
    pub fn axis_insertion_requires_trace(&self, axis: PlotAxis) -> Result<bool, String> {
        self.validate()?;
        let source = self.axis_equation_count(axis);
        let other = self.axis_equation_count(match axis {
            PlotAxis::X => PlotAxis::Y,
            PlotAxis::Y => PlotAxis::X,
        });
        Ok(source >= other)
    }
    pub fn insert_axis_equation(
        &mut self,
        axis: PlotAxis,
        index: usize,
        source: impl Into<String>,
        new_appearance: Option<PlotTraceAppearance>,
    ) -> Result<PlotAxisEditOutcome, String> {
        self.validate()?;
        let count = self.axis_equation_count(axis);
        if index > count {
            return Err("Plot equation insertion index is out of bounds".into());
        }
        if count >= Self::MAX_TRACES {
            return Err("Plot axis equation count exceeds 16".into());
        }
        let requires_trace = self.axis_insertion_requires_trace(axis)?;
        if requires_trace != new_appearance.is_some() {
            return Err(
                "New plot appearance must be supplied exactly when a trace is added".into(),
            );
        }
        let source = source.into();
        if source.len() > 4096 {
            return Err("Plot expression exceeds 4096 bytes".into());
        }
        let mut candidate = self.clone();
        let migrated = candidate.promote_axis_lists()?;
        let lists = candidate.axis_lists_mut().unwrap();
        let id = lists.allocate_equation_id()?;
        equations_mut(lists, axis).insert(
            index,
            PlotAxisEquation {
                id,
                source: source.into(),
                scaling: None,
            },
        );
        let trace_change = if let Some(appearance) = new_appearance {
            lists.appearances.insert(index, appearance.clone());
            PlotAxisTraceChange::Inserted { index, appearance }
        } else {
            PlotAxisTraceChange::Unchanged
        };
        candidate.validate()?;
        *self = candidate;
        Ok(PlotAxisEditOutcome {
            axis,
            index,
            equation_id: id,
            change: PlotAxisEquationChange::Inserted,
            trace_change,
            migrated,
        })
    }
    /// Replace one source without changing its identity, scale or appearance.
    pub fn set_axis_expression(
        &mut self,
        axis: PlotAxis,
        index: usize,
        source: PlotExpression,
    ) -> Result<bool, String> {
        self.validate()?;
        source.validate()?;
        if index >= self.axis_equation_count(axis) {
            return Err("Plot equation source index out of bounds".into());
        }
        let mut candidate = self.clone();
        candidate.promote_axis_lists()?;
        let target = &mut equations_mut(candidate.axis_lists_mut().unwrap(), axis)[index].source;
        if *target == source {
            return Ok(false);
        }
        *target = source;
        candidate.validate()?;
        *self = candidate;
        Ok(true)
    }
    pub fn set_axis_expression_text(
        &mut self,
        axis: PlotAxis,
        index: usize,
        source: impl Into<String>,
    ) -> Result<bool, String> {
        self.set_axis_expression(axis, index, PlotExpression::LegacyText(source.into()))
    }
    pub fn insert_typed_axis_equation(
        &mut self,
        axis: PlotAxis,
        index: usize,
        tree: mathfolio_editor::PortableTree,
        new_appearance: Option<PlotTraceAppearance>,
    ) -> Result<PlotAxisEditOutcome, String> {
        let source = PlotExpression::from_tree(tree)?;
        let mut candidate = self.clone();
        let outcome = candidate.insert_axis_equation(axis, index, "", new_appearance)?;
        equations_mut(candidate.axis_lists_mut().unwrap(), axis)[index].source = source;
        candidate.validate()?;
        *self = candidate;
        Ok(outcome)
    }
    pub fn remove_axis_equation(
        &mut self,
        axis: PlotAxis,
        index: usize,
    ) -> Result<PlotAxisEditOutcome, String> {
        self.validate()?;
        let count = self.axis_equation_count(axis);
        if index >= count {
            return Err("Plot equation removal index is out of bounds".into());
        }
        if count == 1 {
            return Err("A plot axis must retain one equation; clear its source instead".into());
        }
        let other = self.axis_equation_count(match axis {
            PlotAxis::X => PlotAxis::Y,
            PlotAxis::Y => PlotAxis::X,
        });
        let mut candidate = self.clone();
        let migrated = candidate.promote_axis_lists()?;
        let lists = candidate.axis_lists_mut().unwrap();
        let equation = equations_mut(lists, axis).remove(index);
        let trace_change = if count > other {
            PlotAxisTraceChange::Removed {
                index,
                appearance: lists.appearances.remove(index),
            }
        } else {
            PlotAxisTraceChange::Unchanged
        };
        candidate.validate()?;
        let equation_id = equation.id;
        *self = candidate;
        Ok(PlotAxisEditOutcome {
            axis,
            index,
            equation_id,
            change: PlotAxisEquationChange::Removed { equation },
            trace_change,
            migrated,
        })
    }
    /// PH is an empty expression in this supported string model. Scaling/units
    /// and native empty-key predicates remain the caller's responsibility.
    /// None is a true no-op, including no legacy migration or ID allocation.
    pub fn clear_axis_equation(
        &mut self,
        axis: PlotAxis,
        index: usize,
    ) -> Result<Option<PlotAxisEditOutcome>, String> {
        self.validate()?;
        if index >= self.axis_equation_count(axis) {
            return Err("Plot equation clear index is out of bounds".into());
        }
        let mut candidate = self.clone();
        let migrated = candidate.promote_axis_lists()?;
        let equation = &mut equations_mut(candidate.axis_lists_mut().unwrap(), axis)[index];
        if equation.source.is_exact_placeholder() {
            return Ok(None);
        }
        let id = equation.id;
        let cleared = if equation.source.as_tree().is_some() {
            PlotExpression::from_tree(mathfolio_editor::PortableTree {
                kind: mathfolio_editor::NodeKind::Placeholder,
                children: vec![],
            })?
        } else {
            PlotExpression::default()
        };
        let previous_source = std::mem::replace(&mut equation.source, cleared);
        candidate.validate()?;
        *self = candidate;
        Ok(Some(PlotAxisEditOutcome {
            axis,
            index,
            equation_id: id,
            change: PlotAxisEquationChange::Cleared { previous_source },
            trace_change: PlotAxisTraceChange::Unchanged,
            migrated,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::{PlotSource, PlotTraceSpec};
    fn appearance(t: usize) -> PlotTraceAppearance {
        PlotTraceAppearance {
            label: format!("T{t}"),
            ..Default::default()
        }
    }
    fn fixture(n: usize, m: usize) -> PlotSpec {
        PlotSpec {
            source: PlotSource::axis_lists(
                (0..n).map(|i| format!("x{i}")).collect(),
                (0..m).map(|i| format!("y{i}")).collect(),
                (0..n.max(m)).map(appearance).collect(),
            )
            .unwrap(),
            ..Default::default()
        }
    }
    fn ids(spec: &PlotSpec, axis: PlotAxis) -> Vec<u64> {
        equations(spec.axis_lists().unwrap(), axis)
            .iter()
            .map(|e| e.id)
            .collect()
    }
    fn labels(spec: &PlotSpec) -> Vec<String> {
        spec.axis_lists()
            .unwrap()
            .appearances
            .iter()
            .map(|a| a.label.clone())
            .collect()
    }
    // Independent CIL translation: mutable per-equation memberships and global
    // append order. It does not implement the product's Vec appearance formula.
    #[derive(Clone)]
    struct Reference {
        axes: [Vec<Vec<usize>>; 2],
        global: Vec<usize>,
    }
    impl Reference {
        fn new(n: usize, m: usize, global: Vec<usize>) -> Self {
            let mut axes = [vec![vec![]; n], vec![vec![]; m]];
            for t in 0..n.max(m) {
                axes[0][t.min(n - 1)].push(t);
                axes[1][t.min(m - 1)].push(t);
            }
            Self { axes, global }
        }
        fn owner(&self, a: usize, t: usize) -> usize {
            self.axes[a]
                .iter()
                .position(|members| members.contains(&t))
                .unwrap()
        }
        fn sorted(&self) -> Vec<usize> {
            let a = usize::from(self.axes[0].len() <= self.axes[1].len());
            let mut order = self.global.clone();
            order.sort_by_key(|t| self.owner(a, *t));
            order
        }
        fn rearrange(&mut self, a: usize) {
            for i in 0..self.axes[a].len() - 1 {
                if self.axes[a][i].is_empty() && !self.axes[a][i + 1].is_empty() {
                    let t = self.axes[a][i + 1].remove(0);
                    self.axes[a][i].push(t);
                } else if self.axes[a][i].len() > 1 {
                    let rest = self.axes[a][i].split_off(1);
                    for (j, t) in rest.into_iter().enumerate() {
                        self.axes[a][i + 1].insert(j, t);
                    }
                }
            }
        }
        fn insert(&mut self, a: usize, k: usize) -> Option<usize> {
            let (n, m) = (self.axes[a].len(), self.axes[1 - a].len());
            self.axes[a].insert(k, vec![]);
            if n >= m {
                let t = self.global.iter().max().unwrap() + 1;
                self.global.push(t);
                self.axes[a][k].push(t);
                self.axes[1 - a][k.min(m - 1)].insert(k.saturating_sub(m - 1), t);
                self.rearrange(1 - a);
                Some(t)
            } else {
                self.rearrange(a);
                None
            }
        }
        fn remove(&mut self, a: usize, k: usize) -> Option<usize> {
            let (n, m) = (self.axes[a].len(), self.axes[1 - a].len());
            let members = self.axes[a].remove(k);
            if n > m {
                assert_eq!(members.len(), 1);
                let t = members[0];
                let other = self.owner(1 - a, t);
                let p = self.axes[1 - a][other]
                    .iter()
                    .position(|v| *v == t)
                    .unwrap();
                self.axes[1 - a][other].remove(p);
                self.global.retain(|v| *v != t);
                self.rearrange(1 - a);
                Some(t)
            } else {
                let last = self.axes[a].len() - 1;
                for t in members {
                    if k > last {
                        self.axes[a][last].push(t)
                    } else {
                        self.axes[a][k].insert(0, t)
                    }
                }
                self.rearrange(a);
                None
            }
        }
        fn compare(&self, spec: &PlotSpec) {
            let order = self.sorted();
            assert_eq!(
                labels(spec),
                order.iter().map(|t| format!("T{t}")).collect::<Vec<_>>()
            );
            assert_eq!(spec.trace_count(), self.global.len());
            let pairs = spec.resolved_traces().unwrap();
            let lists = spec.axis_lists().unwrap();
            for (i, t) in order.iter().enumerate() {
                assert_eq!(
                    pairs[i].x_expression,
                    lists.x[self.owner(0, *t)].source.as_legacy_text().unwrap()
                );
                assert_eq!(
                    pairs[i].y_expression,
                    lists.y[self.owner(1, *t)].source.as_legacy_text().unwrap()
                );
            }
            for a in 0..2 {
                for (i, members) in self.axes[a].iter().enumerate() {
                    assert_eq!(
                        *members,
                        order
                            .iter()
                            .copied()
                            .filter(|t| self.owner(a, *t) == i)
                            .collect::<Vec<_>>()
                    );
                }
            }
        }
    }
    fn permutations(values: &mut [usize], start: usize, out: &mut Vec<Vec<usize>>) {
        if start == values.len() {
            out.push(values.to_vec());
            return;
        }
        for i in start..values.len() {
            values.swap(i, start);
            permutations(values, start + 1, out);
            values.swap(i, start);
        }
    }
    #[test]
    fn every_bounded_cil_membership_state_and_global_order_matches_actual_model_edits() {
        let (mut states, mut transitions) = (0, 0);
        for n in 1..=4 {
            for m in 1..=4 {
                let mut orders = vec![];
                permutations(&mut (0..n.max(m)).collect::<Vec<_>>(), 0, &mut orders);
                for order in orders {
                    states += 1;
                    let reference = Reference::new(n, m, order);
                    for (a, axis) in [PlotAxis::X, PlotAxis::Y].into_iter().enumerate() {
                        let count = if a == 0 { n } else { m };
                        if count < 4 {
                            for k in 0..=count {
                                let mut r = reference.clone();
                                let added = r.insert(a, k);
                                let mut spec = fixture(n, m);
                                let old_ids = ids(&spec, axis);
                                let old_next = spec.axis_lists().unwrap().next_equation_id;
                                let outcome = spec
                                    .insert_axis_equation(axis, k, "", added.map(appearance))
                                    .unwrap();
                                r.compare(&spec);
                                assert_eq!(outcome.trace_count_delta(), i8::from(added.is_some()));
                                assert_eq!(outcome.equation_id, old_next);
                                assert!(!outcome.migrated);
                                let mut expected = old_ids;
                                expected.insert(k, old_next);
                                assert_eq!(ids(&spec, axis), expected);
                                transitions += 1;
                            }
                        }
                        if count > 1 {
                            for k in 0..count {
                                let mut r = reference.clone();
                                let removed = r.remove(a, k);
                                let mut spec = fixture(n, m);
                                let old_ids = ids(&spec, axis);
                                let old_next = spec.axis_lists().unwrap().next_equation_id;
                                let outcome = spec.remove_axis_equation(axis, k).unwrap();
                                r.compare(&spec);
                                assert_eq!(
                                    outcome.trace_count_delta(),
                                    -i8::from(removed.is_some())
                                );
                                assert_eq!(outcome.equation_id, old_ids[k]);
                                assert_eq!(spec.axis_lists().unwrap().next_equation_id, old_next);
                                let mut expected = old_ids;
                                expected.remove(k);
                                assert_eq!(ids(&spec, axis), expected);
                                transitions += 1;
                            }
                        }
                    }
                }
            }
        }
        assert_eq!((states, transitions), (205, 1828));
    }
    #[test]
    fn failure_is_atomic_for_indices_optional_appearance_budgets_and_allocator() {
        let base = fixture(2, 2);
        let mut failures = vec![
            (
                base.clone(),
                PlotAxis::X,
                3,
                "p".into(),
                Some(appearance(2)),
            ),
            (base.clone(), PlotAxis::X, 1, "p".into(), None),
            (
                fixture(1, 2),
                PlotAxis::X,
                0,
                "p".into(),
                Some(appearance(2)),
            ),
            (
                base.clone(),
                PlotAxis::X,
                1,
                "한".repeat(1366),
                Some(appearance(2)),
            ),
            (
                fixture(16, 1),
                PlotAxis::X,
                0,
                "p".into(),
                Some(appearance(16)),
            ),
        ];
        let mut overflow = base.clone();
        overflow.axis_lists_mut().unwrap().next_equation_id = u64::MAX;
        failures.push((overflow, PlotAxis::Y, 1, "p".into(), Some(appearance(2))));
        let bad = PlotTraceAppearance {
            label: "a".repeat(257),
            ..Default::default()
        };
        failures.push((base.clone(), PlotAxis::Y, 0, "p".into(), Some(bad)));
        for (mut spec, axis, k, source, app) in failures {
            let before = serde_json::to_vec(&spec).unwrap();
            assert!(spec.insert_axis_equation(axis, k, source, app).is_err());
            assert_eq!(serde_json::to_vec(&spec).unwrap(), before);
        }
        for (mut spec, axis, k) in [(fixture(1, 2), PlotAxis::X, 0), (base, PlotAxis::Y, 2)] {
            let before = spec.clone();
            assert!(spec.remove_axis_equation(axis, k).is_err());
            assert_eq!(spec, before);
        }
    }
    #[test]
    fn legacy_migration_is_in_the_successful_transaction_and_equal_text_never_merges() {
        let mut spec = PlotSpec {
            source: PlotSource::legacy("unused x", "unused y", vec![PlotTraceSpec::default(); 2]),
            ..Default::default()
        };
        let old = serde_json::to_vec(&spec).unwrap();
        assert!(spec.insert_axis_equation(PlotAxis::X, 1, "", None).is_err());
        assert_eq!(serde_json::to_vec(&spec).unwrap(), old);
        let outcome = spec
            .insert_axis_equation(PlotAxis::X, 1, "", Some(appearance(2)))
            .unwrap();
        assert!(outcome.migrated);
        assert_eq!(outcome.trace_count_delta(), 1);
        let graph = spec.axis_lists().unwrap();
        assert_eq!(graph.x.len(), 3);
        assert_eq!(graph.y.len(), 2);
        assert_eq!(graph.x[0].source, graph.x[2].source);
        assert_ne!(graph.x[0].id, graph.x[2].id);
        assert_eq!(graph.x[1].id, outcome.equation_id);
        let high = graph.next_equation_id;
        spec.remove_axis_equation(PlotAxis::X, 1).unwrap();
        assert_eq!(spec.axis_lists().unwrap().next_equation_id, high);
        let next = spec
            .insert_axis_equation(PlotAxis::X, 2, "", Some(appearance(3)))
            .unwrap();
        assert_eq!(next.equation_id, high);
    }
    #[test]
    fn unique_placeholder_clear_preserves_identity_appearance_and_noop_wire() {
        let mut spec = fixture(1, 2);
        let id = ids(&spec, PlotAxis::X)[0];
        let apps = labels(&spec);
        let next = spec.axis_lists().unwrap().next_equation_id;
        assert!(spec.remove_axis_equation(PlotAxis::X, 0).is_err());
        let result = spec.clear_axis_equation(PlotAxis::X, 0).unwrap().unwrap();
        assert_eq!(result.equation_id, id);
        assert_eq!(result.trace_count_delta(), 0);
        assert_eq!(spec.primary_x_expression().as_legacy_text().unwrap(), "");
        assert_eq!(labels(&spec), apps);
        assert_eq!(spec.axis_lists().unwrap().next_equation_id, next);
        let old = serde_json::to_vec(&spec).unwrap();
        assert!(spec.clear_axis_equation(PlotAxis::X, 0).unwrap().is_none());
        assert_eq!(serde_json::to_vec(&spec).unwrap(), old);
        let mut legacy = PlotSpec {
            source: PlotSource::single("", "sin(x)"),
            ..Default::default()
        };
        let old = serde_json::to_vec(&legacy).unwrap();
        assert!(
            legacy
                .clear_axis_equation(PlotAxis::X, 0)
                .unwrap()
                .is_none()
        );
        assert_eq!(serde_json::to_vec(&legacy).unwrap(), old);
    }
    #[test]
    fn sequence_source_snapshots_restore_exact_ids_appearances_and_wire_without_selecting_again() {
        let mut spec = fixture(1, 1);
        let mut reference = Reference::new(1, 1, vec![0]);
        let mut snapshots = vec![spec.clone()];
        for (a, k, insert) in [
            (1, 0, true),
            (0, 0, true),
            (1, 1, false),
            (1, 1, true),
            (0, 0, false),
        ] {
            let axis = if a == 0 { PlotAxis::X } else { PlotAxis::Y };
            if insert {
                let t = reference.insert(a, k);
                spec.insert_axis_equation(axis, k, "", t.map(appearance))
                    .unwrap();
            } else {
                reference.remove(a, k);
                spec.remove_axis_equation(axis, k).unwrap();
            }
            reference.compare(&spec);
            snapshots.push(spec.clone());
        }
        // Undo and Redo use stored source snapshots, not inverse edit commands.
        let wires: Vec<_> = snapshots
            .iter()
            .map(|s| serde_json::to_vec(s).unwrap())
            .collect();
        for i in (0..snapshots.len()).rev() {
            spec = snapshots[i].clone();
            assert_eq!(serde_json::to_vec(&spec).unwrap(), wires[i]);
        }
        for (i, snapshot) in snapshots.into_iter().enumerate() {
            spec = snapshot;
            assert_eq!(serde_json::to_vec(&spec).unwrap(), wires[i]);
        }
    }
    #[test]
    fn sixteen_slot_boundaries_and_shorter_to_equal_preserve_source_and_styles() {
        for axis in [PlotAxis::X, PlotAxis::Y] {
            let mut spec = if axis == PlotAxis::X {
                fixture(15, 16)
            } else {
                fixture(16, 15)
            };
            let apps = labels(&spec);
            let out = spec.insert_axis_equation(axis, 7, "", None).unwrap();
            assert_eq!(spec.axis_equation_count(axis), 16);
            assert_eq!(out.trace_count_delta(), 0);
            assert_eq!(labels(&spec), apps);
            assert!(
                spec.insert_axis_equation(axis, 0, "", Some(appearance(16)))
                    .is_err()
            );
            let out = spec.remove_axis_equation(axis, 7).unwrap();
            assert_eq!(out.trace_count_delta(), 0);
            assert_eq!(labels(&spec), apps);
            let mut spec = if axis == PlotAxis::X {
                fixture(16, 15)
            } else {
                fixture(15, 16)
            };
            let out = spec.remove_axis_equation(axis, 8).unwrap();
            assert_eq!(out.trace_count_delta(), -1);
            assert_eq!(spec.trace_count(), 15);
        }
    }
}
