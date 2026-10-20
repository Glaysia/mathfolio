use super::*;
use mathcraft_editor::BinaryOp;
fn region(tree: PortableTree) -> CanvasRegion {
    CanvasRegion {
        id: 42,
        left: -0.,
        top: 123.45678901234567,
        content: CanvasContent::Math(tree),
        source_region: Some("原本µ\"\\\n".into()),
        rich_text: None,
        authored_text: None,
        format: MathFormat::default(),
        calculation_disabled: false,
        calculation_enabled_override: false,
    }
}
fn compare_region(region: &CanvasRegion) {
    let mut out = Vec::new();
    write_region_signature_json(region, &mut out).unwrap();
    assert_eq!(out, serde_json::to_vec(region).unwrap());
}
#[test]
fn framing_matches_every_node_kind_serde_field_and_nested_child_order() {
    let mut kinds = vec![
        NodeKind::Placeholder,
        NodeKind::Identifier("幅µ<&\"\\\n😀".into()),
        NodeKind::AnnotatedIdentifier {
            name: "x".into(),
            labels: "VARIABLE UNIT * NONE".into(),
        },
        NodeKind::NativeIdentifier {
            name: "𝑥".into(),
            labels: "*".into(),
            contextual: Some(true),
            subscript: Some("r😀".into()),
        },
        NodeKind::Number("-0.0e+10".into()),
        NodeKind::Imaginary("2".into()),
        NodeKind::ImaginarySymbol {
            value: "3".into(),
            symbol: "j".into(),
        },
        NodeKind::UnaryNeg,
        NodeKind::LogicalNot,
        NodeKind::Root,
        NodeKind::Absolute,
        NodeKind::Factorial,
        NodeKind::Percent,
        NodeKind::Degree,
        NodeKind::Transpose,
        NodeKind::Conjugate,
        NodeKind::Vectorize,
        NodeKind::Determinant,
        NodeKind::Row,
        NodeKind::Column,
        NodeKind::Call { argument_count: 2 },
        NodeKind::Function { parameter_count: 3 },
        NodeKind::Matrix {
            rows: 2,
            cols: 3,
            display: None,
        },
        NodeKind::Index { indices: 2 },
        NodeKind::Range { has_step: true },
        NodeKind::Program { statement_count: 2 },
        NodeKind::LocalDefine,
        NodeKind::If {
            has_else: true,
            branch_count: 3,
            alternate: true,
        },
        NodeKind::For,
        NodeKind::While,
        NodeKind::Return,
        NodeKind::Break,
        NodeKind::Continue,
        NodeKind::Try,
        NodeKind::Integral,
        NodeKind::Derivative { partial: true },
        NodeKind::Summation,
        NodeKind::Product,
        NodeKind::SymbolicEvaluation { command_count: 2 },
        NodeKind::SymbolicCommand { argument_count: 2 },
        NodeKind::Fraction,
        NodeKind::Power,
        NodeKind::Group,
    ];
    for op in [
        BinaryOp::Add,
        BinaryOp::Subtract,
        BinaryOp::Multiply,
        BinaryOp::Scale,
        BinaryOp::InlineDivide,
        BinaryOp::Define,
        BinaryOp::GlobalDefine,
        BinaryOp::CrossProduct,
        BinaryOp::Polar,
        BinaryOp::Evaluate,
        BinaryOp::Equal,
        BinaryOp::NotEqual,
        BinaryOp::LessThan,
        BinaryOp::LessOrEqual,
        BinaryOp::GreaterThan,
        BinaryOp::GreaterOrEqual,
        BinaryOp::And,
        BinaryOp::Or,
        BinaryOp::Xor,
    ] {
        kinds.push(NodeKind::Binary(op));
    }
    for kind in kinds {
        let tree = PortableTree {
            kind,
            children: vec![
                PortableTree {
                    kind: NodeKind::Number("2".into()),
                    children: vec![],
                },
                PortableTree {
                    kind: NodeKind::NativeIdentifier {
                        name: "m".into(),
                        labels: "UNIT".into(),
                        contextual: None,
                        subscript: None,
                    },
                    children: vec![],
                },
            ],
        };
        compare_region(&region(tree));
    }
}
#[test]
fn framing_preserves_all_metadata_float_spelling_fallback_and_disabled_filters() {
    let mut r = region(PortableTree {
        kind: NodeKind::Placeholder,
        children: vec![],
    });
    r.format = MathFormat {
        explicit: true,
        font_points: -0.,
        result_format: ResultFormat::Scientific,
        precision: 14,
        show_trailing_zeros: true,
        use_e_notation: true,
        use_engineering_format: true,
        exponential_threshold: 7,
        zero_threshold: 20,
        complex_threshold: 3,
        complex_format: ComplexFormat::PolarDegrees,
    };
    r.calculation_disabled = true;
    r.calculation_enabled_override = true;
    for value in [
        -0.,
        0.,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        0.10000000000000002,
        123456789.12345679,
    ] {
        r.left = value;
        r.top = value;
        r.format.font_points = value as f32;
        compare_region(&r);
    }
    for content in [
        CanvasContent::Text("\u{0000}\n한글😀".into()),
        CanvasContent::Opaque {
            xml: "<spec-table xmlns='NS'>\r\nµ&#10;</spec-table>".into(),
            kind: "spec-table".into(),
        },
        CanvasContent::Plot(PlotSpec::default()),
    ] {
        r.content = content;
        compare_region(&r);
    }
    let mut doc = CanvasDocument {
        regions: vec![r.clone(), r],
        native_archive: Some(vec![0, 1, 255]),
        ..Default::default()
    };
    doc.regions[1].calculation_disabled = false;
    doc.regions[1].id = 43;
    for value in [-0., 0., f64::NAN, f64::INFINITY, 12.5] {
        doc.width = value;
        doc.height = value;
        doc.grid = value;
        for active in [false, true] {
            let mut bytes = Vec::new();
            write_calculation_signature_json(&doc, active, &mut bytes).unwrap();
            let rows: Vec<_> = doc
                .regions
                .iter()
                .filter(|r| {
                    !matches!(r.content, CanvasContent::Text(_))
                        && (!active || !r.calculation_disabled)
                })
                .collect();
            assert_eq!(
                bytes,
                serde_json::to_vec(&(rows, doc.width, doc.height, doc.grid)).unwrap()
            );
        }
    }
}
#[test]
fn framing_writer_errors_keep_exact_prefix_and_io_error() {
    struct Limited {
        bytes: Vec<u8>,
        limit: usize,
    }
    impl Write for Limited {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            let count = b.len().min(self.limit.saturating_sub(self.bytes.len()));
            if count == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "test writer budget",
                ));
            }
            self.bytes.extend_from_slice(&b[..count]);
            Ok(count)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let r = region(PortableTree {
        kind: NodeKind::Identifier("µ😀\"\\".into()),
        children: vec![],
    });
    let length = serde_json::to_vec(&r).unwrap().len();
    for limit in [0, 1, 7, 30, length / 2, length - 1] {
        let mut old = Limited {
            bytes: vec![],
            limit,
        };
        let mut new = Limited {
            bytes: vec![],
            limit,
        };
        let a = serde_json::to_writer(&mut old, &r).unwrap_err();
        let b = write_region_signature_json(&r, &mut new).unwrap_err();
        assert_eq!(old.bytes, new.bytes);
        assert_eq!(a.to_string(), b.to_string());
        assert_eq!(a.io_error_kind(), b.io_error_kind());
    }
}

#[test]
fn old_persisted_signature_cache_golden_stays_readable_and_valid() {
    // Captured with the pre-change Vec/derived-Serialize presentation-v1 path.
    const OLD_DOCUMENT: &str = r####"{"version":2,"title":"Untitled worksheet","width":1024.0,"height":2048.0,"grid":12.5,"next_id":100,"regions":[{"id":42,"left":-0.0,"top":20.0,"content":{"Math":{"kind":{"NativeIdentifier":{"name":"幅µ","labels":"UNIT","contextual":false,"subscript":"r"}},"children":[]}},"source_region":"原本µ\"\\\n","format":{"explicit":false,"font_points":11.0,"result_format":"General","precision":3}},{"id":43,"left":-0.0,"top":20.0,"content":{"Opaque":{"xml":"<math><future/></math>","kind":"math"}},"source_region":"原本µ\"\\\n","format":{"explicit":false,"font_points":11.0,"result_format":"General","precision":3},"calculation_disabled":true}],"page_sections":{"explicit":false,"header":{"regions":[]},"footer":{"regions":[]},"starting_page_number":1,"total_pages_offset":0},"calculation_cache":{"source_signature":"9329301324ef39126d2efdb0c97f32cfa601e893db922e18b05577cbbfaf7a11","rows":[{"region_id":42,"status":"Pending","display":null,"error":null,"warnings":[],"symbolic_mathml":null}]},"native_archive":[0,1,255]}"####;
    let doc: CanvasDocument = serde_json::from_str(OLD_DOCUMENT).unwrap();
    doc.validate().unwrap();
    assert_eq!(
        doc.calculation_signature(),
        "9329301324ef39126d2efdb0c97f32cfa601e893db922e18b05577cbbfaf7a11"
    );
    assert_eq!(
        doc.active_calculation_signature(),
        "4815b2e37bee81c65e97b9c4de24e2a5419c9ebe5229715d6592915bada6a152"
    );
    assert_ne!(
        doc.calculation_signature(),
        doc.active_calculation_signature()
    );
    assert!(doc.saved_calculation_cache().is_some());
    assert_eq!(serde_json::to_string(&doc).unwrap(), OLD_DOCUMENT);
}

// Independent original presentation-v1 recipe; do not replace with the new
// framing writer, even when the two source signatures must match.
fn legacy_signature(document: &CanvasDocument, active_only: bool) -> String {
    use sha2::{Digest, Sha256};
    let regions: Vec<_> = document
        .regions
        .iter()
        .filter(|r| {
            !matches!(r.content, CanvasContent::Text(_))
                && (!active_only || !r.calculation_disabled)
        })
        .collect();
    let mut hash = Sha256::new();
    hash.update(b"mathcraft-calculation-presentation-v1\0");
    hash.update(
        serde_json::to_vec(&(regions, document.width, document.height, document.grid)).unwrap(),
    );
    if let Some(archive) = &document.native_archive {
        hash.update(b"\0native\0");
        hash.update(archive);
    }
    format!("{:x}", hash.finalize())
}
#[test]
fn whole_signatures_match_independent_legacy_for_all_content_and_source_changes() {
    let mut doc = CanvasDocument::default();
    let math = region(PortableTree {
        kind: NodeKind::NativeIdentifier {
            name: "𝑥µ\r\n\t\u{0001}".into(),
            labels: "VARIABLE UNIT * NONE".into(),
            contextual: Some(false),
            subscript: Some("r".into()),
        },
        children: vec![],
    });
    let mut text = math.clone();
    text.id = 43;
    text.content = CanvasContent::Text("text\u{0000}\n".into());
    let mut opaque = math.clone();
    opaque.id = 44;
    opaque.content = CanvasContent::Opaque {
        kind: "spec-table".into(),
        xml: "<spec-table xmlns='NS'>µ&amp;\r\n</spec-table>".into(),
    };
    let mut plot = math.clone();
    plot.id = 45;
    plot.content = CanvasContent::Plot(PlotSpec::default());
    doc.regions = vec![math, text, opaque, plot];
    for step in 0..20 {
        match step {
            0 => {}
            1 => doc.regions.swap(0, 2),
            2 => doc.regions[0].id = 999,
            3 => doc.regions[0].source_region = None,
            4 => doc.regions[0].format.precision = 14,
            5 => doc.regions[0].calculation_disabled = true,
            6 => doc.regions[0].calculation_enabled_override = true,
            7 => doc.native_archive = Some(vec![0, 1, 255]),
            8 => doc.native_archive.as_mut().unwrap()[1] = 9,
            9 => doc.width = -0.,
            10 => doc.width = 0.,
            11 => doc.height = f64::NAN,
            12 => doc.height = f64::INFINITY,
            13 => doc.grid = f64::NEG_INFINITY,
            14 => doc.grid = 12.5,
            15 => doc.regions[0].format.font_points = -0.,
            16 => {
                doc.title = "presentation only".into();
                doc.next_id = 9000;
            }
            17 => {
                doc.regions.remove(0);
            }
            18 => doc.regions.clear(),
            19 => doc.native_archive = Some(vec![]),
            _ => unreachable!(),
        }
        assert_eq!(
            doc.calculation_signature(),
            legacy_signature(&doc, false),
            "presentation step {step}"
        );
        assert_eq!(
            doc.active_calculation_signature(),
            legacy_signature(&doc, true),
            "active step {step}"
        );
    }
    for disabled in [false, true] {
        for enabled in [false, true] {
            let mut r = region(PortableTree {
                kind: NodeKind::Placeholder,
                children: vec![],
            });
            r.calculation_disabled = disabled;
            r.calculation_enabled_override = enabled;
            compare_region(&r);
        }
    }
}
