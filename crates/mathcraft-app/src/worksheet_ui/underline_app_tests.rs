//! Candidate actual own-App input/save/print tests. Not original runtime parity.
use super::*;
use mathcraft_document::rich_text_source::TextDecorations;

fn decoration(f: &Fixture, start: usize, end: usize) -> Option<TextDecorations> {
    f.region()
        .validated_authored_text()
        .unwrap()
        .unwrap()
        .selection_format(mathcraft_document::rich_text_source::TextSelection {
            paragraph: 0,
            anchor: start,
            focus: end,
        })
        .unwrap()
        .text_decorations
}
fn command(f: &mut Fixture, value: bool) {
    let context = f.app.rich_text_context().unwrap();
    assert!(context.underline_enabled);
    f.app.ribbon_command(
        &f.ctx,
        RibbonAction::SetTextUnderline {
            value,
            selection_key: context.selection_key,
        },
    );
}

#[test]
fn underline_real_key_toolbar_history_and_known_mixed_have_distinct_policies() {
    let mut f = Fixture::new();
    f.frame(vec![egui::Event::Text("한글😀AB".into())]);
    f.select(1, 3);
    let initial = f.xml();
    let history = f.app.undo.len();
    let generation = f.app.background.submitted_generation();
    f.frame(vec![key(
        egui::Key::U,
        egui::Modifiers::CTRL | egui::Modifiers::COMMAND,
    )]);
    assert_eq!(f.text(), "한글😀AB");
    assert_eq!(decoration(&f, 1, 3), Some(TextDecorations::Underline));
    let underlined = f.xml();
    assert_eq!(f.app.undo.len(), history + 1);
    f.app.undo();
    assert_eq!(f.xml(), initial);
    f.app.redo();
    assert_eq!(f.xml(), underlined);
    f.select(0, 5);
    let context = f.app.rich_text_context().unwrap();
    assert!(context.underline_enabled && context.retained_underline);
    assert_eq!(decoration(&f, 0, 5), None, "Mixed known selection");
    // Toolbar uses the retained checked value, so explicit false clears all.
    command(&mut f, false);
    assert_eq!(decoration(&f, 0, 5), Some(TextDecorations::None));
    f.app.undo();
    f.select(0, 5);
    f.frame(vec![key(
        egui::Key::U,
        egui::Modifiers::CTRL | egui::Modifiers::COMMAND,
    )]);
    assert_eq!(decoration(&f, 0, 5), Some(TextDecorations::Underline));
    f.frame(vec![key(
        egui::Key::U,
        egui::Modifiers::CTRL | egui::Modifiers::COMMAND,
    )]);
    assert_eq!(decoration(&f, 0, 5), Some(TextDecorations::None));
    assert_eq!(f.app.background.submitted_generation(), generation);
    assert!(f.app.last_executed_regions.is_empty() && !f.app.background.running);
}

#[test]
fn collapsed_and_composing_ctrl_u_are_reserved_without_deletion_or_ime_cancel() {
    let mut f = Fixture::new();
    f.frame(vec![egui::Event::Text("앞 한😀 뒤".into())]);
    f.select(4, 4);
    let wire = f.region().authored_text.clone();
    let history = f.app.undo.len();
    f.frame(vec![key(
        egui::Key::U,
        egui::Modifiers::CTRL | egui::Modifiers::COMMAND,
    )]);
    assert_eq!(f.text(), "앞 한😀 뒤");
    assert_eq!(f.region().authored_text, wire);
    assert_eq!(f.app.undo.len(), history);
    f.frame(vec![egui::Event::Ime(egui::ImeEvent::Preedit {
        text: "조합".into(),
        active_range_chars: Some(0..2),
    })]);
    assert!(f.app.text_composition.is_some());
    f.frame(vec![key(
        egui::Key::U,
        egui::Modifiers::CTRL | egui::Modifiers::COMMAND,
    )]);
    assert!(
        f.app.text_composition.is_some(),
        "Reserved U cannot cancel IME"
    );
    assert_eq!(f.region().authored_text, wire);
    assert_eq!(f.app.undo.len(), history);
    f.frame(vec![egui::Event::Ime(egui::ImeEvent::Commit(
        "확정".into(),
    ))]);
    assert_eq!(f.text(), "앞 한😀확정 뒤");
    assert_eq!(f.app.undo.len(), history + 1);
}

#[test]
fn stale_underline_action_cannot_format_a_new_selection() {
    let mut f = Fixture::new();
    f.frame(vec![egui::Event::Text("ABCDE".into())]);
    f.select(0, 2);
    let stale = f.app.rich_text_context().unwrap().selection_key;
    f.select(3, 5);
    let before = f.region().authored_text.clone();
    let history = f.app.undo.len();
    f.app.ribbon_command(
        &f.ctx,
        RibbonAction::SetTextUnderline {
            value: true,
            selection_key: stale,
        },
    );
    assert_eq!(f.region().authored_text, before);
    assert_eq!(f.app.undo.len(), history);
}

fn cold(path: &Path, plain: &str, xml: &str) -> String {
    let mut app = WorksheetApp {
        automatic_calculation: false,
        ..Default::default()
    };
    app.open(path);
    assert!(app.status.starts_with("Opened"), "{}", app.status);
    let region = app
        .document
        .regions
        .iter()
        .find(|r| matches!(&r.content, CanvasContent::Text(t) if t==plain))
        .unwrap();
    let owner = region.source_region.clone().unwrap_or_default();
    if region.authored_text.is_some() {
        assert_eq!(
            region.validated_authored_text().unwrap().unwrap().xaml(),
            xml
        );
    } else {
        let package = NativePackage::open(path).unwrap();
        let authority =
            mathcraft_document::export::read_native_text_authority(&package, &owner).unwrap();
        assert_eq!(authority.original_xaml(), xml);
    }
    assert!(app.last_executed_regions.is_empty() && !app.background.running);
    assert!(app.results.is_empty());
    owner
}

/// Six independent source observations, each single Text owner. Generated
/// caches are own policies, not evidence of Mathcad execution/serialization.
#[test]
#[ignore]
fn underline_artifact_probe() {
    use native_save_ui::SaveIntent;
    let output =
        PathBuf::from(std::env::var_os("MATHCRAFT_UNDERLINE_OUTPUT").expect("Fresh output path"));
    std::fs::create_dir(&output).expect("Output must not exist");
    let mut f = Fixture::new();
    f.frame(vec![egui::Event::Text("한글😀AB".into())]);
    f.select(1, 3);
    f.frame(vec![key(
        egui::Key::U,
        egui::Modifiers::CTRL | egui::Modifiers::COMMAND,
    )]);
    let mut states = Vec::new();
    let capture = |f: &mut Fixture, name: &str, states: &mut Vec<serde_json::Value>| {
        assert_eq!(
            f.app.document.regions.len(),
            1,
            "Actual source has one owner"
        );
        let plain = f.text().to_owned();
        let xml = f.xml();
        let own = format!("{name}.mathcraft");
        let native = format!("{name}.mcdx");
        let pdf = format!("{name}.pdf");
        assert!(
            f.app
                .save_to_destination(Some(output.join(&own)), SaveIntent::SaveAs),
            "{}",
            f.app.status
        );
        assert!(
            f.app
                .save_to_destination(Some(output.join(&native)), SaveIntent::SaveAs),
            "{}",
            f.app.status
        );
        cold(&output.join(&own), &plain, &xml);
        let owner = cold(&output.join(&native), &plain, &xml);
        // Open populates the same source-owned NativeVisuals used by printing.
        let mut print = WorksheetApp {
            automatic_calculation: false,
            ..Default::default()
        };
        print.open(&output.join(&native));
        print
            .export_vector_pdf_for_test(&output.join(&pdf))
            .unwrap();
        states.push(serde_json::json!({"name":name.strip_prefix("underline-").unwrap(),"own":own,"native":native,"pdf":pdf,
            "canvas_owner_id":f.id,"owner_id":owner,"native_owner_id":owner,
            "plain":plain,"expected_plain":plain,"current_xaml":xml,"current_xml":xml,
            "executed_regions":[],"background_running":false}));
    };
    capture(&mut f, "underline-selected", &mut states);
    command(&mut f, false);
    capture(&mut f, "underline-cleared", &mut states);
    f.app.undo();
    capture(&mut f, "underline-undo", &mut states);
    f.app.redo();
    capture(&mut f, "underline-redo", &mut states);
    f.select(1, 3);
    command(&mut f, true);
    let previous = f.id;
    f.frame(vec![key_event_t()]);
    f.id = f.app.selected.unwrap();
    assert_ne!(f.id, previous);
    // Exercise the real owner-removal command, not a filtered export projection.
    f.app.selected = Some(previous);
    f.app.remove_selected();
    f.app.selected = Some(f.id);
    f.app.focus_new = Some(f.id);
    f.frame(vec![]);
    assert_eq!(f.text(), "");
    capture(&mut f, "underline-inherited-empty", &mut states);
    f.select(0, 0);
    f.frame(vec![egui::Event::Text("다음U".into())]);
    assert_eq!(decoration(&f, 0, 3), Some(TextDecorations::Underline));
    capture(&mut f, "underline-inherited-typed", &mut states);
    std::fs::write(
        output.join("observation.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "version":1,"original_runtime":false,"gui":false,"states":states,
        }))
        .unwrap(),
    )
    .unwrap();
}
