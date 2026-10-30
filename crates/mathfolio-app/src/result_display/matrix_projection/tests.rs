use super::*;
fn decimal() -> MathFormat {
    MathFormat {
        result_format: ResultFormat::Decimal,
        ..MathFormat::default()
    }
}
fn xml(rows: usize, cols: usize) -> String {
    let cells = (0..cols)
        .flat_map(|c| (0..rows).map(move |r| format!("<ml:real>{}</ml:real>", r * 1000 + c)))
        .collect::<String>();
    format!(
        "<ml:result xmlns:ml=\"{ML}\"><ml:matrix rows=\"{rows}\" cols=\"{cols}\">{cells}</ml:matrix></ml:result>"
    )
}
fn window(r: usize, c: usize, nr: usize, nc: usize) -> MatrixWindow {
    MatrixWindow {
        row_offset: r,
        col_offset: c,
        rows: nr,
        cols: nc,
    }
}
fn value(rows: usize, cols: usize) -> Arc<Value> {
    Arc::new(Value::Matrix {
        rows,
        cols,
        elements: (0..rows)
            .flat_map(|r| (0..cols).map(move |c| Quantity::scalar((r * 1000 + c) as f64)))
            .collect(),
    })
}
fn plain(p: &MatrixProjection) -> Vec<String> {
    p.cells.iter().map(DisplayMath::plain).collect()
}
#[test]
fn legacy_fifty_native_formats_all_but_retained_projection_formats_only_visible() {
    let raw = xml(50, 50);
    let style = decimal();
    take_formatted_matrix_cells();
    let old = from_native_result(&raw, &style).unwrap();
    assert_eq!(take_formatted_matrix_cells(), 2500);
    assert_eq!(old.cells.last().unwrap().plain(), "…");
    let prepared = PreparedNativeMatrix::parse(&raw).unwrap();
    assert_eq!(
        take_formatted_matrix_cells(),
        0,
        "admission must not format cells"
    );
    assert_eq!(prepared.raw_xml(), raw);
    assert_eq!(prepared.shape(), (50, 50));
    let projection = prepared.project(window(0, 0, 12, 12), &style).unwrap();
    assert_eq!(take_formatted_matrix_cells(), 144);
    assert_eq!(projection.cells.len(), 144);
    assert_eq!(projection.cells.last().unwrap().plain(), "11011");
    assert!(projection.hidden_right && projection.hidden_below);
    assert!(!projection.hidden_left && !projection.hidden_above);
    prepared.project(window(20, 30, 12, 12), &style).unwrap();
    assert_eq!(take_formatted_matrix_cells(), 144);
    assert_eq!(
        prepared.raw_xml(),
        raw,
        "projection never replaces the complete authority"
    );
}
#[test]
fn hundred_matrix_new_profile_retains_all_cells_and_matches_live_row_major() {
    let raw = xml(100, 100);
    let style = decimal();
    assert!(
        from_native_result(&raw, &style).is_none(),
        "legacy4096node guard remains"
    );
    let native = PreparedNativeMatrix::parse(&raw).unwrap();
    let live = PreparedValueMatrix::new(value(100, 100)).unwrap();
    let a = native.project(window(87, 86, 12, 12), &style).unwrap();
    let b = live.project(window(87, 86, 12, 12), &style).unwrap();
    assert_eq!(a, b);
    assert_eq!(plain(&a)[0], "87086");
    assert_eq!(plain(&a)[143], "98097");
    assert!(a.hidden_above && a.hidden_left && a.hidden_below && a.hidden_right);
    assert_eq!(live.shape(), (100, 100));
    let Value::Matrix { elements, .. } = live.value().as_ref() else {
        panic!()
    };
    assert_eq!(elements[9999].value.re, 99099.);
}
#[test]
fn effective_offsets_clamp_checked_and_keep_true_edge_values() {
    let p = PreparedNativeMatrix::parse(&xml(2, 3))
        .unwrap()
        .project(window(usize::MAX, usize::MAX, 12, 12), &decimal())
        .unwrap();
    assert_eq!((p.row_offset, p.col_offset, p.rows, p.cols), (0, 0, 2, 3));
    assert_eq!(plain(&p), ["0", "1", "2", "1000", "1001", "1002"]);
    assert!(!p.hidden_above && !p.hidden_left);
    assert!(!p.hidden_below && !p.hidden_right);
    assert!(
        PreparedNativeMatrix::parse(&xml(2, 3))
            .unwrap()
            .project(window(0, 0, 0, 1), &decimal())
            .is_err()
    );
    assert!(
        PreparedNativeMatrix::parse(&xml(2, 3))
            .unwrap()
            .project(window(0, 0, 65536, 2), &decimal())
            .is_err()
    );
}
#[test]
fn offscreen_invalid_cells_never_acquire_authority() {
    let raw = xml(100, 100);
    for bad in [
        "<ml:real>NaN</ml:real>",
        "<ml:real base='2'>2</ml:real>",
        "<ml:real future='1'>1</ml:real>",
        "<ml:matrix rows='1' cols='1'><ml:real>1</ml:real></ml:matrix>",
        "<ml:complex><ml:real>1</ml:real></ml:complex>",
        "<ml:unitedValue><ml:real>1</ml:real><u:unitMonomial xmlns:u='http://schemas.mathsoft.com/units10'><u:unitReference unit='meter' power-numerator='1' power-denominator='0'/></u:unitMonomial></ml:unitedValue>",
    ] {
        assert!(
            PreparedNativeMatrix::parse(&raw.replace("<ml:real>99099</ml:real>", bad)).is_err(),
            "{bad}"
        );
    }
    let mut bad = (*value(100, 100)).clone();
    let Value::Matrix { elements, .. } = &mut bad else {
        panic!()
    };
    elements[9999].dimension[0] = f64::NAN;
    assert!(PreparedValueMatrix::new(Arc::new(bad)).is_err());
}
#[test]
fn full_shape_count_byte_text_and_structure_bounds_remain_explicit() {
    let raw = xml(2, 3);
    for bad in [
        raw.replace("rows=\"2\"", "rows=\"0\""),
        raw.replace("rows=\"2\"", "rows=\"18446744073709551615\""),
        raw.replace("<ml:real>1002</ml:real>", ""),
        raw.replace(
            "<ml:real>1002</ml:real>",
            "<ml:real>1002</ml:real><ml:real>7</ml:real>",
        ),
        raw.replace("<ml:real>1002</ml:real>", "<ml:real>100<!--x-->2</ml:real>"),
        raw.replace("<ml:real>1002</ml:real>", "<ml:real>100<?x y?>2</ml:real>"),
        raw.replace("<ml:matrix ", "<ml:matrix unknown='x' "),
    ] {
        assert!(PreparedNativeMatrix::parse(&bad).is_err());
    }
    assert!(PreparedNativeMatrix::parse(&" ".repeat(MAX_SOURCE_BYTES + 1)).is_err());
    let long = format!(
        "<matrix xmlns='{ML}' rows='1' cols='1'><str>{}</str></matrix>",
        "a".repeat(MAX_SOURCE_TEXT + 1)
    );
    assert!(PreparedNativeMatrix::parse(&long).is_err());
    assert!(
        PreparedValueMatrix::new(Arc::new(Value::Matrix {
            rows: 2,
            cols: 3,
            elements: vec![Quantity::scalar(1.); 5]
        }))
        .is_err()
    );
}
#[test]
fn expansion_is_separate_from_unchanged_v1_wire_and_bounded_presentation() {
    let p = PreparedNativeMatrix::parse(&xml(100, 100)).unwrap();
    let expanded = p.project(window(0, 0, 13, 13), &decimal()).unwrap();
    assert_eq!(expanded.cells.len(), 169);
    let old = FormattedResult {
        rows: 13,
        cols: 13,
        cells: expanded.cells,
        brackets: true,
    };
    assert_eq!(old.to_saved(), "");
    assert!(
        FormattedResult::from_saved(
            r#"{"version":1,"rows":13,"cols":1,"brackets":true,"cells":[]}"#
        )
        .is_none()
    );
    assert!(
        p.project(window(0, 0, 100, 100), &decimal()).is_err(),
        "window rendering still has4096node budget"
    );
    let legacy = format(value(100, 100).as_ref(), &decimal());
    assert_eq!((legacy.rows, legacy.cols), (12, 12));
    assert_eq!(legacy.cells.last().unwrap().plain(), "…");
}
#[test]
fn native_units_complex_strings_and_metadata_are_validated_before_projection() {
    let raw = format!(
        "<matrix xmlns='{ML}' xmlns:u='{UNITS}' rows='1' cols='3'><unitedValue><complex><real>3</real><imag symbol='j'>4</imag></complex><u:unitMonomial system='SI'><u:unitReference unit='meter' power-numerator='1' power-denominator='1'/></u:unitMonomial></unitedValue><str xml:space='preserve'>한글😀 &amp; </str><real base='16'>00aB</real></matrix>"
    );
    let p = PreparedNativeMatrix::parse(&raw).unwrap();
    let projected = p.project(window(0, 0, 1, 3), &decimal()).unwrap();
    let old = from_native_result(&raw, &decimal()).unwrap();
    assert_eq!(projected.cells, old.cells);
    assert_eq!(p.raw_xml(), raw);
    assert!(plain(&projected)[1].contains("한글😀 & "));
    assert_eq!(plain(&projected)[2], "171");
    for bad in [
        raw.replace("system='SI'", "system='SI' future='x'"),
        raw.replace("unit='meter'", "unit='meter' future='x'"),
        raw.replace("xml:space='preserve'", "xml:space='default'"),
        raw.replace("rows='1'", "rows='1' xmlns:f='urn:fake' f:cols='3'"),
    ] {
        assert!(PreparedNativeMatrix::parse(&bad).is_err());
    }
}
#[test]
fn clone_retains_authority_and_reformat_does_not_reparse_or_mutate_value() {
    let v = value(50, 50);
    let prepared = PreparedValueMatrix::new(v.clone()).unwrap();
    let clone = prepared.clone();
    assert!(Arc::ptr_eq(clone.value(), &v));
    let before = serde_json::to_vec(v.as_ref()).unwrap();
    let mut style = decimal();
    style.precision = 1;
    take_formatted_matrix_cells();
    prepared.project(window(3, 4, 12, 12), &style).unwrap();
    clone.project(window(9, 8, 12, 12), &style).unwrap();
    assert_eq!(take_formatted_matrix_cells(), 288);
    assert_eq!(serde_json::to_vec(v.as_ref()).unwrap(), before);
}

#[test]
fn lower_right_offset_moves_back_to_preserve_requested_window_size() {
    let prepared = PreparedNativeMatrix::parse(&xml(100, 100)).unwrap();
    let requested = window(99, 98, 12, 11);
    let projection = prepared.project(requested, &decimal()).unwrap();
    assert_eq!(requested, window(99, 98, 12, 11));
    assert_eq!(
        (
            projection.row_offset,
            projection.col_offset,
            projection.rows,
            projection.cols
        ),
        (88, 89, 12, 11)
    );
    assert_eq!(plain(&projection)[0], "88089");
    assert_eq!(projection.cells.last().unwrap().plain(), "99099");
    assert!(projection.hidden_above && projection.hidden_left);
    assert!(!projection.hidden_below && !projection.hidden_right);
}
