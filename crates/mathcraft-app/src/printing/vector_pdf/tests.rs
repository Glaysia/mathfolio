use super::*;
use crate::printing::{PageSelection, PrintPlan, PrintSettings, render_vector_pages, write_pdf};
use std::io::Read;

fn fixture(dpi: u16) -> Vec<RasterPage> {
    let plan = PrintPlan::new(&PrintSettings {
        paper_size: [400., 600.],
        margins: [20.; 4],
        body_height: 560.,
        content_bottom: 0.,
        pages: PageSelection::All,
        dpi,
    })
    .unwrap();
    render_vector_pages(&plan, |ui, _, _| {
        let mut fonts = ui.fonts(|f| f.definitions().clone());
        let family = egui::FontFamily::Name("pdf-ligature".into());
        let font = if let std::collections::btree_map::Entry::Vacant(entry) =
            fonts.families.entry(family.clone())
        {
            entry.insert(vec!["Ubuntu-Light".into()]);
            ui.ctx().set_fonts(fonts);
            egui::FontId::proportional(24.)
        } else {
            egui::FontId::new(24., family)
        };
        let p = ui.painter();
        p.text(
            Pos2::new(20., 20.),
            egui::Align2::LEFT_TOP,
            "office affine ffi",
            font,
            Color32::BLACK,
        );
        p.text(
            Pos2::new(20., 60.),
            egui::Align2::LEFT_TOP,
            "α = 3.5 Ω",
            egui::FontId::new(22., egui::FontFamily::Name("math-regular".into())),
            Color32::from_rgb(30, 70, 160),
        );
        p.text(
            Pos2::new(20., 95.),
            egui::Align2::LEFT_TOP,
            "𝑥 = √2",
            egui::FontId::new(20., egui::FontFamily::Name("math-regular".into())),
            Color32::BLACK,
        );
        let galley = p.layout_no_wrap(
            "rotated".into(),
            egui::FontId::proportional(18.),
            Color32::BLACK,
        );
        p.add(
            egui::epaint::TextShape::new(Pos2::new(250., 100.), galley, Color32::BLACK)
                .with_angle(std::f32::consts::FRAC_PI_2),
        );
        p.rect_filled(
            Rect::from_min_size(Pos2::new(20., 160.), Vec2::new(120., 40.)),
            5.,
            Color32::from_rgba_unmultiplied(255, 0, 0, 128),
        );
        p.line_segment(
            [Pos2::new(20., 220.), Pos2::new(140., 220.)],
            Stroke::new(2.5, Color32::BLUE),
        );
        p.circle_stroke(Pos2::new(200., 180.), 20., Stroke::new(1.5, Color32::BLACK));
        p.add(egui::epaint::QuadraticBezierShape::from_points_stroke(
            [
                Pos2::new(20., 260.),
                Pos2::new(80., 230.),
                Pos2::new(140., 260.),
            ],
            false,
            Color32::TRANSPARENT,
            Stroke::new(1., Color32::BLACK),
        ));
        p.with_clip_rect(Rect::from_min_max(
            Pos2::new(20., 300.),
            Pos2::new(140., 320.),
        ))
        .line_segment(
            [Pos2::new(0., 310.), Pos2::new(200., 310.)],
            Stroke::new(3., Color32::GREEN),
        );
        let mut rich = egui::text::LayoutJob::default();
        rich.append(
            "decorated text",
            0.,
            egui::TextFormat {
                font_id: egui::FontId::proportional(18.),
                color: Color32::from_rgb(40, 80, 150),
                italics: true,
                underline: Stroke::new(0.8, Color32::from_rgb(40, 80, 150)),
                strikethrough: Stroke::new(1., Color32::from_rgb(40, 80, 150)),
                background: Color32::from_rgb(240, 240, 180),
                ..Default::default()
            },
        );
        p.galley(Pos2::new(20., 360.), p.layout_job(rich), Color32::BLACK);
    })
    .unwrap()
}

fn decoded_streams(pdf: &[u8]) -> String {
    let mut result = String::new();
    let mut rest = pdf;
    while let Some(start) = rest.windows(7).position(|s| s == b"stream\n") {
        rest = &rest[start + 7..];
        let end = rest.windows(10).position(|s| s == b"\nendstream").unwrap();
        let mut decoded = String::new();
        let _ = flate2::read::ZlibDecoder::new(&rest[..end]).read_to_string(&mut decoded);
        result.push_str(&decoded);
        rest = &rest[end + 10..];
    }
    result
}

#[test]
fn vector_pages_embed_searchable_unicode_and_shaped_ligatures_without_a_raster() {
    let pages = fixture(600);
    assert!(pages.iter().all(|p| p.png.is_empty()));
    let pdf = encode(&pages).unwrap();
    let source = String::from_utf8_lossy(&pdf);
    assert!(source.contains("/FontFile2") && source.contains("/ToUnicode"));
    assert!(source.contains("/MediaBox [0 0 300.00000 450.00000]"));
    assert!(!source.contains("/Subtype /Image"));
    let streams = decoded_streams(&pdf);
    assert!(streams.contains("<03B1>"));
    assert!(streams.contains("<D835DC65>")); // mathematical italic small x
    assert!(streams.contains("<006600660069>")); // actual ffi ligature maps to three characters
    assert!(streams.contains("BT /F") && streams.contains(" Tj ET"));
}

#[test]
fn vector_geometry_preserves_physical_size_clip_curves_alpha_width_and_rotation() {
    let pdf = encode(&fixture(96)).unwrap();
    let source = String::from_utf8_lossy(&pdf);
    assert!(source.contains("/ca 0.5019608 /CA 0.5019608"));
    let streams = decoded_streams(&pdf);
    assert!(streams.contains("0.75 0 0 -0.75 0 450.00000 cm"));
    assert!(streams.contains("20.00000 300.00000 120.00000 20.00000 re W n"));
    assert!(streams.contains("2.50000 w"));
    assert!(streams.contains(" c\n"));
    assert!(streams.contains("-0.0000000 1.0000000 1.0000000 0.0000000")); // clockwise rotated text matrix
    assert_eq!(decoded_streams(&encode(&fixture(600)).unwrap()), streams);
}

#[test]
fn unsupported_content_fails_without_replacing_an_existing_pdf() {
    let mut pages = fixture(96);
    pages[0].vector.shapes.push(ClippedShape {
        clip_rect: Rect::EVERYTHING,
        shape: Shape::Callback(egui::epaint::PaintCallback {
            rect: Rect::from_min_size(Pos2::ZERO, Vec2::splat(50.)),
            callback: Arc::new(()),
        }),
    });
    let directory = crate::printing::JobDirectory::create().unwrap();
    let path = directory.0.join("existing.pdf");
    std::fs::write(&path, b"original contents").unwrap();
    assert!(write_pdf(&path, &pages).unwrap_err().contains("GPU"));
    assert_eq!(std::fs::read(path).unwrap(), b"original contents");
}

#[test]
fn nonfinite_paths_and_textures_are_reported_instead_of_omitted() {
    let mut pages = fixture(96);
    pages[0].vector.shapes.push(ClippedShape {
        clip_rect: Rect::EVERYTHING,
        shape: Shape::line_segment(
            [Pos2::new(f32::NAN, 2.), Pos2::new(3., 4.)],
            Stroke::new(1., Color32::BLACK),
        ),
    });
    assert!(encode(&pages).unwrap_err().contains("유한"));
    pages[0].vector.shapes.pop();
    let mut mesh = egui::epaint::Mesh::with_texture(egui::TextureId::User(8));
    mesh.add_rect_with_uv(
        Rect::from_min_size(Pos2::ZERO, Vec2::splat(10.)),
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1., 1.)),
        Color32::WHITE,
    );
    pages[0].vector.shapes.push(ClippedShape {
        clip_rect: Rect::EVERYTHING,
        shape: Shape::mesh(mesh),
    });
    assert!(encode(&pages).unwrap_err().contains("텍스처"));
}

#[test]
#[ignore = "writes a PDF fixture for independent pypdf/PyMuPDF inspection"]
fn write_vector_pdf_backend_fixture() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../artifacts/rea/vector-pdf-backend.pdf");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    write_pdf(&path, &fixture(600)).unwrap();
    println!("{}", path.display());
}

fn cjk_pages(bytes: Vec<u8>) -> Vec<RasterPage> {
    let plan = PrintPlan::new(&PrintSettings {
        paper_size: [400., 600.],
        margins: [20.; 4],
        body_height: 560.,
        content_bottom: 0.,
        pages: PageSelection::All,
        dpi: 600,
    })
    .unwrap();
    render_vector_pages(&plan, |ui, _, _| {
        let mut fonts = ui.fonts(|f| f.definitions().clone());
        let family = egui::FontFamily::Name("pdf-cjk".into());
        if !fonts.families.contains_key(&family) {
            fonts
                .font_data
                .insert("pdf-cjk".into(), FontData::from_owned(bytes.clone()).into());
            fonts.families.insert(family, vec!["pdf-cjk".into()]);
            ui.ctx().set_fonts(fonts);
            return;
        }
        ui.painter().text(
            Pos2::new(20., 30.),
            egui::Align2::LEFT_TOP,
            "한글 표 인쇄 ABC 123",
            egui::FontId::new(24., family),
            Color32::from_rgb(20, 60, 140),
        );
    })
    .unwrap()
}

#[test]
fn collection_fonts_use_subset_vector_glyphs_and_single_byte_unicode_maps() {
    // A valid single-face TTC around the bundled Ubuntu font exercises the same
    // collection fallback portably, without relying on an installed OS font.
    let fonts = FontDefinitions::default();
    let source = &fonts.font_data["Ubuntu-Light"].font;
    let mut collection = b"ttcf\0\x01\0\0\0\0\0\x01\0\0\0\x10".to_vec();
    collection.extend_from_slice(source);
    let count = u16::from_be_bytes(source[4..6].try_into().unwrap());
    for index in 0..usize::from(count) {
        let at = 16 + 12 + index * 16 + 8;
        let offset = u32::from_be_bytes(collection[at..at + 4].try_into().unwrap()) + 16;
        collection[at..at + 4].copy_from_slice(&offset.to_be_bytes());
    }
    let mut resources = Resources::default();
    let mut used = UsedFont {
        data: FontData::from_owned(collection).into(),
        chars: BTreeMap::new(),
    };
    let font = FontRef::from_index(&used.data.font, 0).unwrap();
    let gid = font.charmap().map('A').unwrap().to_u32() as u16;
    used.chars.insert((gid, "A".into()), (1, 684.));
    resources.fonts.push(used);
    let mut objects = Objects::default();
    let refs = resources.font_objects(&mut objects).unwrap();
    assert!(refs.starts_with("/F0p0 "));
    let data = objects.finish(1).unwrap();
    let source = String::from_utf8_lossy(&data);
    assert!(source.contains("/Subtype /Type3"));
    assert!(!source.contains("/FontFile2") && !source.contains("/Subtype /Image"));
    let streams = decoded_streams(&data);
    assert!(streams.contains("<01> <0041>"));
    assert!(streams.contains(" d1\n") && streams.contains("m\n"));
}

#[test]
#[ignore = "requires the independently licensed Noto CJK font copied from peets"]
fn write_vector_cff_korean_fixture() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../artifacts/rea");
    let bytes = std::fs::read(root.join("NotoSansCJK-Regular.ttc")).unwrap();
    let pages = cjk_pages(bytes);
    let data = encode(&pages).unwrap();
    assert!(String::from_utf8_lossy(&data).contains("/Subtype /Type3"));
    write_pdf(&root.join("vector-cff-korean.pdf"), &pages).unwrap();
}
