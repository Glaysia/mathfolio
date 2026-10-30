//! Prime's NewCollapsibleArea creates an empty container at worksheet insertion.
use super::*;
impl WorksheetApp {
    pub(super) fn can_insert_empty_area(&self) -> bool {
        self.pending.is_none()
            && self.selected.is_none()
            && self.editing_region.is_none()
            && self.insertion.is_some()
    }
    pub(super) fn insert_empty_area(&mut self) -> Result<Option<u64>, String> {
        if !self.can_insert_empty_area() {
            return Ok(None);
        }
        let before = self.snapshot();
        let position = self.insertion.unwrap();
        let height = f64::from(self.page_geometry().major_spacing());
        let xml = mathfolio_document::area_authoring::empty_area_xml(height)?;
        let mut editor = area_ui::AreaEditor::new(&xml)?;
        editor.set_input_grid(self.document.grid)?;
        editor.activate_insertion()?;
        let id = self
            .document
            .add_opaque(position.0, position.1, xml, "Area");
        self.area_editors.insert(id, editor);
        self.selected = Some(id);
        self.editing_region = Some(id);
        self.focus_new = Some(id);
        self.insertion = None;
        self.reveal = Some(position);
        self.remember(before);
        self.status.clear();
        Ok(Some(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(app: &mut WorksheetApp, ctx: &egui::Context, events: Vec<egui::Event>) {
        ctx.run_ui(
            egui::RawInput {
                events,
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1200., 900.))),
                ..Default::default()
            },
            |ui| {
                app.global_editing_shortcuts(ui.ctx());
                app.document_history_shortcuts(ui.ctx());
                app.page_ui(ui);
            },
        )
        .drop_without_applying_deltas();
    }
    #[test]
    fn ctrl_shift_a_creates_empty_area_and_first_text_is_one_creation_draft_transaction() {
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let mut app = WorksheetApp {
            insertion: Some((20., 20.)),
            ..Default::default()
        };
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::Key {
                key: egui::Key::A,
                physical_key: Some(egui::Key::A),
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers {
                    ctrl: true,
                    command: true,
                    shift: true,
                    ..Default::default()
                },
            }],
        );
        assert_eq!(app.document.regions.len(), 1);
        let owner = app.document.regions[0].id;
        let CanvasContent::Opaque { xml, .. } = &app.document.regions[0].content else {
            panic!()
        };
        assert!(
            mathfolio_document::area::area_math_children_xml(xml, &[])
                .unwrap()
                .is_empty()
        );
        assert!(app.last_executed_regions.is_empty());
        assert_eq!(app.undo.len(), 1);
        frame(&mut app, &ctx, vec![]);
        frame(&mut app, &ctx, vec![egui::Event::Text("7".into())]);
        assert_eq!(app.document.regions.len(), 1);
        assert_eq!(app.undo.len(), 2);
        assert!(app.uncommitted_math.contains(&owner));
        assert!(app.last_executed_regions.is_empty());
        let CanvasContent::Opaque { xml, .. } = &app.document.regions[0].content else {
            panic!()
        };
        let children = mathfolio_document::area::area_math_children_xml(xml, &[]).unwrap();
        assert_eq!(children.len(), 1);
        assert!(children[0].mathml.contains("placeholder"));
        assert_eq!(
            app.area_editors[&owner]
                .active_editor()
                .unwrap()
                .to_portable()
                .unwrap()
                .kind,
            NodeKind::Number("7".into())
        );
        app.undo();
        let CanvasContent::Opaque { xml, .. } = &app.document.regions[0].content else {
            panic!()
        };
        assert!(
            mathfolio_document::area::area_math_children_xml(xml, &[])
                .unwrap()
                .is_empty()
        );
        app.redo();
        assert!(app.area_editors[&owner].has_draft_changes());
        assert!(app.uncommitted_math.contains(&owner));
        frame(&mut app, &ctx, vec![]);
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
        assert_eq!(
            app.undo.len(),
            2,
            "Source-only Enter must not add a visible-noop Undo command"
        );
        assert!(app.area_editors[&owner].active_child().is_none());
        let CanvasContent::Opaque { xml, .. } = &app.document.regions[0].content else {
            panic!()
        };
        assert!(
            mathfolio_document::area::area_math_children_xml(xml, &[]).unwrap()[0]
                .mathml
                .contains(">7<")
        );
    }
    #[test]
    fn selected_or_active_math_is_never_wrapped_and_area_is_not_nested() {
        let mut app = WorksheetApp::default();
        let id = app.document.add_math(20., 20.);
        app.selected = Some(id);
        app.editing_region = Some(id);
        let before = app.document.clone();
        assert_eq!(app.insert_empty_area().unwrap(), None);
        assert_eq!(app.document, before);
        assert!(app.undo.is_empty());
        app.selected = None;
        app.editing_region = None;
        app.insertion = Some((20., 100.));
        let owner = app.insert_empty_area().unwrap().unwrap();
        let before = app.document.clone();
        assert_eq!(app.insert_empty_area().unwrap(), None);
        assert_eq!(app.document, before);
        assert_eq!(app.selected, Some(owner));
    }

    fn authored_native_fixture(output: &Path) {
        std::fs::create_dir_all(output).unwrap();
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let mut app = WorksheetApp {
            insertion: Some((20., 20.)),
            ..Default::default()
        };
        app.ribbon_command(&ctx, RibbonAction::InsertArea);
        let owner = app.document.regions[0].id;
        mathfolio_document::export::export_mcdx(&app.document, &output.join("area-empty.mcdx"))
            .unwrap();
        frame(&mut app, &ctx, vec![]);
        frame(&mut app, &ctx, vec![egui::Event::Text("x:=3".into())]);
        assert!(app.uncommitted_math.contains(&owner) && app.last_executed_regions.is_empty());
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
        assert!(app.area_editors[&owner].active_child().is_none());
        frame(&mut app, &ctx, vec![egui::Event::Text("x".into())]);
        assert!(app.uncommitted_math.contains(&owner));
        frame(&mut app, &ctx, vec![egui::Event::Text("=".into())]);
        assert!(app.uncommitted_math.is_empty());
        let query = &app.area_results[&owner]["[0,0,1]"];
        assert!(query.error.is_none(), "{query:?}");
        assert!(matches!(&query.value,Some(Value::Scalar{quantity})if quantity.value.re==3.));
        let CanvasContent::Opaque { xml, .. } = &app.document.regions[0].content else {
            panic!()
        };
        let children = mathfolio_document::area::area_math_children_xml(xml, &[]).unwrap();
        assert_eq!(children.len(), 2);
        assert!(children[1].top > children[0].top);
        app.persist_calculation_cache();
        app.document.validate().unwrap();
        let saved = output.join("area-authored.mathfolio");
        app.document.save(&saved).unwrap();
        let mut reopened = WorksheetApp::default();
        reopened.open(&saved);
        assert!(
            reopened.last_executed_regions.is_empty() && reopened.pending_regions.is_empty(),
            "{} {:?}",
            reopened.status,
            reopened.pending_regions
        );
        assert_eq!(reopened.document.regions.len(), 1);
        assert_eq!(reopened.area_saved[&owner].len(), 2);
        let native = output.join("area-authored.mcdx");
        mathfolio_document::export::export_mcdx(&app.document, &native).unwrap();
        let mut imported = WorksheetApp::default();
        imported.open(&native);
        assert!(imported.last_executed_regions.is_empty());
        assert!(
            imported.pending_regions.is_empty(),
            "{:?}",
            imported.area_saved
        );
        assert_eq!(imported.area_saved[&owner].len(), 2);
        let row = &imported.area_saved[&owner]["[0,0,1]"];
        assert_eq!(row.status, SavedCalculationStatus::Synchronized);
        assert!(row.display.is_some(), "{row:?}");
        imported.recalculate();
        assert_eq!(imported.document.regions.len(), 1);
        let query = &imported.area_results[&owner]["[0,0,1]"];
        assert!(query.error.is_none(), "{query:?}");
        assert!(matches!(&query.value,Some(Value::Scalar{quantity})if quantity.value.re==3.));
        imported.persist_calculation_cache();
        imported
            .document
            .save(&output.join("area-authored-reimported.mathfolio"))
            .unwrap();
        println!(
            "AREA_AUTHORED_NATIVE owners=1 children=2 first_input_execution=0 saved_reopen_execution=0 saved_reopen_pending=0 native_errors=0"
        );
    }
    #[test]
    fn actual_new_area_definition_enter_next_query_save_and_native_reimport() {
        let output = std::env::temp_dir().join(format!(
            "mathfolio-authored-area-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        authored_native_fixture(&output);
        for name in [
            "area-empty.mcdx",
            "area-authored.mcdx",
            "area-authored.mathfolio",
            "area-authored-reimported.mathfolio",
        ] {
            std::fs::remove_file(output.join(name)).unwrap();
        }
        std::fs::remove_dir(output).unwrap();
    }
    #[test]
    #[ignore = "Explicit actual Area authoring/native/cache fixture; no GUI or original runtime"]
    fn area_authoring_save_native_export_reopen_probe() {
        authored_native_fixture(&PathBuf::from(
            std::env::var_os("MATHFOLIO_AREA_AUTHORING_OUTPUT")
                .expect("MATHFOLIO_AREA_AUTHORING_OUTPUT"),
        ));
    }
}
