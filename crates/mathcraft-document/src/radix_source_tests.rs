use crate::canvas::{PlotAxis, PlotExpression, PlotScaling, PlotSpec};
use mathcraft_editor::{NodeKind, NumericBase, PortableTree};

#[test]
fn native_typed_first_math_and_scaling_preserve_based_source_without_computing_it() {
    for (digits, base) in [
        ("001101", NumericBase::Binary),
        ("32.4", NumericBase::Octal),
        ("0aF", NumericBase::Hexadecimal),
    ] {
        let tree = PortableTree {
            kind: NodeKind::based_number(digits, base).unwrap(),
            children: vec![],
        };
        let expression = PlotExpression::from_tree(tree.clone()).unwrap();
        let scaling = PlotScaling::from_tree(tree.clone()).unwrap().unwrap();
        assert_eq!(scaling.as_tree(), &tree);
        let mut spec = PlotSpec::typed_blank_xy();
        spec.set_axis_expression(PlotAxis::Y, 0, expression)
            .unwrap();
        let wire = serde_json::to_string(&spec).unwrap();
        let restored: PlotSpec = serde_json::from_str(&wire).unwrap();
        assert_eq!(restored, spec);
        assert_eq!(restored.primary_y_expression().as_tree(), Some(&tree));
        let payload = crate::native_plot::payload(&spec).unwrap();
        assert!(payload.contains(&format!("base=\"{}\">{digits}</ml:real>", base.radix())));
        let geometry = spec.native_geometry.as_ref().unwrap();
        let owner = format!(
            "<region xmlns=\"http://schemas.mathsoft.com/worksheet50\" width=\"{}\" height=\"{}\">{payload}</region>",
            geometry.requested_width.unwrap(),
            geometry.requested_height.unwrap()
        );
        let parsed = crate::native_plot::parse(&owner).unwrap();
        assert_eq!(parsed.primary_y_expression().as_tree(), Some(&tree));
        assert!(crate::native_plot::same_source(&spec, &parsed).unwrap());
        assert_eq!(crate::native_plot::payload(&parsed).unwrap(), payload);
    }
}
