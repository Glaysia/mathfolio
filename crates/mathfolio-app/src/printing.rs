//! Headless worksheet printing. The normal equation renderer supplies page
//! shapes; a bounded software painter exports the same shapes without a GPU,
//! window, worksheet recalculation or a printer dialog.
#[cfg(test)]
use eframe::egui::Color32;
use eframe::egui::{self, Pos2, Rect, TextureId, Vec2};
use image::ImageEncoder;
use std::collections::HashMap;
#[cfg(not(target_arch = "wasm32"))]
use std::{fs, io::Write, path::Path, process::Command};
mod vector_pdf;

const MAX_PAGES: usize = 1000;
const MAX_PAGE_PIXELS: usize = 40_000_000;
const MAX_OUTPUT_BYTES: usize = 256 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PageSelection {
    All,
    /// Inclusive, one-based document page numbers, independent of header numbering.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    Range {
        first: usize,
        last: usize,
    },
    /// Pages containing selected worksheet regions, zero-based and sorted.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    Indices(Vec<usize>),
}

#[derive(Clone, Debug)]
pub(crate) struct PrintSettings {
    /// Physical paper dimensions in 96-DPI worksheet coordinates.
    pub paper_size: [f32; 2],
    pub margins: [f32; 4],
    /// Use PageGeometry's grid-rounded body height, not raw paper minus margins.
    pub body_height: f64,
    pub content_bottom: f64,
    pub pages: PageSelection,
    pub dpi: u16,
}

#[derive(Clone, Debug)]
pub(crate) struct PrintPlan {
    paper_size: [f32; 2],
    pixels: [usize; 2],
    dpi: u16,
    page_count: usize,
    source_pages: Vec<usize>,
}

impl PrintPlan {
    pub fn new(settings: &PrintSettings) -> Result<Self, String> {
        let [width, height] = settings.paper_size;
        if !width.is_finite()
            || !height.is_finite()
            || width <= 0.
            || height <= 0.
            || settings.margins.iter().any(|v| !v.is_finite() || *v < 0.)
            || settings.margins[0] + settings.margins[2] >= width
            || settings.margins[1] + settings.margins[3] >= height
            || !settings.body_height.is_finite()
            || settings.body_height <= 0.
            || settings.body_height > f64::from(height)
            || !settings.content_bottom.is_finite()
            || settings.content_bottom < 0.
            || !(72..=600).contains(&settings.dpi)
        {
            return Err("인쇄 용지, 여백, 배율 또는 문서 범위가 올바르지 않습니다.".into());
        }
        let count = (settings.content_bottom / settings.body_height)
            .ceil()
            .max(1.);
        if count > MAX_PAGES as f64 {
            return Err(format!("한 번에 {MAX_PAGES}페이지까지 인쇄할 수 있습니다."));
        }
        let page_count = count as usize;
        let source_pages = match &settings.pages {
            PageSelection::All => (0..page_count).collect(),
            PageSelection::Range { first, last }
                if *first > 0 && first <= last && *last <= page_count =>
            {
                (first - 1..*last).collect()
            }
            PageSelection::Range { .. } => {
                return Err(format!("페이지 범위는 1–{page_count} 안이어야 합니다."));
            }
            PageSelection::Indices(indices)
                if !indices.is_empty()
                    && indices.iter().all(|p| *p < page_count)
                    && indices.windows(2).all(|p| p[0] < p[1]) =>
            {
                indices.clone()
            }
            PageSelection::Indices(_) => {
                return Err("선택한 인쇄 페이지가 없거나 범위가 올바르지 않습니다.".into());
            }
        };
        let scale = f32::from(settings.dpi) / 96.;
        let pixels = [
            (width * scale).ceil() as usize,
            (height * scale).ceil() as usize,
        ];
        if pixels[0]
            .checked_mul(pixels[1])
            .is_none_or(|n| n > MAX_PAGE_PIXELS)
        {
            return Err("인쇄 페이지의 픽셀 수가 한계를 초과합니다. 해상도를 낮춰주세요.".into());
        }
        Ok(Self {
            paper_size: settings.paper_size,
            pixels,
            dpi: settings.dpi,
            page_count,
            source_pages,
        })
    }
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub fn page_count(&self) -> usize {
        self.page_count
    }
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub fn source_pages(&self) -> &[usize] {
        &self.source_pages
    }
}

/// Retained page shapes and fonts, plus an optional PNG for Windows GDI.
/// Vector-only jobs leave the PNG empty and never allocate a page bitmap.
#[derive(Clone, Debug)]
pub(crate) struct RasterPage {
    paper_size: [f32; 2],
    #[cfg_attr(
        not(target_os = "windows"),
        allow(
            dead_code,
            reason = "Windows native printing consumes PNG; CUPS consumes the vector PDF"
        )
    )]
    png: Vec<u8>,
    vector: vector_pdf::VectorPage,
}

/// The callback paints a whole paper in ordinary worksheet DIP coordinates;
/// its page index is zero-based and refers to the original document. It must
/// consume existing results only, never run the evaluator to print.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn render_pages(
    plan: &PrintPlan,
    paint: impl FnMut(&mut egui::Ui, usize, usize),
) -> Result<Vec<RasterPage>, String> {
    render_pages_with_mode(plan, paint, true, None)
}

/// PDF/CUPS output keeps the layout's paths and text, without allocating a
/// paper-sized bitmap or encoding a PNG. DPI only affects the native fallback.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn render_vector_pages(
    plan: &PrintPlan,
    paint: impl FnMut(&mut egui::Ui, usize, usize),
) -> Result<Vec<RasterPage>, String> {
    render_pages_with_mode(plan, paint, false, None)
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn render_host_vector_pages(
    plan: &PrintPlan,
    fonts: egui::FontDefinitions,
    paint: impl FnMut(&mut egui::Ui, usize, usize),
) -> Result<Vec<RasterPage>, String> {
    render_pages_with_mode(plan, paint, false, Some(fonts))
}

fn render_pages_with_mode(
    plan: &PrintPlan,
    mut paint: impl FnMut(&mut egui::Ui, usize, usize),
    rasterize: bool,
    fonts: Option<egui::FontDefinitions>,
) -> Result<Vec<RasterPage>, String> {
    let context = egui::Context::default();
    if let Some(fonts) = fonts {
        crate::configure_surface(&context, fonts, crate::Theme::Light);
    } else {
        crate::configure(&context);
    }
    let scale = if rasterize {
        f32::from(plan.dpi) / 96.
    } else {
        1.
    };
    let mut textures = HashMap::<TextureId, egui::ColorImage>::new();
    let mut pages = Vec::with_capacity(plan.source_pages.len());
    let mut bytes = 0usize;
    for &page in &plan.source_pages {
        let mut raster = Vec::new();
        let mut vector = None;
        // egui_plot reserves axis-label space from its previous layout. A pure
        // warm-up pass settles that space without rasterizing or waiting for an
        // application frame; only the second pass becomes printable output.
        for layout_pass in 0..2 {
            let mut input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    Pos2::ZERO,
                    Vec2::new(plan.paper_size[0], plan.paper_size[1]),
                )),
                max_texture_side: Some(8192),
                ..Default::default()
            };
            input
                .viewports
                .entry(egui::ViewportId::ROOT)
                .or_default()
                .native_pixels_per_point = Some(scale);
            let mut output = context.run_ui(input, |ui| {
                ui.set_clip_rect(ui.max_rect());
                paint(ui, page, plan.page_count);
            });
            // TexturesDelta's debug drop asserts that backends consumed it. Take it
            // even on painter errors, rather than leaving a pending font upload.
            let mut delta = std::mem::take(&mut output.textures_delta);
            let sets = std::mem::take(&mut delta.set);
            let frees = std::mem::take(&mut delta.free);
            for (id, changes) in sets.iter().filter(|_| rasterize) {
                for change in changes {
                    let egui::ImageData::Color(image) = &change.image;
                    if let Some([left, top]) = change.pos {
                        let target = textures
                            .get_mut(id)
                            .ok_or("인쇄 글꼴 텍스처 패치의 원본이 없습니다.")?;
                        if left + image.size[0] > target.size[0]
                            || top + image.size[1] > target.size[1]
                        {
                            return Err("인쇄 텍스처 패치 범위가 올바르지 않습니다.".into());
                        }
                        for y in 0..image.size[1] {
                            let offset = (top + y) * target.size[0] + left;
                            target.pixels[offset..offset + image.size[0]].copy_from_slice(
                                &image.pixels[y * image.size[0]..(y + 1) * image.size[0]],
                            );
                        }
                    } else {
                        textures.insert(*id, image.as_ref().clone());
                    }
                }
            }
            if layout_pass == 1 {
                vector = Some(vector_pdf::VectorPage {
                    shapes: output.shapes.clone(),
                    fonts: context.fonts(|fonts| fonts.definitions().clone()),
                });
                if rasterize {
                    let primitives = context
                        .tessellate(std::mem::take(&mut output.shapes), output.pixels_per_point);
                    raster =
                        paint_meshes(plan.pixels, output.pixels_per_point, &primitives, &textures)?;
                }
            }
            for id in &frees {
                textures.remove(id);
            }
        }
        let mut png = Vec::new();
        if rasterize {
            image::codecs::png::PngEncoder::new_with_quality(
                &mut png,
                image::codecs::png::CompressionType::Fast,
                image::codecs::png::FilterType::Adaptive,
            )
            .write_image(
                &raster,
                plan.pixels[0] as u32,
                plan.pixels[1] as u32,
                image::ExtendedColorType::Rgb8,
            )
            .map_err(|e| format!("인쇄 페이지 압축 실패: {e}"))?;
        }
        bytes = bytes
            .checked_add(png.len())
            .ok_or("인쇄 데이터 크기 초과")?;
        if bytes > MAX_OUTPUT_BYTES {
            return Err("인쇄 데이터가 256 MiB를 초과합니다. 페이지 범위를 나눠주세요.".into());
        }
        pages.push(RasterPage {
            paper_size: plan.paper_size,
            png,
            vector: vector.expect("second layout pass"),
        });
    }
    Ok(pages)
}

fn paint_meshes(
    size: [usize; 2],
    scale: f32,
    primitives: &[egui::epaint::ClippedPrimitive],
    textures: &HashMap<TextureId, egui::ColorImage>,
) -> Result<Vec<u8>, String> {
    let mut raster = vec![255; size[0] * size[1] * 3];
    for primitive in primitives {
        let egui::epaint::Primitive::Mesh(mesh) = &primitive.primitive else {
            return Err("GPU 전용 내용은 인쇄할 수 없습니다.".into());
        };
        let texture = textures
            .get(&mesh.texture_id)
            .ok_or("인쇄에 필요한 텍스처가 없습니다.")?;
        if texture.size[0] == 0 || texture.size[1] == 0 {
            return Err("빈 인쇄 텍스처".into());
        }
        let clip = primitive.clip_rect * scale;
        for triangle in mesh.indices.as_chunks::<3>().0 {
            let vertices = [
                mesh.vertices[triangle[0] as usize],
                mesh.vertices[triangle[1] as usize],
                mesh.vertices[triangle[2] as usize],
            ];
            paint_triangle(&mut raster, size, scale, clip, vertices, texture);
        }
    }
    Ok(raster)
}

fn cross(a: Pos2, b: Pos2, p: Pos2) -> f32 {
    (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x)
}
fn top_left(a: Pos2, b: Pos2) -> bool {
    b.y < a.y || (b.y == a.y && b.x > a.x)
}

fn paint_triangle(
    raster: &mut [u8],
    size: [usize; 2],
    scale: f32,
    clip: Rect,
    mut vertices: [egui::epaint::Vertex; 3],
    texture: &egui::ColorImage,
) {
    let mut p = vertices.map(|v| Pos2::new(v.pos.x * scale, v.pos.y * scale));
    let mut area = cross(p[0], p[1], p[2]);
    if !area.is_finite() || area.abs() < 1e-8 {
        return;
    }
    if area < 0. {
        p.swap(1, 2);
        vertices.swap(1, 2);
        area = -area;
    }
    let min = Pos2::new(
        p.iter()
            .map(|p| p.x)
            .fold(f32::INFINITY, f32::min)
            .max(clip.min.x)
            .max(0.),
        p.iter()
            .map(|p| p.y)
            .fold(f32::INFINITY, f32::min)
            .max(clip.min.y)
            .max(0.),
    );
    let max = Pos2::new(
        p.iter()
            .map(|p| p.x)
            .fold(f32::NEG_INFINITY, f32::max)
            .min(clip.max.x)
            .min(size[0] as f32),
        p.iter()
            .map(|p| p.y)
            .fold(f32::NEG_INFINITY, f32::max)
            .min(clip.max.y)
            .min(size[1] as f32),
    );
    let edge_top = [
        top_left(p[1], p[2]),
        top_left(p[2], p[0]),
        top_left(p[0], p[1]),
    ];
    for y in min.y.floor() as usize..max.y.ceil().max(0.) as usize {
        for x in min.x.floor() as usize..max.x.ceil().max(0.) as usize {
            let point = Pos2::new(x as f32 + 0.5, y as f32 + 0.5);
            if !clip.contains(point) {
                continue;
            }
            let edges = [
                cross(p[1], p[2], point),
                cross(p[2], p[0], point),
                cross(p[0], p[1], point),
            ];
            if edges
                .iter()
                .zip(edge_top)
                .any(|(e, top)| *e < 0. || (*e == 0. && !top))
            {
                continue;
            }
            let w = edges.map(|e| e / area);
            let uv = vertices[0].uv.to_vec2() * w[0]
                + vertices[1].uv.to_vec2() * w[1]
                + vertices[2].uv.to_vec2() * w[2];
            let sample = sample_texture(texture, uv);
            let color: [f32; 4] = std::array::from_fn(|c| {
                (f32::from(vertices[0].color[c]) * w[0]
                    + f32::from(vertices[1].color[c]) * w[1]
                    + f32::from(vertices[2].color[c]) * w[2])
                    * sample[c]
                    / 255.
            });
            let offset = (y * size[0] + x) * 3;
            for c in 0..3 {
                raster[offset + c] = (color[c]
                    + f32::from(raster[offset + c]) * (1. - color[3] / 255.))
                    .clamp(0., 255.)
                    .round() as u8;
            }
        }
    }
}

fn sample_texture(texture: &egui::ColorImage, uv: Vec2) -> [f32; 4] {
    let x = (uv.x * texture.size[0] as f32 - 0.5).clamp(0., (texture.size[0] - 1) as f32);
    let y = (uv.y * texture.size[1] as f32 - 0.5).clamp(0., (texture.size[1] - 1) as f32);
    let (x0, y0) = (x.floor() as usize, y.floor() as usize);
    let (x1, y1) = (
        (x0 + 1).min(texture.size[0] - 1),
        (y0 + 1).min(texture.size[1] - 1),
    );
    let (fx, fy) = (x - x0 as f32, y - y0 as f32);
    std::array::from_fn(|c| {
        let a = f32::from(texture.pixels[y0 * texture.size[0] + x0][c]) * (1. - fx)
            + f32::from(texture.pixels[y0 * texture.size[0] + x1][c]) * fx;
        let b = f32::from(texture.pixels[y1 * texture.size[0] + x0][c]) * (1. - fx)
            + f32::from(texture.pixels[y1 * texture.size[0] + x1][c]) * fx;
        a * (1. - fy) + b * fy
    })
}

/// Write paths and searchable, embedded-font text with physical MediaBox sizes.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn write_pdf(path: &Path, pages: &[RasterPage]) -> Result<(), String> {
    if pages.is_empty() || pages.len() > MAX_PAGES {
        return Err("인쇄할 페이지가 없습니다.".into());
    }
    let pdf = vector_pdf::encode(pages)?;
    // Atomic replace also prevents an interrupted export leaving a valid-looking
    // but truncated PDF. A same-directory staging file works on Windows/Linux.
    static NEXT_PDF: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let sequence = NEXT_PDF.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let staging = path.with_extension(format!("pdf-{}-{sequence}.tmp", std::process::id()));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&staging)
        .map_err(|e| format!("PDF 임시 파일 생성 실패: {e}"))?;
    let result = (|| {
        file.write_all(&pdf)
            .and_then(|_| file.sync_all())
            .map_err(|e| format!("PDF 저장 실패: {e}"))?;
        drop(file);
        fs::rename(&staging, path).map_err(|e| format!("PDF 저장 실패: {e}"))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&staging);
    }
    result
}

#[derive(Clone, Debug)]
#[cfg(not(target_arch = "wasm32"))]
pub(crate) struct NativePrintOptions {
    pub job_name: String,
    pub printer: Option<String>,
    pub copies: u16,
}

/// Read printer names without showing the operating system's print dialog.
/// The caller can run this on its print worker so a disconnected print server
/// cannot block worksheet interaction.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn list_printers() -> Result<Vec<String>, String> {
    #[cfg(windows)]
    let mut command = {
        use std::os::windows::process::CommandExt;
        let mut command = Command::new("powershell.exe");
        command.args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command", "[Console]::OutputEncoding=[System.Text.UTF8Encoding]::new($false); Add-Type -AssemblyName System.Drawing; ConvertTo-Json -InputObject @([System.Drawing.Printing.PrinterSettings]::InstalledPrinters) -Compress"]);
        command.creation_flags(0x08000000);
        command
    };
    #[cfg(not(windows))]
    let mut command = {
        let mut command = Command::new("lpstat");
        command.arg("-a");
        command
    };
    let output = bounded_output(&mut command)?;
    if !output.status.success() {
        return Err(format!(
            "프린터 목록을 읽을 수 없습니다: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    #[cfg(windows)]
    let names: Vec<String> = serde_json::from_slice::<Vec<String>>(&output.stdout)
        .map_err(|e| format!("Windows 프린터 목록 응답 오류: {e}"))?;
    #[cfg(not(windows))]
    let names: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.split_whitespace().next().map(str::to_owned))
        .collect();
    let mut names: Vec<_> = names
        .into_iter()
        .filter(|name| {
            !name.is_empty() && name.len() <= 1024 && !name.chars().any(char::is_control)
        })
        .take(1024)
        .collect();
    names.sort();
    names.dedup();
    Ok(names)
}

#[cfg(not(target_arch = "wasm32"))]
fn bounded_output(command: &mut Command) -> Result<std::process::Output, String> {
    use std::{
        process::Stdio,
        time::{Duration, Instant},
    };
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("프린터 목록 명령 실행 실패: {e}"))?;
    let started = Instant::now();
    loop {
        if child.try_wait().map_err(|e| e.to_string())?.is_some() {
            return child.wait_with_output().map_err(|e| e.to_string());
        }
        if started.elapsed() >= Duration::from_secs(10) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("프린터 목록 응답 시간이 초과되었습니다.".into());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// Spool only when explicitly invoked by the application's Print command. The
/// tests never call this function or start a printer, process or dialog.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn submit_native(
    pages: &[RasterPage],
    options: &NativePrintOptions,
) -> Result<(), String> {
    if pages.is_empty()
        || !(1..=999).contains(&options.copies)
        || options.job_name.len() > 1024
        || options
            .printer
            .as_ref()
            .is_some_and(|p| p.is_empty() || p.len() > 1024 || p.chars().any(char::is_control))
    {
        return Err("인쇄 작업 설정이 올바르지 않습니다.".into());
    }
    let directory = JobDirectory::create()?;
    submit_job(&directory.0, pages, options)
}

#[cfg(not(target_arch = "wasm32"))]
struct JobDirectory(std::path::PathBuf);
#[cfg(not(target_arch = "wasm32"))]
impl JobDirectory {
    fn create() -> Result<Self, String> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        for _ in 0..20 {
            let path = std::env::temp_dir().join(format!(
                "mathfolio-print-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(format!("인쇄 임시 폴더 생성 실패: {e}")),
            }
        }
        Err("인쇄 임시 폴더를 만들 수 없습니다.".into())
    }
}
#[cfg(not(target_arch = "wasm32"))]
impl Drop for JobDirectory {
    fn drop(&mut self) {
        // Delete only files this process created in its exclusive job folder;
        // never recursively follow links or remove a user-supplied path.
        if let Ok(files) = fs::read_dir(&self.0) {
            for file in files.flatten() {
                let _ = fs::remove_file(file.path());
            }
        }
        let _ = fs::remove_dir(&self.0);
    }
}

#[cfg(windows)]
#[cfg(not(target_arch = "wasm32"))]
fn submit_job(
    directory: &Path,
    pages: &[RasterPage],
    options: &NativePrintOptions,
) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    if pages.iter().any(|p| p.png.is_empty()) {
        return Err("Windows 네이티브 인쇄에는 래스터 페이지가 필요합니다.".into());
    }
    let paths: Vec<_> = pages
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let path = directory.join(format!("page-{i:04}.png"));
            fs::write(&path, &p.png).map(|_| path.to_string_lossy().into_owned())
        })
        .collect::<Result<_, _>>()
        .map_err(|e| format!("인쇄 페이지 저장 실패: {e}"))?;
    let data = serde_json::json!({"pages":paths,"name":options.job_name,"printer":options.printer,"copies":options.copies,"paper":pages[0].paper_size});
    let input = directory.join("job.json");
    fs::write(&input, data.to_string()).map_err(|e| e.to_string())?;
    let script = directory.join("print.ps1");
    fs::write(&script, WINDOWS_PRINT_SCRIPT).map_err(|e| e.to_string())?;
    let output = Command::new("powershell.exe")
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ])
        .arg(script)
        .arg(input)
        .creation_flags(0x08000000)
        .output()
        .map_err(|e| format!("Windows 인쇄 기능 실행 실패: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "Windows 인쇄 실패: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

#[cfg(windows)]
const WINDOWS_PRINT_SCRIPT: &str = r#"param([string]$JobFile)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$job = Get-Content -LiteralPath $JobFile -Raw -Encoding UTF8 | ConvertFrom-Json
$doc = New-Object System.Drawing.Printing.PrintDocument
try {
  $doc.PrintController = New-Object System.Drawing.Printing.StandardPrintController
  if ($job.printer) { $doc.PrinterSettings.PrinterName = $job.printer }
  if (-not $doc.PrinterSettings.IsValid) { throw 'No usable printer is configured.' }
  if ($doc.PrinterSettings.PrinterName -match 'Microsoft Print to PDF|Microsoft XPS Document Writer|OneNote') { throw 'Select a physical printer; use PDF export for file output.' }
  $doc.DocumentName = [string]$job.name
  $doc.PrinterSettings.Copies = [int16]$job.copies
  $doc.DefaultPageSettings.Margins = New-Object System.Drawing.Printing.Margins(0,0,0,0)
  $w = [int][Math]::Round([double]$job.paper[0] * 100.0 / 96.0)
  $h = [int][Math]::Round([double]$job.paper[1] * 100.0 / 96.0)
  $doc.DefaultPageSettings.Landscape = $w -gt $h
  $doc.DefaultPageSettings.PaperSize = New-Object System.Drawing.Printing.PaperSize('Mathfolio',[Math]::Min($w,$h),[Math]::Max($w,$h))
  $script:pageIndex = 0
  $doc.add_PrintPage({ param($sender,$event)
    $bitmap = [System.Drawing.Image]::FromFile([string]$job.pages[$script:pageIndex])
    try {
      $event.Graphics.TranslateTransform(-$event.PageSettings.HardMarginX,-$event.PageSettings.HardMarginY)
      $event.Graphics.DrawImage($bitmap, [System.Drawing.RectangleF]::new(0,0,$w,$h))
    } finally { $bitmap.Dispose() }
    $script:pageIndex++
    $event.HasMorePages = $script:pageIndex -lt $job.pages.Count
  })
  $doc.Print()
} finally { $doc.Dispose() }
"#;

#[cfg(not(windows))]
#[cfg(not(target_arch = "wasm32"))]
fn submit_job(
    directory: &Path,
    pages: &[RasterPage],
    options: &NativePrintOptions,
) -> Result<(), String> {
    let path = directory.join("worksheet.pdf");
    write_pdf(&path, pages)?;
    let mut lp = Command::new("lp");
    lp.arg("-t")
        .arg(&options.job_name)
        .arg("-n")
        .arg(options.copies.to_string());
    for option in cups_page_options(pages[0].paper_size) {
        lp.arg("-o").arg(option);
    }
    if let Some(printer) = &options.printer {
        lp.arg("-d").arg(printer);
    }
    lp.arg("--").arg(&path);
    let result = match lp.output() {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let mut lpr = Command::new("lpr");
            lpr.arg("-J")
                .arg(&options.job_name)
                .arg(format!("-#{}", options.copies));
            for option in cups_page_options(pages[0].paper_size) {
                lpr.arg("-o").arg(option);
            }
            if let Some(printer) = &options.printer {
                lpr.arg("-P").arg(printer);
            }
            lpr.arg(&path).output()
        }
        result => result,
    }
    .map_err(|e| format!("CUPS 인쇄 명령(lp/lpr)을 실행할 수 없습니다: {e}"))?;
    if result.status.success() {
        Ok(())
    } else {
        Err(format!(
            "CUPS 인쇄 실패: {}",
            String::from_utf8_lossy(&result.stderr).trim()
        ))
    }
}

#[cfg(any(not(windows), test))]
#[cfg(not(target_arch = "wasm32"))]
fn cups_page_options(paper_size: [f32; 2]) -> [String; 3] {
    let [width, height] = paper_size;
    let [short, long] = [width.min(height), width.max(height)];
    let media = if (short - 21. * 96. / 2.54).abs() < 0.2 && (long - 29.7 * 96. / 2.54).abs() < 0.2
    {
        "A4".to_owned()
    } else if (short - 816.).abs() < 0.2 && (long - 1056.).abs() < 0.2 {
        "Letter".to_owned()
    } else {
        format!(
            "Custom.{:.3}x{:.3}mm",
            short * 25.4 / 96.,
            long * 25.4 / 96.
        )
    };
    [
        format!("media={media}"),
        format!(
            "orientation-requested={}",
            if width > height { 4 } else { 3 }
        ),
        "print-scaling=none".into(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    fn settings() -> PrintSettings {
        PrintSettings {
            paper_size: [96., 192.],
            margins: [10.; 4],
            body_height: 170.,
            content_bottom: 341.,
            pages: PageSelection::All,
            dpi: 96,
        }
    }
    #[test]
    fn pagination_respects_grid_body_height_and_original_range_indices() {
        let mut s = settings();
        let plan = PrintPlan::new(&s).unwrap();
        assert_eq!(plan.page_count(), 3);
        assert_eq!(plan.source_pages(), &[0, 1, 2]);
        s.pages = PageSelection::Range { first: 2, last: 3 };
        assert_eq!(PrintPlan::new(&s).unwrap().source_pages(), &[1, 2]);
        for (first, last) in [(0, 1), (3, 2), (1, 4)] {
            s.pages = PageSelection::Range { first, last };
            assert!(PrintPlan::new(&s).is_err());
        }
        s.pages = PageSelection::All;
        s.content_bottom = 340.;
        assert_eq!(PrintPlan::new(&s).unwrap().page_count(), 2);
        s.content_bottom = 0.;
        assert_eq!(PrintPlan::new(&s).unwrap().page_count(), 1);
        s.content_bottom = 341.;
        s.pages = PageSelection::Indices(vec![0, 2]);
        assert_eq!(PrintPlan::new(&s).unwrap().source_pages(), &[0, 2]);
        for indices in [vec![], vec![2, 0], vec![1, 1], vec![3]] {
            s.pages = PageSelection::Indices(indices);
            assert!(PrintPlan::new(&s).is_err());
        }
    }
    #[test]
    fn unbounded_or_nonfinite_print_jobs_are_rejected_before_allocating() {
        for bad in [f64::NAN, f64::INFINITY, -1., 200_000.] {
            let mut s = settings();
            s.content_bottom = bad;
            assert!(PrintPlan::new(&s).is_err());
        }
        let mut s = settings();
        s.paper_size = [100_000., 100_000.];
        assert!(PrintPlan::new(&s).is_err());
        let mut s = settings();
        s.margins = [96.; 4];
        assert!(PrintPlan::new(&s).is_err());
    }
    #[test]
    fn software_painter_clips_and_blends_shared_triangle_edges_once() {
        let texture = egui::ColorImage::filled([1, 1], Color32::WHITE);
        let textures = HashMap::from([(TextureId::Managed(0), texture)]);
        let mut mesh = egui::epaint::Mesh::default();
        mesh.add_colored_rect(
            Rect::from_min_max(Pos2::ZERO, Pos2::new(4., 4.)),
            Color32::from_rgba_premultiplied(128, 0, 0, 128),
        );
        let primitive = egui::epaint::ClippedPrimitive {
            clip_rect: Rect::from_min_max(Pos2::ZERO, Pos2::new(3., 3.)),
            primitive: egui::epaint::Primitive::Mesh(mesh),
        };
        let rgb = paint_meshes([4, 4], 1., &[primitive], &textures).unwrap();
        for y in 0..4 {
            for x in 0..4 {
                let color = &rgb[(y * 4 + x) * 3..(y * 4 + x) * 3 + 3];
                assert_eq!(
                    color,
                    if x < 3 && y < 3 {
                        &[255, 127, 127]
                    } else {
                        &[255, 255, 255]
                    }
                );
            }
        }
    }
    #[test]
    fn cups_media_and_orientation_preserve_document_paper_without_fit_to_page() {
        let a4 = cups_page_options([21. * 96. / 2.54, 29.7 * 96. / 2.54]);
        assert_eq!(
            a4,
            ["media=A4", "orientation-requested=3", "print-scaling=none"]
        );
        assert_eq!(
            cups_page_options([1056., 816.])[0..2],
            ["media=Letter", "orientation-requested=4"]
        );
        assert_eq!(
            cups_page_options([960., 960.])[0],
            "media=Custom.254.000x254.000mm"
        );
    }
    #[test]
    fn headless_fonts_page_range_and_pdf_have_real_page_pixels_and_media_boxes() {
        let mut s = settings();
        s.pages = PageSelection::Range { first: 2, last: 3 };
        let plan = PrintPlan::new(&s).unwrap();
        let mut seen = Vec::new();
        let pages = render_pages(&plan, |ui, page, total| {
            seen.push((page, total));
            ui.painter().text(
                Pos2::new(10., 20.),
                egui::Align2::LEFT_TOP,
                "α = 3.5 Ω",
                egui::FontId::proportional(13.),
                Color32::BLACK,
            );
            ui.painter().rect_filled(
                Rect::from_min_size(Pos2::new(10., 50.), Vec2::splat(10.)),
                0.,
                Color32::RED,
            );
        })
        .unwrap();
        assert_eq!(pages.len(), 2);
        assert!(seen.contains(&(1, 3)) && seen.contains(&(2, 3)));
        let image = image::load_from_memory(&pages[0].png).unwrap().to_rgb8();
        assert_eq!(image.dimensions(), (96, 192));
        assert_eq!(image.get_pixel(15, 55).0, [255, 0, 0]);
        assert!(
            image
                .enumerate_pixels()
                .any(|(_, y, p)| y < 45 && p.0.iter().any(|c| *c < 100))
        );
        let dir = JobDirectory::create().unwrap();
        let path = dir.0.join("test.pdf");
        write_pdf(&path, &pages).unwrap();
        let pdf = fs::read(path).unwrap();
        let source = String::from_utf8_lossy(&pdf);
        assert!(source.contains("/Count 2"));
        assert_eq!(
            source.matches("/MediaBox [0 0 72.00000 144.00000]").count(),
            2
        );
        assert!(source.contains("/ToUnicode"));
        assert!(source.contains("/FontFile2"));
        assert!(!source.contains("/Subtype /Image"));
        assert!(source.ends_with("%%EOF\n"));
    }

    #[test]
    fn dpi_changes_raster_density_without_changing_worksheet_coordinates() {
        let mut s = settings();
        s.content_bottom = 0.;
        s.dpi = 192;
        let plan = PrintPlan::new(&s).unwrap();
        let pages = render_pages(&plan, |ui, _, _| {
            ui.painter().rect_filled(
                Rect::from_min_size(Pos2::new(10., 50.), Vec2::splat(10.)),
                0.,
                Color32::RED,
            );
        })
        .unwrap();
        let image = image::load_from_memory(&pages[0].png).unwrap().to_rgb8();
        assert_eq!(image.dimensions(), (192, 384));
        assert_eq!(image.get_pixel(30, 110).0, [255, 0, 0]);
        assert_eq!(image.get_pixel(15, 55).0, [255, 255, 255]);
        assert_eq!(pages[0].paper_size, [96., 192.]);
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn pdf_bytes(pages: &[RasterPage]) -> Result<Vec<u8>, String> {
    vector_pdf::encode(pages)
}
