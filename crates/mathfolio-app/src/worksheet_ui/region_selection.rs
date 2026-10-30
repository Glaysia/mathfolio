//! Worksheet selection is transient. Gestures never mutate source until drop.
use super::*;
use std::collections::BTreeSet;

pub(super) const CLIPBOARD_PREFIX: &str = "mathfolio-regions:";

#[derive(Default)]
pub(super) struct State {
    pub ids: BTreeSet<u64>,
    /// Measured bounds relative to each region's paper-space anchor.
    pub bounds: HashMap<u64, Rect>,
    pub layer: Option<egui::LayerId>,
    pub paper_origin: egui::Pos2,
    marquee: Option<Marquee>,
    moving: Option<GroupMove>,
}
struct Marquee {
    start: egui::Pos2,
    end: egui::Pos2,
    active: bool,
    previous: Vec<u64>,
}
struct GroupMove {
    start: egui::Pos2,
    positions: Vec<(u64, f64, f64)>,
    members: HashSet<u64>,
    delta: (f64, f64),
}
impl State {
    fn hit_bounds(bounds: Rect, selected: bool) -> Rect {
        Rect::from_min_max(
            bounds.min - vec2(if selected { 14. } else { 0. }, 1.),
            bounds.max + vec2(0., 1.),
        )
    }
    pub fn preview_bottom(&self, heights: &HashMap<u64, f32>) -> Option<f64> {
        self.moving.as_ref().map(|m| {
            m.positions
                .iter()
                .map(|(id, _, y)| {
                    y + m.delta.1 + f64::from(heights.get(id).copied().unwrap_or(180.)) + 8.
                })
                .fold(0., f64::max)
        })
    }
    pub fn preview_right(&self, widths: &HashMap<u64, f32>) -> Option<f32> {
        self.moving.as_ref().map(|m| {
            m.positions
                .iter()
                .map(|(id, x, _)| {
                    (*x + m.delta.0) as f32 + widths.get(id).copied().unwrap_or(180.) + 48.
                })
                .fold(0., f32::max)
        })
    }
    pub fn marquee_cancel(&mut self) {
        self.marquee = None;
        self.moving = None;
    }
    pub fn contains(&self, primary: Option<u64>, id: u64) -> bool {
        primary == Some(id)
            || (primary.is_some_and(|p| self.ids.contains(&p)) && self.ids.contains(&id))
    }
    pub fn busy(&self) -> bool {
        self.marquee.is_some() || self.moving.is_some()
    }
    pub fn preview(&self, id: u64, left: f64, top: f64) -> (f64, f64) {
        self.moving
            .as_ref()
            .filter(|m| m.members.contains(&id))
            .map_or((left, top), |m| (left + m.delta.0, top + m.delta.1))
    }
}

impl WorksheetApp {
    pub(super) fn selected_region_count(&self) -> usize {
        if self
            .selected
            .is_some_and(|id| self.region_selection.ids.contains(&id))
        {
            self.region_selection.ids.len()
        } else {
            usize::from(self.selected.is_some())
        }
    }
    pub(super) fn selected_region_ids(&self) -> Vec<u64> {
        if self
            .selected
            .is_some_and(|id| self.region_selection.ids.contains(&id))
        {
            self.region_selection.ids.iter().copied().collect()
        } else {
            self.selected.into_iter().collect()
        }
    }
    fn select_regions(&mut self, ids: Vec<u64>) {
        self.region_selection.ids = ids.into_iter().collect();
        self.selected = self.region_selection.ids.first().copied();
        self.editing_region = None;
        self.focus_new = None;
        self.insertion = None;
    }
    fn selection_rect(
        &self,
        region: &CanvasRegion,
        geometry: crate::page::PageGeometry,
    ) -> Option<Rect> {
        self.region_selection
            .bounds
            .get(&region.id)
            .map(|r| r.translate(geometry.to_paper(region.left, region.top).to_vec2()))
    }
    fn release_selection_focus(ui: &egui::Ui) {
        ui.memory_mut(|m| {
            if let Some(id) = m.focused() {
                m.surrender_focus(id);
            }
        });
    }
    fn selection_autoscroll(ui: &egui::Ui) {
        if let Some(global) = ui.input(|i| i.pointer.latest_pos()) {
            let point = ui
                .ctx()
                .layer_transform_from_global(ui.layer_id())
                .unwrap_or_default()
                * global;
            let clip = ui.clip_rect();
            let x = if point.x > clip.right() - 16. {
                point.x + 24.
            } else if point.x < clip.left() + 16. {
                point.x - 24.
            } else {
                point.x
            };
            let y = if point.y > clip.bottom() - 16. {
                point.y + 24.
            } else if point.y < clip.top() + 16. {
                point.y - 24.
            } else {
                point.y
            };
            if x != point.x || y != point.y {
                ui.scroll_to_rect(Rect::from_min_size(pos2(x, y), vec2(1., 1.)), None);
            }
        }
    }
    fn page_pointer(ui: &egui::Ui, paper: Rect, point: egui::Pos2) -> egui::Pos2 {
        (ui.ctx()
            .layer_transform_from_global(ui.layer_id())
            .unwrap_or_default()
            * point
            - paper.min)
            .to_pos2()
    }
    /// Before widgets: a selected group's body or move handle starts one move.
    pub(super) fn selection_pointer_before(
        &mut self,
        ui: &egui::Ui,
        paper: Rect,
        geometry: crate::page::PageGeometry,
    ) {
        self.region_selection.layer = Some(ui.layer_id());
        self.region_selection.paper_origin = paper.min;
        if !ui.is_enabled() || ui.ctx().any_popup_open() {
            return;
        }
        if !ui.input(|i| i.focused) {
            self.region_selection.marquee_cancel();
            return;
        }
        if self.region_selection.busy()
            && ui.input_mut(|i| consume_exact_key(i, egui::Modifiers::NONE, egui::Key::Escape))
        {
            if let Some(m) = self.region_selection.marquee.take() {
                self.select_regions(m.previous);
            }
            self.region_selection.moving = None;
            return;
        }
        let press = ui.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers,
                } if modifiers.is_none() => Some(*pos),
                _ => None,
            })
        });
        if !self.region_selection.busy()
            && self.selected_region_count() > 1
            && let Some(press) = press
            && ui.clip_rect().contains(Self::page_pointer(
                ui,
                Rect::from_min_size(egui::Pos2::ZERO, paper.size()),
                press,
            ))
        {
            let point = Self::page_pointer(ui, paper, press);
            let hit = self.document.regions.iter().any(|r| {
                self.region_selection.contains(self.selected, r.id)
                    && self
                        .selection_rect(r, geometry)
                        .is_some_and(|b| State::hit_bounds(b, true).contains(point))
            });
            if hit {
                let positions: Vec<_> = self
                    .document
                    .regions
                    .iter()
                    .filter(|r| self.region_selection.contains(self.selected, r.id))
                    .map(|r| (r.id, r.left, r.top))
                    .collect();
                let members = positions.iter().map(|p| p.0).collect();
                self.region_selection.moving = Some(GroupMove {
                    start: point,
                    positions,
                    members,
                    delta: (0., 0.),
                });
                self.editing_region = None;
                self.focus_new = None;
                Self::release_selection_focus(ui);
            }
        }
        if let Some(point) = ui
            .input(|i| i.pointer.latest_pos())
            .map(|p| Self::page_pointer(ui, paper, p))
        {
            if let Some(m) = &mut self.region_selection.marquee {
                m.end = point;
                m.active |= (m.end - m.start).length() * self.zoom > 4.;
            }
            if let Some(m) = &mut self.region_selection.moving {
                let (_, x, y) = m.positions[0];
                let target_paper = geometry.to_paper(x, y) + (point - m.start);
                let mut target = geometry.to_body_clamped(target_paper);
                if self.expand_right {
                    target.0 = f64::from(target_paper.x - geometry.margins[0]).max(0.);
                }
                let min_x = m
                    .positions
                    .iter()
                    .map(|p| p.1)
                    .fold(f64::INFINITY, f64::min);
                let min_y = m
                    .positions
                    .iter()
                    .map(|p| p.2)
                    .fold(f64::INFINITY, f64::min);
                m.delta = ((target.0 - x).max(-min_x), (target.1 - y).max(-min_y));
            }
        }
    }
    /// After layout: select using the measured region bounds, in any quadrant.
    /// Returns true when a completed gesture must not become a blank-page click.
    pub(super) fn selection_pointer_after(
        &mut self,
        ui: &egui::Ui,
        paper: Rect,
        geometry: crate::page::PageGeometry,
    ) -> bool {
        if !ui.is_enabled() || ui.ctx().any_popup_open() {
            return false;
        }
        let released = ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary));
        if self.region_selection.moving.is_some() {
            if !released && !ui.input(|i| i.pointer.primary_down()) {
                self.region_selection.moving = None;
                return true;
            }
            if released {
                let m = self.region_selection.moving.take().unwrap();
                let (_, x, y) = m.positions[0];
                let grid = self.document.grid;
                let snap = |v: f64| (v / grid).round() * grid;
                let min_x = m
                    .positions
                    .iter()
                    .map(|p| p.1)
                    .fold(f64::INFINITY, f64::min);
                let min_y = m
                    .positions
                    .iter()
                    .map(|p| p.2)
                    .fold(f64::INFINITY, f64::min);
                let delta = (
                    (snap(x + m.delta.0) - x).max(-min_x),
                    (snap(y + m.delta.1) - y).max(-min_y),
                );
                if (m.delta.0.abs() + m.delta.1.abs()) * f64::from(self.zoom) > 4.
                    && delta != (0., 0.)
                {
                    self.commit_region_move(&m.positions, delta);
                }
            } else {
                Self::selection_autoscroll(ui);
            }
            ui.ctx().request_repaint();
            return true;
        }
        if self.region_selection.marquee.is_none() {
            let press = ui.input(|i| {
                i.events.iter().find_map(|e| match e {
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers,
                    } if modifiers.is_none() => Some(*pos),
                    _ => None,
                })
            });
            if let Some(press) = press {
                let point = Self::page_pointer(ui, paper, press);
                let on_body = geometry.to_body(point).is_some()
                    || (self.expand_right
                        && point.x >= geometry.margins[0]
                        && point.x < paper.width()
                        && geometry
                            .to_body(pos2(geometry.margins[0], point.y))
                            .is_some());
                let on_region = self.document.regions.iter().any(|r| {
                    self.selection_rect(r, geometry).is_some_and(|b| {
                        State::hit_bounds(b, self.region_selection.contains(self.selected, r.id))
                            .contains(point)
                    })
                });
                if on_body
                    && ui.clip_rect().contains(point + paper.min.to_vec2())
                    && !on_region
                    && self.dragging.is_none()
                {
                    let end = ui
                        .input(|i| i.pointer.latest_pos())
                        .map_or(point, |p| Self::page_pointer(ui, paper, p));
                    self.region_selection.marquee = Some(Marquee {
                        start: point,
                        end,
                        active: (end - point).length() * self.zoom > 4.,
                        previous: self.selected_region_ids(),
                    });
                    Self::release_selection_focus(ui);
                }
            }
        }
        let Some(m) = &self.region_selection.marquee else {
            return false;
        };
        let active = m.active;
        if active {
            let bounds = Rect::from_two_pos(m.start, m.end);
            let crossing = m.end.x < m.start.x;
            let ids = self
                .document
                .regions
                .iter()
                .filter(|r| !page_break_ui::is_marker(r))
                .filter(|r| {
                    self.selection_rect(r, geometry).is_some_and(|b| {
                        if crossing {
                            bounds.intersects(b)
                        } else {
                            bounds.contains_rect(b)
                        }
                    })
                })
                .map(|r| r.id)
                .collect();
            self.select_regions(ids);
            let color = if crossing {
                Color32::from_rgb(163, 190, 140)
            } else {
                crate::theme::palette(ui.ctx()).accent
            };
            let bounds = bounds.translate(paper.min.to_vec2());
            ui.painter()
                .rect_filled(bounds, 0., color.gamma_multiply(0.09));
            let stroke = Stroke::new(1. / self.zoom, color);
            if crossing {
                ui.painter().extend(egui::Shape::dashed_line(
                    &[
                        bounds.left_top(),
                        bounds.right_top(),
                        bounds.right_bottom(),
                        bounds.left_bottom(),
                        bounds.left_top(),
                    ],
                    stroke,
                    5. / self.zoom,
                    3. / self.zoom,
                ));
            } else {
                ui.painter()
                    .rect_stroke(bounds, 0., stroke, egui::StrokeKind::Inside);
            }
            ui.ctx().request_repaint();
            if !released {
                Self::selection_autoscroll(ui);
            }
        }
        if released || !ui.input(|i| i.pointer.primary_down()) {
            self.region_selection.marquee = None;
        }
        active
    }
    pub(super) fn commit_region_move(&mut self, positions: &[(u64, f64, f64)], delta: (f64, f64)) {
        if delta == (0., 0.) {
            return;
        }
        let before = self.snapshot_with_document(DocumentHistory::Positions(positions.to_vec()));
        let positions: HashMap<_, _> = positions.iter().map(|(id, x, y)| (*id, (*x, *y))).collect();
        for r in &mut self.document.regions {
            if let Some((x, y)) = positions.get(&r.id) {
                r.left = x + delta.0;
                r.top = y + delta.1;
            }
        }
        self.remember(before);
        self.calculation_changed(false);
    }
}
