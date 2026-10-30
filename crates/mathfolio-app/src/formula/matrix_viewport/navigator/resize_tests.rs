//! Unexecuted unit and actual egui-event tests. Existing pan tests unchanged.
use super::*;
use resize::{Handle, Resize, View};

fn geometry() -> Geometry {
    Geometry {
        bracket: Rect::from_min_size(Pos2::ZERO, vec2(100., 100.)),
        step: vec2(10., 10.),
        shape: [40, 50],
        size: [12, 12],
        offset: [10, 10],
        markers: [None; 4],
    }
}
#[test]
fn eight_handles_change_only_the_snapshot_edges_and_preserve_opposite_boundaries() {
    let light = Rect::from_min_size(Pos2::ZERO, vec2(500., 400.));
    let expected = [
        (Handle::Left, [12, 10], [10, 12]),
        (Handle::Right, [12, 14], [10, 10]),
        (Handle::Up, [10, 12], [12, 10]),
        (Handle::Down, [14, 12], [10, 10]),
        (Handle::UpLeft, [10, 10], [12, 12]),
        (Handle::UpRight, [10, 14], [12, 10]),
        (Handle::DownLeft, [14, 10], [10, 12]),
        (Handle::DownRight, [14, 14], [10, 10]),
    ];
    for (handle, size, offset) in expected {
        let resize = Resize::new(handle, Pos2::ZERO, light, geometry());
        assert_eq!(
            resize.requested(pos2(20., 20.)),
            View { size, offset },
            "{handle:?}"
        );
    }
}
#[test]
fn original_sum_truncation_is_not_nearest_cell_rounding_and_does_not_accumulate() {
    let mut r = Resize::new(
        Handle::Right,
        Pos2::ZERO,
        Rect::from_min_size(Pos2::ZERO, vec2(500., 400.)),
        geometry(),
    );
    let first = r.requested(pos2(2.5, 0.));
    assert_eq!(
        first,
        View {
            size: [12, 13],
            offset: [10, 10]
        }
    );
    r.last = first;
    assert_eq!(
        r.requested(pos2(20., 0.)),
        View {
            size: [12, 14],
            offset: [10, 10]
        }
    );
    assert_eq!(
        r.requested(Pos2::ZERO),
        View {
            size: [12, 12],
            offset: [10, 10]
        }
    );
}
#[test]
fn corner_crossing_rejects_both_axes_and_bounds_never_create_zero_or_outside_cells() {
    let light = Rect::from_min_size(Pos2::ZERO, vec2(500., 400.));
    let r = Resize::new(Handle::UpLeft, Pos2::ZERO, light, geometry());
    assert_eq!(r.requested(pos2(130., -30.)), r.last);
    assert_eq!(
        r.requested(pos2(-10000., -10000.)),
        View {
            size: [22, 22],
            offset: [0, 0]
        }
    );
    let r = Resize::new(Handle::DownRight, Pos2::ZERO, light, geometry());
    assert_eq!(
        r.requested(pos2(10000., 10000.)),
        View {
            size: [30, 40],
            offset: [10, 10]
        }
    );
    assert_eq!(r.requested(pos2(f32::NAN, 0.)), r.last);
    for handle in Handle::ALL {
        let r = Resize::new(handle, Pos2::ZERO, light, geometry());
        for x in [-10000., -119., -1., 0., 1., 119., 10000.] {
            for y in [-10000., -119., -1., 0., 1., 119., 10000.] {
                let v = r.requested(pos2(x, y));
                assert!(v.size[0] >= 1 && v.size[1] >= 1);
                assert!(v.offset[0] + v.size[0] <= 40 && v.offset[1] + v.size[1] <= 50);
            }
        }
    }
}
#[test]
fn handle_partition_keeps_center_for_pan_and_has_eight_targets() {
    let dark = Rect::from_min_size(pos2(10., 20.), vec2(100., 80.));
    assert_eq!(Handle::at(dark, dark.center()), None);
    for handle in Handle::ALL {
        assert_eq!(Handle::at(dark, handle.rect(dark).center()), Some(handle));
    }
    let tiny = Rect::from_min_size(Pos2::ZERO, vec2(2., 2.));
    for handle in [
        Handle::UpLeft,
        Handle::UpRight,
        Handle::DownLeft,
        Handle::DownRight,
    ] {
        assert_eq!(Handle::at(tiny, handle.rect(tiny).center()), Some(handle));
    }
}

fn projected(view: View) -> MatrixProjection {
    let mut p = super::projection(view.offset);
    p.rows = view.size[0];
    p.cols = view.size[1];
    p.cells = (0..p.rows)
        .flat_map(|row| {
            (0..p.cols).map(move |col| {
                crate::result_display::DisplayMath::Text(
                    format!("r{}c{}", row + view.offset[0], col + view.offset[1]),
                    MathStyle::Operator,
                )
            })
        })
        .collect();
    p.hidden_above = view.offset[0] > 0;
    p.hidden_left = view.offset[1] > 0;
    p.hidden_below = view.offset[0] + p.rows < p.full_rows;
    p.hidden_right = view.offset[1] + p.cols < p.full_cols;
    p
}
#[test]
fn actual_eight_pointer_handles_preview_then_commit_size_and_offset_without_editor_edits() {
    let expected = [
        (Handle::Left, [12, 10], [10, 12]),
        (Handle::Right, [12, 15], [10, 10]),
        (Handle::Up, [10, 12], [12, 10]),
        (Handle::Down, [15, 12], [10, 10]),
        (Handle::UpLeft, [10, 10], [12, 12]),
        (Handle::UpRight, [10, 15], [12, 10]),
        (Handle::DownLeft, [15, 10], [10, 12]),
        (Handle::DownRight, [15, 15], [10, 10]),
    ];
    for (handle, size, offset) in expected {
        let mut f = super::Fixture::new();
        f.p = Some(projected(View {
            size: [12, 12],
            offset: [10, 10],
        }));
        f.frame(vec![]);
        f.frame(vec![]);
        let source = f.editor.to_portable().unwrap();
        let caret = f.editor.caret();
        let revision = f.editor.arena().revision();
        f.open();
        let response = f
            .ctx
            .read_response(f.id().with(("handle", handle as u8)))
            .unwrap();
        let at = response.rect.center();
        f.frame(vec![egui::Event::PointerMoved(at)]);
        assert_eq!(
            f.frame(super::button(at, true)),
            (
                Some(MatrixViewGesture::BeginInnerResize {
                    size: [12, 12],
                    offset: [10, 10]
                }),
                false,
                false
            )
        );
        let light = f.response("full").rect;
        let moved = at + vec2(light.width() * 2.5 / 50., light.height() * 2.5 / 40.);
        let preview = f.frame(vec![egui::Event::PointerMoved(moved)]);
        let (
            Some(MatrixViewGesture::PreviewInnerResize {
                size: actual_size,
                offset: actual_offset,
            }),
            false,
            false,
        ) = preview
        else {
            panic!("{preview:?}")
        };
        assert_eq!(actual_size, size);
        assert_eq!(actual_offset, offset);
        let view = View {
            size: actual_size,
            offset: actual_offset,
        };
        f.p = Some(projected(view));
        f.frame(vec![]);
        assert_eq!(
            f.frame(super::button(moved, false)),
            (
                Some(MatrixViewGesture::CommitInnerResize {
                    size: actual_size,
                    offset: actual_offset
                }),
                false,
                false
            )
        );
        assert_eq!(f.editor.to_portable().unwrap(), source);
        assert_eq!(f.editor.caret(), caret);
        assert_eq!(f.editor.arena().revision(), revision);
        assert!(!f.editor.undo());
    }
}
#[test]
fn resize_preview_rejection_and_escape_retire_without_a_late_commit() {
    for reject in [true, false] {
        let mut f = super::Fixture::new();
        f.open();
        let at = f
            .ctx
            .read_response(f.id().with(("handle", Handle::DownRight as u8)))
            .unwrap()
            .rect
            .center();
        f.frame(vec![egui::Event::PointerMoved(at)]);
        f.frame(super::button(at, true));
        let moved = at + vec2(15., 15.);
        let (Some(MatrixViewGesture::PreviewInnerResize { size, offset }), _, _) =
            f.frame(vec![egui::Event::PointerMoved(moved)])
        else {
            panic!("missing resize preview")
        };
        let cancel = if reject {
            f.frame(vec![])
        } else {
            f.p = Some(projected(View { size, offset }));
            f.frame(vec![egui::Event::Key {
                key: Key::Escape,
                physical_key: Some(Key::Escape),
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }])
        };
        assert_eq!(cancel.0, Some(MatrixViewGesture::Cancel));
        assert!(f.ctx.data(|d| d.get_temp::<State>(f.id())).is_none());
        assert_eq!(f.frame(super::button(moved, false)).0, None);
    }
}

#[test]
fn actual_tiny_window_move_helper_remains_grabbable_outside_the_inner_handle_partition() {
    let mut f = super::Fixture::new();
    f.p = Some(projected(View {
        size: [1, 1],
        offset: [10, 10],
    }));
    f.frame(vec![]);
    f.frame(vec![]);
    f.open();
    let at = f.response("window").rect.center() + vec2(12., 0.);
    f.frame(vec![egui::Event::PointerMoved(at)]);
    assert_eq!(
        f.frame(super::button(at, true)).0,
        Some(MatrixViewGesture::Begin {
            size: [1, 1],
            offset: [10, 10]
        })
    );
    assert!(
        f.ctx
            .data(|d| d.get_temp::<State>(f.id()))
            .unwrap()
            .pan
            .is_some()
    );
    let light = f.response("full").rect;
    let moved = at + vec2(light.width() * 0.1, light.height() * 0.1);
    assert_eq!(
        f.frame(vec![egui::Event::PointerMoved(moved)]).0,
        Some(MatrixViewGesture::Preview {
            size: [1, 1],
            offset: [14, 15]
        })
    );
    f.p = Some(projected(View {
        size: [1, 1],
        offset: [14, 15],
    }));
    assert_eq!(
        f.frame(super::button(moved, false)).0,
        Some(MatrixViewGesture::Commit {
            size: [1, 1],
            offset: [14, 15]
        })
    );
}
