//! Original Copy-before-Delete ordering, with our bounded hidden-target refusal.
//! This is a compatibility control, not a request to make clipboard transactional.
use super::*;
use mathcraft_editor::SourceMatrixDisplay;

#[test]
fn source_window_cut_publishes_full_rectangle_before_rejected_delete_without_history_change() {
    let source = PortableTree {
        kind: NodeKind::Matrix {
            rows: 10,
            cols: 4,
            display: Some(SourceMatrixDisplay::from_parts(Some([2, 0]), Some([3, 4])).unwrap()),
        },
        children: (0..40)
            .map(|value| PortableTree {
                kind: NodeKind::Number(value.to_string()),
                children: vec![],
            })
            .collect(),
    };
    let mut editor = Editor::from_portable(&source).unwrap();
    let first = editor.arena().get(editor.root()).unwrap().children()[0];
    editor
        .set_atom(first, NodeKind::Number("777".into()))
        .unwrap();
    let redo_source = editor.to_portable().unwrap();
    assert!(editor.undo());
    // Last visible complete row4 is a genuine visible rectangle. Its deletion
    // would shrink size3->2 while targetrow4 falls outside new rows[2,4).
    assert!(editor.select_matrix_cells(editor.root(), 16, 19));
    let context = egui::Context::default();
    crate::configure(&context);
    context
        .run_ui(egui::RawInput::default(), |ui| {
            let _ = formula_ui_with_actions(ui, &mut editor, None, true);
        })
        .drop_without_applying_deltas();
    let before = editor.to_portable().unwrap();
    let caret = editor.caret();
    let selection = editor.selection();
    let revision = editor.arena().revision();
    let mut changed = true;
    let mut evaluated = true;
    let output = context.run_ui(
        egui::RawInput {
            events: vec![egui::Event::Cut],
            ..Default::default()
        },
        |ui| {
            let response = formula_ui_with_actions(ui, &mut editor, None, true);
            changed = response.response.changed();
            evaluated = response.evaluation_requested;
        },
    );
    let copied: Vec<_> = output
        .platform_output
        .commands
        .iter()
        .filter_map(|command| match command {
            egui::OutputCommand::CopyText(value) => Some(value.clone()),
            _ => None,
        })
        .collect();
    output.drop_without_applying_deltas();
    assert_eq!(copied.len(), 1);
    let tree: PortableTree =
        serde_json::from_str(copied[0].strip_prefix(TREE_CLIPBOARD_PREFIX).unwrap()).unwrap();
    tree.validate().unwrap();
    assert_eq!(
        tree.kind,
        NodeKind::Matrix {
            rows: 1,
            cols: 4,
            display: None
        }
    );
    assert!(tree.children == source.children[16..20]);
    assert!(!changed);
    assert!(!evaluated);
    assert!(editor.to_portable().unwrap() == before);
    assert_eq!(editor.caret(), caret);
    assert_eq!(editor.selection(), selection);
    assert_eq!(editor.arena().revision(), revision);
    assert!(
        editor.redo(),
        "rejected deletion must preserve preexisting redo"
    );
    assert!(editor.to_portable().unwrap() == redo_source);
    assert!(editor.undo());
    assert!(editor.to_portable().unwrap() == source);
    assert!(
        !editor.undo(),
        "Cut must not add a source checkpoint when delete refused"
    );
}
