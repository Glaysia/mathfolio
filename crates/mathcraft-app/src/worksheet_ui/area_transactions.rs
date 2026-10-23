//! Area source commits participate in the existing document transaction stream.
use super::*;
impl WorksheetApp {
    pub(super) fn commit_area_edits(&mut self, except: Option<u64>) -> Result<bool, String> {
        let pending: Vec<_> = self
            .area_editors
            .iter()
            .filter(|(id, area)| Some(**id) != except && area.has_draft_changes())
            .map(|(id, _)| *id)
            .collect();
        if pending.is_empty() {
            return Ok(false);
        }
        self.retain_committed_input();
        // Validate all commits in isolated copies before installing any source.
        let mut replacements = vec![];
        for id in &pending {
            let region = self
                .document
                .regions
                .iter()
                .find(|region| region.id == *id)
                .ok_or("Missing Area region")?;
            let CanvasContent::Opaque { xml, kind } = &region.content else {
                return Err("Missing Area source".into());
            };
            if kind != "Area" {
                return Err("Expected Area source".into());
            }
            self.area_editors[id].preview_commit(xml)?;
            replacements.push((*id, xml.clone()));
        }
        let mut changed = false;
        for (id, mut xml) in replacements {
            let area = self.area_editors.get_mut(&id).unwrap();
            changed |= area.commit(&mut xml)?;
            let region = self
                .document
                .regions
                .iter_mut()
                .find(|region| region.id == id)
                .unwrap();
            region.content = CanvasContent::Opaque {
                xml,
                kind: "Area".into(),
            };
            area.deactivate();
        }
        if changed {
            self.mark_dirty();
            for id in pending {
                self.stage_math_edit(id, false);
            }
        }
        Ok(changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn source() -> String {
        r#"<region xmlns="http://schemas.mathsoft.com/worksheet50" xmlns:m="http://schemas.mathsoft.com/math50" region-id="1" left="0" top="0"><Area AreaHeight="130"><regions><region region-id="2" left="20" top="30"><math resultRef="9"><m:define><m:id labels="VARIABLE">x</m:id><m:real>3</m:real></m:define></math></region><region region-id="3" left="20" top="80"><math resultRef="10"><m:eval><m:id labels="VARIABLE">x</m:id><m:unitOverride><m:placeholder/></m:unitOverride></m:eval></math></region></regions></Area></region>"#.into()
    }
    fn scalar(app: &WorksheetApp, owner: u64, key: &str) -> f64 {
        let row = &app.area_results[&owner][key];
        assert!(row.error.is_none(), "{row:?}");
        let Some(Value::Scalar { quantity }) = &row.value else {
            panic!("{row:?}")
        };
        quantity.value.re
    }
    fn frame(app: &mut WorksheetApp, ctx: &egui::Context, events: Vec<egui::Event>) {
        ctx.run_ui(
            egui::RawInput {
                events,
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1200., 900.))),
                ..Default::default()
            },
            |ui| {
                app.document_history_shortcuts(ui.ctx());
                app.page_ui(ui);
            },
        )
        .drop_without_applying_deltas();
    }
    fn app_with_area() -> (WorksheetApp, u64) {
        let mut app = WorksheetApp::default();
        let owner = app.document.add_math(20., 20.);
        app.document.regions[0].content = CanvasContent::Opaque {
            xml: source(),
            kind: "Area".into(),
        };
        app.area_editors
            .insert(owner, area_ui::AreaEditor::new(&source()).unwrap());
        app.selected = Some(owner);
        app.editing_region = Some(owner);
        app.insertion = None;
        (app, owner)
    }
    fn draft(area: &mut area_ui::AreaEditor) {
        area.activate("[0,0,0]").unwrap();
        let editor = area.active_editor_mut().unwrap();
        let rhs = editor.arena().get(editor.root()).unwrap().children()[1];
        editor.set_atom(rhs, NodeKind::Number("7".into())).unwrap();
        area.mark_active_changed();
    }
    #[test]
    fn controller_preflights_every_area_before_installing_any_source_or_history() {
        let (mut app, first) = app_with_area();
        let second = app.document.add_math(20., 200.);
        app.document.regions[1].content = CanvasContent::Opaque {
            xml: source(),
            kind: "Area".into(),
        };
        app.area_editors
            .insert(second, area_ui::AreaEditor::new(&source()).unwrap());
        draft(app.area_editors.get_mut(&first).unwrap());
        draft(app.area_editors.get_mut(&second).unwrap());
        if let CanvasContent::Opaque { xml, .. } = &mut app.document.regions[1].content {
            *xml = xml.replace("AreaHeight=\"130\"", "AreaHeight=\"131\"");
        }
        let before = app.document.clone();
        let history = app.undo.len();
        assert!(app.commit_area_edits(None).is_err());
        assert_eq!(app.document, before);
        assert_eq!(app.undo.len(), history);
        assert!(
            app.area_editors
                .values()
                .all(|area| area.has_draft_changes())
        );
    }
    #[test]
    fn calculate_expression_forces_active_child_instead_of_last_child_in_owner() {
        let (mut app, owner) = app_with_area();
        app.recalculate();
        app.area_editors
            .get_mut(&owner)
            .unwrap()
            .activate("[0,0,0]")
            .unwrap();
        app.calculate_command(CalculationCommand::Expression(owner));
        assert_eq!(app.last_executed_regions, vec!["area-1-0-0-0"]);
        assert_eq!(scalar(&app, owner, "[0,0,1]"), 3.);
    }
    #[test]
    fn undo_redo_restores_area_draft_without_committing_or_evaluating_it() {
        let (mut app, owner) = app_with_area();
        app.recalculate();
        let before = app.input_snapshot();
        draft(app.area_editors.get_mut(&owner).unwrap());
        app.remember(before);
        app.stage_math_edit(owner, false);
        app.last_executed_regions.clear();
        app.undo();
        assert!(
            !app.area_editors
                .get(&owner)
                .is_some_and(|area| area.has_draft_changes())
        );
        app.last_executed_regions.clear();
        app.redo();
        assert!(app.area_editors[&owner].has_draft_changes());
        assert!(app.uncommitted_math.contains(&owner));
        assert!(app.last_executed_regions.is_empty());
        assert_eq!(scalar(&app, owner, "[0,0,1]"), 3.);
        assert!(
            matches!(&app.document.regions[0].content,CanvasContent::Opaque{xml,..}if xml==&source())
        );
    }
    #[test]
    fn actual_area_child_draft_retains_xml_and_result_until_enter_then_saves_nested_results() {
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let mut app = WorksheetApp::default();
        let owner = app.document.add_math(20., 20.);
        let original = source();
        app.document.regions.last_mut().unwrap().content = CanvasContent::Opaque {
            xml: original.clone(),
            kind: "Area".into(),
        };
        app.recalculate();
        assert_eq!(scalar(&app, owner, "[0,0,1]"), 3.);
        app.selected = Some(owner);
        app.editing_region = Some(owner);
        app.focus_new = Some(owner);
        frame(&mut app, &ctx, vec![]);
        let area = app.area_editors.get_mut(&owner).unwrap();
        area.activate("[0,0,0]").unwrap();
        let editor = area.active_editor_mut().unwrap();
        let rhs = editor.arena().get(editor.root()).unwrap().children()[1];
        editor.place_text_selection(rhs, 0, 1).unwrap();
        frame(&mut app, &ctx, vec![]);
        app.last_executed_regions.clear();
        frame(&mut app, &ctx, vec![egui::Event::Text("7".into())]);
        assert!(
            matches!(&app.document.regions[0].content,CanvasContent::Opaque{xml,..}if xml==&original)
        );
        assert!(
            app.uncommitted_math.contains(&owner),
            "draft={} active={:?} editing={:?} focused={:?} source={:?} status={}",
            app.area_editors[&owner].has_draft_changes(),
            app.area_editors[&owner].active_child(),
            app.editing_region,
            ctx.memory(|m| m.focused()),
            app.area_editors[&owner]
                .active_editor()
                .map(|e| e.to_portable()),
            app.status
        );
        assert!(app.last_executed_regions.is_empty());
        assert_eq!(scalar(&app, owner, "[0,0,1]"), 3.);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: Some(egui::Key::Enter),
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(app.uncommitted_math.is_empty());
        assert_eq!(scalar(&app, owner, "[0,0,1]"), 7.);
        app.persist_calculation_cache();
        app.document.validate().unwrap();
        assert!(matches!(
            &app.document.calculation_cache.as_ref().unwrap().rows[0].numeric_source,
            Some(SavedNumericSource::AreaPresentation(_))
        ));
        let path = std::env::temp_dir().join(format!(
            "mathcraft-area-{}-{}.mathcraft",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        app.document.save(&path).unwrap();
        let mut reopened = WorksheetApp::default();
        reopened.open(&path);
        std::fs::remove_file(path).unwrap();
        assert!(reopened.last_executed_regions.is_empty());
        assert!(reopened.pending_regions.is_empty());
        let display = crate::result_display::FormattedResult::from_saved(
            reopened.area_saved[&owner]["[0,0,1]"]
                .display
                .as_deref()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(display.cells[0].plain(), "7");
    }
}
