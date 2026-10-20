//! Bounded legend source metadata. This does not measure native layout.
use super::{
    PlotAxisEquation, PlotAxisLists, PlotAxisSpec, PlotLegendPosition, PlotSource, PlotSpec,
    PlotTraceAppearance, PlotTraceStyle,
};

impl PlotSpec {
    /// Source for an ordinary new empty XY plot. Geometry remains the existing
    /// app default, not a claim about Prime's measured outer/graph dimensions.
    pub fn blank_xy() -> Self {
        let appearance = PlotTraceAppearance {
            style: PlotTraceStyle {
                color: [0, 0, 139, 255],
                line_width: 1.,
                ..Default::default()
            },
            ..Default::default()
        };
        Self {
            source: PlotSource::AxisLists(PlotAxisLists {
                next_equation_id: 3,
                x: vec![PlotAxisEquation {
                    id: 1,
                    source: String::new().into(),
                    scaling: None,
                }],
                y: vec![PlotAxisEquation {
                    id: 2,
                    source: String::new().into(),
                    scaling: None,
                }],
                appearances: vec![appearance],
            }),
            native_geometry: Some(Box::new(super::PlotNativeGeometry {
                requested_width: Some(420.),
                requested_height: Some(260.),
                ..Default::default()
            })),
            x_axis: PlotAxisSpec {
                legend_position: Some(PlotLegendPosition::Bottom),
                ..Default::default()
            },
            y_axis: PlotAxisSpec {
                legend_position: Some(PlotLegendPosition::Right),
                ..Default::default()
            },
            ..Default::default()
        }
    }
    /// New CPU typed authoring source; legacy blank_xy serialization is unchanged.
    pub fn typed_blank_xy() -> Self {
        let mut spec = Self::blank_xy();
        let blank = super::PlotExpression::from_tree(mathcraft_editor::PortableTree {
            kind: mathcraft_editor::NodeKind::Placeholder,
            children: vec![],
        })
        .expect("constant plot placeholder");
        if let PlotSource::AxisLists(lists) = &mut spec.source {
            lists.x[0].source = blank.clone();
            lists.y[0].source = blank;
        }
        spec
    }
    pub(super) fn validate_native_legends(&self) -> Result<(), String> {
        if self
            .x_axis
            .legend_position
            .is_some_and(|p| !matches!(p, PlotLegendPosition::Bottom | PlotLegendPosition::Top))
            || self
                .y_axis
                .legend_position
                .is_some_and(|p| !matches!(p, PlotLegendPosition::Left | PlotLegendPosition::Right))
        {
            return Err("Plot legend position is not a supported boundary for this axis".into());
        }
        for axis in [&self.x_axis, &self.y_axis] {
            if axis.legend_width.is_some_and(|w| !w.is_finite() || w < 0.) {
                return Err("Plot legend width must be finite and nonnegative".into());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        canvas::{CanvasContent, CanvasDocument, PlotLineStyle},
        native::NativePackage,
    };
    #[test]
    fn absent_legend_fields_keep_legacy_axis_and_plot_json_bytes() {
        assert_eq!(
            serde_json::to_string(&PlotAxisSpec::default()).unwrap(),
            r#"{"label":"","range":null}"#
        );
        let old = r#"{"x_expression":"x","y_expression":"sin(x)","width":420.0,"height":260.0,"traces":[],"x_axis":{"label":"","range":null},"y_axis":{"label":"","range":null}}"#;
        let spec: PlotSpec = serde_json::from_str(old).unwrap();
        assert_eq!(spec, PlotSpec::default());
        assert_eq!(serde_json::to_string(&spec).unwrap(), old);
        assert!(spec.x_axis.legend_position.is_none() && spec.y_axis.legend_position.is_none());
        assert!(spec.x_axis.legend_width.is_none() && spec.y_axis.legend_width.is_none());
    }
    #[test]
    fn blank_source_has_one_placeholder_per_axis_explicit_fresh_positions_and_first_trace_style() {
        let blank = PlotSpec::blank_xy();
        blank.validate().unwrap();
        assert_eq!(blank.axis_lists().unwrap().x.len(), 1);
        assert_eq!(blank.axis_lists().unwrap().y.len(), 1);
        assert_eq!(blank.primary_x_expression().as_legacy_text().unwrap(), "");
        assert_eq!(blank.primary_y_expression().as_legacy_text().unwrap(), "");
        assert_eq!(
            blank.x_axis.legend_position,
            Some(PlotLegendPosition::Bottom)
        );
        assert_eq!(
            blank.y_axis.legend_position,
            Some(PlotLegendPosition::Right)
        );
        let trace = &blank.resolved_traces().unwrap()[0];
        assert_eq!(trace.style.color, [0, 0, 139, 255]);
        assert_eq!(trace.style.line, PlotLineStyle::Solid);
        assert_eq!(trace.style.line_width, 1.);
        assert_eq!(
            serde_json::from_slice::<PlotSpec>(&serde_json::to_vec(&blank).unwrap()).unwrap(),
            blank
        );
        assert_eq!(
            (blank.width, blank.height),
            (PlotSpec::default().width, PlotSpec::default().height)
        );
        assert_eq!(
            PlotSpec::default()
                .primary_y_expression()
                .as_legacy_text()
                .unwrap(),
            "sin(x)"
        );
    }
    #[test]
    fn legend_validation_and_signature_keep_absent_explicit_and_signed_zero_distinct() {
        for (x, y) in [
            (PlotLegendPosition::Left, PlotLegendPosition::Left),
            (PlotLegendPosition::Bottom, PlotLegendPosition::Top),
        ] {
            let mut spec = PlotSpec::default();
            spec.x_axis.legend_position = Some(x);
            spec.y_axis.legend_position = Some(y);
            assert!(spec.validate().is_err());
        }
        for width in [-1., f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut spec = PlotSpec::default();
            spec.x_axis.legend_width = Some(width);
            assert!(spec.validate().is_err());
        }
        let mut doc = CanvasDocument::default();
        doc.add_plot(0., 0.);
        let before = doc.calculation_signature();
        let CanvasContent::Plot(spec) = &mut doc.regions[0].content else {
            panic!()
        };
        spec.x_axis.legend_position = Some(PlotLegendPosition::Bottom);
        assert_ne!(doc.calculation_signature(), before);
        let CanvasContent::Plot(spec) = &mut doc.regions[0].content else {
            panic!()
        };
        spec.x_axis.legend_width = Some(0.);
        let plus = doc.calculation_signature();
        let CanvasContent::Plot(spec) = &mut doc.regions[0].content else {
            panic!()
        };
        spec.x_axis.legend_width = Some(-0.);
        assert_ne!(doc.calculation_signature(), plus);
        doc.validate().unwrap();
        let restored: CanvasDocument =
            serde_json::from_slice(&serde_json::to_vec(&doc).unwrap()).unwrap();
        assert_eq!(
            restored.calculation_signature(),
            doc.calculation_signature()
        );
        assert!(
            serde_json::from_str::<PlotAxisSpec>(r#"{"legend_position":"axis_attached"}"#).is_err()
        );
    }
    #[test]
    fn native_package_roundtrip_preserves_boundary_source_and_exact_legend_width_without_execution()
    {
        let dir = std::env::temp_dir().join(format!(
            "mathcraft-legend-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&dir).unwrap();
        let first = dir.join("first.mcdx");
        let copied = dir.join("copy.mcdx");
        let mut doc = CanvasDocument::default();
        doc.add_plot(20., 20.);
        let mut blank = PlotSpec::blank_xy();
        blank.x_axis.legend_position = Some(PlotLegendPosition::Top);
        blank.x_axis.legend_width = Some(123.45678901234567);
        blank.y_axis.legend_width = Some(-0.);
        doc.regions[0].content = CanvasContent::Plot(blank.clone());
        crate::export::export_mcdx(&doc, &first).unwrap();
        let package = NativePackage::open(&first).unwrap();
        let source = package.worksheet_xml().unwrap();
        let parsed = roxmltree::Document::parse(&source).unwrap();
        let region = parsed
            .descendants()
            .find(|n| n.has_tag_name(("http://schemas.mathsoft.com/worksheet50", "region")))
            .unwrap();
        let fragment = crate::spec_table::standalone(region, &source).unwrap();
        let restored = crate::native_plot::parse(&fragment).unwrap();
        assert!(crate::native_plot::same_source(&blank, &restored).unwrap());
        assert_eq!(
            restored.x_axis.legend_width.unwrap().to_bits(),
            blank.x_axis.legend_width.unwrap().to_bits()
        );
        assert_eq!(
            restored.y_axis.legend_width.unwrap().to_bits(),
            (-0.0f64).to_bits()
        );
        doc.native_archive = Some(std::fs::read(&first).unwrap());
        doc.regions[0].source_region = Some(region.attribute("region-id").unwrap().into());
        crate::export::export_mcdx(&doc, &copied).unwrap();
        let copied_package = NativePackage::open(&copied).unwrap();
        assert_eq!(
            package.original_part(package.worksheet_part_name()),
            copied_package.original_part(copied_package.worksheet_part_name())
        );
        std::fs::remove_file(first).unwrap();
        std::fs::remove_file(copied).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
    #[test]
    #[ignore = "Explicit generated legend source packages for offline XSD checks; no target runtime"]
    fn native_xy_legend_schema_fixture() {
        let output = std::path::PathBuf::from(
            std::env::var_os("MATHCRAFT_NATIVE_XY_LEGEND_OUTPUT")
                .expect("MATHCRAFT_NATIVE_XY_LEGEND_OUTPUT"),
        );
        std::fs::create_dir_all(&output).unwrap();
        let mut doc = CanvasDocument::default();
        for i in 0..4 {
            doc.add_plot(20., 20. + i as f64 * 300.);
            let spec = match i {
                0 => PlotSpec::default(),
                1 => PlotSpec::blank_xy(),
                2 => {
                    let mut p = PlotSpec::blank_xy();
                    p.x_axis.legend_position = Some(PlotLegendPosition::Top);
                    p.y_axis.legend_position = Some(PlotLegendPosition::Left);
                    p.x_axis.legend_width = Some(123.45678901234567);
                    p
                }
                _ => {
                    let mut p = PlotSpec::blank_xy();
                    p.x_axis.legend_width = Some(-0.);
                    p.y_axis.legend_width = Some(f64::from_bits(1));
                    p
                }
            };
            doc.regions.last_mut().unwrap().content = CanvasContent::Plot(spec);
        }
        doc.save(&output.join("legend-source.mathcraft")).unwrap();
        let native = output.join("legend-source.mcdx");
        crate::export::export_mcdx(&doc, &native).unwrap();
        let package = NativePackage::open(&native).unwrap();
        std::fs::write(
            output.join("worksheet.xml"),
            package.worksheet_xml().unwrap(),
        )
        .unwrap();
        println!(
            "LEGEND_SOURCE_FIXTURE plots=4 inherited_positions=absent explicit_boundaries=4 width_bits=finite PH=preserved execution=0"
        );
    }
}
