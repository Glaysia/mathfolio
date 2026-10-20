//! Proposed actual egui click/pan tests. No Cargo/runtime/GUI execution by author.
use super::*;
use crate::result_display::DisplayMath;

fn projection(offset: [usize; 2]) -> MatrixProjection {
    let rows = 12;
    let cols = 12;
    MatrixProjection {
        full_rows: 40,
        full_cols: 50,
        row_offset: offset[0],
        col_offset: offset[1],
        rows,
        cols,
        cells: (0..rows)
            .flat_map(|r| {
                (0..cols).map(move |c| {
                    DisplayMath::Text(
                        format!("r{}c{}", r + offset[0], c + offset[1]),
                        MathStyle::Operator,
                    )
                })
            })
            .collect(),
        hidden_above: offset[0] > 0,
        hidden_below: offset[0] + rows < 40,
        hidden_left: offset[1] > 0,
        hidden_right: offset[1] + cols < 50,
    }
}
struct Fixture {
    ctx: egui::Context,
    editor: Editor,
    p: Option<MatrixProjection>,
    owner: Option<egui::Id>,
    marker: Option<Pos2>,
}
impl Fixture {
    fn new() -> Self {
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let editor = Editor::from_portable(&PortableTree {
            kind: NodeKind::Binary(BinaryOp::Evaluate),
            children: vec![
                PortableTree {
                    kind: NodeKind::Number("1".into()),
                    children: vec![],
                },
                PortableTree {
                    kind: NodeKind::Placeholder,
                    children: vec![],
                },
            ],
        })
        .unwrap();
        let mut f = Self {
            ctx,
            editor,
            p: Some(projection([0, 0])),
            owner: None,
            marker: None,
        };
        f.frame(vec![]);
        f.frame(vec![]);
        f
    }
    fn frame(&mut self, events: Vec<egui::Event>) -> (Option<MatrixViewGesture>, bool, bool) {
        let mut output = None;
        self.ctx
            .run_ui(
                egui::RawInput {
                    events,
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1400., 1000.))),
                    ..Default::default()
                },
                |ui| {
                    let p = self.p.as_ref().map(FormulaResult::MatrixViewport);
                    let response = formula_ui_with_actions(ui, &mut self.editor, p, false);
                    let layout = retained_layout(ui, &self.editor, None, 11. * 96. / 72., p);
                    self.marker =
                        find(&layout, response.response.rect.min + vec2(PADDING, PADDING))
                            .and_then(|g| g.markers.into_iter().flatten().next())
                            .map(|r| r.center());
                    self.owner = Some(response.response.id);
                    output = Some((
                        response.matrix_view_gesture,
                        response.response.changed(),
                        response.evaluation_requested,
                    ));
                },
            )
            .drop_without_applying_deltas();
        output.unwrap()
    }
    fn id(&self) -> egui::Id {
        self.owner.unwrap().with("matrix-result-navigator")
    }
    fn response(&self, suffix: &str) -> egui::Response {
        self.ctx.read_response(self.id().with(suffix)).unwrap()
    }
    fn open(&mut self) {
        let at = self.marker.unwrap();
        self.frame(vec![egui::Event::PointerMoved(at)]);
        let press = self.frame(button(at, true));
        let release = self.frame(button(at, false));
        self.frame(vec![]);
        self.frame(vec![]);
        assert_eq!(press, (None, false, false));
        assert_eq!(release, (None, false, false));
        assert!(self.ctx.data(|d| d.get_temp::<State>(self.id())).is_some());
    }
    fn begin(&mut self) -> Pos2 {
        let at = self.response("window").rect.center();
        self.frame(vec![egui::Event::PointerMoved(at)]);
        let begin = self.frame(button(at, true));
        assert_eq!(
            begin,
            (
                Some(MatrixViewGesture::Begin {
                    size: [12, 12],
                    offset: [0, 0]
                }),
                false,
                false
            )
        );
        at
    }
}
fn button(pos: Pos2, pressed: bool) -> Vec<egui::Event> {
    vec![
        egui::Event::PointerMoved(pos),
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        },
    ]
}

#[test]
fn actual_marker_click_and_dark_window_pan_reach_unseen_cells_without_source_edits() {
    let mut f = Fixture::new();
    let source = f.editor.to_portable().unwrap();
    let caret = f.editor.caret();
    let revision = f.editor.arena().revision();
    f.open();
    let start = f.begin();
    let full = f.response("full").rect;
    let at = start + vec2(full.width() / 2., full.height() / 2.);
    let preview = f.frame(vec![egui::Event::PointerMoved(at)]);
    assert_eq!(
        preview,
        (
            Some(MatrixViewGesture::Preview {
                size: [12, 12],
                offset: [20, 25]
            }),
            false,
            false
        )
    );
    // Controller preflight simulation, before the next paint; no direct input API.
    f.p = Some(projection([20, 25]));
    let commit = f.frame(button(at, false));
    assert_eq!(
        commit,
        (
            Some(MatrixViewGesture::Commit {
                size: [12, 12],
                offset: [20, 25]
            }),
            false,
            false
        )
    );
    f.frame(vec![]);
    assert_eq!(f.p.as_ref().unwrap().cells[0].plain(), "r20c25");
    assert_eq!(f.editor.to_portable().unwrap(), source);
    assert_eq!(f.editor.caret(), caret);
    assert_eq!(f.editor.arena().revision(), revision);
    assert!(!f.editor.undo());
}

#[test]
fn navigator_pointer_click_without_motion_returns_same_window_for_noop_history() {
    let mut f = Fixture::new();
    f.open();
    let at = f.begin();
    assert_eq!(
        f.frame(button(at, false)),
        (
            Some(MatrixViewGesture::Commit {
                size: [12, 12],
                offset: [0, 0]
            }),
            false,
            false
        )
    );
}

#[test]
fn navigator_escape_and_pointer_loss_cancel_instead_of_commit() {
    for escape in [true, false] {
        let mut f = Fixture::new();
        f.open();
        let at = f.begin();
        let event = if escape {
            egui::Event::Key {
                key: Key::Escape,
                physical_key: Some(Key::Escape),
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }
        } else {
            egui::Event::PointerGone
        };
        assert_eq!(f.frame(vec![event]).0, Some(MatrixViewGesture::Cancel));
        assert!(f.ctx.data(|d| d.get_temp::<State>(f.id())).is_none());
        assert_eq!(f.frame(button(at, false)).0, None);
    }
}

#[test]
fn stale_target_or_scale_closes_popup_and_cancels_an_active_window() {
    for mode in 0..3 {
        let mut f = Fixture::new();
        f.open();
        f.begin();
        match mode {
            0 => f.p = None,
            1 => f.p = Some(projection([1, 1])),
            _ => f.ctx.set_pixels_per_point(2.),
        }
        assert_eq!(f.frame(vec![]).0, Some(MatrixViewGesture::Cancel));
        assert!(f.ctx.data(|d| d.get_temp::<State>(f.id())).is_none());
    }
}

#[test]
fn rejected_preview_cannot_leave_a_phantom_navigator_transaction() {
    let mut f = Fixture::new();
    f.open();
    let start = f.begin();
    let full = f.response("full").rect;
    let at = start + vec2(full.width() / 2., full.height() / 2.);
    assert!(matches!(
        f.frame(vec![egui::Event::PointerMoved(at)]).0,
        Some(MatrixViewGesture::Preview { .. })
    ));
    // A controller rejected preflight: current projection was not changed.
    assert_eq!(f.frame(vec![]).0, Some(MatrixViewGesture::Cancel));
    assert!(f.ctx.data(|d| d.get_temp::<State>(f.id())).is_none());
    assert_eq!(f.frame(button(at, false)).0, None);
}

#[test]
fn popup_outside_click_retires_without_creating_a_view_command() {
    let mut f = Fixture::new();
    f.open();
    let at = pos2(1300., 950.);
    assert_eq!(f.frame(button(at, true)).0, None);
    assert_eq!(f.frame(button(at, false)).0, None);
    assert!(f.ctx.data(|d| d.get_temp::<State>(f.id())).is_none());
}

#[test]
fn panning_math_is_clamped_to_complete_shape_and_preserves_size() {
    let g = Geometry {
        bracket: Rect::from_min_size(Pos2::ZERO, vec2(100., 100.)),
        step: vec2(10., 10.),
        shape: [40, 50],
        size: [12, 12],
        offset: [0, 0],
        markers: [None; 4],
    };
    let light = Rect::from_min_size(Pos2::ZERO, vec2(240., 160.));
    let p = Pan {
        start: Pos2::ZERO,
        offset: [0, 0],
        last: [0, 0],
        light,
    };
    assert_eq!(requested(&p, g, pos2(10000., 10000.)), [28, 38]);
    assert_eq!(requested(&p, g, pos2(-10000., -10000.)), [0, 0]);
    let dark = window(light, g, [28, 38]);
    assert!(light.contains_rect(dark));
    assert!((dark.right() - light.right()).abs() < 0.001);
    assert!((dark.bottom() - light.bottom()).abs() < 0.001);
}

#[test]
fn retained_marker_hits_equal_painted_glyph_bounds_and_follow_translation() {
    let f = Fixture::new();
    let p = projection([1, 1]);
    let mut captured = None;
    f.ctx
        .run_ui(egui::RawInput::default(), |ui| {
            let measured = measure(ui.painter(), f.editor.root(), &p, 16.);
            let g = *measured.matrix_viewport.as_deref().unwrap();
            let painted: Vec<_> = measured
                .primitives
                .iter()
                .filter_map(|p| match p {
                    Primitive::Text(at, galley) if matches!(galley.text(), "…" | "⋮") => {
                        Some(Rect::from_min_size(at.to_pos2(), galley.size()))
                    }
                    _ => None,
                })
                .collect();
            captured = Some((g, painted));
        })
        .drop_without_applying_deltas();
    let (g, painted) = captured.unwrap();
    assert_eq!(painted.len(), 4);
    for marker in g.markers.into_iter().flatten() {
        assert!(painted.contains(&marker));
    }
    let moved = g.translated(vec2(11., 29.));
    for (old, new) in g.markers.into_iter().zip(moved.markers) {
        assert_eq!(new, old.map(|r| r.translate(vec2(11., 29.))));
    }
}

#[path = "resize_tests.rs"]
mod resize_tests;
