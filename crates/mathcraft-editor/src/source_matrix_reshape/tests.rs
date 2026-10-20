use super::*;

fn display(offset: [u32; 2], size: [u32; 2]) -> SourceMatrixDisplay {
    SourceMatrixDisplay::from_parts(Some(offset), Some(size)).unwrap()
}
fn matrix(rows: usize, cols: usize, d: Option<SourceMatrixDisplay>) -> PortableTree {
    PortableTree {
        kind: NodeKind::Matrix {
            rows,
            cols,
            display: d,
        },
        children: (0..rows * cols)
            .map(|n| PortableTree {
                kind: NodeKind::Number(n.to_string()),
                children: vec![],
            })
            .collect(),
    }
}
fn window(e: &Editor) -> ([usize; 2], [usize; 2]) {
    let NodeKind::Matrix {
        rows,
        cols,
        display,
    } = e.arena.get(e.root).unwrap().kind
    else {
        panic!()
    };
    display.unwrap_or_default().window([rows, cols]).unwrap()
}
fn enter(e: &mut Editor, row: usize, col: usize) {
    let node = e.arena.get(e.root).unwrap();
    let NodeKind::Matrix { cols, .. } = node.kind else {
        panic!()
    };
    let cell = node.children[row * cols + col];
    e.place_caret(cell, Direction::Right).unwrap();
}
fn state(e: &Editor) -> (PortableTree, Caret, Selection, u64) {
    (
        e.to_portable().unwrap(),
        e.caret(),
        e.selection(),
        e.arena().revision(),
    )
}

#[test]
fn inserted_row_keeps_full_children_ids_window_and_one_undo() {
    let source = matrix(10, 4, Some(display([2, 1], [3, 2])));
    let mut e = Editor::from_portable(&source).unwrap();
    enter(&mut e, 4, 2);
    let old_ids = e.arena.get(e.root).unwrap().children.clone();
    assert!(e.insert_matrix_row(Direction::Down));
    assert_eq!(window(&e), ([3, 1], [3, 2])); // position5 == old visible end
    let after = e.to_portable().unwrap();
    assert_eq!(after.children.len(), 44);
    assert_eq!(&after.children[..20], &source.children[..20]);
    assert_eq!(&after.children[24..], &source.children[20..]);
    assert!(
        after.children[20..24]
            .iter()
            .all(|c| c.kind == NodeKind::Placeholder)
    );
    assert_eq!(&e.arena.get(e.root).unwrap().children[..20], &old_ids[..20]);
    assert_eq!(e.caret.node, e.arena.get(e.root).unwrap().children[22]);
    assert_eq!(e.caret.caret_side, Direction::Indeterminate);
    assert!(e.undo());
    assert_eq!(e.to_portable().unwrap(), source);
    assert!(!e.undo());
    assert!(e.redo());
    assert_eq!(e.to_portable().unwrap(), after);
}

#[test]
fn insertion_at_visible_start_and_column_end_follow_axis_specific_transfer() {
    let mut e = Editor::from_portable(&matrix(10, 6, Some(display([2, 1], [3, 2])))).unwrap();
    enter(&mut e, 2, 1);
    assert!(e.insert_matrix_row(Direction::Up));
    assert_eq!(window(&e), ([2, 1], [3, 2])); // before/inside does not follow old data
    enter(&mut e, 3, 2);
    assert!(e.insert_matrix_column(Direction::Right));
    assert_eq!(window(&e), ([2, 2], [3, 2]));
    assert_eq!(e.to_portable().unwrap().children.len(), 77);
}

#[test]
fn deletion_count_shrinks_window_even_when_removed_coordinate_is_outside() {
    let old = Some(display([2, 1], [3, 2]));
    let next = transfer(
        old,
        [10, 6],
        [9, 6],
        Change::Remove { axis: 0, count: 1 },
        [2, 1],
    )
    .unwrap()
    .unwrap();
    assert_eq!(next.window([9, 6]).unwrap(), ([2, 1], [2, 2]));
    let edge = transfer(
        Some(display([7, 1], [3, 2])),
        [10, 6],
        [9, 6],
        Change::Remove { axis: 0, count: 1 },
        [7, 1],
    )
    .unwrap()
    .unwrap();
    assert_eq!(edge.window([9, 6]).unwrap(), ([7, 1], [2, 2]));
    let more = transfer(
        old,
        [10, 6],
        [6, 6],
        Change::Remove { axis: 0, count: 4 },
        [2, 1],
    )
    .unwrap()
    .unwrap();
    assert_eq!(more.window([6, 6]).unwrap(), ([2, 1], [1, 2]));
    // The helper deliberately has no deletion-position argument: 0256 uses
    // deleted count and end overhang only, not visible interval intersection.
}

#[test]
fn visible_row_delete_and_selected_column_delete_keep_survivors() {
    let source = matrix(10, 6, Some(display([2, 1], [3, 3])));
    let mut e = Editor::from_portable(&source).unwrap();
    enter(&mut e, 2, 2);
    assert!(e.delete_matrix_row());
    assert_eq!(window(&e), ([2, 1], [2, 3]));
    let after = e.to_portable().unwrap();
    assert_eq!(&after.children[..12], &source.children[..12]);
    assert_eq!(&after.children[12..], &source.children[18..]);
    assert!(e.undo());
    assert_eq!(e.to_portable().unwrap(), source);
    let mut e = Editor::from_portable(&matrix(10, 6, Some(display([0, 1], [3, 3])))).unwrap();
    assert!(e.select_matrix_cells(e.root, 1, 55)); // columns1 only, all10 rows
    assert!(e.delete_matrix_column());
    assert_eq!(window(&e), ([0, 1], [3, 2]));
    assert_eq!(e.to_portable().unwrap().children.len(), 50);
    assert!(e.undo());
    assert_eq!(
        e.to_portable().unwrap(),
        matrix(10, 6, Some(display([0, 1], [3, 3])))
    );
}

#[test]
fn hidden_final_target_rejects_without_source_caret_revision_or_redo_change() {
    let source = matrix(10, 4, Some(display([2, 1], [3, 2])));
    let mut e = Editor::from_portable(&source).unwrap();
    let first = e.arena.get(e.root).unwrap().children[0];
    e.set_atom(first, NodeKind::Number("999".into())).unwrap();
    let redo = e.to_portable().unwrap();
    assert!(e.undo());
    enter(&mut e, 4, 2);
    let before = state(&e);
    assert!(!e.delete_matrix_row()); // next row4 outside new [2,4)
    assert_eq!(state(&e), before);
    assert!(e.redo());
    assert_eq!(e.to_portable().unwrap(), redo);
}

#[test]
fn last_tab_retains_original_column_zero_rule_without_invented_horizontal_pan() {
    let mut full = Editor::from_portable(&matrix(10, 4, Some(display([7, 0], [3, 4])))).unwrap();
    enter(&mut full, 9, 3);
    assert_eq!(full.matrix_tab(true), Some(true));
    assert_eq!(window(&full), ([8, 0], [3, 4]));
    assert_eq!(
        full.caret.node,
        full.arena.get(full.root).unwrap().children[40]
    );
    let mut shifted = Editor::from_portable(&matrix(10, 4, Some(display([7, 1], [3, 3])))).unwrap();
    enter(&mut shifted, 9, 3);
    let before = state(&shifted);
    assert_eq!(shifted.matrix_tab(true), Some(false));
    assert_eq!(state(&shifted), before);
}

#[test]
fn ordinary_growth_and_explicit_full_display_keep_full_factory_semantics() {
    for d in [
        None,
        Some(SourceMatrixDisplay::default()),
        Some(display([0, 0], [12, 1])),
    ] {
        let mut e = Editor::from_portable(&matrix(12, 1, d)).unwrap();
        enter(&mut e, 11, 0);
        assert!(e.append_matrix_row());
        assert!(matches!(
            e.to_portable().unwrap().kind,
            NodeKind::Matrix {
                rows: 13,
                cols: 1,
                display: None
            }
        ));
        assert_eq!(window(&e), ([0, 0], [13, 1]));
    }
}

#[test]
fn selected_all_reset_and_partial_copy_keep_distinct_factory_contracts() {
    let mut source = matrix(2, 3, Some(display([1, 1], [1, 2])));
    source.children[1] = matrix(2, 2, Some(display([1, 0], [1, 2])));
    let mut e = Editor::from_portable(&source).unwrap();
    assert!(e.select_matrix_cells(e.root, 0, 1));
    let copied = e.copy_matrix_selection().unwrap();
    assert!(matches!(
        copied.kind,
        NodeKind::Matrix {
            rows: 1,
            cols: 2,
            display: None
        }
    ));
    assert_eq!(copied.children, source.children[..2]); // nested viewport survives
    assert_eq!(e.to_portable().unwrap(), source);
    assert!(e.select_matrix_cells(e.root, 0, 5));
    assert!(e.delete_matrix_selection(false));
    assert_eq!(
        e.to_portable().unwrap(),
        PortableTree {
            kind: NodeKind::Matrix {
                rows: 1,
                cols: 1,
                display: None
            },
            children: vec![PortableTree {
                kind: NodeKind::Placeholder,
                children: vec![]
            }],
        }
    );
    assert!(e.undo());
    assert_eq!(e.to_portable().unwrap(), source);
}

#[test]
fn budget_and_nested_hidden_entry_rejections_are_atomic() {
    let mut e = Editor::from_portable(&matrix(64, 64, Some(display([0, 0], [12, 12])))).unwrap();
    let before = state(&e);
    assert!(!e.insert_matrix_row(Direction::Down));
    assert_eq!(state(&e), before);
    let mut source = matrix(4, 2, Some(display([0, 0], [3, 2])));
    source.children[2] = matrix(2, 2, Some(display([1, 1], [1, 1])));
    let mut e = Editor::from_portable(&source).unwrap();
    enter(&mut e, 0, 0);
    let before = state(&e);
    assert!(!e.delete_matrix_row()); // retained new row0 enters nested hidden first leaf
    assert_eq!(state(&e), before);
}
