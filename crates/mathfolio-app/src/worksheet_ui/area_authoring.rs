//! Blank Area input is a container insertion point, not a selected-math wrapper.
//! Creation installs a real placeholder child on the first input command; its
//! ordinary formula editor then owns draft input until Enter commits the source.
//! See analysis/0138. Owner history currently groups one platform Text event;
//! original first-character/rest-of-event command splitting is not reproduced.
use mathfolio_document::area_authoring::{self as source, AddedAreaMath};

pub(super) const DEFAULT_INSERTION: [f64; 2] = [19.2, 19.2];

pub(super) struct AreaAuthoring {
    insertion: [f64; 2],
    focus_requested: bool,
    grid: f64,
}

impl Default for AreaAuthoring {
    fn default() -> Self {
        Self {
            insertion: DEFAULT_INSERTION,
            focus_requested: false,
            grid: mathfolio_document::canvas::CanvasDocument::default().grid,
        }
    }
}

impl AreaAuthoring {
    pub(super) fn insertion_point(&self) -> [f64; 2] {
        self.insertion
    }

    /// Area0c5 snaps, then keeps an exactly top/bottom insertion off the border.
    /// Uses the existing Canvas snap policy; exact original midpoint tolerance
    /// remains outside the evidence established for this controller.
    pub(super) fn set_insertion_point(
        &mut self,
        local: [f64; 2],
        grid: f64,
        height: f64,
    ) -> Result<(), String> {
        if !height.is_finite() || height <= 0. {
            return Err("Area height must be finite and positive".into());
        }
        let mut point = [
            mathfolio_document::canvas::snap_coordinate(local[0], grid)?,
            mathfolio_document::canvas::snap_coordinate(local[1], grid)?,
        ];
        if point[1] == 0. {
            point[1] += grid;
        }
        if point[1] == height {
            point[1] -= grid;
        }
        self.insertion = point;
        self.focus_requested = true;
        Ok(())
    }

    pub(super) fn request_focus(&mut self) {
        self.focus_requested = true;
    }

    pub(super) fn set_grid(&mut self, grid: f64) -> Result<(), String> {
        mathfolio_document::canvas::snap_coordinate(0., grid)?;
        self.grid = grid;
        Ok(())
    }

    pub(super) fn click(&mut self, local: [f64; 2], height: f64) -> Result<(), String> {
        self.set_insertion_point(local, self.grid, height)
    }

    pub(super) fn advance_after_math(
        &mut self,
        left: f64,
        bottom: f64,
        height: f64,
    ) -> Result<(), String> {
        self.set_insertion_point([left, bottom + self.grid / 2.], self.grid, height)
    }

    pub(super) fn take_focus_request(&mut self) -> bool {
        std::mem::take(&mut self.focus_requested)
    }

    /// Candidate only: root's document history must group child installation and
    /// the first actual character as one owner transaction. No editor/history is
    /// cloned and no value is evaluated or saved result is synthesized here.
    pub(super) fn prepare_math(&self, xml: &str) -> Result<AddedAreaMath, String> {
        source::append_math_xml(
            xml,
            self.insertion_point(),
            "<ml:placeholder xmlns:ml='http://schemas.mathsoft.com/math50'/>",
        )
    }
}

/// Read once when owning XML changes: explicit height, measured actualHeight,
/// then retained expanded AreaHeight. The three original fields are distinct.
pub(super) fn expanded_height(xml: &str) -> Result<f32, String> {
    let height = source::area_display_height_xml(xml)?;
    if !height.is_finite() || height < 0. || height > f32::MAX as f64 {
        return Err("Invalid Area height".into());
    }
    Ok((height as f32).max(30.))
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui;
    use mathfolio_document::area;

    fn ui_frame(
        context: &egui::Context,
        editor: &mut super::super::area_ui::AreaEditor,
        xml: &mut String,
        events: Vec<egui::Event>,
    ) -> super::super::area_ui::AreaResponse {
        let mut response = None;
        context
            .run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(800., 600.),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    response = Some(editor.show(ui, xml, &Default::default(), 11.));
                },
            )
            .drop_without_applying_deltas();
        response.unwrap()
    }

    #[test]
    fn default_input_creates_a_real_placeholder_child_candidate_without_mutating_source() {
        let xml = source::empty_area_xml(96.).unwrap();
        let control = AreaAuthoring::default();
        assert_eq!(control.insertion_point(), [19.2, 19.2]);
        let candidate = control.prepare_math(&xml).unwrap();
        assert!(area::area_math_children_xml(&xml, &[]).unwrap().is_empty());
        let child = area::area_math_children_xml(&candidate.xml, &[])
            .unwrap()
            .remove(0);
        assert_eq!([child.left, child.top], [19.2, 19.2]);
        assert!(child.mathml.contains("placeholder"));
        assert_eq!(child.result_ref, None);
        assert_eq!(expanded_height(&xml).unwrap(), 96.);
        assert_eq!(area::areas_from_xml(&xml).unwrap()[0].area_height, 0.);
    }

    #[test]
    fn border_snapping_is_directional_and_invalid_input_preserves_anchor_and_focus() {
        let mut control = AreaAuthoring::default();
        control.set_insertion_point([21., 0.], 9.6, 96.).unwrap();
        assert_eq!(control.insertion_point(), [19.2, 9.6]);
        assert!(control.take_focus_request());
        assert!(!control.take_focus_request());
        control.set_insertion_point([21., 96.], 9.6, 96.).unwrap();
        assert_eq!(control.insertion_point(), [19.2, 86.4]);
        let before = control.insertion_point();
        assert!(
            control
                .set_insertion_point([f64::NAN, 0.], 9.6, 96.)
                .is_err()
        );
        assert_eq!(control.insertion_point(), before);
        assert!(control.take_focus_request());
        assert!(control.set_insertion_point([0., 0.], 0., 96.).is_err());
        assert_eq!(control.insertion_point(), before);
        assert!(!control.take_focus_request());
        control.request_focus();
        assert!(control.take_focus_request());
    }

    #[test]
    fn current_owner_height_is_distinct_from_retained_collapse_height() {
        let xml = "<region xmlns='http://schemas.mathsoft.com/worksheet50' actualHeight='75.6'><Area AreaHeight='120'><regions/></Area></region>";
        assert_eq!(expanded_height(xml).unwrap(), 75.6);
        let fallback = xml.replace(" actualHeight='75.6'", "");
        assert_eq!(expanded_height(&fallback).unwrap(), 120.);
        assert!(expanded_height(&xml.replace("75.6", "NaN")).is_err());
    }

    #[test]
    fn actual_first_text_creates_source_placeholder_and_draft_then_enter_commits_only_child() {
        use super::super::area_ui::AreaEditor;
        use mathfolio_editor::NodeKind;
        let context = egui::Context::default();
        crate::configure(&context);
        let mut xml = source::empty_area_xml(96.).unwrap();
        let mut editor = AreaEditor::new(&xml).unwrap();
        editor.set_input_grid(9.6).unwrap();
        editor.activate_insertion().unwrap();
        let mut first = None;
        context
            .run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(800., 600.),
                    )),
                    events: vec![egui::Event::Text("12".into())],
                    ..Default::default()
                },
                |ui| {
                    first = Some(editor.show(ui, &mut xml, &Default::default(), 11.));
                },
            )
            .drop_without_applying_deltas();
        let first = first.unwrap();
        assert!(first.document_changed && first.draft_changed && first.focused);
        assert!(first.history_changed);
        assert!(!first.calculation_changed && !first.commit_requested);
        assert_eq!(editor.active_child(), Some("[0,0,0]"));
        assert_eq!(first.child_created.as_deref(), Some("[0,0,0]"));
        let child = area::area_math_children_xml(&xml, &[]).unwrap().remove(0);
        assert!(child.mathml.contains("placeholder"));
        assert_eq!(child.result_ref, None);
        assert_eq!(
            editor.active_editor().unwrap().to_portable().unwrap().kind,
            NodeKind::Number("12".into())
        );
        let mut committed = None;
        context
            .run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(800., 600.),
                    )),
                    events: vec![egui::Event::Key {
                        key: egui::Key::Enter,
                        physical_key: Some(egui::Key::Enter),
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    }],
                    ..Default::default()
                },
                |ui| {
                    committed = Some(editor.show(ui, &mut xml, &Default::default(), 11.));
                },
            )
            .drop_without_applying_deltas();
        let committed = committed.unwrap();
        assert!(
            committed.document_changed
                && committed.calculation_changed
                && committed.commit_requested
        );
        assert!(!committed.history_changed);
        assert_eq!(editor.active_child(), None);
        let child = area::area_math_children_xml(&xml, &[]).unwrap().remove(0);
        assert_eq!(
            mathfolio_compat::editor_bridge::tree_from_mathml(&child.mathml)
                .unwrap()
                .kind,
            NodeKind::Number("12".into())
        );
        assert!(!editor.has_draft_changes());
        let mut next = None;
        context
            .run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(800., 600.),
                    )),
                    events: vec![egui::Event::Text("34".into())],
                    ..Default::default()
                },
                |ui| {
                    next = Some(editor.show(ui, &mut xml, &Default::default(), 11.));
                },
            )
            .drop_without_applying_deltas();
        assert_eq!(next.unwrap().child_created.as_deref(), Some("[0,0,1]"));
        let children = area::area_math_children_xml(&xml, &[]).unwrap();
        assert_eq!(children.len(), 2);
        assert_eq!(children[1].left, children[0].left);
        assert!(children[1].top > children[0].top);
        assert_eq!(
            editor.active_editor().unwrap().to_portable().unwrap().kind,
            NodeKind::Number("34".into())
        );
    }

    #[test]
    fn blank_local_pointer_starts_second_child_at_snapped_anchor_without_copying_first() {
        use super::super::area_ui::AreaEditor;
        let context = egui::Context::default();
        crate::configure(&context);
        let empty = source::empty_area_xml(96.).unwrap();
        let first = source::append_math_xml(
            &empty,
            [19.2, 19.2],
            "<ml:real xmlns:ml='http://schemas.mathsoft.com/math50'>12</ml:real>",
        )
        .unwrap();
        let mut xml = first.xml;
        let mut editor = AreaEditor::new(&xml).unwrap();
        editor.set_input_grid(9.6).unwrap();
        editor.activate("[0,0,0]").unwrap();
        let mut frame = |events: Vec<egui::Event>| {
            let mut response = None;
            context
                .run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(800., 600.),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        response = Some(editor.show(ui, &mut xml, &Default::default(), 11.));
                    },
                )
                .drop_without_applying_deltas();
            response.unwrap()
        };
        let warm = frame(vec![]);
        let point = warm.response.rect.min + egui::vec2(70., 70.);
        frame(vec![
            egui::Event::PointerMoved(point),
            egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        let clicked = frame(vec![egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }]);
        assert!(clicked.focused && clicked.child_created.is_none());
        assert!(!clicked.document_changed);
        let typed = frame(vec![egui::Event::Text("34".into())]);
        assert_eq!(typed.child_created.as_deref(), Some("[0,0,1]"));
        assert!(typed.document_changed && typed.draft_changed && !typed.calculation_changed);
        let children = area::area_math_children_xml(&xml, &[]).unwrap();
        assert_eq!(children.len(), 2);
        assert_eq!(
            mathfolio_compat::editor_bridge::tree_from_mathml(&children[0].mathml)
                .unwrap()
                .kind,
            mathfolio_editor::NodeKind::Number("12".into())
        );
        assert!(children[1].mathml.contains("placeholder"));
        assert!((children[1].left - 67.2).abs() < 1e-10);
        assert!((children[1].top - 67.2).abs() < 1e-10);
        assert_eq!(
            editor.active_editor().unwrap().to_portable().unwrap().kind,
            mathfolio_editor::NodeKind::Number("34".into())
        );
    }

    #[test]
    fn program_consumed_enter_keeps_child_focus_and_does_not_commit_or_advance_area() {
        use super::super::area_ui::AreaEditor;
        use mathfolio_editor::{Direction, NodeKind, PortableTree};
        let context = egui::Context::default();
        crate::configure(&context);
        let program = PortableTree {
            kind: NodeKind::Program { statement_count: 1 },
            children: vec![PortableTree {
                kind: NodeKind::Number("12".into()),
                children: vec![],
            }],
        };
        let empty = source::empty_area_xml(96.).unwrap();
        let mut xml = source::append_math_xml(
            &empty,
            [19.2, 19.2],
            &mathfolio_compat::editor_bridge::tree_to_mathml(&program).unwrap(),
        )
        .unwrap()
        .xml;
        let original = xml.clone();
        let mut editor = AreaEditor::new(&xml).unwrap();
        editor.activate("[0,0,0]").unwrap();
        let child = editor.active_editor_mut().unwrap();
        let line = child.arena().get(child.root()).unwrap().children()[0];
        child.place_caret(line, Direction::Right).unwrap();
        ui_frame(&context, &mut editor, &mut xml, vec![]);
        let outcome = ui_frame(
            &context,
            &mut editor,
            &mut xml,
            vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: Some(egui::Key::Enter),
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(outcome.draft_changed && outcome.history_changed);
        assert!(
            !outcome.document_changed && !outcome.calculation_changed && !outcome.commit_requested
        );
        assert_eq!(xml, original);
        assert_eq!(editor.active_child(), Some("[0,0,0]"));
        let draft = editor.active_editor().unwrap().to_portable().unwrap();
        assert_eq!(draft.kind, NodeKind::Program { statement_count: 2 });
        assert_eq!(draft.children.len(), 2);
        assert_eq!(draft.children[0], program.children[0]);
        assert_eq!(draft.children[1].kind, NodeKind::Placeholder);
    }

    #[test]
    fn blank_ime_preedit_does_not_create_child_but_final_text_commit_does() {
        use super::super::area_ui::AreaEditor;
        let context = egui::Context::default();
        crate::configure(&context);
        let mut xml = source::empty_area_xml(96.).unwrap();
        let original = xml.clone();
        let mut editor = AreaEditor::new(&xml).unwrap();
        editor.activate_insertion().unwrap();
        let preedit = ui_frame(
            &context,
            &mut editor,
            &mut xml,
            vec![egui::Event::Ime(egui::ImeEvent::Preedit {
                text: "α".into(),
                active_range_chars: None,
            })],
        );
        assert!(!preedit.document_changed && !preedit.draft_changed && !preedit.history_changed);
        assert_eq!(xml, original);
        assert_eq!(editor.active_child(), None);
        let committed = ui_frame(
            &context,
            &mut editor,
            &mut xml,
            vec![egui::Event::Ime(egui::ImeEvent::Commit("α".into()))],
        );
        assert!(committed.document_changed && committed.draft_changed && committed.history_changed);
        assert!(!committed.calculation_changed);
        assert_eq!(committed.child_created.as_deref(), Some("[0,0,0]"));
        assert_eq!(
            editor.active_editor().unwrap().to_portable().unwrap().kind,
            mathfolio_editor::NodeKind::Identifier("α".into())
        );
    }
}
