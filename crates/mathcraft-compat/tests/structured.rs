use mathcraft_compat::{
    Value,
    editor_bridge::{tree_from_mathml, tree_to_mathml, worksheet_xml},
    evaluate_worksheet,
};
use mathcraft_editor::{BinaryOp, NodeKind, PortableTree};

fn run(fragments: &[&str]) -> Vec<mathcraft_compat::RegionEvaluation> {
    let regions: Vec<_> = fragments
        .iter()
        .enumerate()
        .map(|(i, x)| (i as u64, 20., i as f64 * 100., x.to_string()))
        .collect();
    evaluate_worksheet(&worksheet_xml(&regions).unwrap()).unwrap()
}
fn value(v: &Option<Value>) -> mathcraft_core::Quantity {
    match v.as_ref().unwrap() {
        Value::Scalar { quantity } => *quantity,
        _ => panic!("Expected scalar"),
    }
}

#[test]
fn define_eval_is_evaluated_once_and_display_does_not_rebind() {
    let rows = run(&[
        "<ml:define><ml:id>a</ml:id><ml:real>2</ml:real></ml:define>",
        "<ml:define><ml:id>a</ml:id><ml:eval><ml:apply><ml:plus/><ml:id>a</ml:id><ml:real>1</ml:real></ml:apply><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval></ml:define>",
        "<ml:eval><ml:id>a</ml:id><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>",
        "<ml:define><ml:id>distance</ml:id><ml:eval><ml:apply><ml:scale/><ml:real>2</ml:real><ml:id labels=\"UNIT\">m</ml:id></ml:apply><ml:unitOverride><ml:id labels=\"UNIT\">cm</ml:id></ml:unitOverride></ml:eval></ml:define>",
        "<ml:eval><ml:id>distance</ml:id><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>",
    ]);
    assert!(rows.iter().all(|r| r.error.is_none()), "{rows:?}");
    assert_eq!(value(&rows[1].value).value.re, 3.);
    assert_eq!(value(&rows[2].value).value.re, 3.);
    assert_eq!(value(&rows[3].value).value.re, 2.);
    assert_eq!(value(&rows[3].display_value).value.re, 200.);
    assert_eq!(value(&rows[4].value).value.re, 2.);
}

#[test]
fn function_parameters_shadow_unit_names_and_capture_definition_environment() {
    let rows = run(&[
        "<ml:define><ml:id>a</ml:id><ml:real>2</ml:real></ml:define>",
        "<ml:define><ml:function><ml:id>f</ml:id><ml:boundVars><ml:id>m</ml:id><ml:id>s</ml:id></ml:boundVars></ml:function><ml:apply><ml:plus/><ml:apply><ml:mult/><ml:id>a</ml:id><ml:id>m</ml:id></ml:apply><ml:id>s</ml:id></ml:apply></ml:define>",
        "<ml:define><ml:id>a</ml:id><ml:real>3</ml:real></ml:define>",
        "<ml:eval><ml:apply><ml:id>f</ml:id><ml:sequence><ml:real>4</ml:real><ml:real>1</ml:real></ml:sequence></ml:apply><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>",
    ]);
    assert!(rows.iter().all(|r| r.error.is_none()), "{rows:?}");
    assert_eq!(value(&rows[3].value).value.re, 9.);
}

#[test]
fn contextual_subscript_and_imaginary_symbol_roundtrip_without_name_collision() {
    let rich = "<ml:id labels=\"VARIABLE\" label-is-contextual=\"true\" xml:space=\"preserve\"><p:Span xmlns:p=\"http://schemas.microsoft.com/winfx/2006/xaml/presentation\">Z<ptc:Subscript xmlns:ptc=\"clr-namespace:Ptc.Wpf;assembly=Ptc.Core\">in</ptc:Subscript></p:Span></ml:id>";
    for fragment in [
        rich,
        "<ml:imag symbol=\"j\">2</ml:imag>",
        "<ml:apply><ml:nthRoot/><ml:placeholder/><ml:real>9</ml:real></ml:apply>",
    ] {
        let tree = tree_from_mathml(fragment).unwrap();
        assert_eq!(
            tree_from_mathml(&tree_to_mathml(&tree).unwrap()).unwrap(),
            tree
        );
    }
    let declaration = format!("<ml:define>{rich}<ml:real>8</ml:real></ml:define>");
    let evaluation =
        format!("<ml:eval>{rich}<ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>");
    let rows = run(&[
        &declaration,
        &evaluation,
        "<ml:eval><ml:id labels=\"VARIABLE\">Z_in</ml:id><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>",
    ]);
    assert_eq!(value(&rows[1].value).value.re, 8.);
    assert!(rows[2].error.is_some());
}

#[test]
fn scalar_builtins_preserve_complex_values_and_physical_dimensions() {
    let cases = [
        ("sin", "<ml:real>0</ml:real>", 0., 0.),
        ("cos", "<ml:real>0</ml:real>", 1., 0.),
        ("tan", "<ml:real>0</ml:real>", 0., 0.),
        ("ln", "<ml:real>1</ml:real>", 0., 0.),
        ("log", "<ml:real>100</ml:real>", 2., 0.),
        ("exp", "<ml:real>0</ml:real>", 1., 0.),
        ("sqrt", "<ml:real>9</ml:real>", 3., 0.),
        ("abs", "<ml:imag>2</ml:imag>", 2., 0.),
        ("re", "<ml:imag>2</ml:imag>", 0., 0.),
        ("im", "<ml:imag>2</ml:imag>", 2., 0.),
        ("conj", "<ml:imag>2</ml:imag>", 0., -2.),
    ];
    for (name, arg, re, im) in cases {
        let fragment = format!(
            "<ml:eval><ml:apply><ml:id>{name}</ml:id>{arg}</ml:apply><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>"
        );
        let rows = run(&[&fragment]);
        assert!(rows[0].error.is_none(), "{name}: {:?}", rows[0].error);
        let q = value(&rows[0].value);
        assert!(
            (q.value.re - re).abs() < 1e-12 && (q.value.im - im).abs() < 1e-12,
            "{name}: {q:?}"
        );
    }
    let rows = run(&[
        "<ml:eval><ml:apply><ml:id>im</ml:id><ml:apply><ml:scale/><ml:imag>2</ml:imag><ml:id labels=\"UNIT\">m</ml:id></ml:apply></ml:apply><ml:unitOverride><ml:id labels=\"UNIT\">cm</ml:id></ml:unitOverride></ml:eval>",
    ]);
    assert_eq!(value(&rows[0].display_value).value.re, 200.);
}

#[test]
fn native_wrapper_presentation_is_retained_by_document_and_unknown_execution_flags_fail() {
    let source = "<ws:math resultRef=\"9\"><ml:eval><ml:real>2</ml:real><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval><ws:resultFormat><ws:matrix size=\"12,12\"/></ws:resultFormat><ws:formattingOverride BackgroundColor=\"#FFFFF200\"/></ws:math>";
    assert!(tree_from_mathml(source).is_ok());
    assert!(
        tree_from_mathml(&source.replace("resultRef=", "disable-calc=\"true\" resultRef="))
            .is_err()
    );
    let nested = PortableTree {
        kind: NodeKind::Group,
        children: vec![PortableTree {
            kind: NodeKind::Binary(BinaryOp::Evaluate),
            children: vec![
                PortableTree {
                    kind: NodeKind::Number("1".into()),
                    children: vec![],
                },
                PortableTree {
                    kind: NodeKind::Placeholder,
                    children: vec![],
                },
            ],
        }],
    };
    assert!(tree_to_mathml(&nested).is_err());
}

#[test]
fn matrix_expression_xml_is_column_major_editor_and_values_are_row_major() {
    // Shipped example_matrix_algebra.html + example_matrix_algebra0.png prove
    // expression XML column ordering independently of result XML evidence.
    let matrix = "<ml:matrix rows=\"2\" cols=\"2\"><ml:real>1</ml:real><ml:real>3</ml:real><ml:real>2</ml:real><ml:real>4</ml:real></ml:matrix>";
    let tree = tree_from_mathml(matrix).unwrap();
    assert_eq!(
        tree.children
            .iter()
            .map(|n| match &n.kind {
                NodeKind::Number(v) => v.as_str(),
                _ => panic!(),
            })
            .collect::<Vec<_>>(),
        vec!["1", "2", "3", "4"]
    );
    assert_eq!(
        tree_from_mathml(&tree_to_mathml(&tree).unwrap()).unwrap(),
        tree
    );
    let define = format!("<ml:define><ml:id>A</ml:id>{matrix}</ml:define>");
    let rows = run(&[
        &define,
        "<ml:eval><ml:apply><ml:indexer/><ml:id>A</ml:id><ml:sequence><ml:real>1</ml:real><ml:real>0</ml:real></ml:sequence></ml:apply><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>",
        "<ml:eval><ml:id>A</ml:id><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>",
        "<ml:define><ml:id>ORIGIN</ml:id><ml:real>1</ml:real></ml:define>",
        "<ml:eval><ml:apply><ml:indexer/><ml:id>A</ml:id><ml:sequence><ml:real>2</ml:real><ml:real>1</ml:real></ml:sequence></ml:apply><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>",
        "<ml:eval><ml:apply><ml:indexer/><ml:id>A</ml:id><ml:real>1</ml:real></ml:apply><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>",
    ]);
    assert!(rows[..5].iter().all(|r| r.error.is_none()), "{rows:?}");
    assert_eq!(value(&rows[1].value).value.re, 3.);
    assert_eq!(value(&rows[4].value).value.re, 3.);
    assert!(rows[5].error.as_ref().unwrap().unsupported);
    let Value::Matrix {
        rows: 2,
        cols: 2,
        elements,
    } = rows[2].value.as_ref().unwrap()
    else {
        panic!()
    };
    assert_eq!(
        elements.iter().map(|q| q.value.re).collect::<Vec<_>>(),
        vec![1., 2., 3., 4.]
    );
    assert_eq!(
        rows[2].value.as_ref().unwrap().numeric_json()["elements"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["real"].as_f64().unwrap())
            .collect::<Vec<_>>(),
        vec![1., 3., 2., 4.]
    );
}

#[test]
fn ranges_keep_a_distinct_type_and_do_not_masquerade_as_vectors_or_scalars() {
    for expression in [
        "<ml:range><ml:real>1</ml:real><ml:real>3</ml:real></ml:range>",
        "<ml:range><ml:sequence><ml:real>1</ml:real><ml:real>1.1</ml:real></ml:sequence><ml:real>1.3</ml:real></ml:range>",
    ] {
        let tree = tree_from_mathml(expression).unwrap();
        assert_eq!(
            tree_from_mathml(&tree_to_mathml(&tree).unwrap()).unwrap(),
            tree
        );
    }
    let rows = run(&[
        "<ml:define><ml:id>i</ml:id><ml:range><ml:real>1</ml:real><ml:real>3</ml:real></ml:range></ml:define>",
        "<ml:eval><ml:id>i</ml:id><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>",
        "<ml:define><ml:id>a</ml:id><ml:id>i</ml:id></ml:define>",
        "<ml:eval><ml:apply><ml:plus/><ml:id>i</ml:id><ml:real>1</ml:real></ml:apply><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>",
        "<ml:eval><ml:range><ml:sequence><ml:real>1</ml:real><ml:real>1.1</ml:real></ml:sequence><ml:real>1.3</ml:real></ml:range><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>",
    ]);
    let Value::Range {
        start,
        step,
        end,
        count,
    } = rows[1].value.as_ref().unwrap()
    else {
        panic!()
    };
    assert_eq!(
        (start.value.re, step.value.re, end.value.re, *count),
        (1., 1., 3., 3)
    );
    assert!(rows[2].error.as_ref().unwrap().unsupported);
    let Value::RangeValues { elements } = rows[3].value.as_ref().unwrap() else {
        panic!()
    };
    assert_eq!(
        elements.iter().map(|q| q.value.re).collect::<Vec<_>>(),
        vec![2., 3., 4.]
    );
    assert!(
        matches!(rows[4].value, Some(Value::Range { count: 4, .. })),
        "{:?}",
        rows[4]
    );
}

#[test]
fn only_top_level_eval_lifts_over_one_named_range() {
    let rows = run(&[
        "<ml:define><ml:function><ml:id>f</ml:id><ml:boundVars><ml:id>x</ml:id></ml:boundVars></ml:function><ml:apply><ml:pow/><ml:id>x</ml:id><ml:real>2</ml:real></ml:apply></ml:define>",
        "<ml:define><ml:id>i</ml:id><ml:range><ml:real>0</ml:real><ml:real>2</ml:real></ml:range></ml:define>",
        "<ml:eval><ml:apply><ml:id>f</ml:id><ml:id>i</ml:id></ml:apply><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>",
        "<ml:define><ml:id>k</ml:id><ml:apply><ml:plus/><ml:id>i</ml:id><ml:real>1</ml:real></ml:apply></ml:define>",
        "<ml:define><ml:id>j</ml:id><ml:range><ml:real>0</ml:real><ml:real>2</ml:real></ml:range></ml:define>",
        "<ml:eval><ml:apply><ml:plus/><ml:id>i</ml:id><ml:id>j</ml:id></ml:apply><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>",
    ]);
    let Value::RangeValues { elements } = rows[2].value.as_ref().unwrap() else {
        panic!()
    };
    assert_eq!(
        elements.iter().map(|q| q.value.re).collect::<Vec<_>>(),
        vec![0., 1., 4.]
    );
    assert!(rows[3].error.as_ref().unwrap().unsupported);
    assert!(rows[5].error.as_ref().unwrap().unsupported);
}

#[test]
fn invalid_function_redefinition_hides_earlier_callable() {
    let rows = run(&[
        "<ml:define><ml:function><ml:id>f</ml:id><ml:boundVars><ml:id>x</ml:id></ml:boundVars></ml:function><ml:real>1</ml:real></ml:define>",
        "<ml:define><ml:function><ml:id>f</ml:id><ml:boundVars><ml:real>2</ml:real></ml:boundVars></ml:function><ml:real>8</ml:real></ml:define>",
        "<ml:eval><ml:apply><ml:id>f</ml:id><ml:real>4</ml:real></ml:apply><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>",
    ]);
    assert!(rows[0].error.is_none());
    assert!(rows[1].error.as_ref().unwrap().unsupported);
    assert_eq!(
        rows[2].error.as_ref().unwrap().previous_region.as_deref(),
        Some("1")
    );
    assert!(rows[2].value.is_none());
}
