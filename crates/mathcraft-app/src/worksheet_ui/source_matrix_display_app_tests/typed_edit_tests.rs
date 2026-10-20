//! Actual full-App keyboard/Enter/history and cold persistence. No GUI/runtime.
//! Caret setup selects a FULL-AST address inside the imported source window;
//! source viewport pixel layout/pointer navigation belongs to a separate suite.
use super::*;

const EDITED: [f64; 6] = [1., 20., 3., 4., 5., 6.];
const LENGTH: [f64; 7] = [1., 0., 0., 0., 0., 0., 0.];

fn source(app: &WorksheetApp, owner: u64) -> &PortableTree {
    let content = &app
        .document
        .regions
        .iter()
        .find(|r| r.id == owner)
        .unwrap()
        .content;
    let CanvasContent::Math(tree) = content else {
        panic!("definition is not typed")
    };
    tree
}
fn source_matrix_tree(tree: &PortableTree) -> &PortableTree {
    assert_eq!(tree.kind, NodeKind::Binary(BinaryOp::Define));
    let matrix = &tree.children[1];
    let NodeKind::Matrix {
        rows,
        cols,
        display: Some(display),
    } = matrix.kind
    else {
        panic!("source metadata missing")
    };
    assert_eq!((rows, cols, matrix.children.len()), (2, 3, 6));
    assert_eq!(display.offset(), Some([0, 1]));
    assert_eq!(display.size(), Some([1, 2]));
    assert_eq!(display.window([rows, cols]).unwrap(), ([0, 1], [1, 2]));
    matrix
}
fn assert_number(tree: &PortableTree, expected: &str) {
    let NodeKind::Number(text) = &tree.kind else {
        panic!("not a number")
    };
    assert_eq!(text, expected);
}
fn full_value(value: &Value, expected: &[f64; 6], factor: f64, dimension: [f64; 7]) {
    let Value::Matrix {
        rows,
        cols,
        elements,
    } = value
    else {
        panic!("full matrix value lost")
    };
    assert_eq!((*rows, *cols, elements.len()), (2, 3, 6));
    for (cell, n) in elements.iter().zip(expected) {
        assert!((cell.value.re - n * factor).abs() < 1e-9);
        assert_eq!(cell.value.im, 0.);
        assert_eq!(cell.dimension, dimension);
    }
}
fn key(key: egui::Key, modifiers: egui::Modifiers, pressed: bool) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed,
        repeat: false,
        modifiers,
    }
}
fn full_frame(
    app: &mut WorksheetApp,
    ctx: &egui::Context,
    tick: &mut u64,
    events: Vec<egui::Event>,
) {
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
    panic!("bounded input FIFO did not drain");
}
fn command(app: &mut WorksheetApp, ctx: &egui::Context, tick: &mut u64, k: egui::Key) {
    full_frame(
        app,
        ctx,
        tick,
        vec![
            key(k, egui::Modifiers::COMMAND, true),
            key(k, egui::Modifiers::COMMAND, false),
        ],
    );
}

/// Public ZIP fixture rebuilding only; every existing package part is retained.
/// All six source cells carry explicit UNIT m, and the query requests cm. This
/// preserves a full physical matrix while the existing saved numeric carrier
/// is the full, converted dimensionless display matrix (not one united cell).
fn unit_fixture(root: &Path) -> PathBuf {
    let path = fixture(root, None);
    let package = NativePackage::open(&path).unwrap();
    let raw = package.worksheet_xml().unwrap();
    let xml = roxmltree::Document::parse(&raw).unwrap();
    let matrix = xml
        .descendants()
        .find(|n| n.has_tag_name((ML, "matrix")))
        .unwrap();
    let mut patches: Vec<_> = matrix.children().filter(|n| n.is_element() && !n.has_tag_name((ML, "display")))
        .map(|n| (n.range(), format!("<ml:apply><ml:mult/><ml:real>{}</ml:real><ml:id labels=\"UNIT\">m</ml:id></ml:apply>", n.text().unwrap())))
        .collect();
    assert_eq!(patches.len(), 6);
    let output = xml
        .descendants()
        .find(|n| n.has_tag_name((ML, "unitOverride")))
        .unwrap();
    patches.push((
        output.range(),
        "<ml:unitOverride><ml:id labels=\"UNIT\">cm</ml:id></ml:unitOverride>".to_owned(),
    ));
    patches.sort_by_key(|p| std::cmp::Reverse(p.0.start));
    let mut changed = raw.clone();
    for (range, text) in patches {
        changed.replace_range(range, &text);
    }
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for part in package.parts() {
        writer
            .start_file(
                &part.name,
                zip::write::SimpleFileOptions::default().compression_method(part.compression),
            )
            .unwrap();
        writer
            .write_all(if part.name == package.worksheet_part_name() {
                changed.as_bytes()
            } else {
                part.bytes()
            })
            .unwrap();
    }
    let output = root.join("typed-edit-metres.mcdx");
    NativePackage::from_bytes(&writer.finish().unwrap().into_inner())
        .unwrap()
        .write_new(&output)
        .unwrap();
    output
}

fn saved_full(app: &WorksheetApp, owner: u64, unit: bool) {
    let row = &app.saved_results[&owner];
    assert_eq!(row.status, SavedCalculationStatus::Synchronized);
    assert!(row.error.is_none());
    let factor = if unit { 100. } else { 1. };
    match row
        .numeric_source
        .as_ref()
        .expect("full numeric payload missing")
    {
        SavedNumericSource::EvaluatedValue(json) => full_value(
            &serde_json::from_str(json).unwrap(),
            &EDITED,
            factor,
            [0.; 7],
        ),
        SavedNumericSource::NativeMathml(raw) => {
            let prepared = PreparedNativeMatrix::parse(raw).unwrap();
            assert_eq!(prepared.shape(), (2, 3));
            let projection = prepared
                .project(
                    MatrixWindow {
                        rows: 2,
                        cols: 3,
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
            assert_eq!(
                projection
                    .cells
                    .iter()
                    .map(|c| c.plain())
                    .collect::<Vec<_>>(),
                EDITED.map(|n| format!("{:.0}", n * factor))
            );
        }
        _ => panic!("unexpected numeric source carrier"),
    }
    let preview = FormattedResult::from_saved(row.display.as_deref().unwrap()).unwrap();
    assert_eq!((preview.rows, preview.cols, preview.cells.len()), (2, 3, 6));
    assert_eq!(
        preview.cells.iter().map(|c| c.plain()).collect::<Vec<_>>(),
        EDITED.map(|n| format!("{:.0}", n * factor))
    );
}
fn cold(path: &Path, expected: &PortableTree, unit: bool) -> WorksheetApp {
    let mut app = WorksheetApp {
        automatic_calculation: false,
        ..Default::default()
    };
    app.open(path);
    assert_eq!(source(&app, definition(&app)), expected);
    source_matrix_tree(source(&app, definition(&app)));
    saved_full(&app, query(&app), unit);
    assert!(!app.pending_regions.contains(&query(&app)));
    assert!(app.last_executed_regions.is_empty());
    let generation = app.background.submitted_generation();
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut tick = 0;
    full_frame(&mut app, &ctx, &mut tick, vec![]);
    full_frame(&mut app, &ctx, &mut tick, vec![]);
    assert_eq!(source(&app, definition(&app)), expected);
    saved_full(&app, query(&app), unit);
    assert!(app.last_executed_regions.is_empty());
    assert_eq!(app.background.submitted_generation(), generation);
    assert!(!app.background.running);
    app
}

fn exercise(unit: bool) {
    let scratch = Scratch::new();
    let path = if unit {
        unit_fixture(&scratch.0)
    } else {
        fixture(&scratch.0, None)
    };
    let original_bytes = std::fs::read(&path).unwrap();
    let package = NativePackage::open(&path).unwrap();
    let original_xml = source_matrix(&package);
    let mut app = manual_open(&path);
    let owner = definition(&app);
    let result_owner = query(&app);
    let original = source(&app, owner).clone();
    source_matrix_tree(&original);
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    // One independent baseline setup calculation; all UI editing/history below
    // must preserve its old curve/value until explicit Recalculate.
    app.ribbon_command(&ctx, RibbonAction::Recalculate);
    full_value(
        app.results[&result_owner].value.as_ref().unwrap(),
        &[1., 2., 3., 4., 5., 6.],
        1.,
        if unit { LENGTH } else { [0.; 7] },
    );
    app.last_executed_regions.clear();
    app.undo.clear();
    app.redo.clear();
    app.selected = Some(owner);
    app.focus_new = Some(owner);
    app.insertion = None;
    let mut tick = 0;
    full_frame(&mut app, &ctx, &mut tick, vec![]);
    full_frame(&mut app, &ctx, &mut tick, vec![]);
    let editor = app
        .editors
        .get_mut(&owner)
        .expect("typed imported formula editor missing");
    let matrix = editor.arena().get(editor.root()).unwrap().children()[1];
    let cell = editor.arena().get(matrix).unwrap().children()[1]; // full row0,col1, visible
    let number = if unit {
        editor.arena().get(cell).unwrap().children()[0]
    } else {
        cell
    };
    assert!(
        matches!(editor.arena().get(number).unwrap().kind(), NodeKind::Number(text) if text == "2")
    );
    editor.place_caret(number, Direction::Right).unwrap(); // navigation setup only
    let generation = app.background.submitted_generation();
    full_frame(
        &mut app,
        &ctx,
        &mut tick,
        vec![
            key(egui::Key::Num0, egui::Modifiers::NONE, true),
            egui::Event::Text("0".into()),
            key(egui::Key::Num0, egui::Modifiers::NONE, false),
        ],
    );
    let edited = app.editors[&owner].to_portable().unwrap();
    let mut expected = original.clone();
    let expected_cell = &mut expected.children[1].children[1];
    if unit {
        expected_cell.children[0].kind = NodeKind::Number("20".into());
    } else {
        expected_cell.kind = NodeKind::Number("20".into());
    }
    assert_eq!(edited, expected);
    assert_number(
        if unit {
            &edited.children[1].children[1].children[0]
        } else {
            &edited.children[1].children[1]
        },
        "20",
    );
    assert_eq!(
        source_matrix_tree(&edited).kind,
        source_matrix_tree(&original).kind
    );
    for i in [0, 2, 3, 4, 5] {
        assert_eq!(
            edited.children[1].children[i], original.children[1].children[i],
            "hidden/full source cell {i} changed"
        );
    }
    assert!(app.uncommitted_math.contains(&owner));
    assert!(app.last_executed_regions.is_empty());
    assert_eq!(app.background.submitted_generation(), generation);
    full_value(
        app.results[&result_owner].value.as_ref().unwrap(),
        &[1., 2., 3., 4., 5., 6.],
        1.,
        if unit { LENGTH } else { [0.; 7] },
    );
    assert_eq!(app.undo.len(), 1);
    full_frame(
        &mut app,
        &ctx,
        &mut tick,
        vec![
            key(egui::Key::Enter, egui::Modifiers::NONE, true),
            key(egui::Key::Enter, egui::Modifiers::NONE, false),
        ],
    );
    assert!(!app.uncommitted_math.contains(&owner));
    assert_eq!(source(&app, owner), &expected);
    assert_eq!(
        app.undo.len(),
        1,
        "Enter must not add a source-identical history item"
    );
    assert!(app.last_executed_regions.is_empty());
    full_value(
        app.results[&result_owner].value.as_ref().unwrap(),
        &[1., 2., 3., 4., 5., 6.],
        1.,
        if unit { LENGTH } else { [0.; 7] },
    );
    command(&mut app, &ctx, &mut tick, egui::Key::Z);
    assert_eq!(source(&app, owner), &original);
    source_matrix_tree(source(&app, owner));
    assert_eq!(app.redo.len(), 1);
    command(&mut app, &ctx, &mut tick, egui::Key::Y);
    assert_eq!(source(&app, owner), &expected);
    source_matrix_tree(source(&app, owner));
    assert!(app.last_executed_regions.is_empty());
    app.ribbon_command(&ctx, RibbonAction::Recalculate);
    full_value(
        app.results[&result_owner].value.as_ref().unwrap(),
        &EDITED,
        1.,
        if unit { LENGTH } else { [0.; 7] },
    );
    if unit {
        full_value(
            app.results[&result_owner].display_value.as_ref().unwrap(),
            &EDITED,
            100.,
            [0.; 7],
        );
    }
    app.last_executed_regions.clear();
    let own = scratch.0.join("typed-edited.mathcraft");
    app.path = Some(own.clone());
    assert!(app.save(false), "{}", app.status);
    let mut own_cold = cold(&own, &expected, unit);
    let generation = own_cold.background.submitted_generation();
    own_cold.prepare_native_export().unwrap();
    let native = scratch.0.join("typed-edited.mcdx");
    mathcraft_document::export::export_mcdx(&own_cold.document, &native).unwrap();
    let written = NativePackage::open(&native).unwrap();
    let mut expected_xml =
        original_xml.replacen("<ml:real>2</ml:real>", "<ml:real>20</ml:real>", 1);
    if unit {
        // The fixture ZIP patch deliberately omitted xml:space. A real source
        // edit uses the existing typed identifier writer's canonical spelling;
        // the unchanged original archive is checked separately below.
        let original_unit = "<ml:id labels=\"UNIT\">m</ml:id>";
        assert_eq!(expected_xml.matches(original_unit).count(), 6);
        expected_xml = expected_xml.replace(
            original_unit,
            "<ml:id labels=\"UNIT\" xml:space=\"preserve\">m</ml:id>",
        );
    }
    assert_eq!(source_matrix(&written), expected_xml);
    assert!(own_cold.last_executed_regions.is_empty());
    assert_eq!(own_cold.background.submitted_generation(), generation);
    let native_cold = cold(&native, &expected, unit);
    assert_eq!(
        std::fs::read(&path).unwrap(),
        original_bytes,
        "input archive was rewritten"
    );
    assert_eq!(
        native_cold.document.native_archive.as_deref(),
        Some(std::fs::read(&native).unwrap().as_slice())
    );
}

#[test]
fn imported_typed_source_window_real_cell_text_enter_history_and_full_cold_cache() {
    exercise(false);
}

#[test]
fn imported_typed_unit_cell_text_keeps_hidden_units_full_si_and_converted_cold_cache() {
    exercise(true);
}
