//! Proposed child of authored_text_ui_tests: actual full App body + raw FIFO.
use super::*;
use mathcraft_document::rich_text_source::ParagraphAlignment as A;
fn command(f: &mut Fixture, value: A) {
    let c = f
        .app
        .paragraph_alignment_context()
        .expect("admitted Paragraph context");
    f.app.ribbon_command(
        &f.ctx,
        RibbonAction::SetParagraphAlignment {
            value,
            selection_key: c.selection_key,
        },
    );
}
fn actual_alignment(f: &Fixture) -> Option<A> {
    f.region()
        .validated_authored_text()
        .unwrap()
        .unwrap()
        .paragraph_alignment_info()
        .unwrap()
        .alignment()
}

#[test]
fn actual_alignment_keys_collapsed_source_history_and_no_calculation_submission() {
    let mut f = Fixture::new();
    f.frame(vec![egui::Event::Text("한글😀AB".into())]);
    f.select(2, 2);
    let generation = f.app.background.submitted_generation();
    let initial = f.xml();
    for (key_value, value) in [
        (egui::Key::E, A::Center),
        (egui::Key::R, A::Right),
        (egui::Key::J, A::Justify),
        (egui::Key::L, A::Left),
    ] {
        let old = f.xml();
        let undo = f.app.undo.len();
        f.frame(vec![key(
            key_value,
            egui::Modifiers::CTRL | egui::Modifiers::COMMAND,
        )]);
        assert_eq!(actual_alignment(&f), Some(value));
        assert_eq!(f.text(), "한글😀AB");
        assert_eq!(f.app.undo.len(), undo + 1);
        let current = f.xml();
        let xml = roxmltree::Document::parse(&current).unwrap();
        assert!(
            xml.descendants()
                .filter(|n| n.has_tag_name((
                    "http://schemas.microsoft.com/winfx/2006/xaml/presentation",
                    "Run"
                )))
                .all(|n| n.attribute("TextAlignment").is_none())
        );
        f.app.undo();
        assert_eq!(f.xml(), old);
        f.app.redo();
        assert_eq!(f.xml(), current);
        f.select(2, 2);
        let n = f.app.undo.len();
        command(&mut f, value);
        assert_eq!(f.app.undo.len(), n);
    }
    assert!(initial.contains("<Paragraph>"));
    assert_eq!(f.app.background.submitted_generation(), generation);
    assert!(f.app.last_executed_regions.is_empty() && !f.app.background.running);
}

#[test]
fn raw_prefix_alignment_suffix_is_fifo_with_one_history_step_for_command() {
    let mut f = Fixture::new();
    f.select(0, 0);
    let before = f.app.undo.len();
    f.frame(vec![
        egui::Event::Text("AB".into()),
        key(
            egui::Key::E,
            egui::Modifiers::CTRL | egui::Modifiers::COMMAND,
        ),
        egui::Event::Text("CD".into()),
    ]);
    assert_eq!(f.text(), "ABCD");
    assert_eq!(actual_alignment(&f), Some(A::Center));
    assert_eq!(f.app.undo.len(), before + 3);
    f.app.undo();
    assert_eq!(f.text(), "AB");
    assert_eq!(actual_alignment(&f), Some(A::Center));
    f.app.undo();
    assert_eq!(f.text(), "AB");
    assert_eq!(actual_alignment(&f), Some(A::Left));
    f.app.redo();
    assert_eq!(actual_alignment(&f), Some(A::Center));
    f.app.redo();
    assert_eq!(f.text(), "ABCD");
    assert!(f.app.plot_input_queue.is_empty());
    assert!(f.app.last_executed_regions.is_empty());
}

#[test]
fn paragraph_context_is_independent_collapsed_epoch_and_rejects_stale_actions() {
    let mut f = Fixture::new();
    f.frame(vec![egui::Event::Text("ABCD".into())]);
    f.select(2, 2);
    assert!(f.app.rich_text_context().is_none()); // character formatting remains nonempty-only
    let old = f.app.paragraph_alignment_context().unwrap();
    let xml = f.xml();
    let history = f.app.undo.len();
    f.select(3, 3);
    f.app.ribbon_command(
        &f.ctx,
        RibbonAction::SetParagraphAlignment {
            value: A::Right,
            selection_key: old.selection_key,
        },
    );
    assert_eq!(f.xml(), xml);
    assert_eq!(f.app.undo.len(), history);
    let current = f.app.paragraph_alignment_context().unwrap();
    assert_ne!(old.selection_key, current.selection_key);
    let admissions = f.app.rich_text.authored_admission_attempts;
    for _ in 0..10 {
        f.frame(vec![]);
        assert_eq!(
            f.app.paragraph_alignment_context().unwrap().selection_key,
            current.selection_key
        );
    }
    assert_eq!(f.app.rich_text.authored_admission_attempts, admissions);
    command(&mut f, A::Center);
    let changed = f.app.paragraph_alignment_context().unwrap();
    f.app.undo();
    let restored = f.app.paragraph_alignment_context().unwrap();
    assert_ne!(changed.selection_key, restored.selection_key);
    let restored_xml = f.xml();
    f.app.ribbon_command(
        &f.ctx,
        RibbonAction::SetParagraphAlignment {
            value: A::Right,
            selection_key: changed.selection_key,
        },
    );
    assert_eq!(f.xml(), restored_xml);
}

#[test]
fn composition_extra_modifiers_and_unsupported_second_paragraph_are_atomic() {
    let mut f = Fixture::new();
    f.frame(vec![egui::Event::Text("ABCD".into())]);
    f.select(2, 2);
    let old = f.xml();
    let n = f.app.undo.len();
    f.frame(vec![key(
        egui::Key::E,
        egui::Modifiers::CTRL | egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
    )]);
    assert_eq!(f.xml(), old);
    assert_eq!(f.app.undo.len(), n);
    let before_composition = f.app.paragraph_alignment_context().unwrap();
    f.frame(vec![egui::Event::Ime(egui::ImeEvent::Preedit {
        text: "조합".into(),
        active_range_chars: None,
    })]);
    assert!(
        f.app.text_composition.is_some(),
        "Preedit must start on the actual focused owner"
    );
    f.app.ribbon_command(
        &f.ctx,
        RibbonAction::SetParagraphAlignment {
            value: A::Right,
            selection_key: before_composition.selection_key,
        },
    );
    assert!(
        f.app.text_composition.is_some(),
        "Rejected ribbon command must not cancel composition"
    );
    assert_eq!(f.xml(), old);
    assert_eq!(f.app.undo.len(), n);
    f.frame(vec![key(
        egui::Key::R,
        egui::Modifiers::CTRL | egui::Modifiers::COMMAND,
    )]);
    assert_eq!(f.xml(), old);
    assert_eq!(f.app.undo.len(), n);
    // Existing reconcile policy cancels preedit on ordinary key delivery
    // without IME updates. Alignment did not mutate the source before that.
    assert!(f.app.text_composition.is_none());
    f.select(2, 2);
    f.frame(vec![egui::Event::Ime(egui::ImeEvent::Preedit {
        text: "ㅎ".into(),
        active_range_chars: None,
    })]);
    assert!(f.app.text_composition.is_some());
    f.frame(vec![egui::Event::Ime(egui::ImeEvent::Commit("한".into()))]);
    assert_eq!(f.text(), "AB한CD");
    assert_eq!(actual_alignment(&f), Some(A::Left));
    f.select(1, 1);
    f.frame(vec![key(egui::Key::Enter, egui::Modifiers::NONE)]);
    let split = f.xml();
    let n = f.app.undo.len();
    assert!(f.app.paragraph_alignment_context().is_none());
    f.frame(vec![key(
        egui::Key::J,
        egui::Modifiers::CTRL | egui::Modifiers::COMMAND,
    )]);
    assert_eq!(f.xml(), split);
    assert_eq!(f.app.undo.len(), n);
}

#[test]
fn explicit_run_affinity_is_rebound_after_alignment_and_typing_keeps_style() {
    let mut f = Fixture::new();
    f.frame(vec![egui::Event::Text("ABCD".into())]);
    f.select(1, 3);
    let context = f.app.rich_text_context().unwrap();
    f.app.ribbon_command(
        &f.ctx,
        RibbonAction::SetTextBold {
            value: true,
            selection_key: context.selection_key,
        },
    );
    f.select(3, 3);
    {
        let e = f
            .app
            .rich_text
            .authored_editors
            .get_mut(&f.id)
            .unwrap()
            .as_mut()
            .unwrap();
        e.affinity.remember_run(&e.source, 1).unwrap();
    }
    command(&mut f, A::Right);
    let e = f.app.rich_text.authored_editors[&f.id].as_ref().unwrap();
    assert_eq!(e.affinity.owner(&e.source), Some(1));
    f.frame(vec![egui::Event::Text("X".into())]);
    assert_eq!(f.text(), "ABCXD");
    let source = f.region().validated_authored_text().unwrap().unwrap();
    let format = source
        .selection_format(mathcraft_document::rich_text_source::TextSelection {
            paragraph: 0,
            anchor: 3,
            focus: 4,
        })
        .unwrap();
    assert_eq!(
        format.weight,
        Some(mathcraft_document::rich_text_source::Weight::Bold)
    );
    assert_eq!(actual_alignment(&f), Some(A::Right));
}

#[test]
fn enter_after_nonleft_is_explicit_atomic_limit_not_a_literal_newline_fallback() {
    let mut f = Fixture::new();
    f.frame(vec![egui::Event::Text("ABCD".into())]);
    f.select(2, 2);
    command(&mut f, A::Center);
    let old = f.xml();
    let n = f.app.undo.len();
    f.frame(vec![key(egui::Key::Enter, egui::Modifiers::NONE)]);
    assert_eq!(f.xml(), old);
    assert_eq!(f.text(), "ABCD");
    assert_eq!(f.app.undo.len(), n);
}

#[test]
fn borrowed_canvas_budget_failure_precedes_history_and_editor_install() {
    let mut f = Fixture::new();
    f.frame(vec![egui::Event::Text("AB".into())]);
    f.select(1, 1);
    let used = f.app.document.source_text_bytes().unwrap();
    f.app
        .document
        .title
        .push_str(&"a".repeat(8 * 1024 * 1024 - used - 4));
    assert_eq!(
        f.app.document.source_text_bytes().unwrap(),
        8 * 1024 * 1024 - 4
    );
    let before = f.region().authored_text.clone();
    let history = f.app.undo.len();
    let generation = f.app.background.submitted_generation();
    let context = f.app.paragraph_alignment_context().unwrap();
    f.app.ribbon_command(
        &f.ctx,
        RibbonAction::SetParagraphAlignment {
            value: A::Center,
            selection_key: context.selection_key,
        },
    );
    assert_eq!(f.region().authored_text, before);
    assert_eq!(f.app.undo.len(), history);
    assert_eq!(f.app.background.submitted_generation(), generation);
    assert!(!f.app.status.is_empty());
}

#[test]
fn alignment_keeps_real_calculated_value_cache_signature_and_pending_set() {
    let mut f = Fixture::new();
    f.frame(vec![egui::Event::Text("AB".into())]);
    let id = f.app.document.add_math(400., 38.4);
    let leaf = |kind| PortableTree {
        kind,
        children: vec![],
    };
    f.app
        .document
        .regions
        .iter_mut()
        .find(|r| r.id == id)
        .unwrap()
        .content = CanvasContent::Math(PortableTree {
        kind: NodeKind::Binary(BinaryOp::Evaluate),
        children: vec![
            leaf(NodeKind::Number("7".into())),
            leaf(NodeKind::Placeholder),
        ],
    });
    f.app.recalculate(); // fixture setup executes our Rust calculator once
    assert!(f.app.results[&id].error.is_none());
    assert!(f.app.results[&id].value.is_some());
    f.app.persist_calculation_cache();
    f.app.last_executed_regions.clear();
    f.select(1, 1);
    let value = serde_json::to_vec(&f.app.results[&id].value).unwrap();
    let cache = serde_json::to_vec(&f.app.document.calculation_cache).unwrap();
    let signature = f.app.document.calculation_signature();
    let pending = f.app.pending_regions.clone();
    let generation = f.app.background.submitted_generation();
    f.frame(vec![key(
        egui::Key::R,
        egui::Modifiers::CTRL | egui::Modifiers::COMMAND,
    )]);
    assert_eq!(actual_alignment(&f), Some(A::Right));
    assert_eq!(
        serde_json::to_vec(&f.app.results[&id].value).unwrap(),
        value
    );
    assert_eq!(
        serde_json::to_vec(&f.app.document.calculation_cache).unwrap(),
        cache
    );
    assert_eq!(f.app.document.calculation_signature(), signature);
    assert_eq!(f.app.pending_regions, pending);
    assert_eq!(f.app.background.submitted_generation(), generation);
    assert!(f.app.last_executed_regions.is_empty() && !f.app.background.running);
}

#[test]
fn selected_backspace_alignment_retires_restricted_owner_without_promoting_paste_or_ime() {
    for ime in [false, true] {
        let mut f = Fixture::new();
        f.frame(vec![egui::Event::Text("가A&😀BC한DE꼬".into())]);
        f.select(4, 7);
        let selection_key = f.app.rich_text_context().unwrap().selection_key;
        f.app.ribbon_command(
            &f.ctx,
            RibbonAction::SetTextBold {
                value: true,
                selection_key,
            },
        );
        f.select(2, 8);
        f.frame(vec![key(egui::Key::Backspace, egui::Modifiers::NONE)]);
        assert_eq!(f.text(), "가AE꼬");
        assert!(
            f.app.rich_text.authored_editors[&f.id]
                .as_ref()
                .unwrap()
                .affinity
                .is_backspace_continuation()
        );
        command(&mut f, A::Center);
        let editor = f.app.rich_text.authored_editors[&f.id].as_ref().unwrap();
        assert!(!editor.affinity.is_backspace_continuation());
        assert_eq!(editor.affinity.owner(&editor.source), None);
        let before = f.xml();
        let history = f.app.undo.len();
        if ime {
            f.frame(vec![egui::Event::Ime(egui::ImeEvent::Preedit {
                text: "ㅎ".into(),
                active_range_chars: None,
            })]);
            f.frame(vec![egui::Event::Ime(egui::ImeEvent::Commit("한".into()))]);
        } else {
            f.frame(vec![egui::Event::Paste("X".into())]);
        }
        assert_eq!(f.xml(), before, "{}", f.app.status);
        assert_eq!(f.text(), "가AE꼬");
        assert_eq!(f.app.undo.len(), history);
    }
}

#[test]
fn actual_ribbon_pointer_four_alignments_and_disabled_context_preserve_transactions() {
    fn glyph(shape: &egui::Shape, label: &str) -> Option<egui::Pos2> {
        match shape {
            egui::Shape::Text(t) if t.galley.text() == label => Some(t.pos + t.galley.size() * 0.5),
            egui::Shape::Vec(shapes) => shapes.iter().find_map(|s| glyph(s, label)),
            _ => None,
        }
    }
    fn draw(f: &mut Fixture, events: Vec<egui::Event>, label: &str) -> Option<egui::Pos2> {
        f.frame += 1;
        let mut raw = egui::RawInput {
            events,
            time: Some(f64::from(f.frame)),
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1200., 900.))),
            ..Default::default()
        };
        f.app.prepare_plot_input(&f.ctx, &mut raw);
        let output = f.ctx.run_ui(raw, |ui| f.app.app_ui_frame(ui));
        let point = output.shapes.iter().find_map(|shape| {
            glyph(&shape.shape, label).filter(|point| shape.clip_rect.contains(*point))
        });
        output.drop_without_applying_deltas();
        point
    }
    fn click(f: &mut Fixture, point: egui::Pos2, label: &str) {
        let mut events = vec![egui::Event::PointerMoved(point)];
        for pressed in [true, false] {
            events.push(egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
        }
        draw(f, events, label);
        assert!(f.app.plot_input_queue.is_empty());
    }
    let mut f = Fixture::new();
    f.frame(vec![egui::Event::Text("ABCD".into())]);
    f.app.ribbon.tab = ribbon::RibbonTab::TextFormatting;
    f.app.ribbon.tools_open = true;
    draw(&mut f, vec![], "가운데"); // settle actual ribbon ScrollArea layout
    let initial = f.xml();
    let generation = f.app.background.submitted_generation();
    let initial_history = f.app.undo.len();
    let mut states = vec![initial.clone()];
    for (label, alignment) in [
        ("가운데", A::Center),
        ("오른쪽", A::Right),
        ("양쪽", A::Justify),
        ("왼쪽", A::Left),
    ] {
        f.select(2, 2);
        let before = f.app.paragraph_alignment_context().unwrap();
        let point = draw(&mut f, vec![], label).expect("Visible actual ribbon label glyph");
        let history = f.app.undo.len();
        click(&mut f, point, label);
        assert_eq!(
            actual_alignment(&f),
            Some(alignment),
            "actual pointer action: {label}; {}",
            f.app.status
        );
        assert_eq!(f.text(), "ABCD");
        assert_eq!(f.app.undo.len(), history + 1);
        let after = f.app.paragraph_alignment_context().unwrap();
        assert_ne!(before.selection_key, after.selection_key);
        states.push(f.xml());
    }
    assert_eq!(f.app.undo.len(), initial_history + 4);
    for expected in states[..4].iter().rev() {
        f.app.undo();
        assert_eq!(&f.xml(), expected);
    }
    for expected in states[1..].iter() {
        f.app.redo();
        assert_eq!(&f.xml(), expected);
    }
    // Multi-Paragraph source is deliberately ineligible in this bounded slice.
    // Click the actual disabled button; no direct command helper is involved.
    f.frame(vec![]);
    f.select(1, 1);
    f.frame(vec![key(egui::Key::Enter, egui::Modifiers::NONE)]);
    assert!(f.app.paragraph_alignment_context().is_none());
    let source = f.xml();
    let history = f.app.undo.len();
    let point = draw(&mut f, vec![], "가운데").expect("Disabled actual button is still painted");
    click(&mut f, point, "가운데");
    assert_eq!(f.xml(), source);
    assert_eq!(f.app.undo.len(), history);
    assert!(f.app.paragraph_alignment_context().is_none());
    assert_eq!(f.app.background.submitted_generation(), generation);
    assert!(f.app.last_executed_regions.is_empty() && !f.app.background.running);
}
