//! Independent readonly FlowDocument/table layout shared by canvas and print.
use eframe::egui::{
    self, Color32, FontFamily, FontId, Galley, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2, pos2,
    vec2,
};
use mathcraft_document::native_visual::{
    Alignment, Block, Paragraph, RegionVisual, Rgba, Table, TextStyle,
};
use mathcraft_editor::Editor;
use std::sync::Arc;

fn color(c: Rgba) -> Color32 {
    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3])
}

/// Use installed OS fonts or the independently configured fallback faces. No
/// proprietary Mathcad fonts or other shipped product resources are copied.
pub(crate) fn install_fonts(fonts: &mut egui::FontDefinitions) {
    let fallback = fonts
        .families
        .get(&FontFamily::Name("worksheet-text".into()))
        .cloned()
        .unwrap_or_else(|| fonts.families[&FontFamily::Proportional].clone());
    for (name, regular, bold_paths) in [
        (
            "native",
            None,
            &[
                "C:/Windows/Fonts/tahomabd.ttf",
                "/usr/share/fonts/truetype/liberation2/LiberationSans-Bold.ttf",
            ][..],
        ),
        (
            "tahoma",
            Some("worksheet-text"),
            &[
                "C:/Windows/Fonts/tahomabd.ttf",
                "/usr/share/fonts/truetype/liberation2/LiberationSans-Bold.ttf",
            ][..],
        ),
        (
            "malgun",
            Some("cjk"),
            &[
                "C:/Windows/Fonts/malgunbd.ttf",
                "/usr/share/fonts/opentype/noto/NotoSansCJK-Bold.ttc",
                "/usr/share/fonts/truetype/nanum/NanumGothicBold.ttf",
            ][..],
        ),
    ] {
        let mut faces = fallback.clone();
        if let Some(face) = regular.filter(|n| fonts.font_data.contains_key(*n)) {
            faces.insert(0, face.into());
        }
        fonts.families.insert(
            FontFamily::Name(format!("{name}-text").into()),
            faces.clone(),
        );
        for path in bold_paths {
            if let Ok(bytes) = std::fs::read(path) {
                let key = format!("{name}-text-bold");
                fonts
                    .font_data
                    .insert(key.clone(), egui::FontData::from_owned(bytes).into());
                faces.insert(0, key);
                break;
            }
        }
        fonts
            .families
            .insert(FontFamily::Name(format!("{name}-text-bold").into()), faces);
    }
}

pub(crate) fn text_format(style: &TextStyle, line_height: Option<f32>) -> egui::TextFormat {
    let name = style.font_family.to_ascii_lowercase().replace(' ', "");
    let family = if name.contains("malgungothic") {
        "malgun"
    } else if name == "tahoma" {
        "tahoma"
    } else {
        "native"
    };
    egui::TextFormat {
        font_id: if style.font_family == "math-regular" {
            FontId::new(style.font_size, FontFamily::Name("math-regular".into()))
        } else {
            FontId::new(
                style.font_size,
                FontFamily::Name(
                    format!("{family}-text{}", if style.bold { "-bold" } else { "" }).into(),
                ),
            )
        },
        color: color(style.foreground),
        background: style.background.map(color).unwrap_or(Color32::TRANSPARENT),
        italics: style.italic,
        underline: if style.underline {
            Stroke::new(0.8, color(style.foreground))
        } else {
            Stroke::NONE
        },
        expand_bg: 0.,
        line_height,
        ..Default::default()
    }
}

#[derive(Clone)]
enum Paint {
    Fill(Rect, Color32),
    Text(Vec2, Arc<Galley>),
    Line(Vec2, Vec2, Stroke),
    Math(Vec2, Arc<Editor>, f32),
}
#[derive(Default)]
struct NativeLayout {
    size: Vec2,
    paint: Vec<Paint>,
    first_baseline: Option<f32>,
    table_header_gap_midpoint: Option<f32>,
    cells: Vec<TableCellGeometry>,
}
#[derive(Clone, Debug)]
pub(crate) struct TableCellGeometry {
    pub row: usize,
    pub column: usize,
    pub rect: Rect,
    pub content_origin: Vec2,
    paint: std::ops::Range<usize>,
}
#[derive(Clone)]
struct LayoutCache {
    visual: RegionVisual,
    width: f32,
    fonts: crate::typography::FontCacheKey,
    layout: Arc<NativeLayout>,
}

fn translated(paint: &Paint, offset: Vec2) -> Paint {
    match paint {
        Paint::Fill(rect, c) => Paint::Fill(rect.translate(offset), *c),
        Paint::Text(p, g) => Paint::Text(*p + offset, g.clone()),
        Paint::Line(a, b, s) => Paint::Line(*a + offset, *b + offset, *s),
        Paint::Math(p, e, f) => Paint::Math(*p + offset, e.clone(), *f),
    }
}
fn append(target: &mut NativeLayout, child: &NativeLayout, offset: Vec2) {
    let paint_start = target.paint.len();
    target
        .cells
        .extend(child.cells.iter().cloned().map(|mut cell| {
            cell.rect = cell.rect.translate(offset);
            cell.content_origin += offset;
            cell.paint = (cell.paint.start + paint_start)..(cell.paint.end + paint_start);
            cell
        }));
    if target.first_baseline.is_none() {
        target.first_baseline = child.first_baseline.map(|y| y + offset.y);
    }
    if target.table_header_gap_midpoint.is_none() {
        target.table_header_gap_midpoint = child.table_header_gap_midpoint.map(|y| y + offset.y);
    }
    target
        .paint
        .extend(child.paint.iter().map(|p| translated(p, offset)));
    target.size = target.size.max(child.size + offset);
}
fn width(visual: &RegionVisual, available: f32) -> f32 {
    let available = if available.is_finite() {
        available.clamp(1., 1_000_000.)
    } else {
        340.
    };
    if visual.size[0] > 0. {
        available.min(visual.size[0])
    } else {
        available
    }
}
fn retained_layout(ui: &Ui, visual: &RegionVisual, available: f32) -> Arc<NativeLayout> {
    #[cfg(test)]
    let lookup_timing = crate::frame_profile::scope(crate::frame_profile::Phase::NativeLookup);
    let width = width(visual, available);
    let id = ui.id().with("native-rich-presentation");
    let fonts = crate::typography::font_cache_key(ui.ctx());
    if let Some(cache) = ui.data(|d| d.get_temp::<Arc<LayoutCache>>(id))
        && cache.visual == *visual
        && cache.width == width
        && cache.fonts == fonts
    {
        return cache.layout.clone();
    }
    #[cfg(test)]
    drop(lookup_timing);
    #[cfg(test)]
    let build_timing = crate::frame_profile::scope(crate::frame_profile::Phase::NativeBuild);
    // The supplied Prime PDF places rich text five DIP inside its region;
    // specification-table cells already provide their own padding.
    let text_padding = visual.document.as_ref().map(|d| {
        let spec = d.blocks.iter().any(|block| matches!(block, Block::Table(t) if t.cells.iter().any(|c| c.blocks.iter().any(|b| matches!(b, Block::Math(_))))));
        if spec { 0. } else { 5. }
    }).unwrap_or(0.);
    let mut layout = visual
        .document
        .as_ref()
        .map(|d| layout_blocks(ui, &d.blocks, (width - 2. * text_padding).max(1.)))
        .unwrap_or_default();
    if text_padding > 0. {
        layout.paint = layout
            .paint
            .iter()
            .map(|paint| translated(paint, vec2(text_padding, 0.)))
            .collect();
        layout.size.x += 2. * text_padding;
    }
    layout.size = layout
        .size
        .max(vec2(visual.size[0].min(width), visual.size[1]));
    if let Some(background) = visual.background {
        layout.paint.insert(
            0,
            Paint::Fill(
                Rect::from_min_size(Pos2::ZERO, layout.size),
                color(background),
            ),
        );
    }
    #[cfg(test)]
    drop(build_timing);
    #[cfg(test)]
    let _payload_timing = crate::frame_profile::scope(crate::frame_profile::Phase::NativePayload);
    let layout = Arc::new(layout);
    ui.data_mut(|d| {
        d.insert_temp(
            id,
            Arc::new(LayoutCache {
                visual: visual.clone(),
                width,
                fonts,
                layout: layout.clone(),
            }),
        )
    });
    layout
}

pub(crate) fn measure(ui: &Ui, visual: &RegionVisual, available_width: f32) -> Vec2 {
    retained_layout(ui, visual, available_width).size
}
/// Native region locations are alignment anchors, rather than painted top-lefts.
/// Prime aligns rich text to its first baseline and specification tables to the
/// midpoint between the second header row and first data row.
pub(crate) fn alignment_offset(
    ui: &Ui,
    visual: &RegionVisual,
    available_width: f32,
    spec_table: bool,
) -> f32 {
    let layout = retained_layout(ui, visual, available_width);
    let anchor = if spec_table {
        layout.table_header_gap_midpoint
    } else {
        layout.first_baseline
    };
    -anchor.unwrap_or(0.)
}
pub(crate) fn draw(ui: &mut Ui, visual: &RegionVisual, available_width: f32) -> Response {
    draw_table(ui, visual, available_width, None).0
}
pub(crate) fn draw_table(
    ui: &mut Ui,
    visual: &RegionVisual,
    available_width: f32,
    editing: Option<(usize, usize)>,
) -> (Response, Vec<TableCellGeometry>) {
    let layout = retained_layout(ui, visual, available_width);
    let (rect, response) = ui.allocate_exact_size(layout.size, Sense::hover());
    let hidden = editing.and_then(|(row, column)| {
        layout
            .cells
            .iter()
            .find(|cell| cell.row == row && cell.column == column)
    });
    for (index, paint) in layout.paint.iter().enumerate() {
        if hidden.is_some_and(|cell| cell.paint.contains(&index)) {
            continue;
        }
        match paint {
            Paint::Fill(r, c) => {
                ui.painter()
                    .rect_filled(r.translate(rect.min.to_vec2()), 0., *c);
            }
            Paint::Text(p, g) => ui
                .painter()
                .galley(rect.min + *p, g.clone(), Color32::BLACK),
            Paint::Line(a, b, s) => {
                ui.painter()
                    .line_segment([rect.min + *a, rect.min + *b], *s);
            }
            Paint::Math(offset, editor, points) => {
                let origin = rect.min + *offset;
                let mut child = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt(("native-math", index))
                        .max_rect(Rect::from_min_size(origin, vec2(100_000., 100_000.)))
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                );
                crate::formula::formula_readonly_ui_styled(&mut child, editor, None, None, *points);
            }
        }
    }
    let cells = layout
        .cells
        .iter()
        .cloned()
        .map(|mut cell| {
            cell.rect = cell.rect.translate(rect.min.to_vec2());
            cell.content_origin += rect.min.to_vec2();
            cell
        })
        .collect();
    (response, cells)
}

fn layout_blocks(ui: &Ui, blocks: &[Block], width: f32) -> NativeLayout {
    let mut layout = NativeLayout::default();
    let mut y = 0.;
    for block in blocks {
        let child = match block {
            Block::Paragraph(p) => layout_paragraph(ui, p, width),
            Block::Table(t) => layout_table(ui, t, width),
            Block::Math(math) => {
                #[cfg(test)]
                let _math_timing =
                    crate::frame_profile::scope(crate::frame_profile::Phase::NativeMath);
                // The parser validated every tree through the same editor bridge.
                let editor =
                    Arc::new(Editor::from_portable(&math.tree).expect("validated native math"));
                let (size, baseline) = crate::formula::measured_geometry_readonly_styled(
                    ui,
                    &editor,
                    None,
                    None,
                    math.font_points,
                );
                NativeLayout {
                    size,
                    first_baseline: Some(baseline),
                    paint: vec![Paint::Math(Vec2::ZERO, editor, math.font_points)],
                    ..Default::default()
                }
            }
        };
        append(&mut layout, &child, vec2(0., y));
        y += child.size.y;
    }
    layout
}
fn layout_paragraph(ui: &Ui, paragraph: &Paragraph, width: f32) -> NativeLayout {
    #[cfg(test)]
    let _timing = crate::frame_profile::scope(crate::frame_profile::Phase::NativeParagraph);
    let [left, top, right, bottom] = paragraph.margin;
    let available = (width - left - right).max(1.);
    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = available;
    job.halign = match paragraph.style.alignment {
        Alignment::Left | Alignment::Justify => egui::Align::LEFT,
        Alignment::Center => egui::Align::Center,
        Alignment::Right => egui::Align::RIGHT,
    };
    job.justify = paragraph.style.alignment == Alignment::Justify;
    for run in &paragraph.runs {
        job.append(
            &run.text,
            0.,
            text_format(&run.style, paragraph.line_height),
        );
    }
    if job.text.is_empty() {
        job.append(
            " ",
            0.,
            text_format(&paragraph.style, paragraph.line_height),
        );
    }
    let galley = crate::typography::layout_job(ui.painter(), job);
    let first_baseline = galley.rows.first().and_then(|row| {
        row.glyphs
            .first()
            .map(|glyph| top + row.pos.y + glyph.pos.y)
    });
    let mut layout = NativeLayout {
        size: vec2(
            (galley.size().x + left + right).max(width),
            galley.size().y + top + bottom,
        ),
        paint: vec![],
        first_baseline,
        cells: vec![],
        ..Default::default()
    };
    if let Some(background) = paragraph.style.background {
        layout.paint.push(Paint::Fill(
            Rect::from_min_size(pos2(left, top), vec2(available, galley.size().y)),
            color(background),
        ));
    }
    // Aligned galleys keep their row coordinates relative to the anchor.
    let anchor = match paragraph.style.alignment {
        Alignment::Center => available / 2.,
        Alignment::Right => available,
        _ => 0.,
    };
    layout
        .paint
        .push(Paint::Text(vec2(left + anchor, top), galley));
    layout
}
fn layout_table(ui: &Ui, table: &Table, width: f32) -> NativeLayout {
    #[cfg(test)]
    let _timing = crate::frame_profile::scope(crate::frame_profile::Phase::NativeTable);
    let spec_table = table
        .cells
        .iter()
        .any(|c| c.blocks.iter().any(|b| matches!(b, Block::Math(_))));
    let mut columns = table
        .columns
        .iter()
        .map(|c| c.width.unwrap_or(0.))
        .collect::<Vec<_>>();
    let spacing = table.spacing;
    for cell in &table.cells {
        if cell.column_span == 1 && table.columns[cell.column].width.is_none() {
            let needed = cell
                .blocks
                .iter()
                .map(|block| match block {
                    Block::Math(math) => {
                        let editor =
                            Editor::from_portable(&math.tree).expect("validated native math");
                        crate::formula::measured_size_readonly_styled(
                            ui,
                            &editor,
                            None,
                            None,
                            math.font_points,
                        )
                        .x
                    }
                    Block::Paragraph(p) => layout_paragraph(ui, p, 1_000_000.)
                        .paint
                        .iter()
                        .filter_map(|paint| {
                            if let Paint::Text(_, g) = paint {
                                Some(g.size().x)
                            } else {
                                None
                            }
                        })
                        .fold(0., f32::max),
                    Block::Table(t) => layout_table(ui, t, 340.).size.x,
                })
                .fold(0., f32::max)
                + cell.padding[0]
                + cell.padding[2]
                + cell.border[0]
                + cell.border[2];
            columns[cell.column] = columns[cell.column].max(needed.max(8.));
        }
    }
    let fixed = columns.iter().sum::<f32>() + spacing * (columns.len().saturating_sub(1) as f32);
    let weights = table
        .columns
        .iter()
        .filter(|c| c.width.is_none())
        .map(|c| c.weight)
        .sum::<f32>();
    let available = (width - fixed).max(0.);
    let autos = table
        .columns
        .iter()
        .filter(|c| c.width.is_none() && c.weight == 0.)
        .count();
    for (index, column) in table.columns.iter().enumerate() {
        if column.width.is_none() {
            if column.weight > 0. {
                columns[index] += available * column.weight / weights.max(1.);
            } else if weights == 0. && autos > 0 && !spec_table {
                columns[index] += available / autos as f32;
            }
        }
    }
    let x_at = |column: usize| columns[..column].iter().sum::<f32>() + spacing * column as f32;
    let mut rows = vec![table.row_min_height; table.rows];
    let mut row_baselines = vec![0f32; table.rows];
    let mut row_descents = vec![0f32; table.rows];
    let mut cells = Vec::new();
    for cell in &table.cells {
        let cell_width = columns[cell.column..cell.column + cell.column_span]
            .iter()
            .sum::<f32>()
            + spacing * cell.column_span.saturating_sub(1) as f32;
        let [left, top, right, bottom] = cell.padding;
        let [bl, bt, br, bb] = cell.border;
        let child = layout_blocks(
            ui,
            &cell.blocks,
            (cell_width - left - right - bl - br).max(1.),
        );
        let height = child.size.y + top + bottom + bt + bb;
        if spec_table && cell.row_span == 1 {
            let baseline = child.first_baseline.unwrap_or(0.) + top + bt;
            row_baselines[cell.row] = row_baselines[cell.row].max(baseline);
            row_descents[cell.row] = row_descents[cell.row].max(height - baseline);
            rows[cell.row] = rows[cell.row].max(row_baselines[cell.row] + row_descents[cell.row]);
        }
        let current = rows[cell.row..cell.row + cell.row_span].iter().sum::<f32>()
            + spacing * cell.row_span.saturating_sub(1) as f32;
        if current < height {
            let delta = (height - current) / cell.row_span as f32;
            for row in &mut rows[cell.row..cell.row + cell.row_span] {
                *row += delta;
            }
        }
        cells.push((cell, child, cell_width));
    }
    let mut layout = NativeLayout {
        size: vec2(
            columns.iter().sum::<f32>() + spacing * columns.len().saturating_sub(1) as f32,
            rows.iter().sum::<f32>() + spacing * rows.len().saturating_sub(1) as f32,
        ),
        paint: vec![],
        table_header_gap_midpoint: (rows.len() > 2).then(|| rows[0] + rows[1] + spacing * 1.5),
        ..Default::default()
    };
    if let Some(background) = table.style.background {
        layout.paint.push(Paint::Fill(
            Rect::from_min_size(Pos2::ZERO, layout.size),
            color(background),
        ));
    }
    let mut header_bottom = f32::NEG_INFINITY;
    let mut data_top = f32::INFINITY;
    for (cell, child, cell_width) in cells {
        let y = rows[..cell.row].iter().sum::<f32>() + spacing * cell.row as f32;
        let height = rows[cell.row..cell.row + cell.row_span].iter().sum::<f32>()
            + spacing * cell.row_span.saturating_sub(1) as f32;
        let origin = vec2(x_at(cell.column), y);
        let rect = Rect::from_min_size(Pos2::ZERO + origin, vec2(cell_width, height));
        if let Some(background) = cell.style.background {
            layout.paint.push(Paint::Fill(rect, color(background)));
        }
        for (a, b, thickness) in [
            (rect.left_top(), rect.left_bottom(), cell.border[0]),
            (rect.left_top(), rect.right_top(), cell.border[1]),
            (rect.right_top(), rect.right_bottom(), cell.border[2]),
            (rect.left_bottom(), rect.right_bottom(), cell.border[3]),
        ] {
            if thickness > 0. {
                layout.paint.push(Paint::Line(
                    a.to_vec2(),
                    b.to_vec2(),
                    Stroke::new(thickness, color(cell.border_color)),
                ));
            }
        }
        let free = (cell_width
            - cell.padding[0]
            - cell.padding[2]
            - cell.border[0]
            - cell.border[2]
            - child.size.x)
            .max(0.);
        let align = match cell.style.alignment {
            Alignment::Center => free / 2.,
            Alignment::Right => free,
            _ => 0.,
        };
        let baseline_delta = if spec_table && cell.row_span == 1 {
            row_baselines[cell.row]
                - (child.first_baseline.unwrap_or(0.) + cell.padding[1] + cell.border[1])
        } else {
            0.
        };
        let content_offset = origin
            + vec2(
                cell.padding[0] + cell.border[0] + align,
                cell.padding[1] + cell.border[1] + baseline_delta,
            );
        if spec_table {
            if cell.row == 1 {
                header_bottom = header_bottom.max(content_offset.y + child.size.y);
            }
            if cell.row == 2 {
                data_top = data_top.min(content_offset.y);
            }
        }
        let paint_start = layout.paint.len();
        append(&mut layout, &child, content_offset);
        if spec_table {
            layout.cells.push(TableCellGeometry {
                row: cell.row,
                column: cell.column,
                rect,
                content_origin: content_offset,
                paint: paint_start..layout.paint.len(),
            });
        }
    }
    if spec_table && header_bottom.is_finite() && data_top.is_finite() {
        layout.table_header_gap_midpoint = Some((header_bottom + data_top) / 2.);
    }
    layout
}

#[cfg(test)]
mod tests {
    use super::*;
    use mathcraft_document::native_visual::{TextStyle, read_flow_xml};
    const SAMPLE: &str = r##"<Section xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" FontSize="16" Foreground="#FFFFFFFF" Background="#FF662D91"><Paragraph><Bold>LCCL-S</Bold> 시스템 설계</Paragraph><Table CellSpacing="0"><Table.Columns><TableColumn Width="80"/><TableColumn Width="*"/></Table.Columns><TableRowGroup><TableRow><TableCell ColumnSpan="2" Padding="2" BorderThickness="0,0,0,1"><Paragraph>reference table</Paragraph></TableCell></TableRow><TableRow><TableCell><Paragraph>1</Paragraph></TableCell><TableCell><Paragraph>0.66</Paragraph></TableCell></TableRow></TableRowGroup></Table></Section>"##;
    #[test]
    fn native_rich_text_anchor_uses_first_glyph_baseline_with_paragraph_margin() {
        let context = egui::Context::default();
        crate::configure(&context);
        let document = read_flow_xml(
            r#"<Section xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"><Paragraph Margin="2,7,3,5"><Run FontSize="23">Heading</Run><LineBreak/><Run FontSize="11">Second line</Run></Paragraph></Section>"#,
            TextStyle::default(),
        ).unwrap();
        let visual = RegionVisual {
            size: [240., 0.],
            background: None,
            document: Some(document),
        };
        context
            .run_ui(egui::RawInput::default(), |ui| {
                let layout = retained_layout(ui, &visual, 240.);
                let (offset, galley) = layout
                    .paint
                    .iter()
                    .find_map(|p| match p {
                        Paint::Text(offset, galley) => Some((offset, galley)),
                        _ => None,
                    })
                    .unwrap();
                let row = &galley.rows[0];
                let expected = offset.y + row.pos.y + row.glyphs[0].pos.y;
                assert!(expected > 7.);
                assert!((alignment_offset(ui, &visual, 240., false) + expected).abs() < 0.001);
                assert_eq!(alignment_offset(ui, &visual, 240., true), 0.);
                assert!(Arc::ptr_eq(&layout, &retained_layout(ui, &visual, 240.)));
            })
            .drop_without_applying_deltas();
    }

    #[test]
    fn new_spec_table_uses_natural_width_instead_of_filling_the_viewport() {
        let mut document = mathcraft_document::canvas::CanvasDocument::default();
        let table = mathcraft_document::spec_table::SpecTable::new(1, 1).unwrap();
        let id = document.add_opaque(0., 0., table.xml(), "spec-table");
        let region = document.regions.iter().find(|r| r.id == id).unwrap();
        let mut visuals = mathcraft_document::native_visual::NativeVisuals::default();
        visuals.update_spec_table(region).unwrap();
        let visual = visuals.region(region).unwrap().as_ref().unwrap();
        let context = egui::Context::default();
        crate::configure(&context);
        context
            .run_ui(egui::RawInput::default(), |ui| {
                let narrow = measure(ui, visual, 200.);
                let wide = measure(ui, visual, 1000.);
                assert!(narrow.x > 0. && narrow.x < 100.);
                assert_eq!(narrow, wide);
                assert_eq!(retained_layout(ui, visual, 1000.).cells.len(), 3);
            })
            .drop_without_applying_deltas();
    }

    #[test]
    fn native_spec_table_anchor_tracks_measured_header_rows_and_gap() {
        let context = egui::Context::default();
        crate::configure(&context);
        let document = read_flow_xml(
            r#"<Section xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"><Table CellSpacing="6"><Table.Columns><TableColumn Width="100"/></Table.Columns><TableRowGroup><TableRow><TableCell Padding="2,3,2,5"><Paragraph FontSize="31">Header</Paragraph></TableCell></TableRow><TableRow><TableCell Padding="2,7,2,11"><Paragraph FontSize="12">Unit</Paragraph></TableCell></TableRow><TableRow><TableCell><Paragraph>1</Paragraph></TableCell></TableRow></TableRowGroup></Table></Section>"#,
            TextStyle::default(),
        ).unwrap();
        let Block::Table(table) = &document.blocks[0] else {
            panic!("table")
        };
        let table = table.clone();
        let visual = RegionVisual {
            size: [100., 0.],
            background: None,
            document: Some(document),
        };
        context
            .run_ui(egui::RawInput::default(), |ui| {
                let measured_height = |row: usize| {
                    let cell = &table.cells[row];
                    layout_blocks(ui, &cell.blocks, 96.).size.y + cell.padding[1] + cell.padding[3]
                };
                let second_header_bottom = measured_height(0) + table.spacing + measured_height(1);
                let first_data_top = second_header_bottom + table.spacing;
                let expected = (second_header_bottom + first_data_top) / 2.;
                assert!((alignment_offset(ui, &visual, 100., true) + expected).abs() < 0.001);
                assert!(
                    alignment_offset(ui, &visual, 100., true)
                        < alignment_offset(ui, &visual, 100., false)
                );
                assert!(measure(ui, &visual, 100.).y > expected);
            })
            .drop_without_applying_deltas();
    }

    #[test]
    fn specification_cells_share_row_baselines_and_anchor_uses_content_bounds() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut document = read_flow_xml(r#"<Section xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"><Table><Table.Columns><TableColumn Width="110"/><TableColumn Width="110"/></Table.Columns><TableRowGroup><TableRow><TableCell><Paragraph>h1</Paragraph></TableCell><TableCell><Paragraph>h2</Paragraph></TableCell></TableRow><TableRow><TableCell><Paragraph>u1</Paragraph></TableCell><TableCell><Paragraph>u2</Paragraph></TableCell></TableRow><TableRow><TableCell><Paragraph>data1</Paragraph></TableCell><TableCell><Paragraph>data2</Paragraph></TableCell></TableRow></TableRowGroup></Table></Section>"#, TextStyle::default()).unwrap();
        let Block::Table(table) = &mut document.blocks[0] else {
            panic!("table")
        };
        for (i, cell) in table.cells[..4].iter_mut().enumerate() {
            cell.blocks = vec![Block::Math(mathcraft_document::native_visual::MathBlock {
                tree: mathcraft_editor::PortableTree {
                    kind: mathcraft_editor::NodeKind::Number((i + 1).to_string()),
                    children: vec![],
                },
                font_points: [11., 23., 17., 9.][i],
            })];
        }
        context
            .run_ui(egui::RawInput::default(), |ui| {
                let layout = layout_table(ui, table, 220.);
                let metrics = layout
                    .paint
                    .iter()
                    .filter_map(|p| {
                        if let Paint::Math(offset, editor, points) = p {
                            let (size, baseline) =
                                crate::formula::measured_geometry_readonly_styled(
                                    ui, editor, None, None, *points,
                                );
                            Some((offset.y + baseline, offset.y + size.y))
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();
                assert_eq!(metrics.len(), 4);
                assert!((metrics[0].0 - metrics[1].0).abs() < 0.001);
                assert!((metrics[2].0 - metrics[3].0).abs() < 0.001);
                let header_bottom = metrics[2].1.max(metrics[3].1);
                let data_top = layout
                    .paint
                    .iter()
                    .filter_map(|p| {
                        if let Paint::Text(offset, galley) = p {
                            galley.text().starts_with("data").then_some(offset.y)
                        } else {
                            None
                        }
                    })
                    .reduce(f32::min)
                    .unwrap();
                assert!(
                    (layout.table_header_gap_midpoint.unwrap() - (header_bottom + data_top) / 2.)
                        .abs()
                        < 0.001
                );
            })
            .drop_without_applying_deltas();
    }
    #[test]
    fn native_text_and_table_measure_draw_preserve_colors_cells_and_input() {
        let context = egui::Context::default();
        crate::configure(&context);
        let document = read_flow_xml(SAMPLE, TextStyle::default()).unwrap();
        let visual = RegionVisual {
            size: [240., 0.],
            background: Some([102, 45, 145, 255]),
            document: Some(document),
        };
        let before = visual.clone();
        let events = vec![egui::Event::Text("must stay input".into())];
        let output = context.run_ui(
            egui::RawInput {
                events: events.clone(),
                ..Default::default()
            },
            |ui| {
                let measured = measure(ui, &visual, 240.);
                let response = draw(ui, &visual, 240.);
                assert!((response.rect.height() - measured.y).abs() <= 1.);
                assert_eq!(response.sense, Sense::hover());
                assert!(!response.changed());
                assert_eq!(ui.input(|i| i.events.clone()), events);
                assert_eq!(visual, before);
                let first = retained_layout(ui, &visual, 240.);
                let next = retained_layout(ui, &visual, 240.);
                assert!(Arc::ptr_eq(&first, &next));
                assert!(first.size.y > 32.);
                let texts = first
                    .paint
                    .iter()
                    .filter_map(|p| {
                        if let Paint::Text(_, g) = p {
                            Some(g.text())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();
                for expected in ["LCCL-S 시스템 설계", "reference table", "1", "0.66"] {
                    assert!(texts.contains(&expected));
                }
            },
        );
        assert!(
            output.shapes.iter().any(
                |s| matches!(&s.shape,egui::Shape::Rect(r) if r.fill==color([102,45,145,255]))
            )
        );
        output.drop_without_applying_deltas();
    }

    #[test]
    fn readonly_specification_table_renders_unit_power_and_source_zeros_without_editor_slots() {
        use std::io::{Cursor, Write};
        use zip::{ZipWriter, write::SimpleFileOptions};
        let source = r#"<worksheet xmlns="http://schemas.mathsoft.com/worksheet50" xmlns:ml="http://schemas.mathsoft.com/math50"><regions><region region-id="26" actualWidth="100" actualHeight="125"><spec-table><math><ml:define><ml:id labels="VARIABLE">Area</ml:id><ml:apply><ml:scale/><ml:matrix rows="3" cols="1"><ml:real>1.00</ml:real><ml:placeholder/><ml:real>3.5</ml:real></ml:matrix><ml:apply><ml:pow/><ml:id labels="UNIT">mm</ml:id><ml:real>2</ml:real></ml:apply></ml:apply></ml:define></math></spec-table></region></regions></worksheet>"#;
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        zip.start_file("mathcad/worksheet.xml", SimpleFileOptions::default())
            .unwrap();
        zip.write_all(source.as_bytes()).unwrap();
        let package = mathcraft_document::native::NativePackage::from_bytes(
            &zip.finish().unwrap().into_inner(),
        )
        .unwrap();
        let visuals =
            mathcraft_document::native_visual::NativeVisuals::from_package(&package).unwrap();
        let visual = visuals.regions["26"].as_ref().unwrap();
        let context = egui::Context::default();
        crate::configure(&context);
        let output = context.run_ui(egui::RawInput::default(), |ui| {
            assert!(measure(ui, visual, 100.).y >= 125.);
            let response = draw(ui, visual, 100.);
            assert_eq!(response.sense, Sense::hover());
        });
        fn strings(shape: &egui::Shape, out: &mut Vec<String>) {
            match shape {
                egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
                egui::Shape::Vec(shapes) => {
                    for s in shapes {
                        strings(s, out)
                    }
                }
                _ => {}
            }
        }
        let mut texts = Vec::new();
        for shape in &output.shapes {
            strings(&shape.shape, &mut texts)
        }
        for expected in ["Area", "mm", "2", "1.00", "3.5"] {
            assert!(
                texts.iter().any(|s| s == expected),
                "missing {expected}: {texts:?}"
            );
        }
        assert!(!output.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Rect(rect) if rect.stroke.color==Color32::from_rgb(0,128,255))));
        output.drop_without_applying_deltas();
    }
}
