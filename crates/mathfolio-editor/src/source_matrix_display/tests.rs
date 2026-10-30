use crate::*;

fn matrix(display: Option<SourceMatrixDisplay>) -> PortableTree {
    PortableTree {
        kind: NodeKind::Matrix {
            rows: 2,
            cols: 3,
            display,
        },
        children: (1..=6)
            .map(|i| PortableTree {
                kind: NodeKind::Number(i.to_string()),
                children: vec![],
            })
            .collect(),
    }
}
fn metadata() -> SourceMatrixDisplay {
    SourceMatrixDisplay::from_parts(Some([1, 1]), Some([1, 2])).unwrap()
}

#[test]
fn absent_matrix_metadata_keeps_exact_old_json_and_some_empty_is_distinct() {
    let tree = matrix(None);
    let golden = r#"{"kind":{"Matrix":{"rows":2,"cols":3}},"children":[{"kind":{"Number":"1"},"children":[]},{"kind":{"Number":"2"},"children":[]},{"kind":{"Number":"3"},"children":[]},{"kind":{"Number":"4"},"children":[]},{"kind":{"Number":"5"},"children":[]},{"kind":{"Number":"6"},"children":[]}]}"#;
    assert_eq!(serde_json::to_string(&tree).unwrap(), golden);
    assert_eq!(serde_json::from_str::<PortableTree>(golden).unwrap(), tree);
    let some = matrix(Some(SourceMatrixDisplay::default()));
    let json = serde_json::to_string(&some).unwrap();
    assert!(json.contains(r#""display":{}"#));
    assert_ne!(some, tree);
    assert_eq!(serde_json::from_str::<PortableTree>(&json).unwrap(), some);
}

#[test]
fn wire_is_closed_and_null_is_not_absence() {
    for json in [
        r#"{"offset":null}"#,
        r#"{"size":null}"#,
        r#"{"offset":[0,0],"offset":[0,1]}"#,
        r#"{"size":[1,1],"size":[1,2]}"#,
        r#"{"offset":[0]}"#,
        r#"{"size":[1,1,1]}"#,
        r#"{"offset":[-1,0]}"#,
        r#"{"size":[1.0,1]}"#,
        r#"{"size":[4294967296,1]}"#,
        r#"{"origin":0}"#,
        r#"{"show_indices":true}"#,
    ] {
        assert!(
            serde_json::from_str::<SourceMatrixDisplay>(json).is_err(),
            "{json}"
        );
    }
    let legacy = serde_json::to_string(&matrix(None)).unwrap();
    for extra in [r#", "display":null"#, r#", "display":{},"display":{}"#] {
        let changed = legacy.replacen(r#""cols":3}"#, &format!(r#""cols":3{extra}}}"#), 1);
        assert!(
            serde_json::from_str::<PortableTree>(&changed).is_err(),
            "{changed}"
        );
    }
}

#[test]
fn tuple_validation_preserves_presence_and_full_shape_arity() {
    let cases = [
        (None, None, ([0, 0], [2, 3])),
        (Some([1, 1]), None, ([1, 1], [1, 2])),
        (None, Some([1, 2]), ([0, 0], [1, 2])),
        (Some([0, 0]), Some([2, 3]), ([0, 0], [2, 3])),
    ];
    for (offset, size, expected) in cases {
        let d = SourceMatrixDisplay::from_parts(offset, size).unwrap();
        assert_eq!(d.offset(), offset);
        assert_eq!(d.size(), size);
        assert_eq!(d.window([2, 3]).unwrap(), expected);
        let full = matrix(Some(d));
        full.validate().unwrap();
        assert_eq!(full.kind.arity(), 6);
        let mut truncated = full;
        truncated.children.pop();
        assert!(truncated.validate().is_err());
    }
    assert!(SourceMatrixDisplay::from_parts(None, Some([0, 1])).is_err());
    for d in [
        SourceMatrixDisplay::from_parts(Some([2, 0]), None).unwrap(),
        SourceMatrixDisplay::from_parts(None, Some([3, 1])).unwrap(),
        SourceMatrixDisplay::from_parts(Some([1, 1]), Some([2, 3])).unwrap(),
        SourceMatrixDisplay::from_parts(Some([u32::MAX, 0]), None).unwrap(),
        serde_json::from_str(r#"{"size":[0,1]}"#).unwrap(),
    ] {
        assert!(matrix(Some(d)).validate().is_err());
    }
}

#[test]
fn cell_edit_undo_redo_and_whole_subtree_copy_preserve_metadata_and_ids() {
    let source = matrix(Some(metadata()));
    let mut editor = Editor::from_portable(&source).unwrap();
    let root = editor.root();
    let ids = editor.arena().get(root).unwrap().children().to_vec();
    editor
        .set_atom(ids[0], NodeKind::Number("11".into()))
        .unwrap();
    let changed = editor.to_portable().unwrap();
    assert_eq!(changed.kind, source.kind);
    assert_eq!(changed.children[5], source.children[5]);
    assert_eq!(editor.arena().get(root).unwrap().children(), ids);
    assert_eq!(editor.export_subtree(root).unwrap(), changed);
    assert!(editor.undo());
    assert_eq!(editor.to_portable().unwrap(), source);
    assert!(editor.redo());
    assert_eq!(editor.to_portable().unwrap(), changed);
}

#[test]
fn insertion_with_rectangle_selection_remains_atomic_and_keeps_redo() {
    let source = matrix(Some(metadata()));
    let mut e = Editor::from_portable(&source).unwrap();
    let root = e.root();
    let first = e.arena().get(root).unwrap().children()[0];
    e.set_atom(first, NodeKind::Number("10".into())).unwrap();
    let redo = e.to_portable().unwrap();
    assert!(e.undo());
    assert!(e.select_matrix_cells(root, 0, 5));
    let before = (
        e.to_portable().unwrap(),
        e.caret(),
        e.selection(),
        e.arena().revision(),
    );
    for operation in 0..3 {
        let changed = match operation {
            0 => e.append_matrix_row(),
            1 => e.insert_matrix_row(Direction::Up),
            _ => e.insert_matrix_column(Direction::Right),
        };
        assert!(!changed);
        assert_eq!(
            (
                e.to_portable().unwrap(),
                e.caret(),
                e.selection(),
                e.arena().revision()
            ),
            before
        );
    }
    assert!(e.redo());
    assert_eq!(e.to_portable().unwrap(), redo);
}

#[test]
fn selected_submatrix_copy_gets_new_full_window_but_same_shape_clear_retains_it() {
    let source = matrix(Some(metadata()));
    let mut e = Editor::from_portable(&source).unwrap();
    let root = e.root();
    e.select_matrix_cells(root, 0, 1);
    let copied = e.copy_matrix_selection().unwrap();
    assert!(matches!(
        copied.kind,
        NodeKind::Matrix {
            rows: 1,
            cols: 2,
            display: None
        }
    ));
    assert_eq!(copied.children, source.children[..2]);
    assert_eq!(e.to_portable().unwrap(), source);
    assert!(e.clear_matrix_cells());
    let changed = e.to_portable().unwrap();
    assert_eq!(changed.kind, source.kind);
    assert_eq!(changed.children[0].kind, NodeKind::Placeholder);
    assert_eq!(changed.children[5], source.children[5]);
    assert!(e.undo());
    assert_eq!(e.to_portable().unwrap(), source);
    let mut old = Editor::from_portable(&matrix(None)).unwrap();
    old.select_matrix_cells(old.root(), 0, 1);
    assert!(old.copy_matrix_selection().is_some());
    assert!(old.delete_matrix_selection(false));
}

#[test]
fn nested_matrices_own_metadata_independently_and_outer_shape_remains_ordinary() {
    let inner = matrix(Some(metadata()));
    let source = PortableTree {
        kind: NodeKind::Matrix {
            rows: 1,
            cols: 1,
            display: None,
        },
        children: vec![inner.clone()],
    };
    let mut e = Editor::from_portable(&source).unwrap();
    e.select_subtree(e.root()).unwrap();
    assert_eq!(e.export_subtree(e.root()).unwrap(), source);
    let json = serde_json::to_string(&source).unwrap();
    assert_eq!(json.matches("\"display\"").count(), 1);
    assert_eq!(serde_json::from_str::<PortableTree>(&json).unwrap(), source);
    assert_eq!(e.to_portable().unwrap().children[0], inner);
}
