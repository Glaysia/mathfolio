//! Axis owner routing follows native row/empty-key priority (0157/0179).
//! Production input is segmented before InputState by plot_input_queue.
use super::*;
use inline_edit::{PlotField, PlotFieldPart, SlotFocus, SlotFocusEdge};
use mathfolio_editor::{Direction, Editor};
#[derive(Default)]
pub(super) struct Routed {
    pub changed: bool,
    pub error: Option<String>,
}
fn command_key(ui: &mut egui::Ui, key: egui::Key) -> bool {
    ui.input_mut(|i|{let at=i.events.iter().position(|e|matches!(e,egui::Event::Key{key:k,pressed:true,modifiers,..} if *k==key&&modifiers.command&&!modifiers.alt&&!modifiers.shift));
 if let Some(at)=at{i.events.remove(at);true}else{false}})
}
fn plain_pressed(ui: &egui::Ui, key: egui::Key) -> bool {
    ui.input(|i|i.events.iter().any(|e|matches!(e,egui::Event::Key{key:k,pressed:true,modifiers,..} if *k==key&&modifiers.is_none())))
}
fn inner_row_owner(editor: &Editor) -> bool {
    // The original queries nearest Nary, not just Matrix. Every unsupported
    // container on the live caret path conservatively keeps the inner command;
    // an unsupported/over-budget row command never falls through to a new axis.
    let mut at = Some(editor.caret().node);
    while let Some(id) = at {
        let Some(node) = editor.arena().get(id) else {
            return true;
        };
        match node.kind() {
            NodeKind::Placeholder
            | NodeKind::Identifier(_)
            | NodeKind::AnnotatedIdentifier { .. }
            | NodeKind::NativeIdentifier { .. }
            | NodeKind::Number(_)
            | NodeKind::Imaginary(_)
            | NodeKind::ImaginarySymbol { .. }
            | NodeKind::UnaryNeg
            | NodeKind::LogicalNot
            | NodeKind::Root
            | NodeKind::Absolute
            | NodeKind::Factorial
            | NodeKind::Percent
            | NodeKind::Degree
            | NodeKind::Transpose
            | NodeKind::Conjugate
            | NodeKind::Vectorize
            | NodeKind::Determinant
            | NodeKind::Fraction
            | NodeKind::Power
            | NodeKind::Group
            | NodeKind::Binary(
                BinaryOp::Add
                | BinaryOp::Subtract
                | BinaryOp::Multiply
                | BinaryOp::Scale
                | BinaryOp::InlineDivide
                | BinaryOp::CrossProduct
                | BinaryOp::Polar
                | BinaryOp::Equal
                | BinaryOp::NotEqual
                | BinaryOp::LessThan
                | BinaryOp::LessOrEqual
                | BinaryOp::GreaterThan
                | BinaryOp::GreaterOrEqual
                | BinaryOp::And
                | BinaryOp::Or
                | BinaryOp::Xor,
            ) => (),
            _ => return true,
        }
        at = node.parent();
    }
    false
}
pub(super) fn route(
    ui: &mut egui::Ui,
    source: &PlotSpec,
    edit: &mut InlinePlotEdit,
    active: Option<(PlotField, egui::Id)>,
    composing: bool,
) -> Routed {
    let Some((field, id)) = active else {
        return Routed::default();
    };
    if composing || ui.ctx().any_popup_open() || ui.input(|i| i.pointer.any_pressed()) {
        return Routed::default();
    }
    if edit.can_undo_slot() && command_key(ui, egui::Key::Z) {
        return Routed {
            changed: edit.undo_slot().is_some(),
            error: None,
        };
    }
    // Ctrl+Shift+Z remains the original infinity command.
    if edit.can_redo_slot() && command_key(ui, egui::Key::Y) {
        return Routed {
            changed: edit.redo_slot().is_some(),
            error: None,
        };
    }
    if edit.validation_error().is_some() {
        return Routed::default();
    }
    let shift_enter=ui.input(|i|i.events.iter().any(|e|matches!(e,egui::Event::Key{key:egui::Key::Enter,pressed:true,modifiers,..} if modifiers.shift_only())));
    if shift_enter {
        if field.part == PlotFieldPart::Scaling || edit.expression_is_typed(source, field.axis) {
            let editor = if field.part == PlotFieldPart::Scaling {
                edit.scaling_editor(source, field.axis)
            } else {
                edit.expression_editor(source, field.axis)
            };
            match editor {
                Ok(editor) if inner_row_owner(editor) => return Routed::default(),
                Err(error) => {
                    return Routed {
                        changed: false,
                        error: Some(error),
                    };
                }
                _ => (),
            }
        }
        let left = if field.part != PlotFieldPart::Expression {
            false
        } else if edit.expression_is_typed(source, field.axis) {
            edit.expression_editor(source, field.axis)
                .is_ok_and(|editor| {
                    let mut first = editor.clone_without_history();
                    first.move_to_expression_extreme(false);
                    let actual = editor.caret();
                    let start = first.caret();
                    actual.node == start.node
                        && actual.caret_side == start.caret_side
                        && actual.node_side == start.node_side
                        && actual.value_range.map(|v| v.focus) == start.value_range.map(|v| v.focus)
                })
        } else {
            egui::text_edit::TextEditState::load(ui.ctx(), id)
                .and_then(|s| s.cursor.char_range())
                .is_some_and(|c| c.primary.index.0 == 0)
        };
        super::take_exact_key(ui, egui::Key::Enter, egui::Modifiers::SHIFT);
        let result: Result<inline_edit::SlotEditOutcome, String> =
            edit.insert_slot(source, field.axis, field.part, left);
        return match result {
            Ok(outcome) => {
                if outcome.changed {
                    edit.attach_slot_widget_history(
                        ui,
                        SlotFocus {
                            field: field.axis,
                            part: field.part,
                            edge: SlotFocusEdge::Preserve,
                        },
                        id,
                        outcome.focus,
                    );
                }
                Routed {
                    changed: outcome.changed,
                    error: None,
                }
            }
            Err(error) => Routed {
                changed: false,
                error: Some(error),
            },
        };
    }
    for (key, direction) in [
        (egui::Key::Backspace, Direction::Left),
        (egui::Key::Delete, Direction::Right),
    ] {
        if plain_pressed(ui, key) {
            return match edit.remove_empty_slot(source, field.axis, field.part, direction) {
                Ok(Some(outcome)) => {
                    if outcome.changed {
                        edit.attach_slot_widget_history(
                            ui,
                            SlotFocus {
                                field: field.axis,
                                part: field.part,
                                edge: SlotFocusEdge::Preserve,
                            },
                            id,
                            outcome.focus,
                        );
                    }
                    super::take_plain_key(ui, key);
                    Routed {
                        changed: outcome.changed,
                        error: None,
                    }
                }
                Ok(None) => Routed::default(),
                Err(error) => Routed {
                    changed: false,
                    error: Some(error),
                },
            };
        }
    }
    Routed::default()
}
pub(super) fn focus_id(
    ui: &egui::Ui,
    edit: &InlinePlotEdit,
    target: SlotFocus,
) -> Option<egui::Id> {
    match target.part {
        PlotFieldPart::Expression => {
            if edit.expression_is_typed_for_field(target.field) {
                edit.expression_widget_id(target.field)
            } else {
                Some(edit.field_id(ui, target.field))
            }
        }
        PlotFieldPart::Scaling => edit.scaling_widget_id(target.field),
    }
}
pub(super) fn position_focus(
    ui: &egui::Ui,
    source: &PlotSpec,
    edit: &mut InlinePlotEdit,
    target: SlotFocus,
) {
    if target.edge == SlotFocusEdge::Preserve {
        return;
    }
    let end = target.edge == SlotFocusEdge::End;
    match target.part {
        PlotFieldPart::Expression => {
            if edit.expression_is_typed(source, target.field) {
                if let Ok(editor) = edit.expression_editor(source, target.field) {
                    editor.move_to_expression_extreme(end);
                }
                return;
            }
            let Some(text) =
                inline_edit::field_expression(edit.draft().unwrap_or(source), target.field)
            else {
                return;
            };
            let index = if end { text.chars().count() } else { 0 };
            let id = edit.field_id(ui, target.field);
            let mut state = egui::text_edit::TextEditState::load(ui.ctx(), id).unwrap_or_default();
            state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::one(
                    egui::text::CCursor::new(index),
                )));
            state.store(ui.ctx(), id);
        }
        PlotFieldPart::Scaling => {
            if let Ok(editor) = edit.scaling_editor(source, target.field) {
                editor.move_to_expression_extreme(end);
            }
        }
    }
}
