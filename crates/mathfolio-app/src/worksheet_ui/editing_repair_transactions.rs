//! Cross-feature native fixture built by actual CPU editor Key/Text events.
//! Explicit source/value oracles cover formatted repair and Typeover replacement.
use super::*;
use mathfolio_editor::InsertionMode;

fn node(kind: NodeKind, children: Vec<PortableTree>) -> PortableTree {
    PortableTree { kind, children }
}
fn number(value: &str) -> PortableTree {
    node(NodeKind::Number(value.into()), vec![])
}
fn blank() -> PortableTree {
    node(NodeKind::Placeholder, vec![])
}
fn native(
    name: &str,
    labels: &str,
    contextual: Option<bool>,
    suffix: Option<&str>,
) -> PortableTree {
    node(
        NodeKind::NativeIdentifier {
            name: name.into(),
            labels: labels.into(),
            contextual,
            subscript: suffix.map(str::to_owned),
        },
        vec![],
    )
}
fn binary(kind: NodeKind, left: PortableTree, right: PortableTree) -> PortableTree {
    node(kind, vec![left, right])
}
fn add_tree(left: PortableTree, right: PortableTree) -> PortableTree {
    binary(NodeKind::Binary(BinaryOp::Add), left, right)
}
fn eval(value: PortableTree) -> PortableTree {
    binary(NodeKind::Binary(BinaryOp::Evaluate), value, blank())
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
fn text(value: &str) -> egui::Event {
    egui::Event::Text(value.into())
}
fn start(source: &PortableTree) -> (egui::Context, Editor) {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut editor = Editor::from_portable(source).unwrap();
    ctx.run_ui(Default::default(), |ui| {
        crate::formula::formula_ui_with_actions(ui, &mut editor, None, true);
    })
    .drop_without_applying_deltas();
    (ctx, editor)
}
fn event(ctx: &egui::Context, editor: &mut Editor, event: egui::Event) -> bool {
    let mut changed = false;
    ctx.run_ui(
        egui::RawInput {
            events: vec![event],
            ..Default::default()
        },
        |ui| {
            let response = crate::formula::formula_ui_with_actions(ui, editor, None, true);
            changed = response.response.changed();
            assert!(
                !response.evaluation_requested,
                "plain source editing must not calculate"
            );
        },
    )
    .drop_without_applying_deltas();
    assert!(editor.validate().is_ok());
    changed
}
fn enter_red(ctx: &egui::Context, editor: &mut Editor, source: &PortableTree) {
    editor.place_caret(editor.root(), Direction::Right).unwrap();
    let origin = editor.caret();
    let revision = editor.arena().revision();
    assert!(!event(ctx, editor, key(egui::Key::Backspace)));
    assert_eq!(editor.typeover_target(), Some(editor.root()));
    assert_eq!(editor.to_portable().unwrap(), *source);
    assert_eq!(editor.caret(), origin);
    assert_eq!(editor.arena().revision(), revision);
    assert!(
        !editor.undo(),
        "first red selection must not create an Undo entry"
    );
}

struct Query {
    label: &'static str,
    source: PortableTree,
    expected: (f64, f64),
}
fn query(label: &'static str, editor: Editor, oracle: PortableTree, expected: (f64, f64)) -> Query {
    let source = editor.to_portable().unwrap();
    assert_eq!(source, oracle, "independent source oracle: {label}");
    assert_eq!(editor.typeover_target(), None);
    Query {
        label,
        source,
        expected,
    }
}
fn edited_queries() -> Vec<Query> {
    let mut rows = vec![];
    let source = add_tree(
        native("A", "VARIABLE", None, None),
        native("B", "UNIT", Some(true), Some("dc")),
    );
    let (ctx, mut editor) = start(&source);
    editor
        .place_operator_caret(editor.root(), Direction::Indeterminate)
        .unwrap();
    assert!(event(&ctx, &mut editor, key(egui::Key::Backspace)));
    rows.push(query(
        "explicit-left/right-suffix fusion",
        editor,
        native("AB", "VARIABLE", None, Some("dc")),
        (7., 0.),
    ));

    let source = add_tree(
        native("A", "UNIT", Some(true), None),
        node(NodeKind::Identifier("B".into()), vec![]),
    );
    let (ctx, mut editor) = start(&source);
    editor
        .place_operator_caret(editor.root(), Direction::Indeterminate)
        .unwrap();
    assert!(event(&ctx, &mut editor, key(egui::Key::Delete)));
    rows.push(query(
        "contextual-left cache discarded",
        editor,
        node(NodeKind::Identifier("AB".into()), vec![]),
        (9., 0.),
    ));

    let source = native("V", "VARIABLE", None, Some("d"));
    let (ctx, mut editor) = start(&source);
    editor
        .place_subscript_selection(editor.root(), 1, 1)
        .unwrap();
    assert!(event(&ctx, &mut editor, key(egui::Key::Backspace)));
    assert_eq!(
        editor.to_portable().unwrap(),
        native("V", "VARIABLE", None, None)
    );
    assert_eq!(editor.caret().insertion, InsertionMode::Subscript);
    assert!(event(&ctx, &mut editor, text("c")));
    rows.push(query(
        "pending literal suffix re-entry",
        editor,
        native("V", "VARIABLE", None, Some("c")),
        (5., 0.),
    ));

    let source = native("Voltage", "VARIABLE", None, Some("dc"));
    let (ctx, mut editor) = start(&source);
    editor
        .place_subscript_selection(editor.root(), 0, 0)
        .unwrap();
    assert!(event(&ctx, &mut editor, key(egui::Key::Backspace)));
    rows.push(query(
        "suffix start edits preceding base",
        editor,
        native("Voltag", "VARIABLE", None, Some("dc")),
        (6., 0.),
    ));

    let imaginary = node(
        NodeKind::ImaginarySymbol {
            value: "12".into(),
            symbol: "j".into(),
        },
        vec![],
    );
    let (ctx, mut editor) = start(&imaginary);
    editor.place_caret(editor.root(), Direction::Right).unwrap();
    assert!(event(&ctx, &mut editor, key(egui::Key::Backspace)));
    rows.push(query(
        "imaginary symbol deletion",
        editor,
        number("12"),
        (12., 0.),
    ));

    let source = add_tree(
        number("1"),
        node(
            NodeKind::ImaginarySymbol {
                value: "2".into(),
                symbol: "j".into(),
            },
            vec![],
        ),
    );
    let (ctx, mut editor) = start(&source);
    editor
        .place_operator_caret(editor.root(), Direction::Indeterminate)
        .unwrap();
    assert!(event(&ctx, &mut editor, key(egui::Key::Backspace)));
    rows.push(query(
        "imaginary full lexeme fusion",
        editor,
        imaginary,
        (0., 12.),
    ));

    let typeover_term = binary(NodeKind::Fraction, number("12"), number("2"));
    let (ctx, mut editor) = start(&typeover_term);
    enter_red(&ctx, &mut editor, &typeover_term);
    assert!(event(&ctx, &mut editor, key(egui::Key::Delete)));
    assert_eq!(editor.to_portable().unwrap(), blank());
    assert_eq!(editor.caret().caret_side, Direction::Left);
    assert!(event(&ctx, &mut editor, text("5")));
    rows.push(query(
        "second delete then value",
        editor,
        number("5"),
        (5., 0.),
    ));

    let fraction = binary(NodeKind::Fraction, number("12"), number("2"));
    let (ctx, mut editor) = start(&fraction);
    enter_red(&ctx, &mut editor, &fraction);
    assert!(event(&ctx, &mut editor, text("24")));
    rows.push(query(
        "red fraction text replacement",
        editor,
        number("24"),
        (24., 0.),
    ));

    let (ctx, mut editor) = start(&typeover_term);
    enter_red(&ctx, &mut editor, &typeover_term);
    assert!(event(&ctx, &mut editor, text("%")));
    assert!(event(&ctx, &mut editor, text("50")));
    rows.push(query(
        "red percent template and operand",
        editor,
        node(NodeKind::Percent, vec![number("50")]),
        (0.5, 0.),
    ));

    let (ctx, mut editor) = start(&typeover_term);
    enter_red(&ctx, &mut editor, &typeover_term);
    assert!(event(&ctx, &mut editor, text("(")));
    assert!(event(&ctx, &mut editor, text("7")));
    assert!(!event(&ctx, &mut editor, text(")")));
    rows.push(query(
        "red grouping template and close navigation",
        editor,
        node(NodeKind::Group, vec![number("7")]),
        (7., 0.),
    ));

    let (ctx, mut editor) = start(&typeover_term);
    enter_red(&ctx, &mut editor, &typeover_term);
    assert!(event(&ctx, &mut editor, text("+")));
    assert!(event(&ctx, &mut editor, text("2")));
    assert!(!event(&ctx, &mut editor, key(egui::Key::Tab)));
    assert!(event(&ctx, &mut editor, text("3")));
    rows.push(query(
        "red infix template both operands",
        editor,
        add_tree(number("2"), number("3")),
        (5., 0.),
    ));

    let (ctx, mut editor) = start(&typeover_term);
    enter_red(&ctx, &mut editor, &typeover_term);
    assert!(event(&ctx, &mut editor, text("!")));
    assert!(event(&ctx, &mut editor, text("5")));
    rows.push(query(
        "red factorial template operand",
        editor,
        node(NodeKind::Factorial, vec![number("5")]),
        (120., 0.),
    ));

    let (ctx, mut editor) = start(&typeover_term);
    enter_red(&ctx, &mut editor, &typeover_term);
    assert!(event(&ctx, &mut editor, text("-")));
    assert!(event(&ctx, &mut editor, text("3")));
    rows.push(query(
        "red minus prefix template operand",
        editor,
        node(NodeKind::UnaryNeg, vec![number("3")]),
        (-3., 0.),
    ));
    assert_eq!(rows.len(), 13);
    rows
}
fn add(app: &mut WorksheetApp, y: f64, tree: PortableTree) -> u64 {
    let id = app.document.add_math(20., y);
    app.document.regions.last_mut().unwrap().content = CanvasContent::Math(tree.clone());
    app.editors
        .insert(id, Editor::from_portable(&tree).unwrap());
    id
}
type Expected = (u64, &'static str, (f64, f64));
fn fixture() -> (WorksheetApp, Vec<Expected>) {
    let mut app = WorksheetApp::default();
    for (index, (name, suffix, value)) in [
        ("AB", Some("dc"), "7"),
        ("AB", None, "9"),
        ("V", Some("c"), "5"),
        ("Voltag", Some("dc"), "6"),
    ]
    .into_iter()
    .enumerate()
    {
        add(
            &mut app,
            20. + index as f64 * 140.,
            binary(
                NodeKind::Binary(BinaryOp::Define),
                native(name, "VARIABLE", None, suffix),
                number(value),
            ),
        );
    }
    let expected = edited_queries()
        .into_iter()
        .enumerate()
        .map(|(index, row)| {
            let id = add(&mut app, 580. + index as f64 * 140., eval(row.source));
            (id, row.label, row.expected)
        })
        .collect();
    (app, expected)
}
fn verify(app: &WorksheetApp, expected: &[Expected]) {
    assert_eq!(app.document.regions.len(), 17);
    assert_eq!(app.results.len(), 17);
    assert!(app.results.values().all(|result| result.error.is_none()));
    for &(id, label, (re, im)) in expected {
        let result = &app.results[&id];
        assert!(result.error.is_none(), "{label}: {result:?}");
        let Some(Value::Scalar { quantity }) = &result.value else {
            panic!("{label}: {result:?}")
        };
        assert_eq!(quantity.value.re, re, "{label}");
        assert_eq!(quantity.value.im, im, "{label}");
    }
}

#[test]
fn editing_repair_actual_keys_match_independent_source_and_complex_value_oracles() {
    let (mut app, expected) = fixture();
    app.recalculate();
    verify(&app, &expected);
}

#[test]
#[ignore = "Explicit formatted/Typeover native/PDF integration fixture; no GUI or original runtime"]
fn editing_repair_save_native_export_reopen_probe() {
    let output = PathBuf::from(
        std::env::var_os("MATHFOLIO_EDITING_REPAIR_OUTPUT")
            .expect("MATHFOLIO_EDITING_REPAIR_OUTPUT"),
    );
    std::fs::create_dir_all(&output).unwrap();
    let (mut app, expected) = fixture();
    app.recalculate();
    verify(&app, &expected);
    app.persist_calculation_cache();
    let saved = output.join("editing-repair.mathfolio");
    let native = output.join("editing-repair.mcdx");
    app.document.save(&saved).unwrap();
    mathfolio_document::export::export_mcdx(&app.document, &native).unwrap();
    let mut reopened = WorksheetApp::default();
    reopened.open(&saved);
    assert_eq!(reopened.document.regions.len(), 17);
    assert!(reopened.last_executed_regions.is_empty() && reopened.pending_regions.is_empty());
    assert_eq!(
        reopened
            .saved_results
            .values()
            .filter(|row| row.display.is_some())
            .count(),
        13
    );
    for &(id, label, _) in &expected {
        assert!(
            reopened
                .saved_results
                .get(&id)
                .is_some_and(|row| row.display.is_some()),
            "saved query display missing: {label}"
        );
    }
    let mut imported = WorksheetApp::default();
    imported.open(&native);
    imported.recalculate();
    verify(&imported, &expected);
    imported.persist_calculation_cache();
    imported
        .document
        .save(&output.join("editing-repair-reimported.mathfolio"))
        .unwrap();
    println!(
        "EDITING_REPAIR_NATIVE regions=17 saved_query_displays=13 saved_reopen_execution=0 saved_reopen_pending=0 native_recalculate_errors=0"
    );
}
