//! Bounded XY plotting through the same structural worksheet evaluator as math.
//! Inline rows edit authored expressions and typed scaling trees. Full native
//! operator traversal and measured legend layout remain outside this subset.
use eframe::egui::{self, Color32, Response, Stroke};
use mathcraft_compat::{Value, editor_bridge};
use mathcraft_core::{Expr, Quantity};
use mathcraft_document::canvas::{
    PlotLineStyle, PlotMarker, PlotSource, PlotSpec, PlotTraceSpec, PlotTraceStyle,
};
use mathcraft_editor::{BinaryOp, NodeKind, PortableTree};

mod density;
mod inline_edit;
pub(crate) use inline_edit::InlinePlotEdit;
mod render_points;
#[cfg(test)]
mod slot_key_tests;
mod slot_keys;
#[cfg(test)]
mod string_source_tests;
pub(crate) use render_points::CurvePoints;
mod trace_outcome;
pub(crate) use trace_outcome::TraceOutcome;
mod axis_paint;
mod axis_units;
mod calculation;
pub(crate) use axis_units::TracePresentation;
#[cfg(test)]
pub(crate) use calculation::evaluate;
pub(crate) use calculation::evaluate_in_context;
#[cfg(test)]
mod unit_calculation_tests;

pub(crate) struct DrawResponse {
    pub response: Response,
    pub expression_changed: bool,
    pub commit_requested: bool,
    pub resize_started: bool,
    pub resize_before: Option<Box<PlotSpec>>,
    pub input_error: Option<String>,
}

fn take_plain_key(ui: &mut egui::Ui, key: egui::Key) -> bool {
    take_exact_key(ui, key, egui::Modifiers::NONE)
}

fn take_exact_key(ui: &mut egui::Ui, key: egui::Key, expected: egui::Modifiers) -> bool {
    ui.input_mut(|input| {
        let at=input.events.iter().position(|event|matches!(event,egui::Event::Key{key:pressed,pressed:true,modifiers,..} if *pressed==key && *modifiers==expected));
        if let Some(at)=at {input.events.remove(at);true}else{false}
    })
}

fn abort_inline_ime(ui: &mut egui::Ui, edit: &mut InlinePlotEdit, source: &PlotSpec) -> bool {
    let Some(abort) = edit.abort_ime(source) else {
        return false;
    };
    let id = edit.field_id(ui, abort.field);
    let mut state = egui::text_edit::TextEditState::load(ui.ctx(), id).unwrap_or_default();
    // Retain the local undoer, but collapse the preedit selection so a late
    // empty IME event cannot delete restored confirmed characters.
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::one(
            egui::text::CCursor::new(abort.caret),
        )));
    state.store(ui.ctx(), id);
    abort.changed
}

const MAX_SAMPLES: usize = 2000;
const MAX_SAVED_PLOT_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(target_arch = "wasm32", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct PlotData {
    pub traces: Vec<TraceOutcome>,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(target_arch = "wasm32", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct PlotTraceData {
    pub points: CurvePoints,
    pub implicit_domain: bool,
    pub label: String,
    pub style: PlotTraceStyle,
    pub presentation: Option<std::sync::Arc<TracePresentation>>,
}

fn valid_saved_traces(traces: &[PlotTraceData]) -> bool {
    if traces.is_empty()
        || traces.len() > PlotSpec::MAX_TRACES
        || traces.iter().any(|trace| {
            trace.points.len() > MAX_SAMPLES
                || trace
                    .presentation
                    .as_ref()
                    .is_some_and(|p| p.validate().is_err())
                || trace
                    .points
                    .iter()
                    .flatten()
                    .any(|coordinate| !coordinate.is_finite())
        })
    {
        return false;
    }
    // Reuse the persisted document's bounds and style invariants, including
    // marker frequency (zero would panic when drawing with step_by).
    PlotSpec {
        source: PlotSource::legacy(
            "x",
            "sin(x)",
            traces
                .iter()
                .map(|trace| PlotTraceSpec {
                    label: trace.label.clone(),
                    style: trace.style,
                    ..Default::default()
                })
                .collect(),
        ),
        ..Default::default()
    }
    .validate()
    .is_ok()
}

fn tree(kind: NodeKind, children: Vec<PortableTree>) -> PortableTree {
    PortableTree { kind, children }
}

fn expression_tree(
    expression: &Expr,
    replace: Option<(&str, Quantity)>,
) -> Result<PortableTree, String> {
    let convert = |e| expression_tree(e, replace);
    Ok(match expression {
        Expr::Number(value) => tree(NodeKind::Number(value.to_string()), vec![]),
        Expr::Name(name) => match replace {
            Some((symbol, value)) if name == symbol => axis_units::quantity_tree(value)?,
            _ => tree(NodeKind::Identifier(name.clone()), vec![]),
        },
        Expr::Neg(a) => tree(NodeKind::UnaryNeg, vec![convert(a)?]),
        Expr::Binary(op, a, b) => tree(
            match op {
                '+' => NodeKind::Binary(BinaryOp::Add),
                '-' => NodeKind::Binary(BinaryOp::Subtract),
                '*' => NodeKind::Binary(BinaryOp::Multiply),
                '/' => NodeKind::Fraction,
                '^' => NodeKind::Power,
                _ => return Err(format!("Unsupported plot operator {op}")),
            },
            vec![convert(a)?, convert(b)?],
        ),
        Expr::Call(name, arguments) => {
            let mut children = vec![tree(NodeKind::Identifier(name.clone()), vec![])];
            children.extend(
                arguments
                    .iter()
                    .map(convert)
                    .collect::<Result<Vec<_>, _>>()?,
            );
            tree(
                NodeKind::Call {
                    argument_count: arguments.len(),
                },
                children,
            )
        }
    })
}

fn evaluation(expression: &Expr, replace: Option<(&str, Quantity)>) -> Result<String, String> {
    editor_bridge::tree_to_mathml(&tree(
        NodeKind::Binary(BinaryOp::Evaluate),
        vec![
            expression_tree(expression, replace)?,
            tree(NodeKind::Placeholder, vec![]),
        ],
    ))
}

fn contains_name(expression: &Expr, symbol: &str) -> bool {
    match expression {
        Expr::Name(name) => name == symbol,
        Expr::Neg(a) => contains_name(a, symbol),
        Expr::Binary(_, a, b) => contains_name(a, symbol) || contains_name(b, symbol),
        Expr::Call(_, arguments) => arguments.iter().any(|a| contains_name(a, symbol)),
        Expr::Number(_) => false,
    }
}

fn fault(error: &mathcraft_compat::Fault) -> String {
    format!("{}: {}", error.code, error.detail)
}

fn query(
    context: &mathcraft_compat::CalculationContext,
    expressions: &[String],
) -> Vec<mathcraft_compat::RegionEvaluation> {
    expressions
        .iter()
        .map(|expression| {
            let (value, error) = match context.evaluate_mathml(expression) {
                Ok(value) => (Some(value), None),
                Err(error) => (None, Some(error)),
            };
            mathcraft_compat::RegionEvaluation {
                region_id: String::new(),
                result_ref: None,
                left: 0.,
                top: 0.,
                value,
                display_value: None,
                error,
                redefinition_warning: false,
                matrix_origin: None,
            }
        })
        .collect()
}

fn trace_color(style: PlotTraceStyle) -> Color32 {
    Color32::from_rgba_unmultiplied(
        style.color[0],
        style.color[1],
        style.color[2],
        style.color[3],
    )
}

fn line_style(style: PlotLineStyle) -> Option<egui_plot::LineStyle> {
    match style {
        PlotLineStyle::None => None,
        PlotLineStyle::Solid => Some(egui_plot::LineStyle::Solid),
        PlotLineStyle::Dotted => Some(egui_plot::LineStyle::Dotted { spacing: 5. }),
        PlotLineStyle::Dashed => Some(egui_plot::LineStyle::Dashed { length: 8. }),
        // egui_plot has no alternating dash primitive. These keep the traces
        // distinguishable without claiming pixel parity with Prime.
        PlotLineStyle::DashDot => Some(egui_plot::LineStyle::Dashed { length: 5. }),
        PlotLineStyle::DashDotDot => Some(egui_plot::LineStyle::Dotted { spacing: 8. }),
    }
}

fn marker_shape(marker: PlotMarker, filled: bool) -> Option<(egui_plot::MarkerShape, bool)> {
    match marker {
        PlotMarker::None => None,
        PlotMarker::Cross => Some((egui_plot::MarkerShape::Cross, false)),
        PlotMarker::Plus => Some((egui_plot::MarkerShape::Plus, false)),
        PlotMarker::Circle => Some((egui_plot::MarkerShape::Circle, filled)),
        PlotMarker::Square => Some((egui_plot::MarkerShape::Square, filled)),
        PlotMarker::Diamond => Some((egui_plot::MarkerShape::Diamond, filled)),
        PlotMarker::Point => Some((egui_plot::MarkerShape::Circle, true)),
        PlotMarker::Star => Some((egui_plot::MarkerShape::Asterisk, false)),
        PlotMarker::Triangle => Some((egui_plot::MarkerShape::Up, filled)),
    }
}

fn sparse_markers(
    data: &PlotData,
) -> [Option<std::sync::Arc<[egui_plot::PlotPoint]>>; PlotSpec::MAX_TRACES] {
    std::array::from_fn(|i| {
        data.traces
            .get(i)
            .and_then(TraceOutcome::curve)
            .and_then(|trace| {
                (marker_shape(trace.style.marker, trace.style.marker_filled).is_some()
                    && trace.style.marker_frequency > 1)
                    .then(|| trace.points.sparse(trace.style.marker_frequency as usize))
            })
    })
}

fn draw_trace<'a>(
    plot: &mut egui_plot::PlotUi<'a>,
    trace: &'a PlotTraceData,
    index: usize,
    sparse: Option<&'a [egui_plot::PlotPoint]>,
) {
    let color = trace_color(trace.style);
    let name = if trace.label.is_empty() {
        format!("trace {}", index + 1)
    } else {
        trace.label.clone()
    };
    let line = line_style(trace.style.line);
    if let Some(line_style) = line
        && trace.points.len() > 1
    {
        plot.line(
            egui_plot::Line::new(name.clone(), trace.points.rendered())
                .color(color)
                .width(trace.style.line_width)
                .style(line_style),
        );
    }
    let marker = marker_shape(trace.style.marker, trace.style.marker_filled)
        .or_else(|| (trace.points.len() == 1).then_some((egui_plot::MarkerShape::Circle, true)));
    if let Some((shape, filled)) = marker {
        let points = sparse.unwrap_or_else(|| trace.points.rendered());
        let marker_name = if line.is_some() { String::new() } else { name };
        plot.points(
            egui_plot::Points::new(marker_name, points)
                .radius(trace.style.marker_size)
                .shape(shape)
                .filled(filled)
                .color(color),
        );
    }
}

fn paint_trace_diagnostics(ui: &egui::Ui, data: &PlotData, rect: egui::Rect) {
    let mut row = 0;
    for (index, trace) in data.traces.iter().enumerate() {
        if let Some(message) = trace.diagnostic() {
            let color = if matches!(trace, TraceOutcome::Unavailable(_)) {
                Color32::from_rgb(140, 90, 15)
            } else {
                Color32::DARK_RED
            };
            density::painter_text(
                &ui.painter().with_clip_rect(rect),
                rect.left_top() + egui::vec2(8., 6. + row as f32 * 15.),
                format!("trace {}: {message}", index + 1),
                egui::FontId::proportional(11.),
                color,
            );
            row += 1;
        }
    }
}

/// Print cached traces and their source axis expressions without editing
/// controls or resize handles. Data and axis ranges use the normal renderer.
pub(crate) fn readonly_expression_ui(
    ui: &mut egui::Ui,
    source: &mathcraft_document::canvas::PlotExpression,
    id: egui::Id,
) {
    axis_paint::expression(ui, source.as_ref(), id);
}

pub(crate) fn draw_print(ui: &mut egui::Ui, spec: &PlotSpec, data: &Result<PlotData, String>) {
    let size = egui::vec2(
        spec.width.clamp(120., 2000.),
        spec.height.clamp(100., 2000.),
    );
    let fields = inline_edit::fields(spec);
    ui.allocate_ui_with_layout(size, egui::Layout::top_down(egui::Align::Min), |ui| {
        ui.set_width(size.x);
        ui.spacing_mut().item_spacing = egui::vec2(3., 2.);
        let rows_top = ui.cursor().min.y;
        let mut row_height = 0_f32;
        for field in fields.iter().copied().filter(|f| !f.x) {
            let row = ui.horizontal(|ui| {
                density::axis_label(ui, "y");
                if let Some(source) = inline_edit::field_expression_ref(spec, field) {
                    axis_paint::expression(
                        ui,
                        source,
                        ui.id().with(("printed-expression", field.x, field.index)),
                    );
                }
                axis_paint::source(
                    ui,
                    spec.equation_sources(field.x)
                        .nth(field.index)
                        .and_then(|e| e.scaling),
                    ui.id().with(("printed-scaling", field.x, field.index)),
                );
                axis_paint::residual(ui, spec, spec, data, field);
            });
            row_height = row_height.max(row.response.rect.height());
        }
        let y_height = ui.cursor().min.y - rows_top;
        let x_height = fields.iter().filter(|f| f.x).count() as f32
            * (row_height + ui.spacing().item_spacing.y);
        let body_height = (size.y - y_height - x_height).max(45.);
        match data {
            Ok(data) => {
                let mut plot = egui_plot::Plot::new(ui.id().with("printed-xy"))
                    .width(size.x)
                    .height(body_height)
                    .allow_scroll(false)
                    .allow_drag(false)
                    .allow_zoom(false)
                    .allow_axis_zoom_drag(false)
                    .allow_double_click_reset(false)
                    .show_x(false)
                    .show_y(false)
                    .show_background(false)
                    .show_grid(false);
                if !spec.x_axis.label.is_empty() {
                    plot = plot.x_axis_label(density::axis_title(ui, &spec.x_axis.label));
                }
                if !spec.y_axis.label.is_empty() {
                    plot = plot.y_axis_label(density::axis_title(ui, &spec.y_axis.label));
                }
                if let Some(range) = spec.x_axis.range {
                    plot = plot.default_x_bounds(range.min, range.max);
                }
                if let Some(range) = spec.y_axis.range {
                    plot = plot.default_y_bounds(range.min, range.max);
                }
                if data
                    .traces
                    .iter()
                    .filter_map(TraceOutcome::curve)
                    .any(|trace| !trace.label.is_empty())
                {
                    plot = plot.legend(egui_plot::Legend::default());
                }
                let markers = sparse_markers(data);
                let plotted = plot.show(ui, |plot| {
                    for (index, trace) in data.traces.iter().enumerate() {
                        if let Some(curve) = trace.curve() {
                            draw_trace(plot, curve, index, markers[index].as_deref());
                        }
                    }
                });
                paint_trace_diagnostics(ui, data, plotted.response.rect);
            }
            Err(error) => {
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(size.x, body_height), egui::Sense::hover());
                ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink(10.)))
                    .colored_label(Color32::DARK_RED, error);
            }
        }
        for field in fields.iter().copied().filter(|f| f.x) {
            ui.horizontal(|ui| {
                density::axis_label(ui, "x");
                if let Some(source) = inline_edit::field_expression_ref(spec, field) {
                    axis_paint::expression(
                        ui,
                        source,
                        ui.id().with(("printed-expression", field.x, field.index)),
                    );
                }
                axis_paint::source(
                    ui,
                    spec.equation_sources(field.x)
                        .nth(field.index)
                        .and_then(|e| e.scaling),
                    ui.id().with(("printed-scaling", field.x, field.index)),
                );
                axis_paint::residual(ui, spec, spec, data, field);
            });
        }
    });
}

/// Typed scaling editor: a local input transaction never mutates committed source.
fn draw_scaling(
    ui: &mut egui::Ui,
    spec: &PlotSpec,
    edit: &mut InlinePlotEdit,
    field: inline_edit::AxisField,
    id: egui::Id,
) -> (bool, Option<egui::Id>) {
    let pending = edit.scaling_widget_id(field).and_then(|widget| {
        edit.begin_field_command(ui, spec, field, inline_edit::PlotFieldPart::Scaling, widget)
    });
    let painted = ui
        .scope_builder(egui::UiBuilder::new().id(id), |ui| {
            let editor = edit.scaling_editor(spec, field)?;
            let history_key=ui.input(|i|i.events.iter().any(|event|matches!(event,egui::Event::Key{key:egui::Key::Z|egui::Key::Y,pressed:true,modifiers,..} if modifiers.command&&!modifiers.shift&&!modifiers.alt)));
            if history_key {
                return Ok::<_,String>(crate::formula::formula_ui_styled(ui,editor,None,false,None,11.).response);
            }
            let before = editor.arena().revision();
            Ok::<_, String>(editor.edit_group(|editor| {
                let response =
                    crate::formula::formula_ui_styled(ui, editor, None, false, None, 11.);
                if editor.arena().revision() != before {
                    editor.auto_label_scaling_identifiers();
                }
                response.response
            }))
        })
        .inner;
    let Ok(response) = painted else {
        return (false, None);
    };
    edit.record_scaling_widget(field, response.id);
    let changed = match edit.stage_scaling(spec, field) {
        Ok(changed) => changed,
        Err(error) => {
            response.clone().on_hover_text(error);
            false
        }
    };
    edit.finish_field_command(ui, spec, pending);
    (changed, response.has_focus().then_some(response.id))
}

/// First math retains explicit labels/context/literal subscripts. It has no
/// scaling-field default label pass and never promotes LegacyText by focus.
fn draw_expression(
    ui: &mut egui::Ui,
    spec: &PlotSpec,
    edit: &mut InlinePlotEdit,
    field: inline_edit::AxisField,
    alias: egui::Id,
) -> egui::Response {
    let old_widget = edit.expression_widget_id(field);
    if let Err(error) = edit.expression_editor(spec, field) {
        return ui.colored_label(Color32::RED, error);
    }
    let generation = edit
        .expression_generation(field)
        .expect("prepared typed plot expression");
    if edit.expression_widget_id(field) != old_widget
        && let Some(old) = old_widget
    {
        ui.memory_mut(|m| m.surrender_focus(old));
        ui.data_mut(|d| d.remove::<String>(old.with("ime-preedit")));
    }
    let painted=ui.scope_builder(egui::UiBuilder::new().id(alias.with(("typed-expression",generation))),|ui|{
        let editor=edit.expression_editor(spec,field)?;
        let history_key=ui.input(|i|i.events.iter().any(|event|matches!(event,egui::Event::Key{key:egui::Key::Z|egui::Key::Y,pressed:true,modifiers,..} if modifiers.command&&!modifiers.shift&&!modifiers.alt)));
        if history_key {return Ok::<_,String>(crate::formula::formula_ui_styled(ui,editor,None,false,None,11.).response);}
        Ok(editor.edit_group(|editor|crate::formula::formula_ui_styled(ui,editor,None,false,None,11.).response))
    }).inner;
    let mut response = match painted {
        Ok(response) => response,
        Err(error) => return ui.colored_label(Color32::RED, error),
    };
    edit.record_expression_widget(field, response.id);
    if ui.memory(|m| m.has_focus(alias)) {
        response.request_focus();
    }
    match edit.stage_expression(spec, field) {
        Ok(true) => response.mark_changed(),
        Ok(false) => (),
        Err(error) => {
            response.clone().on_hover_text(error);
        }
    }
    response
}

/// In-sheet axis drafts and interactive axes. Expressions never mutate source;
/// commit/calculation belong to the worksheet controller. Resize changes only
/// dimensions, and panning changes neither source nor draft.
pub(crate) fn draw(
    ui: &mut egui::Ui,
    spec: &mut PlotSpec,
    edit: &mut InlinePlotEdit,
    data: &Result<PlotData, String>,
    focus: bool,
) -> DrawResponse {
    let size = egui::vec2(
        spec.width.clamp(120., 2000.),
        spec.height.clamp(100., 2000.),
    );
    let focus_id = ui.make_persistent_id("plot_had_focus");
    let had_focus = ui.data(|d| d.get_temp::<bool>(focus_id).unwrap_or(false));
    ui.data_mut(|d| d.insert_temp(focus_id, focus));
    let mut fields = edit.prepare_fields(spec);
    let mut ids: Vec<_> = fields
        .iter()
        .map(|field| edit.field_id(ui, *field))
        .collect();
    use inline_edit::PlotFieldPart;
    let mut traversal: Vec<inline_edit::PlotField> = inline_edit::traversal_fields(&fields);
    let mut focus_ids: Vec<_> = traversal
        .iter()
        .map(|field| match field.part {
            PlotFieldPart::Expression => edit
                .expression_widget_id(field.axis)
                .unwrap_or_else(|| edit.field_id(ui, field.axis)),
            PlotFieldPart::Scaling => edit
                .scaling_widget_id(field.axis)
                .unwrap_or_else(|| edit.scaling_id(ui, field.axis)),
        })
        .collect();
    let focused_index = ui.memory(|memory| focus_ids.iter().position(|id| memory.has_focus(*id)));
    let active_scaling = focused_index
        .and_then(|i| (traversal[i].part == PlotFieldPart::Scaling).then_some(traversal[i].axis));
    edit.scaling_focus_changed(ui.ctx(), active_scaling);
    let active_typed = focused_index.and_then(|i| {
        (traversal[i].part == PlotFieldPart::Expression
            && edit.expression_is_typed(spec, traversal[i].axis))
        .then_some(traversal[i].axis)
    });
    edit.expression_focus_changed(ui.ctx(), active_typed);
    let field_was_focused = focused_index.is_some();
    let had_changes = edit.has_changes();
    let mut composition_aborted = if edit
        .ime_field()
        .is_some_and(|field| !ui.memory(|m| m.has_focus(edit.field_id(ui, field))))
    {
        abort_inline_ime(ui, edit, spec)
    } else {
        false
    };
    if let Some(index) = focused_index
        && traversal[index].part == PlotFieldPart::Expression
        && !edit.expression_is_typed(spec,traversal[index].axis)
        && edit.ime_field().is_none()
        && ui.input(|input| input.events.iter().any(|event| matches!(event,egui::Event::Ime(egui::ImeEvent::Preedit{text,..}) if !text.is_empty()))) {
        let caret = egui::text_edit::TextEditState::load(ui.ctx(), focus_ids[index])
            .and_then(|state|state.cursor.char_range()).map(|range|range.sorted_cursors()[0].index.0);
        edit.begin_ime(spec, traversal[index].axis, caret);
    }
    let composition_key = if let Some(field) = active_scaling {
        ui.input(|input| edit.observe_scaling_ime(field, &input.events))
    } else if let Some(field) = active_typed {
        ui.input(|input| edit.observe_expression_ime(field, &input.events))
    } else if field_was_focused {
        ui.input(|input| edit.observe_ime(&input.events))
    } else {
        edit.is_composing()
    };
    if composition_key {
        // A single-line TextEdit otherwise surrenders focus on Shift+Enter,
        // aborting its preedit before the owning input method confirms it.
        take_exact_key(ui, egui::Key::Enter, egui::Modifiers::SHIFT);
    }
    let routed = slot_keys::route(
        ui,
        spec,
        edit,
        focused_index.map(|i| (traversal[i], focus_ids[i])),
        composition_key,
    );
    edit.observe_native_history_command(ui);
    edit.apply_pending_widget_history(ui.ctx());
    let slot_focus = edit.take_slot_focus();
    if routed.changed || slot_focus.is_some() {
        fields = edit.prepare_fields(spec);
        ids = fields.iter().map(|f| edit.field_id(ui, *f)).collect();
        traversal = inline_edit::traversal_fields(&fields);
        focus_ids = traversal
            .iter()
            .map(|f| match f.part {
                PlotFieldPart::Expression => edit
                    .expression_widget_id(f.axis)
                    .unwrap_or_else(|| edit.field_id(ui, f.axis)),
                PlotFieldPart::Scaling => edit
                    .scaling_widget_id(f.axis)
                    .unwrap_or_else(|| edit.scaling_id(ui, f.axis)),
            })
            .collect();
        ui.ctx().request_repaint();
    }
    if let Some(target) = slot_focus {
        slot_keys::position_focus(ui, spec, edit, target);
        if let Some(id) = slot_keys::focus_id(ui, edit, target) {
            ui.memory_mut(|m| m.request_focus(id));
        }
    }
    // Intercept only internal transitions. Outbound traversal belongs to egui.
    let tab_target = if !routed.changed
        && slot_focus.is_none()
        && !composition_key
        && !ui.input(|input| input.pointer.any_pressed())
    {
        let from = ui
            .memory(|m| focus_ids.iter().position(|id| m.had_focus_last_frame(*id)))
            .or(focused_index);
        if let Some(from) = from {
            if from + 1 < focus_ids.len() && take_plain_key(ui, egui::Key::Tab) {
                Some(focus_ids[from + 1])
            } else if from > 0 && take_exact_key(ui, egui::Key::Tab, egui::Modifiers::SHIFT) {
                Some(focus_ids[from - 1])
            } else if traversal[from].part == PlotFieldPart::Scaling
                && from + 1 == focus_ids.len()
                && take_plain_key(ui, egui::Key::Tab)
            {
                // A structural formula normally owns Tab for operand traversal.
                // At the last scaling field this is the enclosing plot's exit.
                ui.memory_mut(|m| m.move_focus(egui::FocusDirection::Next));
                None
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };
    if let Some(target) = tab_target {
        ui.memory_mut(|memory| {
            memory.move_focus(egui::FocusDirection::None);
            memory.request_focus(target);
        });
    }
    // egui can release keyboard focus on Escape before widgets are drawn. The
    // previous field focus is relevant only if no pointer/popup/other widget
    // has acquired this key; never turn a global Escape into a plot command.
    let escaped_field = ui
        .memory(|memory| focus_ids.iter().any(|id| memory.had_focus_last_frame(*id)))
        && ui.memory(|memory| memory.focused().is_none())
        && !ui.input(|input| input.pointer.any_pressed())
        && !ui.ctx().any_popup_open();
    let cancelled = (field_was_focused || escaped_field) && take_plain_key(ui, egui::Key::Escape);
    // Swallow the editor's Enter while composing too: allowing the single-line
    // TextEdit to surrender focus would turn IME confirmation into owner commit.
    let enter_pressed = !cancelled && field_was_focused && take_plain_key(ui, egui::Key::Enter);
    let commit_requested = enter_pressed && !composition_key;
    let mut legacy_layouter = density::singleline;
    let mut focused_field = None;
    let mut changed = routed.changed;
    let region = ui.allocate_ui_with_layout(size, egui::Layout::top_down(egui::Align::Min), |ui| {
        ui.set_width(size.x);
        ui.spacing_mut().item_spacing = egui::vec2(3., 2.);
        let rows_top = ui.cursor().min.y;
        let mut row_height = 0_f32;
        for (index, field) in fields.iter().copied().enumerate().filter(|(_, f)| !f.x) {
            let row = ui.horizontal(|ui| {
                density::axis_label(ui, "y");
                let pending = edit.begin_field_command(
                    ui,
                    spec,
                    field,
                    inline_edit::PlotFieldPart::Expression,
                    edit.expression_widget_id(field).unwrap_or(ids[index]),
                );
                let y = if edit.expression_is_typed(spec, field) {
                    draw_expression(ui, spec, edit, field, ids[index])
                } else {
                    ui.add(
                        egui::TextEdit::singleline(&mut edit.field_buffer(spec, field))
                            .id(ids[index])
                            .desired_width((size.x - 115.).max(40.))
                            .char_limit(4096)
                            .layouter(&mut legacy_layouter)
                            .frame(if focus || field_was_focused {
                                egui::Frame::new()
                                    .inner_margin(2.)
                                    .stroke(Stroke::new(0.5, Color32::from_gray(170)))
                            } else {
                                egui::Frame::NONE
                            }),
                    )
                };
                if index == 0 && focus && !had_focus && !field_was_focused {
                    y.request_focus();
                }
                if y.has_focus() {
                    focused_field = Some(y.id);
                }
                edit.finish_field_command(ui, spec, pending);
                changed |= y.changed();
                let (scaling_changed, scaling_focus) =
                    draw_scaling(ui, spec, edit, field, ids[index].with("scaling"));
                changed |= scaling_changed;
                focused_field = scaling_focus.or(focused_field);
                axis_paint::residual(ui, spec, edit.draft().unwrap_or(spec), data, field);
            });
            row_height = row_height.max(row.response.rect.height());
        }
        let y_height = ui.cursor().min.y - rows_top;
        let x_height = fields.iter().filter(|f| f.x).count() as f32
            * (row_height + ui.spacing().item_spacing.y);
        // This is an app allocation policy, not a native graph-size constant.
        // Min-height overflow remains visible without mutating source dimensions.
        let body_height = (size.y - y_height - x_height).max(45.);
        match data {
            Ok(data) => {
                let mut plot = egui_plot::Plot::new(ui.id().with("xy"))
                    .width(size.x)
                    .height(body_height)
                    .allow_scroll(false)
                    .allow_drag(true)
                    .allow_zoom(true)
                    .show_background(false)
                    .show_grid(false);
                if !spec.x_axis.label.is_empty() {
                    plot = plot.x_axis_label(density::axis_title(ui, &spec.x_axis.label));
                }
                if !spec.y_axis.label.is_empty() {
                    plot = plot.y_axis_label(density::axis_title(ui, &spec.y_axis.label));
                }
                if let Some(range) = spec.x_axis.range {
                    plot = plot.default_x_bounds(range.min, range.max);
                }
                if let Some(range) = spec.y_axis.range {
                    plot = plot.default_y_bounds(range.min, range.max);
                }
                if data
                    .traces
                    .iter()
                    .filter_map(TraceOutcome::curve)
                    .any(|trace| !trace.label.is_empty())
                {
                    plot = plot.legend(egui_plot::Legend::default());
                }
                let markers = sparse_markers(data);
                let plotted = plot.show(ui, |plot| {
                    for (index, trace) in data.traces.iter().enumerate() {
                        if let Some(curve) = trace.curve() {
                            draw_trace(plot, curve, index, markers[index].as_deref());
                        }
                    }
                });
                paint_trace_diagnostics(ui, data, plotted.response.rect);
            }
            Err(error) => {
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(size.x, body_height), egui::Sense::hover());
                ui.painter().rect_stroke(
                    rect,
                    0.,
                    Stroke::new(0.5, Color32::from_gray(170)),
                    egui::StrokeKind::Inside,
                );
                ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink(10.)))
                    .colored_label(Color32::from_rgb(170, 25, 25), error);
            }
        }
        for (index, field) in fields.iter().copied().enumerate().filter(|(_, f)| f.x) {
            ui.horizontal(|ui| {
                density::axis_label(ui, "x");
                let pending = edit.begin_field_command(
                    ui,
                    spec,
                    field,
                    inline_edit::PlotFieldPart::Expression,
                    edit.expression_widget_id(field).unwrap_or(ids[index]),
                );
                let x = if edit.expression_is_typed(spec, field) {
                    draw_expression(ui, spec, edit, field, ids[index])
                } else {
                    ui.add(
                        egui::TextEdit::singleline(&mut edit.field_buffer(spec, field))
                            .id(ids[index])
                            .desired_width(
                                if field.index == 0
                                    && data.as_ref().is_ok_and(PlotData::has_implicit_domain)
                                {
                                    (size.x - 210.).max(40.)
                                } else {
                                    (size.x - 115.).max(40.)
                                },
                            )
                            .char_limit(4096)
                            .layouter(&mut legacy_layouter)
                            .frame(if focus || field_was_focused {
                                egui::Frame::new()
                                    .inner_margin(2.)
                                    .stroke(Stroke::new(0.5, Color32::from_gray(170)))
                            } else {
                                egui::Frame::NONE
                            }),
                    )
                };
                if x.has_focus() {
                    focused_field = Some(x.id);
                }
                edit.finish_field_command(ui, spec, pending);
                changed |= x.changed();
                let (scaling_changed, scaling_focus) =
                    draw_scaling(ui, spec, edit, field, ids[index].with("scaling"));
                changed |= scaling_changed;
                focused_field = scaling_focus.or(focused_field);
                axis_paint::residual(ui, spec, edit.draft().unwrap_or(spec), data, field);
                if field.index == 0 && data.as_ref().is_ok_and(PlotData::has_implicit_domain) {
                    ui.label(
                        egui::RichText::new("−10 … 10 / 500점")
                            .size(10.)
                            .color(Color32::from_gray(110)),
                    )
                    .on_hover_text("미정의 독립 변수의 도표 전용 표본 범위입니다.");
                }
            });
        }
    });
    let mut response = region.response;
    if let Some(target) = slot_focus {
        slot_keys::position_focus(ui, spec, edit, target);
        if let Some(id) = slot_keys::focus_id(ui, edit, target) {
            ui.memory_mut(|m| m.request_focus(id));
            focused_field = Some(id);
        }
    }

    let resize_started;
    let resize_before;
    {
        let handle = egui::Rect::from_min_size(
            response.rect.max - egui::vec2(13., 13.),
            egui::vec2(13., 13.),
        );
        let resize = ui
            .interact(handle, ui.id().with("plot_resize"), egui::Sense::drag())
            .on_hover_cursor(egui::CursorIcon::ResizeNwSe);
        resize_started = resize.drag_started();
        resize_before = resize_started.then(|| Box::new(spec.clone()));
        for d in [2., 6., 10.] {
            ui.painter().line_segment(
                [
                    handle.right_bottom() - egui::vec2(d, 1.),
                    handle.right_bottom() - egui::vec2(1., d),
                ],
                Stroke::new(0.7, Color32::GRAY),
            );
        }
        if resize.dragged() {
            let width = (spec.width + resize.drag_delta().x).clamp(120., 2000.);
            let height = (spec.height + resize.drag_delta().y).clamp(100., 2000.);
            // Validate the draft first: rejected geometry must not partially
            // change either source or its pending text snapshot.
            let mut candidate = spec.clone();
            if candidate.set_outer_size(width, height).is_ok() && edit.dimensions(width, height) {
                *spec = candidate;
            }
        }
    }
    // Keep the full plot's rectangle, but report the focused text field's ID so
    // the worksheet does not steal keyboard input or move it while selecting.
    if let Some(id) = focused_field {
        response.id = id;
    }
    if let Some(target) = tab_target {
        ui.memory_mut(|memory| memory.request_focus(target));
        response.id = target;
    }
    if edit.ime_cancelled()
        || edit
            .ime_field()
            .is_some_and(|field| !ui.memory(|memory| memory.has_focus(edit.field_id(ui, field))))
    {
        composition_aborted |= abort_inline_ime(ui, edit, spec);
    } else {
        edit.finish_ime();
    }
    if cancelled {
        edit.cancel();
    }
    edit.normalize(spec);
    let expression_changed =
        (changed || cancelled || composition_aborted) && (had_changes || edit.has_changes());
    if expression_changed || resize_started {
        response.mark_changed();
    }
    DrawResponse {
        response,
        expression_changed,
        commit_requested,
        resize_started,
        resize_before,
        input_error: routed.error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mathcraft_document::canvas::PlotTraceAppearance;
    fn ready(trace: &TraceOutcome) -> &PlotTraceData {
        match trace {
            TraceOutcome::Ready(curve) => curve,
            _ => panic!("expected ready trace"),
        }
    }
    fn ready_mut(trace: &mut TraceOutcome) -> &mut PlotTraceData {
        match trace {
            TraceOutcome::Ready(curve) => curve,
            _ => panic!("expected ready trace"),
        }
    }
    #[test]
    fn independent_trace_failures_and_placeholders_preserve_every_ordinal() {
        for failed in 0..3 {
            let mut ys = vec!["x+1".into(), "x+2".into(), "x+3".into()];
            ys[failed] = "1/(x+10)".into();
            let appearances = (0..3)
                .map(|i| PlotTraceAppearance {
                    label: format!("source {i}"),
                    ..Default::default()
                })
                .collect();
            let spec = PlotSpec {
                source: PlotSource::axis_lists(vec!["x".into()], ys, appearances).unwrap(),
                ..Default::default()
            };
            let data = evaluate_plot(spec, &[]).unwrap();
            assert_eq!(data.traces.len(), 3);
            assert!(data.has_failures());
            for (index, outcome) in data.traces.iter().enumerate() {
                if index == failed {
                    assert!(matches!(outcome, TraceOutcome::Failed(_)));
                    assert!(outcome.curve().is_none());
                } else {
                    let curve = ready(outcome);
                    assert_eq!(curve.label, format!("source {index}"));
                    assert_eq!(curve.points.first(), Some(&[-10., -9. + index as f64]));
                    assert_eq!(curve.points.len(), 500);
                }
            }
            assert_eq!(PlotData::from_saved(&data.to_saved()), Some(data));
        }
        let spec = PlotSpec {
            source: PlotSource::axis_lists(
                vec!["x".into()],
                vec!["x".into(), String::new(), "x^2".into()],
                vec![PlotTraceAppearance::default(); 3],
            )
            .unwrap(),
            ..Default::default()
        };
        let data = evaluate_plot(spec, &[]).unwrap();
        assert!(matches!(data.traces[1], TraceOutcome::Incomplete));
        assert_eq!(ready(&data.traces[0]).points.first(), Some(&[-10., -10.]));
        assert_eq!(ready(&data.traces[2]).points.first(), Some(&[-10., 100.]));
        let blank = evaluate_plot(
            PlotSpec {
                source: PlotSource::single("", ""),
                ..Default::default()
            },
            &[],
        )
        .unwrap();
        assert_eq!(blank.traces, [TraceOutcome::Incomplete]);
        assert!(blank.first_curve().is_none() && !blank.has_failures());
        let shared_bad = PlotSpec {
            source: PlotSource::axis_lists(
                vec!["missing+1".into()],
                vec!["2".into(), "3".into()],
                vec![PlotTraceAppearance::default(); 2],
            )
            .unwrap(),
            ..Default::default()
        };
        let failed = evaluate_plot(shared_bad, &[]).unwrap();
        assert!(
            failed
                .traces
                .iter()
                .all(|slot| matches!(slot, TraceOutcome::Failed(_)))
        );
        assert!(failed.first_curve().is_none());
    }

    #[test]
    fn mixed_trace_render_keeps_successes_and_reports_original_error_ordinal() {
        let ctx = egui::Context::default();
        let curve = |color, points: Vec<[f64; 2]>| PlotTraceData {
            presentation: None,
            points: points.into(),
            implicit_domain: false,
            label: String::new(),
            style: PlotTraceStyle {
                color,
                ..Default::default()
            },
        };
        let data = PlotData {
            traces: vec![
                TraceOutcome::Ready(curve([0, 70, 200, 255], vec![[0., 0.], [1., 1.]])),
                TraceOutcome::Failed("failure belongs here".into()),
                TraceOutcome::Ready(curve([20, 170, 40, 255], vec![[0., 1.], [1., 0.]])),
            ],
        };
        let saved = data.to_saved();
        let spec = PlotSpec {
            source: PlotSource::axis_lists(
                vec!["x".into()],
                vec!["x".into(), "missing".into(), "1-x".into()],
                [[0, 70, 200, 255], [0, 70, 150, 255], [20, 170, 40, 255]]
                    .map(|color| PlotTraceAppearance {
                        style: PlotTraceStyle {
                            color,
                            ..Default::default()
                        },
                        ..Default::default()
                    })
                    .into(),
            )
            .unwrap(),
            ..Default::default()
        };
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            draw_print(ui, &spec, &Ok(data.clone()))
        });
        fn texts(shape: &egui::Shape, out: &mut Vec<String>) {
            match shape {
                egui::Shape::Text(text) => out.push(text.galley.text().into()),
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        texts(shape, out)
                    }
                }
                _ => (),
            }
        }
        let mut labels = vec![];
        for shape in &output.shapes {
            texts(&shape.shape, &mut labels)
        }
        assert!(
            labels
                .iter()
                .any(|text| text == "trace 2: failure belongs here")
        );
        let meshes = ctx.tessellate(std::mem::take(&mut output.shapes), output.pixels_per_point);
        for color in [[0, 70, 200, 255], [20, 170, 40, 255]] {
            assert!(meshes.iter().any(|primitive|matches!(&primitive.primitive,egui::epaint::Primitive::Mesh(mesh) if mesh.vertices.iter().any(|v|v.color.to_array()==color))));
        }
        assert_eq!(data.to_saved(), saved);
        output.drop_without_applying_deltas();
    }

    #[test]
    fn saved_curve_json_keeps_legacy_fields_and_numeric_lexemes_after_render_preparation() {
        let points: CurvePoints = vec![[-0.0, 1e-9], [1.0, 2.5]].into();
        let data = PlotData {
            traces: vec![TraceOutcome::Ready(PlotTraceData {
                presentation: None,
                points,
                implicit_domain: false,
                label: String::new(),
                style: PlotTraceStyle::default(),
            })],
        };
        let expected = r#"{"traces":[{"implicit_domain":false,"label":"","points":[[-0.0,1e-9],[1.0,2.5]],"style":{"color":[0,70,150,255],"line":"Solid","line_width":1.5,"marker":"None","marker_filled":false,"marker_frequency":1,"marker_size":3.0}}],"version":1}"#;
        assert_eq!(data.to_saved(), expected);
        ready(&data.traces[0]).points.rendered();
        ready(&data.traces[0]).points.sparse(3);
        assert_eq!(data.to_saved(), expected);
        let restored = PlotData::from_saved(expected).unwrap();
        assert_eq!(restored, data);
        assert_eq!(
            restored.first_curve().unwrap().points[0][0].to_bits(),
            (-0f64).to_bits()
        );
        assert_eq!(
            restored
                .first_curve()
                .unwrap()
                .points
                .prepared_payload_bytes(),
            0
        );
        assert!(std::ptr::eq(
            restored.first_curve().unwrap().points.as_slice(),
            ready(&restored.traces[0]).points.as_slice()
        ));
    }

    #[test]
    fn printed_axis_lists_show_authored_slots_without_repeating_shared_equations() {
        use mathcraft_document::canvas::PlotTraceAppearance;
        let ctx = egui::Context::default();
        for shared_x in [true, false] {
            let (x, y) = if shared_x {
                (vec!["shared_x".into()], vec!["y1".into(), "y2".into()])
            } else {
                (vec!["x1".into(), "x2".into()], vec!["shared_y".into()])
            };
            let spec = PlotSpec {
                source: PlotSource::axis_lists(x, y, vec![PlotTraceAppearance::default(); 2])
                    .unwrap(),
                ..Default::default()
            };
            let before = spec.clone();
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                draw_print(ui, &spec, &Err("Retained previous result".into()));
            });
            output.textures_delta.clear();
            fn texts(shape: &egui::Shape, out: &mut Vec<String>) {
                match shape {
                    egui::Shape::Text(text) => out.push(text.galley.text().into()),
                    egui::Shape::Vec(shapes) => {
                        for shape in shapes {
                            texts(shape, out);
                        }
                    }
                    _ => (),
                }
            }
            let mut labels = vec![];
            for shape in &output.shapes {
                texts(&shape.shape, &mut labels);
            }
            let expected = if shared_x {
                ["shared_x", "y1", "y2"]
            } else {
                ["x1", "x2", "shared_y"]
            };
            for label in expected {
                assert_eq!(labels.iter().filter(|actual| *actual == label).count(), 1);
            }
            assert_eq!(spec, before);
            output.drop_without_applying_deltas();
        }
    }

    #[test]
    fn printed_plot_uses_saved_points_on_first_frame_and_keeps_axis_expressions() {
        let context = egui::Context::default();
        let spec = PlotSpec {
            width: 320.,
            height: 240.,
            source: PlotSource::single("saved_x_axis", "invalid_but_saved_y()"),
            native_geometry: None,
            x_axis: mathcraft_document::canvas::PlotAxisSpec {
                label: "time (s)".into(),
                range: None,
                ..Default::default()
            },
            y_axis: mathcraft_document::canvas::PlotAxisSpec {
                label: "voltage (V)".into(),
                range: None,
                ..Default::default()
            },
        };
        let points = vec![[100., 200.], [500., 700.], [900., 300.]];
        let data = Ok(PlotData {
            traces: vec![TraceOutcome::Ready(PlotTraceData {
                presentation: None,
                points: points.into(),
                implicit_domain: false,
                label: "measured".into(),
                style: PlotTraceStyle {
                    color: [220, 20, 30, 255],
                    ..Default::default()
                },
            })],
        });
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800., 600.),
                )),
                ..Default::default()
            },
            |ui| draw_print(ui, &spec, &data),
        );
        fn texts(shape: &egui::Shape, out: &mut Vec<String>) {
            match shape {
                egui::Shape::Text(text) => out.push(text.galley.text().into()),
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        texts(shape, out)
                    }
                }
                _ => {}
            }
        }
        let mut labels = Vec::new();
        for shape in &output.shapes {
            texts(&shape.shape, &mut labels);
        }
        for expected in [
            "saved_x_axis",
            "invalid_but_saved_y()",
            "time (s)",
            "voltage (V)",
            "measured",
        ] {
            assert!(
                labels.iter().any(|label| label == expected),
                "missing printed content: {expected}"
            );
        }
        let primitives =
            context.tessellate(std::mem::take(&mut output.shapes), output.pixels_per_point);
        output.textures_delta.clear();
        let visible_red = primitives
            .iter()
            .map(|primitive| match &primitive.primitive {
                egui::epaint::Primitive::Mesh(mesh) => mesh
                    .vertices
                    .iter()
                    .filter(|v| {
                        v.color.r() > 180
                            && v.color.g() < 50
                            && v.color.b() < 60
                            && primitive.clip_rect.contains(v.pos)
                    })
                    .count(),
                _ => 0,
            })
            .sum::<usize>();
        assert!(
            visible_red > 4,
            "saved trace must be visible without a previous application frame"
        );
    }

    fn expr(source: &str) -> PortableTree {
        expression_tree(&mathcraft_core::parse(source).unwrap(), None).unwrap()
    }
    fn definition(id: u64, name: &str, rhs: PortableTree) -> (u64, f64, f64, String) {
        (
            id,
            0.,
            id as f64 * 10.,
            editor_bridge::tree_to_mathml(&tree(
                NodeKind::Binary(BinaryOp::Define),
                vec![tree(NodeKind::Identifier(name.into()), vec![]), rhs],
            ))
            .unwrap(),
        )
    }
    fn evaluate_plot(
        spec: PlotSpec,
        prior: &[(u64, f64, f64, String)],
    ) -> Result<PlotData, String> {
        evaluate(&spec, prior, 999, 0., 100.)
    }

    fn saved_plot_fixture() -> PlotData {
        // Synthetic presentation deliberately has no valid axis source. Its
        // restoration exercises no query(), parse() or calculation path.
        let traces = vec![
            PlotTraceData {
                presentation: None,
                points: vec![[1., 7.], [2., 3.]].into(),
                implicit_domain: false,
                label: "measured voltage".into(),
                style: PlotTraceStyle {
                    line: PlotLineStyle::Dashed,
                    color: [180, 30, 60, 200],
                    marker: PlotMarker::Circle,
                    marker_filled: true,
                    marker_frequency: 2,
                    ..Default::default()
                },
            },
            PlotTraceData {
                presentation: None,
                points: vec![[-2., 0.25]].into(),
                implicit_domain: true,
                label: "saved fit".into(),
                style: PlotTraceStyle::default(),
            },
        ];
        PlotData {
            traces: traces.into_iter().map(TraceOutcome::Ready).collect(),
        }
    }

    #[test]
    fn saved_plot_roundtrips_points_styles_and_legacy_aliases_without_evaluation() {
        let original = saved_plot_fixture();
        let restored = PlotData::from_saved(&original.to_saved()).unwrap();
        assert_eq!(restored, original);
        assert_eq!(
            restored.first_curve().unwrap().points,
            ready(&restored.traces[0]).points
        );
        assert!(!ready(&restored.traces[0]).implicit_domain);
        assert!(restored.has_implicit_domain());
    }

    #[test]
    fn saved_plot_rejects_malformed_versions_points_and_unsafe_styles() {
        let valid = saved_plot_fixture().to_saved();
        for invalid in ["", "{}", "[]", "null", "{\"version\":1,\"traces\":[]}"] {
            assert!(PlotData::from_saved(invalid).is_none());
        }
        let original: serde_json::Value = serde_json::from_str(&valid).unwrap();
        let mut malformed = vec![];
        let mut item = original.clone();
        item["version"] = 2.into();
        malformed.push(item);
        let mut item = original.clone();
        item["traces"][0]["points"][0] = serde_json::json!([1., 2., 3.]);
        malformed.push(item);
        let mut item = original.clone();
        item["traces"][0]["points"][0][0] = serde_json::Value::Null;
        malformed.push(item);
        let mut item = original.clone();
        item["traces"][0]["points"][0][0] = "NaN".into();
        malformed.push(item);
        let mut item = original.clone();
        item["traces"][0]["implicit_domain"] = 1.into();
        malformed.push(item);
        let mut item = original.clone();
        item["traces"][0]["label"] = "x".repeat(257).into();
        malformed.push(item);
        let mut item = original.clone();
        item["traces"][0]["style"]["marker_frequency"] = 0.into();
        malformed.push(item);
        let mut item = original.clone();
        item["traces"][0]["style"]["line_width"] = 100.into();
        malformed.push(item);
        let mut item = original.clone();
        item["traces"][0]["style"]["marker_size"] = 0.into();
        malformed.push(item);
        let mut item = original.clone();
        item["traces"][0]["style"]["line"] = "foreign".into();
        malformed.push(item);
        let mut item = original.clone();
        item["unexpected"] = true.into();
        malformed.push(item);
        let mut item = original;
        item["traces"][0]["unexpected"] = true.into();
        malformed.push(item);
        for item in malformed {
            assert!(PlotData::from_saved(&item.to_string()).is_none(), "{item}");
        }
        assert!(PlotData::from_saved(&valid.replace("7.0", "1e999")).is_none());
    }

    #[test]
    fn saved_plot_limits_payload_trace_count_points_and_nonfinite_coordinates() {
        assert!(PlotData::from_saved(&" ".repeat(MAX_SAVED_PLOT_BYTES + 1)).is_none());
        let mut data = saved_plot_fixture();
        data.traces = vec![data.traces[0].clone(); PlotSpec::MAX_TRACES + 1];
        assert!(data.to_saved().is_empty());
        let mut data = saved_plot_fixture();
        ready_mut(&mut data.traces[0]).points = vec![[1., 2.]; MAX_SAMPLES + 1].into();
        assert!(data.to_saved().is_empty());
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut data = saved_plot_fixture();
            let mut changed = ready(&data.traces[0]).points.to_vec();
            changed[0][0] = value;
            ready_mut(&mut data.traces[0]).points = changed.into();
            assert!(data.to_saved().is_empty());
        }
        // Validate limits when reading too, even for otherwise well-formed JSON.
        let mut json: serde_json::Value =
            serde_json::from_str(&saved_plot_fixture().to_saved()).unwrap();
        json["traces"][0]["points"] = serde_json::json!(vec![[1., 2.]; MAX_SAMPLES + 1]);
        assert!(PlotData::from_saved(&json.to_string()).is_none());
        let trace = json["traces"][1].clone();
        json["traces"] = serde_json::Value::Array(vec![trace; PlotSpec::MAX_TRACES + 1]);
        assert!(PlotData::from_saved(&json.to_string()).is_none());
    }

    #[test]
    fn implicit_domain_and_whole_input_parser() {
        let data = evaluate_plot(PlotSpec::default(), &[]).unwrap();
        assert_eq!(data.first_curve().unwrap().points.len(), 500);
        assert!(data.has_implicit_domain());
        assert!((data.first_curve().unwrap().points[0][1] - (-10_f64).sin()).abs() < 1e-12);
        assert_eq!(data.first_curve().unwrap().points.first().unwrap()[0], -10.);
        assert_eq!(data.first_curve().unwrap().points.last().unwrap()[0], 10.);
        for invalid in ["sin(x) junk", "x:=2", "x;2", "[1,2]"] {
            assert!(
                evaluate_plot(
                    PlotSpec {
                        source: PlotSource::single("x", invalid),
                        ..Default::default()
                    },
                    &[]
                )
                .unwrap()
                .has_failures()
            );
        }
    }

    #[test]
    fn cached_plot_context_includes_later_globals_and_never_reexecutes_prerequisites() {
        let prior = vec![
            definition(1, "a", expr("2")),
            (
                2,
                0.,
                200.,
                "<ml:globalDefine><ml:id>g</ml:id><ml:real>5</ml:real></ml:globalDefine>".into(),
            ),
        ];
        let xml = editor_bridge::worksheet_xml(&prior).unwrap();
        let mut calculator = mathcraft_compat::WorksheetCalculator::default();
        calculator.update(&xml, None).unwrap();
        assert!(calculator.context_before(0., 100.).is_err());
        let calculated = calculator
            .calculate(mathcraft_compat::CalculationRequest::ThroughRegion(
                "1".into(),
            ))
            .unwrap();
        assert_eq!(calculated.executed_region_ids, ["2", "1"]);
        let context = calculator.context_before(0., 100.).unwrap();
        let data = evaluate_in_context(
            &PlotSpec {
                source: PlotSource::single("x", "a*x+g"),
                ..Default::default()
            },
            &context,
        )
        .unwrap();
        assert_eq!(data.first_curve().unwrap().points.len(), 500);
        assert_eq!(
            data.first_curve().unwrap().points.first(),
            Some(&[-10., -15.])
        );
        assert_eq!(data.first_curve().unwrap().points.last(), Some(&[10., 25.]));
        let unchanged = calculator
            .calculate(mathcraft_compat::CalculationRequest::ThroughRegion(
                "1".into(),
            ))
            .unwrap();
        assert!(unchanged.executed_region_ids.is_empty());
        assert_eq!(unchanged.pending_region_ids, ["2"]);
        let standalone = evaluate(
            &PlotSpec {
                source: PlotSource::single("x", "a*x+g"),
                ..Default::default()
            },
            &prior,
            999,
            0.,
            100.,
        )
        .unwrap();
        assert_eq!(
            standalone.first_curve().unwrap().points,
            data.first_curve().unwrap().points
        );
    }

    #[test]
    fn prior_vector_and_function_are_evaluated_pointwise_without_rebinding() {
        let vector = tree(
            NodeKind::Matrix {
                rows: 3,
                cols: 1,
                display: None,
            },
            vec![expr("-2"), expr("0"), expr("3")],
        );
        let function = tree(
            NodeKind::Function { parameter_count: 1 },
            vec![
                tree(NodeKind::Identifier("f".into()), vec![]),
                tree(NodeKind::Identifier("t".into()), vec![]),
            ],
        );
        let f = (
            2,
            0.,
            20.,
            editor_bridge::tree_to_mathml(&tree(
                NodeKind::Binary(BinaryOp::Define),
                vec![function, expr("t^2+1")],
            ))
            .unwrap(),
        );
        let prior = vec![definition(1, "x", vector), f];
        let before = prior.clone();
        let data = evaluate_plot(
            PlotSpec {
                source: PlotSource::single("x", "f(x)"),
                ..Default::default()
            },
            &prior,
        )
        .unwrap();
        assert_eq!(
            data.first_curve().unwrap().points,
            vec![[-2., 5.], [0., 1.], [3., 10.]]
        );
        assert!(!data.has_implicit_domain());
        assert_eq!(prior, before);
    }

    #[test]
    fn explicit_range_and_independent_vectors() {
        let range = tree(
            NodeKind::Range { has_step: true },
            vec![expr("0"), expr("0.5"), expr("1")],
        );
        let prior = vec![definition(1, "t", range)];
        let data = evaluate_plot(
            PlotSpec {
                source: PlotSource::single("t", "t^2"),
                ..Default::default()
            },
            &prior,
        )
        .unwrap();
        assert_eq!(
            data.first_curve().unwrap().points,
            vec![[0., 0.], [0.5, 0.25], [1., 1.]]
        );
        let v = |a, b| {
            tree(
                NodeKind::Matrix {
                    rows: 2,
                    cols: 1,
                    display: None,
                },
                vec![expr(a), expr(b)],
            )
        };
        let data = evaluate_plot(
            PlotSpec {
                source: PlotSource::single("a", "b"),
                ..Default::default()
            },
            &[
                definition(1, "a", v("1", "2")),
                definition(2, "b", v("3", "5")),
            ],
        )
        .unwrap();
        assert_eq!(data.first_curve().unwrap().points, vec![[1., 3.], [2., 5.]]);
    }

    #[test]
    fn rejects_failed_bindings_complex_and_oversized_ranges() {
        let bad = definition(1, "x", expr("missing"));
        assert!(
            evaluate_plot(PlotSpec::default(), &[bad])
                .unwrap()
                .has_failures()
        );
        for y in ["sqrt(-1)", "1/(x-x)"] {
            assert!(
                evaluate_plot(
                    PlotSpec {
                        source: PlotSource::single("x", y),
                        ..Default::default()
                    },
                    &[]
                )
                .unwrap()
                .has_failures(),
                "{y}"
            );
        }
        let range = tree(
            NodeKind::Range { has_step: false },
            vec![expr("1"), expr("2001")],
        );
        assert!(
            evaluate_plot(PlotSpec::default(), &[definition(1, "x", range)])
                .unwrap()
                .traces[0]
                .diagnostic()
                .unwrap()
                .contains("2000")
        );
    }

    #[test]
    fn data_axes_take_first_matrix_column_and_truncate_unequal_vectors() {
        let matrix = tree(
            NodeKind::Matrix {
                rows: 3,
                cols: 2,
                display: None,
            },
            vec![
                expr("1"),
                expr("90"),
                expr("2"),
                expr("91"),
                expr("3"),
                expr("92"),
            ],
        );
        let y = tree(
            NodeKind::Matrix {
                rows: 2,
                cols: 1,
                display: None,
            },
            vec![expr("10"), expr("20")],
        );
        let data = evaluate_plot(
            PlotSpec {
                source: PlotSource::single("a", "b"),
                ..Default::default()
            },
            &[definition(1, "a", matrix), definition(2, "b", y)],
        )
        .unwrap();
        assert_eq!(
            data.first_curve().unwrap().points,
            vec![[1., 10.], [2., 20.]]
        );
    }

    #[test]
    fn multiple_traces_evaluate_independently_with_labels_and_styles() {
        let mut spec = PlotSpec::default();
        spec.promote_legacy_trace();
        spec.legacy_traces_mut().unwrap()[0].label = "sine".into();
        spec.legacy_traces_mut().unwrap()[0].style.line = PlotLineStyle::Dashed;
        spec.legacy_traces_mut().unwrap().push(PlotTraceSpec {
            y_expression: "cos(x)".into(),
            label: "cosine".into(),
            style: PlotTraceStyle {
                color: [190, 40, 50, 255],
                marker: PlotMarker::Circle,
                marker_frequency: 10,
                ..Default::default()
            },
            ..Default::default()
        });
        spec.x_axis.label = "time".into();
        spec.x_axis.range = Some(mathcraft_document::canvas::PlotRange {
            min: -12.,
            max: 12.,
        });
        spec.y_axis.label = "amplitude".into();
        let data = evaluate_plot(spec.clone(), &[]).unwrap();
        assert_eq!(data.traces.len(), 2);
        assert_eq!(ready(&data.traces[0]).points.len(), 500);
        assert_eq!(ready(&data.traces[1]).points.len(), 500);
        assert_eq!(ready(&data.traces[0]).label, "sine");
        assert_eq!(ready(&data.traces[1]).style.color, [190, 40, 50, 255]);
        assert_eq!(
            data.first_curve().unwrap().points,
            ready(&data.traces[0]).points
        );
        assert!(data.has_implicit_domain());
        assert!((ready(&data.traces[1]).points[0][1] - (-10_f64).cos()).abs() < 1e-12);

        let context = egui::Context::default();
        crate::configure(&context);
        let result = Ok(data);
        let output = context.run_ui(egui::RawInput::default(), |ui| {
            let response = draw(
                ui,
                &mut spec,
                &mut InlinePlotEdit::default(),
                &result,
                false,
            );
            assert!(response.response.rect.is_finite());
        });
        output.drop_without_applying_deltas();
        assert_eq!(spec.x_axis.label, "time");
        assert_eq!(spec.trace_count(), 2);
    }

    #[test]
    fn resizing_reports_one_undo_command_for_the_whole_drag() {
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let mut spec = PlotSpec::default();
        let trace = PlotTraceData {
            presentation: None,
            points: vec![[0., 0.], [1., 1.]].into(),
            implicit_domain: false,
            label: String::new(),
            style: PlotTraceStyle::default(),
        };
        let data = Ok(PlotData {
            traces: vec![TraceOutcome::Ready(trace)],
        });
        let render = |ctx: &egui::Context, spec: &mut PlotSpec, events| {
            let mut response = None;
            let output = ctx.run_ui(
                egui::RawInput {
                    events,
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1000., 800.),
                    )),
                    ..Default::default()
                },
                |ui| {
                    response =
                        Some(draw(ui, spec, &mut InlinePlotEdit::default(), &data, false).response);
                },
            );
            output.drop_without_applying_deltas();
            response.unwrap()
        };
        render(&ctx, &mut spec, vec![]);
        let initial = render(&ctx, &mut spec, vec![]);
        let handle = initial.rect.max - egui::vec2(3., 3.);
        let before = (spec.width, spec.height);
        render(&ctx, &mut spec, vec![egui::Event::PointerMoved(handle)]);
        let start = render(
            &ctx,
            &mut spec,
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
        let first = render(
            &ctx,
            &mut spec,
            vec![egui::Event::PointerMoved(handle + egui::vec2(20., 10.))],
        );
        let second = render(
            &ctx,
            &mut spec,
            vec![egui::Event::PointerMoved(handle + egui::vec2(40., 20.))],
        );
        let end = render(
            &ctx,
            &mut spec,
            vec![egui::Event::PointerButton {
                pos: handle + egui::vec2(40., 20.),
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(start.changed());
        assert!(!first.changed());
        assert!(!second.changed());
        assert!(!end.changed());
        assert_eq!((spec.width, spec.height), (before.0 + 40., before.1 + 20.));
    }
}
