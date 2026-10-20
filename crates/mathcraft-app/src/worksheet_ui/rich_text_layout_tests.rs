use super::*;
use mathcraft_document::native_visual::{Paragraph, Run};

fn document() -> RichDocument {
    let regular = TextStyle::default();
    let bold = TextStyle {
        bold: true,
        italic: true,
        foreground: [10, 20, 30, 128],
        background: Some([40, 50, 60, 128]),
        font_size: 24.,
        ..regular.clone()
    };
    RichDocument {
        blocks: vec![
            Block::Paragraph(Paragraph {
                runs: vec![
                    Run {
                        text: "한😀".into(),
                        style: regular.clone(),
                    },
                    Run {
                        text: "AB".into(),
                        style: bold,
                    },
                ],
                style: regular.clone(),
                margin: [0.; 4],
                line_height: None,
            }),
            Block::Paragraph(Paragraph {
                runs: vec![],
                style: regular.clone(),
                margin: [0.; 4],
                line_height: None,
            }),
        ],
        style: regular,
        imported_plain_text: "한😀AB\n".into(),
    }
}

#[test]
fn exact_utf8_spans_and_paragraph_scalar_offsets() {
    let cache = RichTextLayout::from_validated(&document(), "한😀AB\n").unwrap();
    assert_eq!(cache.paragraph_bytes(0, 1..2).unwrap(), 3..7);
    assert_eq!(cache.paragraphs[1].scalar_start, 5);
    assert_eq!(cache.paragraphs[1].bytes, 10..10);
    let projected = cache.project("한😀AB\n", None).unwrap();
    let job = projected.job("한😀AB\n", 300.).unwrap();
    assert_eq!(job.text, "한😀AB\n");
    assert_eq!(
        job.sections
            .iter()
            .map(|s| s.byte_range.start.0..s.byte_range.end.0)
            .collect::<Vec<_>>(),
        vec![0..7, 7..9, 9..10]
    );
    assert!(job.sections[1].format.italics);
    assert_ne!(
        job.sections[1].format.background,
        egui::Color32::TRANSPARENT
    );
    assert_eq!(job.sections[1].format.font_id.size, 24.);
}

#[test]
fn preedit_replacement_uses_explicit_run_style_and_preserves_suffix() {
    let cache = RichTextLayout::from_validated(&document(), "한😀AB\n").unwrap();
    let projection = cache
        .project(
            "한조합AB\n",
            Some(Replacement {
                old_bytes: 3..7,
                inserted_style: cache.run_styles[1],
            }),
        )
        .unwrap();
    let job = projection.job("한조합AB\n", 300.).unwrap();
    assert_eq!(job.text, "한조합AB\n");
    assert_eq!(
        job.sections[1].byte_range.start.0..job.sections[1].byte_range.end.0,
        3..9
    );
    assert!(job.sections[1].format.italics);
    assert_eq!(
        job.sections[2].byte_range.start.0..job.sections[2].byte_range.end.0,
        9..11
    );
    // Immutable source/cancel path remains unchanged.
    assert_eq!(
        cache
            .project("한😀AB\n", None)
            .unwrap()
            .job("한😀AB\n", 300.)
            .unwrap()
            .text,
        "한😀AB\n"
    );
}

#[test]
fn mismatch_multibyte_boundary_and_stale_projection_fail_closed() {
    let cache = RichTextLayout::from_validated(&document(), "한😀AB\n").unwrap();
    assert!(cache.project("wrong", None).is_err());
    assert!(
        cache
            .project(
                "한xAB\n",
                Some(Replacement {
                    old_bytes: 4..7,
                    inserted_style: 0
                })
            )
            .is_err()
    );
    let p = cache.project("한😀AB\n", None).unwrap();
    assert!(p.job("한😀ABC\n", 300.).is_err());
    let mut bad = document();
    let Block::Paragraph(p) = &mut bad.blocks[0] else {
        unreachable!()
    };
    p.margin[0] = 5.;
    assert!(RichTextLayout::from_validated(&bad, "한😀AB\n").is_err());
}

#[test]
fn empty_source_never_adds_synthetic_space() {
    let mut d = document();
    d.blocks.clear();
    let c = RichTextLayout::from_validated(&d, "").unwrap();
    assert_eq!(c.project("", None).unwrap().job("", 300.).unwrap().text, "");
}

/// Headless glyph test uses actual shared font installation/typography. This is
/// an independent app layout consistency check, not original WPF pixel parity.
#[test]
fn same_shared_format_has_same_glyph_metrics_in_typography_path() {
    let ctx = egui::Context::default();
    let mut fonts = egui::FontDefinitions::default();
    crate::typography::install(&mut fonts);
    crate::native_presentation::install_fonts(&mut fonts);
    ctx.set_fonts(fonts);
    let d = document();
    let cache = RichTextLayout::from_validated(&d, "한😀AB\n").unwrap();
    let projected = cache.project("한😀AB\n", None).unwrap();
    let mut metrics = None;
    let output = ctx.run_ui(egui::RawInput::default(), |ui| {
        let actual = projected.layout(ui, "한😀AB\n", 300.).unwrap();
        let mut expected = LayoutJob::default();
        expected.wrap.max_width = 300.;
        let Block::Paragraph(p) = &d.blocks[0] else {
            unreachable!()
        };
        for run in &p.runs {
            expected.append(
                &run.text,
                0.,
                crate::native_presentation::text_format(&run.style, None),
            );
        }
        expected.append(
            "\n",
            0.,
            crate::native_presentation::text_format(&d.style, None),
        );
        let expected = crate::typography::layout_job(ui.painter(), expected);
        metrics = Some((
            actual.text().to_owned(),
            actual.size(),
            expected.size(),
            actual.rows.len(),
            expected.rows.len(),
        ));
    });
    output.drop_without_applying_deltas();
    let (text, actual, expected, actual_rows, expected_rows) = metrics.unwrap();
    assert_eq!(text, "한😀AB\n");
    assert_eq!(actual, expected);
    assert_eq!(actual_rows, expected_rows);
}

#[test]
fn single_all_empty_paragraph_uses_paragraph_visual_style_without_run_owner_selection() {
    let paragraph_style = TextStyle {
        font_size: 19.,
        foreground: [12, 34, 56, 255],
        ..Default::default()
    };
    let visual = RichDocument {
        style: TextStyle::default(),
        imported_plain_text: String::new(),
        blocks: vec![Block::Paragraph(Paragraph {
            style: paragraph_style.clone(),
            margin: [0.; 4],
            line_height: None,
            runs: vec![
                Run {
                    text: String::new(),
                    style: TextStyle {
                        font_size: 11.,
                        ..Default::default()
                    },
                },
                Run {
                    text: String::new(),
                    style: TextStyle {
                        font_size: 27.,
                        bold: true,
                        ..Default::default()
                    },
                },
            ],
        })],
    };
    let cache = RichTextLayout::from_validated(&visual, "").unwrap();
    assert_eq!(cache.run_styles.len(), 2);
    let projection = cache.project("", None).unwrap();
    let job = projection.job("", 340.).unwrap();
    assert_eq!(job.text, "");
    assert_eq!(job.sections.len(), 1);
    assert_eq!(job.sections[0].format.font_id.size, 19.);
    assert_eq!(
        job.sections[0].format.color,
        egui::Color32::from_rgba_unmultiplied(12, 34, 56, 255)
    );
    assert_ne!(job.sections[0].format.font_id.size, 11.);
    assert_ne!(job.sections[0].format.font_id.size, 27.);
    // The layouter owns no RunAffinity and has not modified the source projection.
    assert!(visual.blocks.iter().all(|b| matches!(b,Block::Paragraph(p) if p.runs.len()==2 && p.runs.iter().all(|r|r.text.is_empty()))));
}
