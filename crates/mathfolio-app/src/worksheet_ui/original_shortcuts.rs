//! Global shortcuts must not consume a more specific math-editor gesture.
//! egui's consume_key intentionally ignores extra Shift/Alt; these application
//! commands require exact modifiers. Original contracts: analysis/0057.
use super::WorksheetApp;
use eframe::egui;

pub(super) fn consume_exact_key(
    input: &mut egui::InputState,
    modifiers: egui::Modifiers,
    key: egui::Key,
) -> bool {
    let mut consumed = false;
    input.events.retain(|event| {
        let matches=matches!(event,egui::Event::Key{key:actual,modifiers:actual_modifiers,pressed:true,..}
            if *actual==key && actual_modifiers.matches_exact(modifiers));
        consumed|=matches;
        !matches
    });
    consumed
}

impl WorksheetApp {
    pub(super) fn region_clipboard_shortcuts(&mut self, ctx: &egui::Context) {
        if self.editing_region.is_some() {
            return;
        }
        let copy = ctx.input_mut(|i| consume_exact_key(i, egui::Modifiers::COMMAND, egui::Key::C))
            || ctx.input(|i| {
                i.events
                    .iter()
                    .any(|event| matches!(event, egui::Event::Copy))
            });
        let cut = ctx.input_mut(|i| consume_exact_key(i, egui::Modifiers::COMMAND, egui::Key::X))
            || ctx.input(|i| {
                i.events
                    .iter()
                    .any(|event| matches!(event, egui::Event::Cut))
            });
        if copy || cut {
            self.copy_region(ctx, cut);
        }
        let pasted = ctx.input(|i| {
            i.events.iter().find_map(|event| {
                if let egui::Event::Paste(text) = event {
                    (text.starts_with("mathcraft-region:") || text.starts_with("mathcraft-tree:"))
                        .then(|| text.clone())
                } else {
                    None
                }
            })
        });
        if let Some(text) = pasted
            && self.paste_region(&text)
        {
            ctx.input_mut(|i| {
                i.events
                    .retain(|event| !matches!(event, egui::Event::Paste(_)))
            });
        }
        if ctx.input_mut(|i| consume_exact_key(i, egui::Modifiers::NONE, egui::Key::Delete)) {
            self.remove_selected();
        }
    }

    pub(super) fn document_history_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input()
            && self.editing_region.is_some_and(|id| {
                self.document.regions.iter().any(|region| {
                    region.id == id && matches!(region.content, super::CanvasContent::Plot(_))
                })
            })
        {
            // The inline TextEdit owns its draft's character history.
            return;
        }
        if ctx.input_mut(|input| consume_exact_key(input, egui::Modifiers::COMMAND, egui::Key::Z)) {
            self.undo();
        }
        if ctx.input_mut(|input| consume_exact_key(input, egui::Modifiers::COMMAND, egui::Key::Y)) {
            self.redo();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        formula,
        worksheet_ui::{CanvasContent, Editor, NodeKind},
    };
    use mathfolio_editor::{BinaryOp, InsertionMode};

    fn command(shift: bool) -> egui::Modifiers {
        egui::Modifiers {
            ctrl: true,
            command: true,
            shift,
            ..egui::Modifiers::NONE
        }
    }
    fn key(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed: true,
            repeat: false,
            modifiers,
        }
    }
    fn frame(app: &mut WorksheetApp, ctx: &egui::Context, events: Vec<egui::Event>) {
        frame_with_modifiers(app, ctx, events, egui::Modifiers::NONE)
            .drop_without_applying_deltas();
    }
    fn frame_with_modifiers(
        app: &mut WorksheetApp,
        ctx: &egui::Context,
        mut events: Vec<egui::Event>,
        modifiers: egui::Modifiers,
    ) -> egui::FullOutput {
        events.insert(0, egui::Event::ModifiersChanged(modifiers));
        ctx.run_ui(
            egui::RawInput {
                events,
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1200., 900.),
                )),
                ..Default::default()
            },
            |ui| {
                app.global_editing_shortcuts(ui.ctx());
                app.document_history_shortcuts(ui.ctx());
                app.region_clipboard_shortcuts(ui.ctx());
                app.page_ui(ui);
            },
        )
    }
    fn fixture() -> (WorksheetApp, egui::Context, u64) {
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let mut app = WorksheetApp {
            automatic_calculation: false,
            ..Default::default()
        };
        let id = app.insert((38.4, 38.4), false);
        frame(&mut app, &ctx, vec![]);
        (app, ctx, id)
    }

    #[test]
    fn global_routing_preserves_original_label_cycle_and_exact_modifiers() {
        let (mut app, ctx, id) = fixture();
        frame(&mut app, &ctx, vec![egui::Event::Text("voltage".into())]);
        let zoom = app.zoom;
        for expected in [
            "VARIABLE", "UNIT", "CONSTANT", "FUNCTION", "SYSTEM", "KEYWORD", "*", "VARIABLE",
        ] {
            frame(&mut app, &ctx, vec![key(egui::Key::Q, command(false))]);
            let tree = app.editors[&id].to_portable().unwrap();
            assert!(
                matches!(tree.kind, NodeKind::AnnotatedIdentifier { ref name, ref labels } if name == "voltage" && labels == expected),
                "{tree:?}"
            );
            assert!(app.last_executed_regions.is_empty());
            assert_eq!(app.zoom, zoom);
        }
        let before = app.editors[&id].to_portable().unwrap();
        let history = app.undo.len();
        let mut release = key(egui::Key::Q, command(false));
        if let egui::Event::Key { pressed, .. } = &mut release {
            *pressed = false;
        }
        frame(&mut app, &ctx, vec![release]);
        frame(&mut app, &ctx, vec![key(egui::Key::Q, command(true))]);
        let alt = egui::Modifiers {
            alt: true,
            ..command(false)
        };
        frame(&mut app, &ctx, vec![key(egui::Key::Q, alt)]);
        assert_eq!(app.editors[&id].to_portable().unwrap(), before);
        assert_eq!(app.undo.len(), history);
        app.undo();
        let previous = app.editors[&id].to_portable().unwrap();
        assert!(
            matches!(previous.kind, NodeKind::AnnotatedIdentifier { ref labels, .. } if labels == "*")
        );
        app.redo();
        assert_eq!(app.editors[&id].to_portable().unwrap(), before);
    }

    #[test]
    fn automatic_label_ribbon_restores_inference_without_moving_native_caret() {
        let (mut app, ctx, id) = fixture();
        let tree = mathfolio_editor::PortableTree {
            kind: NodeKind::Binary(BinaryOp::Evaluate),
            children: vec![
                mathfolio_editor::PortableTree {
                    kind: NodeKind::NativeIdentifier {
                        name: "m".into(),
                        labels: "UNIT".into(),
                        contextual: Some(true),
                        subscript: None,
                    },
                    children: vec![],
                },
                mathfolio_editor::PortableTree {
                    kind: NodeKind::Placeholder,
                    children: vec![],
                },
            ],
        };
        app.document
            .regions
            .iter_mut()
            .find(|r| r.id == id)
            .unwrap()
            .content = CanvasContent::Math(tree.clone());
        let mut editor = Editor::from_portable(&tree).unwrap();
        let atom = editor.arena().get(editor.root()).unwrap().children()[0];
        editor.place_text_caret(atom, 0).unwrap();
        let caret = editor.caret();
        app.editors.insert(id, editor);
        app.recalculate();
        app.last_executed_regions.clear();
        app.ribbon_command(&ctx, crate::ribbon::RibbonAction::SetLabel("VARIABLE"));
        assert_eq!(app.editors[&id].caret(), caret);
        app.ribbon_command(&ctx, crate::ribbon::RibbonAction::SetLabel("*"));
        assert_eq!(app.editors[&id].caret(), caret);
        assert!(app.last_executed_regions.is_empty());
        let fragment =
            crate::worksheet_math::tree_to_mathml(&app.editors[&id].to_portable().unwrap())
                .unwrap();
        assert!(fragment.contains("labels=\"*\""));
        assert!(!fragment.contains("label-is-contextual"));
        app.commit_math_edits();
        assert!(app.last_executed_regions.is_empty());
        app.recalculate();
        assert_eq!(
            super::super::value_text(app.results[&id].value.as_ref().unwrap()),
            "1 m"
        );
        app.undo();
        app.commit_math_edits();
        app.recalculate();
        assert!(app.results[&id].error.is_some());
    }

    #[test]
    fn label_cycle_draft_and_none_fallback_follow_manual_commit_contract() {
        let (mut app, ctx, id) = fixture();
        frame(&mut app, &ctx, vec![egui::Event::Text("m=".into())]);
        let root = app.editors[&id].root();
        let name = app.editors[&id].arena().get(root).unwrap().children()[0];
        app.editors
            .get_mut(&id)
            .unwrap()
            .place_text_caret(name, 1)
            .unwrap();
        assert_eq!(
            super::super::value_text(app.results[&id].value.as_ref().unwrap()),
            "1 m"
        );
        let saved = serde_json::to_value(&app.results[&id].value).unwrap();
        // The last-executed list belongs to the explicit '=' fixture setup.
        app.last_executed_regions.clear();
        for _ in 0..7 {
            frame(&mut app, &ctx, vec![key(egui::Key::Q, command(false))]);
            assert_eq!(
                serde_json::to_value(&app.results[&id].value).unwrap(),
                saved
            );
            assert!(app.last_executed_regions.is_empty());
        }
        let tree = app.editors[&id].to_portable().unwrap();
        assert!(
            matches!(&tree.children[0].kind, NodeKind::AnnotatedIdentifier { labels, .. } if labels == "*")
        );
        frame(
            &mut app,
            &ctx,
            vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
        );
        assert!(app.last_executed_regions.is_empty());
        app.recalculate();
        assert!(
            app.results[&id].error.is_none(),
            "{:?}",
            app.results[&id].error
        );
        assert_eq!(
            super::super::value_text(app.results[&id].value.as_ref().unwrap()),
            "1 m"
        );
    }

    #[test]
    fn global_history_and_plot_routing_preserve_literal_infinity_and_logical_or_input() {
        let (mut app, ctx, id) = fixture();
        let zoom = app.zoom;
        frame(&mut app, &ctx, vec![egui::Event::Text("I".into())]);
        frame(&mut app, &ctx, vec![key(egui::Key::Minus, command(false))]);
        assert_eq!(app.zoom, zoom);
        assert_eq!(app.editors[&id].caret().insertion, InsertionMode::Subscript);
        frame(&mut app, &ctx, vec![egui::Event::Text("dc".into())]);
        let undo_before = app.undo.len();
        frame(&mut app, &ctx, vec![key(egui::Key::Z, command(true))]);
        assert!(
            matches!(app.editors[&id].to_portable().unwrap().kind,NodeKind::NativeIdentifier{ref name,subscript:Some(ref suffix),..}if name=="I"&&suffix=="dc∞")
        );
        assert!(app.undo.len() > undo_before);
        assert!(app.redo.is_empty());
        assert_eq!(app.zoom, zoom);
        assert_eq!(app.document.regions.len(), 1);

        let (mut app, ctx, id) = fixture();
        frame(&mut app, &ctx, vec![egui::Event::Text("1".into())]);
        frame(&mut app, &ctx, vec![key(egui::Key::Num2, command(true))]);
        frame(&mut app, &ctx, vec![egui::Event::Text("0".into())]);
        assert_eq!(
            app.document.regions.len(),
            1,
            "logical OR must not insert a plot"
        );
        assert_eq!(
            app.editors[&id].to_portable().unwrap().kind,
            NodeKind::Binary(BinaryOp::Or)
        );
        assert!(app.last_executed_regions.is_empty());
    }

    #[test]
    fn global_text_and_history_commands_do_not_steal_array_operator_shortcuts() {
        for (shortcut, expected) in [
            (egui::Key::T, NodeKind::Transpose),
            (egui::Key::R, NodeKind::Row),
            (egui::Key::C, NodeKind::Column),
        ] {
            let (mut app, ctx, id) = fixture();
            frame(&mut app, &ctx, vec![egui::Event::Text("M".into())]);
            frame(&mut app, &ctx, vec![key(shortcut, command(true))]);
            let tree = app.editors[&id].to_portable().unwrap();
            assert_eq!(tree.kind, expected);
            assert!(matches!(&tree.children[0].kind, NodeKind::Identifier(name) if name == "M"));
            assert_eq!(app.document.regions.len(), 1);
            assert!(!app.text_mode);
            assert!(app.last_executed_regions.is_empty());
        }
    }

    #[test]
    fn backend_preserves_column_key_and_real_copy_as_distinct_events() {
        let (mut app, ctx, id) = fixture();
        frame(&mut app, &ctx, vec![egui::Event::Text("M".into())]);
        let output = frame_with_modifiers(
            &mut app,
            &ctx,
            vec![key(egui::Key::C, command(true))],
            command(true),
        );
        assert_eq!(
            app.editors[&id].to_portable().unwrap().kind,
            NodeKind::Column
        );
        assert_eq!(app.document.regions.len(), 1);
        assert!(
            !output
                .platform_output
                .commands
                .iter()
                .any(|command| matches!(command, egui::OutputCommand::CopyText(_)))
        );
        output.drop_without_applying_deltas();

        let tree = app.editors[&id].to_portable().unwrap();
        let editor = app.editors.get_mut(&id).unwrap();
        editor.select_subtree(editor.root()).unwrap();
        let output = frame_with_modifiers(&mut app, &ctx, vec![egui::Event::Copy], command(true));
        assert_eq!(app.editors[&id].to_portable().unwrap(), tree);
        assert!(
            output
                .platform_output
                .commands
                .iter()
                .any(|command| matches!(command, egui::OutputCommand::CopyText(_)))
        );
        output.drop_without_applying_deltas();

        // Dedicated Copy and Ctrl+Shift+Insert remain clipboard commands,
        // including when a whole worksheet region owns selection.
        app.editing_region = None;
        let output = ctx.run_ui(
            egui::RawInput {
                events: vec![
                    egui::Event::ModifiersChanged(command(true)),
                    egui::Event::Copy,
                ],
                ..Default::default()
            },
            |ui| app.region_clipboard_shortcuts(ui.ctx()),
        );
        assert!(
            output
                .platform_output
                .commands
                .iter()
                .any(|command| matches!(command, egui::OutputCommand::CopyText(_)))
        );
        output.drop_without_applying_deltas();
    }

    #[test]
    fn global_vector_and_cross_shortcuts_survive_application_command_routing() {
        for (key_code, modifiers, expected) in [
            (
                egui::Key::Backtick,
                command(true),
                NodeKind::Binary(BinaryOp::GlobalDefine),
            ),
            (egui::Key::Num6, command(true), NodeKind::Vectorize),
            (
                egui::Key::Num8,
                command(false),
                NodeKind::Binary(BinaryOp::CrossProduct),
            ),
        ] {
            let (mut app, ctx, id) = fixture();
            frame(&mut app, &ctx, vec![egui::Event::Text("glob".into())]);
            frame(&mut app, &ctx, vec![key(key_code, modifiers)]);
            let tree = app.editors[&id].to_portable().unwrap();
            assert_eq!(tree.kind, expected);
            assert_eq!(app.document.regions.len(), 1);
            assert!(!app.text_mode);
            assert!(app.last_executed_regions.is_empty());
            assert!(matches!(&tree.children[0].kind, NodeKind::Identifier(name) if name == "glob"));
        }
    }

    #[test]
    fn polar_shortcut_survives_global_print_command_routing() {
        let (mut app, ctx, id) = fixture();
        frame(&mut app, &ctx, vec![egui::Event::Text("2".into())]);
        frame(&mut app, &ctx, vec![key(egui::Key::P, command(true))]);
        assert_eq!(
            app.editors[&id].to_portable().unwrap().kind,
            NodeKind::Binary(BinaryOp::Polar)
        );
        assert!(!app.print_ui.open);
        assert_eq!(app.document.regions.len(), 1);
        assert!(app.last_executed_regions.is_empty());
    }

    #[test]
    fn control_f1_and_plain_f1_have_separate_actions_and_unassigned_variants_pass_through() {
        let mut app = WorksheetApp::default();
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let minimized = app.ribbon.minimized;
        let help = app.show_shortcuts;
        let run = |app: &mut WorksheetApp, modifiers| {
            let output = ctx.run_ui(
                egui::RawInput {
                    events: vec![key(egui::Key::F1, modifiers)],
                    ..Default::default()
                },
                |ui| app.global_editing_shortcuts(ui.ctx()),
            );
            output.drop_without_applying_deltas();
        };
        run(&mut app, command(false));
        assert_eq!(app.ribbon.minimized, !minimized);
        assert_eq!(app.show_shortcuts, help);
        run(&mut app, egui::Modifiers::NONE);
        assert_eq!(app.ribbon.minimized, !minimized);
        assert_eq!(app.show_shortcuts, !help);
        run(&mut app, command(true));
        assert_eq!(app.ribbon.minimized, !minimized);
        assert_eq!(app.show_shortcuts, !help);
        run(
            &mut app,
            egui::Modifiers {
                alt: true,
                ..egui::Modifiers::NONE
            },
        );
        assert_eq!(app.show_shortcuts, !help);
    }

    #[test]
    fn f5_calculates_only_visible_prefix_and_control_f5_calculates_whole_worksheet() {
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let mut app = WorksheetApp {
            automatic_calculation: false,
            ..Default::default()
        };
        let mut ids = vec![];
        for (top, source) in [(20., "a:=2"), (80., "a="), (2000., "a=")] {
            let mut editor = Editor::new();
            formula::input_text(&mut editor, source);
            let id = app.document.add_math(20., top);
            app.document.regions.last_mut().unwrap().content =
                CanvasContent::Math(editor.to_portable().unwrap());
            ids.push(id);
        }
        app.insertion = None;
        app.visible_calculation_end = Some(ids[1]);
        app.visible_regions.extend([ids[0], ids[1]]);
        let run = |app: &mut WorksheetApp, modifiers| {
            let output = ctx.run_ui(
                egui::RawInput {
                    events: vec![key(egui::Key::F5, modifiers)],
                    ..Default::default()
                },
                |ui| app.global_editing_shortcuts(ui.ctx()),
            );
            output.drop_without_applying_deltas();
        };
        run(
            &mut app,
            egui::Modifiers {
                shift: true,
                ..egui::Modifiers::NONE
            },
        );
        assert!(app.last_executed_regions.is_empty());
        run(&mut app, egui::Modifiers::NONE);
        assert!(app.last_executed_regions.contains(&ids[1].to_string()));
        assert!(!app.last_executed_regions.contains(&ids[2].to_string()));
        assert!(app.pending_regions.contains(&ids[2]));
        run(&mut app, command(false));
        assert!(app.last_executed_regions.contains(&ids[2].to_string()));
        assert!(!app.calculation_pending);
        assert!(!app.automatic_calculation);
    }
}
