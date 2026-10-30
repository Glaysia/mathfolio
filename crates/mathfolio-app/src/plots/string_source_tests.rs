//! Original expression-only authoring permits STRING; it remains nonnumeric
//! source and must never be replaced by an invented curve/domain value.
use super::*;
use inline_edit::AxisField;

fn key(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
    }
}

struct Harness {
    ctx: egui::Context,
    spec: PlotSpec,
    original: PlotSpec,
    edit: InlinePlotEdit,
    data: Result<PlotData, String>,
    field: Option<AxisField>,
    widget: Option<egui::Id>,
}
impl Harness {
    fn new() -> Self {
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let spec = PlotSpec::typed_blank_xy();
        let mut h = Self {
            ctx,
            original: spec.clone(),
            spec,
            edit: InlinePlotEdit::default(),
            data: Ok(PlotData {
                traces: vec![TraceOutcome::Incomplete],
            }),
            field: None,
            widget: None,
        };
        h.frame(true, vec![]);
        h.frame(false, vec![]);
        let id = h.widget.unwrap();
        h.ctx.memory_mut(|m| m.request_focus(id));
        h.frame(false, vec![]);
        h
    }
    fn frame(&mut self, focus: bool, events: Vec<egui::Event>) -> DrawResponse {
        let mut result = None;
        let saved = self.data.as_ref().unwrap().to_saved();
        self.ctx
            .run_ui(
                egui::RawInput {
                    events,
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1400., 1000.),
                    )),
                    ..Default::default()
                },
                |ui| {
                    result = Some(draw(ui, &mut self.spec, &mut self.edit, &self.data, focus));
                    let field = inline_edit::fields(self.edit.draft().unwrap_or(&self.spec))
                        .into_iter()
                        .find(|field| !field.x && field.index == 0)
                        .unwrap();
                    self.widget = self.edit.expression_widget_id(field);
                    self.field = Some(field);
                },
            )
            .drop_without_applying_deltas();
        assert_eq!(self.spec, self.original, "draft must not publish source");
        assert_eq!(
            self.data.as_ref().unwrap().to_saved(),
            saved,
            "draft must not recalculate a curve"
        );
        result.unwrap()
    }
    fn source(&self) -> &PortableTree {
        self.edit
            .draft()
            .unwrap_or(&self.spec)
            .primary_y_expression()
            .as_tree()
            .unwrap()
    }
}

#[test]
fn actual_quote_and_operator_text_stage_a_string_first_math_until_enter() {
    for content in ["", "12 + (한글)=\""] {
        let mut h = Harness::new();
        let input = format!("\"{content}");
        let result = h.frame(false, vec![egui::Event::Text(input)]);
        assert!(result.expression_changed);
        assert!(!result.commit_requested);
        assert!(result.input_error.is_none());
        assert!(h.edit.validation_error().is_none());
        assert_eq!(h.source().kind, NodeKind::StringLiteral(content.into()));
        assert!(h.edit.has_changes());
        assert!(h.edit.draft().unwrap().primary_y_expression().is_complete());
        assert!(
            !h.edit
                .draft()
                .unwrap()
                .primary_y_expression()
                .is_exact_placeholder()
        );
        assert!(
            h.edit.draft().unwrap().axis_lists().unwrap().y[0]
                .scaling
                .is_none()
        );
        let request = h.frame(false, vec![key(egui::Key::Enter, egui::Modifiers::NONE)]);
        assert!(request.commit_requested);
        let committed = h.edit.take().unwrap();
        assert_eq!(
            committed.primary_y_expression().as_tree().unwrap().kind,
            NodeKind::StringLiteral(content.into())
        );
        assert!(
            h.data
                .as_ref()
                .unwrap()
                .traces
                .iter()
                .all(|trace| matches!(trace, TraceOutcome::Incomplete))
        );
    }
}

#[test]
fn typed_string_field_undo_redo_preserve_literal_type_and_source_authority() {
    let mut h = Harness::new();
    h.frame(false, vec![egui::Event::Text("\"a=+(b)\"".into())]);
    let wanted = PortableTree {
        kind: NodeKind::StringLiteral("a=+(b)\"".into()),
        children: vec![],
    };
    assert_eq!(h.source(), &wanted);
    h.frame(false, vec![key(egui::Key::Z, egui::Modifiers::COMMAND)]);
    assert_eq!(h.source().kind, NodeKind::Placeholder);
    h.frame(false, vec![key(egui::Key::Y, egui::Modifiers::COMMAND)]);
    assert_eq!(h.source(), &wanted);
    assert!(h.edit.validation_error().is_none());
    let committed = h.edit.take().unwrap();
    let xml = mathfolio_document::native_plot::payload(&committed).unwrap();
    let owner = format!("<region xmlns=\"http://schemas.mathsoft.com/worksheet50\">{xml}</region>");
    let reopened = mathfolio_document::native_plot::parse(&owner).unwrap();
    assert_eq!(reopened.primary_y_expression().as_tree(), Some(&wanted));
}
