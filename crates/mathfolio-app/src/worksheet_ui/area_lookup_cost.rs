//! Test-only large-Area CPU profile and exact-source index regressions. No original runtime, GUI, screenshots or engine runs.
use super::*;
use mathfolio_document::native_visual::{
    Block, Paragraph, RegionVisual, RichDocument, Run, TextStyle,
};
use std::time::Instant;

struct Fixture {
    xml: String,
    math_count: usize,
    child_count: usize,
    results: Vec<crate::result_display::FormattedResult>,
    native: RegionVisual,
    mixed: bool,
}
impl Fixture {
    fn new(count: usize, mixed: bool, collapsed: bool) -> Self {
        const WS: &str = "http://schemas.mathsoft.com/worksheet50";
        const ML: &str = "http://schemas.mathsoft.com/math50";
        let mut children = String::new();
        let mut results = Vec::new();
        let mut math_count = 0;
        for i in 0..count {
            use std::fmt::Write;
            write!(
                children,
                "<region region-id='{i}' left='10' top='{}'>",
                30 + i * 36
            )
            .unwrap();
            if mixed && i % 5 == 4 {
                children.push_str("<text/>");
            } else {
                write!(children,"<math resultRef='{i}'><ml:eval><ml:real>{}</ml:real><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval></math>",i+1).unwrap();
                math_count += 1;
            }
            children.push_str("</region>");
            results.push(crate::result_display::format(
                &Value::Scalar {
                    quantity: mathfolio_core::Quantity::scalar((i + 1) as f64),
                },
                &MathFormat::default(),
            ));
        }
        let xml = format!(
            "<region xmlns='{WS}' xmlns:ml='{ML}' region-id='owner'><Area AreaHeight='{}' IsCollapsed='{collapsed}'><regions>{children}</regions></Area></region>",
            count * 36 + 60
        );
        let style = TextStyle::default();
        let native = RegionVisual {
            size: [80., 20.],
            background: None,
            document: Some(RichDocument {
                blocks: vec![Block::Paragraph(Paragraph {
                    runs: vec![Run {
                        text: "native note".into(),
                        style: style.clone(),
                    }],
                    style: style.clone(),
                    margin: [0.; 4],
                    line_height: None,
                })],
                style,
                imported_plain_text: "native note".into(),
            }),
        };
        Self {
            xml,
            math_count,
            child_count: count,
            results,
            native,
            mixed,
        }
    }
    fn views(&self) -> HashMap<String, area_ui::AreaChildView<'_>> {
        (0..self.child_count)
            .map(|i| {
                let inert = self.mixed && i % 5 == 4;
                (
                    area_ui::child_key(&[0, 0, i]),
                    area_ui::AreaChildView {
                        display: (!inert).then_some(&self.results[i]),
                        native: inert.then_some(&self.native),
                        local: Some([10., (30 + i * 36) as f64]),
                        kind: inert.then_some("text"),
                        font_points: Some(if i % 2 == 0 { 11. } else { 12. }),
                        ..Default::default()
                    },
                )
            })
            .collect()
    }
}
fn frame(
    editor: &mut area_ui::AreaEditor,
    xml: &mut String,
    views: &HashMap<String, area_ui::AreaChildView<'_>>,
    ctx: &egui::Context,
    observe: bool,
) -> serde_json::Value {
    let mut run = || {
        let profile = crate::frame_profile::Frame::start();
        let start = Instant::now();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1200., 900.))),
                ..Default::default()
            },
            |ui| {
                let response = editor.show(ui, xml, views, 11.);
                assert!(
                    !response.document_changed
                        && !response.calculation_changed
                        && !response.draft_changed
                        && !response.history_changed
                );
            },
        );
        let ui_ns = start.elapsed().as_nanos();
        let shapes = output.shapes.len();
        let start = Instant::now();
        let primitives = {
            let _phase = crate::frame_profile::scope(crate::frame_profile::Phase::Tessellation);
            ctx.tessellate(std::mem::take(&mut output.shapes), output.pixels_per_point)
        };
        let tessellation_ns = start.elapsed().as_nanos();
        let mut vertices = 0usize;
        let mut indices = 0usize;
        for primitive in &primitives {
            if let egui::epaint::Primitive::Mesh(mesh) = &primitive.primitive {
                vertices += mesh.vertices.len();
                indices += mesh.indices.len();
            }
        }
        std::hint::black_box(&primitives);
        drop(primitives);
        output.drop_without_applying_deltas();
        let phases = profile.finish();
        serde_json::json!({"ui_ns":ui_ns,"tessellation_ns":tessellation_ns,"ui_plus_tessellation_ns":ui_ns+tessellation_ns,"shapes":shapes,"vertices":vertices,"indices":indices,"profile":phases})
    };
    if observe {
        crate::frame_profile::record(run)
    } else {
        run()
    }
}
fn one_case(count: usize, mixed: bool, collapsed: bool, observe: bool) -> serde_json::Value {
    let fixture = Fixture::new(count, mixed, collapsed);
    let original = fixture.xml.clone();
    let mut xml = original.clone();
    let views = fixture.views();
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let start = Instant::now();
    let mut editor = area_ui::AreaEditor::new(&xml).unwrap();
    let construct_ns = start.elapsed().as_nanos();
    assert_eq!(editor.math_sources().count(), fixture.math_count);
    let cold = frame(&mut editor, &mut xml, &views, &ctx, observe);
    let mut warm = Vec::new();
    for _ in 0..3 {
        warm.push(frame(&mut editor, &mut xml, &views, &ctx, observe));
    }
    assert_eq!(xml, original);
    assert!(!editor.has_draft_changes());
    assert_eq!(editor.math_sources().count(), fixture.math_count);
    if observe {
        let expected = if collapsed {
            0
        } else {
            fixture.math_count as u64
        };
        assert_eq!(
            cold["profile"]["phases"]["formula_initial_layout"]["calls"].as_u64(),
            Some(expected)
        );
        assert_eq!(
            cold["profile"]["phases"]["formula_typeset"]["calls"].as_u64(),
            Some(expected)
        );
        for sample in &warm {
            assert_eq!(sample["profile"]["phases"]["formula_typeset"]["calls"], 0);
            assert_eq!(
                sample["profile"]["phases"]["formula_initial_layout"]["calls"].as_u64(),
                Some(expected)
            );
        }
    }
    let (index, keys) = editor.lookup_cost_index_snapshot();
    assert_eq!(index.len(), keys.len());
    let index_memory = serde_json::json!({
        "entries": index.len(), "capacity": index.capacity(),
        "owned_key_bytes": index.keys().map(String::len).sum::<usize>(),
        "owned_key_capacity_bytes": index.keys().map(String::capacity).sum::<usize>(),
        "entry_capacity_bytes": index.capacity() * std::mem::size_of::<(String, usize)>(),
        "accounting": "owned key allocation capacity plus entry-capacity model; excludes control bytes, allocator overhead; not RSS"
    });
    serde_json::json!({"index_memory":index_memory,"children":count,"math_children":fixture.math_count,"inert_children":count-fixture.math_count,"mixed":mixed,"collapsed":collapsed,"observer":observe,"viewport_height":900,"last_child_y":30+(count-1)*36,"source_bytes":xml.len(),"construct_ns":construct_ns,"cold":cold,"warm":warm,"contracts":{"source_unchanged":true,"all_math_children_retained":true,"eager_formula_calls_checked":observe,"warm_typeset_zero_checked":observe,"no_events_or_calculation":true}})
}
#[test]
#[ignore = "CPU-only large Area source/frame+tessellation profile; reserve quiet CPU, not original runtime"]
fn area_large_frame_cost_profile() {
    let observe = std::env::var_os("MATHFOLIO_AREA_LOOKUP_OBSERVE").is_some();
    let counts = std::env::var("MATHFOLIO_AREA_LOOKUP_COUNTS")
        .ok()
        .map(|raw| {
            raw.split(',')
                .map(|n| n.parse::<usize>().unwrap())
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| vec![16, 256, 1024, 2048]);
    for count in counts {
        assert!(count > 0 && count <= 4096);
        for mixed in [false, true] {
            println!("AREA_LOOKUP {}", one_case(count, mixed, false, observe));
        }
        if count == 1024 {
            println!("AREA_LOOKUP {}", one_case(count, false, true, observe));
        }
    }
}

fn assert_index_matches_linear(editor: &area_ui::AreaEditor, paths: &[Vec<usize>]) {
    let (index, keys) = editor.lookup_cost_index_snapshot();
    assert_eq!(index.len(), keys.len());
    for (position, key) in keys.iter().enumerate() {
        assert_eq!(index.get(*key), Some(&position));
    }
    for key in paths.iter().map(|path| area_ui::child_key(path)).chain([
        "[]".into(),
        "[999,0,1]".into(),
        "not a path".into(),
    ]) {
        assert_eq!(
            index.get(&key).copied(),
            keys.iter().position(|old| *old == key)
        );
    }
    let actual: Vec<_> = editor
        .math_sources()
        .map(|source| area_ui::child_key(&source.region_path))
        .collect();
    assert_eq!(keys, actual.iter().map(String::as_str).collect::<Vec<_>>());
}

#[test]
fn area_lookup_index_matches_linear_for_mixed_readonly_and_duplicate_ids() {
    let fixture = Fixture::new(16, true, false);
    // Uneditable math must remain indexed, unlike native text. IDs may repeat;
    // the source element path, rather than region-id, identifies each child.
    let xml = fixture
        .xml
        .replace("<ml:real>3</ml:real>", "<ml:unsupported/>")
        .replace("region-id='3'", "region-id='2'");
    let mut editor = area_ui::AreaEditor::new(&xml).unwrap();
    let paths = (0..16).map(|i| vec![0, 0, i]).collect::<Vec<_>>();
    assert_index_matches_linear(&editor, &paths);
    assert!(editor.activate(&area_ui::child_key(&[0, 0, 2])).is_err());
    assert!(
        editor
            .lookup_cost_index_snapshot()
            .0
            .contains_key(&area_ui::child_key(&[0, 0, 2]))
    );
}

#[test]
fn area_lookup_index_tracks_source_restore_insert_remove_and_failed_install() {
    let fixture = Fixture::new(8, true, false);
    let mut xml = fixture.xml.clone();
    let mut editor = area_ui::AreaEditor::new(&xml).unwrap();
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let views = fixture.views();
    let paths = (0..10).map(|i| vec![0, 0, i]).collect::<Vec<_>>();
    let mut versions = vec![
        Fixture::new(3, false, false).xml,
        Fixture::new(10, true, false).xml,
        fixture.xml.clone(),
    ];
    // Reorder native IDs and values without altering their actual element paths.
    versions.push(
        fixture
            .xml
            .replace("region-id='0'", "region-id='7'")
            .replace("region-id='1'", "region-id='0'"),
    );
    for version in versions {
        xml = version;
        frame(&mut editor, &mut xml, &views, &ctx, false);
        assert_index_matches_linear(&editor, &paths);
    }
    let before = editor.lookup_cost_index_snapshot().0.clone();
    xml = "<region><Area>".into();
    frame(&mut editor, &mut xml, &views, &ctx, false);
    assert_eq!(editor.lookup_cost_index_snapshot().0, &before);
    assert_index_matches_linear(&editor, &paths);
    xml = fixture.xml.clone();
    frame(&mut editor, &mut xml, &views, &ctx, false);
    editor.set_display(&mut xml, Some(true), None).unwrap();
    assert_index_matches_linear(&editor, &paths);
    editor.set_display(&mut xml, Some(false), None).unwrap();
    editor.activate_insertion().unwrap();
    editor.begin_math_at_insertion(&mut xml).unwrap();
    assert_index_matches_linear(&editor, &paths);
}

#[test]
fn area_lookup_index_reinstall_preserves_active_editor_caret_and_selection() {
    let fixture = Fixture::new(8, false, false);
    let mut xml = fixture.xml.clone();
    let mut editor = area_ui::AreaEditor::new(&xml).unwrap();
    let key = area_ui::child_key(&[0, 0, 3]);
    editor.activate(&key).unwrap();
    let active = editor.active_editor().unwrap();
    let before = (
        active.to_portable().unwrap(),
        active.caret(),
        active.selection(),
    );
    editor.set_display(&mut xml, None, Some(true)).unwrap();
    assert_eq!(editor.active_child(), Some(key.as_str()));
    let active = editor.active_editor().unwrap();
    assert_eq!(
        (
            active.to_portable().unwrap(),
            active.caret(),
            active.selection()
        ),
        before
    );
    assert_index_matches_linear(&editor, &(0..8).map(|i| vec![0, 0, i]).collect::<Vec<_>>());
}
