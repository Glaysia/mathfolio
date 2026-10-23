//! The same egui chrome is used by native and future WASM hosts.
use super::*;
use crate::{command_catalog as catalog, theme};

pub(super) fn show(
    parent: &mut Ui,
    state: &mut RibbonState,
    current: &RibbonContext<'_>,
) -> Vec<RibbonAction> {
    let mut actions = Vec::new();
    state.input_ids.clear();
    state
        .input_ids
        .push(egui::Id::new("ribbon-rich-text-font-points"));
    state.text_format.sync(current.text_format);
    state.command_states = catalog::COMMANDS
        .iter()
        .map(|c| c.disabled_reason(current))
        .collect();
    let p = theme::palette(parent.ctx());
    egui::Panel::top("mathcraft-chrome")
        .frame(
            egui::Frame::NONE
                .fill(p.panel)
                .inner_margin(egui::Margin::symmetric(12, 8)),
        )
        .show(parent, |ui| {
            let narrow = ui.available_width() < 620.;
            egui::containers::Sides::new().shrink_left().show(
                ui,
                |ui| {
                    ui.set_height(34.);
                    ui.menu_button("파일", |ui| {
                        ui.set_min_width(220.);
                        for action in [
                            RibbonAction::New,
                            RibbonAction::Open,
                            RibbonAction::Save,
                            RibbonAction::SaveAs,
                            RibbonAction::ExportMcdx,
                            RibbonAction::ExportPdf,
                            RibbonAction::Print,
                            RibbonAction::PageSetup,
                            RibbonAction::Undo,
                            RibbonAction::Redo,
                        ] {
                            menu_command(ui, current, action, &mut actions);
                        }
                        if narrow {
                            theme_menu(ui);
                        }
                    });
                    for (title, action) in [
                        ("저장", RibbonAction::Save),
                        ("실행 취소", RibbonAction::Undo),
                        ("다시 실행", RibbonAction::Redo),
                    ] {
                        if narrow && matches!(action, RibbonAction::Undo | RibbonAction::Redo) {
                            continue;
                        }
                        let label = match action {
                            RibbonAction::Undo | RibbonAction::Redo => " ",
                            _ => title,
                        };
                        let response = button(ui, current, action, label);
                        if matches!(action, RibbonAction::Undo | RibbonAction::Redo) {
                            paint_icon(
                                ui.painter(),
                                Rect::from_center_size(response.rect.center(), Vec2::splat(18.)),
                                if action == RibbonAction::Undo {
                                    Icon::Undo
                                } else {
                                    Icon::Redo
                                },
                                if response.enabled() {
                                    p.accent
                                } else {
                                    ui.visuals().weak_text_color()
                                },
                            );
                        }
                        response.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::WidgetType::Button,
                                response.enabled(),
                                title,
                            )
                        });
                        if response.clicked() {
                            actions.push(action);
                        }
                    }
                    ui.add_space(6.);
                    ui.add(
                        egui::Label::new(egui::RichText::new(current.document_name).strong())
                            .truncate(),
                    )
                    .on_hover_text(current.document_name);
                },
                |ui| {
                    if ui
                        .button(if narrow { "명령" } else { "명령 검색" })
                        .on_hover_text("명령·기존 단축키·예제를 검색합니다.  F1")
                        .clicked()
                    {
                        state.help_requested = true;
                        state.help_focus = true;
                    }
                    if ui
                        .selectable_label(state.inspector_open, "속성")
                        .on_hover_text("선택한 수식·텍스트·도표의 속성 패널")
                        .clicked()
                    {
                        state.inspector_open = !state.inspector_open;
                    }
                    if !narrow {
                        theme_menu(ui);
                    }
                },
            );
            if !state.minimized {
                ui.add_space(5.);
                egui::ScrollArea::horizontal()
                    .id_salt("authoring-tools")
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.set_height(34.);
                            for (icon, title, action) in [
                                (Icon::Math, "수식", RibbonAction::InsertMath),
                                (Icon::Text, "텍스트", RibbonAction::InsertText),
                                (Icon::Matrix, "표", RibbonAction::InsertTable),
                                (Icon::Plot, "도표", RibbonAction::InsertPlot),
                                (Icon::Function, "풀이", RibbonAction::InsertSolveBlock),
                            ] {
                                let response =
                                    button(ui, current, action, &format!("     {title}"));
                                paint_icon(
                                    ui.painter(),
                                    Rect::from_center_size(
                                        response.rect.left_center() + egui::vec2(15., 0.),
                                        Vec2::splat(17.),
                                    ),
                                    icon,
                                    p.accent,
                                );
                                if response.clicked() {
                                    actions.push(action);
                                }
                            }
                            ui.separator();
                            let response = ui.add(
                                egui::Button::new(
                                    egui::RichText::new("계산  F5").color(p.button_text),
                                )
                                .fill(p.button),
                            );
                            if response
                                .on_hover_text(catalog::tooltip(RibbonAction::CalculateNow, "계산"))
                                .clicked()
                            {
                                actions.push(RibbonAction::CalculateNow);
                            }
                            ui.menu_button("계산 설정", |ui| {
                                menu_command(ui, current, RibbonAction::Recalculate, &mut actions);
                                menu_command(
                                    ui,
                                    current,
                                    RibbonAction::StopAllCalculations,
                                    &mut actions,
                                );
                                ui.separator();
                                let mut automatic = current.automatic_calculation;
                                if ui.checkbox(&mut automatic, "자동 계산").changed() {
                                    actions.push(RibbonAction::SetAutomaticCalculation(automatic));
                                }
                                menu_command(
                                    ui,
                                    current,
                                    RibbonAction::PreviousError,
                                    &mut actions,
                                );
                                menu_command(ui, current, RibbonAction::NextError, &mut actions);
                            });
                            ui.separator();
                            if ui
                                .selectable_label(state.tools_open, "전체 도구")
                                .on_hover_text("연산자·함수·행렬·문서 등 모든 기존 도구를 엽니다.")
                                .clicked()
                            {
                                state.tools_open = !state.tools_open;
                            }
                        });
                    });
                if state.tools_open {
                    ui.add_space(6.);
                    egui::Frame::new()
                        .fill(p.surface)
                        .corner_radius(8)
                        .inner_margin(7)
                        .show(ui, |ui| {
                            egui::ScrollArea::horizontal()
                                .id_salt("all-tool-tabs")
                                .scroll_bar_visibility(
                                    egui::scroll_area::ScrollBarVisibility::AlwaysHidden,
                                )
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        for tab in RibbonTab::ALL {
                                            if tab_button(ui, tab.label(), state.tab == tab, false)
                                                .clicked()
                                            {
                                                state.tab = tab;
                                            }
                                        }
                                    });
                                });
                            ui.add_space(4.);
                            egui::ScrollArea::horizontal()
                                .id_salt("all-tool-controls")
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        // Preserve the existing handlers and their source-admitted availability.
                                        ui.spacing_mut().item_spacing = egui::vec2(3., 0.);
                                        ui.spacing_mut().button_padding = egui::vec2(5., 2.);
                                        ui.spacing_mut().interact_size.y = 21.;
                                        match state.tab {
                                            RibbonTab::Math => math_tab(ui, current, &mut actions),
                                            RibbonTab::InputOutput => io_tab(ui, &mut actions),
                                            RibbonTab::Functions => functions_tab(ui, &mut actions),
                                            RibbonTab::Matrices => {
                                                matrices_tab(ui, current, &mut actions)
                                            }
                                            RibbonTab::Plots => {
                                                plots_tab(ui, current, &mut actions)
                                            }
                                            RibbonTab::MathFormatting => {
                                                math_format_tab(ui, current, &mut actions)
                                            }
                                            RibbonTab::TextFormatting => text_format_tab(
                                                ui,
                                                current,
                                                &mut state.text_format,
                                                &mut actions,
                                            ),
                                            RibbonTab::Calculation => {
                                                calculation_tab(ui, current, &mut actions)
                                            }
                                            RibbonTab::Document => {
                                                document_tab(ui, current, &mut actions)
                                            }
                                            RibbonTab::Resources => resources_tab(ui),
                                        }
                                    });
                                });
                        });
                }
            }
        });
    if state.inspector_open {
        egui::Panel::right("selection-properties").default_size(266.).min_size(240.).max_size(380.).resizable(true)
            .frame(egui::Frame::NONE.fill(p.panel).inner_margin(14))
            .show(parent, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("속성");
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("닫기").clicked() { state.inspector_open = false; }
                    });
                });
                ui.separator();
                egui::ScrollArea::vertical().id_salt("selection-properties-scroll").show(ui, |ui| {
                    if current.has_math { math_properties(ui, current, &mut actions, &mut state.input_ids); }
                    else if current.text_format.is_some() || current.paragraph_alignment.is_some() {
                        text_properties(ui, current, &mut state.text_format, &mut actions, &mut state.input_ids);
                    } else if current.has_plot {
                        ui.strong("XY 도표");
                        ui.label("축·곡선·범위와 선 모양을 설정합니다.");
                        if button(ui, current, RibbonAction::EditPlot, "도표 설정 열기").clicked() { actions.push(RibbonAction::EditPlot); }
                    } else {
                        ui.strong("선택한 영역의 속성");
                        ui.label("수식·텍스트·도표를 선택하면 적용 가능한 속성을 표시합니다.");
                    }
                    if let Some(disabled) = current.region_calculation_disabled {
                        ui.add_space(14.);
                        ui.separator();
                        let mut enabled = !disabled;
                        if ui.checkbox(&mut enabled, "이 영역 계산").changed() { actions.push(RibbonAction::SetRegionCalculationDisabled(!enabled)); }
                    }
                    ui.add_space(18.);
                    ui.separator();
                    ui.small("기존 키는 그대로입니다. 명령 검색에서 단축키와 입력 예제를 확인하세요.");
                });
            });
    }
    actions
}

fn theme_menu(ui: &mut Ui) {
    ui.menu_button("테마", |ui| {
        let selected = theme::theme_mode(ui.ctx());
        for (mode, title) in [
            (theme::ThemeMode::FollowHost, "호스트 테마 따르기"),
            (theme::ThemeMode::Light, "밝게"),
            (theme::ThemeMode::Dark, "어둡게"),
            (theme::ThemeMode::Nord, "Nord"),
        ] {
            if ui.selectable_label(selected == mode, title).clicked() {
                theme::set_theme_mode(ui.ctx(), mode);
                ui.close();
            }
        }
        if !theme::has_host_theme(ui.ctx()) {
            ui.separator();
            ui.small(
                "호스트가 테마를 전달하면 자동으로 적용합니다. 현재는 기본 테마를 사용합니다.",
            );
        }
    })
    .response
    .on_hover_text(
        "테마 설정 · 호스트의 배경·글자·강조색을 따르거나 밝게/어둡게를 직접 선택합니다.",
    );
}

fn button(ui: &mut Ui, current: &RibbonContext<'_>, action: RibbonAction, label: &str) -> Response {
    let command = catalog::find(action);
    let reason = command.and_then(|c| c.disabled_reason(current));
    ui.add_enabled(
        reason.is_none(),
        egui::Button::new(label).min_size(egui::vec2(30., 30.)),
    )
    .on_hover_text(catalog::tooltip(action, label))
    .on_disabled_hover_text(reason.unwrap_or("현재 문맥에서는 사용할 수 없습니다."))
}

fn menu_command(
    ui: &mut Ui,
    current: &RibbonContext<'_>,
    action: RibbonAction,
    actions: &mut Vec<RibbonAction>,
) {
    if let Some(command) = catalog::find(action) {
        let label = if command.keys.is_empty() {
            command.title.to_owned()
        } else {
            format!("{}    {}", command.title, command.keys)
        };
        if button(ui, current, action, &label).clicked() {
            actions.push(action);
            ui.close();
        }
    }
}

fn math_properties(
    ui: &mut Ui,
    c: &RibbonContext<'_>,
    actions: &mut Vec<RibbonAction>,
    inputs: &mut Vec<egui::Id>,
) {
    ui.strong(if c.in_table {
        "표 · 수식 서식"
    } else {
        "수식 · 결과 서식"
    });
    ui.add_space(8.);
    ui.label("결과 표시");
    let mut format = c.result_format;
    egui::ComboBox::from_id_salt("properties-result-format")
        .width(ui.available_width() - 8.)
        .selected_text(["일반", "소수", "과학", "공학", "백분율"][usize::from(format).min(4)])
        .show_ui(ui, |ui| {
            for (index, title) in ["일반", "소수", "과학", "공학", "백분율"]
                .iter()
                .enumerate()
            {
                if ui
                    .selectable_value(&mut format, index as u8, *title)
                    .changed()
                {
                    actions.push(RibbonAction::SetResultFormat(format));
                }
            }
        });
    ui.horizontal(|ui| {
        ui.label("소수 자릿수");
        let mut precision = c.precision;
        let response = ui.add(
            egui::DragValue::new(&mut precision)
                .range(0..=15)
                .update_while_editing(false),
        );
        inputs.push(response.id);
        if response.changed() && precision != c.precision {
            actions.push(RibbonAction::SetPrecision(precision));
        }
    });
    let mut zeros = c.number_format.show_trailing_zeros;
    if ui.checkbox(&mut zeros, "후행 0 표시").changed() {
        actions.push(RibbonAction::SetTrailingZeros(zeros));
    }
    ui.add_space(10.);
    ui.label("복소수 표시");
    let choices = [
        (ComplexFormat::I, "i"),
        (ComplexFormat::J, "j"),
        (ComplexFormat::PolarRadians, "극형식 · rad"),
        (ComplexFormat::PolarPiRadians, "극형식 · π rad"),
        (ComplexFormat::PolarDegrees, "극형식 · °"),
    ];
    let mut complex = c.number_format.complex_format;
    egui::ComboBox::from_id_salt("properties-complex")
        .selected_text(choices.iter().find(|(v, _)| *v == complex).unwrap().1)
        .show_ui(ui, |ui| {
            for (value, title) in choices {
                if ui.selectable_value(&mut complex, value, title).changed() {
                    actions.push(RibbonAction::SetComplexFormat(value));
                }
            }
        });
    ui.add_space(10.);
    ui.horizontal(|ui| {
        ui.label("글꼴 크기");
        let mut size = c.font_points;
        let response = ui.add(
            egui::DragValue::new(&mut size)
                .range(4. ..=72.)
                .suffix(" pt")
                .update_while_editing(false),
        );
        inputs.push(response.id);
        if response.changed() && size != c.font_points {
            actions.push(RibbonAction::SetFontSize(size));
        }
    });
    if let Some(indices) = c.matrix_indices {
        let mut show = indices.show_indices;
        if ui.checkbox(&mut show, "행렬 인덱스 표시").changed() {
            actions.push(RibbonAction::SetMatrixIndices {
                owner: indices.owner,
                show,
            });
        }
    }
    ui.add_space(12.);
    ui.small("라벨과 임계값 등 추가 서식은 전체 도구 → 계산 서식 지정에서 설정할 수 있습니다.");
}

fn text_properties(
    ui: &mut Ui,
    c: &RibbonContext<'_>,
    draft: &mut TextFormatDraft,
    actions: &mut Vec<RibbonAction>,
    inputs: &mut Vec<egui::Id>,
) {
    ui.strong("텍스트 서식");
    if let Some(text) = c.text_format.filter(|c| c.enabled) {
        ui.add_space(8.);
        ui.horizontal(|ui| {
            ui.label("글꼴 크기");
            let response = ui.add(
                egui::TextEdit::singleline(&mut draft.size)
                    .desired_width(65.)
                    .hint_text("pt"),
            );
            inputs.push(response.id);
            if (response.lost_focus()
                || response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                && let Some(action) = draft.commit_size()
            {
                actions.push(action);
            }
        });
        ui.horizontal(|ui| {
            for (label, value, action) in [
                (
                    "굵게",
                    text.retained_bold,
                    RibbonAction::SetTextBold {
                        value: !text.retained_bold,
                        selection_key: text.selection_key,
                    },
                ),
                (
                    "기울임",
                    text.retained_italic,
                    RibbonAction::SetTextItalic {
                        value: !text.retained_italic,
                        selection_key: text.selection_key,
                    },
                ),
            ] {
                if ui.selectable_label(value, label).clicked() {
                    actions.push(action);
                }
            }
            if ui
                .add_enabled(
                    text.underline_set_enabled,
                    egui::Button::new("밑줄").selected(text.retained_underline),
                )
                .clicked()
            {
                actions.push(RibbonAction::SetTextUnderline {
                    value: !text.retained_underline,
                    selection_key: text.selection_key,
                });
            }
        });
        ui.horizontal(|ui| {
            ui.label("글자 색");
            if ui
                .color_edit_button_srgba_unmultiplied(&mut draft.color)
                .changed()
            {
                draft.color_dirty = true;
            }
        });
        let commit = !ui.ctx().any_popup_open()
            || ui.input(|i| i.pointer.any_released() || i.key_pressed(egui::Key::Enter));
        if commit && let Some(action) = draft.commit_color() {
            actions.push(action);
        }
    } else {
        ui.label("문자를 선택하면 글꼴 크기·색과 강조를 바꿀 수 있습니다.");
    }
    if let Some(paragraph) = c.paragraph_alignment {
        ui.add_space(10.);
        ui.label("문단 맞춤");
        ui.horizontal(|ui| {
            for (value, label, keys) in [
                (
                    mathcraft_document::rich_text_source::ParagraphAlignment::Left,
                    "왼쪽",
                    "왼쪽 맞춤 · Ctrl+L",
                ),
                (
                    mathcraft_document::rich_text_source::ParagraphAlignment::Center,
                    "가운데",
                    "가운데 맞춤 · Ctrl+E",
                ),
                (
                    mathcraft_document::rich_text_source::ParagraphAlignment::Right,
                    "오른쪽",
                    "오른쪽 맞춤 · Ctrl+R",
                ),
                (
                    mathcraft_document::rich_text_source::ParagraphAlignment::Justify,
                    "양쪽",
                    "양쪽 맞춤 · Ctrl+J",
                ),
            ] {
                if ui
                    .selectable_label(paragraph.current == Some(value), label)
                    .on_hover_text(keys)
                    .clicked()
                {
                    actions.push(RibbonAction::SetParagraphAlignment {
                        value,
                        selection_key: paragraph.selection_key,
                    });
                }
            }
        });
    }
}

pub(super) fn help(
    ctx: &egui::Context,
    state: &mut RibbonState,
    open: &mut bool,
) -> Vec<RibbonAction> {
    let mut actions = Vec::new();
    let mut dismiss = false;
    egui::Modal::new(egui::Id::new("mathcraft-command-help")).show(ctx, |ui| {
        ui.set_width((ctx.content_rect().width() - 64.).clamp(260., 620.));
        ui.horizontal(|ui| {
            ui.heading("명령 및 단축키");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("닫기").clicked() {
                    dismiss = true;
                }
            });
        });
        ui.label("기존 키 그대로 · 이름, 키 조합, 입력 예제로 검색");
        let input = ui.add(
            egui::TextEdit::singleline(&mut state.help_query)
                .hint_text("예: 아래첨자, Ctrl+Y, 행렬, 계산")
                .desired_width(f32::INFINITY),
        );
        if state.help_focus {
            input.request_focus();
            state.help_focus = false;
        }
        ui.separator();
        egui::ScrollArea::vertical()
            .id_salt("command-help-scroll")
            .max_height((ctx.content_rect().height() - 240.).max(120.))
            .show(ui, |ui| {
                let mut count = 0;
                for &(keys, title, scope, description) in catalog::EDITING_HELP {
                    if !catalog::text_matches(&state.help_query, &[keys, title, scope, description])
                    {
                        continue;
                    }
                    count += 1;
                    ui.horizontal_wrapped(|ui| {
                        ui.strong(title);
                        ui.label(egui::RichText::new(keys).monospace());
                        ui.weak(scope);
                    });
                    ui.label(description);
                    ui.add_space(8.);
                }
                for (index, command) in catalog::COMMANDS
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| c.matches(&state.help_query))
                {
                    count += 1;
                    let reason = state.command_states.get(index).copied().flatten();
                    ui.horizontal_wrapped(|ui| {
                        let label = format!("{}    {}", command.title, command.keys);
                        if ui
                            .add_enabled(
                                reason.is_none(),
                                egui::Button::new(label).min_size(egui::vec2(260., 30.)),
                            )
                            .on_hover_text(command.tooltip())
                            .on_disabled_hover_text(reason.unwrap_or(""))
                            .clicked()
                        {
                            actions.push(command.action);
                            dismiss = true;
                        }
                        ui.weak(command.category);
                    });
                    ui.label(command.description);
                    if !command.example.is_empty() {
                        ui.small(format!("예: {}", command.example));
                    }
                    if let Some(reason) = reason {
                        ui.weak(reason);
                    }
                    ui.add_space(10.);
                }
                if count == 0 {
                    ui.label("일치하는 명령이 없습니다. 다른 이름이나 키 조합으로 검색하세요.");
                }
            });
        ui.separator();
        ui.small("Enter는 수정 확정 · Ctrl+−는 이름 아래첨자 · Ctrl+Y는 다시 실행");
    });
    if dismiss {
        *open = false;
    }
    actions
}
