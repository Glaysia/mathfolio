//! Inert native text/paint presentation. This never loads WPF or executes XAML.
#[cfg(test)]
#[path = "native_visual/refresh_tests.rs"]
mod rich_refresh_tests;
use crate::{
    canvas::{CanvasContent, CanvasRegion},
    native::{NativePackage, xml_text},
};
use mathfolio_editor::{NodeKind, PortableTree};
use roxmltree::{Document, Node};
use std::{
    collections::{HashMap, HashSet},
    io::{Cursor, Read},
};
use zip::ZipArchive;

const WS: &str = "http://schemas.mathsoft.com/worksheet50";
const XAML: &str = "http://schemas.microsoft.com/winfx/2006/xaml/presentation";
const REL: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const FLOW_REL: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/flowDocument";
const MAX_XML: usize = 8 * 1024 * 1024;
const MAX_CELLS: usize = 16_384;
pub type Rgba = [u8; 4];

#[derive(Clone, Debug, Default)]
pub struct NativeVisuals {
    pub regions: HashMap<String, Result<RegionVisual, String>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RegionVisual {
    pub size: [f32; 2],
    pub background: Option<Rgba>,
    pub document: Option<RichDocument>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RichDocument {
    pub blocks: Vec<Block>,
    pub style: TextStyle,
    pub imported_plain_text: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Block {
    Paragraph(Paragraph),
    Table(Table),
    Math(MathBlock),
}
#[derive(Clone, Debug, PartialEq)]
pub struct MathBlock {
    pub tree: PortableTree,
    pub font_points: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextStyle {
    pub font_family: String,
    /// XAML text FontSize is already DIP, unlike math formattingOverride.
    pub font_size: f32,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub foreground: Rgba,
    pub background: Option<Rgba>,
    pub alignment: Alignment,
}
impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font_family: "Tahoma".into(),
            font_size: 44. / 3.,
            bold: false,
            italic: false,
            underline: false,
            foreground: [0, 0, 0, 255],
            background: None,
            alignment: Alignment::Left,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Alignment {
    Left,
    Center,
    Right,
    Justify,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    pub text: String,
    pub style: TextStyle,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Paragraph {
    pub runs: Vec<Run>,
    pub style: TextStyle,
    pub margin: [f32; 4],
    pub line_height: Option<f32>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Column {
    pub width: Option<f32>,
    pub weight: f32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Table {
    pub columns: Vec<Column>,
    pub rows: usize,
    pub cells: Vec<Cell>,
    pub spacing: f32,
    pub style: TextStyle,
    pub row_min_height: f32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Cell {
    pub row: usize,
    pub column: usize,
    pub row_span: usize,
    pub column_span: usize,
    pub blocks: Vec<Block>,
    pub padding: [f32; 4],
    pub border: [f32; 4],
    pub border_color: Rgba,
    pub style: TextStyle,
}

impl RichDocument {
    pub fn plain_text(&self) -> String {
        fn blocks_text(blocks: &[Block]) -> String {
            blocks
                .iter()
                .map(|block| match block {
                    Block::Paragraph(p) => {
                        p.runs.iter().map(|r| r.text.as_str()).collect::<String>()
                    }
                    Block::Table(t) => (0..t.rows)
                        .map(|row| {
                            t.cells
                                .iter()
                                .filter(|cell| cell.row == row)
                                .map(|cell| blocks_text(&cell.blocks))
                                .collect::<Vec<_>>()
                                .join("\t")
                        })
                        .collect::<Vec<_>>()
                        .join("\n"),
                    Block::Math(_) => String::new(),
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
        blocks_text(&self.blocks)
    }
    pub fn table_cell_count(&self) -> usize {
        fn count(blocks: &[Block]) -> usize {
            blocks
                .iter()
                .map(|b| match b {
                    Block::Paragraph(_) => 0,
                    Block::Math(_) => 0,
                    Block::Table(t) => {
                        t.cells.len() + t.cells.iter().map(|c| count(&c.blocks)).sum::<usize>()
                    }
                })
                .sum()
        }
        count(&self.blocks)
    }
}

/// Area-local presentation keys are JSON element-child paths relative to the
/// owning worksheet region, matching `area_math_children_xml` on that fragment.
/// Source region IDs never participate, so even repeated IDs cannot alias.
pub type AreaChildVisuals = HashMap<String, Result<RegionVisual, String>>;

/// Read direct Area child presentation once during import. This is separate
/// from the root-only `NativeVisuals::from_package` path and never runs XAML,
/// formulas, package callbacks or external relationships.
#[derive(Clone, Debug, PartialEq)]
pub struct AreaChildVisualEntry {
    /// Element-child path relative to the owning worksheet region.
    pub region_path: Vec<usize>,
    pub key: String,
    pub left: f64,
    pub top: f64,
    pub kind: String,
    pub visual: Result<RegionVisual, String>,
}

pub fn area_child_visuals(
    package: &NativePackage,
    area_region_path: &[usize],
) -> Result<AreaChildVisuals, String> {
    Ok(area_child_visual_entries(package, area_region_path)?
        .into_iter()
        .map(|entry| (entry.key, entry.visual))
        .collect())
}

/// Read the same visuals with source-local placement and kind metadata, for an
/// Area-local canvas which does not duplicate the children at worksheet level.
pub fn area_child_visual_entries(
    package: &NativePackage,
    area_region_path: &[usize],
) -> Result<Vec<AreaChildVisualEntry>, String> {
    let area = package
        .areas()?
        .into_iter()
        .find(|area| area.region_path == area_region_path)
        .ok_or("Path does not identify a native Area region")?;
    let source = package.worksheet_xml()?;
    let doc = parse(&source)?;
    let mut out = Vec::new();
    let mut keys = HashSet::new();
    for child_path in area.child_region_paths {
        let relative = child_path
            .strip_prefix(area_region_path)
            .ok_or("Native Area child path escaped its owner")?;
        let key = serde_json::to_string(relative).map_err(|e| e.to_string())?;
        let region_path = relative.to_vec();
        let mut region = doc.root_element();
        for index in child_path {
            region = region
                .children()
                .filter(Node::is_element)
                .nth(index)
                .ok_or("Native Area child path no longer exists")?;
        }
        if !region.has_tag_name((WS, "region")) {
            return Err("Native Area child path is not a region".into());
        }
        let coordinate = |name| -> Result<f64, String> {
            let value: f64 = region
                .attribute(name)
                .ok_or_else(|| format!("Missing Area child {name}"))?
                .parse()
                .map_err(|_| format!("Invalid Area child {name}"))?;
            if !value.is_finite() {
                return Err(format!("Nonfinite Area child {name}"));
            }
            Ok(value)
        };
        let left = coordinate("left")?;
        let top = coordinate("top")?;
        let content = region.children().find(Node::is_element);
        let kind = content.map_or("empty", |n| n.tag_name().name()).to_owned();
        let visual = match content {
            Some(content)
                if content.has_tag_name((WS, "math"))
                    || content.has_tag_name((WS, "text"))
                    || content.has_tag_name((WS, "spec-table")) =>
            {
                read_region(package, region, content)
            }
            Some(content) => Err(format!(
                "Unsupported native Area child presentation: {}",
                content.tag_name().name()
            )),
            None => Err("Native Area child has no content".into()),
        };
        if !keys.insert(key.clone()) {
            return Err("Duplicate native Area child path".into());
        }
        out.push(AreaChildVisualEntry {
            region_path,
            key,
            left,
            top,
            kind,
            visual,
        });
    }
    Ok(out)
}

fn rich_overlay_prefix(id: u64) -> String {
    format!("\0rich:{id}:")
}
fn rich_overlay_key(region: &CanvasRegion) -> Option<String> {
    if region.rich_text.is_none()
        && region.source_region.is_none()
        && let Some(wire) = region.authored_text.as_deref()
    {
        use std::fmt::Write;
        let mut key = rich_overlay_prefix(region.id);
        key.push_str("authored:");
        for byte in wire.cached_visual_fingerprint()? {
            write!(key, "{byte:02x}").expect("writing an authored source key cannot fail");
        }
        return Some(key);
    }
    let wire = region.rich_text.as_ref()?;
    let digest = wire.visual_source_fingerprint();
    use std::fmt::Write;
    let mut key = rich_overlay_prefix(region.id);
    key.reserve(64);
    for byte in digest {
        write!(key, "{byte:02x}").expect("writing a source key to String cannot fail");
    }
    Some(key)
}
impl NativeVisuals {
    pub fn remove_rich_text_overlay(&mut self, id: u64) {
        let prefix = rich_overlay_prefix(id);
        self.regions.retain(|key, _| !key.starts_with(&prefix));
    }
    /// Validated authored source uses its own fingerprint; no old native owner
    /// or same-plain cache can supply its style. Called on source publication.
    pub fn update_authored_text(
        &mut self,
        region: &CanvasRegion,
        source: &crate::rich_text_source::AuthoredRichText,
    ) -> Result<(), String> {
        let CanvasContent::Text(plain) = &region.content else {
            return Err("Expected authored Text region".into());
        };
        if region.source_region.is_some()
            || region.rich_text.is_some()
            || region.authored_text.as_deref() != Some(source.wire())
            || source.plain_text() != plain
        {
            return Err("Authored visual source/carrier mismatch".into());
        }
        let document = read_flow_xml(source.xaml(), TextStyle::default())?;
        if document.imported_plain_text != *plain {
            return Err("Authored visual/plain mismatch".into());
        }
        let key = rich_overlay_key(region).ok_or("Missing validated authored visual key")?;
        let visual = RegionVisual {
            size: [0.; 2],
            background: document.style.background,
            document: Some(document),
        };
        self.remove_rich_text_overlay(region.id);
        self.regions.insert(key, Ok(visual));
        Ok(())
    }
    /// Call once after committed source change/Open/Undo/Redo, not each frame.
    /// Original imported entries are never overwritten. None restores them.
    pub fn update_rich_text(
        &mut self,
        region: &CanvasRegion,
        source: &crate::rich_text_source::ValidatedRichText,
    ) -> Result<(), String> {
        self.remove_rich_text_overlay(region.id);
        let CanvasContent::Text(plain) = &region.content else {
            return Err("Expected text region".into());
        };
        let wire = region
            .rich_text
            .as_deref()
            .ok_or("Rich text carrier missing")?;
        if wire != source.wire() {
            return Err("Rich text validated source/carrier mismatch".into());
        }
        source.validate_plain(plain)?;
        let owner = &wire.origin().identity().owner_region_id;
        if region.source_region.as_ref().is_some_and(|id| id != owner) {
            return Err("Rich text source owner mismatch".into());
        }
        let original = self
            .regions
            .get(owner)
            .ok_or("Original native text visual missing")?
            .as_ref()
            .map_err(Clone::clone)?
            .clone();
        let base = original
            .document
            .as_ref()
            .ok_or("Original text style missing")?;
        let mut document = read_flow_xml(source.xaml(), base.style.clone())?;
        if document.imported_plain_text != *plain {
            return Err("Current rich visual/plain mismatch".into());
        }
        document.style.background = original.background;
        let visual = RegionVisual {
            size: original.size,
            background: original.background,
            document: Some(document),
        };
        if region.source_region.is_none() {
            self.regions
                .insert(format!("\0canvas:{}", region.id), Ok(original.clone()));
        }
        self.regions.insert(
            rich_overlay_key(region).ok_or("Rich visual key missing")?,
            Ok(visual),
        );
        Ok(())
    }

    /// Local tables can be regenerated from their canvas XML during undo.
    /// Keep imported entries, whose untouched rich parts still belong to the
    /// preserved archive, but release removed local tables immediately.
    pub fn remove_local_region(&mut self, id: u64) {
        self.regions.remove(&format!("\0canvas:{id}"));
        self.remove_rich_text_overlay(id);
    }

    /// Replace a cached table from committed native XML, never the stale archive.
    pub fn update_spec_table(&mut self, region: &CanvasRegion) -> Result<(), String> {
        let CanvasContent::Opaque { xml, kind } = &region.content else {
            return Err("Expected an opaque native specification table".into());
        };
        if kind != "spec-table" {
            return Err("Expected a native specification table".into());
        }
        // XML attributes cannot contain NUL, so this local key cannot collide
        // with any preserved native source ID even in a malformed package.
        let id = region
            .source_region
            .clone()
            .unwrap_or_else(|| format!("\0canvas:{}", region.id));
        let result = parse_table_visual(xml, region.format);
        let status = result.as_ref().map(|_| ()).map_err(Clone::clone);
        self.regions.insert(id, result);
        status
    }
    pub fn from_package(package: &NativePackage) -> Result<Self, String> {
        let source = package.worksheet_xml()?;
        let doc = parse(&source)?;
        let regions = doc
            .root_element()
            .children()
            .find(|n| n.has_tag_name((WS, "regions")))
            .ok_or("Native worksheet regions missing")?;
        let mut out = Self::default();
        for region in regions
            .children()
            .filter(|n| n.has_tag_name((WS, "region")))
        {
            let id = region
                .attribute("region-id")
                .ok_or("Native visual region ID missing")?
                .to_string();
            if out.regions.contains_key(&id) {
                return Err("Duplicate native visual region ID".into());
            }
            let Some(content) = region.children().find(Node::is_element) else {
                continue;
            };
            if content.has_tag_name((WS, "math"))
                || content.has_tag_name((WS, "text"))
                || content.has_tag_name((WS, "spec-table"))
            {
                out.regions
                    .insert(id, read_region(package, region, content));
            }
        }
        Ok(out)
    }
    pub fn region(&self, region: &CanvasRegion) -> Option<&Result<RegionVisual, String>> {
        // A live carrier must never fall through to an old same-plain visual.
        if region.rich_text.is_some() || region.authored_text.is_some() {
            let CanvasContent::Text(_) = &region.content else {
                return None;
            };
            if region.rich_text.is_some() && region.authored_text.is_some() {
                return None;
            }
            if let Some(wire) = &region.rich_text
                && region
                    .source_region
                    .as_ref()
                    .is_some_and(|id| id != &wire.origin().identity().owner_region_id)
            {
                return None;
            }
            if region.authored_text.is_some() && region.source_region.is_some() {
                return None;
            }
            let result = self.regions.get(&rich_overlay_key(region)?)?;
            if let (Ok(visual), CanvasContent::Text(plain)) = (result, &region.content)
                && visual
                    .document
                    .as_ref()
                    .is_some_and(|d| d.imported_plain_text != *plain)
            {
                return None;
            }
            return Some(result);
        }
        let local;
        let id = if let Some(id) = &region.source_region {
            id.as_str()
        } else {
            local = format!("\0canvas:{}", region.id);
            &local
        };
        let result = self.regions.get(id)?;
        if let (Ok(visual), CanvasContent::Text(current)) = (result, &region.content)
            && let Some(document) = &visual.document
            && current != &document.imported_plain_text
        {
            return None;
        }
        Some(result)
    }
}

fn parse(xml: &str) -> Result<Document<'_>, String> {
    if xml.len() > MAX_XML {
        return Err("Native visual XML exceeds 8 MiB".into());
    }
    Document::parse_with_options(
        xml,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: 100_000,
            ..Default::default()
        },
    )
    .map_err(|e| e.to_string())
}
fn size_attribute(n: Node<'_, '_>, name: &str) -> Result<Option<f32>, String> {
    n.attribute(name)
        .map(|v| {
            v.parse::<f32>()
                .ok()
                .filter(|v| v.is_finite() && *v >= 0. && *v <= 1_000_000.)
                .ok_or_else(|| format!("Invalid native visual {name}"))
        })
        .transpose()
}
fn read_region(
    package: &NativePackage,
    region: Node<'_, '_>,
    content: Node<'_, '_>,
) -> Result<RegionVisual, String> {
    let size = [
        size_attribute(region, "width")?
            .or(size_attribute(region, "actualWidth")?)
            .unwrap_or(0.),
        size_attribute(region, "actualHeight")?
            .or(size_attribute(region, "height")?)
            .unwrap_or(0.),
    ];
    if content.has_tag_name((WS, "spec-table")) {
        return read_spec_table(package, content, size);
    }
    if content.has_tag_name((WS, "math")) {
        let format = content
            .children()
            .find(|n| n.has_tag_name((WS, "formattingOverride")));
        return Ok(RegionVisual {
            size,
            background: format
                .and_then(|n| n.attribute("BackgroundColor"))
                .map(parse_color)
                .transpose()?,
            document: None,
        });
    }
    if content
        .children()
        .any(|n| n.tag_name().namespace() == Some(WS))
    {
        return Err("Embedded worksheet regions in native text are not yet supported".into());
    }
    let inline = content
        .children()
        .find(|n| n.has_tag_name((XAML, "FlowDocument")));
    let mut style = TextStyle::default();
    if let Some(inline) = inline {
        style = inherit_style(inline, &style)?
    }
    let mut document = if let Some(reference) = content.attribute("item-idref") {
        let target =
            resolve_relationship(package, package.worksheet_part_name(), reference, FLOW_REL)?;
        let bytes = package
            .original_part(&target)
            .ok_or("Native FlowDocument relationship target is missing")?;
        read_flow_package(bytes, style)?
    } else if let Some(inline) = inline {
        let blocks = parse_blocks(inline, &style, 0, &mut 0)?;
        RichDocument {
            blocks,
            style,
            imported_plain_text: legacy_plain_text(inline),
        }
    } else {
        return Err("Native text has neither a FlowDocument nor a relationship".into());
    };
    if document.blocks.is_empty() {
        return Err("Native FlowDocument has no supported text or table content".into());
    }
    // The worksheet's FlowDocument stores the region-wide background while the
    // packaged Section may additionally store backgrounds on individual runs.
    let background = inline
        .and_then(|n| n.attribute("Background"))
        .map(parse_color)
        .transpose()?
        .or(document.style.background);
    document.style.background = background;
    Ok(RegionVisual {
        size,
        background,
        document: Some(document),
    })
}

fn read_spec_table(
    package: &NativePackage,
    node: Node<'_, '_>,
    size: [f32; 2],
) -> Result<RegionVisual, String> {
    let defaults = crate::formatting::read_worksheet_math_format_defaults(package)?;
    read_spec_table_with_defaults(node, size, defaults)
}

/// Rebuild one edited table without loading or executing the source worksheet.
pub fn parse_table_visual(
    xml: &str,
    defaults: crate::canvas::MathFormat,
) -> Result<RegionVisual, String> {
    let doc = parse(xml)?;
    let region = doc
        .descendants()
        .find(|n| n.has_tag_name((WS, "region")))
        .ok_or("Native table region wrapper missing")?;
    let table = region
        .children()
        .find(|n| n.has_tag_name((WS, "spec-table")))
        .ok_or("Native specification table missing")?;
    let size = [
        size_attribute(region, "width")?
            .or(size_attribute(region, "actualWidth")?)
            .unwrap_or(0.),
        size_attribute(region, "actualHeight")?
            .or(size_attribute(region, "height")?)
            .unwrap_or(0.),
    ];
    read_spec_table_with_defaults(table, size, defaults)
}

fn read_spec_table_with_defaults(
    node: Node<'_, '_>,
    size: [f32; 2],
    defaults: crate::canvas::MathFormat,
) -> Result<RegionVisual, String> {
    let source = node.document().input_text();
    let model = crate::spec_table::SpecTable::parse(&crate::spec_table::standalone(node, source)?)?;
    let mut parsed = Vec::new();
    let rows = model.row_count();
    for (column, math) in model
        .columns()
        .iter()
        .zip(node.children().filter(|n| n.has_tag_name((WS, "math"))))
    {
        let format = crate::formatting::read_math_format_with_defaults(math, defaults);
        parsed.push((
            column.header.tree.clone(),
            column.unit.as_ref().map(|u| u.tree.clone()),
            column.cells.clone(),
            format.font_points,
        ));
    }
    let mut style = TextStyle {
        font_family: "math-regular".into(),
        font_size: defaults.font_points * 96. / 72.,
        alignment: Alignment::Center,
        ..Default::default()
    };
    let mut cells = Vec::new();
    for (column, (header, unit, data, font_points)) in parsed.iter().enumerate() {
        let header_style = TextStyle {
            alignment: Alignment::Center,
            ..style.clone()
        };
        cells.push(Cell {
            row: 0,
            column,
            row_span: 1,
            column_span: 1,
            blocks: vec![Block::Math(MathBlock {
                tree: header.clone(),
                font_points: *font_points,
            })],
            padding: [2., 0., 2., 0.],
            border: [0.; 4],
            border_color: [0, 0, 0, 255],
            style: header_style.clone(),
        });
        let unit = unit.as_ref().map(|tree| {
            Block::Math(MathBlock {
                tree: PortableTree {
                    kind: NodeKind::Group,
                    children: vec![tree.clone()],
                },
                font_points: *font_points,
            })
        });
        cells.push(Cell {
            row: 1,
            column,
            row_span: 1,
            column_span: 1,
            blocks: unit.into_iter().collect(),
            padding: [2., 0., 2., 0.],
            border: [0., 0., 0., 0.8],
            border_color: [0, 0, 0, 255],
            style: header_style,
        });
        for row in 0..rows {
            let cell_style = TextStyle {
                font_size: *font_points * 96. / 72.,
                ..style.clone()
            };
            let blocks = match data.get(row).map(|cell| &cell.tree.kind) {
                None | Some(NodeKind::Placeholder) => vec![Block::Paragraph(Paragraph {
                    runs: vec![Run {
                        text: String::new(),
                        style: cell_style.clone(),
                    }],
                    style: cell_style.clone(),
                    margin: [0.; 4],
                    line_height: None,
                })],
                Some(NodeKind::Number(text)) => vec![Block::Paragraph(Paragraph {
                    runs: vec![Run {
                        text: text.clone(),
                        style: cell_style.clone(),
                    }],
                    style: cell_style.clone(),
                    margin: [0.; 4],
                    line_height: None,
                })],
                Some(_) => vec![Block::Math(MathBlock {
                    tree: data[row].tree.clone(),
                    font_points: *font_points,
                })],
            };
            cells.push(Cell {
                row: row + 2,
                column,
                row_span: 1,
                column_span: 1,
                blocks,
                padding: [2., 0., 2., 0.],
                border: [0.; 4],
                border_color: [0, 0, 0, 255],
                style: cell_style,
            });
        }
    }
    style.alignment = Alignment::Left;
    let table = Table {
        columns: vec![
            Column {
                width: None,
                // A retained native measurement constrains the whole table.
                // New/structurally edited tables have no measured width and
                // keep the natural widths of their cells.
                weight: if size[0] > 0. { 1. } else { 0. }
            };
            parsed.len()
        ],
        rows: rows + 2,
        cells,
        spacing: 0.,
        row_min_height: if size[1] > 0. {
            size[1] / (rows + 2) as f32
        } else {
            0.
        },
        style: style.clone(),
    };
    Ok(RegionVisual {
        size,
        background: None,
        document: Some(RichDocument {
            blocks: vec![Block::Table(table)],
            style,
            imported_plain_text: String::new(),
        }),
    })
}

pub fn resolve_relationship(
    package: &NativePackage,
    owner: &str,
    id: &str,
    expected_type: &str,
) -> Result<String, String> {
    let (directory, file) = owner
        .rsplit_once('/')
        .ok_or("Native owner part has no directory")?;
    let part = format!("{directory}/_rels/{file}.rels");
    let xml = package
        .original_xml_part(&part)?
        .ok_or("Native relationship part is missing")?;
    let doc = parse(&xml)?;
    let matches = doc
        .root_element()
        .children()
        .filter(|n| n.has_tag_name((REL, "Relationship")) && n.attribute("Id") == Some(id))
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err("Native visual relationship is missing or duplicated".into());
    }
    let rel = matches[0];
    if rel.attribute("TargetMode") == Some("External") {
        return Err("External native visual relationships are not supported".into());
    }
    if rel.attribute("Type") != Some(expected_type) {
        return Err("Native visual relationship has an unexpected type".into());
    }
    let target = rel
        .attribute("Target")
        .ok_or("Native visual relationship target is missing")?;
    if target.contains(['\\', '?', '#']) || target.contains("://") {
        return Err("Invalid native visual relationship target".into());
    }
    let joined = if target.starts_with('/') {
        target.trim_start_matches('/').into()
    } else {
        format!("{directory}/{target}")
    };
    let mut parts = Vec::new();
    for part in joined.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop().ok_or("Native visual target escapes package")?;
            }
            p => parts.push(p),
        }
    }
    if parts.is_empty() {
        return Err("Native visual target is empty".into());
    }
    Ok(parts.join("/"))
}

pub fn read_flow_package(bytes: &[u8], base: TextStyle) -> Result<RichDocument, String> {
    let mut zip = ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| format!("Invalid native text package: {e}"))?;
    if zip.len() > 256 {
        return Err("Native text package has too many parts".into());
    }
    let mut matches = 0;
    for n in 0..zip.len() {
        if zip
            .by_index(n)
            .map_err(|e| e.to_string())?
            .name()
            .map_err(|e| e.to_string())?
            == "Xaml/Document.xaml"
        {
            matches += 1
        }
    }
    if matches != 1 {
        return Err("Native text package has missing or duplicated Document.xaml".into());
    }
    let mut file = zip
        .by_name("Xaml/Document.xaml")
        .map_err(|e| e.to_string())?;
    if file.size() > MAX_XML as u64 {
        return Err("Native FlowDocument exceeds 8 MiB".into());
    }
    let mut data = Vec::new();
    file.by_ref()
        .take(MAX_XML as u64 + 1)
        .read_to_end(&mut data)
        .map_err(|e| e.to_string())?;
    let xml = xml_text(&data)?;
    read_flow_xml(&xml, base)
}

pub fn read_flow_xml(xml: &str, base: TextStyle) -> Result<RichDocument, String> {
    let doc = parse(xml)?;
    let root = doc.root_element();
    if root.tag_name().namespace() != Some(XAML)
        || !matches!(root.tag_name().name(), "FlowDocument" | "Section")
    {
        return Err("Native text root is not a FlowDocument/Section".into());
    }
    let style = inherit_style(root, &base)?;
    let mut cells = 0;
    Ok(RichDocument {
        blocks: parse_blocks(root, &style, 0, &mut cells)?,
        style,
        imported_plain_text: legacy_plain_text(root),
    })
}

fn legacy_plain_text(root: Node<'_, '_>) -> String {
    root.descendants()
        .filter(|n| n.has_tag_name((XAML, "Paragraph")))
        .map(|p| {
            p.descendants()
                .filter(Node::is_text)
                .filter_map(|n| n.text())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn parse_color(value: &str) -> Result<Rgba, String> {
    let h = value.strip_prefix('#').filter(|v| v.is_ascii());
    let hex =
        |v: &str| u8::from_str_radix(v, 16).map_err(|_| "Invalid native visual color".to_string());
    match h {
        Some(v) if v.len() == 8 => Ok([
            hex(&v[2..4])?,
            hex(&v[4..6])?,
            hex(&v[6..8])?,
            hex(&v[0..2])?,
        ]),
        Some(v) if v.len() == 6 => Ok([hex(&v[0..2])?, hex(&v[2..4])?, hex(&v[4..6])?, 255]),
        Some(v) if v.len() == 3 => Ok([
            hex(&v[0..1])? * 17,
            hex(&v[1..2])? * 17,
            hex(&v[2..3])? * 17,
            255,
        ]),
        Some(v) if v.len() == 4 => Ok([
            hex(&v[1..2])? * 17,
            hex(&v[2..3])? * 17,
            hex(&v[3..4])? * 17,
            hex(&v[0..1])? * 17,
        ]),
        _ => match value.to_ascii_lowercase().as_str() {
            "transparent" => Ok([0, 0, 0, 0]),
            "black" => Ok([0, 0, 0, 255]),
            "white" => Ok([255; 4]),
            "red" => Ok([255, 0, 0, 255]),
            "blue" => Ok([0, 0, 255, 255]),
            "yellow" => Ok([255, 255, 0, 255]),
            _ => Err("Unsupported native visual color".into()),
        },
    }
}
fn thickness(node: Node<'_, '_>, name: &str) -> Result<[f32; 4], String> {
    let Some(text) = node.attribute(name) else {
        return Ok([0.; 4]);
    };
    let v = text
        .split([',', ' '])
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.parse::<f32>()
                .map_err(|_| "Invalid native thickness".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    if v.iter().any(|v| !v.is_finite() || *v < 0. || *v > 4096.) {
        return Err("Native thickness exceeds limits".into());
    }
    match v.as_slice() {
        [all] => Ok([*all; 4]),
        [x, y] => Ok([*x, *y, *x, *y]),
        [a, b, c, d] => Ok([*a, *b, *c, *d]),
        _ => Err("Native thickness must have 1, 2 or 4 values".into()),
    }
}
fn inherit_style(n: Node<'_, '_>, parent: &TextStyle) -> Result<TextStyle, String> {
    let mut style = parent.clone();
    // A background paints its owning text element; do not repeatedly paint the
    // same inherited translucent brush at every descendant layer.
    style.background = None;
    if let Some(v) = n.attribute("FontFamily") {
        style.font_family = v.to_string()
    }
    if let Some(v) = size_attribute(n, "FontSize")? {
        if !(0.5..=4096.).contains(&v) {
            return Err("Native text font size exceeds limits".into());
        }
        style.font_size = v
    }
    if let Some(v) = n.attribute("FontWeight") {
        style.bold = matches!(
            v,
            "Bold"
                | "SemiBold"
                | "DemiBold"
                | "ExtraBold"
                | "Black"
                | "Heavy"
                | "700"
                | "800"
                | "900"
        )
    }
    if let Some(v) = n.attribute("FontStyle") {
        style.italic = matches!(v, "Italic" | "Oblique")
    }
    if let Some(v) = n.attribute("TextDecorations") {
        style.underline = v.split(',').any(|v| v.trim() == "Underline")
    }
    if let Some(v) = n.attribute("Foreground") {
        style.foreground = parse_color(v)?
    }
    if let Some(v) = n.attribute("Background") {
        style.background = Some(parse_color(v)?)
    }
    if let Some(v) = n.attribute("TextAlignment") {
        style.alignment = match v {
            "Left" => Alignment::Left,
            "Center" => Alignment::Center,
            "Right" => Alignment::Right,
            "Justify" => Alignment::Justify,
            _ => return Err("Unsupported native text alignment".into()),
        }
    }
    if n.attribute("FlowDirection") == Some("RightToLeft") {
        return Err("Native right-to-left text layout is not yet supported".into());
    }
    Ok(style)
}
fn parse_blocks(
    node: Node<'_, '_>,
    style: &TextStyle,
    depth: usize,
    cells: &mut usize,
) -> Result<Vec<Block>, String> {
    if depth > 32 {
        return Err("Native text nesting exceeds limits".into());
    }
    let mut blocks = Vec::new();
    for child in node.children().filter(Node::is_element) {
        if child.tag_name().namespace() != Some(XAML) {
            return Err("Unsupported native text object namespace".into());
        }
        let inherited = inherit_style(child, style)?;
        match child.tag_name().name() {
            "Section" => blocks.extend(parse_blocks(child, &inherited, depth + 1, cells)?),
            "Paragraph" => {
                let mut runs = Vec::new();
                parse_runs(child, &inherited, &mut runs, 0)?;
                let line_height = match child.attribute("LineHeight") {
                    None | Some("Auto") => None,
                    Some(_) => size_attribute(child, "LineHeight")?,
                };
                blocks.push(Block::Paragraph(Paragraph {
                    runs,
                    style: inherited,
                    margin: thickness(child, "Margin")?,
                    line_height,
                }));
            }
            "Table" => blocks.push(Block::Table(parse_table(
                child,
                &inherited,
                depth + 1,
                cells,
            )?)),
            name => return Err(format!("Unsupported native text block: {name}")),
        }
    }
    Ok(blocks)
}
fn parse_runs(
    node: Node<'_, '_>,
    style: &TextStyle,
    out: &mut Vec<Run>,
    depth: usize,
) -> Result<(), String> {
    if depth > 48 {
        return Err("Native inline text nesting exceeds limits".into());
    }
    if node.has_tag_name((XAML, "Run"))
        && let Some(text) = node.attribute("Text")
    {
        out.push(Run {
            text: text.into(),
            style: style.clone(),
        });
    }
    for child in node.children() {
        if child.is_text() {
            out.push(Run {
                text: child.text().unwrap_or("").into(),
                style: style.clone(),
            });
            continue;
        }
        if !child.is_element() {
            continue;
        }
        if child.tag_name().namespace() != Some(XAML) {
            return Err("Unsupported native inline object namespace".into());
        }
        let mut current = inherit_style(child, style)?;
        match child.tag_name().name() {
            "Run" | "Span" | "Hyperlink" => parse_runs(child, &current, out, depth + 1)?,
            "Bold" => {
                current.bold = true;
                parse_runs(child, &current, out, depth + 1)?
            }
            "Italic" => {
                current.italic = true;
                parse_runs(child, &current, out, depth + 1)?
            }
            "Underline" => {
                current.underline = true;
                parse_runs(child, &current, out, depth + 1)?
            }
            "LineBreak" => out.push(Run {
                text: "\n".into(),
                style: current,
            }),
            name => return Err(format!("Unsupported native inline object: {name}")),
        }
    }
    Ok(())
}
fn parse_table(
    node: Node<'_, '_>,
    style: &TextStyle,
    depth: usize,
    total: &mut usize,
) -> Result<Table, String> {
    let mut columns = Vec::new();
    if let Some(cols) = node
        .children()
        .find(|n| n.has_tag_name((XAML, "Table.Columns")))
    {
        for col in cols.children().filter(Node::is_element) {
            if !col.has_tag_name((XAML, "TableColumn")) {
                return Err("Unsupported native table column object".into());
            }
            let v = col.attribute("Width").unwrap_or("Auto");
            let (width, weight) = if v == "Auto" {
                (None, 1.)
            } else if let Some(s) = v.strip_suffix('*') {
                let w = if s.is_empty() {
                    1.
                } else {
                    s.parse::<f32>()
                        .map_err(|_| "Invalid native table star width")?
                };
                if !w.is_finite() || w <= 0. {
                    return Err("Invalid native table star width".into());
                }
                (None, w)
            } else {
                (
                    Some(
                        v.parse::<f32>()
                            .ok()
                            .filter(|v| v.is_finite() && *v > 0. && *v <= 1_000_000.)
                            .ok_or("Invalid native table column width")?,
                    ),
                    0.,
                )
            };
            columns.push(Column { width, weight });
        }
    }
    if columns.len() > 256 {
        return Err("Native table has too many columns".into());
    }
    let mut rows = Vec::new();
    for group in node.children().filter(Node::is_element) {
        match group.tag_name().name() {
            "Table.Columns" => {}
            "TableRowGroup" => {
                let group_style = inherit_style(group, style)?;
                for row in group.children().filter(Node::is_element) {
                    if !row.has_tag_name((XAML, "TableRow")) {
                        return Err("Unsupported native table row object".into());
                    }
                    rows.push((row, inherit_style(row, &group_style)?));
                }
            }
            _ => return Err("Unsupported native table child".into()),
        }
    }
    if rows.len() > 1024 {
        return Err("Native table has too many rows".into());
    }
    let mut occupied = HashSet::new();
    let mut cells = Vec::new();
    let mut count = columns.len();
    for (row, (source, row_style)) in rows.iter().enumerate() {
        let mut column = 0;
        for cell in source.children().filter(Node::is_element) {
            if !cell.has_tag_name((XAML, "TableCell")) {
                return Err("Unsupported native table cell object".into());
            }
            *total += 1;
            if *total > MAX_CELLS {
                return Err("Native table cell budget exceeded".into());
            }
            while occupied.contains(&(row, column)) {
                column += 1
            }
            let span = |name: &str| {
                cell.attribute(name)
                    .unwrap_or("1")
                    .parse::<usize>()
                    .ok()
                    .filter(|v| *v > 0 && *v <= 256)
                    .ok_or_else(|| format!("Invalid native table {name}"))
            };
            let row_span = span("RowSpan")?;
            let column_span = span("ColumnSpan")?;
            if column + column_span > 256 || row + row_span > rows.len() {
                return Err("Native table span exceeds table bounds".into());
            }
            for r in row..row + row_span {
                for c in column..column + column_span {
                    if !occupied.insert((r, c)) {
                        return Err("Overlapping native table spans".into());
                    }
                }
            }
            let style = inherit_style(cell, row_style)?;
            cells.push(Cell {
                row,
                column,
                row_span,
                column_span,
                blocks: parse_blocks(cell, &style, depth + 1, total)?,
                padding: thickness(cell, "Padding")?,
                border: thickness(cell, "BorderThickness")?,
                border_color: cell
                    .attribute("BorderBrush")
                    .map(parse_color)
                    .transpose()?
                    .unwrap_or([0, 0, 0, 255]),
                style,
            });
            column += column_span;
            count = count.max(column);
        }
    }
    if count == 0 || rows.is_empty() {
        return Err("Native table has no cells".into());
    }
    if !columns.is_empty() && count > columns.len() {
        return Err("Native table cells exceed declared columns".into());
    }
    while columns.len() < count {
        columns.push(Column {
            width: None,
            weight: 1.,
        })
    }
    Ok(Table {
        columns,
        rows: rows.len(),
        cells,
        spacing: size_attribute(node, "CellSpacing")?.unwrap_or(2.),
        style: style.clone(),
        row_min_height: 0.,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::{ZipWriter, write::SimpleFileOptions};
    const RICH: &str = r##"<Section xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" FontFamily="Tahoma" FontSize="16" Foreground="#FFFFFFFF"><Paragraph Background="#FF662D91"><Run FontWeight="Bold">title</Run><Run FontStyle="Italic"> value</Run></Paragraph><Table CellSpacing="0"><Table.Columns><TableColumn Width="60"/><TableColumn Width="2*"/></Table.Columns><TableRowGroup><TableRow><TableCell RowSpan="2" Padding="1,2" BorderBrush="#FF00008B" BorderThickness="0,0,1,0"><Paragraph>first</Paragraph></TableCell><TableCell><Paragraph><Run Foreground="#FF00008B">μH</Run></Paragraph></TableCell></TableRow><TableRow><TableCell><Paragraph><Underline>third</Underline></Paragraph></TableCell></TableRow></TableRowGroup></Table></Section>"##;
    fn zip_bytes(parts: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        for (name, bytes) in parts {
            writer
                .start_file(*name, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }
    fn package(relation: &str) -> NativePackage {
        let flow = zip_bytes(&[("Xaml/Document.xaml", RICH.as_bytes())]);
        let worksheet = format!(
            r##"<worksheet xmlns="{WS}" xmlns:x="{XAML}"><regions><region region-id="0" width="240" actualHeight="21" top="0" left="0"><text item-idref="text0"><x:FlowDocument Background="#FF662D91" Foreground="#FFFFFFFF"/></text></region><region region-id="1" top="30" left="0"><math><formattingOverride BackgroundColor="#FFFFF200"/></math></region></regions></worksheet>"##
        );
        let rels = format!(r#"<Relationships xmlns="{REL}">{relation}</Relationships>"#);
        NativePackage::from_bytes(&zip_bytes(&[
            ("mathcad/worksheet.xml", worksheet.as_bytes()),
            ("mathcad/_rels/worksheet.xml.rels", rels.as_bytes()),
            ("mathcad/xaml/Flow0.XamlPackage", &flow),
        ]))
        .unwrap()
    }
    fn valid_relation() -> String {
        format!(r#"<Relationship Id="text0" Type="{FLOW_REL}" Target="xaml/Flow0.XamlPackage"/>"#)
    }
    #[test]
    fn area_visuals_use_owner_relative_paths_and_reuse_inert_rich_readers() {
        let flow = zip_bytes(&[("Xaml/Document.xaml", RICH.as_bytes())]);
        let worksheet = format!(
            r##"<worksheet xmlns="{WS}" xmlns:ml="http://schemas.mathsoft.com/math50" xmlns:x="{XAML}"><regions><region region-id="0" left="0" top="0"><math><formattingOverride BackgroundColor="#FF112233"/></math></region><region region-id="1" left="0" top="30"><Area><regions><region region-id="0" left="9.6" top="9.6" width="240" actualHeight="21"><text item-idref="text0"><x:FlowDocument Background="#FF662D91"/></text></region><region region-id="0" left="9.6" top="40" width="100" actualHeight="25"><math><formattingOverride BackgroundColor="#FFFFF200"/></math></region><region region-id="3" left="9.6" top="70"><spec-table><math><ml:define><ml:id>A</ml:id><ml:matrix rows="1" cols="1"><ml:real>1.00</ml:real></ml:matrix></ml:define></math></spec-table></region><region region-id="4" left="0" top="100"><unknown data="preserved"/></region></regions></Area></region></regions></worksheet>"##
        );
        let rels = format!(
            r#"<Relationships xmlns="{REL}">{}</Relationships>"#,
            valid_relation()
        );
        let package = NativePackage::from_bytes(&zip_bytes(&[
            ("mathcad/worksheet.xml", worksheet.as_bytes()),
            ("mathcad/_rels/worksheet.xml.rels", rels.as_bytes()),
            ("mathcad/xaml/Flow0.XamlPackage", &flow),
        ]))
        .unwrap();
        let original = package
            .original_part("mathcad/worksheet.xml")
            .unwrap()
            .to_vec();
        let root_before = NativeVisuals::from_package(&package).unwrap();
        assert_eq!(root_before.regions.len(), 1);
        let visuals = area_child_visuals(&package, &[0, 1]).unwrap();
        assert_eq!(visuals.len(), 4);
        let entries = area_child_visual_entries(&package, &[0, 1]).unwrap();
        assert_eq!(entries[0].region_path, vec![0, 0, 0]);
        assert_eq!(entries[0].key, "[0,0,0]");
        assert_eq!((entries[0].left, entries[0].top), (9.6, 9.6));
        assert_eq!(entries[0].kind, "text");
        assert_eq!(entries[3].kind, "unknown");
        let text = visuals["[0,0,0]"].as_ref().unwrap();
        assert_eq!(text.size, [240., 21.]);
        assert_eq!(text.background, Some([102, 45, 145, 255]));
        let document = text.document.as_ref().unwrap();
        assert_eq!(document.table_cell_count(), 3);
        assert!(document.plain_text().contains("μH"));
        assert_eq!(
            visuals["[0,0,1]"].as_ref().unwrap().background,
            Some([255, 242, 0, 255])
        );
        assert!(
            visuals["[0,0,2]"]
                .as_ref()
                .unwrap()
                .document
                .as_ref()
                .unwrap()
                .table_cell_count()
                > 0
        );
        assert!(
            visuals["[0,0,3]"]
                .as_ref()
                .unwrap_err()
                .contains("Unsupported")
        );
        assert_eq!(
            NativeVisuals::from_package(&package).unwrap().regions,
            root_before.regions
        );
        assert_eq!(
            package.original_part("mathcad/worksheet.xml").unwrap(),
            original
        );
        assert!(area_child_visuals(&package, &[0, 0]).is_err());
    }
    #[test]
    fn rich_text_table_spans_styles_and_nested_package_roundtrip_without_flattening() {
        let direct = read_flow_xml(RICH, TextStyle::default()).unwrap();
        let zipped = read_flow_package(
            &zip_bytes(&[("Xaml/Document.xaml", RICH.as_bytes())]),
            TextStyle::default(),
        )
        .unwrap();
        assert_eq!(direct, zipped);
        assert_eq!(direct.table_cell_count(), 3);
        assert_eq!(direct.imported_plain_text, "title value\nfirst\nμH\nthird");
        let Block::Paragraph(title) = &direct.blocks[0] else {
            panic!("missing paragraph")
        };
        assert_eq!(title.style.background, Some([102, 45, 145, 255]));
        assert!(title.runs[0].style.bold);
        assert!(title.runs[1].style.italic);
        assert_eq!(title.runs[0].style.background, None);
        let Block::Table(table) = &direct.blocks[1] else {
            panic!("missing table")
        };
        assert_eq!(table.columns[0].width, Some(60.));
        assert_eq!(table.columns[1].weight, 2.);
        assert_eq!(table.cells[0].row_span, 2);
        assert_eq!(table.cells[2].column, 1);
        assert_eq!(table.cells[0].padding, [1., 2., 1., 2.]);
        assert_eq!(table.cells[0].border_color, [0, 0, 139, 255]);
    }
    #[test]
    fn native_visual_regions_resolve_owner_relationships_and_never_repaint_stale_text() {
        let package = package(&valid_relation());
        let original = package
            .original_part("mathcad/worksheet.xml")
            .unwrap()
            .to_vec();
        let visuals = NativeVisuals::from_package(&package).unwrap();
        let visual = visuals.regions["0"].as_ref().unwrap();
        assert_eq!(visual.background, Some([102, 45, 145, 255]));
        assert_eq!(
            visuals.regions["1"].as_ref().unwrap().background,
            Some([255, 242, 0, 255])
        );
        let text = visual
            .document
            .as_ref()
            .unwrap()
            .imported_plain_text
            .clone();
        let mut document = crate::canvas::CanvasDocument::default();
        let id = document.add_text(0., 0., &text);
        let region = document.regions.iter_mut().find(|r| r.id == id).unwrap();
        region.source_region = Some("0".into());
        assert!(visuals.region(region).is_some());
        region.content = CanvasContent::Text("edited text".into());
        assert!(visuals.region(region).is_none());
        assert_eq!(
            package.original_part("mathcad/worksheet.xml").unwrap(),
            original
        );
    }
    #[test]
    fn malformed_or_unsupported_native_objects_remain_explicit_failures() {
        for relation in [
            String::new(),
            valid_relation().replace("xaml/Flow0.XamlPackage", "xaml/missing.XamlPackage"),
            valid_relation().replace("/>", " TargetMode=\"External\"/>"),
        ] {
            let visual = NativeVisuals::from_package(&package(&relation)).unwrap();
            assert!(visual.regions["0"].is_err());
        }
        assert!(
            read_flow_xml(
                &RICH.replace(
                    "<Run FontWeight=\"Bold\">title</Run>",
                    "<InlineUIContainer><Image Source=\"remote\"/></InlineUIContainer>"
                ),
                TextStyle::default()
            )
            .is_err()
        );
        assert!(
            read_flow_xml(
                &RICH.replace("RowSpan=\"2\"", "RowSpan=\"3\""),
                TextStyle::default()
            )
            .is_err()
        );
        assert!(
            read_flow_xml(
                "<!DOCTYPE a [<!ENTITY x 'value'>]><Section>&x;</Section>",
                TextStyle::default()
            )
            .is_err()
        );
        assert!(parse_color("#💩💩").is_err());
        assert!(parse_color("#GGFFFFFF").is_err());
        assert_eq!(parse_color("#8044AABB").unwrap(), [68, 170, 187, 128]);
    }
    #[test]
    fn literal_specification_table_retains_math_headers_units_precision_and_empty_cells() {
        let worksheet = format!(
            r#"<worksheet xmlns="{WS}" xmlns:ml="http://schemas.mathsoft.com/math50"><regions><region region-id="26" actualWidth="100" actualHeight="125"><spec-table><math><ml:define><ml:id labels="VARIABLE">Area</ml:id><ml:apply><ml:scale/><ml:matrix rows="3" cols="1"><ml:real>1.00</ml:real><ml:placeholder/><ml:real>3.5</ml:real></ml:matrix><ml:apply><ml:pow/><ml:id labels="UNIT">mm</ml:id><ml:real>2</ml:real></ml:apply></ml:apply></ml:define></math></spec-table></region></regions></worksheet>"#
        );
        let package = NativePackage::from_bytes(&zip_bytes(&[(
            "mathcad/worksheet.xml",
            worksheet.as_bytes(),
        )]))
        .unwrap();
        let visuals = NativeVisuals::from_package(&package).unwrap();
        let visual = visuals.regions["26"].as_ref().unwrap();
        let Block::Table(table) = &visual.document.as_ref().unwrap().blocks[0] else {
            panic!("missing spec table")
        };
        assert_eq!(table.rows, 5);
        assert_eq!(table.columns.len(), 1);
        assert_eq!(table.row_min_height, 25.);
        let Block::Math(unit) = &table.cells[1].blocks[0] else {
            panic!("missing unit tree")
        };
        assert_eq!(unit.tree.kind, NodeKind::Group);
        assert!(serde_json::to_string(&unit.tree).unwrap().contains("UNIT"));
        let values = table
            .cells
            .iter()
            .filter(|c| c.row >= 2)
            .map(|c| {
                if let Block::Paragraph(p) = &c.blocks[0] {
                    p.runs[0].text.as_str()
                } else {
                    panic!("missing literal")
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(values, ["1.00", "", "3.5"]);
    }

    #[test]
    #[ignore = "Reads explicitly selected local user packages without executing any worksheet"]
    fn inspect_selected_native_visual_packages() {
        let files = std::env::var("MATHFOLIO_NATIVE_VISUAL_FILES")
            .expect("Set semicolon-separated source paths");
        for path in files.split(';') {
            let package = NativePackage::open(std::path::Path::new(path)).unwrap();
            let visuals = NativeVisuals::from_package(&package).unwrap();
            let errors = visuals
                .regions
                .values()
                .filter_map(|r| r.as_ref().err())
                .collect::<Vec<_>>();
            let texts = visuals
                .regions
                .values()
                .filter_map(|r| r.as_ref().ok())
                .filter(|r| r.document.is_some())
                .count();
            let cells = visuals
                .regions
                .values()
                .filter_map(|r| r.as_ref().ok())
                .filter_map(|r| r.document.as_ref())
                .map(RichDocument::table_cell_count)
                .sum::<usize>();
            let backgrounds = visuals
                .regions
                .values()
                .filter_map(|r| r.as_ref().ok())
                .filter_map(|r| r.background)
                .collect::<Vec<_>>();
            if path.ends_with("1015_LCC_S.mcdx") {
                let visual = visuals.regions["26"].as_ref().unwrap();
                let Block::Table(table) = &visual.document.as_ref().unwrap().blocks[0] else {
                    panic!("missing exact source table")
                };
                assert_eq!(
                    (table.columns.len(), table.rows, table.cells.len()),
                    (6, 12, 72)
                );
                let values = table
                    .cells
                    .iter()
                    .filter(|c| c.row >= 2)
                    .map(|c| {
                        if let Block::Paragraph(p) = &c.blocks[0] {
                            p.runs[0].text.as_str()
                        } else {
                            panic!("missing source literal")
                        }
                    })
                    .collect::<Vec<_>>();
                assert_eq!(values.iter().filter(|v| !v.is_empty()).count(), 55);
                assert_eq!(values.iter().filter(|v| v.is_empty()).count(), 5);
                assert!(values.contains(&"1.00"));
                assert_eq!(
                    table
                        .cells
                        .iter()
                        .filter(|c| c.row < 2)
                        .filter(|c| matches!(c.blocks.first(), Some(Block::Math(_))))
                        .count(),
                    12
                );
                assert!((visual.size[1] - 317.0432).abs() < 0.001);
            }
            println!(
                "{}",
                serde_json::json!({"path":path,"regions":visuals.regions.len(),"rich_texts":texts,"table_cells":cells,"backgrounds":backgrounds,"errors":errors})
            );
            assert!(errors.is_empty());
        }
    }
}
