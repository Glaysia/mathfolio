//! Independent Mathcad package export for the supported flat canvas subset.
//! Untouched parts survive source-based export. Saved results remain inert and
//! are reused only when the whole ordered program and package context match.
//! Schema checks do not establish that Prime can reopen a generated file.
use crate::{
    canvas::{
        CanvasContent, CanvasDocument, CanvasRegion, DateSavedFormat, PageBand, PageBandContent,
        PageField, PageNumberFormat,
    },
    native::{NativePackage, xml_text},
};
use mathcraft_compat::editor_bridge::{tree_from_mathml, tree_to_mathml};
use roxmltree::{Document, Node};
#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;
use std::{
    collections::{HashMap, HashSet},
    io::{Cursor, Read, Write},
    ops::Range,
};
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

const WS: &str = "http://schemas.mathsoft.com/worksheet50";
const ML: &str = "http://schemas.mathsoft.com/math50";
const RESULTS: &str = "http://schemas.mathsoft.com/result10";
const REL: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const CONTENT: &str = "http://schemas.openxmlformats.org/package/2006/content-types";
const FLOW_REL: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/flowDocument";
const MAIN_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.mathprocessingml.mathcad.main+xml";
type Patches = Vec<(Range<usize>, String)>;

mod area_authoring;
mod authored_export;
mod calculation_options;
mod matrix_views;
mod plot_results;
mod plot_source;
mod saved_results;
mod text_source;
pub use text_source::RichTextAdmissionStats;
// Canvas validation independently admits authored source. This path validates
// only imported native carriers so a mixed own document can be saved safely.
pub(crate) fn validate_imported_rich_text_carriers(
    canvas: &CanvasDocument,
    package: &NativePackage,
) -> Result<RichTextAdmissionStats, String> {
    text_source::validate_carriers(canvas, package)
}
/// Source-only admission: no recursive Canvas validation or export/result writes.
pub fn validate_rich_text_carriers(
    canvas: &CanvasDocument,
    package: &NativePackage,
) -> Result<RichTextAdmissionStats, String> {
    let stats = text_source::validate_carriers(canvas, package)?;
    authored_export::validate_regions(canvas.regions.iter(), package, stats.source_bytes)?;
    Ok(stats)
}
/// Command-only append preflight. No document mutation, serialization or result
/// projection. Do not call for a committed character in a cached RichEditor.
pub fn validate_rich_text_append(
    canvas: &CanvasDocument,
    package: &NativePackage,
    candidate: &CanvasRegion,
) -> Result<RichTextAdmissionStats, String> {
    let stats = text_source::validate_append(canvas, package, candidate)?;
    authored_export::validate_regions(
        canvas.regions.iter().chain(std::iter::once(candidate)),
        package,
        stats.source_bytes,
    )?;
    Ok(stats)
}
/// Command-only selected Text replacement preflight (e.g. range formatting).
pub fn validate_rich_text_replacement(
    canvas: &CanvasDocument,
    package: &NativePackage,
    candidate: &CanvasRegion,
) -> Result<RichTextAdmissionStats, String> {
    let stats = text_source::validate_replacement(canvas, package, candidate)?;
    authored_export::validate_regions(
        canvas.regions.iter().map(|old| {
            if old.id == candidate.id {
                candidate
            } else {
                old
            }
        }),
        package,
        stats.source_bytes,
    )?;
    Ok(stats)
}

#[cfg(test)]
mod text_source_tests;

/// Resolve and validate immutable native Text source without exporting it.
pub fn read_native_text_authority(
    package: &NativePackage,
    owner: &str,
) -> Result<std::sync::Arc<crate::rich_text_source::OriginalTextAuthority>, String> {
    text_source::read_native_authority(package, owner, "mathcad/settings/presentation.xml")
}

struct ResultIds {
    next: i32,
    pending: Vec<i32>,
    next_region: u64,
    owners: HashMap<String, u64>,
    plots: HashMap<u64, Vec<i32>>,
}
impl ResultIds {
    fn allocate(&mut self) -> Result<i32, String> {
        let id = self.next;
        self.next = self
            .next
            .checked_add(1)
            .ok_or("Result ID space exhausted")?;
        self.pending.push(id);
        Ok(id)
    }
    fn allocate_region(&mut self) -> Result<String, String> {
        let id = self.next_region;
        self.next_region = id
            .checked_add(1)
            .ok_or("Native region ID space exhausted")?;
        Ok(id.to_string())
    }
}

/// Writes a new .mcdx, refusing every existing destination, including originals.
/// Source region references must be the original top-level region-id strings.
#[cfg(not(target_arch = "wasm32"))]
pub fn export_mcdx(doc: &CanvasDocument, path: &Path) -> Result<(), String> {
    doc.validate()?;
    let mut package = match &doc.native_archive {
        Some(bytes) => NativePackage::from_bytes(bytes)?,
        None => minimal_package(doc)?,
    };
    export_into(doc, &mut package)?;
    package.write_new(path)
}

/// Full existing export proof/order, bounded native bytes; no filesystem IO.
pub fn prepare_mcdx(
    doc: &CanvasDocument,
) -> Result<crate::native_save::PreparedNativeSave, String> {
    doc.validate()?;
    let mut package = match &doc.native_archive {
        Some(bytes) => NativePackage::from_bytes(bytes)?,
        None => minimal_package(doc)?,
    };
    export_into(doc, &mut package)?;
    crate::native_save::PreparedNativeSave::from_package(package)
}

impl CanvasDocument {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn export_mcdx(&self, path: &Path) -> Result<(), String> {
        export_mcdx(self, path)
    }
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn parse(s: &str) -> Result<Document<'_>, String> {
    Document::parse_with_options(
        s,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: 250_000,
            ..Default::default()
        },
    )
    .map_err(|e| e.to_string())
}

fn apply(mut xml: String, mut patches: Patches) -> Result<String, String> {
    patches.sort_by_key(|p| p.0.start);
    for pair in patches.windows(2) {
        if pair[0].0.end > pair[1].0.start {
            return Err("Overlapping export patches".into());
        }
    }
    for (range, replacement) in patches.into_iter().rev() {
        xml.replace_range(range, &replacement);
    }
    Ok(xml)
}

fn same_native_xml(left: Node<'_, '_>, right: Node<'_, '_>) -> bool {
    let mut pending = vec![(left, right)];
    while let Some((left, right)) = pending.pop() {
        if left.node_type() != right.node_type()
            || left.tag_name() != right.tag_name()
            || left.text() != right.text()
        {
            return false;
        }
        if left.attributes().len() != right.attributes().len()
            || left.attributes().any(|a| {
                let value = match a.namespace() {
                    Some(namespace) => right.attribute((namespace, a.name())),
                    None => right.attribute(a.name()),
                };
                value != Some(a.value())
            })
        {
            return false;
        }
        let l = left.children().collect::<Vec<_>>();
        let r = right.children().collect::<Vec<_>>();
        if l.len() != r.len() {
            return false;
        }
        pending.extend(l.into_iter().zip(r));
    }
    true
}

fn tag_end(xml: &str, start: usize) -> Result<usize, String> {
    let mut quote = None;
    for (i, ch) in xml[start..].char_indices() {
        match (quote, ch) {
            (Some(q), c) if q == c => quote = None,
            (None, '\'' | '"') => quote = Some(ch),
            (None, '>') => return Ok(start + i),
            _ => {}
        }
    }
    Err("Unclosed XML tag".into())
}

fn attrs(
    node: Node<'_, '_>,
    xml: &str,
    values: &[(&str, Option<String>)],
) -> Result<Patches, String> {
    let mut patches = Vec::new();
    let mut additions = String::new();
    for (name, value) in values {
        if let Some(attr) = node.attribute_node(*name) {
            patches.push((
                attr.range(),
                value
                    .as_ref()
                    .map(|v| format!("{name}=\"{}\"", escape(v)))
                    .unwrap_or_default(),
            ));
        } else if let Some(value) = value {
            additions.push_str(&format!(" {name}=\"{}\"", escape(value)));
        }
    }
    if !additions.is_empty() {
        let end = tag_end(xml, node.range().start)?;
        let at = if xml.as_bytes()[end - 1] == b'/' {
            end - 1
        } else {
            end
        };
        patches.push((at..at, additions));
    }
    Ok(patches)
}

fn insert_child(xml: &str, child: &str) -> Result<String, String> {
    // Fragments may use a prefix declared on the original worksheet root.
    let start = match parse(xml) {
        Ok(doc) => doc.root_element().range().start,
        Err(_) if xml.starts_with('<') && !xml.starts_with("<?") && !xml.starts_with("<!") => 0,
        Err(e) => return Err(e),
    };
    let end = tag_end(xml, start)?;
    if xml.as_bytes()[end - 1] == b'/' {
        let qname = xml[start + 1..]
            .split(|c: char| c.is_whitespace() || c == '/' || c == '>')
            .next()
            .ok_or("XML tag missing")?;
        let mut out = xml.to_owned();
        out.replace_range(end - 1..end + 1, &format!(">{child}</{qname}>"));
        Ok(out)
    } else {
        let at = xml.rfind("</").ok_or("XML closing tag missing")?;
        let mut out = xml.to_owned();
        out.insert_str(at, child);
        Ok(out)
    }
}

pub(crate) fn set_xml(package: &mut NativePackage, name: &str, text: String) -> Result<(), String> {
    // Preserve the source part's encoding as well as its declaration.
    let original = package.original_part(name).unwrap_or(&[]);
    let bytes = if original.starts_with(&[0xff, 0xfe]) || original.starts_with(&[b'<', 0, b'?', 0])
    {
        let prefix = if original.starts_with(&[0xff, 0xfe]) {
            vec![0xff, 0xfe]
        } else {
            vec![]
        };
        prefix
            .into_iter()
            .chain(text.encode_utf16().flat_map(u16::to_le_bytes))
            .collect()
    } else if original.starts_with(&[0xfe, 0xff]) || original.starts_with(&[0, b'<', 0, b'?']) {
        let prefix = if original.starts_with(&[0xfe, 0xff]) {
            vec![0xfe, 0xff]
        } else {
            vec![]
        };
        prefix
            .into_iter()
            .chain(text.encode_utf16().flat_map(u16::to_be_bytes))
            .collect()
    } else if original.starts_with(&[0xef, 0xbb, 0xbf]) {
        [0xef, 0xbb, 0xbf].into_iter().chain(text.bytes()).collect()
    } else {
        text.into_bytes()
    };
    package.set_part(name, bytes)
}

struct SourceProjection {
    ids: ResultIds,
    unchanged: bool,
    references: HashMap<i32, usize>,
    deferred_text: text_source::DeferredTextEdits,
}
/// Prove the whole ordered source plus external package context against the
/// supplied original package, without IO, evaluator calls, or result writes.
/// Caller must supply the actual retained archive associated with this canvas.
/// The ephemeral package copy is bounded by NativePackage's existing limits.
pub fn source_context_matches(
    canvas: &CanvasDocument,
    package: &NativePackage,
) -> Result<bool, String> {
    canvas.validate()?;
    let mut projected = package.clone();
    Ok(project_source(canvas, &mut projected)?.unchanged)
}
fn export_into(canvas: &CanvasDocument, package: &mut NativePackage) -> Result<(), String> {
    let SourceProjection {
        ids: result_ids,
        unchanged,
        references,
        deferred_text,
    } = project_source(canvas, package)?;
    let deferred_matrix_views =
        matrix_views::prepare(canvas, package, &result_ids.owners, &references)?;
    if !unchanged {
        invalidate_caches(package)?;
    }
    append_pending_results(package, &result_ids.pending)?;
    saved_results::write(canvas, package, &result_ids.owners, &references)?;
    plot_results::write(canvas, package, &result_ids.plots, &references)?;
    // Descriptor input mode is not a change to the existing ordered program.
    // Patch only after result/source proof; toggling it must not recalculate ASTs.
    calculation_options::write(canvas.bin_oct_hex_mode, package)?;
    matrix_views::apply_deferred(package, deferred_matrix_views)?;
    // Text has an independent owner receipt and may COW its worksheet binding.
    text_source::apply_deferred(package, deferred_text)?;
    Ok(())
}
fn project_source(
    canvas: &CanvasDocument,
    package: &mut NativePackage,
) -> Result<SourceProjection, String> {
    // The public entry points validate the whole Canvas first. Also bind each
    // authored source here before the imported-only prepare intentionally skips it.
    for region in &canvas.regions {
        region.validated_authored_text()?;
    }
    let original_context = crate::native_result::program::ContextSnapshot::capture(package)?;
    let mut text_edits = text_source::prepare(canvas, package)?;
    if canvas
        .regions
        .iter()
        .any(|region| region.authored_text.is_some())
    {
        // Imported range-format receipts are still bound to the initial real
        // catalog and owner. Apply BEFORE authored catalog/part allocation,
        // without weakening any binding/hash check. This is now an ordinary
        // source/context mutation and cannot use the inert deferred shortcut.
        // Capture original_context stays above all changes. None keeps the
        // exact old inert path and final deferred application unchanged.
        text_source::apply_deferred(package, std::mem::take(&mut text_edits.deferred))?;
    }
    // Rich CONTENT edits keep exact styles but change context before proof.
    text_source::apply_ordinary(package, text_edits.ordinary)?;
    let math_defaults = crate::formatting::read_worksheet_math_format_defaults(package)?;
    let name = package.worksheet_part_name().to_string();
    let source = package.worksheet_xml()?;
    let doc = parse(&source)?;
    let root = doc.root_element();
    let regions = root
        .children()
        .find(|n| n.has_tag_name((WS, "regions")))
        .ok_or("Missing worksheet regions")?;
    let original: Vec<_> = regions
        .children()
        .filter(|n| n.has_tag_name((WS, "region")))
        .collect();
    let mut by_source = HashMap::new();
    let native_regions = package.regions()?;
    let old_references = result_references(package)?;
    let mut result_ids = ResultIds {
        next: package
            .caches()?
            .iter()
            .filter_map(|c| crate::native::parse_result_id(c.result_id.as_deref()?).ok())
            .chain(old_references.keys().copied())
            .max()
            .unwrap_or(0)
            .max(0)
            .checked_add(1)
            .ok_or("Result ID space exhausted")?,
        pending: vec![],
        plots: HashMap::new(),
        owners: canvas
            .regions
            .iter()
            .filter_map(|region| Some((region.source_region.clone()?, region.id)))
            .collect(),
        next_region: native_regions
            .iter()
            .filter_map(|r| r.region_id.as_deref()?.parse::<u64>().ok())
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or("Native region ID space exhausted")?,
    };
    for region in &canvas.regions {
        if let Some(id) = &region.source_region
            && by_source.insert(id.as_str(), region).is_some()
        {
            return Err("Duplicate source region references".into());
        }
    }
    let mut changes = Vec::new();
    for old in original {
        let id = old
            .attribute("region-id")
            .ok_or("Original region has no ID")?;
        let text = if let Some(new) = by_source.remove(id) {
            edit_region(
                old,
                (&source, math_defaults),
                new,
                id,
                package,
                &name,
                &mut result_ids,
            )?
        } else {
            String::new()
        };
        changes.push((old.range(), text));
    }
    if !by_source.is_empty() {
        return Err("Source region reference does not identify a top-level original region".into());
    }
    let mut additions = String::new();
    for region in canvas.regions.iter().filter(|r| r.source_region.is_none()) {
        let id = result_ids.allocate_region()?;
        result_ids.owners.insert(id.clone(), region.id);
        additions.push_str(&new_region(
            region,
            &id,
            package,
            &name,
            &mut result_ids,
            &mut text_edits.derived,
        )?);
    }
    text_source::assert_all_derived_bound(&text_edits.derived)?;
    if !additions.is_empty() {
        let fragment = &source[regions.range()];
        // The original regions patches overlap their parent replacement. Apply
        // them within that replacement first, retaining comments and whitespace.
        let mut inner_changes = Vec::new();
        for (range, text) in changes {
            inner_changes.push((
                range.start - regions.range().start..range.end - regions.range().start,
                text,
            ));
        }
        let updated = apply(fragment.into(), inner_changes)?;
        changes = vec![(regions.range(), insert_child(&updated, &additions)?)];
    }
    set_xml(package, &name, apply(source.clone(), changes)?)?;
    if canvas.page_sections.explicit {
        export_page_band(
            package,
            "mathcad/header.xml",
            "header",
            &canvas.page_sections.header,
        )?;
        export_page_band(
            package,
            "mathcad/footer.xml",
            "footer",
            &canvas.page_sections.footer,
        )?;
        update_page_numbering(canvas, package)?;
    }
    if canvas.page_setup.is_some() {
        update_page_setup(canvas, package)?;
    }
    // Reusing isolated unchanged rows would retain stale downstream results.
    // Prove the entire ordered program plus external package context instead.
    let mut cache_ids = HashSet::new();
    let unique_results = package.caches()?.iter().all(|row| {
        row.result_id
            .as_deref()
            .and_then(|id| crate::native::parse_result_id(id).ok())
            .is_some_and(|id| cache_ids.insert(id))
    });
    let references = result_references(package)?;
    let unique_references = references.values().all(|count| *count == 1);
    let unchanged = unique_results
        && unique_references
        && original_context.matches(package)?
        && crate::native_result::program::same_calculation_program(
            &source,
            &package.worksheet_xml()?,
        )
        .unwrap_or(false);
    Ok(SourceProjection {
        ids: result_ids,
        unchanged,
        references,
        deferred_text: text_edits.deferred,
    })
}

/// Columns and embedded calculatables can have Math resultRef without owning
/// a worksheet region. Reserve and count those identities too, even when their
/// saved resultData is missing. Header/footer XML shares the package ID space.
fn result_references(package: &NativePackage) -> Result<HashMap<i32, usize>, String> {
    package.result_reference_counts()
}

fn page_number_template(format: PageNumberFormat) -> &'static str {
    match format {
        PageNumberFormat::Number => "@PageNo",
        PageNumberFormat::PageNumber => "Page_@PageNo",
        PageNumberFormat::AbbreviatedPageNumber => "p._@PageNo",
        PageNumberFormat::NumberOfTotal => "@PageNo_of_@PagesTotal",
        PageNumberFormat::PageNumberOfTotal => "Page_@PageNo_of_@PagesTotal",
        PageNumberFormat::DashedNumber => "-_@PageNo_-",
        PageNumberFormat::ParenthesizedNumber => "(@PageNo)",
        PageNumberFormat::NumberParenthesizedTotal => "@PageNo_(@PagesTotal)",
    }
}

fn date_saved_template(format: DateSavedFormat) -> &'static str {
    match format {
        DateSavedFormat::MonthDayYear => "mm_dd_yyyy",
        DateSavedFormat::MonthDayShortYear => "mm_dd_yy",
        DateSavedFormat::DayMonthYear => "dd_mm_yyyy",
        DateSavedFormat::DayMonthShortYear => "dd_mm_yy",
        DateSavedFormat::YearMonthDay => "yyyy_mm_dd",
        DateSavedFormat::SystemShort => "System_Short",
        DateSavedFormat::SystemLong => "System_Long",
    }
}

fn page_field_xml(field: PageField) -> String {
    match field {
        PageField::PageNumber(format) => format!(
            "<fieldText><text/><pageNumber template=\"{}\"/></fieldText>",
            page_number_template(format)
        ),
        PageField::DateSaved(format) => format!(
            "<fieldText><text/><dateSaved template=\"{}\"/></fieldText>",
            date_saved_template(format)
        ),
        PageField::FileName => "<fieldText><text/><fileName/></fieldText>".into(),
        PageField::FilePath => "<fieldText><text/><filePath/></fieldText>".into(),
    }
}

fn export_page_band(
    package: &mut NativePackage,
    part: &str,
    root_name: &str,
    band: &PageBand,
) -> Result<(), String> {
    let source = package
        .original_part(part)
        .map(xml_text)
        .transpose()?
        .ok_or_else(|| format!("Missing native {root_name} part"))?;
    let doc = parse(&source)?;
    let root = doc.root_element();
    if !root.has_tag_name((WS, root_name)) {
        return Err(format!("Native {root_name} part has an unexpected root"));
    }
    let regions = root
        .children()
        .find(|n| n.has_tag_name((WS, "regions")))
        .ok_or_else(|| format!("Native {root_name} part has no regions"))?;
    let regions_range = regions.range();
    let mut serialized = String::new();
    for region in &band.regions {
        let payload = match &region.content {
            PageBandContent::Text(text) => format!(
                "<text item-idref=\"{}\"/>",
                add_text_part(package, part, &format!("{root_name}{}", region.id), text)?
            ),
            PageBandContent::Field(field) => page_field_xml(*field),
        };
        serialized.push_str(&format!(
            "<region xmlns=\"{WS}\" region-id=\"{}\" left=\"{}\" top=\"{}\">{payload}</region>",
            region.id, region.left, region.top
        ));
    }
    let replacement = format!("<regions xmlns=\"{WS}\">{serialized}</regions>");
    set_xml(
        package,
        part,
        apply(source, vec![(regions_range, replacement)])?,
    )
}

fn update_page_numbering(
    canvas: &CanvasDocument,
    package: &mut NativePackage,
) -> Result<(), String> {
    let part = "mathcad/settings/presentation.xml";
    let source = package
        .original_part(part)
        .map(xml_text)
        .transpose()?
        .ok_or("Missing native presentation settings")?;
    let doc = parse(&source)?;
    let page_model = doc
        .root_element()
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "pageModel")
        .ok_or("Native presentation settings have no page model")?;
    let patches = attrs(
        page_model,
        &source,
        &[
            (
                "starting-page-number",
                Some(canvas.page_sections.starting_page_number.to_string()),
            ),
            (
                "total-pages-offset",
                Some(canvas.page_sections.total_pages_offset.to_string()),
            ),
        ],
    )?;
    set_xml(package, part, apply(source, patches)?)
}

fn update_page_setup(canvas: &CanvasDocument, package: &mut NativePackage) -> Result<(), String> {
    let setup = canvas.page_setup.as_ref().ok_or("Page setup is missing")?;
    setup.validate()?;
    let part = "mathcad/settings/presentation.xml";
    let source = package
        .original_xml_part(part)?
        .ok_or("Missing native presentation settings")?;
    let doc = parse(&source)?;
    let model = doc
        .descendants()
        .find(|n| n.is_element() && n.tag_name().name() == "pageModel")
        .ok_or("Native presentation settings have no page model")?;
    let margins = setup.margins.map(|v| v.to_string()).join(",");
    let patches = attrs(
        model,
        &source,
        &[
            ("paper-code", Some(setup.paper.code().into())),
            (
                "orientation",
                Some(
                    if setup.landscape {
                        "Landscape"
                    } else {
                        "Portrait"
                    }
                    .into(),
                ),
            ),
            ("margin-type", Some("Custom".into())),
            ("page-margin", Some(margins)),
            ("page-width", Some((canvas.width * 72. / 96.).to_string())),
            ("page-height", Some((canvas.height * 72. / 96.).to_string())),
        ],
    )?;
    set_xml(package, part, apply(source, patches)?)
}

fn context(node: Node<'_, '_>, fragment: &str) -> (String, usize) {
    let mut opening = "<context".to_string();
    for ns in node.namespaces() {
        if ns.name() == Some("xml") {
            continue;
        }
        let key = ns
            .name()
            .map(|n| format!("xmlns:{n}"))
            .unwrap_or_else(|| "xmlns".into());
        opening.push_str(&format!(" {key}=\"{}\"", escape(ns.uri())));
    }
    opening.push('>');
    let len = opening.len();
    (format!("{opening}{fragment}</context>"), len)
}

fn number_format_patches(
    number: Node<'_, '_>,
    xml: &str,
    original: &crate::canvas::MathFormat,
    updated: &crate::canvas::MathFormat,
) -> Result<Patches, String> {
    let kind_changed = original.result_format != updated.result_format;
    let mut original_target = *original;
    original_target.result_format = updated.result_format;
    let previous = crate::formatting::number_attributes(&original_target);
    let desired = crate::formatting::number_attributes(updated);
    let mut values = Vec::new();
    for (name, value) in &desired {
        let unsupported_style = !matches!(
            number.tag_name().name(),
            "general" | "decimal" | "scientific" | "engineering" | "percent"
        );
        if unsupported_style
            && !kind_changed
            && !matches!(
                *name,
                "precision" | "imaginary-value" | "zero-threshold" | "complex-threshold"
            )
        {
            continue;
        }
        // Radix is not implemented. Preserve it on unrelated edits; an explicit
        // switch to a format whose XSD fixes decimal must obey that constraint.
        if *name == "radix"
            && !(kind_changed
                && matches!(
                    updated.result_format,
                    crate::canvas::ResultFormat::Scientific
                        | crate::canvas::ResultFormat::Engineering
                        | crate::canvas::ResultFormat::Percent
                ))
        {
            continue;
        }
        let before = previous
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value);
        if before != Some(value)
            || (kind_changed && number.attribute(*name).is_none())
            || (*name == "radix" && number.attribute("radix") != Some(value.as_str()))
        {
            values.push((*name, Some(value.clone())));
        }
    }
    if kind_changed {
        for name in [
            "use-e-notation",
            "use-engineering-format",
            "use-mixed-numbers",
            "zero-threshold",
            "exponential-threshold",
            "complex-threshold",
        ] {
            if !desired.iter().any(|(key, _)| *key == name) && number.attribute(name).is_some() {
                values.push((name, None));
            }
        }
    }
    let mut patches = attrs(number, xml, &values)?;
    if kind_changed {
        let start = number.range().start + 1;
        let end = xml[start..]
            .find(|c: char| c.is_whitespace() || c == '/' || c == '>')
            .ok_or("Malformed number format name")?
            + start;
        let old_name = &xml[start..end];
        let local = crate::formatting::style_name(updated.result_format);
        let name = old_name.rsplit_once(':').map_or_else(
            || local.to_string(),
            |(prefix, _)| format!("{prefix}:{local}"),
        );
        patches.push((start..end, name.clone()));
        if !xml[number.range()].trim_end().ends_with("/>") {
            let close = xml[number.range()]
                .rfind("</")
                .ok_or("Number format closing name missing")?
                + number.range().start
                + 2;
            patches.push((close..close + old_name.len(), name));
        }
    }
    Ok(patches)
}

fn edit_region(
    old: Node<'_, '_>,
    source_context: (&str, crate::canvas::MathFormat),
    region: &CanvasRegion,
    id: &str,
    package: &mut NativePackage,
    worksheet: &str,
    result_ids: &mut ResultIds,
) -> Result<String, String> {
    let (source, math_defaults) = source_context;
    let original_raw = &source[old.range()];
    let raw = match &region.content {
        CanvasContent::Opaque { xml, .. } => xml.as_str(),
        _ => original_raw,
    };
    let (wrapped, start) = context(old, raw);
    let doc = parse(&wrapped)?;
    let node = doc
        .root_element()
        .children()
        .find(Node::is_element)
        .ok_or("Empty original region")?;
    if !node.has_tag_name((WS, "region")) {
        return Err("Opaque content must preserve a worksheet region wrapper".into());
    }
    let mut patches = attrs(
        node,
        &wrapped,
        &[
            ("left", Some(region.left.to_string())),
            ("top", Some(region.top.to_string())),
            ("region-id", Some(id.into())),
        ],
    )?;
    if region.supports_calculation_toggle()
        && !matches!(region.content, CanvasContent::Plot(_))
        && let Some(item) = node.children().find(|n| {
            n.tag_name().namespace() == Some(WS)
                && matches!(
                    n.tag_name().name(),
                    "math"
                        | "plot"
                        | "include"
                        | "mathBoard"
                        | "specTable"
                        | "spec-table"
                        | "solveblock"
                )
        })
        && matches!(item.attribute("disable-calc"), Some("true" | "1"))
            != region.calculation_disabled
    {
        patches.extend(attrs(
            item,
            &wrapped,
            &[(
                "disable-calc",
                Some(region.calculation_disabled.to_string()),
            )],
        )?);
    }
    match &region.content {
        CanvasContent::Math(tree) => {
            let math = node
                .children()
                .find(|n| n.has_tag_name((WS, "math")))
                .ok_or("Cannot change a non-math source region to math")?;
            let expr = math
                .children()
                .find(|n| n.is_element() && n.tag_name().namespace() == Some(ML))
                .ok_or("Source math has no expression")?;
            let (original_expr, _) = context(expr, &wrapped[expr.range()]);
            let expression_changed = tree_from_mathml(&original_expr).as_ref().ok() != Some(tree);

            if region.format.explicit {
                let original_format =
                    crate::formatting::read_math_format_with_defaults(math, math_defaults);
                let mut addition = String::new();
                if region.format.font_points != original_format.font_points {
                    if let Some(format) = math
                        .children()
                        .find(|n| n.has_tag_name((WS, "formattingOverride")))
                    {
                        patches.extend(attrs(
                            format,
                            &wrapped,
                            &[("FontSize", Some(region.format.font_points.to_string()))],
                        )?);
                    } else {
                        addition.push_str(&format!(
                            "<formattingOverride FontSize=\"{}\"/>",
                            region.format.font_points
                        ));
                    }
                }
                if !crate::formatting::number_format_eq(&region.format, &original_format) {
                    if let Some(format) = math
                        .children()
                        .find(|n| n.has_tag_name((WS, "resultFormat")))
                    {
                        let old_number = crate::formatting::number_node(format);
                        if let Some(number) = old_number {
                            patches.extend(number_format_patches(
                                number,
                                &wrapped,
                                &original_format,
                                &region.format,
                            )?);
                        } else {
                            patches.push((
                                format.range(),
                                insert_child(
                                    &wrapped[format.range()],
                                    &crate::formatting::number_xml(&region.format),
                                )?,
                            ));
                        }
                    } else {
                        addition.push_str(&format!(
                            "<resultFormat>{}</resultFormat>",
                            crate::formatting::number_xml(&region.format)
                        ));
                    }
                }
                if !addition.is_empty() {
                    let at = wrapped[math.range()]
                        .rfind("</")
                        .ok_or("Math closing element missing")?
                        + math.range().start;
                    patches.push((at..at, addition));
                }
            }
            let original_format =
                crate::formatting::read_math_format_with_defaults(math, math_defaults);
            let format_changed = region.format.explicit
                && (region.format.font_points != original_format.font_points
                    || !crate::formatting::number_format_eq(&region.format, &original_format));
            if format_changed && !expression_changed {
                patches.extend(attrs(
                    node,
                    &wrapped,
                    &[("actualWidth", None), ("actualHeight", None)],
                )?);
            }
            if expression_changed {
                let serialized = tree_to_mathml(tree)?;
                patches.push((expr.range(), bind_math(&serialized)?));
                patches.extend(attrs(
                    math,
                    &wrapped,
                    &[
                        ("resultRef", Some(result_ids.allocate()?.to_string())),
                        ("error", None),
                    ],
                )?);
                patches.extend(attrs(
                    node,
                    &wrapped,
                    &[("actualWidth", None), ("actualHeight", None)],
                )?);
            } else if math.attribute("resultRef").is_none() {
                patches.extend(attrs(
                    math,
                    &wrapped,
                    &[("resultRef", Some(result_ids.allocate()?.to_string()))],
                )?);
            }
        }
        CanvasContent::Text(text) => {
            let old_text = node
                .children()
                .find(|n| n.has_tag_name((WS, "text")))
                .ok_or("Cannot change a non-text source region to text")?;
            // prepare() admitted every carrier before reaching this loop.
            // Content carriers already installed COW; inert format carriers
            // remain deliberately unchanged until after source/result proof.
            if region.rich_text.is_none()
                && read_text(package, worksheet, old_text.attribute("item-idref"))?.as_deref()
                    != Some(text)
            {
                if old_text.children().any(|n| n.is_element()) {
                    return Err("Editing text with embedded regions is unsupported".into());
                }
                let rel = add_text_part(package, worksheet, id, text)?;
                patches.extend(attrs(old_text, &wrapped, &[("item-idref", Some(rel))])?);
                patches.extend(attrs(
                    node,
                    &wrapped,
                    &[("actualWidth", None), ("actualHeight", None)],
                )?);
            }
        }
        CanvasContent::Plot(_) => {
            patches.extend(plot_source::edit(node, &wrapped, region, result_ids)?)
        }
        CanvasContent::Opaque { kind, .. } if kind == "Area" => {
            let old_area = old
                .children()
                .find(|n| n.has_tag_name((WS, "Area")))
                .ok_or("Original Area missing")?;
            let new_area = node
                .children()
                .find(|n| n.has_tag_name((WS, "Area")))
                .ok_or("Edited Area missing")?;
            if old_area.children().any(|n| {
                n.has_tag_name((WS, "ProtectionData")) || n.has_tag_name((WS, "ProtectedItem"))
            }) && !same_native_xml(old_area, new_area)
            {
                return Err("Protected Area edits are unsupported".into());
            }
            fn area_regions<'a, 'i>(area: Node<'a, 'i>) -> Vec<Node<'a, 'i>> {
                area.children()
                    .find(|n| n.has_tag_name((WS, "regions")))
                    .map(|n| {
                        n.children()
                            .filter(|n| n.has_tag_name((WS, "region")))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            }
            let before = area_regions(old_area);
            let after = area_regions(new_area);
            if before.iter().any(|original| {
                !after
                    .iter()
                    .any(|child| child.attribute("region-id") == original.attribute("region-id"))
            }) {
                return Err("Removing or renaming native Area children is unsupported".into());
            }
            let mut changed = false;
            let mut seen = HashSet::new();
            for child in after {
                let child_id = child.attribute("region-id").ok_or("Area child has no ID")?;
                if !seen.insert(child_id) {
                    return Err("Duplicate Area child ID".into());
                }
                let original = before
                    .iter()
                    .find(|n| n.attribute("region-id") == Some(child_id));
                let Some(original) = original else {
                    if !child_id
                        .strip_prefix("mathcraft-area-new-")
                        .is_some_and(|suffix| suffix.parse::<u64>().is_ok_and(|id| id > 0))
                    {
                        return Err(
                            "Area child does not identify an original or authored child".into()
                        );
                    }
                    patches.extend(area_authoring::new_math(child, &wrapped, result_ids)?);
                    changed = true;
                    continue;
                };
                let old_math = original.children().find(|n| n.has_tag_name((WS, "math")));
                let new_math = child.children().find(|n| n.has_tag_name((WS, "math")));
                let (Some(old_math), Some(new_math)) = (old_math, new_math) else {
                    if old_math.is_some() || new_math.is_some() {
                        return Err("Changing native Area child kinds is unsupported".into());
                    }
                    continue;
                };
                fn expression<'a, 'i>(math: Node<'a, 'i>) -> Result<Node<'a, 'i>, String> {
                    let roots = math
                        .children()
                        .filter(|n| n.is_element() && n.tag_name().namespace() == Some(ML))
                        .collect::<Vec<_>>();
                    if roots.len() == 1 {
                        Ok(roots[0])
                    } else {
                        Err("Area math needs one expression root".to_owned())
                    }
                }
                let old_expr = expression(old_math)?;
                let new_expr = expression(new_math)?;
                let equal = same_native_xml(old_expr, new_expr) || {
                    let old_bound = crate::spec_table::standalone(old_expr, source)?;
                    let new_bound = crate::spec_table::standalone(new_expr, &wrapped)?;
                    match (tree_from_mathml(&old_bound), tree_from_mathml(&new_bound)) {
                        (Ok(a), Ok(b)) => a == b,
                        _ => false,
                    }
                };
                if !equal {
                    // A raw Opaque edit must satisfy the same supported tree
                    // contract as the public Area mutation helper.
                    tree_from_mathml(&crate::spec_table::standalone(new_expr, &wrapped)?)?;
                    changed = true;
                    patches.extend(attrs(
                        new_math,
                        &wrapped,
                        &[
                            ("resultRef", Some(result_ids.allocate()?.to_string())),
                            ("error", None),
                        ],
                    )?);
                    patches.extend(attrs(
                        child,
                        &wrapped,
                        &[("actualWidth", None), ("actualHeight", None)],
                    )?);
                } else if old_math.attribute("resultRef").is_none() {
                    patches.extend(attrs(
                        new_math,
                        &wrapped,
                        &[("resultRef", Some(result_ids.allocate()?.to_string()))],
                    )?);
                } else if new_math.attribute("resultRef") != old_math.attribute("resultRef") {
                    // Ref edits cannot attach an unrelated saved result to an
                    // unchanged expression, or leave required refs absent.
                    patches.extend(attrs(
                        new_math,
                        &wrapped,
                        &[(
                            "resultRef",
                            old_math.attribute("resultRef").map(str::to_owned),
                        )],
                    )?);
                }
            }
            if changed {
                patches.extend(attrs(
                    node,
                    &wrapped,
                    &[("actualWidth", None), ("actualHeight", None)],
                )?);
            }
        }
        CanvasContent::Opaque { kind, .. } if kind == "spec-table" => {
            let (original, _) = context(old, original_raw);
            let original_doc = parse(&original)?;
            let original_region = original_doc
                .root_element()
                .children()
                .find(Node::is_element)
                .ok_or("Native source table region missing")?;
            // A future unsupported native cell remains losslessly exportable if
            // only the region position/toggle changed; editing never fabricates it.
            let old_table = original_region
                .children()
                .find(|n| n.has_tag_name((WS, "spec-table")))
                .ok_or("Native source table missing")?;
            let new_table = node
                .children()
                .find(|n| n.has_tag_name((WS, "spec-table")))
                .ok_or("Edited native table missing")?;
            if !same_native_xml(old_table, new_table) {
                let before = crate::spec_table::SpecTable::parse(&original)?;
                let after = crate::spec_table::SpecTable::parse(&wrapped)?;
                let table = node
                    .children()
                    .find(|n| n.has_tag_name((WS, "spec-table")))
                    .ok_or("Edited native table is missing")?;
                let columns = table
                    .children()
                    .filter(|n| n.has_tag_name((WS, "math")))
                    .collect::<Vec<_>>();
                let mut changed = false;
                for (index, (column, math)) in after.columns().iter().zip(columns).enumerate() {
                    let equal = before.columns().get(index).is_some_and(|original| {
                        original.header.tree == column.header.tree
                            && original.unit.as_ref().map(|c| &c.tree)
                                == column.unit.as_ref().map(|c| &c.tree)
                            && original
                                .cells
                                .iter()
                                .map(|c| &c.tree)
                                .eq(column.cells.iter().map(|c| &c.tree))
                    });
                    if !equal {
                        changed = true;
                        patches.extend(attrs(
                            math,
                            &wrapped,
                            &[
                                ("resultRef", Some(result_ids.allocate()?.to_string())),
                                ("error", None),
                            ],
                        )?);
                    }
                }
                if changed {
                    patches.extend(attrs(
                        node,
                        &wrapped,
                        &[("actualWidth", None), ("actualHeight", None)],
                    )?);
                }
            }
        }
        CanvasContent::Opaque { .. } => {}
    }
    let out = apply(wrapped, patches)?;
    Ok(out[start..out.len() - "</context>".len()].into())
}

pub(crate) fn bind_math(xml: &str) -> Result<String, String> {
    if let Ok(doc) = parse(xml)
        && doc.root_element().tag_name().namespace() == Some(ML)
    {
        return Ok(xml.into());
    }
    let end = xml
        .find(|c: char| c.is_whitespace() || c == '/' || c == '>')
        .ok_or("Math element missing")?;
    Ok(format!("{} xmlns:ml=\"{ML}\"{}", &xml[..end], &xml[end..]))
}

fn new_region(
    region: &CanvasRegion,
    id: &str,
    package: &mut NativePackage,
    worksheet: &str,
    result_ids: &mut ResultIds,
    derived: &mut text_source::DerivedTextEdits,
) -> Result<String, String> {
    if region.authored_text.is_some() {
        return authored_export::fresh_region(region, id, package, worksheet);
    }
    if matches!(region.content, CanvasContent::Text(_)) && region.rich_text.is_some() {
        return text_source::fresh_region(region, id, package, worksheet, derived);
    }
    let payload = match &region.content {
        CanvasContent::Math(tree) => format!(
            "<math resultRef=\"{}\"{}>{}{}</math>",
            result_ids.allocate()?,
            if region.calculation_disabled {
                " disable-calc=\"true\""
            } else {
                ""
            },
            bind_math(&tree_to_mathml(tree)?)?,
            crate::formatting::local_xml(&region.format)
        ),
        CanvasContent::Text(text) => format!(
            "<text item-idref=\"{}\"/>",
            add_text_part(package, worksheet, id, text)?
        ),
        CanvasContent::Plot(_) => plot_source::fresh(region, result_ids)?,
        CanvasContent::Opaque { xml, kind } if kind == "spec-table" => {
            crate::spec_table::SpecTable::parse(xml)?;
            let document = parse(xml)?;
            let table = document
                .descendants()
                .find(|n| n.has_tag_name((WS, "spec-table")))
                .ok_or("New specification table missing")?;
            let payload = crate::spec_table::standalone(table, xml)?;
            let document = parse(&payload)?;
            let mut patches = Vec::new();
            for math in document
                .root_element()
                .children()
                .filter(|n| n.has_tag_name((WS, "math")))
            {
                patches.extend(attrs(
                    math,
                    &payload,
                    &[
                        ("resultRef", Some(result_ids.allocate()?.to_string())),
                        ("error", None),
                    ],
                )?);
            }
            patches.extend(attrs(
                document.root_element(),
                &payload,
                &[(
                    "disable-calc",
                    Some(region.calculation_disabled.to_string()),
                )],
            )?);
            apply(payload, patches)?
        }
        CanvasContent::Opaque { xml, kind } if kind == "Area" => {
            if region.calculation_disabled {
                return Err("New Area parent disable-calc semantics are unsupported".into());
            }
            area_authoring::payload(xml, result_ids)?
        }
        CanvasContent::Opaque { xml, kind } if kind == "pageBreak" => {
            crate::page_break::is_page_break(region)?;
            if region.calculation_disabled || region.left != 0. {
                return Err("New page break must be enabled and at the worksheet left edge".into());
            }
            let source = parse(xml)?;
            let root = source.root_element();
            if root.attributes().any(|attribute| {
                attribute.namespace().is_some()
                    || !["region-id", "left", "top", "height"].contains(&attribute.name())
            }) || root
                .attribute("height")
                .and_then(|height| height.parse::<f64>().ok())
                != Some(2.)
                || root
                    .children()
                    .filter(Node::is_element)
                    .any(|node| !node.has_tag_name((WS, "pageBreak")))
            {
                return Err("New page-break metadata or geometry is unsupported".into());
            }
            "<pageBreak/>".into()
        }
        CanvasContent::Opaque { .. } => {
            return Err(
                "Adding opaque regions without original source context is unsupported".into(),
            );
        }
    };
    let area_height = if let CanvasContent::Opaque { xml, kind } = &region.content
        && kind == "Area"
    {
        let source = parse(xml)?;
        let mut dimensions = String::new();
        for name in ["height", "actualHeight"] {
            if let Some(height) = source.root_element().attribute(name) {
                if !height
                    .parse::<f64>()
                    .is_ok_and(|value| value.is_finite() && value >= 0.)
                {
                    return Err("Invalid new Area owning height".into());
                }
                dimensions.push_str(&format!(" {name}=\"{}\"", escape(height)));
            }
        }
        dimensions
    } else if matches!(&region.content,CanvasContent::Opaque{kind,..} if kind == "pageBreak") {
        " height=\"2\"".into()
    } else {
        String::new()
    };
    let plot_geometry = if let CanvasContent::Plot(spec) = &region.content {
        plot_source::geometry_attributes(spec)
            .into_iter()
            .filter_map(|(name, value)| value.map(|value| format!(" {name}=\"{value}\"")))
            .collect::<String>()
    } else {
        String::new()
    };
    Ok(format!(
        "<region xmlns=\"{WS}\" region-id=\"{id}\" left=\"{}\" top=\"{}\"{area_height}{plot_geometry}>{payload}</region>",
        region.left, region.top
    ))
}

fn relationship_part(worksheet: &str) -> Result<String, String> {
    let (folder, name) = worksheet
        .rsplit_once('/')
        .ok_or("Worksheet part lacks folder")?;
    Ok(format!("{folder}/_rels/{name}.rels"))
}

fn read_text(
    package: &NativePackage,
    worksheet: &str,
    id: Option<&str>,
) -> Result<Option<String>, String> {
    let Some(id) = id else { return Ok(None) };
    let rel_name = relationship_part(worksheet)?;
    let Some(bytes) = package.original_part(&rel_name) else {
        return Ok(None);
    };
    let xml = xml_text(bytes)?;
    let doc = parse(&xml)?;
    let Some(rel) = doc
        .root_element()
        .children()
        .find(|n| n.attribute("Id") == Some(id))
    else {
        return Ok(None);
    };
    if rel.attribute("TargetMode") == Some("External") {
        return Err("External text relationship is unsupported".into());
    }
    let target = rel.attribute("Target").ok_or("Missing text target")?;
    let part = if target.starts_with('/') {
        target.trim_start_matches('/').to_string()
    } else {
        format!("{}/{target}", worksheet.rsplit_once('/').unwrap().0)
    };
    let Some(bytes) = package.original_part(&part) else {
        return Ok(None);
    };
    let mut zip = ZipArchive::new(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let mut file = zip
        .by_name("Xaml/Document.xaml")
        .map_err(|e| e.to_string())?;
    if file.size() > 8 * 1024 * 1024 {
        return Err("Text package exceeds XML limit".into());
    }
    let mut bytes = Vec::new();
    file.by_ref()
        .take(8 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    let xml = xml_text(&bytes)?;
    let doc = parse(&xml)?;
    Ok(Some(
        doc.descendants()
            .filter(|n| n.is_element() && n.tag_name().name() == "Paragraph")
            .map(|p| {
                p.descendants()
                    .filter(Node::is_text)
                    .filter_map(|n| n.text())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n"),
    ))
}

fn zip_bytes(parts: &[(String, Vec<u8>)]) -> Result<Vec<u8>, String> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in parts {
        zip.start_file(
            name,
            SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
        )
        .map_err(|e| e.to_string())?;
        zip.write_all(bytes).map_err(|e| e.to_string())?;
    }
    Ok(zip.finish().map_err(|e| e.to_string())?.into_inner())
}

struct TextPartReservation {
    rel_name: String,
    rel_xml: String,
    name: String,
    rel_id: String,
}
fn reserve_text_part(
    package: &NativePackage,
    worksheet: &str,
    id: &str,
    strict_reserved: bool,
) -> Result<TextPartReservation, String> {
    let rel_name = relationship_part(worksheet)?;
    let rel_xml = package
        .original_part(&rel_name)
        .map(xml_text)
        .transpose()?
        .unwrap_or_else(|| format!("<Relationships xmlns=\"{REL}\"/>"));
    let rel_doc = parse(&rel_xml)?;
    let ids: HashSet<_> = rel_doc
        .root_element()
        .children()
        .filter_map(|n| n.attribute("Id"))
        .collect();
    let reserved = if strict_reserved {
        let types = package
            .original_xml_part("[Content_Types].xml")?
            .ok_or("Package content types are missing")?;
        let doc = parse(&types)?;
        if !doc.root_element().has_tag_name((CONTENT, "Types")) {
            return Err("Invalid content type root".into());
        }
        // Even a foreign/otherwise malformed declaration reserves its exact
        // PartName. An absent ZIP member must not make that identity reusable.
        doc.descendants()
            .filter_map(|n| n.attribute("PartName"))
            .map(|name| name.trim_start_matches('/').to_owned())
            .collect::<HashSet<_>>()
    } else {
        HashSet::new()
    };
    let mut seq = 0u64;
    let (name, rel_id) = loop {
        let name = format!("mathcad/xaml/MathcraftText{id}_{seq}.XamlPackage");
        let rel_id = format!("MathcraftText{id}_{seq}");
        if package.original_part(&name).is_none()
            && !ids.contains(rel_id.as_str())
            && !reserved.contains(&name)
        {
            break (name, rel_id);
        }
        seq = seq.checked_add(1).ok_or("Text identity space exhausted")?;
    };
    Ok(TextPartReservation {
        rel_name,
        rel_xml,
        name,
        rel_id,
    })
}
fn install_text_package_part(
    package: &mut NativePackage,
    reservation: TextPartReservation,
    nested: Vec<u8>,
) -> Result<String, String> {
    let TextPartReservation {
        rel_name,
        rel_xml,
        name,
        rel_id,
    } = reservation;
    package.set_part(&name, nested)?;
    set_xml(
        package,
        &rel_name,
        insert_child(
            &rel_xml,
            &format!(
                "<Relationship xmlns=\"{REL}\" Id=\"{rel_id}\" Type=\"{FLOW_REL}\" Target=\"/{name}\"/>"
            ),
        )?,
    )?;
    let types = xml_text(
        package
            .original_part("[Content_Types].xml")
            .ok_or("Package content types are missing")?,
    )?;
    set_xml(
        package,
        "[Content_Types].xml",
        insert_child(
            &types,
            &format!(
                "<Override xmlns=\"{CONTENT}\" PartName=\"/{name}\" ContentType=\"application/zip\"/>"
            ),
        )?,
    )?;
    Ok(rel_id)
}
/// Shared with the strictly validated native rich-text COW adapter. Ordinary
/// plain Text keeps its former reservation/error order and generated package.
fn add_text_package_part(
    package: &mut NativePackage,
    worksheet: &str,
    id: &str,
    nested: Vec<u8>,
    strict_reserved: bool,
) -> Result<String, String> {
    let reservation = reserve_text_part(package, worksheet, id, strict_reserved)?;
    install_text_package_part(package, reservation, nested)
}
/// Shared exact nested-XAML scaffolding; callers validate source beforehand.
/// No new ZIP builder/options/order or plain Text policy is introduced.
fn text_package_from_xaml(xaml: Vec<u8>) -> Result<Vec<u8>, String> {
    zip_bytes(&[
        ("Xaml/Document.xaml".into(), xaml),
        ("[Content_Types].xml".into(), format!("<Types xmlns=\"{CONTENT}\"><Default Extension=\"xaml\" ContentType=\"application/vnd.ms-wpf.xaml+xml\"/><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/></Types>").into_bytes()),
        ("_rels/.rels".into(), format!("<Relationships xmlns=\"{REL}\"><Relationship Id=\"MathcraftDocument\" Type=\"http://schemas.microsoft.com/wpf/2005/10/xaml/entry\" Target=\"/Xaml/Document.xaml\"/></Relationships>").into_bytes()),
    ])
}

fn add_text_part(
    package: &mut NativePackage,
    worksheet: &str,
    id: &str,
    text: &str,
) -> Result<String, String> {
    let reservation = reserve_text_part(package, worksheet, id, false)?;
    let mut xaml = "<FlowDocument xmlns=\"http://schemas.microsoft.com/winfx/2006/xaml/presentation\" xml:space=\"preserve\">".to_string();
    for paragraph in text.split('\n') {
        xaml.push_str(&format!(
            "<Paragraph><Run>{}</Run></Paragraph>",
            escape(paragraph)
        ));
    }
    xaml.push_str("</FlowDocument>");
    parse(&xaml)?;
    let nested = text_package_from_xaml(xaml.into_bytes())?;
    install_text_package_part(package, reservation, nested)
}

fn invalidate_caches(package: &mut NativePackage) -> Result<(), String> {
    let names: Vec<_> = package
        .parts()
        .iter()
        .filter(|p| p.name.to_ascii_lowercase().ends_with(".xml"))
        .map(|p| p.name.clone())
        .collect();
    for name in names {
        let text = xml_text(package.original_part(&name).unwrap())?;
        let doc = parse(&text)?;
        if !doc.root_element().has_tag_name((RESULTS, "resultsList")) {
            continue;
        }
        let mut patches = Vec::new();
        for result in doc
            .root_element()
            .children()
            .filter(|n| n.has_tag_name((RESULTS, "resultData")))
        {
            patches.extend(attrs(
                result,
                &text,
                &[("calculation-status", Some("Pending".into()))],
            )?);
            // Prime keeps saved display data while the row is Pending. The
            // status, rather than deleting values/errors/warnings/tag data,
            // prevents it from being consumed as a current computed value.
        }
        set_xml(package, &name, apply(text, patches)?)?;
    }
    Ok(())
}

fn append_pending_results(package: &mut NativePackage, ids: &[i32]) -> Result<(), String> {
    if ids.is_empty() {
        return Ok(());
    }
    let name = "mathcad/result.xml";
    let xml = package
        .original_part(name)
        .map(xml_text)
        .transpose()?
        .unwrap_or_else(|| format!("<resultsList xmlns=\"{RESULTS}\"/>"));
    if !parse(&xml)?
        .root_element()
        .has_tag_name((RESULTS, "resultsList"))
    {
        return Err("Result part path is occupied by an unrelated document".into());
    }
    let mut children = String::new();
    for id in ids {
        children.push_str(&format!(
            "<resultData xmlns=\"{RESULTS}\" result-id=\"{id}\" calculation-status=\"Pending\"/>"
        ));
    }
    let absent = package.original_part(name).is_none();
    set_xml(package, name, insert_child(&xml, &children)?)?;
    if absent {
        let types = xml_text(
            package
                .original_part("[Content_Types].xml")
                .ok_or("Missing package content types")?,
        )?;
        set_xml(
            package,
            "[Content_Types].xml",
            insert_child(
                &types,
                &format!(
                    "<Override xmlns=\"{CONTENT}\" PartName=\"/{name}\" ContentType=\"application/vnd.openxmlformats-officedocument.mathprocessingml.mathcad.result+xml\"/>"
                ),
            )?,
        )?;
    }
    Ok(())
}

fn minimal_package(doc: &CanvasDocument) -> Result<NativePackage, String> {
    if ![
        0.2 * 96.0,
        0.1 * 96.0,
        0.5 * 37.79527559055118,
        0.25 * 37.79527559055118,
    ]
    .iter()
    .any(|g| (doc.grid - g).abs() < 1e-8)
    {
        return Err("MCDX export supports the observed Standard/Fine Letter/A4 grids only".into());
    }
    let paper = if (doc.grid - 18.89763779527559).abs() < 1e-8
        || (doc.grid - 9.448818897637795).abs() < 1e-8
    {
        "A4"
    } else {
        "Letter"
    };
    let fine = doc.grid < 12.0;
    let overrides = [
        ("/mathcad/worksheet.xml", MAIN_TYPE),
        ("/mathcad/header.xml", MAIN_TYPE),
        ("/mathcad/footer.xml", MAIN_TYPE),
        (
            "/mathcad/settings/calculation.xml",
            "application/vnd.openxmlformats-officedocument.mathprocessingml.mathcad.settings.calculation+xml",
        ),
        (
            "/mathcad/settings/presentation.xml",
            "application/vnd.openxmlformats-officedocument.mathprocessingml.mathcad.settings.presentation+xml",
        ),
        (
            "/mathcad/result.xml",
            "application/vnd.openxmlformats-officedocument.mathprocessingml.mathcad.result+xml",
        ),
        (
            "/docProps/app.xml",
            "application/mathcad.extended-properties+xml",
        ),
        (
            "/docProps/core.xml",
            "application/vnd.openxmlformats-package.core-properties+xml",
        ),
    ];
    let mut types = format!(
        "<Types xmlns=\"{CONTENT}\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>"
    );
    for (name, kind) in overrides {
        types.push_str(&format!(
            "<Override PartName=\"{name}\" ContentType=\"{kind}\"/>"
        ));
    }
    types.push_str("</Types>");
    let mut rels = format!("<Relationships xmlns=\"{REL}\">");
    for (id, target) in [
        ("Worksheet", "worksheet"),
        ("Header", "header"),
        ("Footer", "footer"),
    ] {
        rels.push_str(&format!("<Relationship Id=\"Mathcraft{id}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"/mathcad/{target}.xml\"/>"));
    }
    rels.push_str("<Relationship Id=\"MathcraftCore\" Type=\"http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties\" Target=\"/docProps/core.xml\"/></Relationships>");
    let parts: Vec<(String, Vec<u8>)> = vec![
        ("[Content_Types].xml".into(), types.into_bytes()), ("_rels/.rels".into(), rels.into_bytes()),
        ("mathcad/worksheet.xml".into(), format!("<worksheet xmlns=\"{WS}\" xmlns:ml=\"{ML}\"><regions/></worksheet>").into_bytes()),
        ("mathcad/header.xml".into(), format!("<header xmlns=\"{WS}\"><regions/></header>").into_bytes()),
        ("mathcad/footer.xml".into(), format!("<footer xmlns=\"{WS}\"><regions/></footer>").into_bytes()),
        ("mathcad/result.xml".into(), format!("<resultsList xmlns=\"{RESULTS}\"/>").into_bytes()),
        ("mathcad/settings/calculation.xml".into(), b"<calculation xmlns=\"http://schemas.ptc.com/mathcad/settings/calculation10\"/>".to_vec()),
        ("mathcad/settings/presentation.xml".into(), format!("<presentation xmlns=\"http://schemas.ptc.com/mathcad/settings/presentation10\"><pageModel paper-code=\"{paper}\" grid-size=\"{}\" orientation=\"Portrait\" margin-type=\"Standard\"/><mathRendering><ws:results xmlns:ws=\"http://schemas.mathsoft.com/worksheet50\"><ws:matrix size=\"12,12\" offset=\"0,0\" show-indices=\"false\" expand-nested-arrays=\"false\"/></ws:results></mathRendering></presentation>", if fine { "Fine" } else { "Standard" }).into_bytes()),
        ("docProps/app.xml".into(), b"<properties xmlns=\"http://schemas.mathsoft.com/extended-properties\"><application>Mathcraft</application><appVersion>0.2.0</appVersion><serializationVersion>12.0.0.0</serializationVersion></properties>".to_vec()),
        ("docProps/core.xml".into(), format!("<cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><dc:title>{}</dc:title></cp:coreProperties>", escape(&doc.title)).into_bytes()),
    ];
    NativePackage::from_bytes(&zip_bytes(&parts)?)
}

#[cfg(test)]
mod tests {
    #[test]
    fn area_child_edits_allocate_fresh_result_ids_without_flattening_or_changing_siblings() {
        let source = format!(
            r#"<worksheet xmlns="{WS}" xmlns:ml="{ML}" xmlns:f="urn:future"><regions><region region-id="2" left="4" top="50" actualWidth="100" actualHeight="120"><Area AreaHeight="120"><regions><region region-id="3" left="19.2" top="9.6" actualWidth="30" actualHeight="20"><math resultRef="7" error="old"><ml:real>12</ml:real><formattingOverride FontSize="11"/><f:keep exact="yes"/></math></region><region region-id="4" left="40" top="9.6"><math resultRef="8"><ml:real>34</ml:real></math></region></regions></Area></region></regions></worksheet>"#
        );
        let results = format!(
            r#"<resultsList xmlns="{RESULTS}" xmlns:ml="{ML}"><resultData result-id="7" calculation-status="UpToDate"><ml:real>12</ml:real></resultData><resultData result-id="8" calculation-status="UpToDate"><ml:real>34</ml:real></resultData></resultsList>"#
        );
        for utf16 in [false, true] {
            let worksheet_bytes = if utf16 {
                [0xff, 0xfe]
                    .into_iter()
                    .chain(source.encode_utf16().flat_map(u16::to_le_bytes))
                    .collect()
            } else {
                source.as_bytes().to_vec()
            };
            let bytes = zip_bytes(&[
                ("mathcad/worksheet.xml".into(), worksheet_bytes),
                ("mathcad/result.xml".into(), results.as_bytes().to_vec()),
                ("custom/keep.bin".into(), vec![9, 8, 7]),
            ])
            .unwrap();
            let mut package = NativePackage::from_bytes(&bytes).unwrap();
            let doc = parse(&source).unwrap();
            let region = doc
                .descendants()
                .find(|n| n.attribute("region-id") == Some("2"))
                .unwrap();
            let fragment = crate::spec_table::standalone(region, &source).unwrap();
            let child = crate::area::area_math_children_xml(&fragment, &[])
                .unwrap()
                .remove(0);
            let edited = crate::area::replace_area_math_xml(
                &fragment,
                &[],
                &child.region_path,
                &format!("<ml:real xmlns:ml=\"{ML}\">99</ml:real>"),
            )
            .unwrap()
            .unwrap();
            let mut canvas = CanvasDocument::default();
            canvas.add_opaque(4., 50., &edited, "Area");
            canvas.regions[0].source_region = Some("2".into());
            export_into(&canvas, &mut package).unwrap();
            let out = package.worksheet_xml().unwrap();
            let parsed = parse(&out).unwrap();
            let changed = parsed
                .descendants()
                .find(|n| n.attribute("region-id") == Some("3"))
                .unwrap();
            assert_eq!(changed.attribute("left"), Some("19.2"));
            assert_eq!(changed.attribute("top"), Some("9.6"));
            assert!(changed.attribute("actualWidth").is_none());
            let math = changed
                .children()
                .find(|n| n.has_tag_name((WS, "math")))
                .unwrap();
            assert_eq!(math.attribute("resultRef"), Some("9"));
            assert!(math.attribute("error").is_none());
            assert!(out.contains("<f:keep exact=\"yes\"/>"));
            let unchanged = parsed
                .descendants()
                .find(|n| n.attribute("region-id") == Some("4"))
                .unwrap();
            assert_eq!(
                unchanged
                    .children()
                    .find(|n| n.has_tag_name((WS, "math")))
                    .unwrap()
                    .attribute("resultRef"),
                Some("8")
            );
            assert_eq!(
                parsed
                    .descendants()
                    .filter(|n| n.has_tag_name((WS, "region")))
                    .count(),
                3
            );
            let cached = package
                .original_xml_part("mathcad/result.xml")
                .unwrap()
                .unwrap();
            let cached = parse(&cached).unwrap();
            assert!(
                cached
                    .descendants()
                    .any(|n| n.attribute("result-id") == Some("9")
                        && n.attribute("calculation-status") == Some("Pending"))
            );
            assert_eq!(
                package.original_part("custom/keep.bin"),
                Some(&[9, 8, 7][..])
            );
            if utf16 {
                assert!(
                    package
                        .original_part("mathcad/worksheet.xml")
                        .unwrap()
                        .starts_with(&[0xff, 0xfe])
                );
            }
        }
    }

    #[test]
    fn new_spec_table_structural_commands_save_and_export_as_native_matrix_columns() {
        use crate::spec_table::{CellSlot, SpecTable};
        use mathcraft_editor::{NodeKind, PortableTree};
        let dir = Dir::new();
        let canvas_path = dir.path("new-table.mathcraft");
        let native_path = dir.path("new-table.mcdx");
        let mut table = SpecTable::new(1, 1).unwrap();
        table
            .set_cell(
                0,
                CellSlot::Header,
                &tree_from_mathml("<ml:id labels=\"VARIABLE\">lengths</ml:id>").unwrap(),
            )
            .unwrap();
        table
            .set_cell(
                0,
                CellSlot::Unit,
                &tree_from_mathml("<ml:id labels=\"UNIT\">mm</ml:id>").unwrap(),
            )
            .unwrap();
        table
            .set_cell(
                0,
                CellSlot::Body(0),
                &PortableTree {
                    kind: NodeKind::Number("1.00".into()),
                    children: vec![],
                },
            )
            .unwrap();
        table.insert_row(1).unwrap();
        table.insert_column(1).unwrap();
        table
            .set_cell(
                1,
                CellSlot::Header,
                &tree_from_mathml("<ml:id labels=\"VARIABLE\">currents</ml:id>").unwrap(),
            )
            .unwrap();
        table
            .set_cell(
                1,
                CellSlot::Unit,
                &tree_from_mathml("<ml:id labels=\"UNIT\">A</ml:id>").unwrap(),
            )
            .unwrap();
        let matrix = PortableTree {
            kind: NodeKind::Matrix {
                rows: 2,
                cols: 2,
                display: None,
            },
            children: ["1.00", "2", "3", "4"]
                .into_iter()
                .map(|v| PortableTree {
                    kind: NodeKind::Number(v.into()),
                    children: vec![],
                })
                .collect(),
        };
        table.paste_matrix(0, 0, &matrix).unwrap();
        let mut doc = CanvasDocument::default();
        let id = doc.add_opaque(57.6, 76.8, table.xml(), "spec-table");
        assert!(doc.regions[0].source_region.is_none());
        doc.save(&canvas_path).unwrap();
        let reopened = CanvasDocument::load(&canvas_path).unwrap();
        assert_eq!(doc, reopened);
        reopened.export_mcdx(&native_path).unwrap();
        let package = NativePackage::open(&native_path).unwrap();
        let source = package.worksheet_xml().unwrap();
        let xml = parse(&source).unwrap();
        let native_region = xml
            .descendants()
            .find(|n| n.has_tag_name((WS, "region")))
            .unwrap();
        assert_eq!(
            native_region.attribute("region-id"),
            Some(id.to_string().as_str())
        );
        assert_eq!(
            native_region.attribute("left"),
            Some(doc.regions[0].left.to_string().as_str())
        );
        let table = SpecTable::parse(&source).unwrap();
        assert_eq!((table.row_count(), table.col_count()), (2, 2));
        assert_eq!(table.copy_matrix(0..2, 0..2).unwrap(), matrix);
        assert_eq!(
            table.cell(0, CellSlot::Unit).unwrap(),
            tree_from_mathml("<ml:id labels=\"UNIT\">mm</ml:id>").unwrap()
        );
        let matrices = xml
            .descendants()
            .filter(|n| n.has_tag_name((ML, "matrix")))
            .collect::<Vec<_>>();
        assert_eq!(matrices.len(), 2);
        assert!(
            matrices
                .iter()
                .all(|n| n.attribute("rows") == Some("2") && n.attribute("cols") == Some("1"))
        );
        let references = xml
            .descendants()
            .filter(|n| n.has_tag_name((WS, "math")))
            .map(|n| n.attribute("resultRef").unwrap().to_string())
            .collect::<HashSet<_>>();
        assert_eq!(references.len(), 2);
        let results = package
            .original_xml_part("mathcad/result.xml")
            .unwrap()
            .unwrap();
        let results = parse(&results).unwrap();
        for id in references {
            assert!(
                results
                    .descendants()
                    .any(|n| n.attribute("result-id") == Some(id.as_str())
                        && n.attribute("calculation-status") == Some("Pending"))
            );
        }
    }

    #[test]
    fn spec_table_export_preserves_untouched_columns_and_invalidates_only_edited_column_cache() {
        use crate::spec_table::{CellSlot, SpecTable};
        use mathcraft_editor::{NodeKind, PortableTree};
        let region = format!(
            r#"<region xmlns="{WS}" xmlns:ml="{ML}" xmlns:f="urn:future" region-id="26" left="10" top="20" actualHeight="100"><spec-table><math resultRef="7"><ml:define><ml:id>A</ml:id><ml:apply><ml:scale/><ml:matrix rows="2" cols="1"><ml:real>1.00</ml:real><ml:placeholder/></ml:matrix><ml:id labels="UNIT">A</ml:id></ml:apply></ml:define><formattingOverride FontSize="11"/><f:keep exact=" yes "/></math><math resultRef="8"><ml:define><ml:id>B</ml:id><ml:matrix rows="1" cols="1"><ml:real>8.00</ml:real></ml:matrix></ml:define></math></spec-table></region>"#
        );
        let mut package = minimal_package(&CanvasDocument::default()).unwrap();
        package
            .set_part(
                "mathcad/worksheet.xml",
                format!("<worksheet xmlns=\"{WS}\"><regions>{region}</regions></worksheet>")
                    .into_bytes(),
            )
            .unwrap();
        package.set_part("mathcad/result.xml",format!("<resultsList xmlns=\"{RESULTS}\"><resultData result-id=\"7\" calculation-status=\"Synchronized\"/><resultData result-id=\"8\" calculation-status=\"Synchronized\"/></resultsList>").into_bytes()).unwrap();
        package.set_part("custom/keep.bin", vec![9, 8, 7]).unwrap();
        let mut table = SpecTable::parse(&region).unwrap();
        table
            .set_cell(
                0,
                CellSlot::Body(0),
                &PortableTree {
                    kind: NodeKind::Number("12.50".into()),
                    children: vec![],
                },
            )
            .unwrap();
        let doc = CanvasDocument {
            next_id: 2,
            regions: vec![CanvasRegion {
                id: 1,
                left: 10.,
                top: 20.,
                content: CanvasContent::Opaque {
                    kind: "spec-table".into(),
                    xml: table.xml().into(),
                },
                source_region: Some("26".into()),
                rich_text: None,
                authored_text: None,
                format: Default::default(),
                calculation_disabled: false,
                calculation_enabled_override: false,
            }],
            ..Default::default()
        };
        export_into(&doc, &mut package).unwrap();
        let source = package.worksheet_xml().unwrap();
        let parsed = parse(&source).unwrap();
        let columns = parsed
            .descendants()
            .filter(|n| n.has_tag_name((WS, "math")))
            .collect::<Vec<_>>();
        assert_ne!(columns[0].attribute("resultRef"), Some("7"));
        assert_eq!(columns[1].attribute("resultRef"), Some("8"));
        let updated = SpecTable::parse(&source).unwrap();
        assert_eq!(
            updated.cell(0, CellSlot::Body(0)).unwrap().kind,
            NodeKind::Number("12.50".into())
        );
        assert!(source.contains("<ml:real>8.00</ml:real>"));
        assert!(source.contains("<f:keep exact=\" yes \"/>"));
        assert!(source.contains("<formattingOverride FontSize=\"11\"/>"));
        assert_eq!(
            package.original_part("custom/keep.bin"),
            Some(&[9, 8, 7][..])
        );
        let results = package
            .original_xml_part("mathcad/result.xml")
            .unwrap()
            .unwrap();
        let result_doc = parse(&results).unwrap();
        assert!(result_doc.descendants().any(|n| n.attribute("result-id")
            == columns[0].attribute("resultRef")
            && n.attribute("calculation-status") == Some("Pending")));
    }

    use super::*;
    use mathcraft_editor::{NodeKind, PortableTree};
    use std::sync::atomic::{AtomicU64, Ordering};

    struct Dir(std::path::PathBuf);
    impl Dir {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let dir = std::env::temp_dir().join(format!(
                "mathcraft-export-{}-{stamp}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&dir).unwrap();
            Self(dir)
        }
        fn path(&self, name: &str) -> std::path::PathBuf {
            self.0.join(name)
        }
    }
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn real(value: &str) -> CanvasContent {
        CanvasContent::Math(PortableTree {
            kind: NodeKind::Number(value.into()),
            children: vec![],
        })
    }
    fn capture(path: &Path, name: &str) {
        if let Some(dir) = std::env::var_os("MATHCRAFT_EXPORT_TEST_CAPTURE") {
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::copy(path, Path::new(&dir).join(name)).unwrap();
        }
    }

    #[test]
    fn independent_new_package_has_structural_math_and_nested_xaml_text() {
        let dir = Dir::new();
        let path = dir.path("new.mcdx");
        let mut doc = CanvasDocument {
            title: "자체 생성 & math".into(),
            ..Default::default()
        };
        doc.add_math(19.2, 38.4);
        doc.regions[0].content = real("3.5");
        doc.add_text(100.0, 100.0, "한글 & <note>\nsecond line");
        export_mcdx(&doc, &path).unwrap();
        let package = NativePackage::open(&path).unwrap();
        let regions = package.regions().unwrap();
        assert_eq!(regions.len(), 2);
        let xml = package.worksheet_xml().unwrap();
        let parsed = parse(&xml).unwrap();
        let text = parsed
            .descendants()
            .find(|n| n.has_tag_name((WS, "text")))
            .unwrap();
        assert_eq!(
            read_text(
                &package,
                package.worksheet_part_name(),
                text.attribute("item-idref")
            )
            .unwrap()
            .as_deref(),
            Some("한글 & <note>\nsecond line")
        );
        assert!(
            package
                .original_part("docProps/app.xml")
                .unwrap()
                .windows(9)
                .any(|s| s == b"Mathcraft")
        );
        assert_eq!(package.caches().unwrap().len(), 1);
        assert_eq!(
            package.caches().unwrap()[0].calculation_status.as_deref(),
            Some("Pending")
        );
        capture(&path, "new.mcdx");
        assert!(export_mcdx(&doc, &path).is_err());
    }

    #[test]
    fn calculation_disable_exports_new_math_and_can_reenable_preserved_native_math() {
        let dir = Dir::new();
        let original = dir.path("disabled.mcdx");
        let mut document = CanvasDocument::default();
        document.add_math(0., 0.);
        document.regions[0].content = real("2");
        document.regions[0].calculation_disabled = true;
        export_mcdx(&document, &original).unwrap();
        let package = NativePackage::open(&original).unwrap();
        let source = package.worksheet_xml().unwrap();
        let parsed = parse(&source).unwrap();
        let math = parsed
            .descendants()
            .find(|n| n.has_tag_name((WS, "math")))
            .unwrap();
        assert_eq!(math.attribute("disable-calc"), Some("true"));
        let source_id = math
            .parent_element()
            .unwrap()
            .attribute("region-id")
            .unwrap()
            .to_owned();
        document.native_archive = Some(std::fs::read(&original).unwrap());
        document.regions[0].source_region = Some(source_id);
        document.regions[0].calculation_disabled = false;
        document.regions[0].content = real("3");
        let reenabled = dir.path("reenabled.mcdx");
        export_mcdx(&document, &reenabled).unwrap();
        let source = NativePackage::open(&reenabled)
            .unwrap()
            .worksheet_xml()
            .unwrap();
        let parsed = parse(&source).unwrap();
        let math = parsed
            .descendants()
            .find(|n| n.has_tag_name((WS, "math")))
            .unwrap();
        assert_eq!(math.attribute("disable-calc"), Some("false"));
        assert_eq!(
            math.descendants()
                .find(|n| n.has_tag_name((ML, "real")))
                .unwrap()
                .text(),
            Some("3")
        );
    }

    #[test]
    fn page_setup_exports_exact_paper_orientation_and_custom_thickness() {
        let dir = Dir::new();
        let path = dir.path("landscape.mcdx");
        let paper = crate::canvas::PaperSize::Legal;
        let [width, height] = paper.size(true);
        let mut doc = CanvasDocument {
            width,
            height,
            page_setup: Some(crate::canvas::PageSetup {
                paper,
                landscape: true,
                margins: [36., 48., 60., 72.],
            }),
            ..Default::default()
        };
        doc.add_text(19.2, 19.2, "Landscape worksheet");
        export_mcdx(&doc, &path).unwrap();
        capture(&path, "page-setup.mcdx");
        let package = NativePackage::open(&path).unwrap();
        let source = package
            .original_xml_part("mathcad/settings/presentation.xml")
            .unwrap()
            .unwrap();
        let parsed = parse(&source).unwrap();
        let model = parsed
            .descendants()
            .find(|n| n.tag_name().name() == "pageModel")
            .unwrap();
        assert_eq!(model.attribute("paper-code"), Some("Legal"));
        assert_eq!(model.attribute("orientation"), Some("Landscape"));
        assert_eq!(model.attribute("margin-type"), Some("Custom"));
        assert_eq!(model.attribute("page-margin"), Some("36,48,60,72"));
        assert!(
            (model
                .attribute("page-width")
                .unwrap()
                .parse::<f64>()
                .unwrap()
                - 1008.)
                .abs()
                < 0.001
        );
        assert!(
            (model
                .attribute("page-height")
                .unwrap()
                .parse::<f64>()
                .unwrap()
                - 612.)
                .abs()
                < 0.001
        );
    }

    #[test]
    fn positioned_header_footer_fields_and_page_numbering_export_to_native_parts() {
        let dir = Dir::new();
        let path = dir.path("page-sections.mcdx");
        let mut doc = CanvasDocument::default();
        doc.add_header_text(19.2, 9.6, "Project Atlas");
        doc.add_header_field(500.0, 9.6, PageField::FileName);
        doc.add_header_field(250.0, 9.6, PageField::FilePath);
        doc.add_footer_field(
            480.0,
            9.6,
            PageField::PageNumber(PageNumberFormat::PageNumberOfTotal),
        );
        doc.add_footer_field(19.2, 9.6, PageField::DateSaved(DateSavedFormat::SystemLong));
        doc.page_sections.starting_page_number = 7;
        doc.page_sections.total_pages_offset = 2;
        export_mcdx(&doc, &path).unwrap();

        let package = NativePackage::open(&path).unwrap();
        let header = package
            .original_xml_part("mathcad/header.xml")
            .unwrap()
            .unwrap();
        let header_doc = parse(&header).unwrap();
        let header_regions: Vec<_> = header_doc
            .descendants()
            .filter(|n| n.has_tag_name((WS, "region")))
            .collect();
        assert_eq!(header_regions.len(), 3);
        let text = header_doc
            .descendants()
            .find(|n| n.has_tag_name((WS, "text")))
            .unwrap();
        assert_eq!(
            read_text(&package, "mathcad/header.xml", text.attribute("item-idref"))
                .unwrap()
                .as_deref(),
            Some("Project Atlas")
        );
        assert!(header.contains("<fileName"));
        assert!(header.contains("<filePath"));

        let footer = package
            .original_xml_part("mathcad/footer.xml")
            .unwrap()
            .unwrap();
        assert!(footer.contains("template=\"Page_@PageNo_of_@PagesTotal\""));
        assert!(footer.contains("template=\"System_Long\""));
        let presentation = package
            .original_xml_part("mathcad/settings/presentation.xml")
            .unwrap()
            .unwrap();
        let presentation_doc = parse(&presentation).unwrap();
        let model = presentation_doc
            .descendants()
            .find(|n| n.tag_name().name() == "pageModel")
            .unwrap();
        assert_eq!(model.attribute("starting-page-number"), Some("7"));
        assert_eq!(model.attribute("total-pages-offset"), Some("2"));
        capture(&path, "page-sections.mcdx");
    }

    #[test]
    fn source_owned_page_sections_remain_byte_exact_until_explicitly_edited() {
        let dir = Dir::new();
        let source_path = dir.path("page-source.mcdx");
        let out = dir.path("page-source-edited.mcdx");
        let mut package = minimal_package(&CanvasDocument::default()).unwrap();
        let original_header = format!(
            "<header xmlns=\"{WS}\"><regions><!-- source-owned --><region region-id=\"91\" left=\"1\" top=\"2\"><fieldText><text/><filePath/></fieldText></region></regions></header>"
        );
        package
            .set_part("mathcad/header.xml", original_header.clone().into_bytes())
            .unwrap();
        package.write_new(&source_path).unwrap();
        let mut doc = CanvasDocument {
            native_archive: Some(std::fs::read(&source_path).unwrap()),
            ..Default::default()
        };
        doc.add_text(0.0, 0.0, "body edit");
        assert!(!doc.page_sections.explicit);
        export_mcdx(&doc, &out).unwrap();
        let result = NativePackage::open(&out).unwrap();
        assert_eq!(
            result.original_part("mathcad/header.xml").unwrap(),
            original_header.as_bytes()
        );
    }

    #[test]
    fn source_export_updates_math_position_text_add_delete_and_invalidates_all_results() {
        let dir = Dir::new();
        let source_path = dir.path("source.mcdx");
        let out = dir.path("edited.mcdx");
        let mut package = minimal_package(&CanvasDocument::default()).unwrap();
        let text_ref =
            add_text_part(&mut package, "mathcad/worksheet.xml", "2", "old text").unwrap();
        let area = "<ws:region region-id=\"3\" left=\"0\" top=\"80\" tag=\"keep\"><ws:Area><ws:regions><ws:region region-id=\"90\" left=\"19.2\" top=\"19.2\"><ws:math><ml:real>17</ml:real></ws:math></ws:region></ws:regions></ws:Area></ws:region>";
        let worksheet = format!(
            "<ws:worksheet xmlns:ws=\"{WS}\" xmlns:ml=\"{ML}\"><ws:regions><!-- untouched -->\
            <ws:region region-id=\"1\" left=\"0\" top=\"0\" actualWidth=\"100\" actualHeight=\"25\" tag=\"keep-math\"><ws:math resultRef=\"7\"><ml:real>2</ml:real></ws:math></ws:region>\
            <ws:region region-id=\"2\" left=\"0\" top=\"40\"><ws:text item-idref=\"{text_ref}\"/></ws:region>{area}\
            <ws:region region-id=\"9\" left=\"0\" top=\"160\"><ws:math><ml:real>999</ml:real></ws:math></ws:region>\
            </ws:regions></ws:worksheet>"
        );
        package
            .set_part("mathcad/worksheet.xml", worksheet.into_bytes())
            .unwrap();
        let cache = format!(
            "<resultsList xmlns=\"{RESULTS}\" xmlns:ml=\"{ML}\" xmlns:u=\"urn:unknown\"><resultData result-id=\"7\" calculation-status=\"Synchronized\"><ml:result><ml:real>99</ml:real></ml:result><u:future keep=\"yes\"/></resultData></resultsList>"
        );
        package
            .set_part("mathcad/result.xml", cache.clone().into_bytes())
            .unwrap();
        package
            .set_part("custom/second-results.xml", cache.into_bytes())
            .unwrap();
        package
            .set_part("custom/unknown.bin", vec![0, 255, 18, 42])
            .unwrap();
        package
            .set_part(
                "custom/unknown.xml",
                b"<future exact=' keep  '><!-- opaque --></future>".to_vec(),
            )
            .unwrap();
        package.write_new(&source_path).unwrap();
        let original = std::fs::read(&source_path).unwrap();
        let untouched_relationships = package
            .original_part("mathcad/_rels/worksheet.xml.rels")
            .unwrap()
            .to_vec();
        let mut doc = CanvasDocument {
            native_archive: Some(original.clone()),
            next_id: 5,
            regions: vec![
                CanvasRegion {
                    id: 1,
                    left: 57.6,
                    top: 76.8,
                    content: real("5"),
                    source_region: Some("1".into()),
                    rich_text: None,
                    authored_text: None,
                    format: Default::default(),
                    calculation_disabled: false,
                    calculation_enabled_override: false,
                },
                CanvasRegion {
                    id: 2,
                    left: 0.0,
                    top: 100.0,
                    content: CanvasContent::Text("new 한글 & text".into()),
                    source_region: Some("2".into()),
                    rich_text: None,
                    authored_text: None,
                    format: Default::default(),
                    calculation_disabled: false,
                    calculation_enabled_override: false,
                },
                CanvasRegion {
                    id: 3,
                    left: 0.0,
                    top: 150.0,
                    content: CanvasContent::Opaque {
                        xml: area.into(),
                        kind: "Area".into(),
                    },
                    source_region: Some("3".into()),
                    rich_text: None,
                    authored_text: None,
                    format: Default::default(),
                    calculation_disabled: false,
                    calculation_enabled_override: false,
                },
                CanvasRegion {
                    id: 4,
                    left: 200.0,
                    top: 200.0,
                    content: real("11"),
                    source_region: None,
                    rich_text: None,
                    authored_text: None,
                    format: Default::default(),
                    calculation_disabled: false,
                    calculation_enabled_override: false,
                },
            ],
            ..Default::default()
        };
        export_mcdx(&doc, &out).unwrap();
        let saved = NativePackage::open(&out).unwrap();
        assert_eq!(
            saved.original_part("custom/unknown.bin"),
            Some(&[0, 255, 18, 42][..])
        );
        assert_eq!(
            saved.original_part("custom/unknown.xml"),
            package.original_part("custom/unknown.xml")
        );
        let rels = xml_text(
            saved
                .original_part("mathcad/_rels/worksheet.xml.rels")
                .unwrap(),
        )
        .unwrap();
        let old_rels = xml_text(&untouched_relationships).unwrap();
        assert!(
            rels.contains(
                &old_rels[old_rels.find('>').unwrap() + 1..old_rels.rfind("</").unwrap()]
            )
        );
        let regions = saved.regions().unwrap();
        assert!(regions.iter().any(|r| r.region_id.as_deref() == Some("90")));
        assert!(!regions.iter().any(|r| r.region_id.as_deref() == Some("9")));
        assert!(regions.iter().any(|r| r.region_id.as_deref() == Some("91")));
        let first = regions
            .iter()
            .find(|r| r.region_id.as_deref() == Some("1"))
            .unwrap();
        assert_eq!(first.left, Some(57.6));
        assert!(first.raw_xml.contains("keep-math"));
        assert!(!first.raw_xml.contains("actualWidth"));
        assert!(first.raw_xml.contains(">5</ml:real>"));
        assert!(
            saved
                .caches()
                .unwrap()
                .iter()
                .all(|c| c.calculation_status.as_deref() == Some("Pending"))
        );
        assert!(
            saved
                .original_xml_part("mathcad/result.xml")
                .unwrap()
                .unwrap()
                .contains("<ml:real>99</ml:real>")
        );
        // Two historical duplicate-ID rows remain inert, plus fresh references
        // for changed Math, the formerly unreferenced Area child and new Math.
        assert_eq!(saved.caches().unwrap().len(), 5);
        assert_eq!(
            first.math.as_ref().unwrap().result_ref.as_deref(),
            Some("8")
        );
        assert!(
            xml_text(saved.original_part("custom/second-results.xml").unwrap())
                .unwrap()
                .contains("u:future")
        );
        assert_eq!(std::fs::read(&source_path).unwrap(), original);
        assert!(export_mcdx(&doc, &source_path).is_err());
        doc.regions[0].source_region = Some("missing".into());
        assert!(export_mcdx(&doc, &dir.path("bad.mcdx")).is_err());
        capture(&out, "edited.mcdx");
    }

    #[test]
    fn unchanged_text_keeps_rich_payload_and_existing_relationship_part_exact() {
        let dir = Dir::new();
        let first = dir.path("source.mcdx");
        let out = dir.path("unchanged.mcdx");
        let mut source = CanvasDocument::default();
        source.add_text(20.0, 20.0, "text");
        export_mcdx(&source, &first).unwrap();
        let bytes = std::fs::read(&first).unwrap();
        let original = NativePackage::from_bytes(&bytes).unwrap();
        source.native_archive = Some(bytes);
        source.regions[0].source_region = Some("1".into());
        export_mcdx(&source, &out).unwrap();
        let saved = NativePackage::open(&out).unwrap();
        for part in original
            .parts()
            .iter()
            .filter(|p| p.name.ends_with("XamlPackage") || p.name.ends_with(".rels"))
        {
            assert_eq!(saved.original_part(&part.name), Some(part.bytes()));
        }
    }

    #[test]
    fn supported_source_edit_preserves_math_display_siblings_and_utf16() {
        let dir = Dir::new();
        let first = dir.path("first.mcdx");
        let source_path = dir.path("source.mcdx");
        let out = dir.path("edited-valid.mcdx");
        let mut doc = CanvasDocument::default();
        doc.add_math(20.0, 20.0);
        doc.regions[0].content = real("2");
        export_mcdx(&doc, &first).unwrap();
        let mut source = NativePackage::open(&first).unwrap();
        let xml = source.worksheet_xml().unwrap().replace(
            "</math>",
            "<resultFormat><general/></resultFormat><formattingOverride FontSize=\"14\"/></math>",
        );
        let utf16: Vec<_> = [0xff, 0xfe]
            .into_iter()
            .chain(xml.encode_utf16().flat_map(u16::to_le_bytes))
            .collect();
        source.set_part("mathcad/worksheet.xml", utf16).unwrap();
        source
            .set_part("custom/opaque.bin", vec![0, 255, 17])
            .unwrap();
        source.write_new(&source_path).unwrap();
        doc.native_archive = Some(std::fs::read(&source_path).unwrap());
        doc.regions[0].source_region = Some("1".into());
        doc.regions[0].content = real("5");
        doc.add_text(20.0, 80.0, "새 메모");
        export_mcdx(&doc, &out).unwrap();
        let result = NativePackage::open(&out).unwrap();
        assert!(
            result
                .original_part("mathcad/worksheet.xml")
                .unwrap()
                .starts_with(&[0xff, 0xfe])
        );
        let xml = result.worksheet_xml().unwrap();
        assert!(xml.contains(
            "<resultFormat><general/></resultFormat><formattingOverride FontSize=\"14\"/>"
        ));
        assert!(xml.contains(">5</ml:real>"));
        assert_eq!(
            result.original_part("custom/opaque.bin"),
            Some(&[0, 255, 17][..])
        );
        assert_eq!(result.caches().unwrap().len(), 2);
        capture(&out, "edited-valid.mcdx");
    }
    #[test]
    fn number_format_and_font_overrides_export_and_edit_without_losing_other_properties() {
        let dir = Dir::new();
        let first = dir.path("formatted.mcdx");
        let source_path = dir.path("formatted-source.mcdx");
        let edited = dir.path("formatted-edited.mcdx");
        let mut doc = CanvasDocument::default();
        doc.add_math(20., 20.);
        doc.regions[0].content = real("2");
        doc.regions[0].format = crate::canvas::MathFormat {
            font_points: 18.,
            result_format: crate::canvas::ResultFormat::Scientific,
            precision: 6,
            explicit: true,
            ..Default::default()
        };
        export_mcdx(&doc, &first).unwrap();
        let mut source = NativePackage::open(&first).unwrap();
        let xml = source.worksheet_xml().unwrap();
        let parsed = parse(&xml).unwrap();
        let math = parsed
            .descendants()
            .find(|n| n.has_tag_name((WS, "math")))
            .unwrap();
        assert_eq!(
            crate::formatting::read_math_format(math),
            doc.regions[0].format
        );
        let xml = xml
            .replace("<scientific ", "<scientific zero-threshold=\"12\" ")
            .replace("FontSize=\"18\"", "FontSize=\"18\" Bold=\"true\"")
            .replace("left=\"", "actualWidth=\"100\" actualHeight=\"30\" left=\"");
        source
            .set_part("mathcad/worksheet.xml", xml.into_bytes())
            .unwrap();
        source.write_new(&source_path).unwrap();
        doc.native_archive = Some(std::fs::read(&source_path).unwrap());
        doc.regions[0].source_region = Some("1".into());
        doc.regions[0].format.zero_threshold = 12;
        doc.regions[0].format.precision = 8;
        doc.regions[0].format.font_points = 24.;
        doc.regions[0].content = real("3");
        export_mcdx(&doc, &edited).unwrap();
        let output = NativePackage::open(&edited)
            .unwrap()
            .worksheet_xml()
            .unwrap();
        assert!(output.contains("precision=\"8\""));
        assert!(output.contains("FontSize=\"24\""));
        assert!(output.contains("Bold=\"true\""));
        assert!(output.contains("zero-threshold=\"12\""));
        assert!(!output.contains("actualWidth"));
        assert!(output.contains(">3</ml:real>"));
        capture(&edited, "formatted-edited.mcdx");
    }
    #[test]
    fn percentage_complex_edits_obey_fixed_radix_and_preserve_matrix_and_font_options() {
        use crate::canvas::{ComplexFormat, MathFormat, ResultFormat};
        let dir = Dir::new();
        let first = dir.path("format-general.mcdx");
        let archived = dir.path("format-preserved-source.mcdx");
        let edited = dir.path("format-percent.mcdx");
        let mut doc = CanvasDocument::default();
        doc.add_math(0., 0.);
        doc.regions[0].content = real("0.94359");
        doc.regions[0].format = MathFormat {
            explicit: true,
            complex_format: ComplexFormat::PolarDegrees,
            zero_threshold: 12,
            ..Default::default()
        };
        export_mcdx(&doc, &first).unwrap();
        let mut source = NativePackage::open(&first).unwrap();
        let xml = source.worksheet_xml().unwrap()
            .replace("radix=\"dec\"", "radix=\"hex\"")
            .replace("</resultFormat>", "<matrix size=\"7,8\" offset=\"0,0\" show-indices=\"true\" expand-nested-arrays=\"false\"/></resultFormat>")
            .replace("FontSize=\"11\"", "FontSize=\"11\" Bold=\"true\"");
        source
            .set_part("mathcad/worksheet.xml", xml.into_bytes())
            .unwrap();
        source.write_new(&archived).unwrap();
        doc.native_archive = Some(std::fs::read(&archived).unwrap());
        doc.regions[0].source_region = Some("1".into());
        doc.regions[0].format = MathFormat {
            result_format: ResultFormat::Percent,
            complex_format: ComplexFormat::J,
            precision: 7,
            show_trailing_zeros: true,
            font_points: 20.,
            ..doc.regions[0].format
        };
        export_mcdx(&doc, &edited).unwrap();
        let output = NativePackage::open(&edited)
            .unwrap()
            .worksheet_xml()
            .unwrap();
        let parsed = parse(&output).unwrap();
        let math = parsed
            .descendants()
            .find(|n| n.has_tag_name((WS, "math")))
            .unwrap();
        let percent = math
            .descendants()
            .find(|n| n.has_tag_name((WS, "percent")))
            .unwrap();
        assert_eq!(percent.attribute("precision"), Some("7"));
        assert_eq!(percent.attribute("show-trailing-zeros"), Some("true"));
        assert_eq!(percent.attribute("imaginary-value"), Some("j"));
        assert_eq!(percent.attribute("radix"), Some("dec"));
        assert_eq!(percent.attribute("zero-threshold"), Some("12"));
        assert!(percent.attribute("exponential-threshold").is_none());
        assert!(percent.attribute("complex-threshold").is_none());
        assert!(output.contains("size=\"7,8\""));
        assert!(output.contains("Bold=\"true\""));
        assert!(output.contains("FontSize=\"20\""));
        capture(&edited, "percent-complex-edited.mcdx");
    }

    #[test]
    fn unrelated_format_edits_preserve_roundtrip_precision_and_unsupported_fraction() {
        use crate::canvas::{ComplexFormat, MathFormat};
        let source = format!(
            "<resultFormat xmlns=\"{WS}\"><general precision=\"round-trip-decimal-places\" radix=\"bin\" imaginary-value=\"polarDegrees\"/><matrix size=\"4,5\"/></resultFormat>"
        );
        let parsed = parse(&source).unwrap();
        let number = crate::formatting::number_node(parsed.root_element()).unwrap();
        let original = MathFormat {
            explicit: true,
            complex_format: ComplexFormat::PolarDegrees,
            ..Default::default()
        };
        let updated = MathFormat {
            show_trailing_zeros: true,
            ..original
        };
        let output = apply(
            source.clone(),
            number_format_patches(number, &source, &original, &updated).unwrap(),
        )
        .unwrap();
        assert!(output.contains("precision=\"round-trip-decimal-places\""));
        assert!(output.contains("radix=\"bin\""));
        assert!(output.contains("imaginary-value=\"polarDegrees\""));
        assert!(output.contains("size=\"4,5\""));
        let fraction = source
            .replace("general", "fraction")
            .replace("radix=\"bin\"", "radix=\"dec\" use-mixed-numbers=\"true\"");
        let parsed = parse(&fraction).unwrap();
        let number = crate::formatting::number_node(parsed.root_element()).unwrap();
        let updated = MathFormat {
            complex_format: ComplexFormat::J,
            ..original
        };
        let output = apply(
            fraction.clone(),
            number_format_patches(number, &fraction, &original, &updated).unwrap(),
        )
        .unwrap();
        assert!(output.contains("<fraction "));
        assert!(output.contains("precision=\"round-trip-decimal-places\""));
        assert!(output.contains("radix=\"dec\""));
        assert!(output.contains("imaginary-value=\"j\""));
        assert!(!output.contains("show-trailing-zeros"));
        assert!(output.contains("use-mixed-numbers=\"true\""));
        let updated = MathFormat {
            result_format: crate::canvas::ResultFormat::Percent,
            ..original
        };
        let output = apply(
            fraction.clone(),
            number_format_patches(number, &fraction, &original, &updated).unwrap(),
        )
        .unwrap();
        assert!(output.contains("<percent "));
        assert!(!output.contains("use-mixed-numbers"));
    }

    #[test]
    fn plot_export_failure_does_not_create_a_partial_native_document() {
        let dir = Dir::new();
        let path = dir.path("plot.mcdx");
        let mut doc = CanvasDocument::default();
        doc.add_math(0., 0.);
        doc.add_plot(0., 100.);
        let CanvasContent::Plot(spec) = &mut doc.regions[1].content else {
            unreachable!()
        };
        spec.add_trace(crate::canvas::PlotTraceSpec {
            y_expression: "cos(x)".into(),
            label: "second trace".into(),
            ..Default::default()
        })
        .unwrap();
        assert!(export_mcdx(&doc, &path).is_err());
        assert!(!path.exists());
    }
}

#[cfg(test)]
mod plot_result_updates_tests;

#[cfg(test)]
mod plot_geometry_tests;
