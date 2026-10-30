use super::*;
use mathfolio_document::native_visual::{Paragraph, Run};

fn explicit(size: f32, color: [u8; 4]) -> TextStyle {
    TextStyle {
        font_size: size,
        foreground: color,
        bold: true,
        italic: true,
        ..TextStyle::default()
    }
}
fn para(text: &str, style: TextStyle) -> Block {
    Block::Paragraph(Paragraph {
        runs: vec![Run {
            text: text.into(),
            style,
        }],
        style: TextStyle::default(),
        margin: [0.; 4],
        line_height: None,
    })
}
fn source(blocks: Vec<Block>, plain: &str) -> RichDocument {
    RichDocument {
        blocks,
        style: TextStyle::default(),
        imported_plain_text: plain.into(),
    }
}
#[test]
fn explicit_single_empty_run_has_its_actual_font_and_color_without_synthetic_text() {
    let color = [18, 52, 86, 255];
    let style = explicit(27., color);
    let d = source(vec![para("", style)], "");
    let cache = RichTextLayout::from_validated(&d, "").unwrap();
    let job = cache.project("", None).unwrap().job("", 300.).unwrap();
    assert_eq!(job.text, "");
    assert_eq!(job.sections.len(), 1);
    assert_eq!(
        job.sections[0].byte_range.start.0..job.sections[0].byte_range.end.0,
        0..0
    );
    assert_eq!(job.sections[0].format.font_id.size, 27.);
    assert_eq!(
        job.sections[0].format.color,
        egui::Color32::from_rgb(18, 52, 86)
    );
    assert!(job.sections[0].format.italics);
}
#[test]
fn empty_first_middle_and_last_paragraph_keep_distinct_source_row_styles() {
    let d = source(
        vec![
            para("", explicit(27., [1, 2, 3, 255])),
            para("A", explicit(17., [4, 5, 6, 255])),
            para("", explicit(31., [7, 8, 9, 255])),
            para("", explicit(23., [10, 11, 12, 255])),
        ],
        "\nA\n\n",
    );
    let cache = RichTextLayout::from_validated(&d, "\nA\n\n").unwrap();
    let job = cache
        .project("\nA\n\n", None)
        .unwrap()
        .job("\nA\n\n", 300.)
        .unwrap();
    let sections = job
        .sections
        .iter()
        .map(|s| {
            (
                s.byte_range.start.0..s.byte_range.end.0,
                s.format.font_id.size,
            )
        })
        .collect::<Vec<_>>();
    assert!(sections.contains(&(0..0, 27.)));
    assert!(sections.contains(&(0..1, 27.)));
    assert!(sections.contains(&(1..2, 17.)));
    assert!(sections.contains(&(3..3, 31.)));
    assert!(sections.contains(&(3..4, 31.)));
    assert!(sections.contains(&(4..4, 23.)));
    assert_eq!(job.text, "\nA\n\n");
}
#[test]
fn deletion_preview_retains_emptied_run_style_and_unchanged_empty_siblings() {
    let d = source(
        vec![
            para("", explicit(27., [1, 2, 3, 255])),
            para("A", explicit(17., [4, 5, 6, 255])),
            para("", explicit(31., [7, 8, 9, 255])),
        ],
        "\nA\n",
    );
    let cache = RichTextLayout::from_validated(&d, "\nA\n").unwrap();
    let projection = cache
        .project(
            "\n\n",
            Some(Replacement {
                old_bytes: 1..2,
                inserted_style: cache.run_styles[1],
            }),
        )
        .unwrap();
    let job = projection.job("\n\n", 300.).unwrap();
    let sections = job
        .sections
        .iter()
        .map(|s| {
            (
                s.byte_range.start.0..s.byte_range.end.0,
                s.format.font_id.size,
            )
        })
        .collect::<Vec<_>>();
    assert!(sections.contains(&(0..0, 27.)));
    assert!(sections.contains(&(1..1, 17.)));
    assert!(sections.contains(&(2..2, 31.)));
}
#[test]
fn multiple_paragraphs_with_ambiguous_empty_runs_stay_rejected() {
    let mut d = source(vec![para("", explicit(27., [1, 2, 3, 255]))], "");
    let Block::Paragraph(p) = &mut d.blocks[0] else {
        unreachable!()
    };
    p.runs.push(Run {
        text: String::new(),
        style: explicit(31., [7, 8, 9, 255]),
    });
    // A single empty Paragraph now has an explicit visualization-only policy.
    // Several Paragraphs still receive no ambiguous Run style permission.
    d.blocks.push(para("A", explicit(17., [4, 5, 6, 255])));
    d.imported_plain_text = "\nA".into();
    assert!(RichTextLayout::from_validated(&d, "\nA").is_err());
}
#[test]
fn single_empty_row_height_matches_independent_explicit_zero_length_job() {
    let style = explicit(27., [18, 52, 86, 255]);
    let d = source(vec![para("", style.clone())], "");
    let cache = RichTextLayout::from_validated(&d, "").unwrap();
    let ctx = egui::Context::default();
    let mut fonts = egui::FontDefinitions::default();
    crate::typography::install(&mut fonts);
    crate::native_presentation::install_fonts(&mut fonts);
    ctx.set_fonts(fonts);
    let mut captured = None;
    let output = ctx.run_ui(egui::RawInput::default(), |ui| {
        let actual = cache.project("", None).and_then(|p| p.layout(ui, "", 300.));
        let mut expected = LayoutJob::default();
        expected.wrap.max_width = 300.;
        expected.sections.push(LayoutSection {
            leading_space: 0.,
            byte_range: egui::text::ByteIndex(0)..egui::text::ByteIndex(0),
            format: crate::native_presentation::text_format(&style, None),
        });
        let expected = crate::typography::layout_job(ui.painter(), expected);
        captured = Some((
            actual.map(|g| (g.size(), g.rows.len())),
            expected.size(),
            expected.rows.len(),
        ));
    });
    output.drop_without_applying_deltas();
    let (actual, size, rows) = captured.unwrap();
    let (actual_size, actual_rows) = actual.unwrap();
    assert_eq!(actual_size, size);
    assert_eq!(actual_rows, rows);
    assert!(size.y > 0.);
}

#[test]
fn deletion_preview_blank_rows_match_independent_explicit_section_heights() {
    let styles = [
        explicit(27., [1, 2, 3, 255]),
        explicit(17., [4, 5, 6, 255]),
        explicit(31., [7, 8, 9, 255]),
    ];
    let d = source(
        vec![
            para("", styles[0].clone()),
            para("A", styles[1].clone()),
            para("", styles[2].clone()),
        ],
        "\nA\n",
    );
    let cache = RichTextLayout::from_validated(&d, "\nA\n").unwrap();
    let projected = cache
        .project(
            "\n\n",
            Some(Replacement {
                old_bytes: 1..2,
                inserted_style: cache.run_styles[1],
            }),
        )
        .unwrap();
    let ctx = egui::Context::default();
    let mut fonts = egui::FontDefinitions::default();
    crate::typography::install(&mut fonts);
    crate::native_presentation::install_fonts(&mut fonts);
    ctx.set_fonts(fonts);
    let mut capture = None;
    let output = ctx.run_ui(egui::RawInput::default(), |ui| {
        let actual = projected.layout(ui, "\n\n", 300.);
        let mut expected = LayoutJob {
            text: "\n\n".into(),
            ..Default::default()
        };
        expected.wrap.max_width = 300.;
        for (range, index) in [(0..0, 0), (0..1, 0), (1..1, 1), (1..2, 1), (2..2, 2)] {
            expected.sections.push(LayoutSection {
                leading_space: 0.,
                byte_range: egui::text::ByteIndex(range.start)..egui::text::ByteIndex(range.end),
                format: crate::native_presentation::text_format(&styles[index], None),
            });
        }
        let expected = crate::typography::layout_job(ui.painter(), expected);
        capture = Some((
            actual.map(|g| (g.size(), g.rows.len())),
            expected.size(),
            expected.rows.len(),
        ));
    });
    output.drop_without_applying_deltas();
    let (actual, size, rows) = capture.unwrap();
    let (actual_size, actual_rows) = actual.unwrap();
    assert_eq!(actual_size, size);
    assert_eq!(actual_rows, 3);
    assert_eq!(actual_rows, rows);
}
