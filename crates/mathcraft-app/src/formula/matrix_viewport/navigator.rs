//! Result-only navigator pan and eight inner resize handles. Original H193/Control1415/1424 prove light full
//! bounds, dark current window and pan command. Fixed preview dimensions/colors,
//! hit tolerance and pan rounding below are independent rendering policies.
//! Resize uses original margin-snapshot/truncation semantics; cancel is our policy.
//! No native window, values, ORIGIN labels, parser, source or calculator here.
use super::*;
mod resize;
use resize::{Handle, Resize, View};

#[derive(Clone)]
struct State {
    geometry: Geometry,
    root: NodeId,
    revision: u64,
    layer: egui::LayerId,
    scale: u32,
    ppp: u32,
    anchor: Rect, // global; PopupAnchor::ParentRect takes global coordinates
    frame: u64,
    expected_view: Option<View>,
    pan: Option<Pan>,
    resize: Option<Resize>,
}
#[derive(Clone)]
struct Pan {
    start: Pos2,
    offset: [usize; 2],
    last: [usize; 2],
    light: Rect,
}
fn scale(ui: &Ui) -> u32 {
    ui.ctx()
        .layer_transform_to_global(ui.layer_id())
        .unwrap_or_default()
        .scaling
        .to_bits()
}
fn window(light: Rect, geometry: Geometry, offset: [usize; 2]) -> Rect {
    let by = vec2(
        light.width() / geometry.shape[1] as f32,
        light.height() / geometry.shape[0] as f32,
    );
    Rect::from_min_size(
        light.min + vec2(offset[1] as f32 * by.x, offset[0] as f32 * by.y),
        vec2(
            geometry.size[1] as f32 * by.x,
            geometry.size[0] as f32 * by.y,
        ),
    )
    .intersect(light)
}
fn requested(pan: &Pan, geometry: Geometry, pointer: Pos2) -> [usize; 2] {
    if !pointer.is_finite() {
        return pan.last;
    }
    let delta = pointer - pan.start;
    let row = (pan.offset[0] as f64
        + delta.y as f64 / pan.light.height() as f64 * geometry.shape[0] as f64)
        .round()
        .clamp(0., (geometry.shape[0] - geometry.size[0]) as f64) as usize;
    let col = (pan.offset[1] as f64
        + delta.x as f64 / pan.light.width() as f64 * geometry.shape[1] as f64)
        .round()
        .clamp(0., (geometry.shape[1] - geometry.size[1]) as f64) as usize;
    [row, col]
}
impl State {
    fn compatible(&self, ui: &Ui, editor: &Editor, now: Option<Geometry>) -> bool {
        let frame = ui.ctx().cumulative_frame_nr();
        self.root == editor.root()
            && self.revision == editor.arena().revision()
            && self.layer == ui.layer_id()
            && self.scale == scale(ui)
            && self.ppp == ui.ctx().pixels_per_point().to_bits()
            && frame.saturating_sub(self.frame) <= 1
            && now.is_some_and(|g| {
                g.shape == self.geometry.shape
                    && View::from_geometry(g)
                        == self
                            .expected_view
                            .unwrap_or(View::from_geometry(self.geometry))
            })
    }
}

pub(super) fn interact(
    ui: &mut Ui,
    owner: egui::Id,
    editor: &Editor,
    geometry: Option<Geometry>,
) -> Interaction {
    let id = owner.with("matrix-result-navigator");
    let mut state = ui.data(|d| d.get_temp::<State>(id));
    let mut output = Interaction::default();
    if let Some(active) = &state {
        let escape = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape));
        let gone = ui.input(|i| {
            i.events
                .iter()
                .any(|e| matches!(e, egui::Event::PointerGone))
        });
        if escape || gone || !active.compatible(ui, editor, geometry) {
            ui.data_mut(|d| d.remove::<State>(id));
            return Interaction {
                owns_pointer: true,
                gesture: Some(MatrixViewGesture::Cancel),
            };
        }
    }
    // Exact glyph bounds, cached with the layout. No broadened whole-result hit.
    if state.is_none()
        && let Some(g) = geometry
    {
        for (ordinal, rect) in g.markers.iter().enumerate() {
            if let Some(rect) = rect {
                let response = ui.interact(*rect, id.with(("marker", ordinal)), Sense::click());
                output.owns_pointer |= response.is_pointer_button_down_on();
                if response.clicked_by(egui::PointerButton::Primary)
                    && ui.input(|i| i.modifiers.is_none())
                {
                    let anchor = ui
                        .ctx()
                        .layer_transform_to_global(ui.layer_id())
                        .unwrap_or_default()
                        * response.rect;
                    state = Some(State {
                        geometry: g,
                        root: editor.root(),
                        revision: editor.arena().revision(),
                        layer: ui.layer_id(),
                        scale: scale(ui),
                        ppp: ui.ctx().pixels_per_point().to_bits(),
                        anchor,
                        frame: ui.ctx().cumulative_frame_nr(),
                        expected_view: None,
                        pan: None,
                        resize: None,
                    });
                    output.owns_pointer = true;
                    break;
                }
            }
        }
    }
    let Some(mut active) = state else {
        return output;
    };
    let Some(g) = geometry else { unreachable!() };
    active.frame = ui.ctx().cumulative_frame_nr();
    // Current effective geometry is authoritative after a successful preflight.
    active.geometry = g;
    active.expected_view = None;
    output.owns_pointer = true;
    let mut open = true;
    let mut abort = false;
    egui::Popup::new(
        id.with("popup"),
        ui.ctx().clone(),
        active.anchor,
        ui.layer_id(),
    )
    .open_bool(&mut open)
    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
    .show(|popup| {
        let (light, _) = popup.allocate_exact_size(vec2(240., 160.), Sense::hover());
        popup.interact(light, id.with("full"), Sense::hover());
        let offset = active.pan.as_ref().map_or(g.offset, |p| p.last);
        let view = active.resize.as_ref().map_or(
            View {
                size: g.size,
                offset,
            },
            |r| r.last,
        );
        let mut miniature = g;
        miniature.size = view.size;
        let dark = window(light, miniature, view.offset);
        popup
            .painter()
            .rect_filled(light, 0., Color32::from_gray(215));
        popup
            .painter()
            .rect_filled(dark, 0., Color32::from_gray(80));
        // Small-window Move helper is original-positive. The union/clipping
        // priority here is our policy; edge/corner targets below take priority.
        let mut hit = dark.expand(3.).intersect(light);
        if dark.width() < 40. || dark.height() < 40. {
            hit = hit.union(Rect::from_center_size(dark.center(), vec2(40., 40.)).intersect(light));
        }
        let response = popup.interact(hit, id.with("window"), Sense::drag());
        let handles = Handle::ALL.map(|handle| {
            (
                handle,
                popup.interact(
                    handle.rect(dark),
                    id.with(("handle", handle as u8)),
                    Sense::drag(),
                ),
            )
        });
        let pointer = popup.input(|i| i.pointer.interact_pos()).map(|p| {
            popup
                .ctx()
                .layer_transform_from_global(popup.layer_id())
                .unwrap_or_default()
                * p
        });
        let handle = pointer.and_then(|p| Handle::at(dark, p));
        if active.pan.is_none()
            && active.resize.is_none()
            && (response.is_pointer_button_down_on()
                || handles.iter().any(|(_, r)| r.is_pointer_button_down_on()))
            && popup.input(|i| {
                i.pointer.button_pressed(egui::PointerButton::Primary) && i.modifiers.is_none()
            })
            && let Some(start) = pointer
        {
            if let Some(handle) = handle {
                active.resize = Some(Resize::new(handle, start, light, g));
            } else {
                active.pan = Some(Pan {
                    start,
                    offset: g.offset,
                    last: g.offset,
                    light,
                });
            }
            output.gesture = Some(if active.resize.is_some() {
                MatrixViewGesture::BeginInnerResize {
                    size: g.size,
                    offset: g.offset,
                }
            } else {
                MatrixViewGesture::Begin {
                    size: g.size,
                    offset: g.offset,
                }
            });
        } else if let Some(resize) = active.resize.as_mut() {
            let released = popup.input(|i| i.pointer.button_released(egui::PointerButton::Primary));
            if released {
                if let Some(pointer) = pointer {
                    resize.last = resize.requested(pointer);
                }
                active.expected_view = Some(resize.last);
                output.gesture = Some(MatrixViewGesture::CommitInnerResize {
                    size: resize.last.size,
                    offset: resize.last.offset,
                });
                active.resize = None;
            } else if !popup.input(|i| i.pointer.primary_down()) {
                output.gesture = Some(MatrixViewGesture::Cancel);
                abort = true;
            } else if let Some(pointer) = pointer {
                let next = resize.requested(pointer);
                if next != resize.last {
                    resize.last = next;
                    active.expected_view = Some(next);
                    output.gesture = Some(MatrixViewGesture::PreviewInnerResize {
                        size: next.size,
                        offset: next.offset,
                    });
                }
            }
        } else if let Some(pan) = active.pan.as_mut() {
            let released = popup.input(|i| i.pointer.button_released(egui::PointerButton::Primary));
            if released {
                if let Some(pointer) = pointer {
                    pan.last = requested(pan, g, pointer);
                }
                active.expected_view = Some(View {
                    size: g.size,
                    offset: pan.last,
                });
                output.gesture = Some(MatrixViewGesture::Commit {
                    size: g.size,
                    offset: pan.last,
                });
                active.pan = None;
            } else if !popup.input(|i| i.pointer.primary_down()) {
                output.gesture = Some(MatrixViewGesture::Cancel);
                abort = true;
            } else if let Some(pointer) = pointer {
                let next = requested(pan, g, pointer);
                if next != pan.last {
                    pan.last = next;
                    active.expected_view = Some(View {
                        size: g.size,
                        offset: next,
                    });
                    output.gesture = Some(MatrixViewGesture::Preview {
                        size: g.size,
                        offset: next,
                    });
                }
            }
        }
        if let Some(handle) = active
            .resize
            .as_ref()
            .map(|r| r.handle)
            .or_else(|| handles.iter().find(|(_, r)| r.hovered()).map(|(h, _)| *h))
        {
            popup.ctx().set_cursor_icon(handle.cursor());
        } else if response.hovered() || active.pan.is_some() {
            popup.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }
    });
    if abort {
        open = false;
    }
    if open {
        ui.data_mut(|d| d.insert_temp(id, active));
    } else {
        if active.pan.is_some() || active.resize.is_some() || output.gesture.is_some() {
            output.gesture = Some(MatrixViewGesture::Cancel);
        }
        ui.data_mut(|d| d.remove::<State>(id));
    }
    output
}

#[cfg(test)]
mod tests;
