//! Readonly vector-print snapshot for an authoritative Opaque Area. No input,
//! cached MathML evaluation, collapse control or worksheet mutation (0132/0134).
use super::area_ui::{AreaChildView, child_key};
use crate::{formula, native_presentation, result_display::FormattedResult, worksheet_math};
use eframe::egui::{self, Color32, Pos2, Rect, Stroke, UiBuilder, Vec2, vec2};
use mathfolio_document::{area, native_visual::RegionVisual};
use mathfolio_editor::{Editor, PortableTree};
use std::collections::HashMap;

pub(super) struct AreaPrintSnapshot {
    width: f32,
    height: f32,
    collapsed: bool,
    border: bool,
    error: bool,
    children: Vec<Child>,
}
struct Child {
    key: String,
    local: Vec2,
    visual: Option<RegionVisual>,
    content: Content,
}
enum Content {
    Math {
        editor: Box<Editor>,
        display: Option<FormattedResult>,
        labelled: Option<PortableTree>,
        font_points: f32,
        dim: bool,
    },
    Native {
        spec_table: bool,
    },
}
impl AreaPrintSnapshot {
    /// Copy committed presentation, never draft undo histories or an evaluator.
    /// Unsupported expanded children fail the job instead of silently vanishing.
    pub(super) fn new(
        xml: &str,
        views: &HashMap<String, AreaChildView<'_>>,
        width: f32,
        default_font_points: f32,
        has_calculation_error: bool,
    ) -> Result<Self, String> {
        if !width.is_finite()
            || width <= 0.
            || !default_font_points.is_finite()
            || default_font_points <= 0.
        {
            return Err("Invalid Area print dimensions or font".into());
        }
        let metadata = area::areas_from_xml(xml)?
            .into_iter()
            .find(|a| a.region_path.is_empty())
            .ok_or("Area printing requires its owning region fragment")?;
        let height = mathfolio_document::area_authoring::area_display_height_xml(xml)? as f32;
        if !height.is_finite() {
            return Err("Area print height exceeds supported coordinates".into());
        }
        let mut out = Self {
            width,
            height,
            collapsed: metadata.is_collapsed,
            border: metadata.is_border_visible,
            error: has_calculation_error,
            children: vec![],
        };
        if !out.height.is_finite() {
            return Err("Area print height exceeds supported coordinates".into());
        }
        if out.collapsed {
            return Ok(out);
        }
        if metadata.protected {
            return Err("Expanded protected Area printing is unsupported".into());
        }
        let mut math: HashMap<_, _> = area::area_math_children_xml(xml, &[])?
            .into_iter()
            .map(|child| (child_key(&child.region_path), child))
            .collect();
        for path in metadata.child_region_paths {
            let key = child_key(&path);
            let view = views.get(&key).copied().unwrap_or_default();
            if let Some(source) = math.remove(&key) {
                let tree = worksheet_math::tree_from_mathml(&source.mathml)?;
                let editor = Editor::from_portable(&tree).map_err(|e| e.to_string())?;
                if !(source.left as f32).is_finite() || !(source.top as f32).is_finite() {
                    return Err("Area math placement exceeds supported coordinates".into());
                }
                let font_points = view.font_points.unwrap_or(default_font_points);
                if !font_points.is_finite() || font_points <= 0. {
                    return Err("Invalid Area child print font".into());
                }
                out.children.push(Child {
                    key,
                    local: vec2(source.left as f32, source.top as f32),
                    visual: view.native.cloned(),
                    content: Content::Math {
                        editor: Box::new(editor),
                        display: view.display.cloned(),
                        labelled: view.labelled.cloned(),
                        font_points,
                        dim: view.pending || source.calculation_disabled,
                    },
                });
            } else {
                let local = view
                    .local
                    .ok_or_else(|| format!("Area child {key} has no source placement"))?;
                if local
                    .iter()
                    .any(|n| !n.is_finite() || !(*n as f32).is_finite())
                {
                    return Err("Invalid Area child print placement".into());
                }
                let visual = view
                    .native
                    .filter(|v| v.document.is_some())
                    .ok_or_else(|| {
                        format!(
                            "Area child {key} cannot be printed: {}",
                            view.error.unwrap_or("unsupported native presentation")
                        )
                    })?;
                out.children.push(Child {
                    key,
                    local: vec2(local[0] as f32, local[1] as f32),
                    visual: Some(visual.clone()),
                    content: Content::Native {
                        spec_table: view.kind == Some("spec-table"),
                    },
                });
            }
        }
        Ok(out)
    }
    fn child_geometry(&self, ui: &egui::Ui, child: &Child) -> (Vec2, Vec2, f32) {
        let width = child
            .visual
            .as_ref()
            .map(|v| v.size[0])
            .filter(|w| *w > 0.)
            .unwrap_or((self.width - child.local.x).max(80.));
        match &child.content {
            Content::Math {
                editor,
                display,
                labelled,
                font_points,
                ..
            } => {
                let (size, baseline) = formula::measured_geometry_readonly_styled(
                    ui,
                    editor,
                    display.as_ref().map(formula::FormulaResult::Formatted),
                    labelled.as_ref(),
                    *font_points,
                );
                (child.local - vec2(0., baseline), size, width)
            }
            Content::Native { spec_table } => {
                let visual = child.visual.as_ref().expect("validated native Area child");
                let offset = native_presentation::alignment_offset(ui, visual, width, *spec_table);
                (
                    child.local + vec2(0., offset),
                    native_presentation::measure(ui, visual, width),
                    width,
                )
            }
        }
    }
    /// Normal collapsed content contributes no printed ink or icon. The
    /// documented collapsed-error line is the exception even if borders hide.
    pub(super) fn measure(&self, ui: &egui::Ui) -> Vec2 {
        if self.collapsed {
            return vec2(self.width, if self.error { 1. } else { 0. });
        }
        let mut size = vec2(
            self.width,
            self.height.max(if self.border { 1. } else { 0. }),
        );
        for child in &self.children {
            let (offset, child_size, _) = self.child_geometry(ui, child);
            size = size.max(offset + child_size);
        }
        size
    }
    /// Paint at an already page-transformed owner origin, clipped only by the
    /// caller's page viewport. All output remains ordinary vector/text shapes.
    pub(super) fn paint(&self, ui: &mut egui::Ui, origin: Pos2, clip: Rect) -> Rect {
        let size = self.measure(ui);
        let rect = Rect::from_min_size(origin, size);
        let painter = ui.painter().with_clip_rect(clip);
        if self.collapsed {
            if self.error {
                painter.hline(
                    origin.x..=origin.x + self.width,
                    origin.y,
                    Stroke::new(0.8, Color32::from_rgb(175, 40, 45)),
                );
            }
            return rect;
        }
        if self.border {
            for y in [origin.y, origin.y + size.y] {
                painter.hline(
                    origin.x..=origin.x + self.width,
                    y,
                    Stroke::new(0.8, Color32::GRAY),
                );
            }
        }
        for child in &self.children {
            let (offset, child_size, width) = self.child_geometry(ui, child);
            let child_origin = origin + offset;
            let mut inner = ui.new_child(
                UiBuilder::new()
                    .id_salt(("area-print-child", &child.key))
                    .max_rect(Rect::from_min_size(
                        child_origin,
                        vec2(width.max(child_size.x), child_size.y.max(1.)),
                    ))
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            inner.set_clip_rect(clip);
            match &child.content {
                Content::Math {
                    editor,
                    display,
                    labelled,
                    font_points,
                    dim,
                } => {
                    if *dim {
                        inner.set_opacity(0.45);
                    }
                    if let Some([r, g, b, a]) = child.visual.as_ref().and_then(|v| v.background) {
                        inner.painter().rect_filled(
                            Rect::from_min_size(child_origin, child_size),
                            0.,
                            Color32::from_rgba_unmultiplied(r, g, b, a),
                        );
                    }
                    formula::formula_readonly_ui_styled(
                        &mut inner,
                        editor,
                        display.as_ref().map(formula::FormulaResult::Formatted),
                        labelled.as_ref(),
                        *font_points,
                    );
                }
                Content::Native { .. } => {
                    native_presentation::draw(&mut inner, child.visual.as_ref().unwrap(), width);
                }
            }
        }
        rect
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::printing::{self, PageSelection, PrintPlan, PrintSettings};
    use mathfolio_document::native_visual::{Block, Paragraph, RichDocument, Run, TextStyle};
    fn xml(collapsed: bool, border: bool, extra: &str) -> String {
        format!(
            r#"<region xmlns="http://schemas.mathsoft.com/worksheet50" xmlns:ml="http://schemas.mathsoft.com/math50"><Area AreaHeight="150" IsCollapsed="{collapsed}" IsBorderVisible="{border}">{extra}<regions><region region-id="7" left="19.2" top="38.4"><math resultRef="3"><ml:eval><ml:apply><ml:plus/><ml:real>12</ml:real><ml:real>34</ml:real></ml:apply><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval></math></region><region region-id="8" left="57.6" top="76.8"><text/></region></regions></Area></region>"#
        )
    }
    fn rich() -> RegionVisual {
        let style = TextStyle::default();
        RegionVisual {
            size: [190., 30.],
            background: None,
            document: Some(RichDocument {
                blocks: vec![Block::Paragraph(Paragraph {
                    runs: vec![Run {
                        text: "Area vector text Ω".into(),
                        style: style.clone(),
                    }],
                    style: style.clone(),
                    margin: [0.; 4],
                    line_height: None,
                })],
                style,
                imported_plain_text: "Area vector text Ω".into(),
            }),
        }
    }
    fn render(snapshot: &AreaPrintSnapshot) -> Vec<printing::RasterPage> {
        let plan = PrintPlan::new(&PrintSettings {
            paper_size: [400., 300.],
            margins: [10.; 4],
            body_height: 280.,
            content_bottom: 200.,
            pages: PageSelection::All,
            dpi: 96,
        })
        .unwrap();
        printing::render_vector_pages(&plan, |ui, _, _| {
            snapshot.paint(ui, Pos2::new(10., 10.), ui.max_rect());
        })
        .unwrap()
    }
    fn snapshot(collapsed: bool, border: bool, error: bool) -> AreaPrintSnapshot {
        let native = rich();
        let result = crate::result_display::from_native_result(
            r#"<ml:real xmlns:ml="http://schemas.mathsoft.com/math50">46</ml:real>"#,
            &Default::default(),
        )
        .unwrap();
        let views = HashMap::from([
            (
                "[0,0,0]".into(),
                AreaChildView {
                    display: Some(&result),
                    ..Default::default()
                },
            ),
            (
                "[0,0,1]".into(),
                AreaChildView {
                    native: Some(&native),
                    local: Some([57.6, 76.8]),
                    kind: Some("text"),
                    ..Default::default()
                },
            ),
        ]);
        AreaPrintSnapshot::new(&xml(collapsed, border, ""), &views, 300., 11., error).unwrap()
    }
    #[test]
    fn expanded_local_math_and_rich_text_produce_vector_pdf_without_images() {
        let snapshot = snapshot(false, true, false);
        let pages = render(&snapshot);
        let path = std::env::temp_dir().join(format!(
            "mathfolio-area-print-{}-{}.pdf",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        printing::write_pdf(&path, &pages).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        std::fs::remove_file(path).unwrap();
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.starts_with("%PDF-1.7"));
        assert!(text.contains("/ToUnicode"));
        assert!(!text.contains("/Subtype /Image"));
        let context = egui::Context::default();
        crate::configure(&context);
        let mut output = context.run_ui(Default::default(), |ui| {
            let math = &snapshot.children[0];
            let (offset, _, _) = snapshot.child_geometry(ui, math);
            assert_eq!(offset.x, 19.2);
            assert!(offset.y < 38.4);
            let native = &snapshot.children[1];
            let (offset, _, _) = snapshot.child_geometry(ui, native);
            assert_eq!(offset.x, 57.6);
            assert!(offset.y <= 76.8);
            assert!(snapshot.measure(ui).y >= 150.);
        });
        output.textures_delta.clear();
    }
    #[test]
    fn collapsed_print_excludes_children_and_icons_but_keeps_hidden_border_error_line() {
        for (error, expected) in [(false, 0.), (true, 1.)] {
            let snapshot = snapshot(true, false, error);
            assert!(snapshot.children.is_empty());
            let context = egui::Context::default();
            crate::configure(&context);
            let mut output = context.run_ui(Default::default(), |ui| {
                assert_eq!(snapshot.measure(ui).y, expected);
                snapshot.paint(ui, Pos2::new(10., 10.), ui.max_rect());
            });
            let lines = output
                .shapes
                .iter()
                .filter_map(|s| {
                    if let egui::Shape::LineSegment { stroke, .. } = s.shape {
                        Some(stroke)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            assert_eq!(lines.len(), usize::from(error));
            if error {
                assert_eq!(lines[0].color, Color32::from_rgb(175, 40, 45));
            }
            assert!(
                output
                    .shapes
                    .iter()
                    .all(|s| !matches!(s.shape, egui::Shape::Text(_)))
            );
            output.textures_delta.clear();
        }
    }
    #[test]
    fn unsupported_expanded_children_fail_instead_of_omitting_original_content() {
        assert!(
            AreaPrintSnapshot::new(&xml(false, false, ""), &HashMap::new(), 300., 11., false)
                .is_err()
        );
        assert!(
            AreaPrintSnapshot::new(
                &xml(false, false, "<ProtectionData/>"),
                &HashMap::new(),
                300.,
                11.,
                false
            )
            .is_err()
        );
        assert!(
            AreaPrintSnapshot::new(
                &xml(true, false, "<ProtectionData/>"),
                &HashMap::new(),
                300.,
                11.,
                false
            )
            .is_ok()
        );
    }
    #[test]
    #[ignore = "Writes a selected headless Area vector PDF fixture, never runs a worksheet"]
    fn area_vector_pdf_export_probe() {
        let output = std::path::PathBuf::from(
            std::env::var("MATHFOLIO_AREA_PRINT_OUTPUT").expect("Set an output directory"),
        );
        std::fs::create_dir_all(&output).unwrap();
        for (name, collapsed, border, error) in [
            ("area-expanded", false, true, false),
            ("area-collapsed", true, false, false),
            ("area-collapsed-error", true, false, true),
        ] {
            printing::write_pdf(
                &output.join(format!("{name}.pdf")),
                &render(&snapshot(collapsed, border, error)),
            )
            .unwrap();
        }
        println!("AREA_VECTOR_PDF expanded=1 collapsed=1 collapsed_error=1 raster_images=0");
    }
}
