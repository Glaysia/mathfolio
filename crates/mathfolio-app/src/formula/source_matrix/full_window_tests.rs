//! Production measure/readonly-print helper equivalence; no display presence
//! should select different geometry when all full-source cells are visible.
use super::*;

#[derive(Debug, PartialEq)]
enum Ink {
    Text(Vec2, String, Vec2, bool),
    Line(Vec2, Vec2, bool),
    Placeholder(Rect),
}
#[derive(Debug, PartialEq)]
struct Geometry {
    node: NodeId,
    size: Vec2,
    baseline: f32,
    atom: Option<(String, Vec2)>,
    text_offset: Vec2,
    hit_bounds: Option<Rect>,
    ink: Vec<Ink>,
    children: Vec<(Vec2, Geometry)>,
    result_geometry: bool,
}
fn geometry(value: &FormulaBox) -> Geometry {
    Geometry {
        node: value.node,
        size: value.size,
        baseline: value.baseline,
        atom: value
            .atom
            .as_ref()
            .map(|galley| (galley.text().into(), galley.size())),
        text_offset: value.text_offset,
        hit_bounds: value.hit_bounds,
        ink: value
            .primitives
            .iter()
            .map(|primitive| match primitive {
                Primitive::Text(position, galley) => {
                    Ink::Text(*position, galley.text().into(), galley.size(), false)
                }
                Primitive::FocusedText(position, galley) => {
                    Ink::Text(*position, galley.text().into(), galley.size(), true)
                }
                Primitive::Line(a, b) => Ink::Line(*a, *b, false),
                Primitive::MinorFence(a, b) => Ink::Line(*a, *b, true),
                Primitive::Placeholder(rect) => Ink::Placeholder(*rect),
            })
            .collect(),
        children: value
            .children
            .iter()
            .map(|(position, child)| (*position, geometry(child)))
            .collect(),
        result_geometry: value.matrix_viewport.is_some(),
    }
}

type HitObservation = (
    NodeId,
    Rect,
    InsertionMode,
    Option<usize>,
    Option<Direction>,
    Pos2,
);

#[derive(Debug, PartialEq)]
struct Observation {
    geometry: Geometry,
    hits: Vec<HitObservation>,
    measured_print: Option<(Vec2, f32)>,
    allocated_print: Option<Vec2>,
    print_pixel: Option<f32>,
}
fn observe(display: Option<SourceMatrixDisplay>, readonly: bool) -> Observation {
    let source = PortableTree {
        kind: NodeKind::Matrix {
            rows: 3,
            cols: 4,
            display,
        },
        children: (0..12)
            .map(|number| PortableTree {
                kind: NodeKind::Number(number.to_string()),
                children: vec![],
            })
            .collect(),
    };
    let editor = Editor::from_portable(&source).unwrap();
    let context = egui::Context::default();
    crate::configure(&context);
    let mut observation = None;
    let output = context.run_ui(egui::RawInput::default(), |ui| {
        // A fresh Context for every variant prevents a cache key shared by
        // separate equal-revision arenas from masking a renderer difference.
        let layout = if readonly {
            readonly_layout(ui, &editor, None, None, 11.)
        } else {
            Arc::new(measure_with_display(
                ui.painter(),
                editor.arena(),
                editor.root(),
                11. * 96. / 72.,
                None,
            ))
        };
        let mut hits = vec![];
        collect_hits(&layout, Pos2::ZERO, &mut hits);
        let measured_print =
            readonly.then(|| measured_geometry_readonly_styled(ui, &editor, None, None, 11.));
        let allocated_print = readonly.then(|| {
            formula_readonly_ui_styled(ui, &editor, None, None, 11.)
                .rect
                .size()
        });
        observation = Some(Observation {
            geometry: geometry(&layout),
            hits: hits
                .iter()
                .map(|hit| {
                    (
                        hit.node,
                        hit.rect,
                        hit.insertion,
                        hit.operator_part,
                        hit.operator_side,
                        hit.text_origin,
                    )
                })
                .collect(),
            measured_print,
            allocated_print,
            print_pixel: readonly.then(|| ui.ctx().pixels_per_point().recip()),
        });
    });
    output.drop_without_applying_deltas();
    assert!(
        editor.to_portable().unwrap() == source,
        "measurement/printing mutated full source"
    );
    observation.unwrap()
}
fn full_windows() -> [SourceMatrixDisplay; 4] {
    [
        SourceMatrixDisplay::from_parts(None, None).unwrap(),
        SourceMatrixDisplay::from_parts(Some([0, 0]), None).unwrap(),
        SourceMatrixDisplay::from_parts(None, Some([3, 4])).unwrap(),
        SourceMatrixDisplay::from_parts(Some([0, 0]), Some([3, 4])).unwrap(),
    ]
}

#[test]
fn full_display_presence_matches_legacy_source_geometry_and_every_real_hit() {
    let legacy = observe(None, false);
    assert_eq!(legacy.geometry.children.len(), 12);
    assert_eq!(legacy.hits.len(), 13);
    for display in full_windows() {
        assert_eq!(observe(Some(display), false), legacy);
    }
}

#[test]
fn full_display_presence_matches_print_source_measure_and_paint_helpers() {
    let legacy = observe(None, true);
    // As in the existing readonly paint oracle, egui snaps the allocated
    // response rectangle to physical pixels. Keep variant equivalence exact.
    let delta = legacy.allocated_print.unwrap() - legacy.measured_print.unwrap().0;
    let pixel = legacy.print_pixel.unwrap();
    assert!(delta.x.abs() <= pixel && delta.y.abs() <= pixel);
    for display in full_windows() {
        assert_eq!(observe(Some(display), true), legacy);
    }
}
