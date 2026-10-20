//! Literal authoring, explicit commit, scalar calculation, inert cold results.
use super::*;
fn tree(kind: NodeKind, children: Vec<PortableTree>) -> PortableTree {
    PortableTree { kind, children }
}
fn atom(kind: NodeKind) -> PortableTree {
    tree(kind, vec![])
}
fn eval(value: PortableTree) -> PortableTree {
    tree(
        NodeKind::Binary(BinaryOp::Evaluate),
        vec![value, atom(NodeKind::Placeholder)],
    )
}
fn call(name: &str, args: Vec<PortableTree>) -> PortableTree {
    let count = args.len();
    let mut children = vec![atom(NodeKind::AnnotatedIdentifier {
        name: name.into(),
        labels: "FUNCTION".into(),
    })];
    children.extend(args);
    tree(
        NodeKind::Call {
            argument_count: count,
        },
        children,
    )
}
fn native(app: &mut WorksheetApp, path: &Path) {
    app.prepare_native_export().unwrap();
    mathcraft_document::export::export_mcdx(&app.document, path).unwrap();
}
fn save(app: &mut WorksheetApp, path: &Path) {
    app.path = Some(path.to_owned());
    assert!(app.save(false), "{}", app.status);
}
fn text(app: &WorksheetApp, id: u64) -> String {
    app.results
        .get(&id)
        .and_then(|r| r.value.as_ref())
        .and_then(|v| {
            if let Value::String { value } = v {
                Some(value.to_string())
            } else {
                None
            }
        })
        .unwrap()
}
fn displayed(app: &WorksheetApp, id: u64) -> String {
    let style = &app
        .document
        .regions
        .iter()
        .find(|r| r.id == id)
        .unwrap()
        .format;
    if let Some(row) = app.results.get(&id)
        && let Some(value) = &row.value
    {
        return crate::result_display::format(value, style).cells[0].plain();
    }
    numeric_cache::display(&app.saved_results[&id], style)
        .unwrap()
        .cells[0]
        .plain()
}
fn run(output: &Path) {
    std::fs::create_dir_all(output).unwrap();
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let frame = super::super::tests::frame;
    let mut app = WorksheetApp {
        automatic_calculation: false,
        insertion: Some((38.4, 38.4)),
        ..Default::default()
    };
    let value = "LCC + 1 = \"A\" 한글";
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::Text(format!("s:=\"{value}"))],
    );
    let definition = app.selected.unwrap();
    assert!(app.last_executed_regions.is_empty());
    frame(&mut app, &ctx, vec![super::super::tests::enter()]);
    let definition_tree = match &app
        .document
        .regions
        .iter()
        .find(|r| r.id == definition)
        .unwrap()
        .content
    {
        CanvasContent::Math(t) => t,
        _ => panic!(),
    };
    assert_eq!(
        definition_tree.children[1].kind,
        NodeKind::StringLiteral(value.into())
    );
    let query = app.document.add_math(38.4, 160.);
    app.document
        .regions
        .iter_mut()
        .find(|r| r.id == query)
        .unwrap()
        .content = CanvasContent::Math(eval(call(
        "concat",
        vec![
            atom(NodeKind::Identifier("s".into())),
            atom(NodeKind::StringLiteral(" / done".into())),
        ],
    )));
    let empty = app.document.add_math(38.4, 220.);
    app.document
        .regions
        .iter_mut()
        .find(|r| r.id == empty)
        .unwrap()
        .content = CanvasContent::Math(eval(atom(NodeKind::StringLiteral(String::new()))));
    app.recalculate();
    assert_eq!(text(&app, query), format!("{value} / done"));
    assert_eq!(text(&app, empty), "");
    native(&mut app, &output.join("string-values.mcdx"));
    save(&mut app, &output.join("string-values.mathcraft"));
    let mut cold = WorksheetApp::default();
    cold.open(&output.join("string-values.mcdx"));
    assert!(cold.last_executed_regions.is_empty());
    assert_eq!(displayed(&cold, query), format!("“{value} / done”"));
    assert_eq!(displayed(&cold, empty), "“”");
    native(&mut cold, &output.join("string-values-copy.mcdx"));
    let original = NativePackage::open(&output.join("string-values.mcdx")).unwrap();
    let copy = NativePackage::open(&output.join("string-values-copy.mcdx")).unwrap();
    assert_eq!(
        original.original_part("mathcad/result.xml"),
        copy.original_part("mathcad/result.xml")
    );
    cold.automatic_calculation = false;
    cold.selected = Some(definition);
    cold.focus_new = Some(definition);
    cold.insertion = None;
    let cold_ctx = egui::Context::default();
    crate::configure(&cold_ctx);
    frame(&mut cold, &cold_ctx, vec![]);
    let editor = cold.editors.get_mut(&definition).unwrap();
    let rhs = editor.arena().get(editor.root()).unwrap().children()[1];
    editor
        .place_text_caret(rhs, value.encode_utf16().count())
        .unwrap();
    let undo = cold.undo.len();
    let generation = cold.background.submitted_generation();
    frame(&mut cold, &cold_ctx, vec![egui::Event::Text("!".into())]);
    assert!(cold.last_executed_regions.is_empty());
    assert_eq!(cold.background.submitted_generation(), generation);
    // Ordinary worksheet math records the text command immediately; calculation
    // remains committed to the previous value until Enter. Plot drafts have a
    // different owner-level journal and must not define this history contract.
    assert_eq!(cold.undo.len(), undo + 1);
    assert!(cold.uncommitted_math.contains(&definition));
    assert_eq!(displayed(&cold, query), format!("“{value} / done”"));
    frame(&mut cold, &cold_ctx, vec![super::super::tests::enter()]);
    assert_eq!(cold.undo.len(), undo + 1);
    assert!(!cold.uncommitted_math.contains(&definition));
    assert!(cold.pending_regions.contains(&query));
    assert!(cold.last_executed_regions.is_empty());
    save(&mut cold, &output.join("string-values-pending.mathcraft"));
    let mut pending = WorksheetApp::default();
    pending.open(&output.join("string-values-pending.mathcraft"));
    assert!(pending.last_executed_regions.is_empty());
    assert_eq!(displayed(&pending, query), format!("“{value} / done”"));
    pending.recalculate();
    assert_eq!(text(&pending, query), format!("{value}! / done"));
    native(&mut pending, &output.join("string-values-edited.mcdx"));
    let mut reopened = WorksheetApp::default();
    reopened.open(&output.join("string-values-edited.mcdx"));
    assert!(reopened.last_executed_regions.is_empty());
    assert_eq!(displayed(&reopened, query), format!("“{value}! / done”"));
    let mut snapshot = reopened.print_snapshot();
    let plan = snapshot.plan(PageSelection::All, 300).unwrap();
    let pages = printing::render_vector_pages(&plan, |ui, p, t| snapshot.paint(ui, p, t)).unwrap();
    printing::write_pdf(&output.join("string-values.pdf"), &pages).unwrap();
    println!(
        "STRING_FLOW real_quote_operator_text=true enter_commit=true native_own_cold_execution=0 empty_distinct=true pending_previous=true fresh_string_result=true vector_pdf=true"
    );
}
#[test]
fn string_authoring_commit_calculation_native_cache_reopen_and_vector_print() {
    let path = std::env::temp_dir().join(format!(
        "mathcraft-string-values-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    run(&path);
    for entry in std::fs::read_dir(&path).unwrap() {
        std::fs::remove_file(entry.unwrap().path()).unwrap();
    }
    std::fs::remove_dir(path).unwrap();
}
#[test]
#[ignore = "Explicit scalar STRING authoring/native/cold/vector-PDF probe"]
fn string_values_save_native_export_reopen_probe() {
    run(&PathBuf::from(
        std::env::var_os("MATHCRAFT_STRING_VALUES_OUTPUT").expect("MATHCRAFT_STRING_VALUES_OUTPUT"),
    ));
}
