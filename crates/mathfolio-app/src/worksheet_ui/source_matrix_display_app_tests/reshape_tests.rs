//! Full App events plus existing RibbonAction dispatch; no native dialogs/GUI.
//! Caret setup only navigates to a full AST address. No direct source mutation
//! or direct editor shape method substitutes for the tested App commands.
use super::*;

const LENGTH: [f64; 7] = [1., 0., 0., 0., 0., 0., 0.];

fn source(app: &WorksheetApp, owner: u64) -> &PortableTree {
    match &app
        .document
        .regions
        .iter()
        .find(|r| r.id == owner)
        .unwrap()
        .content
    {
        CanvasContent::Math(tree) => tree,
        _ => panic!("source definition is not typed"),
    }
}
fn matrix(tree: &PortableTree) -> &PortableTree {
    assert_eq!(tree.kind, NodeKind::Binary(BinaryOp::Define));
    assert!(matches!(tree.children[1].kind, NodeKind::Matrix { .. }));
    &tree.children[1]
}
fn shape(tree: &PortableTree) -> (usize, usize) {
    let NodeKind::Matrix { rows, cols, .. } = matrix(tree).kind else {
        unreachable!()
    };
    assert_eq!(matrix(tree).children.len(), rows * cols);
    (rows, cols)
}
fn window(tree: &PortableTree) -> ([usize; 2], [usize; 2]) {
    let NodeKind::Matrix {
        rows,
        cols,
        display,
    } = matrix(tree).kind
    else {
        unreachable!()
    };
    display.unwrap_or_default().window([rows, cols]).unwrap()
}
fn ids(app: &WorksheetApp) -> (u64, u64) {
    let mut definition = None;
    let mut query = None;
    for r in &app.document.regions {
        if let CanvasContent::Math(t) = &r.content {
            if matches!(t.kind, NodeKind::Binary(BinaryOp::Define)) {
                assert!(definition.replace(r.id).is_none());
            }
            if matches!(t.kind, NodeKind::Binary(BinaryOp::Evaluate)) {
                assert!(query.replace(r.id).is_none());
            }
        }
    }
    (definition.unwrap(), query.unwrap())
}
fn key(k: egui::Key, m: egui::Modifiers, pressed: bool) -> egui::Event {
    egui::Event::Key {
        key: k,
        physical_key: Some(k),
        pressed,
        repeat: false,
        modifiers: m,
    }
}
fn frame(app: &mut WorksheetApp, ctx: &egui::Context, tick: &mut u64, events: Vec<egui::Event>) {
    let mut events = events;
    for _ in 0..16 {
        *tick += 1;
        let mut raw = egui::RawInput {
            events,
            time: Some(*tick as f64 / 60.),
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1200., 900.))),
            ..Default::default()
        };
        eframe::App::raw_input_hook(app, ctx, &mut raw);
        ctx.run_ui(raw, |ui| app.app_ui_frame(ui))
            .drop_without_applying_deltas();
        if app.plot_input_queue.is_empty() {
            return;
        }
        events = vec![];
    }
    panic!("input FIFO did not drain");
}
fn press(
    app: &mut WorksheetApp,
    ctx: &egui::Context,
    tick: &mut u64,
    k: egui::Key,
    m: egui::Modifiers,
) {
    frame(app, ctx, tick, vec![key(k, m, true), key(k, m, false)]);
}
fn enter(app: &mut WorksheetApp, ctx: &egui::Context, tick: &mut u64, owner: u64) {
    let history = app.undo.len();
    press(app, ctx, tick, egui::Key::Enter, egui::Modifiers::NONE);
    assert!(!app.uncommitted_math.contains(&owner));
    assert_eq!(
        app.undo.len(),
        history,
        "Enter must not add identical source history"
    );
}
fn focus(
    app: &mut WorksheetApp,
    ctx: &egui::Context,
    tick: &mut u64,
    owner: u64,
    row: usize,
    col: usize,
    side: Direction,
) {
    app.selected = Some(owner);
    app.focus_new = Some(owner);
    app.insertion = None;
    frame(app, ctx, tick, vec![]);
    frame(app, ctx, tick, vec![]);
    let e = app.editors.get_mut(&owner).unwrap();
    let m = e.arena().get(e.root()).unwrap().children()[1];
    let NodeKind::Matrix { cols, .. } = e.arena().get(m).unwrap().kind() else {
        panic!()
    };
    let mut cell = e.arena().get(m).unwrap().children()[row * *cols + col];
    // Initial unit-bearing cells are binary expressions; navigate to their
    // number without rewriting labels or selecting the whole source matrix.
    while let Some(first) = e.arena().get(cell).unwrap().children().first() {
        cell = *first;
    }
    e.place_caret(cell, side).unwrap();
}
fn numeric(
    value: &Value,
    rows: usize,
    cols: usize,
    expected: &[f64],
    factor: f64,
    dimension: [f64; 7],
) {
    let Value::Matrix {
        rows: r,
        cols: c,
        elements,
    } = value
    else {
        panic!("complete matrix lost")
    };
    assert_eq!((*r, *c, elements.len()), (rows, cols, rows * cols));
    assert_eq!(expected.len(), rows * cols);
    for (cell, n) in elements.iter().zip(expected) {
        assert!((cell.value.re - n * factor).abs() < 1e-8);
        assert_eq!(cell.value.im, 0.);
        assert_eq!(cell.dimension, dimension);
    }
}
fn saved(app: &WorksheetApp, owner: u64, rows: usize, cols: usize, expected: &[f64], factor: f64) {
    let row = &app.saved_results[&owner];
    assert_eq!(row.status, SavedCalculationStatus::Synchronized);
    assert!(row.error.is_none());
    match row
        .numeric_source
        .as_ref()
        .expect("full numeric cache missing")
    {
        SavedNumericSource::EvaluatedValue(json) => numeric(
            &serde_json::from_str(json).unwrap(),
            rows,
            cols,
            expected,
            factor,
            [0.; 7],
        ),
        SavedNumericSource::NativeMathml(raw) => {
            let p = PreparedNativeMatrix::parse(raw).unwrap();
            assert_eq!(p.shape(), (rows, cols));
            let view = p
                .project(
                    MatrixWindow {
                        rows,
                        cols,
                        row_offset: 0,
                        col_offset: 0,
                    },
                    &MathFormat {
                        result_format: ResultFormat::Decimal,
                        precision: 0,
                        ..Default::default()
                    },
                )
                .unwrap();
            assert_eq!(view.cells.len(), rows * cols);
            assert_eq!(
                view.cells.iter().map(|c| c.plain()).collect::<Vec<_>>(),
                expected
                    .iter()
                    .map(|n| format!("{:.0}", n * factor))
                    .collect::<Vec<_>>()
            );
        }
        _ => panic!("unexpected numeric cache carrier"),
    }
}
fn seed(root: &Path, name: &str, rows: usize, cols: usize, size: [u32; 2], unit: bool) -> PathBuf {
    let mut cells = String::new();
    for col in 0..cols {
        for row in 0..rows {
            let n = row * cols + col + 1;
            if unit {
                cells.push_str(&format!("<ml:apply><ml:mult/><ml:real>{n}</ml:real><ml:id labels=\"UNIT\">m</ml:id></ml:apply>"));
            } else {
                cells.push_str(&format!("<ml:real>{n}</ml:real>"));
            }
        }
    }
    let mut d = CanvasDocument::default();
    for (top, body) in [
        (
            30.,
            format!(
                "<ml:define><ml:id labels=\"VARIABLE\">M</ml:id><ml:matrix rows=\"{rows}\" cols=\"{cols}\"><ml:display offset=\"0,0\" size=\"{},{}\"/>{cells}</ml:matrix></ml:define>",
                size[0], size[1]
            ),
        ),
        (
            520.,
            format!(
                "<ml:eval><ml:id labels=\"VARIABLE\">M</ml:id><ml:unitOverride>{}</ml:unitOverride></ml:eval>",
                if unit {
                    "<ml:id labels=\"UNIT\">cm</ml:id>"
                } else {
                    "<ml:placeholder/>"
                }
            ),
        ),
    ] {
        let owner = d.add_math(20., top);
        let r = d.regions.iter_mut().find(|r| r.id == owner).unwrap();
        r.content = CanvasContent::Math(tree(&body));
        r.format = MathFormat {
            explicit: true,
            result_format: ResultFormat::Decimal,
            precision: 0,
            ..Default::default()
        };
    }
    let path = root.join(format!("{name}-input.mcdx"));
    mathfolio_document::export::export_mcdx(&d, &path).unwrap();
    path
}
fn start(path: &Path) -> (WorksheetApp, egui::Context, u64, u64, u64) {
    let mut app = manual_open(path);
    let (owner, result) = ids(&app);
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    app.ribbon_command(&ctx, RibbonAction::Recalculate);
    assert!(app.results[&owner].error.is_none());
    assert!(app.results[&result].error.is_none());
    app.last_executed_regions.clear();
    app.undo.clear();
    app.redo.clear();
    (app, ctx, 0, owner, result)
}
fn ribbon(app: &mut WorksheetApp, ctx: &egui::Context, owner: u64, action: RibbonAction) {
    let n = app.undo.len();
    let generation = app.background.submitted_generation();
    app.ribbon_command(ctx, action);
    assert_eq!(
        app.undo.len(),
        n + 1,
        "accepted ribbon shape/edit is one transaction"
    );
    assert!(app.uncommitted_math.contains(&owner));
    assert!(app.last_executed_regions.is_empty());
    assert_eq!(app.background.submitted_generation(), generation);
}
fn undo_redo(
    app: &mut WorksheetApp,
    ctx: &egui::Context,
    tick: &mut u64,
    owner: u64,
    before: &PortableTree,
    after: &PortableTree,
) {
    press(app, ctx, tick, egui::Key::Z, egui::Modifiers::COMMAND);
    assert_eq!(source(app, owner), before);
    press(app, ctx, tick, egui::Key::Y, egui::Modifiers::COMMAND);
    assert_eq!(source(app, owner), after);
    assert!(app.last_executed_regions.is_empty());
}
fn cold(
    path: &Path,
    expected: &PortableTree,
    rows: usize,
    cols: usize,
    numbers: &[f64],
    factor: f64,
) {
    let mut app = WorksheetApp {
        automatic_calculation: false,
        ..Default::default()
    };
    app.open(path);
    let (owner, result) = ids(&app);
    assert_eq!(source(&app, owner), expected);
    saved(&app, result, rows, cols, numbers, factor);
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut tick = 0;
    let generation = app.background.submitted_generation();
    frame(&mut app, &ctx, &mut tick, vec![]);
    frame(&mut app, &ctx, &mut tick, vec![]);
    assert_eq!(source(&app, owner), expected);
    saved(&app, result, rows, cols, numbers, factor);
    assert!(!app.pending_regions.contains(&result));
    assert!(app.last_executed_regions.is_empty());
    assert_eq!(app.background.submitted_generation(), generation);
    assert!(!app.background.running);
}
fn persist(
    app: &mut WorksheetApp,
    root: &Path,
    name: &str,
    owner: u64,
    numbers: &[f64],
    unit: bool,
) {
    let expected = source(app, owner).clone();
    let (rows, cols) = shape(&expected);
    let factor = if unit { 100. } else { 1. };
    let own = root.join(format!("{name}.mathfolio"));
    app.path = Some(own.clone());
    assert!(app.save(false), "{}", app.status);
    cold(&own, &expected, rows, cols, numbers, factor);
    app.prepare_native_export().unwrap();
    let native = root.join(format!("{name}.mcdx"));
    mathfolio_document::export::export_mcdx(&app.document, &native).unwrap();
    let p = NativePackage::open(&native).unwrap();
    let raw = p.worksheet_xml().unwrap();
    let xml = roxmltree::Document::parse(&raw).unwrap();
    let m = xml
        .descendants()
        .find(|n| n.has_tag_name((ML, "matrix")))
        .unwrap();
    let standalone = worksheet_math::tree_to_mathml(matrix(&expected)).unwrap();
    // The standalone writer declares Math50 on its root; the saved fragment
    // inherits the same namespace from the worksheet. Compare all remaining
    // source bytes exactly after removing only that known root declaration.
    let root = format!("<ml:matrix xmlns:ml=\"{ML}\"");
    let expected_xml = format!(
        "<ml:matrix{}",
        standalone
            .strip_prefix(&root)
            .expect("standalone Math50 root")
    );
    assert_eq!(
        &raw[m.range()],
        expected_xml,
        "exact typed source writer, all full cells and display"
    );
    // The existing bridge canonically expands new AnnotatedIdentifier UNITs
    // to explicit NativeIdentifier fields on native reopen. No label is lost.
    let canonical = tree(&worksheet_math::tree_to_mathml(&expected).unwrap());
    cold(&native, &canonical, rows, cols, numbers, factor);
    assert!(app.last_executed_regions.is_empty());
}

#[test]
fn source_window_ribbon_row_column_shape_edits_enter_history_hidden_noop_and_cold_full_value() {
    let scratch = Scratch::new();
    let path = seed(&scratch.0, "shape", 4, 4, [3, 3], false);
    let original_bytes = std::fs::read(&path).unwrap();
    let (mut app, ctx, mut tick, owner, result) = start(&path);
    let original = source(&app, owner).clone();
    let numbers = (1..=16).map(f64::from).collect::<Vec<_>>();
    numeric(
        app.results[&result].value.as_ref().unwrap(),
        4,
        4,
        &numbers,
        1.,
        [0.; 7],
    );
    for (row_axis, insert, delete) in [
        (
            true,
            RibbonAction::InsertMatrixRow,
            RibbonAction::DeleteMatrixRow,
        ),
        (
            false,
            RibbonAction::InsertMatrixColumn,
            RibbonAction::DeleteMatrixColumn,
        ),
    ] {
        focus(&mut app, &ctx, &mut tick, owner, 0, 0, Direction::Left);
        let before = source(&app, owner).clone();
        ribbon(&mut app, &ctx, owner, insert);
        let inserted = source(&app, owner).clone();
        assert_eq!(shape(&inserted), if row_axis { (5, 4) } else { (4, 5) });
        assert!(matrix(&inserted).children[0].kind == NodeKind::Placeholder);
        enter(&mut app, &ctx, &mut tick, owner);
        undo_redo(&mut app, &ctx, &mut tick, owner, &before, &inserted);
        focus(&mut app, &ctx, &mut tick, owner, 0, 0, Direction::Right);
        // Retained cell after removal is a real numeric leaf in the window.
        ribbon(&mut app, &ctx, owner, delete);
        enter(&mut app, &ctx, &mut tick, owner);
        let removed = source(&app, owner).clone();
        assert_eq!(shape(&removed), (4, 4));
        assert_eq!(matrix(&removed).children, matrix(&original).children);
        undo_redo(&mut app, &ctx, &mut tick, owner, &inserted, &removed);
        numeric(
            app.results[&result].value.as_ref().unwrap(),
            4,
            4,
            &numbers,
            1.,
            [0.; 7],
        );
    }
    assert_eq!(window(source(&app, owner)), ([0, 0], [2, 2]));
    focus(&mut app, &ctx, &mut tick, owner, 1, 0, Direction::Right);
    let before = (
        source(&app, owner).clone(),
        app.editors[&owner].caret(),
        app.editors[&owner].arena().revision(),
        app.undo.len(),
    );
    app.ribbon_command(&ctx, RibbonAction::DeleteMatrixRow); // target row1 hidden after size2→1
    assert_eq!(
        (
            source(&app, owner).clone(),
            app.editors[&owner].caret(),
            app.editors[&owner].arena().revision(),
            app.undo.len()
        ),
        before
    );
    assert!(app.last_executed_regions.is_empty());
    app.ribbon_command(&ctx, RibbonAction::Recalculate);
    numeric(
        app.results[&result].value.as_ref().unwrap(),
        4,
        4,
        &numbers,
        1.,
        [0.; 7],
    );
    app.last_executed_regions.clear();
    persist(&mut app, &scratch.0, "shape-edited", owner, &numbers, false);
    assert_eq!(std::fs::read(path).unwrap(), original_bytes);
}

#[test]
fn source_unit_window_inserted_row_real_text_and_unit_menu_keep_hidden_si_and_cold_converted_cache()
{
    let scratch = Scratch::new();
    let path = seed(&scratch.0, "unit-row", 4, 4, [3, 4], true);
    let original_bytes = std::fs::read(&path).unwrap();
    let (mut app, ctx, mut tick, owner, result) = start(&path);
    let original = source(&app, owner).clone();
    let original_numbers = (1..=16).map(f64::from).collect::<Vec<_>>();
    focus(&mut app, &ctx, &mut tick, owner, 0, 0, Direction::Left);
    ribbon(&mut app, &ctx, owner, RibbonAction::InsertMatrixRow);
    assert_eq!(shape(source(&app, owner)), (5, 4));
    assert_eq!(window(source(&app, owner)), ([0, 0], [3, 4]));
    for col in 0..4 {
        focus(
            &mut app,
            &ctx,
            &mut tick,
            owner,
            0,
            col,
            Direction::Indeterminate,
        );
        frame(
            &mut app,
            &ctx,
            &mut tick,
            vec![egui::Event::Text((101 + col).to_string())],
        );
        ribbon(&mut app, &ctx, owner, RibbonAction::InsertUnit("m"));
    }
    let edited = source(&app, owner).clone();
    assert_eq!(
        &matrix(&edited).children[4..],
        &matrix(&original).children[..]
    );
    for (col, cell) in matrix(&edited).children[..4].iter().enumerate() {
        assert_eq!(cell.kind, NodeKind::Binary(BinaryOp::Scale));
        assert_eq!(
            cell.children[0].kind,
            NodeKind::Number((101 + col).to_string())
        );
        assert!(
            matches!(&cell.children[1].kind,NodeKind::AnnotatedIdentifier{name,labels} if name=="m"&&labels=="UNIT")
        );
    }
    enter(&mut app, &ctx, &mut tick, owner);
    numeric(
        app.results[&result].value.as_ref().unwrap(),
        4,
        4,
        &original_numbers,
        1.,
        LENGTH,
    );
    assert!(app.last_executed_regions.is_empty());
    // Undo/Redo the final explicit unit insertion without losing row metadata.
    press(
        &mut app,
        &ctx,
        &mut tick,
        egui::Key::Z,
        egui::Modifiers::COMMAND,
    );
    assert_eq!(window(source(&app, owner)), ([0, 0], [3, 4]));
    press(
        &mut app,
        &ctx,
        &mut tick,
        egui::Key::Y,
        egui::Modifiers::COMMAND,
    );
    assert_eq!(source(&app, owner), &edited);
    let numbers = [101., 102., 103., 104.]
        .into_iter()
        .chain(original_numbers)
        .collect::<Vec<_>>();
    app.ribbon_command(&ctx, RibbonAction::Recalculate);
    numeric(
        app.results[&result].value.as_ref().unwrap(),
        5,
        4,
        &numbers,
        1.,
        LENGTH,
    );
    numeric(
        app.results[&result].display_value.as_ref().unwrap(),
        5,
        4,
        &numbers,
        100.,
        [0.; 7],
    );
    app.last_executed_regions.clear();
    persist(
        &mut app,
        &scratch.0,
        "unit-row-edited",
        owner,
        &numbers,
        true,
    );
    assert_eq!(std::fs::read(path).unwrap(), original_bytes);
}

#[test]
fn source_explicit_full_window_last_tab_twelve_to_thirteen_is_full_factory_and_cold_full_payload() {
    let scratch = Scratch::new();
    let path = seed(&scratch.0, "last-tab", 12, 1, [12, 1], false);
    let original_bytes = std::fs::read(&path).unwrap();
    let (mut app, ctx, mut tick, owner, result) = start(&path);
    let original = source(&app, owner).clone();
    assert!(matches!(
        matrix(&original).kind,
        NodeKind::Matrix {
            display: Some(_),
            ..
        }
    ));
    focus(&mut app, &ctx, &mut tick, owner, 11, 0, Direction::Right);
    let generation = app.background.submitted_generation();
    press(
        &mut app,
        &ctx,
        &mut tick,
        egui::Key::Tab,
        egui::Modifiers::NONE,
    );
    assert!(app.uncommitted_math.contains(&owner));
    let grown = source(&app, owner).clone();
    assert!(matches!(
        matrix(&grown).kind,
        NodeKind::Matrix {
            rows: 13,
            cols: 1,
            display: None
        }
    ));
    assert_eq!(
        &matrix(&grown).children[..12],
        &matrix(&original).children[..]
    );
    assert_eq!(matrix(&grown).children[12].kind, NodeKind::Placeholder);
    assert_eq!(app.undo.len(), 1);
    frame(
        &mut app,
        &ctx,
        &mut tick,
        vec![egui::Event::Text("13".into())],
    );
    enter(&mut app, &ctx, &mut tick, owner);
    let expected = source(&app, owner).clone();
    assert_eq!(
        matrix(&expected).children[12].kind,
        NodeKind::Number("13".into())
    );
    assert!(app.last_executed_regions.is_empty());
    assert_eq!(app.background.submitted_generation(), generation);
    let old = (1..=12).map(f64::from).collect::<Vec<_>>();
    numeric(
        app.results[&result].value.as_ref().unwrap(),
        12,
        1,
        &old,
        1.,
        [0.; 7],
    );
    undo_redo(&mut app, &ctx, &mut tick, owner, &grown, &expected);
    let numbers = (1..=13).map(f64::from).collect::<Vec<_>>();
    app.ribbon_command(&ctx, RibbonAction::Recalculate);
    numeric(
        app.results[&result].value.as_ref().unwrap(),
        13,
        1,
        &numbers,
        1.,
        [0.; 7],
    );
    app.last_executed_regions.clear();
    persist(
        &mut app,
        &scratch.0,
        "last-tab-edited",
        owner,
        &numbers,
        false,
    );
    assert_eq!(std::fs::read(path).unwrap(), original_bytes);
}
