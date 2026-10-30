//! Avoid calculation-source preparation for gestures that only move a caret,
//! change a selection or request a later clipboard event.
use super::*;

pub(super) fn may_edit(event: &egui::Event) -> bool {
    match event {
        egui::Event::Key {
            key,
            pressed: true,
            modifiers,
            ..
        } => {
            if matches!(
                key,
                egui::Key::ArrowLeft
                    | egui::Key::ArrowRight
                    | egui::Key::ArrowUp
                    | egui::Key::ArrowDown
                    | egui::Key::Home
                    | egui::Key::End
                    | egui::Key::PageUp
                    | egui::Key::PageDown
            ) {
                return false;
            }
            // Ctrl+Shift+C is the column operator. Preserve its mutation path.
            if modifiers.command
                && !modifiers.alt
                && !modifiers.shift
                && matches!(key, egui::Key::C | egui::Key::V | egui::Key::A)
            {
                return false;
            }
            true
        }
        egui::Event::Text(_) | egui::Event::Paste(_) | egui::Event::Cut => true,
        egui::Event::Ime(egui::ImeEvent::Commit(_) | egui::ImeEvent::DeleteSurrounding { .. }) => {
            true
        }
        // Composition updates change widget preedit state only. The committed
        // text/deletion event captures its own source baseline before mutation.
        egui::Event::Ime(_) => false,
        _ => false,
    }
}

/// Keep the conservative predicate for mixed batches and every editing mode
/// except a single Enter that can only leave an unchanged formula widget.
pub(super) fn page_may_edit(app: &WorksheetApp, ui: &egui::Ui) -> bool {
    // Plot text uses a separate lazy buffer. It cannot mutate committed math,
    // so it needs neither a worksheet baseline nor a per-key history snapshot.
    if !ui.ctx().any_popup_open()
        && (ui.ctx().egui_wants_keyboard_input() || ui.input(|input|input.events.iter().any(|event|matches!(event,egui::Event::Key{key:egui::Key::Escape,pressed:true,modifiers,..} if modifiers.is_none()))))
        && app.editing_region.is_some_and(|id| {
            app.document
                .regions
                .iter()
                .any(|region| region.id == id && matches!(region.content, CanvasContent::Plot(_)))
        })
        && !ui.input(|input| input.pointer.any_pressed())
    {
        return false;
    }
    let commit_only =
        app.focus_new.is_none()
            && app.selected.is_some()
            && app.selected == app.editing_region
            && !ui.ctx().any_popup_open()
            && !app
                .table_editors
                .values()
                .any(|table| table.has_draft_changes())
            && app.selected.is_some_and(|id| {
                app.document.regions.iter().any(|region| {
                    region.id == id && matches!(region.content, CanvasContent::Math(_))
                }) && app
                    .editors
                    .get(&id)
                    .is_some_and(|editor| !editor.is_in_program())
            })
            && ui.input(|input| {
                !input.pointer.any_pressed()
                    && matches!(input.events.as_slice(), [egui::Event::Key {
                    key: egui::Key::Enter, pressed: true, modifiers, ..
                }] if modifiers.is_none())
            });
    !commit_only && ui.input(|input| input.events.iter().any(may_edit))
}

pub(super) fn ribbon_needs_baseline(action: RibbonAction) -> bool {
    !matches!(
        action,
        RibbonAction::New
            | RibbonAction::Open
            | RibbonAction::Copy
            | RibbonAction::Paste
            | RibbonAction::Print
            | RibbonAction::ExportPdf
            | RibbonAction::PageSetup
            | RibbonAction::EditPlot
            | RibbonAction::InsertSolveBlock
            | RibbonAction::InsertArea
            | RibbonAction::EditPageSections
            | RibbonAction::StopAllCalculations
            | RibbonAction::SetAutomaticCalculation(false)
            | RibbonAction::SetBinOctHexMode(_)
            | RibbonAction::SetParagraphAlignment { .. }
            | RibbonAction::SetMatrixIndices { .. }
            | RibbonAction::PreviousError
            | RibbonAction::NextError
            | RibbonAction::ToggleWarnings
            | RibbonAction::Zoom(_)
            | RibbonAction::ToggleGrid
            | RibbonAction::SetDraftView(_)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(key: egui::Key, pressed: bool, modifiers: egui::Modifiers) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed,
            repeat: false,
            modifiers,
        }
    }

    fn fixture() -> (WorksheetApp, egui::Context, u64) {
        fixture_from_tree(PortableTree {
            kind: NodeKind::Identifier("voltage".into()),
            children: vec![],
        })
    }

    fn fixture_from_tree(tree: PortableTree) -> (WorksheetApp, egui::Context, u64) {
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let mut app = WorksheetApp::default();
        let id = app.document.add_math(0., 0.);
        app.document.regions[0].content = CanvasContent::Math(tree.clone());
        app.editors
            .insert(id, Editor::from_portable(&tree).unwrap());
        app.selected = Some(id);
        app.focus_new = Some(id);
        app.insertion = None;
        super::super::tests::frame(&mut app, &ctx, vec![]);
        (app, ctx, id)
    }

    fn no_retained_input(app: &WorksheetApp) {
        assert!(app.committed_input.is_none());
        assert!(!app.background.preparation.has_committed_baseline());
    }

    #[test]
    fn caret_selection_and_release_do_not_capture_a_cold_worksheet() {
        for event in [
            key(egui::Key::ArrowLeft, true, egui::Modifiers::NONE),
            key(egui::Key::ArrowRight, true, egui::Modifiers::SHIFT),
            key(egui::Key::Home, true, egui::Modifiers::NONE),
            key(egui::Key::End, true, egui::Modifiers::NONE),
            key(egui::Key::ArrowLeft, false, egui::Modifiers::NONE),
            key(egui::Key::Delete, false, egui::Modifiers::NONE),
            key(egui::Key::A, true, egui::Modifiers::COMMAND),
            egui::Event::Copy,
        ] {
            let (mut app, ctx, id) = fixture();
            let before = app.document.regions[0].clone();
            super::super::tests::frame(&mut app, &ctx, vec![event]);
            assert_eq!(app.document.regions[0], before);
            assert!(app.uncommitted_math.is_empty() && app.undo.is_empty() && !app.dirty);
            assert!(app.last_executed_regions.is_empty());
            assert_eq!(app.editing_region, Some(id));
            no_retained_input(&app);
        }
    }

    #[test]
    fn ribbon_copy_view_and_draft_dialogs_do_not_capture_input() {
        for action in [
            RibbonAction::Copy,
            RibbonAction::Paste,
            RibbonAction::Zoom(1.25),
            RibbonAction::ToggleGrid,
            RibbonAction::SetDraftView(true),
            RibbonAction::ToggleWarnings,
            RibbonAction::PageSetup,
            RibbonAction::EditPageSections,
            RibbonAction::InsertSolveBlock,
            RibbonAction::EditPlot,
        ] {
            let (mut app, ctx, _) = fixture();
            let before = app.document.regions[0].clone();
            app.ribbon_command(&ctx, action);
            assert_eq!(app.document.regions[0], before);
            assert!(app.uncommitted_math.is_empty() && app.undo.is_empty() && !app.dirty);
            assert!(app.last_executed_regions.is_empty());
            no_retained_input(&app);
        }
    }

    #[test]
    fn column_shortcut_and_matrix_tab_still_capture_before_mutating() {
        let (mut app, ctx, id) = fixture();
        let modifiers = egui::Modifiers {
            shift: true,
            ..egui::Modifiers::COMMAND
        };
        super::super::tests::frame(&mut app, &ctx, vec![key(egui::Key::C, true, modifiers)]);
        assert_eq!(
            app.editors[&id].to_portable().unwrap().kind,
            NodeKind::Column
        );
        assert!(app.uncommitted_math.contains(&id));
        assert!(
            app.committed_input.is_some() || app.background.preparation.has_committed_baseline()
        );
        assert!(app.last_executed_regions.is_empty());

        let tree = PortableTree {
            kind: NodeKind::Matrix {
                rows: 1,
                cols: 1,
                display: None,
            },
            children: vec![PortableTree {
                kind: NodeKind::Number("1".into()),
                children: vec![],
            }],
        };
        let (mut app, ctx, id) = fixture_from_tree(tree);
        super::super::tests::frame(
            &mut app,
            &ctx,
            vec![key(egui::Key::Tab, true, egui::Modifiers::NONE)],
        );
        assert_eq!(
            app.editors[&id].to_portable().unwrap().kind,
            NodeKind::Matrix {
                rows: 2,
                cols: 1,
                display: None
            },
            "editing={:?} focus={:?} caret={:?} selection={:?}",
            app.editing_region,
            ctx.memory(|m| m.focused()),
            app.editors[&id].caret(),
            app.editors[&id].selection()
        );
        assert!(app.uncommitted_math.contains(&id));
        assert!(
            app.committed_input.is_some() || app.background.preparation.has_committed_baseline()
        );
        assert!(app.last_executed_regions.is_empty());
    }

    #[test]
    fn first_shift_tab_moves_to_the_previous_cell_without_leaving_the_formula() {
        let tree = PortableTree {
            kind: NodeKind::Matrix {
                rows: 1,
                cols: 2,
                display: None,
            },
            children: ["1", "2"]
                .map(|value| PortableTree {
                    kind: NodeKind::Number(value.into()),
                    children: vec![],
                })
                .to_vec(),
        };
        let (mut app, ctx, id) = fixture_from_tree(tree.clone());
        let editor = app.editors.get_mut(&id).unwrap();
        let cells = editor
            .arena()
            .get(editor.root())
            .unwrap()
            .children()
            .to_vec();
        editor.place_caret(cells[1], Direction::Right).unwrap();
        let focused = ctx.memory(|memory| memory.focused());
        super::super::tests::frame(
            &mut app,
            &ctx,
            vec![key(egui::Key::Tab, true, egui::Modifiers::SHIFT)],
        );
        assert_eq!(app.editors[&id].caret().node, cells[0]);
        assert_eq!(ctx.memory(|memory| memory.focused()), focused);
        assert_eq!(app.editing_region, Some(id));
        assert_eq!(app.editors[&id].to_portable().unwrap(), tree);
        assert!(app.uncommitted_math.is_empty() && app.undo.is_empty() && !app.dirty);
        assert!(app.last_executed_regions.is_empty());
    }

    #[test]
    #[allow(deprecated)]
    fn ime_composition_only_preserves_source_caret_selection_and_both_histories() {
        for ime in [
            egui::ImeEvent::Enabled,
            egui::ImeEvent::Disabled,
            egui::ImeEvent::Preedit {
                text: "한".into(),
                active_range_chars: Some(0..1),
            },
            egui::ImeEvent::Preedit {
                text: String::new(),
                active_range_chars: None,
            },
        ] {
            let (mut app, ctx, id) = fixture();
            let before = app.document.regions.clone();
            let caret = app.editors[&id].caret();
            let selection = app.editors[&id].selection();
            let revision = app.editors[&id].arena().revision();
            let undo = app.snapshot();
            let redo = app.snapshot();
            app.undo.push(undo);
            app.redo.push(redo);
            super::super::tests::frame(&mut app, &ctx, vec![egui::Event::Ime(ime)]);
            assert_eq!(app.document.regions, before);
            assert_eq!(app.editors[&id].caret(), caret);
            assert_eq!(app.editors[&id].selection(), selection);
            assert_eq!(app.editors[&id].arena().revision(), revision);
            assert_eq!((app.undo.len(), app.redo.len()), (1, 1));
            assert!(app.uncommitted_math.is_empty() && !app.dirty);
            no_retained_input(&app);
        }
    }

    #[test]
    fn one_plain_formula_enter_does_not_capture_or_change_history() {
        let (mut app, ctx, id) = fixture();
        let before = app.document.regions.clone();
        let undo = app.snapshot();
        let redo = app.snapshot();
        app.undo.push(undo);
        app.redo.push(redo);
        super::super::tests::frame(
            &mut app,
            &ctx,
            vec![key(egui::Key::Enter, true, egui::Modifiers::NONE)],
        );
        assert_eq!(app.document.regions, before);
        assert_eq!((app.undo.len(), app.redo.len()), (1, 1));
        assert!(app.uncommitted_math.is_empty() && !app.dirty);
        assert!(app.editing_region.is_none() && app.selected.is_none() && app.insertion.is_some());
        assert!(app.editors.contains_key(&id));
        no_retained_input(&app);
    }

    fn classify(app: &WorksheetApp, ctx: &egui::Context, events: Vec<egui::Event>) -> bool {
        let mut result = None;
        ctx.run_ui(
            egui::RawInput {
                events,
                ..Default::default()
            },
            |ui| {
                result = Some(page_may_edit(app, ui));
            },
        )
        .drop_without_applying_deltas();
        result.unwrap()
    }

    #[test]
    fn enter_exceptions_and_mixed_batches_keep_conservative_capture() {
        let (mut app, ctx, id) = fixture();
        let enter = key(egui::Key::Enter, true, egui::Modifiers::NONE);
        assert!(!classify(&app, &ctx, vec![enter.clone()]));
        assert!(classify(
            &app,
            &ctx,
            vec![
                key(egui::Key::ArrowLeft, true, egui::Modifiers::NONE),
                enter.clone()
            ]
        ));
        assert!(classify(
            &app,
            &ctx,
            vec![egui::Event::Text("x".into()), enter.clone()]
        ));
        assert!(classify(
            &app,
            &ctx,
            vec![key(egui::Key::Enter, true, egui::Modifiers::SHIFT)]
        ));
        app.focus_new = Some(id);
        assert!(classify(&app, &ctx, vec![enter.clone()]));
        app.focus_new = None;
        app.document.regions[0].content = CanvasContent::Text("text".into());
        assert!(classify(&app, &ctx, vec![enter.clone()]));
        let program = PortableTree {
            kind: NodeKind::Program { statement_count: 1 },
            children: vec![PortableTree {
                kind: NodeKind::Number("1".into()),
                children: vec![],
            }],
        };
        let (mut app, ctx, id) = fixture_from_tree(program);
        assert!(app.editors[&id].is_in_program());
        assert!(classify(&app, &ctx, vec![enter.clone()]));
        app.focus_new = Some(id);
        super::super::tests::frame(&mut app, &ctx, vec![]);
        let editor = app.editors.get_mut(&id).unwrap();
        let line = editor.arena().get(editor.root()).unwrap().children()[0];
        editor.place_text_caret(line, 1).unwrap();
        super::super::tests::frame(&mut app, &ctx, vec![enter]);
        assert!(app.uncommitted_math.contains(&id));
        assert!(app.background.preparation.has_committed_baseline());
        assert!(matches!(
            app.editors[&id].to_portable().unwrap().kind,
            NodeKind::Program { statement_count: 2 }
        ));
    }
}
