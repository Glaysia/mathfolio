//! Child of existing original-XAML-rich Fixture tests; no original execution.
use super::*;
use mathfolio_document::rich_text_source::ParagraphAlignment as A;

#[test]
fn actual_native_alignment_key_cold_own_native_pdf_and_undo_preserve_source() {
    let mut f = Fixture::new();
    f.select(2, 2);
    let context = f
        .app
        .paragraph_alignment_context()
        .expect("original explicit LTR fixture");
    let original = f.xml();
    let stored = roxmltree::Document::parse(&original).unwrap();
    assert_eq!(
        stored.root_element().attribute("TextAlignment"),
        Some("Left")
    );
    assert_eq!(
        stored.root_element().attribute("FlowDirection"),
        Some("LeftToRight")
    );
    assert!(
        stored
            .descendants()
            .find(|n| n.tag_name().name() == "Paragraph")
            .unwrap()
            .attribute("TextAlignment")
            .is_none()
    );
    assert_eq!(
        context.current,
        Some(A::Left),
        "Explicit root source alignment is inherited; no default is inferred"
    );
    let history = f.app.undo.len();
    let generation = f.app.background.submitted_generation();
    f.frame(vec![key(egui::Key::E, egui::Modifiers::COMMAND)]);
    let centered = f.xml();
    assert_eq!(f.text(), ORIGINAL);
    assert_eq!(f.app.undo.len(), history + 1);
    let parsed = roxmltree::Document::parse(&centered).unwrap();
    assert_eq!(
        parsed
            .descendants()
            .find(|n| n.tag_name().name() == "Paragraph")
            .unwrap()
            .attribute("TextAlignment"),
        Some("Center")
    );
    assert!(
        f.app.rich_text.editors[&f.id]
            .as_ref()
            .unwrap()
            .source
            .prove_format_only()
            .is_ok()
    );
    f.app.undo();
    assert_eq!(f.xml(), original);
    f.app.redo();
    assert_eq!(f.xml(), centered);
    let own = f.save("paragraph-alignment.mathfolio");
    let native = f.root.join("paragraph-alignment.mcdx");
    assert!(
        f.app
            .save_to_destination(Some(native.clone()), native_save_ui::SaveIntent::SaveAs),
        "{}",
        f.app.status
    );
    f.app
        .export_vector_pdf_for_test(&f.root.join("paragraph-alignment.pdf"))
        .unwrap();
    for path in [own, native] {
        let mut cold = WorksheetApp {
            automatic_calculation: false,
            ..Default::default()
        };
        cold.open(&path);
        assert!(cold.status.starts_with("Opened"), "{}", cold.status);
        let id = cold
            .document
            .regions
            .iter()
            .find(|r| matches!(&r.content,CanvasContent::Text(t) if t==ORIGINAL))
            .unwrap()
            .id;
        cold.selected = Some(id);
        cold.prepare_rich_text();
        let source = &cold.rich_text.editors[&id].as_ref().unwrap().source;
        assert_eq!(source.xaml(), centered);
        assert_eq!(
            source.paragraph_alignment_info().unwrap().alignment(),
            Some(A::Center)
        );
        assert!(cold.last_executed_regions.is_empty() && !cold.background.running);
    }
    assert_eq!(f.app.background.submitted_generation(), generation);
    assert!(f.app.last_executed_regions.is_empty());
}

#[test]
fn native_alignment_does_not_consume_an_unrelated_widgets_command() {
    let mut f = Fixture::new();
    f.select(1, 1);
    let before = f.xml();
    let n = f.app.undo.len();
    let other = egui::Id::new("unrelated-widget");
    f.ctx.memory_mut(|m| m.request_focus(other));
    let mut remaining = false;
    let output = f.ctx.run_ui(
        egui::RawInput {
            events: vec![key(egui::Key::J, egui::Modifiers::COMMAND)],
            ..Default::default()
        },
        |ui| {
            f.app.paragraph_alignment_shortcuts(ui.ctx());
            remaining = ui.input(|i| {
                i.events.iter().any(|e| {
                    matches!(
                        e,
                        egui::Event::Key {
                            key: egui::Key::J,
                            pressed: true,
                            ..
                        }
                    )
                })
            });
        },
    );
    output.drop_without_applying_deltas();
    assert!(remaining);
    assert_eq!(f.xml(), before);
    assert_eq!(f.app.undo.len(), n);
}
