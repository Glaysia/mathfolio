//! Proposed headless app geometry regressions. Not executed in this artifact.
//! Uses actual egui TextEdit/custom layouter and pointer events, not a mock hit map.
use super::*;
use mathcraft_document::native_visual::{Paragraph, Run};

fn visual(alignment: Alignment, text: &str) -> RichDocument {
    let style = TextStyle {
        alignment,
        font_size: 20.,
        foreground: [18, 52, 86, 255],
        ..Default::default()
    };
    RichDocument {
        style: TextStyle::default(), // explicit Paragraph owns alignment, not root
        blocks: vec![Block::Paragraph(Paragraph {
            runs: vec![Run {
                text: text.into(),
                style: style.clone(),
            }],
            style,
            margin: [0.; 4],
            line_height: None,
        })],
        imported_plain_text: text.into(),
    }
}
struct Capture {
    galley: Arc<egui::Galley>,
    origin: egui::Pos2,
    clip: egui::Rect,
    cursor: Option<egui::text::CCursorRange>,
    error: Option<String>,
    primitives: usize,
}
struct Fixture {
    ctx: egui::Context,
    cache: RichTextLayout,
    text: String,
    frame: u32,
    width: f32,
    id: egui::Id,
}
impl Fixture {
    fn new(alignment: Alignment, text: &str, width: f32) -> Self {
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        Self {
            ctx,
            cache: RichTextLayout::from_validated(&visual(alignment, text), text).unwrap(),
            text: text.into(),
            frame: 0,
            width,
            id: egui::Id::new("alignment-actual-textedit"),
        }
    }
    fn frame(&mut self, events: Vec<egui::Event>) -> Capture {
        self.frame += 1;
        let raw = egui::RawInput {
            events,
            time: Some(f64::from(self.frame)),
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(900., 700.),
            )),
            ..Default::default()
        };
        let project = self.cache.project(&self.text, None).unwrap();
        let mut captured = None;
        let mut error = None;
        let full =
            self.ctx.run_ui(raw, |ui| {
                ui.set_width(self.width);
                let mut layouter = |ui: &egui::Ui, buffer: &dyn egui::TextBuffer, width: f32| {
                    match project.layout(ui, buffer.as_str(), width) {
                        Ok(galley) => galley,
                        Err(e) => {
                            error = Some(e);
                            ui.painter().layout_no_wrap(
                                buffer.as_str().into(),
                                egui::FontId::proportional(20.),
                                egui::Color32::BLACK,
                            )
                        }
                    }
                };
                let output = egui::TextEdit::multiline(&mut self.text)
                    .id(self.id)
                    .frame(egui::Frame::NONE)
                    .desired_rows(1)
                    .desired_width(self.width)
                    .horizontal_align(self.cache.horizontal_align())
                    .layouter(&mut layouter)
                    .show(ui);
                captured = Some((
                    output.galley.clone(),
                    output.galley_pos,
                    output.text_clip_rect,
                    output.cursor_range,
                ));
            });
        let shapes = full.shapes.clone();
        let ppp = full.pixels_per_point;
        full.drop_without_applying_deltas(); // all checks after FullOutput disposal
        let (galley, galley_pos, clip, cursor) = captured.expect("TextEdit was drawn");
        let origin = galley_pos - egui::vec2(galley.rect.left(), 0.);
        let primitives = self.ctx.tessellate(shapes, ppp).len();
        Capture {
            galley,
            origin,
            clip,
            cursor,
            error,
            primitives,
        }
    }
    fn click(&mut self, pos: egui::Pos2) -> Capture {
        self.frame(vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        self.frame(vec![egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }])
    }
}

#[test]
fn real_textedit_alignment_caret_pointer_roundtrip_uses_same_paint_origin() {
    for alignment in [
        Alignment::Left,
        Alignment::Center,
        Alignment::Right,
        Alignment::Justify,
    ] {
        for width in [220., 380.] {
            let mut f = Fixture::new(alignment, "A한😀B", width);
            f.frame(vec![]);
            let painted = f.frame(vec![]);
            assert!(painted.error.is_none());
            assert!(painted.primitives > 0);
            let row = painted.galley.rows[0]
                .rect()
                .translate(painted.origin.to_vec2());
            match alignment {
                Alignment::Left | Alignment::Justify => {
                    assert!((row.left() - painted.clip.left()).abs() <= 1.)
                }
                Alignment::Center => {
                    assert!((row.center().x - painted.clip.center().x).abs() <= 1.)
                }
                Alignment::Right => assert!((row.right() - painted.clip.right()).abs() <= 1.),
            }
            for index in [0, 1, 2, 3, 4] {
                let local = painted
                    .galley
                    .pos_from_cursor(egui::text::CCursor::new(index));
                let pos = local.center() + painted.origin.to_vec2();
                let actual = f.click(pos);
                assert!(actual.error.is_none());
                let cursor = actual.cursor.unwrap();
                assert!(cursor.is_empty());
                assert_eq!(cursor.primary.index.0, index);
                assert_eq!(f.text, "A한😀B");
            }
        }
    }
}

#[test]
fn justified_wrapped_text_retains_all_scalars_and_pointer_ownership_on_each_row() {
    let text = "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu";
    let mut f = Fixture::new(Alignment::Justify, text, 185.);
    f.frame(vec![]);
    let painted = f.frame(vec![]);
    assert!(painted.error.is_none());
    assert!(painted.galley.rows.len() > 1);
    let job = f
        .cache
        .project(text, None)
        .unwrap()
        .job(text, 185.)
        .unwrap();
    assert!(job.justify);
    assert_eq!(job.text, text);
    let mut start = 0;
    for row in &painted.galley.rows {
        if row.glyphs.is_empty() {
            continue;
        }
        let index = start + 1;
        let local = painted
            .galley
            .pos_from_cursor(egui::text::CCursor::new(index));
        let actual = f.click(local.center() + painted.origin.to_vec2());
        assert_eq!(actual.cursor.unwrap().primary.index.0, index);
        assert_eq!(f.text, text);
        start += row.glyphs.len();
    }
}

#[test]
fn empty_alignment_and_source_style_are_kept_without_synthetic_glyphs() {
    for alignment in [Alignment::Center, Alignment::Right, Alignment::Justify] {
        let mut f = Fixture::new(alignment, "", 280.);
        f.frame(vec![]);
        let frame = f.frame(vec![]);
        assert!(frame.error.is_none());
        assert!(frame.galley.text().is_empty());
        let job = f.cache.project("", None).unwrap().job("", 280.).unwrap();
        assert_eq!(
            job.sections[0].format.color,
            egui::Color32::from_rgb(18, 52, 86)
        );
        assert_eq!(job.sections[0].format.font_id.size, 20.);
        assert_eq!(job.text, "");
    }
    let mut mixed = visual(Alignment::Right, "first");
    mixed
        .blocks
        .push(visual(Alignment::Left, "second").blocks.remove(0));
    assert!(RichTextLayout::from_validated(&mixed, "first\nsecond").is_err());
}
