use super::*;

fn tree(offset: Option<[u32; 2]>, size: Option<[u32; 2]>) -> PortableTree {
    PortableTree {
        kind: NodeKind::Matrix {
            rows: 4,
            cols: 4,
            display: Some(SourceMatrixDisplay::from_parts(offset, size).unwrap()),
        },
        children: (0..16)
            .map(|i| PortableTree {
                kind: NodeKind::Number(i.to_string()),
                children: vec![],
            })
            .collect(),
    }
}
fn display(editor: &Editor) -> SourceMatrixDisplay {
    let NodeKind::Matrix {
        display: Some(display),
        ..
    } = editor.arena.get(editor.root).unwrap().kind
    else {
        panic!()
    };
    display
}

#[test]
fn imported_entry_uses_real_visible_cell_without_source_change_or_undo() {
    let source = tree(Some([1, 2]), Some([2, 2]));
    let mut editor = Editor::from_portable(&source).unwrap();
    assert_eq!(
        editor.caret.node,
        editor.arena.get(editor.root).unwrap().children[6]
    );
    assert!(editor.to_portable().unwrap() == source);
    assert!(!editor.undo());
}

#[test]
fn full_row_step_pans_with_stable_ids_and_one_old_caret_checkpoint() {
    let mut editor = Editor::from_portable(&tree(Some([0, 0]), Some([2, 2]))).unwrap();
    let cells = editor.arena.get(editor.root).unwrap().children.clone();
    editor.place_caret(cells[4], Direction::Right).unwrap();
    let old_caret = editor.caret;
    let old = editor.to_portable().unwrap();
    assert!(editor.source_matrix_step_row(editor.root, true));
    assert_eq!(editor.caret.node, cells[8]);
    assert!(editor.reveal_source_matrix_caret_after_navigation(old_caret));
    assert_eq!(display(&editor).offset(), Some([1, 0]));
    assert_eq!(editor.arena.get(editor.root).unwrap().children, cells);
    let changed = editor.to_portable().unwrap();
    assert!(editor.undo());
    assert_eq!(editor.caret, old_caret);
    assert!(editor.to_portable().unwrap() == old);
    assert!(editor.redo());
    assert!(editor.to_portable().unwrap() == changed);
}

#[test]
fn offset_only_pan_materializes_old_effective_size_not_new_tail() {
    let mut editor = Editor::from_portable(&tree(Some([1, 1]), None)).unwrap();
    let cells = editor.arena.get(editor.root).unwrap().children.clone();
    let before = editor.caret;
    editor.place_caret(cells[0], Direction::Left).unwrap();
    assert!(editor.reveal_source_matrix_caret_after_navigation(before));
    assert_eq!(display(&editor).size(), Some([3, 3]));
    assert_eq!(display(&editor).offset(), Some([0, 0]));
}

#[test]
fn visible_movement_has_no_source_checkpoint() {
    let source = tree(None, Some([2, 2]));
    let mut editor = Editor::from_portable(&source).unwrap();
    let before = editor.caret;
    let second = editor.arena.get(editor.root).unwrap().children[1];
    editor.place_caret(second, Direction::Left).unwrap();
    assert!(!editor.reveal_source_matrix_caret_after_navigation(before));
    assert!(editor.to_portable().unwrap() == source);
    assert!(!editor.undo());
}

#[test]
fn newly_entered_nested_window_clamps_without_mutating_its_source() {
    let nested = tree(Some([2, 2]), Some([2, 2]));
    let source = PortableTree {
        kind: NodeKind::Binary(BinaryOp::Add),
        children: vec![
            PortableTree {
                kind: NodeKind::Number("1".into()),
                children: vec![],
            },
            nested,
        ],
    };
    let mut editor = Editor::from_portable(&source).unwrap();
    let before = editor.caret;
    let nested = editor.arena.get(editor.root).unwrap().children[1];
    let cells = editor.arena.get(nested).unwrap().children.clone();
    editor.place_caret(cells[0], Direction::Right).unwrap();
    assert!(!editor.reveal_source_matrix_caret_after_navigation(before));
    assert_eq!(editor.caret.node, cells[10]);
    assert!(editor.to_portable().unwrap() == source);
    assert!(!editor.undo());
}

#[test]
fn paste_and_load_enter_visible_real_cells() {
    let source = tree(Some([2, 1]), Some([2, 3]));
    for replace in [true, false] {
        let mut editor = Editor::new();
        if replace {
            editor.replace_selection(&source).unwrap();
        } else {
            editor.load_portable(&source).unwrap();
        }
        assert_eq!(
            editor.caret.node,
            editor.arena.get(editor.root).unwrap().children[9]
        );
        assert!(editor.to_portable().unwrap() == source);
        assert!(editor.undo());
        assert!(matches!(
            editor.arena.get(editor.root).unwrap().kind,
            NodeKind::Placeholder
        ));
    }
}
