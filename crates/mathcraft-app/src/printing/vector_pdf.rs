//! Vector PDF backend for the retained egui display list. It embeds the same
//! independently licensed fonts used by the app, with explicit Unicode CMaps.
//! Unsupported drawing operations fail the export rather than dropping content.
use super::{MAX_OUTPUT_BYTES, RasterPage};
use eframe::egui::{self, Color32, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2};
use egui::epaint::{ClippedShape, ColorMode, PathStroke, TextShape};
use egui::{FontData, FontDefinitions};
use flate2::{Compression, write::ZlibEncoder};
use skrifa::{
    FontRef, MetadataProvider,
    instance::{LocationRef, Size},
};
use std::{
    collections::{BTreeMap, HashMap},
    fmt::Write as _,
    io::Write,
    sync::Arc,
};

mod font_subset;
#[cfg(test)]
mod tests;

#[derive(Clone, Debug)]
pub(super) struct VectorPage {
    pub shapes: Vec<ClippedShape>,
    pub fonts: FontDefinitions,
}

#[derive(Default)]
struct Objects(Vec<Vec<u8>>);
impl Objects {
    fn reserve(&mut self) -> usize {
        self.0.push(Vec::new());
        self.0.len()
    }
    fn add(&mut self, value: impl Into<Vec<u8>>) -> usize {
        let id = self.reserve();
        self.set(id, value);
        id
    }
    fn set(&mut self, id: usize, value: impl Into<Vec<u8>>) {
        self.0[id - 1] = value.into();
    }
    fn stream(&mut self, dictionary: &str, bytes: &[u8]) -> Result<usize, String> {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(bytes).map_err(|e| e.to_string())?;
        let bytes = encoder.finish().map_err(|e| e.to_string())?;
        let mut body = format!(
            "<< {dictionary} /Filter /FlateDecode /Length {} >>\nstream\n",
            bytes.len()
        )
        .into_bytes();
        body.extend_from_slice(&bytes);
        body.extend_from_slice(b"\nendstream");
        Ok(self.add(body))
    }
    fn finish(self, root: usize) -> Result<Vec<u8>, String> {
        let mut out = b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n".to_vec();
        let mut offsets = vec![0];
        for (index, body) in self.0.iter().enumerate() {
            offsets.push(out.len());
            out.extend_from_slice(format!("{} 0 obj\n", index + 1).as_bytes());
            out.extend_from_slice(body);
            out.extend_from_slice(b"\nendobj\n");
            if out.len() > MAX_OUTPUT_BYTES {
                return Err("PDF 데이터가 256 MiB를 초과합니다.".into());
            }
        }
        let xref = out.len();
        out.extend_from_slice(
            format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len()).as_bytes(),
        );
        for offset in &offsets[1..] {
            out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
        }
        out.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root {root} 0 R >>\nstartxref\n{xref}\n%%EOF\n",
                offsets.len()
            )
            .as_bytes(),
        );
        Ok(out)
    }
}

struct UsedFont {
    data: Arc<FontData>,
    chars: BTreeMap<(u16, String), (u16, f32)>, // (GID, Unicode cluster) -> CID, advance
}
impl UsedFont {
    fn needs_outline_font(&self) -> bool {
        self.data.index != 0 || !self.data.font.starts_with(&[0, 1, 0, 0])
    }
    fn name_and_code(&self, index: usize, cid: u16) -> (String, String) {
        if self.needs_outline_font() {
            (
                format!("F{index}p{}", (cid - 1) / 255),
                format!("{:02X}", (cid - 1) % 255 + 1),
            )
        } else {
            (format!("F{index}"), format!("{cid:04X}"))
        }
    }
}

#[derive(Default)]
struct PdfPen {
    path: String,
    position: Pos2,
}
impl skrifa::outline::pen::OutlinePen for PdfPen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.position = Pos2::new(x, y);
        point(&mut self.path, self.position, "m");
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.position = Pos2::new(x, y);
        point(&mut self.path, self.position, "l");
    }
    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        let end = Pos2::new(x, y);
        let control = Pos2::new(cx, cy);
        let a = self.position + (control - self.position) * (2. / 3.);
        let b = end + (control - end) * (2. / 3.);
        self.curve_to(a.x, a.y, b.x, b.y, x, y);
    }
    fn curve_to(&mut self, ax: f32, ay: f32, bx: f32, by: f32, x: f32, y: f32) {
        writeln!(self.path, "{ax:.5} {ay:.5} {bx:.5} {by:.5} {x:.5} {y:.5} c").unwrap();
        self.position = Pos2::new(x, y);
    }
    fn close(&mut self) {
        self.path.push_str("h\n");
    }
}

fn unicode_cmap(entries: &[(u16, &str)], single_byte: bool) -> String {
    let mut cmap = String::from(
        "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n/CMapName /MathcraftUnicode def\n/CMapType 2 def\n1 begincodespacerange\n",
    );
    cmap.push_str(if single_byte {
        "<00> <FF>\n"
    } else {
        "<0000> <FFFF>\n"
    });
    cmap.push_str("endcodespacerange\n");
    for chunk in entries.chunks(100) {
        writeln!(cmap, "{} beginbfchar", chunk.len()).unwrap();
        for (cid, text) in chunk {
            let unicode = text
                .encode_utf16()
                .map(|v| format!("{v:04X}"))
                .collect::<String>();
            let code = if single_byte {
                format!("{cid:02X}")
            } else {
                format!("{cid:04X}")
            };
            writeln!(cmap, "<{code}> <{unicode}>").unwrap();
        }
        cmap.push_str("endbfchar\n");
    }
    cmap.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
    cmap
}

fn outline_font(
    objects: &mut Objects,
    index: usize,
    used: &UsedFont,
    font: &FontRef<'_>,
) -> Result<String, String> {
    let metrics = font.metrics(Size::unscaled(), LocationRef::default());
    let scale = 1000. / f32::from(metrics.units_per_em);
    let bbox = metrics.bounds.ok_or("PDF 글꼴 경계가 없습니다.")?;
    let mut entries: Vec<_> = used.chars.iter().collect();
    entries.sort_by_key(|(_, v)| v.0);
    let mut refs = String::new();
    for (part, chunk) in entries.chunks(255).enumerate() {
        let mut procs = String::new();
        let mut differences = String::new();
        let mut widths = String::new();
        let mut cmap = Vec::new();
        for (n, ((gid, unicode), (_, width))) in chunk.iter().enumerate() {
            let cid = (n + 1) as u16;
            let mut pen = PdfPen::default();
            let glyph = font
                .outline_glyphs()
                .get(skrifa::GlyphId::new(u32::from(*gid)))
                .ok_or("PDF 글리프 윤곽선이 없습니다.")?;
            glyph
                .draw(
                    skrifa::outline::DrawSettings::unhinted(
                        Size::unscaled(),
                        LocationRef::default(),
                    ),
                    &mut pen,
                )
                .map_err(|e| format!("PDF 글리프 윤곽선 해석 실패: {e}"))?;
            let proc = format!(
                "{width:.5} 0 {:.5} {:.5} {:.5} {:.5} d1\n{scale:.8} 0 0 {scale:.8} 0 0 cm\n{}f\n",
                bbox.x_min * scale,
                bbox.y_min * scale,
                bbox.x_max * scale,
                bbox.y_max * scale,
                pen.path
            );
            let id = objects.stream("", proc.as_bytes())?;
            write!(procs, "/g{cid} {id} 0 R ").unwrap();
            write!(differences, "/g{cid} ").unwrap();
            write!(widths, "{width:.5} ").unwrap();
            cmap.push((cid, unicode.as_str()));
        }
        let cmap = objects.stream("", unicode_cmap(&cmap, true).as_bytes())?;
        let id=objects.add(format!("<< /Type /Font /Subtype /Type3 /Name /MathcraftOutline{index}p{part} /FontBBox [{:.5} {:.5} {:.5} {:.5}] /FontMatrix [0.001 0 0 0.001 0 0] /CharProcs << {procs} >> /Encoding << /Type /Encoding /Differences [1 {differences}] >> /FirstChar 1 /LastChar {} /Widths [{widths}] /Resources << >> /ToUnicode {cmap} 0 R >>",bbox.x_min*scale,bbox.y_min*scale,bbox.x_max*scale,bbox.y_max*scale,chunk.len()));
        write!(refs, "/F{index}p{part} {id} 0 R ").unwrap();
    }
    Ok(refs)
}
#[derive(Default)]
struct Resources {
    fonts: Vec<UsedFont>,
    font_indices: HashMap<(usize, u32), usize>,
    alphas: BTreeMap<u8, usize>,
    edge_masks: Vec<([Pos2; 3], [u8; 3])>,
}
impl Resources {
    fn glyph(
        &mut self,
        fonts: &FontDefinitions,
        id: &egui::FontId,
        ch: char,
    ) -> Result<(usize, u16, f32, f32), String> {
        let names = fonts
            .families
            .get(&id.family)
            .ok_or("PDF 글꼴 모음이 없습니다.")?;
        for name in names {
            let data = fonts.font_data.get(name).ok_or("PDF 글꼴이 없습니다.")?;
            let font = FontRef::from_index(&data.font, data.index)
                .map_err(|e| format!("PDF 글꼴 분석 실패: {e}"))?;
            let mapped = if matches!(ch, '\t' | '\u{2009}' | '\u{202F}') {
                ' '
            } else {
                ch
            };
            let Some(gid) = font.charmap().map(mapped) else {
                continue;
            };
            if gid.to_u32() == 0 {
                continue;
            }
            let key = (Arc::as_ptr(data) as usize, data.index);
            let index = if let Some(index) = self.font_indices.get(&key) {
                *index
            } else {
                let index = self.fonts.len();
                self.fonts.push(UsedFont {
                    data: data.clone(),
                    chars: BTreeMap::new(),
                });
                self.font_indices.insert(key, index);
                index
            };
            let cid = self.add_glyph(index, gid.to_u32() as u16, &ch.to_string())?;
            let size = id.size * data.tweak.scale;
            use egui::emath::GuiRounding as _;
            let offset = (size * data.tweak.y_offset_factor + data.tweak.y_offset).round_ui();
            return Ok((index, cid, size, offset));
        }
        Err(format!(
            "PDF 글꼴에 문자 U+{:04X} ({ch})가 없습니다.",
            ch as u32
        ))
    }
    fn add_glyph(&mut self, index: usize, gid: u16, unicode: &str) -> Result<u16, String> {
        let used = &mut self.fonts[index];
        let key = (gid, unicode.to_owned());
        if let Some((cid, _)) = used.chars.get(&key) {
            return Ok(*cid);
        }
        let font =
            FontRef::from_index(&used.data.font, used.data.index).map_err(|e| e.to_string())?;
        let metrics = font.metrics(Size::unscaled(), LocationRef::default());
        let width = font
            .glyph_metrics(Size::unscaled(), LocationRef::default())
            .advance_width(skrifa::GlyphId::new(u32::from(gid)))
            .unwrap_or(0.)
            * 1000.
            / f32::from(metrics.units_per_em);
        let cid = u16::try_from(used.chars.len() + 1)
            .map_err(|_| "PDF 글꼴 문자 수가 한계를 초과합니다.")?;
        used.chars.insert(key, (cid, width));
        Ok(cid)
    }
    fn color(&mut self, out: &mut String, color: Color32, stroke: bool) {
        let [r, g, b, a] = color.to_srgba_unmultiplied();
        let next = self.alphas.len();
        let alpha = *self.alphas.entry(a).or_insert(next);
        writeln!(
            out,
            "/A{alpha} gs {:.6} {:.6} {:.6} {}",
            f32::from(r) / 255.,
            f32::from(g) / 255.,
            f32::from(b) / 255.,
            if stroke { "RG" } else { "rg" }
        )
        .unwrap();
    }
    /// egui's text decoration meshes contain a solid core and an alpha fringe.
    /// Keep that fringe as a vector Gouraud soft mask, never as a bitmap.
    fn triangle(
        &mut self,
        out: &mut String,
        points: [Pos2; 3],
        colors: [Color32; 3],
    ) -> Result<(), String> {
        if colors.iter().all(|c| *c == colors[0]) {
            return path_paint(
                out,
                self,
                &polyline(&points, true)?,
                colors[0],
                Stroke::NONE,
            );
        }
        finite(&points)?;
        let base = colors.iter().max_by_key(|c| c.a()).copied().unwrap();
        if base.a() == 0 {
            return Ok(());
        }
        let base_rgb = base.to_srgba_unmultiplied();
        if colors
            .iter()
            .filter(|c| c.a() != 0)
            .any(|c| c.to_srgba_unmultiplied()[..3] != base_rgb[..3])
        {
            return Err("PDF는 다색 그라데이션 메시를 지원하지 않습니다.".into());
        }
        let bounds = Rect::from_points(&points);
        if !bounds.is_positive() {
            return Ok(());
        }
        let index = self.edge_masks.len();
        self.edge_masks.push((points, colors.map(|c| c.a())));
        writeln!(
            out,
            "q /M{index} gs {:.6} {:.6} {:.6} rg",
            f32::from(base_rgb[0]) / 255.,
            f32::from(base_rgb[1]) / 255.,
            f32::from(base_rgb[2]) / 255.
        )
        .unwrap();
        out.push_str(&polyline(&points, true)?);
        out.push_str("f\nQ\n");
        Ok(())
    }
    fn mask_objects(&self, objects: &mut Objects) -> Result<String, String> {
        let mut refs = String::new();
        for (index, (points, alphas)) in self.edge_masks.iter().enumerate() {
            let bounds = Rect::from_points(points);
            let mut bytes = Vec::new();
            for (p, alpha) in points.iter().zip(alphas) {
                bytes.push(0); // Free-form Gouraud triangle vertex.
                for value in [
                    (p.x - bounds.left()) / bounds.width(),
                    (p.y - bounds.top()) / bounds.height(),
                ] {
                    bytes.extend_from_slice(
                        &((f64::from(value) * f64::from(u32::MAX)).round() as u32).to_be_bytes(),
                    );
                }
                bytes.push(*alpha);
            }
            let shade=objects.stream(&format!("/ShadingType 4 /ColorSpace /DeviceGray /BitsPerCoordinate 32 /BitsPerComponent 8 /BitsPerFlag 8 /Decode [{:.5} {:.5} {:.5} {:.5} 0 1]",bounds.left(),bounds.right(),bounds.top(),bounds.bottom()),&bytes)?;
            let group=objects.stream(&format!("/Type /XObject /Subtype /Form /FormType 1 /BBox [{:.5} {:.5} {:.5} {:.5}] /Group << /Type /Group /S /Transparency /CS /DeviceGray /I true >> /Resources << /Shading << /S0 {shade} 0 R >> >>",bounds.left(),bounds.top(),bounds.right(),bounds.bottom()),b"/S0 sh\n")?;
            let state=objects.add(format!("<< /Type /ExtGState /ca 1 /CA 1 /SMask << /S /Luminosity /G {group} 0 R /BC [0] >> >>"));
            write!(refs, "/M{index} {state} 0 R ").unwrap();
        }
        Ok(refs)
    }
    fn font_objects(&self, objects: &mut Objects) -> Result<String, String> {
        use skrifa::raw::TableProvider;
        let mut refs = String::new();
        for (index, used) in self.fonts.iter().enumerate() {
            let font =
                FontRef::from_index(&used.data.font, used.data.index).map_err(|e| e.to_string())?;
            if let Ok(os2) = font.os2() {
                let flags = os2.fs_type();
                if flags & 0x0202 != 0 {
                    return Err(
                        "이 글꼴의 라이선스가 PDF 윤곽선 글꼴 포함을 허용하지 않습니다.".into(),
                    );
                }
            }
            if used.needs_outline_font() {
                refs.push_str(&outline_font(objects, index, used, &font)?);
                continue;
            }
            let metrics = font.metrics(Size::unscaled(), LocationRef::default());
            let scale = 1000. / f32::from(metrics.units_per_em);
            let bbox = metrics.bounds.ok_or("PDF 글꼴 경계가 없습니다.")?;
            let subset = if font.os2().is_ok_and(|table| table.fs_type() & 0x100 != 0) {
                used.data.font.to_vec()
            } else {
                font_subset::subset(&used.data.font, used.chars.keys().map(|(gid, _)| *gid))?
            };
            let font_file = objects.stream(&format!("/Length1 {}", subset.len()), &subset)?;
            let name = format!("MathcraftFont{index}");
            let descriptor = objects.add(format!("<< /Type /FontDescriptor /FontName /{name} /Flags 32 /FontBBox [{:.4} {:.4} {:.4} {:.4}] /ItalicAngle {:.4} /Ascent {:.4} /Descent {:.4} /CapHeight {:.4} /StemV 80 /FontFile2 {font_file} 0 R >>", bbox.x_min*scale, bbox.y_min*scale, bbox.x_max*scale, bbox.y_max*scale, metrics.italic_angle, metrics.ascent*scale, metrics.descent*scale, metrics.cap_height.unwrap_or(metrics.ascent)*scale));
            let mut cmap = String::from(
                "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n/CMapName /MathcraftUnicode def\n/CMapType 2 def\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
            );
            let entries: Vec<_> = used.chars.iter().collect();
            for chunk in entries.chunks(100) {
                writeln!(cmap, "{} beginbfchar", chunk.len()).unwrap();
                for ((_, text), (cid, _)) in chunk {
                    let unicode = text
                        .encode_utf16()
                        .map(|v| format!("{v:04X}"))
                        .collect::<String>();
                    writeln!(cmap, "<{cid:04X}> <{unicode}>").unwrap();
                }
                cmap.push_str("endbfchar\n");
            }
            cmap.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
            let unicode = objects.stream("", cmap.as_bytes())?;
            let mut mapping = vec![0u8; (used.chars.len() + 1) * 2];
            let mut widths = String::new();
            for ((gid, _), (cid, width)) in &used.chars {
                mapping[usize::from(*cid) * 2..usize::from(*cid) * 2 + 2]
                    .copy_from_slice(&gid.to_be_bytes());
                write!(widths, "{cid} [{width:.4}] ").unwrap();
            }
            let mapping = objects.stream("", &mapping)?;
            let descendant = objects.add(format!("<< /Type /Font /Subtype /CIDFontType2 /BaseFont /{name} /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> /FontDescriptor {descriptor} 0 R /CIDToGIDMap {mapping} 0 R /W [{widths}] >>"));
            let type0 = objects.add(format!("<< /Type /Font /Subtype /Type0 /BaseFont /{name} /Encoding /Identity-H /DescendantFonts [{descendant} 0 R] /ToUnicode {unicode} 0 R >>"));
            write!(refs, "/F{index} {type0} 0 R ").unwrap();
        }
        Ok(refs)
    }
}

fn point(out: &mut String, p: Pos2, operator: &str) {
    writeln!(out, "{:.5} {:.5} {operator}", p.x, p.y).unwrap();
}
fn finite(points: &[Pos2]) -> Result<(), String> {
    if points.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) {
        Err("PDF 도형 좌표가 유한하지 않습니다.".into())
    } else {
        Ok(())
    }
}
fn path_paint(
    out: &mut String,
    resources: &mut Resources,
    path: &str,
    fill: Color32,
    stroke: Stroke,
) -> Result<(), String> {
    if !stroke.width.is_finite() || stroke.width < 0. {
        return Err("PDF 선 너비가 올바르지 않습니다.".into());
    }
    if fill.a() != 0 {
        resources.color(out, fill, false);
        out.push_str(path);
        out.push_str("f\n");
    }
    if stroke.width > 0. && stroke.color.a() != 0 {
        resources.color(out, stroke.color, true);
        writeln!(out, "{:.5} w 1 J 1 j", stroke.width).unwrap();
        out.push_str(path);
        out.push_str("S\n");
    }
    Ok(())
}
fn simple_stroke(stroke: &PathStroke) -> Result<Stroke, String> {
    match stroke.color {
        ColorMode::Solid(color) if stroke.width == 0. || stroke.kind == StrokeKind::Middle => {
            Ok(Stroke::new(stroke.width, color))
        }
        _ => Err("PDF는 색상 콜백 또는 안쪽/바깥쪽 경로 선을 지원하지 않습니다.".into()),
    }
}
fn polyline(points: &[Pos2], closed: bool) -> Result<String, String> {
    finite(points)?;
    let mut out = String::new();
    if let Some(first) = points.first() {
        point(&mut out, *first, "m");
        for p in &points[1..] {
            point(&mut out, *p, "l");
        }
        if closed {
            out.push_str("h\n");
        }
    }
    Ok(out)
}
fn rotate(p: Pos2, origin: Pos2, angle: f32) -> Pos2 {
    let (s, c) = angle.sin_cos();
    let d = p - origin;
    origin + Vec2::new(c * d.x - s * d.y, s * d.x + c * d.y)
}
fn rounded_rect(rect: Rect, radii: [f32; 4], origin: Pos2, angle: f32) -> Result<String, String> {
    let [nw, ne, se, sw] = radii.map(|v| v.max(0.).min(rect.width().min(rect.height()) * 0.5));
    let [l, t, r, b] = [rect.left(), rect.top(), rect.right(), rect.bottom()];
    finite(&[rect.min, rect.max, origin])?;
    if !angle.is_finite() {
        return Err("PDF 도형 회전이 유한하지 않습니다.".into());
    }
    let mut out = String::new();
    let transform = |x, y| rotate(Pos2::new(x, y), origin, angle);
    point(&mut out, transform(l + nw, t), "m");
    let k = 0.552_284_8;
    for (line, c1, c2, end) in [
        (
            (r - ne, t),
            (r - ne + k * ne, t),
            (r, t + ne - k * ne),
            (r, t + ne),
        ),
        (
            (r, b - se),
            (r, b - se + k * se),
            (r - se + k * se, b),
            (r - se, b),
        ),
        (
            (l + sw, b),
            (l + sw - k * sw, b),
            (l, b - sw + k * sw),
            (l, b - sw),
        ),
        (
            (l, t + nw),
            (l, t + nw - k * nw),
            (l + nw - k * nw, t),
            (l + nw, t),
        ),
    ] {
        point(&mut out, transform(line.0, line.1), "l");
        let [a, b, c] = [c1, c2, end].map(|p| transform(p.0, p.1));
        writeln!(
            out,
            "{:.5} {:.5} {:.5} {:.5} {:.5} {:.5} c",
            a.x, a.y, b.x, b.y, c.x, c.y
        )
        .unwrap();
    }
    out.push_str("h\n");
    Ok(out)
}
fn ellipse(center: Pos2, radius: Vec2) -> Result<String, String> {
    finite(&[center, center + radius])?;
    if radius.x < 0. || radius.y < 0. {
        return Err("PDF 타원 반경이 올바르지 않습니다.".into());
    }
    let k = 0.552_284_8;
    let mut out = String::new();
    point(&mut out, center + Vec2::new(radius.x, 0.), "m");
    for (a, b, c) in [
        ([1., k], [k, 1.], [0., 1.]),
        ([-k, 1.], [-1., k], [-1., 0.]),
        ([-1., -k], [-k, -1.], [0., -1.]),
        ([k, -1.], [1., -k], [1., 0.]),
    ] {
        let [a, b, c] = [a, b, c].map(|p| center + Vec2::new(p[0] * radius.x, p[1] * radius.y));
        writeln!(
            out,
            "{:.5} {:.5} {:.5} {:.5} {:.5} {:.5} c",
            a.x, a.y, b.x, b.y, c.x, c.y
        )
        .unwrap();
    }
    out.push_str("h\n");
    Ok(out)
}

struct TextGlyph {
    font: usize,
    cid: u16,
    size: f32,
    offset: f32,
    color: Color32,
    italics: bool,
}

/// Reproduce egui's shaping (including ffi ligatures), while keeping egui's
/// positioned baselines. The public Galley deliberately omits shaped GIDs.
/// Clusters consuming multiple Unicode characters receive one ToUnicode entry.
fn shaped_row(
    resources: &mut Resources,
    fonts: &FontDefinitions,
    job: &egui::text::LayoutJob,
    cursor: &mut egui::epaint::text::ByteIndex,
    count: usize,
) -> Result<Vec<Option<TextGlyph>>, String> {
    struct Input {
        font: usize,
        size: f32,
        offset: f32,
        section: usize,
        ch: char,
    }
    let mut inputs = Vec::with_capacity(count);
    for _ in 0..count {
        let ch = job
            .text
            .get(cursor.0..)
            .and_then(|s| s.chars().next())
            .ok_or("PDF 문자와 원본 문자열 길이가 다릅니다.")?;
        let section = job
            .sections
            .iter()
            .position(|s| s.byte_range.contains(cursor))
            .ok_or("PDF 문자 서식이 없습니다.")?;
        let format = &job.sections[section].format;
        if !format.coords.as_ref().is_empty() {
            return Err("PDF는 현재 가변 글꼴 축 재정의를 지원하지 않습니다.".into());
        }
        let (font, _, size, offset) = resources.glyph(fonts, &format.font_id, ch)?;
        inputs.push(Input {
            font,
            size,
            offset,
            section,
            ch,
        });
        cursor.0 += ch.len_utf8();
    }
    let mut output = Vec::with_capacity(count);
    let mut first = 0;
    while first < inputs.len() {
        let mut end = first + 1;
        while end < inputs.len()
            && inputs[end].font == inputs[first].font
            && inputs[end].section == inputs[first].section
        {
            end += 1;
        }
        let run = &inputs[first..end];
        let data = &resources.fonts[run[0].font].data;
        if !data.tweak.coords.as_ref().is_empty() {
            return Err("PDF는 현재 가변 글꼴 축 재정의를 지원하지 않습니다.".into());
        }
        let font = FontRef::from_index(&data.font, data.index).map_err(|e| e.to_string())?;
        let run_text = run.iter().map(|i| i.ch).collect::<String>();
        let mut buffer = harfrust::UnicodeBuffer::new();
        let mut flags = harfrust::BufferFlags::empty();
        if first == 0 {
            flags |= harfrust::BufferFlags::BEGINNING_OF_TEXT;
        }
        if end == inputs.len() {
            flags |= harfrust::BufferFlags::END_OF_TEXT;
        }
        buffer.set_flags(flags);
        buffer.push_str(&run_text);
        buffer.guess_segment_properties();
        let shaper_data = harfrust::ShaperData::new(&font);
        let shaped = shaper_data
            .shaper(&font)
            .build()
            .shape(buffer, harfrust::ShapeOptions::new());
        let infos = shaped.glyph_infos();
        if infos.windows(2).any(|w| w[0].cluster > w[1].cluster) {
            return Err(
                "PDF의 오른쪽에서 왼쪽으로 쓰는 문자 조판은 아직 지원하지 않습니다.".into(),
            );
        }
        let mut index = 0;
        while index < infos.len() {
            let start = infos[index].cluster as usize;
            let mut next = index + 1;
            while next < infos.len() && infos[next].cluster == infos[index].cluster {
                next += 1;
            }
            let stop = if next == infos.len() {
                run_text.len()
            } else {
                infos[next].cluster as usize
            };
            let cluster = run_text
                .get(start..stop)
                .ok_or("PDF 문자 클러스터 범위 오류")?;
            let chars: Vec<_> = cluster.chars().collect();
            let glyphs = next - index;
            if glyphs > chars.len() || (glyphs > 1 && glyphs != chars.len()) {
                return Err("PDF의 다중 글리프 클러스터 조판은 아직 지원하지 않습니다.".into());
            }
            for (n, info) in infos[index..next].iter().enumerate() {
                if info.glyph_id == 0 {
                    return Err("PDF 문자 조판 결과에 누락된 글리프가 있습니다.".into());
                }
                let unicode = if glyphs == 1 {
                    cluster.to_owned()
                } else {
                    chars[n].to_string()
                };
                let cid = resources.add_glyph(
                    run[0].font,
                    u16::try_from(info.glyph_id).map_err(|_| "PDF 글리프 번호 초과")?,
                    &unicode,
                )?;
                output.push(Some(TextGlyph {
                    font: run[0].font,
                    cid,
                    size: run[0].size,
                    offset: run[0].offset,
                    color: job.sections[run[0].section].format.color,
                    italics: job.sections[run[0].section].format.italics,
                }));
            }
            for _ in glyphs..chars.len() {
                output.push(None);
            }
            index = next;
        }
        first = end;
    }
    if output.len() != count {
        return Err("PDF와 화면의 문자 조판 결과가 다릅니다.".into());
    }
    Ok(output)
}

fn text(
    out: &mut String,
    resources: &mut Resources,
    shape: &TextShape,
    fonts: &FontDefinitions,
) -> Result<(), String> {
    if !shape.angle.is_finite() || !shape.opacity_factor.is_finite() {
        return Err("PDF 문자 회전/투명도가 올바르지 않습니다.".into());
    }
    let transform = |p: Pos2| shape.pos + rotate(p, Pos2::ZERO, shape.angle).to_vec2();
    let color = |c: Color32| {
        if c == Color32::PLACEHOLDER {
            shape.fallback_color
        } else {
            c
        }
        .gamma_multiply(shape.opacity_factor)
    };
    let mut cursor = egui::epaint::text::ByteIndex(0);
    if shape.galley.elided {
        return Err("PDF는 생략된 문자열 조판을 지원하지 않습니다.".into());
    }
    for row in &shape.galley.rows {
        let shaped = shaped_row(
            resources,
            fonts,
            &shape.galley.job,
            &mut cursor,
            row.glyphs.len(),
        )?;
        // Backgrounds precede glyphs; decorations follow them in the retained mesh.
        let mesh = &row.visuals.mesh;
        for phase in 0..2 {
            for triangle in mesh.indices.as_chunks::<3>().0 {
                if triangle
                    .iter()
                    .any(|i| row.visuals.glyph_vertex_range.contains(&(*i as usize)))
                {
                    continue;
                }
                let before = triangle
                    .iter()
                    .all(|i| (*i as usize) < row.visuals.glyph_vertex_range.start);
                if before != (phase == 0) {
                    continue;
                }
                let vertices =
                    [triangle[0], triangle[1], triangle[2]].map(|i| mesh.vertices[i as usize]);
                let points = vertices.map(|v| transform(row.pos + v.pos.to_vec2()));
                resources.triangle(out, points, vertices.map(|v| color(v.color)))?;
            }
            if phase == 1 {
                break;
            }
            for (glyph, metadata) in row.glyphs.iter().zip(&shaped) {
                let Some(TextGlyph {
                    font,
                    cid,
                    size,
                    offset,
                    color: foreground,
                    italics,
                }) = metadata
                else {
                    continue;
                };
                let (font, cid, size, offset) = (*font, *cid, *size, *offset);
                if !size.is_finite() || size <= 0. {
                    return Err("PDF 문자 크기가 올바르지 않습니다.".into());
                }
                resources.color(
                    out,
                    color(shape.override_text_color.unwrap_or(*foreground)),
                    false,
                );
                let shift = if *italics {
                    (glyph.uv_rect.offset.y + glyph.uv_rect.size.y - offset) * 0.25
                } else {
                    0.
                };
                let p = transform(row.pos + glyph.pos.to_vec2() + Vec2::new(shift, offset));
                finite(&[p])?;
                let (s, c) = shape.angle.sin_cos();
                let skew = if *italics { 0.25 } else { 0. };
                let (font_name, code) = resources.fonts[font].name_and_code(font, cid);
                writeln!(out,"BT /{font_name} {size:.5} Tf {c:.7} {s:.7} {:.7} {:.7} {:.5} {:.5} Tm <{code}> Tj ET",s+c*skew,-c+s*skew,p.x,p.y).unwrap();
            }
        }
        if row.ends_with_newline {
            cursor.0 += 1;
        }
        if shape.underline.width > 0. {
            let y = row.pos.y + row.size.y;
            let points = [
                transform(Pos2::new(row.pos.x, y)),
                transform(Pos2::new(row.pos.x + row.size.x, y)),
            ];
            path_paint(
                out,
                resources,
                &polyline(&points, false)?,
                Color32::TRANSPARENT,
                Stroke::new(shape.underline.width, color(shape.underline.color)),
            )?;
        }
    }
    Ok(())
}

fn shape(
    out: &mut String,
    resources: &mut Resources,
    item: &Shape,
    fonts: &FontDefinitions,
) -> Result<(), String> {
    match item {
        Shape::Noop => {}
        Shape::Vec(shapes) => {
            for item in shapes {
                shape(out, resources, item, fonts)?;
            }
        }
        Shape::Text(item) => text(out, resources, item, fonts)?,
        Shape::LineSegment { points, stroke } => path_paint(
            out,
            resources,
            &polyline(points, false)?,
            Color32::TRANSPARENT,
            *stroke,
        )?,
        Shape::Path(item) => path_paint(
            out,
            resources,
            &polyline(&item.points, item.closed)?,
            item.fill,
            simple_stroke(&item.stroke)?,
        )?,
        Shape::Circle(item) => path_paint(
            out,
            resources,
            &ellipse(item.center, Vec2::splat(item.radius))?,
            item.fill,
            item.stroke,
        )?,
        Shape::Ellipse(item) => path_paint(
            out,
            resources,
            &ellipse(item.center, item.radius)?,
            item.fill,
            item.stroke,
        )?,
        Shape::Rect(item) => {
            if item.brush.is_some() || item.blur_width != 0. {
                return Err("PDF는 텍스처/흐림 사각형을 지원하지 않습니다.".into());
            }
            let radii = [
                item.corner_radius.nw,
                item.corner_radius.ne,
                item.corner_radius.se,
                item.corner_radius.sw,
            ]
            .map(f32::from);
            let path = rounded_rect(item.rect, radii, item.rect.center(), item.angle)?;
            path_paint(out, resources, &path, item.fill, Stroke::NONE)?;
            let shift = match item.stroke_kind {
                StrokeKind::Inside => -item.stroke.width * 0.5,
                StrokeKind::Middle => 0.,
                StrokeKind::Outside => item.stroke.width * 0.5,
            };
            if item.stroke.width > 0. && item.stroke.color.a() != 0 {
                let rect = item.rect.expand(shift);
                if rect.is_positive() {
                    path_paint(
                        out,
                        resources,
                        &rounded_rect(
                            rect,
                            radii.map(|r| r + shift),
                            item.rect.center(),
                            item.angle,
                        )?,
                        Color32::TRANSPARENT,
                        item.stroke,
                    )?;
                }
            }
        }
        Shape::QuadraticBezier(item) => {
            finite(&item.points)?;
            let [p0, p1, p2] = item.points;
            let controls = [p0 + (p1 - p0) * (2. / 3.), p2 + (p1 - p2) * (2. / 3.), p2];
            let mut path = String::new();
            point(&mut path, p0, "m");
            writeln!(
                path,
                "{:.5} {:.5} {:.5} {:.5} {:.5} {:.5} c",
                controls[0].x, controls[0].y, controls[1].x, controls[1].y, p2.x, p2.y
            )
            .unwrap();
            if item.closed {
                path.push_str("h\n");
            }
            path_paint(
                out,
                resources,
                &path,
                item.fill,
                simple_stroke(&item.stroke)?,
            )?;
        }
        Shape::CubicBezier(item) => {
            finite(&item.points)?;
            let [p0, a, b, c] = item.points;
            let mut path = String::new();
            point(&mut path, p0, "m");
            writeln!(
                path,
                "{:.5} {:.5} {:.5} {:.5} {:.5} {:.5} c",
                a.x, a.y, b.x, b.y, c.x, c.y
            )
            .unwrap();
            if item.closed {
                path.push_str("h\n");
            }
            path_paint(
                out,
                resources,
                &path,
                item.fill,
                simple_stroke(&item.stroke)?,
            )?;
        }
        Shape::Mesh(mesh) => {
            for triangle in mesh.indices.as_chunks::<3>().0 {
                let vertices = [
                    mesh.vertices[triangle[0] as usize],
                    mesh.vertices[triangle[1] as usize],
                    mesh.vertices[triangle[2] as usize],
                ];
                if mesh.texture_id != egui::TextureId::default()
                    || vertices.iter().any(|v| v.uv != egui::epaint::WHITE_UV)
                {
                    return Err("PDF는 텍스처 또는 그라데이션 메시를 지원하지 않습니다.".into());
                }
                resources.triangle(out, vertices.map(|v| v.pos), vertices.map(|v| v.color))?;
            }
        }
        Shape::Callback(_) => return Err("PDF는 GPU 콜백 도형을 지원하지 않습니다.".into()),
    }
    Ok(())
}

pub(super) fn encode(pages: &[RasterPage]) -> Result<Vec<u8>, String> {
    let mut resources = Resources::default();
    let mut contents = Vec::with_capacity(pages.len());
    for page in pages {
        let mut out = format!("q\n0.75 0 0 -0.75 0 {:.5} cm\n", page.paper_size[1] * 0.75);
        let paper = Rect::from_min_size(
            Pos2::ZERO,
            Vec2::new(page.paper_size[0], page.paper_size[1]),
        );
        for clipped in &page.vector.shapes {
            let clip = clipped.clip_rect.intersect(paper);
            if !clip.is_positive() {
                continue;
            }
            finite(&[clip.min, clip.max])?;
            writeln!(
                out,
                "q {:.5} {:.5} {:.5} {:.5} re W n",
                clip.left(),
                clip.top(),
                clip.width(),
                clip.height()
            )
            .unwrap();
            shape(&mut out, &mut resources, &clipped.shape, &page.vector.fonts)?;
            out.push_str("Q\n");
            if out.len() > MAX_OUTPUT_BYTES {
                return Err("PDF 페이지 데이터 크기가 한계를 초과합니다.".into());
            }
        }
        out.push_str("Q\n");
        contents.push(out);
    }
    let mut objects = Objects::default();
    let catalog = objects.reserve();
    let tree = objects.reserve();
    let fonts = resources.font_objects(&mut objects)?;
    let mut alphas = String::new();
    alphas.push_str(&resources.mask_objects(&mut objects)?);
    for (alpha, index) in &resources.alphas {
        let value = f32::from(*alpha) / 255.;
        let id = objects.add(format!(
            "<< /Type /ExtGState /ca {value:.7} /CA {value:.7} >>"
        ));
        write!(alphas, "/A{index} {id} 0 R ").unwrap();
    }
    let resource_id = objects.add(format!(
        "<< /Font << {fonts} >> /ExtGState << {alphas} >> >>"
    ));
    let mut kids = String::new();
    for (page, content) in pages.iter().zip(contents) {
        let stream = objects.stream("", content.as_bytes())?;
        let [w, h] = page.paper_size.map(|v| v * 0.75);
        let id = objects.add(format!("<< /Type /Page /Parent {tree} 0 R /MediaBox [0 0 {w:.5} {h:.5}] /Resources {resource_id} 0 R /Contents {stream} 0 R >>"));
        write!(kids, "{id} 0 R ").unwrap();
    }
    objects.set(
        tree,
        format!("<< /Type /Pages /Count {} /Kids [{kids}] >>", pages.len()),
    );
    objects.set(catalog, format!("<< /Type /Catalog /Pages {tree} 0 R >>"));
    objects.finish(catalog)
}
