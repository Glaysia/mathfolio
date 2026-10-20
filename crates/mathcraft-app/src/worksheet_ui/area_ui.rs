//! Opaque Area XML owns local child regions. Draft editors never become flat
//! CanvasRegions; only explicit commit patches the original child expression.
use super::*;
use mathcraft_document::area::{self, NativeArea, NativeAreaMath};

pub(super) fn child_key(path: &[usize]) -> String {
    serde_json::to_string(path).expect("integer region path is serializable")
}

#[derive(Clone, Copy, Default)]
pub(super) struct AreaChildView<'a> {
    pub display: Option<&'a crate::result_display::FormattedResult>,
    pub labelled: Option<&'a PortableTree>,
    pub error: Option<&'a str>,
    pub pending: bool,
    pub native: Option<&'a mathcraft_document::native_visual::RegionVisual>,
    pub local: Option<[f64; 2]>,
    pub kind: Option<&'a str>,
    pub font_points: Option<f32>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct AreaMutation {
    pub document_changed: bool,
    pub calculation_changed: bool,
}

pub(super) struct AreaResponse {
    pub response: egui::Response,
    pub document_changed: bool,
    pub calculation_changed: bool,
    pub draft_changed: bool,
    /// Visible typing/creation/control changes; committing the same visible
    /// draft into XML on Enter/focus change is not an additional Undo command.
    pub history_changed: bool,
    pub evaluation_requested: Option<String>,
    pub commit_requested: bool,
    pub focused: bool,
    /// Newly installed source placeholder; root groups it with first input in
    /// one owner-history command, distinct from a subsequent source commit.
    pub child_created: Option<String>,
    pub error: Option<String>,
}

struct ChildEditor {
    source: NativeAreaMath,
    key: String,
    committed: Option<PortableTree>,
    editor: Option<Editor>,
    changed: bool,
    focus: bool,
    widget: Option<egui::Id>,
    error: Option<String>,
}

pub(super) struct AreaEditor {
    source: String,
    area: NativeArea,
    children: Vec<ChildEditor>,
    // Rebuilt with the exact installed source; vector order remains authoritative.
    children_by_key: HashMap<String, usize>,
    active: Option<String>,
    error: Option<String>,
    authoring: super::area_authoring::AreaAuthoring,
    display_height: f32,
    input_height: f32,
}

impl AreaEditor {
    pub(super) fn new(xml: &str) -> Result<Self, String> {
        let area = Self::metadata(xml)?;
        let mut state = Self {
            source: String::new(),
            area,
            children: vec![],
            children_by_key: HashMap::new(),
            active: None,
            error: None,
            authoring: Default::default(),
            display_height: 30.,
            input_height: 30.,
        };
        state.install(xml)?;
        Ok(state)
    }

    fn metadata(xml: &str) -> Result<NativeArea, String> {
        area::areas_from_xml(xml)?
            .into_iter()
            .find(|a| a.region_path.is_empty())
            .ok_or_else(|| "Area editor requires its namespace-complete owning region".into())
    }

    fn install(&mut self, xml: &str) -> Result<(), String> {
        let area = Self::metadata(xml)?;
        let display_height = super::area_authoring::expanded_height(xml)?;
        let sources = area::area_math_children_xml(xml, &[])?;
        let mut previous = std::mem::take(&mut self.children);
        let mut children = Vec::with_capacity(sources.len());
        for source in sources {
            let key = child_key(&source.region_path);
            let parsed = worksheet_math::tree_from_mathml(&source.mathml);
            let current = previous
                .iter()
                .position(|old| old.key == key)
                .map(|index| previous.swap_remove(index));
            let child = if let Some(mut old) = current
                && parsed.as_ref().ok().is_some_and(|tree| {
                    old.committed.as_ref() == Some(tree)
                        || old
                            .editor
                            .as_ref()
                            .and_then(|e| e.to_portable().ok())
                            .as_ref()
                            == Some(tree)
                }) {
                let became_committed = old
                    .editor
                    .as_ref()
                    .and_then(|e| e.to_portable().ok())
                    .as_ref()
                    == parsed.as_ref().ok();
                old.source = source;
                old.committed = parsed.ok();
                if became_committed {
                    old.changed = false;
                }
                old
            } else {
                match parsed.and_then(|tree| {
                    Editor::from_portable(&tree)
                        .map(|editor| (tree, editor))
                        .map_err(|e| e.to_string())
                }) {
                    Ok((tree, editor)) => ChildEditor {
                        source,
                        key,
                        committed: Some(tree),
                        editor: Some(editor),
                        changed: false,
                        focus: false,
                        widget: None,
                        error: None,
                    },
                    Err(error) => ChildEditor {
                        source,
                        key,
                        committed: None,
                        editor: None,
                        changed: false,
                        focus: false,
                        widget: None,
                        error: Some(error),
                    },
                }
            };
            children.push(child);
        }
        let children_by_key = children
            .iter()
            .enumerate()
            .map(|(index, child)| (child.key.clone(), index))
            .collect();
        self.area = area;
        self.children = children;
        self.children_by_key = children_by_key;
        self.source = xml.into();
        self.display_height = display_height;
        self.input_height = display_height;
        if self.area.protected
            || self.area.is_collapsed
            || !self
                .children
                .iter()
                .any(|c| Some(&c.key) == self.active.as_ref())
        {
            self.active = None;
            for child in &mut self.children {
                child.focus = false;
            }
        }
        Ok(())
    }

    pub(super) fn activate(&mut self, key: &str) -> Result<(), String> {
        if self.area.protected {
            return Err("Protected Area child changes are unsupported".into());
        }
        if self.area.is_collapsed {
            return Err("Collapsed Area children are not visible".into());
        }
        let child = self
            .children
            .iter_mut()
            .find(|c| c.key == key && c.editor.is_some())
            .ok_or("Unsupported or missing Area math child")?;
        child.focus = true;
        self.active = Some(key.into());
        Ok(())
    }

    pub(super) fn activate_insertion(&mut self) -> Result<(), String> {
        if self.area.protected || self.area.is_collapsed {
            return Err("Area insertion is not available".into());
        }
        self.active = None;
        self.authoring.request_focus();
        Ok(())
    }

    pub(super) fn set_input_grid(&mut self, grid: f64) -> Result<(), String> {
        self.authoring.set_grid(grid)
    }

    /// Set the blank container's local anchor. The caller separately commits
    /// any prior child and calls activate_insertion to switch editing focus.
    pub(super) fn set_insertion_point(&mut self, local: [f64; 2]) -> Result<(), String> {
        if self.area.protected || self.area.is_collapsed {
            return Err("Area insertion is not available".into());
        }
        self.authoring.click(local, self.input_height as f64)
    }

    /// Install a real placeholder child before the first formula input. Root's
    /// owner history groups this source change with that first character.
    pub(super) fn begin_math_at_insertion(&mut self, xml: &mut String) -> Result<String, String> {
        if *xml != self.source {
            return Err("Area source changed before child creation".into());
        }
        let candidate = self.authoring.prepare_math(xml)?;
        let key = child_key(&candidate.region_path);
        self.install(&candidate.xml)?;
        self.activate(&key)?;
        *xml = candidate.xml;
        Ok(key)
    }
    #[cfg(test)]
    pub(super) fn lookup_cost_index_snapshot(&self) -> (&HashMap<String, usize>, Vec<&str>) {
        (
            &self.children_by_key,
            self.children
                .iter()
                .map(|child| child.key.as_str())
                .collect(),
        )
    }

    pub(super) fn math_sources(&self) -> impl Iterator<Item = &NativeAreaMath> {
        self.children.iter().map(|child| &child.source)
    }
    pub(super) fn active_child(&self) -> Option<&str> {
        self.active.as_deref()
    }
    pub(super) fn has_draft_changes(&self) -> bool {
        self.children.iter().any(|c| c.changed)
    }
    pub(super) fn active_editor(&self) -> Option<&Editor> {
        self.children
            .iter()
            .find(|c| Some(&c.key) == self.active.as_ref())?
            .editor
            .as_ref()
    }
    pub(super) fn active_editor_mut(&mut self) -> Option<&mut Editor> {
        self.children
            .iter_mut()
            .find(|c| Some(&c.key) == self.active.as_ref())?
            .editor
            .as_mut()
    }
    pub(super) fn mark_active_changed(&mut self) {
        if let Some(child) = self
            .children
            .iter_mut()
            .find(|c| Some(&c.key) == self.active.as_ref())
        {
            child.changed = child
                .editor
                .as_ref()
                .and_then(|e| e.to_portable().ok())
                .as_ref()
                != child.committed.as_ref();
        }
    }
    pub(super) fn active_snapshot(&self) -> Option<(String, Editor)> {
        Some((
            self.active.clone()?,
            self.active_editor()?.clone_without_history(),
        ))
    }
    pub(super) fn restore_active_snapshot(
        &mut self,
        key: &str,
        editor: Editor,
    ) -> Result<(), String> {
        if self.area.protected {
            return Err("Protected Area child changes are unsupported".into());
        }
        if self.area.is_collapsed {
            return Err("Collapsed Area children are not visible".into());
        }
        editor.to_portable().map_err(|e| e.to_string())?;
        let child = self
            .children
            .iter_mut()
            .find(|c| c.key == key && c.editor.is_some())
            .ok_or("Missing Area math child")?;
        child.changed = child.committed.as_ref() != editor.to_portable().ok().as_ref();
        child.editor = Some(editor);
        child.focus = true;
        self.active = Some(key.into());
        Ok(())
    }
    pub(super) fn deactivate(&mut self) {
        self.active = None;
    }

    fn patched_drafts(&self, xml: &str, only: Option<&str>) -> Result<String, String> {
        if xml != self.source {
            return Err("Area source changed outside its draft editor".into());
        }
        let mut candidate = xml.to_owned();
        for child in self
            .children
            .iter()
            .filter(|c| c.changed && only.is_none_or(|key| key == c.key))
        {
            if self.area.protected {
                return Err("Protected Area child changes are unsupported".into());
            }
            let tree = child
                .editor
                .as_ref()
                .ok_or("Unsupported Area child")?
                .to_portable()
                .map_err(|e| e.to_string())?;
            let mathml = worksheet_math::tree_to_mathml(&tree)?;
            if let Some(changed) =
                area::replace_area_math_xml(&candidate, &[], &child.source.region_path, &mathml)?
            {
                candidate = changed;
            }
        }
        Ok(candidate)
    }
    fn commit_only(&mut self, xml: &mut String, only: Option<&str>) -> Result<bool, String> {
        let candidate = self.patched_drafts(xml, only)?;
        let changed = candidate != *xml;
        // install validates the whole fragment before touching its prior state.
        self.install(&candidate)?;
        *xml = candidate;
        self.error = None;
        Ok(changed)
    }
    /// Validate a multi-Area controller transaction without cloning editors or
    /// installing any source. The caller can preview every owner before commit.
    pub(super) fn preview_commit(&self, xml: &str) -> Result<Option<String>, String> {
        let candidate = self.patched_drafts(xml, None)?;
        if candidate == xml {
            return Ok(None);
        }
        Self::metadata(&candidate)?;
        area::area_math_children_xml(&candidate, &[])?;
        Ok(Some(candidate))
    }
    pub(super) fn commit(&mut self, xml: &mut String) -> Result<bool, String> {
        self.commit_only(xml, None)
    }

    pub(super) fn set_display(
        &mut self,
        xml: &mut String,
        collapsed: Option<bool>,
        border: Option<bool>,
    ) -> Result<AreaMutation, String> {
        let candidate = self.patched_drafts(xml, None)?;
        let calculation_changed = candidate != *xml;
        let candidate =
            area::set_area_display_xml(&candidate, &[], collapsed, border)?.unwrap_or(candidate);
        let document_changed = candidate != *xml;
        self.install(&candidate)?;
        *xml = candidate;
        self.error = None;
        Ok(AreaMutation {
            document_changed,
            calculation_changed,
        })
    }
    pub(super) fn toggle_calculation(&mut self, xml: &mut String) -> Result<AreaMutation, String> {
        let candidate = self.patched_drafts(xml, None)?;
        let candidate = area::toggle_area_calculation_xml(&candidate, &[])?.unwrap_or(candidate);
        let changed = candidate != *xml;
        self.install(&candidate)?;
        *xml = candidate;
        self.error = None;
        Ok(AreaMutation {
            document_changed: changed,
            calculation_changed: changed,
        })
    }

    pub(super) fn show(
        &mut self,
        ui: &mut egui::Ui,
        xml: &mut String,
        child_views: &HashMap<String, AreaChildView<'_>>,
        font_points: f32,
    ) -> AreaResponse {
        if *xml != self.source
            && let Err(error) = self.install(xml)
        {
            self.error = Some(error);
        }
        let top = ui.cursor().min;
        let width = ui.available_width().max(80.);
        let mut height = if self.area.is_collapsed {
            22.
        } else {
            self.display_height
        };
        let mut geometry = HashMap::with_capacity(self.children.len());
        if !self.area.is_collapsed {
            for child in &self.children {
                if let Some(editor) = &child.editor {
                    let view = child_views.get(&child.key).copied().unwrap_or_default();
                    let child_font_points = view.font_points.unwrap_or(font_points);
                    let (size, baseline) = formula::measured_geometry_retained_styled_for_id(
                        ui,
                        ui.id().with(("area-child", &child.key)),
                        editor,
                        view.display.map(formula::FormulaResult::Formatted),
                        view.labelled,
                        child_font_points,
                    );
                    height = height.max(child.source.top as f32 - baseline + size.y + 4.);
                    geometry.insert(child.key.clone(), (size, baseline));
                }
            }
            for (key, view) in child_views {
                if self.children_by_key.contains_key(key) {
                    continue;
                }
                if let (Some([_, y]), Some(native)) = (view.local, view.native) {
                    height = height.max(y as f32 + native.size[1] + 4.);
                }
            }
        }
        if !self.area.is_collapsed {
            self.input_height = height;
        }
        let (rect, mut response) =
            ui.allocate_exact_size(vec2(width, height.max(22.)), Sense::click());
        if self.authoring.take_focus_request() {
            response.request_focus();
        }
        let container_clicked = response.clicked();
        let container_pointer = response.interact_pointer_pos();
        let mut occupied = Vec::new();
        let mut mutation = AreaMutation::default();
        let mut draft_changed = false;
        let mut evaluation_requested = None;
        let mut commit_requested = false;
        let mut focused = response.has_focus() && !self.area.is_collapsed && !self.area.protected;
        let mut child_created = None;
        let mut active_geometry = None;
        let mut clicked = None;
        let previous = self.active.clone();
        if self.area.is_collapsed {
            for child in &self.children {
                if let Some(widget) = child.widget {
                    ui.memory_mut(|memory| memory.surrender_focus(widget));
                }
            }
        }
        let collapsed_error = self.area.is_collapsed
            && self.children.iter().any(|child| {
                child_views
                    .get(&child.key)
                    .is_some_and(|view| view.error.is_some())
            });
        if self.area.is_border_visible || collapsed_error {
            ui.painter().hline(
                rect.x_range(),
                rect.top() + 10.,
                Stroke::new(0.8, if collapsed_error { RED } else { Color32::GRAY }),
            );
            if !self.area.is_collapsed {
                ui.painter().hline(
                    rect.x_range(),
                    rect.bottom() - 1.,
                    Stroke::new(0.8, Color32::GRAY),
                );
            }
        }
        let toggle_rect = Rect::from_min_size(top, vec2(18., 20.));
        let toggle = ui
            .add_enabled_ui(!self.area.protected, |ui| {
                ui.put(
                    toggle_rect,
                    egui::Button::new(if self.area.is_collapsed { "+" } else { "−" }).frame(false),
                )
            })
            .inner;
        if toggle.clicked() {
            match self.set_display(xml, Some(!self.area.is_collapsed), None) {
                Ok(m) => mutation = m,
                Err(error) => self.error = Some(error),
            }
        }
        response = response.union(toggle);
        let mut display_command = None;
        let mut calculation_command = false;
        response.context_menu(|ui| {
            ui.add_enabled_ui(!self.area.protected, |ui| {
                if ui
                    .button(if self.area.is_collapsed {
                        "영역 펼치기"
                    } else {
                        "영역 접기"
                    })
                    .clicked()
                {
                    display_command = Some((Some(!self.area.is_collapsed), None));
                    ui.close();
                }
                if ui
                    .button(if self.area.is_border_visible {
                        "테두리 숨기기"
                    } else {
                        "테두리 표시"
                    })
                    .clicked()
                {
                    display_command = Some((None, Some(!self.area.is_border_visible)));
                    ui.close();
                }
                if ui.button("영역 계산 전환").clicked() {
                    calculation_command = true;
                    ui.close();
                }
            });
        });
        if let Some((collapsed, border)) = display_command {
            match self.set_display(xml, collapsed, border) {
                Ok(m) => mutation = m,
                Err(error) => self.error = Some(error),
            }
        }
        if calculation_command {
            match self.toggle_calculation(xml) {
                Ok(m) => mutation = m,
                Err(error) => self.error = Some(error),
            }
        }
        let controls_changed = mutation.document_changed;
        if !self.area.is_collapsed
            && !self.area.protected
            && self.active.is_none()
            && response.has_focus()
            && !ui.ctx().any_popup_open()
            && ui.input(|input| {
                !input.modifiers.ctrl
                    && !input.modifiers.command
                    && input.events.iter().any(|event| match event {
                        egui::Event::Text(text)
                        | egui::Event::Paste(text)
                        | egui::Event::Ime(egui::ImeEvent::Commit(text)) => {
                            text.chars().any(|character| !character.is_control())
                        }
                        _ => false,
                    })
            })
        {
            match self.begin_math_at_insertion(xml) {
                Ok(key) => {
                    mutation.document_changed = true;
                    child_created = Some(key);
                }
                Err(error) => self.error = Some(error),
            }
        }
        if !self.area.is_collapsed {
            for path in &self.area.child_region_paths {
                let key = child_key(path);
                let view = child_views.get(&key).copied().unwrap_or_default();
                let math = self
                    .children_by_key
                    .get(&key)
                    .and_then(|&index| self.children.get(index));
                if math.is_some_and(|c| c.editor.is_some()) {
                    continue;
                }
                let local = view
                    .local
                    .or_else(|| math.map(|c| [c.source.left, c.source.top]));
                let location = local.map_or(rect.min + vec2(22., height - 22.), |[x, y]| {
                    rect.min + vec2(x as f32, y as f32)
                });
                let mut inner = ui.new_child(
                    UiBuilder::new()
                        .id_salt(("area-inert-child", &key))
                        .max_rect(Rect::from_min_size(
                            location,
                            vec2((width - (location.x - rect.left())).max(80.), 1000.),
                        ))
                        .layout(egui::Layout::left_to_right(egui::Align::Min)),
                );
                inner.set_clip_rect(ui.clip_rect());
                if let Some(native) = view.native {
                    let child_response =
                        crate::native_presentation::draw(&mut inner, native, width);
                    occupied.push(child_response.rect);
                    response = response.union(child_response);
                } else {
                    let label = inner.colored_label(
                        AMBER,
                        format!(
                            "지원하지 않는 하위 영역 ({})",
                            view.kind.unwrap_or("unknown")
                        ),
                    );
                    let error = view
                        .error
                        .or_else(|| math.and_then(|c| c.error.as_deref()))
                        .unwrap_or("원본 XML은 보존됩니다.");
                    occupied.push(label.rect);
                    response = response.union(label.on_hover_text(error));
                }
            }
            for child in &mut self.children {
                let view = child_views.get(&child.key).copied().unwrap_or_default();
                let child_font_points = view.font_points.unwrap_or(font_points);
                let Some(editor) = &mut child.editor else {
                    continue;
                };
                let display = view.display.map(formula::FormulaResult::Formatted);
                let child_id = ui.id().with(("area-child", &child.key));
                // A collapsed Area can be expanded by its control in this same
                // frame; only those newly visible children need a late measure.
                let (size, baseline) = geometry.get(&child.key).copied().unwrap_or_else(|| {
                    formula::measured_geometry_retained_styled_for_id(
                        ui,
                        child_id,
                        editor,
                        display,
                        view.labelled,
                        child_font_points,
                    )
                });
                let origin =
                    rect.min + vec2(child.source.left as f32, child.source.top as f32 - baseline);
                let mut inner = ui.new_child(
                    UiBuilder::new()
                        .id(child_id)
                        .max_rect(Rect::from_min_size(
                            origin,
                            vec2((width - child.source.left as f32).max(100.), 1000.),
                        ))
                        .layout(egui::Layout::left_to_right(egui::Align::Min)),
                );
                inner.set_clip_rect(ui.clip_rect());
                if view.pending || child.source.calculation_disabled {
                    inner.set_opacity(0.45);
                }
                if let Some([r, g, b, a]) = view.native.and_then(|visual| visual.background) {
                    inner.painter().rect_filled(
                        Rect::from_min_size(origin, size),
                        0.,
                        Color32::from_rgba_unmultiplied(r, g, b, a),
                    );
                }
                if self.area.protected {
                    inner.disable();
                }
                let outcome = formula::formula_ui_styled(
                    &mut inner,
                    editor,
                    display,
                    child.focus,
                    view.labelled,
                    child_font_points,
                );
                occupied.push(outcome.response.rect);
                child.focus = false;
                child.widget = Some(outcome.response.id);
                let changed = outcome.response.changed();
                child.changed |= changed;
                draft_changed |= changed;
                if outcome.response.has_focus() {
                    focused = true;
                    self.active = Some(child.key.clone());
                    active_geometry = Some((
                        child.source.left,
                        (outcome.response.rect.bottom() - rect.top()) as f64,
                    ));
                }
                if outcome.response.clicked() {
                    clicked = Some(child.key.clone());
                }
                if outcome.evaluation_requested {
                    evaluation_requested = Some(child.key.clone());
                    commit_requested = true;
                }
                if let Some(error) = view.error.or(child.error.as_deref()) {
                    response = response.on_hover_text(error);
                }
                response = response.union(outcome.response);
            }
        }
        if let Some(key) = clicked {
            self.active = Some(key);
        } else if container_clicked
            && let Some(pointer) = container_pointer
            && !toggle_rect.contains(pointer)
            && !occupied.iter().any(|rect| rect.contains(pointer))
            && !self.area.is_collapsed
            && !self.area.protected
        {
            match self.set_insertion_point([
                (pointer.x - rect.left()) as f64,
                (pointer.y - rect.top()) as f64,
            ]) {
                Ok(()) => {
                    self.active = None;
                    focused = true;
                    response.request_focus();
                }
                Err(error) => self.error = Some(error),
            }
        }
        if previous != self.active
            && let Some(old) = previous
        {
            match self.commit_only(xml, Some(&old)) {
                Ok(changed) => {
                    mutation.document_changed |= changed;
                    mutation.calculation_changed |= changed;
                }
                Err(error) => self.error = Some(error),
            }
        }
        let ordinary_enter = focused
            && ui.input_mut(|input| {
                consume_exact_key(input, egui::Modifiers::NONE, egui::Key::Enter)
            });
        if ordinary_enter {
            commit_requested = true;
        }
        if commit_requested {
            match self.commit(xml) {
                Ok(changed) => {
                    mutation.document_changed |= changed;
                    mutation.calculation_changed |= changed;
                    if ordinary_enter && let Some((left, bottom)) = active_geometry {
                        match self
                            .authoring
                            .advance_after_math(left, bottom, height as f64)
                        {
                            Ok(()) => {
                                self.active = None;
                                response.request_focus();
                                focused = true;
                            }
                            Err(error) => self.error = Some(error),
                        }
                    }
                }
                Err(error) => {
                    self.error = Some(error);
                    evaluation_requested = None;
                    commit_requested = false;
                }
            }
        }
        if mutation.document_changed || draft_changed {
            response.mark_changed();
        }
        AreaResponse {
            response,
            document_changed: mutation.document_changed,
            calculation_changed: mutation.calculation_changed,
            draft_changed,
            history_changed: draft_changed || child_created.is_some() || controls_changed,
            evaluation_requested,
            commit_requested,
            focused,
            child_created,
            error: self.error.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn number(value: &str) -> PortableTree {
        PortableTree {
            kind: NodeKind::Number(value.into()),
            children: vec![],
        }
    }
    fn xml(protected: bool, query: bool) -> String {
        let first = if query {
            PortableTree {
                kind: NodeKind::Binary(BinaryOp::Evaluate),
                children: vec![
                    number("12"),
                    PortableTree {
                        kind: NodeKind::Placeholder,
                        children: vec![],
                    },
                ],
            }
        } else {
            number("12")
        };
        let mathml = worksheet_math::tree_to_mathml(&first).unwrap();
        format!(
            r#"<ws:region xmlns:ws="http://schemas.mathsoft.com/worksheet50" xmlns:ml="http://schemas.mathsoft.com/math50" xmlns:u="urn:future" region-id="10" left="30" top="100"><ws:Area AreaHeight="150" IsCollapsed="false" IsBorderVisible="true" u:keep="yes">{}<!--preserved--><ws:regions><ws:region region-id="11" left="30" top="35"><ws:math resultRef="r11">{mathml}</ws:math></ws:region><ws:region region-id="12" left="80" top="85"><ws:math resultRef="r12" disable-calc="true"><ml:real>34</ml:real></ws:math></ws:region><ws:region region-id="13" left="140" top="110"><ws:text><u:future/></ws:text></ws:region></ws:regions></ws:Area></ws:region>"#,
            if protected {
                "<ws:ProtectionData/>"
            } else {
                ""
            }
        )
    }
    fn key(key: egui::Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }
    }
    fn context() -> egui::Context {
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        ctx
    }
    fn frame(
        ctx: &egui::Context,
        area: &mut AreaEditor,
        xml: &mut String,
        views: &HashMap<String, AreaChildView<'_>>,
        events: Vec<egui::Event>,
    ) -> AreaResponse {
        let mut response = None;
        ctx.run_ui(
            egui::RawInput {
                events,
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(600., 400.))),
                ..Default::default()
            },
            |ui| {
                response = Some(area.show(ui, xml, views, 11.));
            },
        )
        .drop_without_applying_deltas();
        response.unwrap()
    }

    #[test]
    fn actual_typing_retains_source_and_saved_display_until_enter_then_patches_only_one_child() {
        let mut source = xml(false, false);
        let original = source.clone();
        let mut area = AreaEditor::new(&source).unwrap();
        let child = area.children[0].key.clone();
        assert_eq!(child, "[0,0,0]");
        area.activate(&child).unwrap();
        let editor = area.active_editor_mut().unwrap();
        editor.place_text_caret(editor.root(), 2).unwrap();
        let saved = crate::result_display::format(
            &Value::Scalar {
                quantity: mathcraft_core::Quantity::scalar(12.),
            },
            &MathFormat::default(),
        );
        let views = HashMap::from([(
            child.clone(),
            AreaChildView {
                display: Some(&saved),
                ..Default::default()
            },
        )]);
        let ctx = context();
        let idle = frame(&ctx, &mut area, &mut source, &views, vec![]);
        assert!(!idle.document_changed && !idle.calculation_changed && !idle.draft_changed);
        let edited = frame(
            &ctx,
            &mut area,
            &mut source,
            &views,
            vec![egui::Event::Text("3".into())],
        );
        assert!(edited.draft_changed && edited.focused);
        assert!(
            !edited.document_changed && !edited.calculation_changed && !edited.commit_requested
        );
        assert_eq!(source, original);
        assert!(area.has_draft_changes());
        assert_eq!(
            area.active_editor().unwrap().to_portable().unwrap(),
            number("123")
        );
        let committed = frame(
            &ctx,
            &mut area,
            &mut source,
            &views,
            vec![key(egui::Key::Enter)],
        );
        assert!(
            committed.document_changed
                && committed.calculation_changed
                && committed.commit_requested
        );
        assert_eq!(area.active_child(), None);
        assert!(!area.has_draft_changes());
        let children = area::area_math_children_xml(&source, &[]).unwrap();
        assert_eq!(
            worksheet_math::tree_from_mathml(&children[0].mathml).unwrap(),
            number("123")
        );
        assert_eq!(children[0].result_ref, None);
        assert_eq!(children[1].result_ref.as_deref(), Some("r12"));
        assert_eq!((children[0].left, children[0].top), (30., 35.));
        assert!(
            source.contains("<!--preserved-->")
                && source.contains("u:keep=\"yes\"")
                && source.contains("<u:future/>")
                && source.contains("region-id=\"13\"")
        );
        area.activate(&child).unwrap();
        let editor = area.active_editor_mut().unwrap();
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), number("12"));
        area.mark_active_changed();
        assert!(area.has_draft_changes());
        assert!(area.commit(&mut source).unwrap());
        assert_eq!(
            worksheet_math::tree_from_mathml(
                &area::area_math_children_xml(&source, &[]).unwrap()[0].mathml
            )
            .unwrap(),
            number("12")
        );
    }

    #[test]
    fn preview_of_multiple_area_drafts_never_installs_an_earlier_owner_on_later_failure() {
        let original = xml(false, false);
        let mut first = AreaEditor::new(&original).unwrap();
        let mut second = AreaEditor::new(&original).unwrap();
        for area in [&mut first, &mut second] {
            let key = area.children[0].key.clone();
            area.activate(&key).unwrap();
            let e = area.active_editor_mut().unwrap();
            e.set_atom(e.root(), NodeKind::Number("123".into()))
                .unwrap();
            e.set_atom(e.root(), NodeKind::Number("124".into()))
                .unwrap();
            assert!(e.undo());
            area.mark_active_changed();
        }
        let first_caret = first.active_editor().unwrap().caret();
        let candidate = first
            .preview_commit(&original)
            .unwrap()
            .expect("first candidate is valid");
        assert_ne!(candidate, original);
        let stale = original.replace("AreaHeight=\"150\"", "AreaHeight=\"151\"");
        assert!(second.preview_commit(&stale).is_err());
        assert_eq!(first.source, original);
        assert_eq!(second.source, original);
        assert!(first.has_draft_changes() && second.has_draft_changes());
        assert_eq!(first.active_editor().unwrap().caret(), first_caret);
        for area in [&mut first, &mut second] {
            assert!(area.active_editor_mut().unwrap().redo());
            assert_eq!(
                area.active_editor().unwrap().to_portable().unwrap(),
                number("124")
            );
        }
        let protected = xml(true, false);
        let area = AreaEditor::new(&protected).unwrap();
        assert_eq!(area.preview_commit(&protected).unwrap(), None);
    }
    #[test]
    fn display_changes_preserve_caches_and_bulk_toggle_updates_children_independently() {
        let mut source = xml(false, false);
        let mut area = AreaEditor::new(&source).unwrap();
        let m = area
            .set_display(&mut source, Some(true), Some(false))
            .unwrap();
        assert_eq!(
            m,
            AreaMutation {
                document_changed: true,
                calculation_changed: false
            }
        );
        assert!(source.contains("resultRef=\"r11\"") && source.contains("resultRef=\"r12\""));
        assert!(area.area.is_collapsed && !area.area.is_border_visible);
        assert_eq!(
            area.set_display(&mut source, Some(true), Some(false))
                .unwrap(),
            AreaMutation::default()
        );
        assert_eq!(
            area.toggle_calculation(&mut source).unwrap(),
            AreaMutation {
                document_changed: true,
                calculation_changed: true
            }
        );
        assert!(
            area::area_math_children_xml(&source, &[])
                .unwrap()
                .iter()
                .all(|c| c.calculation_disabled)
        );
        assert!(
            area.toggle_calculation(&mut source)
                .unwrap()
                .calculation_changed
        );
        assert!(
            area::area_math_children_xml(&source, &[])
                .unwrap()
                .iter()
                .all(|c| !c.calculation_disabled)
        );
        assert!(area.area.is_collapsed && !area.area.is_border_visible);
        assert!(source.contains("resultRef=\"r11\"") && source.contains("resultRef=\"r12\""));
    }

    #[test]
    fn protected_or_stale_source_rejection_preserves_xml_draft_caret_and_redo() {
        let mut protected = xml(true, false);
        let before = protected.clone();
        let mut area = AreaEditor::new(&protected).unwrap();
        let key = area.children[0].key.clone();
        assert!(area.activate(&key).is_err());
        assert!(area.set_display(&mut protected, Some(true), None).is_err());
        assert!(area.toggle_calculation(&mut protected).is_err());
        assert_eq!(protected, before);
        let mut source = xml(false, false);
        let mut area = AreaEditor::new(&source).unwrap();
        let key = area.children[0].key.clone();
        area.activate(&key).unwrap();
        let e = area.active_editor_mut().unwrap();
        let root = e.root();
        e.set_atom(root, NodeKind::Number("123".into())).unwrap();
        e.set_atom(root, NodeKind::Number("124".into())).unwrap();
        assert!(e.undo());
        let caret = e.caret();
        area.mark_active_changed();
        source = source.replace("AreaHeight=\"150\"", "AreaHeight=\"151\"");
        let stale = source.clone();
        assert!(area.commit(&mut source).is_err());
        assert_eq!(source, stale);
        assert_eq!(
            area.active_editor().unwrap().to_portable().unwrap(),
            number("123")
        );
        assert_eq!(area.active_editor().unwrap().caret(), caret);
        assert!(area.active_editor_mut().unwrap().redo());
        assert_eq!(
            area.active_editor().unwrap().to_portable().unwrap(),
            number("124")
        );
    }

    #[test]
    fn child_snapshot_restores_only_active_draft_and_source_sync_handles_external_undo() {
        let mut source = xml(false, false);
        let original = source.clone();
        let mut area = AreaEditor::new(&source).unwrap();
        let key = area.children[0].key.clone();
        area.activate(&key).unwrap();
        let e = area.active_editor_mut().unwrap();
        e.set_atom(e.root(), NodeKind::Number("123".into()))
            .unwrap();
        area.mark_active_changed();
        let mut snapshot = area.active_snapshot().unwrap();
        assert!(
            !snapshot.1.undo(),
            "worksheet snapshots must not recursively retain per-key history"
        );
        assert!(area.commit(&mut source).unwrap());
        area.deactivate();
        area.install(&original).unwrap();
        area.restore_active_snapshot(&snapshot.0, snapshot.1)
            .unwrap();
        assert_eq!(area.active_child(), Some(key.as_str()));
        assert!(area.has_draft_changes());
        assert_eq!(area.children[1].committed, Some(number("34")));
        assert_eq!(
            area.active_editor().unwrap().to_portable().unwrap(),
            number("123")
        );
    }

    #[test]
    fn actual_local_child_pointer_focus_commits_previous_draft_without_flattening_or_touching_sibling()
     {
        let mut source = xml(false, false);
        let mut area = AreaEditor::new(&source).unwrap();
        let first = area.children[0].key.clone();
        let second = area.children[1].key.clone();
        area.activate(&first).unwrap();
        let e = area.active_editor_mut().unwrap();
        e.place_text_caret(e.root(), 2).unwrap();
        let ctx = context();
        let initial = frame(&ctx, &mut area, &mut source, &HashMap::new(), vec![]);
        frame(
            &ctx,
            &mut area,
            &mut source,
            &HashMap::new(),
            vec![egui::Event::Text("3".into())],
        );
        let point = initial.response.rect.min + vec2(86., 81.);
        let down = frame(
            &ctx,
            &mut area,
            &mut source,
            &HashMap::new(),
            vec![
                egui::Event::PointerMoved(point),
                egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        let up = frame(
            &ctx,
            &mut area,
            &mut source,
            &HashMap::new(),
            vec![egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(down.document_changed || up.document_changed);
        assert_eq!(area.active_child(), Some(second.as_str()));
        assert_eq!(area.children.len(), 2);
        let children = area::area_math_children_xml(&source, &[]).unwrap();
        assert_eq!(
            worksheet_math::tree_from_mathml(&children[0].mathml).unwrap(),
            number("123")
        );
        assert_eq!(children[1].result_ref.as_deref(), Some("r12"));
        assert!(source.contains("region-id=\"13\"") && source.contains("<u:future/>"));
    }

    #[test]
    fn supplied_saved_child_display_and_readonly_local_native_sibling_are_painted_without_source_change()
     {
        let mut source = xml(false, true);
        let original = source.clone();
        let mut area = AreaEditor::new(&source).unwrap();
        let ctx = context();
        let first = area.children[0].key.clone();
        let other = child_key(&area.area.child_region_paths[2]);
        let saved = crate::result_display::FormattedResult {
            rows: 1,
            cols: 1,
            cells: vec![crate::result_display::DisplayMath::Text(
                "9".repeat(180),
                crate::typography::MathStyle::Operator,
            )],
            brackets: false,
        };
        let native = mathcraft_document::native_visual::RegionVisual {
            size: [80., 25.],
            background: Some([230, 240, 250, 255]),
            document: None,
        };
        let views = HashMap::from([
            (
                first,
                AreaChildView {
                    display: Some(&saved),
                    ..Default::default()
                },
            ),
            (
                other,
                AreaChildView {
                    native: Some(&native),
                    local: Some([140., 170.]),
                    kind: Some("text"),
                    ..Default::default()
                },
            ),
        ]);
        let response = frame(&ctx, &mut area, &mut source, &views, vec![]);
        assert!(response.response.rect.height() >= 199.);
        assert!(
            response.response.rect.width() > 600.,
            "saved long result contributes to actual formula geometry"
        );
        assert!(
            !response.document_changed && !response.calculation_changed && !response.draft_changed
        );
        assert!(response.error.is_none());
        assert_eq!(source, original);
    }

    #[test]
    fn child_native_background_and_font_use_the_same_local_baseline_geometry_as_formula() {
        let mut source = xml(false, false);
        let original = source.clone();
        let mut area = AreaEditor::new(&source).unwrap();
        let key = area.children[0].key.clone();
        let background = Color32::from_rgb(240, 230, 110);
        let native = mathcraft_document::native_visual::RegionVisual {
            size: [0., 0.],
            background: Some([240, 230, 110, 255]),
            document: None,
        };
        let views = HashMap::from([(
            key,
            AreaChildView {
                native: Some(&native),
                font_points: Some(22.),
                ..Default::default()
            },
        )]);
        let ctx = context();
        let mut expected = None;
        let mut actual = None;
        let output = ctx.run_ui(Default::default(), |ui| {
            let (size, baseline) = formula::measured_geometry_retained_styled_for_id(
                ui,
                ui.id(),
                area.children[0].editor.as_ref().unwrap(),
                None,
                None,
                22.,
            );
            let origin = ui.cursor().min;
            expected = Some(Rect::from_min_size(
                origin + vec2(30., 35. - baseline),
                size,
            ));
            actual = Some(area.show(ui, &mut source, &views, 11.));
        });
        fn background_rect(shape: &egui::epaint::Shape, color: Color32) -> Option<Rect> {
            match shape {
                egui::epaint::Shape::Rect(rect) if rect.fill == color => Some(rect.rect),
                egui::epaint::Shape::Vec(shapes) => {
                    shapes.iter().find_map(|s| background_rect(s, color))
                }
                _ => None,
            }
        }
        let drawn = output
            .shapes
            .iter()
            .find_map(|s| background_rect(&s.shape, background))
            .expect("source native background is painted");
        assert_eq!(drawn, expected.unwrap());
        let response = actual.unwrap();
        assert!(
            !response.document_changed && !response.draft_changed && !response.calculation_changed
        );
        assert_eq!(source, original);
        assert_eq!(area.math_sources().count(), 2);
        output.drop_without_applying_deltas();
    }

    #[test]
    fn collapsed_error_line_survives_hidden_border_and_display_commit_keeps_calculation_separate() {
        let mut source = xml(false, false);
        let mut area = AreaEditor::new(&source).unwrap();
        let key = area.children[0].key.clone();
        area.activate(&key).unwrap();
        let e = area.active_editor_mut().unwrap();
        e.set_atom(e.root(), NodeKind::Number("123".into()))
            .unwrap();
        area.mark_active_changed();
        assert_eq!(
            area.set_display(&mut source, Some(true), Some(false))
                .unwrap(),
            AreaMutation {
                document_changed: true,
                calculation_changed: true
            }
        );
        assert_eq!(area.active_child(), None);
        assert!(area.activate(&key).is_err());
        assert!(source.contains("resultRef=\"r12\"") && !source.contains("resultRef=\"r11\""));
        let original = source.clone();
        let views = HashMap::from([(
            key,
            AreaChildView {
                error: Some("division by zero"),
                ..Default::default()
            },
        )]);
        let ctx = context();
        let output = ctx.run_ui(Default::default(), |ui| {
            let response = area.show(ui, &mut source, &views, 11.);
            assert!(
                !response.document_changed
                    && !response.calculation_changed
                    && !response.draft_changed
            );
            assert!(!response.focused);
        });
        assert!(output.shapes.iter().any(
            |s| matches!(&s.shape,egui::epaint::Shape::LineSegment{stroke,..} if stroke.color==RED)
        ));
        assert_eq!(source, original);
        output.drop_without_applying_deltas();
    }

    #[test]
    fn equals_requests_existing_child_evaluation_without_rewriting_xml_or_saved_reference() {
        let mut source = xml(false, true);
        let original = source.clone();
        let mut area = AreaEditor::new(&source).unwrap();
        let key = area.children[0].key.clone();
        area.activate(&key).unwrap();
        let e = area.active_editor_mut().unwrap();
        e.place_caret(e.root(), Direction::Right).unwrap();
        let ctx = context();
        frame(&ctx, &mut area, &mut source, &HashMap::new(), vec![]);
        let response = frame(
            &ctx,
            &mut area,
            &mut source,
            &HashMap::new(),
            vec![egui::Event::Text("=".into())],
        );
        assert_eq!(response.evaluation_requested.as_deref(), Some(key.as_str()));
        assert!(response.commit_requested);
        assert!(
            !response.document_changed && !response.calculation_changed && !response.draft_changed
        );
        assert_eq!(source, original);
    }
}
