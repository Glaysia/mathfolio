//! Keep platform IME preedit out of committed worksheet source and history.
//! The native rich-text carrier is independent of this plain Text path.
use super::*;

pub(super) struct Composition {
    pub id: u64,
    draft: String,
    cursor: egui::text::CCursorRange,
    displaced: String,
    widget: egui::text_edit::TextEditState,
}

pub(super) fn widget_id(id: u64) -> egui::Id {
    egui::Id::new(("worksheet-text", id))
}

pub(super) fn cancel(composition: &mut Option<Composition>, ctx: &egui::Context) {
    if let Some(mut composition) = composition.take() {
        composition.widget.clear_undoer();
        composition.widget.store(ctx, widget_id(composition.id));
    }
}

pub(super) fn reconcile(
    composition: &mut Option<Composition>,
    ctx: &egui::Context,
    selected: Option<u64>,
    focus_new: Option<u64>,
) {
    let interrupted = composition.as_ref().is_some_and(|composition| {
        selected != Some(composition.id)
            || focus_new.is_some_and(|id| id != composition.id)
            || !ctx.memory(|memory| memory.has_focus(widget_id(composition.id)))
            // An ordinary pointer/key edit starts from committed text. Native
            // integrations normally consume composition keys before delivery;
            // a frame carrying IME updates is still handled by TextEdit itself.
            || ctx.input(|input| {
                !input.events.iter().any(|event| matches!(event, egui::Event::Ime(_)))
                    && input.events.iter().any(|event| matches!(event,
                        egui::Event::Key { pressed: true, .. }
                        | egui::Event::Text(_) | egui::Event::Paste(_) | egui::Event::Cut
                        | egui::Event::PointerButton { pressed: true, .. }))
                    || input.events.iter().any(|event| matches!(event, egui::Event::WindowFocused(false)))
            })
    });
    if interrupted {
        cancel(composition, ctx);
    }
}

#[derive(Default)]
struct Delivery {
    starts: bool,
    committed: bool,
    active: bool,
    cancelled: bool,
    // Disabled is a legacy cancellation signal that egui itself ignores.
    remove_preedit: bool,
    surrounding_deleted: bool,
}

impl Delivery {
    #[allow(deprecated)]
    fn read(ui: &egui::Ui, id: egui::Id, was_active: bool) -> Self {
        let mut result = Self {
            active: was_active,
            ..Default::default()
        };
        if !ui.memory(|memory| memory.owns_ime_events(id)) {
            return result;
        }
        ui.input(|input| {
            for event in &input.events {
                match event {
                    egui::Event::Ime(egui::ImeEvent::Preedit { text, .. })
                        if !matches!(text.as_str(), "\n" | "\r") =>
                    {
                        result.starts |= !text.is_empty();
                        result.cancelled = if text.is_empty() {
                            result.cancelled || result.active
                        } else {
                            false
                        };
                        result.active = !text.is_empty();
                        result.remove_preedit = false;
                    }
                    egui::Event::Ime(egui::ImeEvent::Commit(text))
                        if !matches!(text.as_str(), "\n" | "\r") =>
                    {
                        result.cancelled = if text.is_empty() {
                            result.cancelled || result.active
                        } else {
                            false
                        };
                        result.committed |= !text.is_empty();
                        result.active = false;
                        result.remove_preedit = false;
                    }
                    egui::Event::Ime(egui::ImeEvent::Disabled) if result.active => {
                        result.active = false;
                        result.cancelled = true;
                        result.remove_preedit = true;
                    }
                    egui::Event::Ime(egui::ImeEvent::DeleteSurrounding { .. }) => {
                        result.surrounding_deleted = true;
                    }
                    _ => {}
                }
            }
        });
        result
    }
}

fn selection_state(cursor: egui::text::CCursorRange) -> egui::text_edit::TextEditState {
    let mut widget = egui::text_edit::TextEditState::default();
    widget.cursor.set_char_range(Some(cursor));
    widget
}

fn replace_preedit(draft: &str, cursor: egui::text::CCursorRange, displaced: &str) -> String {
    let [start, end] = cursor.sorted_cursors();
    let mut published: String = draft.chars().take(start.index.0).collect();
    published.push_str(displaced);
    published.extend(draft.chars().skip(end.index.0));
    published
}

pub(super) struct Output {
    pub response: egui::Response,
    pub changed: bool,
    pub error: Option<String>,
}

pub(super) struct Presentation<'a> {
    pub style: Option<&'a mathcraft_document::native_visual::TextStyle>,
    pub rich: Option<authored_text_input::Editor<'a>>,
    pub rich_wire: &'a mut Option<Box<mathcraft_document::rich_text_source::RichTextWire>>,
    pub authored_wire: &'a mut Option<Box<mathcraft_document::rich_text_source::AuthoredTextWire>>,
    pub headroom: Option<&'a Result<usize, String>>,
    pub suppress_paragraph_break: bool,
}

pub(super) fn show(
    ui: &mut egui::Ui,
    id: u64,
    text: &mut String,
    cursors: &mut HashMap<u64, egui::text::CCursorRange>,
    composition: &mut Option<Composition>,
    focus_new: bool,
    presentation: Presentation<'_>,
) -> Output {
    let Presentation {
        style,
        mut rich,
        rich_wire,
        authored_wire,
        headroom,
        suppress_paragraph_break,
    } = presentation;
    let text_id = widget_id(id);
    let was_active = composition.as_ref().is_some_and(|state| state.id == id);
    if focus_new {
        if !was_active {
            // Region IDs repeat across documents; a fresh owner must never
            // inherit the former widget's IME purpose or preedit selection.
            let range = cursors.get(&id).copied().unwrap_or_else(|| {
                egui::text::CCursorRange::one(egui::text::CCursor::new(text.chars().count()))
            });
            selection_state(range).store(ui.ctx(), text_id);
        }
        ui.memory_mut(|memory| memory.request_focus(text_id));
    }
    let delivery = Delivery::read(ui, text_id, was_active);
    let mut before_cursor = composition
        .as_ref()
        .filter(|s| s.id == id)
        .map(|s| s.cursor)
        .or_else(|| cursors.get(&id).copied())
        .unwrap_or_default();
    let owns_keys = ui.memory(|memory| memory.has_focus(text_id));
    let pointer_inside = ui
        .input(|input| input.pointer.interact_pos())
        .is_some_and(|pos| ui.clip_rect().contains(pos) && ui.max_rect().contains(pos));
    if ui.input(|input| {
        input.events.iter().any(|event| {
            (pointer_inside && matches!(event, egui::Event::PointerButton { pressed: true, .. }))
                || (owns_keys
                    && matches!(
                        event,
                        egui::Event::Key {
                            key: egui::Key::ArrowLeft
                                | egui::Key::ArrowRight
                                | egui::Key::ArrowUp
                                | egui::Key::ArrowDown
                                | egui::Key::Home
                                | egui::Key::End
                                | egui::Key::PageUp
                                | egui::Key::PageDown,
                            pressed: true,
                            ..
                        }
                    ))
        })
    }) && let Some(editor) = rich.as_mut()
    {
        editor.navigate();
    }
    // Capture actual Ctrl+A/ordinary Text BEFORE any key consumption.
    let select_all_composing = was_active
        || composition.is_some()
        || delivery.starts
        || delivery.committed
        || delivery.active
        || delivery.cancelled
        || delivery.surrounding_deleted;
    if let Some(editor) = rich.as_mut() {
        ui.input(|input| {
            editor.begin_select_all_delivery(
                id,
                before_cursor,
                &input.events,
                owns_keys,
                select_all_composing,
            );
        });
    }
    let mut structural_changed = false;
    let mut structural_error = None;
    if owns_keys
        && let Some(editor) = rich.as_mut()
        && ui.input_mut(|input| consume_exact_key(input, egui::Modifiers::NONE, egui::Key::Enter))
    {
        // A confirmation's original batch provenance survives queue delivery.
        // Consume it here so neither TextEdit nor a later paint pass treats it
        // as a fresh paragraph command after composition was reconciled.
        let confirming = suppress_paragraph_break
            || was_active
            || ui.input(|input| {
                input
                    .events
                    .iter()
                    .any(|event| matches!(event, egui::Event::Ime(_)))
            });
        if !confirming {
            let result = headroom
                .ok_or_else(|| "Missing committed Text source budget".to_string())
                .and_then(|budget| budget.as_ref().copied().map_err(Clone::clone))
                .and_then(|budget| editor.split_paragraph(before_cursor, budget));
            match result {
                Ok((wire, cursor)) => {
                    *text = editor.plain_text().to_owned();
                    wire.install(rich_wire, authored_wire);
                    cursors.insert(id, cursor);
                    selection_state(cursor).store(ui.ctx(), text_id);
                    before_cursor = cursor;
                    structural_changed = true;
                }
                Err(error) => structural_error = Some(error),
            }
        }
    }
    if !was_active && delivery.starts {
        let widget = egui::text_edit::TextEditState::load(ui.ctx(), text_id).unwrap_or_default();
        let cursor = widget
            .cursor
            .char_range()
            .or_else(|| cursors.get(&id).copied())
            .unwrap_or_else(|| {
                egui::text::CCursorRange::one(egui::text::CCursor::new(text.chars().count()))
            });
        *composition = Some(Composition {
            id,
            draft: text.clone(),
            cursor,
            displaced: cursor.slice_str(text).to_owned(),
            widget,
        });
    }
    // Observe the actual admitted delivery BEFORE TextEdit consumes its key.
    // Composition and unknown/mixed events never create this single-use proof.
    let composing_delivery = was_active
        || composition.is_some()
        || delivery.starts
        || delivery.committed
        || delivery.active
        || delivery.cancelled
        || delivery.surrounding_deleted;
    if let Some(editor) = rich.as_mut() {
        ui.input(|input| {
            editor.observe_backspace_delivery(&input.events, owns_keys, composing_delivery)
        });
    }
    let selected_backspace = rich.as_ref().and_then(|editor| {
        ui.input(|input| {
            editor.capture_selected_backspace(
                id,
                before_cursor,
                &input.events,
                owns_keys,
                composing_delivery,
            )
        })
    });
    let active = composition.as_mut().filter(|state| state.id == id);
    let buffer = active.map_or(&mut *text, |state| &mut state.draft);
    let font = style
        .map(|style| FontId::proportional(style.font_size))
        .unwrap_or_else(typography::body_font);
    let color = style
        .map(|style| {
            let [r, g, b, a] = style.foreground;
            Color32::from_rgba_unmultiplied(r, g, b, a)
        })
        .unwrap_or(Color32::BLACK);
    let mut layout_error = structural_error;
    let horizontal_align = rich
        .as_ref()
        .map_or(egui::Align::LEFT, |editor| editor.horizontal_align());
    let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, width: f32| {
        if let Some(editor) = rich.as_ref() {
            match editor.layout(ui, text.as_str(), width, before_cursor) {
                Ok(layout) => return layout,
                Err(error) => layout_error = Some(error),
            }
        }
        typography::layout_job(
            ui.painter(),
            egui::text::LayoutJob::simple(text.as_str().into(), font.clone(), color, width),
        )
    };
    let output = egui::TextEdit::multiline(buffer)
        .id(text_id)
        .frame(egui::Frame::NONE)
        .desired_rows(1)
        .desired_width(340.)
        .hint_text("텍스트를 입력하세요")
        .font(font.clone())
        .text_color(color)
        .char_limit(65536)
        .horizontal_align(horizontal_align)
        .layouter(&mut layouter)
        .show(ui);
    if let Some(editor) = rich.as_mut() {
        editor.observe_select_all_selection(id, output.cursor_range);
    }
    // Same-plain committed replacement may still consolidate appearance.
    let mut changed = output.response.response.changed()
        || rich
            .as_ref()
            .is_some_and(|editor| editor.has_select_all_replacement());
    if composition.as_ref().is_some_and(|state| state.id == id) {
        changed = false;
        let cursor = output.cursor_range.unwrap_or_default();
        if delivery.committed {
            let state = composition.as_mut().unwrap();
            let published = if delivery.active || delivery.remove_preedit {
                replace_preedit(&state.draft, cursor, "")
            } else {
                state.draft.clone()
            };
            changed = *text != published;
            *text = published;
            if delivery.active {
                // A platform may commit one syllable and begin the next in a
                // single batch. Publish the completed prefix, keep its final
                // preedit in the view, and use an empty committed selection.
                let cursor = egui::text::CCursorRange::one(cursor.sorted_cursors()[0]);
                state.cursor = cursor;
                state.displaced.clear();
                state.widget = selection_state(cursor);
                cursors.insert(id, cursor);
            } else {
                *composition = None;
                let cursor = if delivery.remove_preedit {
                    egui::text::CCursorRange::one(cursor.sorted_cursors()[0])
                } else {
                    cursor
                };
                cursors.insert(id, cursor);
                selection_state(cursor).store(ui.ctx(), text_id);
            }
        } else if delivery.surrounding_deleted {
            let state = composition.as_mut().unwrap();
            let published = replace_preedit(&state.draft, cursor, &state.displaced);
            changed = *text != published;
            *text = published;
            let start = cursor.sorted_cursors()[0].index.0;
            let end = start + state.displaced.chars().count();
            let (primary, secondary) = if state.cursor.primary.index >= state.cursor.secondary.index
            {
                (end, start)
            } else {
                (start, end)
            };
            state.cursor.primary.index = egui::text::CharIndex(primary);
            state.cursor.secondary.index = egui::text::CharIndex(secondary);
            state.widget = selection_state(state.cursor);
            cursors.insert(id, state.cursor);
        } else if delivery.cancelled {
            let mut state = composition.take().unwrap();
            cursors.insert(id, state.cursor);
            state.widget.clear_undoer();
            state.widget.store(ui.ctx(), text_id);
            ui.ctx().request_repaint();
        }
    } else if let Some(range) = output.cursor_range {
        cursors.insert(id, range);
    }
    if changed && let Some(editor) = rich.as_mut() {
        let after = cursors.get(&id).copied().unwrap_or(before_cursor);
        let committed = headroom
            .ok_or_else(|| "Missing committed Text source budget".to_string())
            .and_then(|budget| budget.as_ref().copied().map_err(Clone::clone))
            .and_then(|budget| {
                editor.commit_with_backspace(
                    id,
                    text,
                    before_cursor,
                    after,
                    budget,
                    selected_backspace,
                )
            });
        match committed {
            Ok(Some(wire)) => {
                wire.install(rich_wire, authored_wire);
                layout_error = None;
            }
            Ok(None) => {
                changed = false;
                layout_error = None;
            }
            Err(error) => {
                *text = editor.plain_text().to_owned();
                changed = false;
                *composition = None;
                cursors.insert(id, before_cursor);
                selection_state(before_cursor).store(ui.ctx(), text_id);
                layout_error = Some(error);
                ui.ctx().request_repaint();
            }
        }
    }
    Output {
        response: output.response.response,
        changed: changed || structural_changed,
        error: layout_error,
    }
}
