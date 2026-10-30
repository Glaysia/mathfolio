//! Independent Area frame/counter audit. Test-only; never original GUI/runtime.
use super::*;
fn source(count: usize) -> String {
    let ws = "http://schemas.mathsoft.com/worksheet50";
    let ml = "http://schemas.mathsoft.com/math50";
    let children=(0..count).map(|i|format!("<region region-id='{i}' left='10' top='{}'><math resultRef='{i}'><ml:eval><ml:real>{}</ml:real><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval></math></region>",30+i*45,12345+i)).collect::<String>();
    format!(
        "<region xmlns='{ws}' xmlns:ml='{ml}' region-id='owner'><Area AreaHeight='200'><regions>{children}</regions></Area></region>"
    )
}
fn area_frame(
    editor: &mut area_ui::AreaEditor,
    source: &mut String,
    ctx: &egui::Context,
    font: f32,
) -> serde_json::Value {
    crate::frame_profile::record(|| {
        let frame = crate::frame_profile::Frame::start();
        ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1200., 900.))),
                ..Default::default()
            },
            |ui| {
                let outcome = editor.show(ui, source, &HashMap::new(), font);
                assert!(
                    !outcome.document_changed
                        && !outcome.draft_changed
                        && !outcome.calculation_changed
                );
            },
        )
        .drop_without_applying_deltas();
        frame.finish().unwrap()
    })
}
#[test]
fn area_warm_children_share_their_geometry_and_paint_cache_owner() {
    for count in [1, 2, 8] {
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let mut xml = source(count);
        let original = xml.clone();
        let mut area = area_ui::AreaEditor::new(&xml).unwrap();
        for _ in 0..3 {
            area_frame(&mut area, &mut xml, &ctx, 11.);
        }
        let warm = area_frame(&mut area, &mut xml, &ctx, 11.);
        assert_eq!(
            warm["phases"]["formula_typeset"]["calls"], 0,
            "count={count}, {warm}"
        );
        assert_eq!(xml, original);
        assert!(!area.has_draft_changes());
        let changed = area_frame(&mut area, &mut xml, &ctx, 17.);
        assert!(
            changed["phases"]["formula_typeset"]["calls"]
                .as_u64()
                .unwrap()
                > 0
        );
        let next = area_frame(&mut area, &mut xml, &ctx, 17.);
        assert_eq!(
            next["phases"]["formula_typeset"]["calls"], 0,
            "font invalidation must settle"
        );
    }
}

fn resolved_source(flags: &str, metadata: &str) -> String {
    let ws = "http://schemas.mathsoft.com/worksheet50";
    let ml = "http://schemas.mathsoft.com/math50";
    format!(
        "<region xmlns='{ws}' xmlns:ml='{ml}' region-id='owner'><Area {flags}><regions><region region-id='definition' left='0' top='0'><math {metadata} resultRef='1'><ml:define><ml:id>m</ml:id><ml:real>7</ml:real></ml:define></math></region><region region-id='query' left='0' top='40'><math {metadata} resultRef='2'><ml:eval><ml:apply><ml:mult/><ml:id>m</ml:id><ml:id labels='UNIT'>m</ml:id></ml:apply><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval><resultFormat/></math></region></regions></Area></region>"
    )
}
fn identifier_labels(tree: &PortableTree) -> Vec<(String, String)> {
    let mut stack = vec![tree];
    let mut labels = vec![];
    while let Some(node) = stack.pop() {
        match &node.kind {
            NodeKind::AnnotatedIdentifier { name, labels: l }
            | NodeKind::NativeIdentifier {
                name, labels: l, ..
            } => labels.push((name.clone(), l.clone())),
            _ => {}
        }
        stack.extend(node.children.iter().rev());
    }
    labels
}
#[test]
fn resolved_area_occurrence_labels_are_eager_source_bound_and_cached_without_execution() {
    for metadata in ["", "disable-calc='false'", "disable-calc='0'"] {
        let mut app = WorksheetApp::default();
        let original = resolved_source("IsCollapsed='true'", metadata);
        let owner = app.document.add_opaque(0., 0., original.clone(), "Area");
        let input = app.prepare_calculation_input().unwrap();
        assert!(app.last_executed_regions.is_empty());
        assert!(app.results.is_empty());
        assert!(input.faults.is_empty());
        assert_eq!(input.area_labelled[&owner].len(), 2);
        let labels = identifier_labels(&input.area_labelled[&owner]["[0,0,1]"]);
        assert_eq!(
            labels,
            [("m".into(), "VARIABLE".into()), ("m".into(), "UNIT".into())]
        );
        assert!(
            matches!(&app.document.regions[0].content,CanvasContent::Opaque{xml,..}if xml==&original)
        );
        let cached = app.prepare_calculation_input().unwrap();
        assert!(Arc::ptr_eq(&input.area_labelled, &cached.area_labelled));
        // A source edit creates a new labelled projection and prunes old child data.
        app.document.regions[0].content = CanvasContent::Opaque {
            kind: "Area".into(),
            xml: original.replace("<ml:id>m</ml:id><ml:real>7", "<ml:id>x</ml:id><ml:real>7"),
        };
        let changed = app.prepare_calculation_input().unwrap();
        assert!(!Arc::ptr_eq(&input.area_labelled, &changed.area_labelled));
        assert_eq!(
            identifier_labels(&changed.area_labelled[&owner]["[0,0,1]"])[0],
            ("m".into(), "UNIT".into())
        );
    }
}
#[test]
fn unsupported_area_or_child_metadata_has_no_resolved_projection_or_new_calc_fault() {
    let mut app = WorksheetApp::default();
    let owner = app
        .document
        .add_opaque(0., 0., resolved_source("disable-calc='true'", ""), "Area");
    let input = app.prepare_calculation_input().unwrap();
    assert!(input.area_labelled.is_empty());
    assert!(input.faults.is_empty());
    assert!(app.last_executed_regions.is_empty());
    app.document.regions[0].content = CanvasContent::Opaque {
        kind: "Area".into(),
        xml: resolved_source("", "future='unsupported'"),
    };
    let input = app.prepare_calculation_input().unwrap();
    assert!(!input.area_labelled.contains_key(&owner));
    assert!(input.faults.is_empty());
    assert!(app.last_executed_regions.is_empty());
}

fn app_frame(app: &mut WorksheetApp, ctx: &egui::Context, events: Vec<egui::Event>) {
    ctx.run_ui(
        egui::RawInput {
            events,
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1200., 900.))),
            ..Default::default()
        },
        |ui| app.page_ui(ui),
    )
    .drop_without_applying_deltas();
}
#[test]
#[allow(deprecated)]
fn area_navigation_composition_and_text_drafts_do_not_prepare_or_execute_source() {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut app = WorksheetApp::default();
    let original = source(2);
    let owner = app.document.add_opaque(20., 20., original.clone(), "Area");
    app.selected = Some(owner);
    app.editing_region = Some(owner);
    app.focus_new = Some(owner);
    app.insertion = None;
    app_frame(&mut app, &ctx, vec![]);
    app_frame(&mut app, &ctx, vec![]);
    for event in [
        egui::Event::Key {
            key: egui::Key::ArrowLeft,
            physical_key: Some(egui::Key::ArrowLeft),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        },
        egui::Event::Key {
            key: egui::Key::ArrowLeft,
            physical_key: Some(egui::Key::ArrowLeft),
            pressed: false,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        },
        egui::Event::Ime(egui::ImeEvent::Preedit {
            text: "draft composition".into(),
            active_range_chars: None,
        }),
    ] {
        app_frame(&mut app, &ctx, vec![event]);
        assert!(app.committed_input.is_none());
        assert!(!app.background.preparation.has_committed_baseline());
        assert!(app.last_executed_regions.is_empty());
        assert!(app.uncommitted_math.is_empty());
    }
    app_frame(
        &mut app,
        &ctx,
        vec![egui::Event::Ime(egui::ImeEvent::Disabled)],
    );
    let editor = app
        .area_editors
        .get_mut(&owner)
        .unwrap()
        .active_editor_mut()
        .unwrap();
    let operand = editor.arena().get(editor.root()).unwrap().children()[0];
    editor.place_text_selection(operand, 0, 5).unwrap();
    app_frame(&mut app, &ctx, vec![]);
    app_frame(&mut app, &ctx, vec![egui::Event::Text("7".into())]);
    assert!(
        app.area_editors[&owner].has_draft_changes(),
        "selected={:?} editing={:?} active={:?} focus={:?} tree={:?} status={} xml={:?}",
        app.selected,
        app.editing_region,
        app.area_editors[&owner].active_child(),
        ctx.memory(|m| m.focused()),
        app.area_editors[&owner]
            .active_editor()
            .map(|e| e.to_portable()),
        app.status,
        app.document.regions[0].content
    );
    assert!(app.uncommitted_math.contains(&owner));
    assert!(app.committed_input.is_none());
    assert!(app.background.preparation.has_committed_baseline());
    assert!(app.last_executed_regions.is_empty());
    assert!(
        matches!(&app.document.regions[0].content,CanvasContent::Opaque{xml,..}if xml==&original)
    );
}
