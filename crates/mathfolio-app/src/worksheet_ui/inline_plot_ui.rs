//! Inline axis text stays outside the committed document until a command.
use super::*;
impl WorksheetApp {
    /// Commit all dirty owners except the one still receiving inline input.
    /// Calculation is a separate caller decision, so F5 cannot submit twice.
    pub(super) fn commit_plot_edits(&mut self, keep: Option<u64>) -> Result<bool, String> {
        if let Some(error) = self
            .inline_plots
            .iter()
            .filter(|(id, _)| Some(**id) != keep)
            .find_map(|(_, state)| state.validation_error())
        {
            return Err(error.to_owned());
        }
        if self
            .inline_plots
            .iter()
            .any(|(id, state)| Some(*id) != keep && state.is_composing())
        {
            return Err("문자 조합을 먼저 확정하세요".into());
        }
        let mut ids: Vec<_> = self
            .inline_plots
            .iter()
            .filter(|(id, state)| Some(**id) != keep && state.has_changes())
            .map(|(id, _)| *id)
            .collect();
        ids.sort_unstable();
        for id in &ids {
            let CanvasContent::Plot(source) = &self
                .document
                .regions
                .iter()
                .find(|r| r.id == *id)
                .ok_or("Missing plot owner")?
                .content
            else {
                return Err("Missing plot source".into());
            };
            self.inline_plots[id].validate_source(source)?;
            let draft = self.inline_plots[id]
                .draft()
                .ok_or("Missing inline plot draft")?;
            draft.validate()?;
        }
        let mut changed = false;
        for id in ids {
            let Some(draft) = self
                .inline_plots
                .get_mut(&id)
                .and_then(crate::plots::InlinePlotEdit::take)
            else {
                continue;
            };
            let Some(original) =
                self.document.regions.iter().find(|region| {
                    region.id == id && matches!(region.content, CanvasContent::Plot(_))
                })
            else {
                continue;
            };
            if matches!(&original.content,CanvasContent::Plot(current) if current==&draft) {
                continue;
            }
            let changed_topology = matches!(&original.content,CanvasContent::Plot(current) if !super::plot_lifecycle::same_plot_topology(current,&draft));
            let before =
                self.snapshot_with_document(DocumentHistory::Region(Box::new(original.clone())));
            if changed_topology {
                let DocumentHistory::Region(original) = &before.document else {
                    unreachable!()
                };
                let CanvasContent::Plot(old) = &original.content else {
                    unreachable!()
                };
                self.reconcile_plot_axis_pairs(id, old, &draft);
            }
            self.document
                .regions
                .iter_mut()
                .find(|region| region.id == id)
                .unwrap()
                .content = CanvasContent::Plot(draft);
            self.remember(before);
            self.mark_plot_commit_pending(id);
            changed = true;
        }
        if changed {
            self.background.invalidate_reply();
        }
        Ok(changed)
    }
    pub(super) fn mark_plot_commit_pending(&mut self, id: u64) {
        if self
            .document
            .regions
            .iter()
            .any(|region| region.id == id && !region.calculation_disabled)
        {
            self.mark_regions_pending(vec![id]);
        }
        if let Some(saved) = self.saved_results.get_mut(&id) {
            saved.status = SavedCalculationStatus::Pending;
        }
        self.calculation_pending = !self.pending_regions.is_empty();
        self.last_executed_regions.clear();
    }
    /// A resize also ends the pending axis edit. Retain one region checkpoint
    /// with the original source and pre-gesture dimensions for this gesture.
    pub(super) fn begin_plot_resize(&mut self, id: u64, original_spec: PlotSpec) {
        let Some(current) = self.document.regions.iter().find(|region| region.id == id) else {
            return;
        };
        let CanvasContent::Plot(current_spec) = &current.content else {
            return;
        };
        let latest = [current_spec.width, current_spec.height];
        let mut original = current.clone();
        original.content = CanvasContent::Plot(original_spec);
        let before = self.snapshot_with_document(DocumentHistory::Region(Box::new(original)));
        let candidate = self
            .inline_plots
            .get(&id)
            .filter(|state| !state.is_composing() && state.validate_source(current_spec).is_ok())
            .and_then(crate::plots::InlinePlotEdit::draft)
            .filter(|draft| draft.validate().is_ok())
            .cloned();
        if let Some(mut draft) = candidate {
            match draft.set_outer_size(latest[0], latest[1]) {
                Ok(_) => {
                    if !super::plot_lifecycle::same_plot_topology(current_spec, &draft) {
                        let old = current_spec.clone();
                        self.reconcile_plot_axis_pairs(id, &old, &draft);
                    }
                    self.inline_plots.get_mut(&id).unwrap().take();
                    self.document
                        .regions
                        .iter_mut()
                        .find(|region| region.id == id)
                        .unwrap()
                        .content = CanvasContent::Plot(draft);
                    self.mark_plot_commit_pending(id);
                    self.background.invalidate_reply();
                }
                Err(error) => self.status = error,
            }
        }
        self.remember(before);
    }
    pub(super) fn finish_plot_edits(&mut self, keep: Option<u64>) {
        if self
            .inline_plots
            .values()
            .any(crate::plots::InlinePlotEdit::is_composing)
        {
            return;
        }
        match self.commit_plot_edits(keep) {
            Ok(true) if self.uncommitted_math.is_empty() => self.calculation_changed(false),
            Ok(true) if self.automatic_calculation => {
                self.calculate_command(CalculationCommand::Automatic)
            }
            Ok(true) => (),
            Ok(false) => (),
            Err(error) => self.status = format!("도표 입력 오류: {error}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{enter, frame};
    use super::*;
    fn key(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }
    fn fixture(automatic: bool) -> (WorksheetApp, egui::Context, u64) {
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let mut app = WorksheetApp {
            automatic_calculation: false,
            ..Default::default()
        };
        let id = app.document.add_plot(20., 20.);
        app.recalculate();
        app.persist_calculation_cache();
        app.restore_saved_calculations();
        app.last_executed_regions.clear();
        app.automatic_calculation = automatic;
        app.selected = Some(id);
        app.focus_new = Some(id);
        app.insertion = None;
        frame(&mut app, &ctx, vec![]);
        frame(&mut app, &ctx, vec![]);
        (app, ctx, id)
    }
    fn replace(app: &mut WorksheetApp, ctx: &egui::Context, text: &str) {
        frame(
            app,
            ctx,
            vec![
                key(egui::Key::A, egui::Modifiers::COMMAND),
                egui::Event::Text(text.into()),
            ],
        );
    }
    fn expression(app: &WorksheetApp, id: u64) -> &str {
        let CanvasContent::Plot(spec) = &app
            .document
            .regions
            .iter()
            .find(|r| r.id == id)
            .unwrap()
            .content
        else {
            panic!()
        };
        spec.primary_y_expression().as_legacy_text().unwrap()
    }
    #[test]
    fn retained_sixteen_trace_frames_hover_and_typing_do_not_submit_calculation() {
        let (mut app, ctx, id) = fixture(false);
        let CanvasContent::Plot(source) = &mut app
            .document
            .regions
            .iter_mut()
            .find(|r| r.id == id)
            .unwrap()
            .content
        else {
            panic!()
        };
        source.promote_legacy_trace();
        for _ in 1..PlotSpec::MAX_TRACES {
            source.add_trace(PlotTraceSpec::default()).unwrap();
        }
        app.recalculate();
        app.persist_calculation_cache();
        app.restore_saved_calculations();
        app.last_executed_regions.clear();
        // The fixture replaced a legacy list outside a UI command. Explicitly
        // focus its new generation instead of inheriting an old ordinal caret.
        app.focus_new = Some(id);
        for _ in 0..10 {
            frame(&mut app, &ctx, vec![]);
        }
        let document = app.document.clone();
        let curve = app.plot_results[&id].clone();
        assert_eq!(curve.as_ref().unwrap().traces.len(), 16);
        let saved = app.saved_results.clone();
        let generation = app.background.submitted_generation();
        let history = app.undo.len();
        for i in 0..20 {
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(egui::pos2(170. + i as f32, 200.))],
            );
        }
        frame(&mut app, &ctx, vec![egui::Event::Text("+1".into())]);
        assert!(app.inline_plots[&id].has_changes());
        for _ in 0..10 {
            frame(&mut app, &ctx, vec![]);
        }
        assert_eq!(app.document, document);
        assert_eq!(app.plot_results[&id], curve);
        assert_eq!(app.saved_results, saved);
        assert_eq!(app.background.submitted_generation(), generation);
        assert_eq!(app.undo.len(), history);
        assert!(app.pending_regions.is_empty() && app.last_executed_regions.is_empty());
        assert!(!app.background.running);
    }
    #[test]
    fn real_typing_keeps_source_curve_cache_history_and_preparation_inert_until_enter() {
        let (mut app, ctx, id) = fixture(false);
        let source = app.document.clone();
        let points = app.plot_results[&id].clone();
        let saved = app.saved_results.clone();
        replace(&mut app, &ctx, "2*x");
        for text in ["+", "1"] {
            frame(&mut app, &ctx, vec![egui::Event::Text(text.into())]);
        }
        assert!(app.inline_plots[&id].has_changes());
        assert_eq!(app.document, source);
        assert_eq!(app.plot_results[&id], points);
        assert_eq!(app.saved_results, saved);
        assert!(
            app.undo.is_empty()
                && app.last_executed_regions.is_empty()
                && app.pending_regions.is_empty()
        );
        assert!(
            app.committed_input.is_none() && !app.background.preparation.has_committed_baseline()
        );
        frame(&mut app, &ctx, vec![enter()]);
        assert_eq!(expression(&app, id), "2*x+1");
        assert_eq!(app.undo.len(), 1);
        assert!(matches!(app.undo[0].document, DocumentHistory::Region(_)));
        assert!(app.pending_regions.contains(&id) && app.last_executed_regions.is_empty());
        super::super::tests::assert_pending_curves_unchanged(&app.plot_results[&id], &points);
        app.recalculate();
        assert!(
            app.plot_results[&id]
                .as_ref()
                .unwrap()
                .first_curve()
                .unwrap()
                .points
                .iter()
                .all(|p| p[1] == 2. * p[0] + 1.)
        );
    }
    #[test]
    fn automatic_enter_calculates_once_and_unchanged_enter_keeps_history() {
        let (mut app, ctx, id) = fixture(true);
        replace(&mut app, &ctx, "2*x");
        assert!(app.last_executed_regions.is_empty());
        frame(&mut app, &ctx, vec![enter()]);
        assert_eq!(app.last_executed_regions, vec![id.to_string()]);
        assert!(app.pending_regions.is_empty());
        assert_eq!(app.undo.len(), 1);
        assert!(
            app.plot_results[&id]
                .as_ref()
                .unwrap()
                .first_curve()
                .unwrap()
                .points
                .iter()
                .all(|p| p[1] == 2. * p[0])
        );
        frame(&mut app, &ctx, vec![enter()]);
        assert_eq!(app.undo.len(), 1);
    }
    #[test]
    fn explicit_calculation_commits_before_now_fast_path_and_source_undo_resets_the_buffer() {
        for command in [CalculationCommand::Now, CalculationCommand::Worksheet] {
            let (mut app, ctx, id) = fixture(false);
            replace(&mut app, &ctx, "3*x");
            app.calculate_command(command);
            assert_eq!(expression(&app, id), "3*x");
            assert!(!app.inline_plots[&id].has_changes());
            assert_eq!(app.last_executed_regions, vec![id.to_string()]);
            assert!(
                app.plot_results[&id]
                    .as_ref()
                    .unwrap()
                    .first_curve()
                    .unwrap()
                    .points
                    .iter()
                    .all(|p| p[1] == 3. * p[0])
            );
            app.undo();
            assert_eq!(expression(&app, id), "sin(x)");
            frame(&mut app, &ctx, vec![]);
            assert_eq!(expression(&app, id), "sin(x)");
            app.redo();
            assert_eq!(expression(&app, id), "3*x");
        }
    }
    #[test]
    fn automatic_refresh_does_not_commit_text_and_save_sync_commits_once() {
        let (mut app, ctx, id) = fixture(false);
        replace(&mut app, &ctx, "4*x");
        app.calculate_command(CalculationCommand::Automatic);
        assert_eq!(expression(&app, id), "sin(x)");
        assert!(app.inline_plots[&id].has_changes());
        assert!(app.last_executed_regions.is_empty());
        app.sync_editors().unwrap();
        assert_eq!(expression(&app, id), "4*x");
        assert!(app.pending_regions.contains(&id));
        assert_eq!(app.undo.len(), 1);
        app.sync_editors().unwrap();
        assert_eq!(app.undo.len(), 1);
    }
    #[test]
    fn escape_and_net_noop_keep_committed_state_and_history() {
        let (mut app, ctx, id) = fixture(false);
        replace(&mut app, &ctx, "5*x");
        frame(
            &mut app,
            &ctx,
            vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
        );
        assert_eq!(expression(&app, id), "sin(x)");
        assert!(!app.inline_plots[&id].has_changes());
        assert!(app.undo.is_empty());
        app.focus_new = Some(id);
        frame(&mut app, &ctx, vec![]);
        replace(&mut app, &ctx, "sin(x)");
        frame(&mut app, &ctx, vec![enter()]);
        assert_eq!(expression(&app, id), "sin(x)");
        assert!(app.undo.is_empty() && app.last_executed_regions.is_empty());
    }
    #[test]
    fn focus_exit_commits_once_but_axis_tab_keeps_one_aggregate_draft() {
        let (mut app, ctx, id) = fixture(false);
        replace(&mut app, &ctx, "6*x");
        frame(
            &mut app,
            &ctx,
            vec![key(egui::Key::Tab, egui::Modifiers::NONE)],
        );
        assert_eq!(expression(&app, id), "sin(x)");
        assert!(app.inline_plots[&id].has_changes());
        assert!(app.undo.is_empty());
        let point = egui::pos2(600., 600.);
        for pressed in [true, false] {
            frame(
                &mut app,
                &ctx,
                vec![
                    egui::Event::PointerMoved(point),
                    egui::Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
        assert_eq!(expression(&app, id), "6*x");
        assert_eq!(app.undo.len(), 1);
        assert!(app.pending_regions.contains(&id) && app.last_executed_regions.is_empty());
    }
    #[test]
    fn invalid_byte_budget_keeps_draft_and_cannot_replace_committed_source_on_enter() {
        let (mut app, ctx, id) = fixture(false);
        replace(&mut app, &ctx, &"한".repeat(1500));
        frame(&mut app, &ctx, vec![enter()]);
        assert_eq!(expression(&app, id), "sin(x)");
        assert!(app.inline_plots[&id].has_changes());
        assert!(app.undo.is_empty() && app.last_executed_regions.is_empty());
        assert!(app.status.contains("4096"), "{}", app.status);
    }
    #[test]
    fn resize_and_pending_text_have_one_checkpoint_and_never_revert_current_dimensions() {
        let (mut app, ctx, id) = fixture(false);
        replace(&mut app, &ctx, "7*x");
        let CanvasContent::Plot(spec) = &mut app.document.regions[0].content else {
            panic!()
        };
        let original = spec.clone();
        spec.set_outer_size(spec.width + 40., spec.height + 20.)
            .unwrap();
        app.begin_plot_resize(id, original.clone());
        let CanvasContent::Plot(spec) = &app.document.regions[0].content else {
            panic!()
        };
        assert_eq!(
            [spec.width, spec.height],
            [original.width + 40., original.height + 20.]
        );
        assert_eq!(spec.primary_y_expression().as_legacy_text(), Some("7*x"));
        assert!(!app.inline_plots[&id].has_changes());
        assert_eq!(app.undo.len(), 1);
        app.undo();
        assert_eq!(expression(&app, id), "sin(x)");
        let CanvasContent::Plot(spec) = &app.document.regions[0].content else {
            panic!()
        };
        assert_eq!(spec, &original);
        app.redo();
        assert_eq!(expression(&app, id), "7*x");
        let CanvasContent::Plot(spec) = &app.document.regions[0].content else {
            panic!()
        };
        assert_eq!(spec.width, original.width + 40.);
    }
    #[test]
    fn modal_entry_commits_inline_once_and_cancel_or_unchanged_apply_does_not_revert_it() {
        let (mut app, ctx, id) = fixture(false);
        replace(&mut app, &ctx, "8*x");
        app.ribbon_command(&ctx, RibbonAction::EditPlot);
        assert_eq!(expression(&app, id), "8*x");
        assert_eq!(app.undo.len(), 1);
        assert!(!app.commit_plot_draft());
        assert_eq!(app.undo.len(), 1);
        app.ribbon_command(&ctx, RibbonAction::EditPlot);
        app.plot_draft = None;
        assert_eq!(expression(&app, id), "8*x");
        assert_eq!(app.undo.len(), 1);
    }
    #[test]
    fn invalid_inline_draft_blocks_modal_entry_without_losing_source_or_input() {
        let (mut app, ctx, id) = fixture(false);
        replace(&mut app, &ctx, &"한".repeat(1500));
        app.ribbon_command(&ctx, RibbonAction::EditPlot);
        assert!(app.plot_draft.is_none());
        assert!(app.inline_plots[&id].has_changes());
        assert_eq!(expression(&app, id), "sin(x)");
        assert!(app.undo.is_empty());
    }
    #[test]
    fn real_control_z_reaches_text_history_without_popping_unrelated_document_undo() {
        let (mut app, ctx, id) = fixture(false);
        let unrelated = app.snapshot();
        app.undo.push(unrelated);
        replace(&mut app, &ctx, "9*x");
        let mut output = ctx.run_ui(
            egui::RawInput {
                events: vec![key(egui::Key::Z, egui::Modifiers::COMMAND)],
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1200., 900.))),
                ..Default::default()
            },
            |ui| {
                app.document_history_shortcuts(ui.ctx());
                app.page_ui(ui);
            },
        );
        crate::frame_profile::tessellate(&ctx, &mut output);
        output.drop_without_applying_deltas();
        assert_eq!(app.undo.len(), 1);
        assert_eq!(expression(&app, id), "sin(x)");
        assert!(!app.inline_plots[&id].has_changes());
    }
    #[test]
    fn ime_preedit_enter_and_commit_do_not_publish_source_or_submit_calculation() {
        let (mut app, ctx, id) = fixture(false);
        frame(
            &mut app,
            &ctx,
            vec![key(egui::Key::A, egui::Modifiers::COMMAND)],
        );
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::Ime(egui::ImeEvent::Preedit {
                text: "한".into(),
                active_range_chars: Some(0..1),
            })],
        );
        assert!(app.inline_plots[&id].is_composing());
        assert_eq!(expression(&app, id), "sin(x)");
        assert!(app.commit_plot_edits(None).is_err());
        app.calculate_command(CalculationCommand::Worksheet);
        assert!(app.undo.is_empty() && app.last_executed_regions.is_empty());
        frame(&mut app, &ctx, vec![enter()]);
        assert_eq!(expression(&app, id), "sin(x)");
        assert!(app.inline_plots[&id].is_composing());
        frame(
            &mut app,
            &ctx,
            vec![
                egui::Event::Ime(egui::ImeEvent::Commit("2*x".into())),
                enter(),
            ],
        );
        assert!(!app.inline_plots[&id].is_composing());
        assert_eq!(expression(&app, id), "sin(x)");
        assert!(app.undo.is_empty() && app.last_executed_regions.is_empty());
        frame(&mut app, &ctx, vec![enter()]);
        assert_eq!(expression(&app, id), "2*x");
        assert_eq!(app.undo.len(), 1);
        assert!(app.pending_regions.contains(&id));
    }
    #[test]
    fn now_without_a_visible_bound_keeps_committed_curve_pending_and_invalidates_old_replies() {
        let (mut app, ctx, id) = fixture(false);
        let points = app.plot_results[&id].clone();
        let generation = app.background.submitted_generation();
        replace(&mut app, &ctx, "11*x");
        app.visible_calculation_end = None;
        app.calculate_command(CalculationCommand::Now);
        assert_eq!(expression(&app, id), "11*x");
        assert!(app.pending_regions.contains(&id));
        super::super::tests::assert_pending_curves_unchanged(&app.plot_results[&id], &points);
        assert!(app.last_executed_regions.is_empty());
        assert!(app.background.submitted_generation() > generation);
        app.persist_calculation_cache();
        let row = &app.document.saved_calculation_cache().unwrap().rows[0];
        assert_eq!(row.status, SavedCalculationStatus::Pending);
        app.restore_saved_calculations();
        assert!(app.pending_regions.contains(&id));
        super::super::tests::assert_pending_curves_unchanged(&app.plot_results[&id], &points);
    }
    #[test]
    fn leaving_plot_does_not_implicitly_commit_a_new_math_draft_in_the_same_frame() {
        let (mut app, ctx, id) = fixture(false);
        replace(&mut app, &ctx, "12*x");
        let math = app.document.add_math(20., 400.);
        app.retain_committed_input();
        let mut editor = Editor::new();
        formula::input_text(&mut editor, "a:=9");
        app.document
            .regions
            .iter_mut()
            .find(|region| region.id == math)
            .unwrap()
            .content = CanvasContent::Math(editor.to_portable().unwrap());
        app.editors.insert(math, editor);
        app.stage_math_edit(math, false);
        app.editing_region = Some(math);
        app.selected = Some(math);
        app.finish_plot_edits(Some(math));
        assert_eq!(expression(&app, id), "12*x");
        assert!(app.uncommitted_math.contains(&math));
        assert!(app.pending_regions.contains(&id) && app.pending_regions.contains(&math));
        assert!(app.last_executed_regions.is_empty());
    }
    #[test]
    fn resize_during_ime_keeps_preedit_outside_committed_source_and_undo_clears_it() {
        let (mut app, ctx, id) = fixture(false);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::Ime(egui::ImeEvent::Preedit {
                text: "한".into(),
                active_range_chars: None,
            })],
        );
        assert!(app.inline_plots[&id].is_composing());
        let CanvasContent::Plot(spec) = &mut app.document.regions[0].content else {
            panic!()
        };
        let original = spec.clone();
        spec.set_outer_size(spec.width + 30., spec.height).unwrap();
        app.begin_plot_resize(id, original);
        assert_eq!(expression(&app, id), "sin(x)");
        assert!(app.inline_plots[&id].is_composing());
        assert!(app.inline_plots[&id].has_changes());
        assert_eq!(app.undo.len(), 1);
        app.undo();
        assert_eq!(expression(&app, id), "sin(x)");
        assert!(!app.inline_plots[&id].has_changes());
    }
    fn resize_handle(app: &mut WorksheetApp, ctx: &egui::Context, id: u64) -> egui::Pos2 {
        let mut handle = None;
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1200., 900.))),
                ..Default::default()
            },
            |ui| {
                let origin = ui.available_rect_before_wrap().min;
                let region = app
                    .document
                    .regions
                    .iter()
                    .find(|region| region.id == id)
                    .unwrap();
                let top = app
                    .page_geometry()
                    .to_paper(region.left, region.top)
                    .to_vec2();
                app.page_ui(ui);
                handle = Some(
                    origin + top + vec2(app.region_widths[&id], app.region_heights[&id])
                        - vec2(3., 3.),
                );
            },
        );
        output.drop_without_applying_deltas();
        handle.unwrap()
    }
    fn drag(app: &mut WorksheetApp, ctx: &egui::Context, handle: egui::Pos2, delta: egui::Vec2) {
        frame(app, ctx, vec![egui::Event::PointerMoved(handle)]);
        frame(
            app,
            ctx,
            vec![
                egui::Event::PointerMoved(handle),
                egui::Event::PointerButton {
                    pos: handle,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        for offset in [delta * 0.5, delta] {
            frame(app, ctx, vec![egui::Event::PointerMoved(handle + offset)]);
        }
        frame(
            app,
            ctx,
            vec![egui::Event::PointerButton {
                pos: handle + delta,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
    }
    #[test]
    fn real_resize_clamped_to_same_size_has_no_history_calculation_or_reply_invalidation() {
        let (mut app, ctx, id) = fixture(false);
        let CanvasContent::Plot(spec) = &mut app.document.regions[0].content else {
            panic!()
        };
        spec.width = 120.;
        spec.height = 100.;
        let handle = resize_handle(&mut app, &ctx, id);
        let original = app.document.clone();
        let generation = app.background.submitted_generation();
        drag(&mut app, &ctx, handle, vec2(-40., -20.));
        assert_eq!(app.document, original);
        assert!(app.undo.is_empty() && app.last_executed_regions.is_empty());
        assert_eq!(app.background.submitted_generation(), generation);
        assert!(app.plot_resize_start.is_none());
    }
    #[test]
    fn real_resize_waits_for_actual_geometry_and_records_one_region_undo_for_the_drag() {
        let (mut app, ctx, id) = fixture(false);
        let handle = resize_handle(&mut app, &ctx, id);
        let original = app.document.regions[0].clone();
        drag(&mut app, &ctx, handle, vec2(80., 40.));
        let resized = app.document.regions[0].clone();
        assert_ne!(resized, original);
        assert_eq!(app.undo.len(), 1);
        app.undo();
        assert_eq!(app.document.regions[0], original);
        app.redo();
        assert_eq!(app.document.regions[0], resized);
    }

    #[test]
    fn real_resize_captures_exact_native_graph_request_and_actual_before_the_first_delta() {
        use mathfolio_document::canvas::{PlotGraphSize, PlotNativeGeometry};
        let (mut app, ctx, id) = fixture(false);
        let CanvasContent::Plot(plot) = &mut app.document.regions[0].content else {
            panic!()
        };
        plot.native_geometry = Some(Box::new(PlotNativeGeometry {
            graph_size: Some(PlotGraphSize {
                width: 321.00000000001,
                height: 217.125,
            }),
            requested_width: Some(420.00000000001),
            requested_height: Some(260.00000000002),
            actual_width: Some(418.75),
            actual_height: Some(259.5),
        }));
        let handle = resize_handle(&mut app, &ctx, id);
        let original = app.document.regions[0].clone();
        drag(&mut app, &ctx, handle, vec2(80., 40.));
        let resized = app.document.regions[0].clone();
        let CanvasContent::Plot(before) = &original.content else {
            panic!()
        };
        let CanvasContent::Plot(after) = &resized.content else {
            panic!()
        };
        assert_ne!([before.width, before.height], [after.width, after.height]);
        let old = before.native_geometry.as_ref().unwrap();
        let new = after.native_geometry.as_ref().unwrap();
        assert!(
            new.graph_size
                .as_ref()
                .unwrap()
                .same_source(old.graph_size.as_ref().unwrap())
        );
        assert_eq!(new.actual_width, None);
        assert_eq!(new.actual_height, None);
        assert_eq!(new.requested_width, Some(f64::from(after.width)));
        assert_eq!(new.requested_height, Some(f64::from(after.height)));
        assert_eq!(app.undo.len(), 1);
        assert!(app.last_executed_regions.is_empty());
        app.undo();
        assert_eq!(app.document.regions[0], original);
        app.redo();
        assert_eq!(app.document.regions[0], resized);
    }
}
