//! Blank root crosshair page-break commands. See the static native contract0141.
//! Transactions measure committed presentation without evaluating or editing it.
use super::*;
use mathcraft_document::page_break::{
    self, PageBreakBounds, PageBreakBoundsKind, PageBreakLayout, RemovePageBreakDirection,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PageBreakRequest {
    Insert,
    Above,
    Below,
}

#[derive(Clone, Copy)]
pub(super) struct QueuedPageBreak {
    request: PageBreakRequest,
    point: (f64, f64),
}

pub(super) fn is_marker(region: &CanvasRegion) -> bool {
    matches!(&region.content, CanvasContent::Opaque {kind, ..} if kind == "pageBreak")
}

#[derive(Default)]
pub(super) struct MarkerCache {
    entries: HashMap<u64, (String, Result<(), String>, bool)>,
    #[cfg(test)]
    parses: usize,
}
impl MarkerCache {
    fn validate(&mut self, region: &CanvasRegion) -> Result<(), String> {
        let CanvasContent::Opaque { xml, .. } = &region.content else {
            return Err("Invalid page-break marker source".into());
        };
        if self
            .entries
            .get(&region.id)
            .is_none_or(|(old, _, _)| old != xml)
        {
            #[cfg(test)]
            {
                self.parses += 1;
            }
            let result = page_break::is_page_break(region).and_then(|valid| {
                if valid {
                    Ok(())
                } else {
                    Err("Invalid page-break marker source".into())
                }
            });
            self.entries.insert(region.id, (xml.clone(), result, true));
        }
        let (_, result, seen) = self
            .entries
            .get_mut(&region.id)
            .expect("validated marker slot");
        *seen = true;
        result.clone()
    }
}

impl WorksheetApp {
    pub(super) fn can_request_page_break(&self) -> bool {
        self.pending.is_none()
            && self.selected.is_none()
            && self.editing_region.is_none()
            && self.focus_new.is_none()
            && self.insertion.is_some()
            && self.uncommitted_math.is_empty()
            && self.dragging.is_none()
            && self.plot_draft.is_none()
            && self.solve_draft.is_none()
            && self.page_sections_draft.is_none()
    }
    pub(super) fn page_break_shortcuts(&mut self, ctx: &egui::Context) {
        if !self.can_request_page_break() || ctx.any_popup_open() {
            return;
        }
        // Run before the ordinary region clipboard Delete handler, which consumes
        // Delete even when no ordinary region is selected.
        for (key, modifiers, request) in [
            (
                egui::Key::Enter,
                egui::Modifiers::COMMAND,
                PageBreakRequest::Insert,
            ),
            (
                egui::Key::Backspace,
                egui::Modifiers::NONE,
                PageBreakRequest::Above,
            ),
            (
                egui::Key::Delete,
                egui::Modifiers::NONE,
                PageBreakRequest::Below,
            ),
        ] {
            if ctx.input_mut(|input| consume_exact_key(input, modifiers, key)) {
                self.queue_page_break(request);
                return;
            }
        }
    }
    pub(super) fn queue_page_break(&mut self, request: PageBreakRequest) {
        if self.can_request_page_break() {
            self.page_break_request = Some(QueuedPageBreak {
                request,
                point: self.insertion.unwrap(),
            });
        }
    }
    pub(super) fn page_break_removal_availability(&self) -> (bool, bool) {
        if !self.can_request_page_break() {
            return (false, false);
        }
        let mut breaks = self
            .document
            .regions
            .iter()
            .filter(|region| is_marker(region));
        let Some(region) = breaks.next() else {
            return (false, false);
        };
        if breaks.next().is_some() {
            return (false, false);
        }
        let y = self.insertion.unwrap().1;
        (
            y - self.document.grid < region.top && region.top <= y,
            y < region.top && region.top <= y + self.document.grid,
        )
    }
    fn page_break_layout(&self) -> PageBreakLayout {
        PageBreakLayout {
            body_height: self.page_geometry().body_height_exact(),
            grid_height: self.document.grid,
        }
    }
    fn area_break_height(
        &self,
        ui: &egui::Ui,
        region: &CanvasRegion,
        xml: &str,
    ) -> Result<f32, String> {
        let metadata = mathcraft_document::area::areas_from_xml(xml)?
            .into_iter()
            .find(|a| a.region_path.is_empty())
            .ok_or("Invalid owning Area")?;
        if metadata.protected {
            return Err("Protected Area page-break movement is unsupported".into());
        }
        if metadata.is_collapsed {
            return Ok(22.);
        }
        let mut cache = area_presentation::AreaPresentationCache::default();
        cache.refresh(xml, region.format)?;
        let empty = HashMap::new();
        let saved_empty = HashMap::new();
        let prepared = cache.prepare(
            region.id,
            self.area_results.get(&region.id).unwrap_or(&empty),
            self.area_saved.get(&region.id).unwrap_or(&saved_empty),
            &self.area_pending,
        );
        let native = self
            .area_visuals
            .get(&region.id)
            .map(Vec::as_slice)
            .unwrap_or_default();
        let mut views = prepared.borrow_views(native);
        if let Some(labelled) = self
            .committed_input
            .as_ref()
            .and_then(|input| input.area_labelled.get(&region.id))
        {
            for (key, tree) in labelled {
                views.entry(key.clone()).or_default().labelled = Some(tree);
            }
        }
        let sources = mathcraft_document::area::area_math_children_xml(xml, &[])?;
        let keys: HashSet<_> = sources
            .iter()
            .map(|source| area_ui::child_key(&source.region_path))
            .collect();
        let mut height = area_authoring::expanded_height(xml)?;
        for source in sources {
            let key = area_ui::child_key(&source.region_path);
            let view = views.get(&key).copied().unwrap_or_default();
            let tree = worksheet_math::tree_from_mathml(&source.mathml)?;
            let editor = Editor::from_portable(&tree).map_err(|e| e.to_string())?;
            let (size, baseline) = formula::measured_geometry_readonly_styled(
                ui,
                &editor,
                view.display.map(formula::FormulaResult::Formatted),
                view.labelled,
                view.font_points.unwrap_or(region.format.font_points),
            );
            height = height.max(source.top as f32 - baseline + size.y + 4.);
        }
        for path in metadata.child_region_paths {
            let key = area_ui::child_key(&path);
            if keys.contains(&key) {
                continue;
            }
            let view = views
                .get(&key)
                .ok_or("Area child geometry is unavailable")?;
            let (Some([_, y]), Some(native)) = (view.local, view.native) else {
                return Err("Unsupported Area child page-break geometry".into());
            };
            height = height.max(y as f32 + native.size[1] + 4.);
        }
        Ok(height.max(22.))
    }
    /// Measure every owner, including offscreen content. Never trust old cached
    /// response heights after font/result/source changes.
    pub(super) fn page_break_bounds(
        &self,
        ui: &mut egui::Ui,
    ) -> Result<Vec<PageBreakBounds>, String> {
        let mut out = Vec::new();
        for region in &self.document.regions {
            if page_break::is_page_break(region)? {
                continue;
            }
            let width =
                (self.paper_size().x - self.page_geometry().margins[0] - region.left as f32 - 15.)
                    .max(100.);
            let mut measure = ui.new_child(
                UiBuilder::new()
                    .id_salt(("page-break-measure", region.id))
                    .max_rect(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        vec2(width, 1_000_000.),
                    ))
                    .layout(egui::Layout::left_to_right(egui::Align::Min))
                    .invisible()
                    .disabled(),
            );
            // Generic native visuals intentionally report Area as unsupported;
            // its independent source-bound child presentation is authoritative.
            let visual = if matches!(&region.content, CanvasContent::Opaque {kind,..} if kind == "Area")
            {
                None
            } else {
                self.native_visuals
                    .region(region)
                    .map(Result::as_ref)
                    .transpose()
                    .map_err(Clone::clone)?
            };
            let native_anchor = native_alignment(region, &self.native_visuals);
            let (height, offset, kind) = match &region.content {
                CanvasContent::Opaque { xml, kind } if kind == "Area" => (
                    self.area_break_height(&measure, region, xml)?,
                    0.,
                    PageBreakBoundsKind::PageWide,
                ),
                _ if visual.is_some_and(|v| v.document.is_some()) => {
                    let visual = visual.unwrap();
                    let width = if visual.size[0] > 0. {
                        visual.size[0]
                    } else {
                        width
                    };
                    (
                        crate::native_presentation::measure(&measure, visual, width).y,
                        if native_anchor {
                            crate::native_presentation::alignment_offset(
                                &measure,
                                visual,
                                width,
                                is_spec_table(region),
                            )
                        } else {
                            0.
                        },
                        PageBreakBoundsKind::Ordinary,
                    )
                }
                CanvasContent::Math(tree) => {
                    let editor = Editor::from_portable(tree).map_err(|e| e.to_string())?;
                    let display = self
                        .results
                        .get(&region.id)
                        .and_then(|row| row.display_value.as_ref().or(row.value.as_ref()))
                        .map(|value| crate::result_display::format(value, &region.format))
                        .or_else(|| {
                            self.saved_results
                                .get(&region.id)
                                .and_then(|row| numeric_cache::display(row, &region.format))
                                .or_else(|| {
                                    self.saved_results
                                        .get(&region.id)
                                        .and_then(|row| row.display.as_deref())
                                        .and_then(
                                            crate::result_display::FormattedResult::from_saved,
                                        )
                                })
                        });
                    let (size, baseline) = formula::measured_geometry_readonly_styled(
                        &measure,
                        &editor,
                        display.as_ref().map(formula::FormulaResult::Formatted),
                        self.labelled_trees.get(&region.id),
                        region.format.font_points,
                    );
                    (
                        size.y,
                        if native_anchor { -baseline } else { 0. },
                        PageBreakBoundsKind::Ordinary,
                    )
                }
                CanvasContent::Text(text) => {
                    let mut text = text.clone();
                    let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, width: f32| {
                        typography::layout_job(
                            ui.painter(),
                            egui::text::LayoutJob::simple(
                                text.as_str().into(),
                                typography::body_font(),
                                Color32::BLACK,
                                width,
                            ),
                        )
                    };
                    let output = egui::TextEdit::multiline(&mut text)
                        .id_salt(("page-break-text", region.id))
                        .frame(egui::Frame::NONE)
                        .desired_rows(1)
                        .desired_width(340.)
                        .font(typography::body_font())
                        .interactive(false)
                        .layouter(&mut layouter)
                        .show(&mut measure);
                    (
                        output.response.response.rect.height(),
                        0.,
                        PageBreakBoundsKind::Ordinary,
                    )
                }
                CanvasContent::Plot(spec) => (spec.height, 0., PageBreakBoundsKind::Ordinary),
                _ => {
                    return Err(format!(
                        "Region {} has unsupported page-break geometry",
                        region.id
                    ));
                }
            };
            if !height.is_finite() || !offset.is_finite() || height < 0. {
                return Err("Invalid page-break presentation geometry".into());
            }
            out.push(PageBreakBounds {
                region_id: region.id,
                bottom: region.top + f64::from((offset + height).max(0.)),
                kind,
            });
        }
        Ok(out)
    }
    pub(super) fn apply_page_break_request(&mut self, ui: &mut egui::Ui) {
        let Some(queued) = self.page_break_request.take() else {
            return;
        };
        if !self.can_request_page_break()
            || self.insertion != Some(queued.point)
            || ui.ctx().any_popup_open()
        {
            return;
        }
        let request = queued.request;
        let mut apply = || {
            let point = self
                .insertion
                .map(|(x, y)| [x, y])
                .ok_or("Page break requires a blank insertion point")?;
            let bounds = self.page_break_bounds(ui)?;
            let layout = self.page_break_layout();
            match request {
                PageBreakRequest::Insert => {
                    page_break::insert_page_break(&self.document, layout, point, &bounds)
                }
                PageBreakRequest::Above | PageBreakRequest::Below => {
                    let direction = if request == PageBreakRequest::Above {
                        RemovePageBreakDirection::Above
                    } else {
                        RemovePageBreakDirection::Below
                    };
                    let ids = self
                        .document
                        .regions
                        .iter()
                        .filter(|region| is_marker(region))
                        .map(|region| region.id)
                        .collect::<Vec<_>>();
                    if ids.len() != 1 {
                        return Err("Only a single adjacent page break can be removed".into());
                    }
                    page_break::remove_single_page_break(
                        &self.document,
                        layout,
                        point,
                        ids[0],
                        direction,
                        &bounds,
                    )
                }
            }
        };
        match apply() {
            Err(error) => self.status = error,
            Ok(change) => {
                let before = self.snapshot();
                self.document = change.document;
                self.insertion = Some((change.insertion[0], change.insertion[1]));
                self.selected = None;
                self.editing_region = None;
                self.focus_new = None;
                self.reveal = self.insertion;
                self.remember(before);
                self.status.clear();
                self.calculation_changed(false);
                ui.ctx().request_repaint();
            }
        }
    }
    pub(super) fn paint_page_break_markers(
        &mut self,
        ui: &egui::Ui,
        paper: Rect,
        geometry: crate::page::PageGeometry,
    ) {
        if self.expand_right {
            self.page_break_markers.entries.clear();
            return;
        }
        for (_, _, seen) in self.page_break_markers.entries.values_mut() {
            *seen = false;
        }
        let mut breaks = Vec::new();
        for region in self
            .document
            .regions
            .iter()
            .filter(|region| is_marker(region))
        {
            match self.page_break_markers.validate(region) {
                Ok(())
                    if region.top.is_finite() && region.left.abs() < 1e-10 && region.top >= 0. =>
                {
                    breaks.push(region.top)
                }
                Ok(()) => self.status = "Invalid page-break marker coordinates".into(),
                Err(error) => self.status = error,
            }
        }
        self.page_break_markers
            .entries
            .retain(|_, (_, _, seen)| *seen);
        breaks.sort_by(f64::total_cmp);
        for top in breaks {
            let page = ((top + self.document.grid / 2.) / geometry.body_height_exact()).trunc();
            let y = paper.top() + geometry.stride() * page as f32
                - (geometry.stride() - geometry.height) / 2.
                + 1.;
            ui.painter().extend(egui::Shape::dashed_line(
                &[
                    pos2(paper.left(), y),
                    pos2(paper.left() + geometry.width, y),
                ],
                Stroke::new(4., Color32::from_gray(120)),
                8.,
                8.,
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn context() -> egui::Context {
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        ctx
    }
    fn key(key: egui::Key, command: bool) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed: true,
            repeat: false,
            modifiers: if command {
                egui::Modifiers {
                    ctrl: true,
                    command: true,
                    ..Default::default()
                }
            } else {
                egui::Modifiers::NONE
            },
        }
    }
    fn frame(app: &mut WorksheetApp, ctx: &egui::Context, events: Vec<egui::Event>) {
        ctx.run_ui(
            egui::RawInput {
                events,
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1200., 1800.))),
                ..Default::default()
            },
            |ui| {
                app.global_editing_shortcuts(ui.ctx());
                app.document_history_shortcuts(ui.ctx());
                app.region_clipboard_shortcuts(ui.ctx());
                app.page_ui(ui);
            },
        )
        .drop_without_applying_deltas();
    }
    fn math(app: &mut WorksheetApp, left: f64, top: f64) -> u64 {
        let id = app.document.add_math(left, top);
        app.document.regions.last_mut().unwrap().left = left;
        app.document.regions.last_mut().unwrap().top = top;
        app.document.regions.last_mut().unwrap().content = CanvasContent::Math(PortableTree {
            kind: NodeKind::Number("7".into()),
            children: vec![],
        });
        id
    }
    fn breaks(app: &WorksheetApp) -> usize {
        app.document.regions.iter().filter(|r| is_marker(r)).count()
    }

    #[test]
    fn blank_ctrl_enter_moves_suffix_and_crosshair_as_one_undo_redo() {
        let ctx = context();
        let mut app = WorksheetApp {
            automatic_calculation: false,
            insertion: Some((10., 100.)),
            ..Default::default()
        };
        math(&mut app, 0., 40.);
        math(&mut app, 20., 100.);
        math(&mut app, 0., 130.);
        let before = app.document.clone();
        let h = app.page_break_layout().body_height;
        frame(&mut app, &ctx, vec![key(egui::Key::Enter, true)]);
        assert_eq!(breaks(&app), 1, "{}", app.status);
        assert_eq!(app.undo.len(), 1);
        assert_eq!(app.document.regions[0], before.regions[0]);
        assert_eq!(app.document.regions[1].top, h);
        assert_eq!(app.document.regions[2].top, h + 30.);
        assert_eq!(app.insertion, Some((0., h)));
        assert!(app.selected.is_none());
        assert!(app.last_executed_regions.is_empty());
        let after = app.document.clone();
        app.undo();
        assert_eq!(app.document, before);
        assert_eq!(app.insertion, Some((10., 100.)));
        app.redo();
        assert_eq!(app.document, after);
        assert_eq!(app.insertion, Some((0., h)));
    }
    #[test]
    fn active_or_selected_ctrl_enter_and_ribbon_never_insert_a_break() {
        let ctx = context();
        let mut app = WorksheetApp::default();
        let id = math(&mut app, 0., 30.);
        app.selected = Some(id);
        app.focus_new = Some(id);
        app.insertion = None;
        frame(&mut app, &ctx, vec![]);
        frame(&mut app, &ctx, vec![key(egui::Key::Enter, true)]);
        assert_eq!(breaks(&app), 0);
        assert_eq!(app.document.regions.len(), 1);
        app.ribbon_command(&ctx, RibbonAction::InsertPageBreak);
        frame(&mut app, &ctx, vec![]);
        assert_eq!(breaks(&app), 0);
        assert!(app.undo.is_empty());
    }
    #[test]
    fn ribbon_is_deferred_and_blank_backspace_removes_only_adjacent_break_atomically() {
        let ctx = context();
        let mut app = WorksheetApp {
            automatic_calculation: false,
            insertion: Some((0., 100.)),
            ..Default::default()
        };
        app.ribbon_command(&ctx, RibbonAction::InsertPageBreak);
        assert_eq!(breaks(&app), 0);
        frame(&mut app, &ctx, vec![]);
        assert_eq!(breaks(&app), 1);
        assert_eq!(app.undo.len(), 1);
        let inserted = app.document.clone();
        frame(&mut app, &ctx, vec![key(egui::Key::Backspace, false)]);
        assert_eq!(breaks(&app), 0, "{}", app.status);
        assert_eq!(app.undo.len(), 2);
        assert_eq!(app.insertion, Some((0., 0.)));
        app.undo();
        assert_eq!(app.document, inserted);
        assert_eq!(breaks(&app), 1);
        app.insertion = Some((0., 20.));
        let before = app.document.clone();
        let history = app.undo.len();
        app.ribbon_command(&ctx, RibbonAction::RemovePageBreakAbove);
        frame(&mut app, &ctx, vec![]);
        assert_eq!(app.document, before);
        assert_eq!(app.undo.len(), history);
        assert!(!app.status.is_empty());
    }
    #[test]
    fn below_delete_uses_live_previous_bottom_not_a_fixed_page_delta() {
        let ctx = context();
        let mut app = WorksheetApp {
            automatic_calculation: false,
            insertion: Some((0., 100.)),
            ..Default::default()
        };
        math(&mut app, 0., 30.);
        let next = math(&mut app, 0., 150.);
        frame(&mut app, &ctx, vec![key(egui::Key::Enter, true)]);
        let h = app.page_break_layout().body_height;
        app.insertion = Some((0., h - app.document.grid));
        frame(&mut app, &ctx, vec![key(egui::Key::Delete, false)]);
        assert_eq!(breaks(&app), 0, "{}", app.status);
        assert_eq!(app.undo.len(), 2);
        let next = app.document.regions.iter().find(|r| r.id == next).unwrap();
        assert!(next.top > 30. && next.top < 200.);
        assert!(app.last_executed_regions.is_empty());
    }
    #[test]
    fn invalid_geometry_or_native_break_rejects_without_mutation_or_history() {
        let ctx = context();
        for (xml, kind) in [
            ("<bad/>", "unknown"),
            (
                "<region xmlns='http://schemas.mathsoft.com/worksheet50'><pageBreak bogus='1'/></region>",
                "pageBreak",
            ),
        ] {
            let mut app = WorksheetApp {
                insertion: Some((0., 100.)),
                ..Default::default()
            };
            app.document.add_opaque(0., 20., xml, kind);
            let before = app.document.clone();
            frame(&mut app, &ctx, vec![key(egui::Key::Enter, true)]);
            assert_eq!(app.document, before);
            assert!(app.undo.is_empty());
            assert!(!app.status.is_empty());
        }
    }
    #[test]
    fn marker_is_passive_excluded_from_tab_and_remove_selected() {
        let ctx = context();
        let mut app = WorksheetApp {
            automatic_calculation: false,
            insertion: Some((0., 100.)),
            ..Default::default()
        };
        let id = math(&mut app, 0., 30.);
        frame(&mut app, &ctx, vec![key(egui::Key::Enter, true)]);
        let marker = app
            .document
            .regions
            .iter()
            .find(|r| is_marker(r))
            .unwrap()
            .id;
        frame(&mut app, &ctx, vec![key(egui::Key::Tab, false)]);
        assert_eq!(app.selected, Some(id));
        app.selected = Some(marker);
        let before = app.document.clone();
        let n = app.undo.len();
        app.remove_selected();
        assert_eq!(app.document, before);
        assert_eq!(app.undo.len(), n);
        app.selected = None;
        app.insertion = Some((0., 100.));
        let geometry = app.page_geometry();
        let y = geometry.stride() - (geometry.stride() - geometry.height) / 2. + 1.;
        // run_ui's root origin is zero; a click on the passive marker must only
        // use the worksheet background, never select the stored marker owner.
        frame(
            &mut app,
            &ctx,
            vec![
                egui::Event::PointerMoved(pos2(200., y)),
                egui::Event::PointerButton {
                    pos: pos2(200., y),
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerButton {
                pos: pos2(200., y),
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert_ne!(app.selected, Some(marker));
        assert_eq!(app.document, before);
    }
    #[test]
    fn area_bounds_are_eager_and_crossing_page_wide_region_is_atomic() {
        let ctx = context();
        let mut app = WorksheetApp {
            automatic_calculation: false,
            insertion: Some((0., 100.)),
            ..Default::default()
        };
        let xml = "<region xmlns='http://schemas.mathsoft.com/worksheet50' xmlns:ml='http://schemas.mathsoft.com/math50'><Area AreaHeight='200'><regions><region left='2' top='160'><math><ml:real>7</ml:real></math></region></regions></Area></region>";
        let id = app.document.add_opaque(0., 20., xml, "Area");
        let before = app.document.clone();
        let mut measured = None;
        ctx.run_ui(egui::RawInput::default(), |ui| {
            measured = Some(app.page_break_bounds(ui).unwrap());
        })
        .drop_without_applying_deltas();
        let measured = measured.unwrap();
        assert_eq!(measured[0].region_id, id);
        assert_eq!(measured[0].kind, PageBreakBoundsKind::PageWide);
        assert!(measured[0].bottom >= before.regions[0].top + 200.);
        frame(&mut app, &ctx, vec![key(egui::Key::Enter, true)]);
        assert_eq!(app.document, before);
        assert!(app.undo.is_empty());
        assert!(app.status.contains("page-wide"));
        assert!(app.last_executed_regions.is_empty());
    }
    #[test]
    fn imported_area_owner_visual_error_does_not_hide_supported_child_bounds() {
        let ctx = context();
        let mut app = WorksheetApp::default();
        let xml = "<region xmlns='http://schemas.mathsoft.com/worksheet50' xmlns:ml='http://schemas.mathsoft.com/math50' height='120'><Area AreaHeight='0'><regions><region left='2' top='60'><math><ml:real>7</ml:real></math></region></regions></Area></region>";
        let id = app.document.add_opaque(0., 20., xml, "Area");
        app.document.regions[0].source_region = Some("owner".into());
        Arc::make_mut(&mut app.native_visuals)
            .regions
            .insert("owner".into(), Err("Unsupported native Area visual".into()));
        ctx.run_ui(egui::RawInput::default(), |ui| {
            let bounds = app.page_break_bounds(ui).unwrap();
            assert_eq!(bounds[0].region_id, id);
            assert!(bounds[0].bottom >= app.document.regions[0].top + 120.);
        })
        .drop_without_applying_deltas();
        app.document.regions[0].content = CanvasContent::Math(PortableTree {
            kind: NodeKind::Number("7".into()),
            children: vec![],
        });
        ctx.run_ui(egui::RawInput::default(), |ui| {
            assert!(app.page_break_bounds(ui).is_err())
        })
        .drop_without_applying_deltas();
    }
    #[test]
    fn deferred_request_is_cancelled_by_selection_point_change_and_new_document() {
        let ctx = context();
        let mut app = WorksheetApp::default();
        app.queue_page_break(PageBreakRequest::Insert);
        app.insertion = Some((0., 100.));
        frame(&mut app, &ctx, vec![]);
        assert_eq!(breaks(&app), 0);
        app.queue_page_break(PageBreakRequest::Insert);
        let id = math(&mut app, 0., 20.);
        app.selected = Some(id);
        frame(&mut app, &ctx, vec![]);
        assert_eq!(breaks(&app), 0);
        app.selected = None;
        app.queue_page_break(PageBreakRequest::Insert);
        app.apply(Pending::New);
        frame(&mut app, &ctx, vec![]);
        assert_eq!(breaks(&app), 0);
        assert!(app.undo.is_empty());
    }

    #[test]
    #[ignore = "Writes native page-break authoring/save/reopen fixture; no GUI or original runtime"]
    fn page_break_authoring_save_native_export_reopen_probe() {
        let output = std::env::var_os("MATHCRAFT_PAGE_BREAK_OUTPUT")
            .map(PathBuf::from)
            .expect("MATHCRAFT_PAGE_BREAK_OUTPUT");
        std::fs::create_dir_all(&output).unwrap();
        let ctx = context();
        let mut app = WorksheetApp {
            automatic_calculation: false,
            insertion: Some((0., 100.)),
            ..Default::default()
        };
        for (top, value) in [(30., "7"), (150., "14")] {
            math(&mut app, 0., top);
            app.document.regions.last_mut().unwrap().content = CanvasContent::Math(PortableTree {
                kind: NodeKind::Binary(BinaryOp::Evaluate),
                children: vec![
                    PortableTree {
                        kind: NodeKind::Number(value.into()),
                        children: vec![],
                    },
                    PortableTree {
                        kind: NodeKind::Placeholder,
                        children: vec![],
                    },
                ],
            });
        }
        app.recalculate();
        assert!(app.results.values().all(|row| row.error.is_none()));
        assert_eq!(app.results.len(), 2);
        app.last_executed_regions.clear();
        frame(&mut app, &ctx, vec![key(egui::Key::Enter, true)]);
        assert_eq!(breaks(&app), 1, "{}", app.status);
        assert_eq!(app.undo.len(), 1);
        let after = app.document.clone();
        app.undo();
        assert_eq!(breaks(&app), 0);
        app.redo();
        assert_eq!(app.document, after);
        // Refresh explicit saved presentations after movement; this is the only
        // intentional execution in the fixture setup, never during reopen.
        app.recalculate();
        app.persist_calculation_cache();
        app.document.validate().unwrap();
        let saved = output.join("page-break.mathcraft");
        app.document.save(&saved).unwrap();
        let mut reopened = WorksheetApp::default();
        reopened.open(&saved);
        assert_eq!(breaks(&reopened), 1);
        assert!(reopened.last_executed_regions.is_empty());
        assert!(reopened.pending_regions.is_empty(), "{}", reopened.status);
        let native = output.join("page-break.mcdx");
        mathcraft_document::export::export_mcdx(&app.document, &native).unwrap();
        let mut imported = WorksheetApp::default();
        imported.open(&native);
        assert_eq!(breaks(&imported), 1, "{}", imported.status);
        assert!(imported.last_executed_regions.is_empty());
        assert!(imported.pending_regions.is_empty(), "{}", imported.status);
        let original: Vec<_> = app
            .document
            .regions
            .iter()
            .map(|r| (r.left, r.top, is_marker(r)))
            .collect();
        let actual: Vec<_> = imported
            .document
            .regions
            .iter()
            .map(|r| (r.left, r.top, is_marker(r)))
            .collect();
        assert_eq!(original, actual);
        assert_eq!(imported.saved_results.len(), 2);
        assert!(
            imported
                .saved_results
                .values()
                .all(|row| row.status == SavedCalculationStatus::Synchronized)
        );
        imported.persist_calculation_cache();
        imported
            .document
            .save(&output.join("page-break-reimported.mathcraft"))
            .unwrap();
        println!(
            "PAGE_BREAK_PROBE {}",
            serde_json::json!({"regions":3,"math_results":2,"breaks":1,"blank_ctrl_enter":true,"one_undo_redo":true,"saved_and_native_reopen_exec":0,"native_coordinate_pairs":original})
        );
    }
    #[test]
    fn passive_marker_validation_is_exact_source_cached_pruned_and_draft_hidden() {
        let ctx = context();
        let mut app = WorksheetApp {
            automatic_calculation: false,
            insertion: Some((0., 100.)),
            ..Default::default()
        };
        frame(&mut app, &ctx, vec![key(egui::Key::Enter, true)]);
        frame(&mut app, &ctx, vec![]);
        let count = app.page_break_markers.parses;
        assert_eq!(count, 1);
        for _ in 0..3 {
            frame(&mut app, &ctx, vec![]);
        }
        assert_eq!(app.page_break_markers.parses, count);
        let CanvasContent::Opaque { xml, .. } = &mut app.document.regions[0].content else {
            panic!()
        };
        *xml = xml.replace("<pageBreak/>", "<pageBreak invalid='1'/>");
        frame(&mut app, &ctx, vec![]);
        assert_eq!(app.page_break_markers.parses, count + 1);
        assert!(!app.status.is_empty());
        frame(&mut app, &ctx, vec![]);
        assert_eq!(app.page_break_markers.parses, count + 1);
        app.document.regions.clear();
        frame(&mut app, &ctx, vec![]);
        assert!(app.page_break_markers.entries.is_empty());
        app.queue_page_break(PageBreakRequest::Insert);
        frame(&mut app, &ctx, vec![]);
        frame(&mut app, &ctx, vec![]);
        assert_eq!(app.page_break_markers.entries.len(), 1);
        app.expand_right = true;
        frame(&mut app, &ctx, vec![]);
        assert!(app.page_break_markers.entries.is_empty());
    }
    #[test]
    fn readonly_live_bounds_match_painted_math_plain_text_and_native_rich_presentation() {
        let ctx = context();
        let mut app = WorksheetApp::default();
        let math_id = math(&mut app, 0., 20.);
        let text_id = app.document.add_text(0., 100., "plain text\nsecond line");
        let native_id = app.document.add_text(0., 250., "native note");
        app.document.regions.last_mut().unwrap().source_region = Some("native-note".into());
        let rich=mathcraft_document::native_visual::read_flow_xml("<FlowDocument xmlns='http://schemas.microsoft.com/winfx/2006/xaml/presentation'><Paragraph>native note</Paragraph></FlowDocument>",Default::default()).unwrap();
        let visual = mathcraft_document::native_visual::RegionVisual {
            size: [180., 40.],
            background: None,
            document: Some(rich),
        };
        Arc::make_mut(&mut app.native_visuals)
            .regions
            .insert("native-note".into(), Ok(visual.clone()));
        let source = app.document.clone();
        frame(&mut app, &ctx, vec![]);
        ctx.run_ui(egui::RawInput::default(), |ui| {
            let bounds = app.page_break_bounds(ui).unwrap();
            for id in [math_id, text_id, native_id] {
                let region = app.document.regions.iter().find(|r| r.id == id).unwrap();
                let bound = bounds.iter().find(|b| b.region_id == id).unwrap();
                let offset = if id == native_id {
                    crate::native_presentation::alignment_offset(ui, &visual, 180., false)
                } else {
                    0.
                };
                let expected = region.top + f64::from(offset + app.region_heights[&id]);
                assert!(
                    (bound.bottom - expected).abs() < 0.01,
                    "owner{id}: {} != {expected}",
                    bound.bottom
                );
            }
        })
        .drop_without_applying_deltas();
        assert_eq!(app.document, source);
        assert!(app.last_executed_regions.is_empty());
        assert!(app.undo.is_empty());
        // A fresh read must use the new font rather than the previous response height.
        let old = app.region_heights[&math_id];
        app.document.regions[0].format.font_points = 40.;
        ctx.run_ui(egui::RawInput::default(), |ui| {
            let bound = app
                .page_break_bounds(ui)
                .unwrap()
                .into_iter()
                .find(|b| b.region_id == math_id)
                .unwrap();
            assert!(bound.bottom - app.document.regions[0].top > f64::from(old));
        })
        .drop_without_applying_deltas();
    }
    #[test]
    fn blank_control_enter_does_not_accept_extra_shift_or_alt() {
        let ctx = context();
        for modifiers in [
            egui::Modifiers {
                ctrl: true,
                command: true,
                shift: true,
                ..Default::default()
            },
            egui::Modifiers {
                ctrl: true,
                command: true,
                alt: true,
                ..Default::default()
            },
        ] {
            let mut app = WorksheetApp::default();
            let before = app.document.clone();
            let mut event = key(egui::Key::Enter, true);
            let egui::Event::Key {
                modifiers: actual, ..
            } = &mut event
            else {
                panic!()
            };
            *actual = modifiers;
            frame(&mut app, &ctx, vec![event]);
            assert_eq!(app.document, before);
            assert!(app.undo.is_empty());
            assert_eq!(breaks(&app), 0);
        }
    }
    #[test]
    fn valid_break_is_inert_in_prepared_input_but_malformed_marker_remains_a_fault() {
        let ctx = context();
        let mut app = WorksheetApp {
            automatic_calculation: false,
            insertion: Some((0., 100.)),
            ..Default::default()
        };
        math(&mut app, 0., 30.);
        frame(&mut app, &ctx, vec![key(egui::Key::Enter, true)]);
        let input = app.prepare_calculation_input().unwrap();
        assert!(input.faults.is_empty());
        app.recalculate();
        assert!(app.calculation_error.is_none());
        assert!(app.results.values().all(|row| row.error.is_none()));
        let marker = app
            .document
            .regions
            .iter_mut()
            .find(|r| is_marker(r))
            .unwrap();
        let id = marker.id;
        let CanvasContent::Opaque { xml, .. } = &mut marker.content else {
            panic!()
        };
        *xml = xml.replace("<pageBreak/>", "<pageBreak invalid='1'/>");
        let input = app.prepare_calculation_input().unwrap();
        assert!(input.faults.contains_key(&id));
    }
}
