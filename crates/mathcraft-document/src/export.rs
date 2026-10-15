//! Independent Mathcad package export for the supported flat canvas subset.
//! Untouched parts survive source-based export; cached computations are invalidated.
//! Schema checks do not establish that Prime can reopen a generated file.
use crate::{
    canvas::{CanvasContent, CanvasDocument, CanvasRegion},
    native::{NativePackage, xml_text},
};
use mathcraft_compat::editor_bridge::{tree_from_mathml, tree_to_mathml};
use roxmltree::{Document, Node};
use std::{
    collections::{HashMap, HashSet},
    io::{Cursor, Read, Write},
    ops::Range,
    path::Path,
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

struct ResultIds {
    next: i32,
    pending: Vec<i32>,
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
}

/// Writes a new .mcdx, refusing every existing destination, including originals.
/// Source region references must be the original top-level region-id strings.
pub fn export_mcdx(doc: &CanvasDocument, path: &Path) -> Result<(), String> {
    doc.validate()?;
    let mut package = match &doc.native_archive {
        Some(bytes) => NativePackage::from_bytes(bytes)?,
        None => minimal_package(doc)?,
    };
    export_into(doc, &mut package)?;
    package.write_new(path)
}

impl CanvasDocument {
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

fn set_xml(package: &mut NativePackage, name: &str, text: String) -> Result<(), String> {
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

fn export_into(canvas: &CanvasDocument, package: &mut NativePackage) -> Result<(), String> {
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
    let mut result_ids = ResultIds {
        next: package
            .caches()?
            .iter()
            .filter_map(|c| c.result_id.as_deref()?.parse::<i32>().ok())
            .chain(
                native_regions
                    .iter()
                    .filter_map(|r| r.math.as_ref()?.result_ref.as_deref()?.parse::<i32>().ok()),
            )
            .max()
            .unwrap_or(0)
            .max(0)
            .checked_add(1)
            .ok_or("Result ID space exhausted")?,
        pending: vec![],
    };
    for region in &canvas.regions {
        if let Some(id) = &region.source_region
            && by_source.insert(id.as_str(), region).is_some()
        {
            return Err("Duplicate source region references".into());
        }
    }
    let mut next = native_regions
        .iter()
        .filter_map(|r| r.region_id.as_deref()?.parse::<u64>().ok())
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or("Native region ID space exhausted")?;
    let mut changes = Vec::new();
    for old in original {
        let id = old
            .attribute("region-id")
            .ok_or("Original region has no ID")?;
        let text = if let Some(new) = by_source.remove(id) {
            edit_region(old, &source, new, id, package, &name, &mut result_ids)?
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
        let id = next.to_string();
        next = next
            .checked_add(1)
            .ok_or("Native region ID space exhausted")?;
        additions.push_str(&new_region(region, &id, package, &name, &mut result_ids)?);
    }
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
    set_xml(package, &name, apply(source, changes)?)?;
    invalidate_caches(package)?;
    append_pending_results(package, &result_ids.pending)?;
    Ok(())
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

fn edit_region(
    old: Node<'_, '_>,
    source: &str,
    region: &CanvasRegion,
    id: &str,
    package: &mut NativePackage,
    worksheet: &str,
    result_ids: &mut ResultIds,
) -> Result<String, String> {
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
                let original_format = crate::formatting::read_math_format(math);
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
                if region.format.precision != original_format.precision
                    || region.format.result_format != original_format.result_format
                {
                    if let Some(format) = math
                        .children()
                        .find(|n| n.has_tag_name((WS, "resultFormat")))
                    {
                        let old_number = format.children().find(|n| {
                            n.tag_name().namespace() == Some(WS)
                                && matches!(
                                    n.tag_name().name(),
                                    "general" | "decimal" | "scientific" | "engineering"
                                )
                        });
                        if let Some(number) = old_number {
                            if number.tag_name().name()
                                == crate::formatting::style_name(region.format.result_format)
                            {
                                patches.extend(attrs(
                                    number,
                                    &wrapped,
                                    &[("precision", Some(region.format.precision.to_string()))],
                                )?);
                            } else {
                                patches.push((
                                    number.range(),
                                    crate::formatting::number_xml(&region.format),
                                ));
                            }
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
            let original_format = crate::formatting::read_math_format(math);
            let format_changed = region.format.explicit
                && (region.format.font_points != original_format.font_points
                    || region.format.precision != original_format.precision
                    || region.format.result_format != original_format.result_format);
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
            }
        }
        CanvasContent::Text(text) => {
            let old_text = node
                .children()
                .find(|n| n.has_tag_name((WS, "text")))
                .ok_or("Cannot change a non-text source region to text")?;
            if read_text(package, worksheet, old_text.attribute("item-idref"))?.as_deref()
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
            return Err(
                "XY plot MCDX export is not implemented; save this worksheet as .mathcraft".into(),
            );
        }
        CanvasContent::Opaque { .. } => {}
    }
    let out = apply(wrapped, patches)?;
    Ok(out[start..out.len() - "</context>".len()].into())
}

fn bind_math(xml: &str) -> Result<String, String> {
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
) -> Result<String, String> {
    let payload = match &region.content {
        CanvasContent::Math(tree) => format!(
            "<math resultRef=\"{}\">{}{}</math>",
            result_ids.allocate()?,
            bind_math(&tree_to_mathml(tree)?)?,
            crate::formatting::local_xml(&region.format)
        ),
        CanvasContent::Text(text) => format!(
            "<text item-idref=\"{}\"/>",
            add_text_part(package, worksheet, id, text)?
        ),
        CanvasContent::Plot(_) => {
            return Err(
                "XY plot MCDX export is not implemented; save this worksheet as .mathcraft".into(),
            );
        }
        CanvasContent::Opaque { .. } => {
            return Err(
                "Adding opaque regions without original source context is unsupported".into(),
            );
        }
    };
    Ok(format!(
        "<region xmlns=\"{WS}\" region-id=\"{id}\" left=\"{}\" top=\"{}\">{payload}</region>",
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

fn add_text_part(
    package: &mut NativePackage,
    worksheet: &str,
    id: &str,
    text: &str,
) -> Result<String, String> {
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
    let mut seq = 0u64;
    let (name, rel_id) = loop {
        let name = format!("mathcad/xaml/MathcraftText{id}_{seq}.XamlPackage");
        let rel_id = format!("MathcraftText{id}_{seq}");
        if package.original_part(&name).is_none() && !ids.contains(rel_id.as_str()) {
            break (name, rel_id);
        }
        seq = seq.checked_add(1).ok_or("Text identity space exhausted")?;
    };
    let mut xaml = "<FlowDocument xmlns=\"http://schemas.microsoft.com/winfx/2006/xaml/presentation\" xml:space=\"preserve\">".to_string();
    for paragraph in text.split('\n') {
        xaml.push_str(&format!(
            "<Paragraph><Run>{}</Run></Paragraph>",
            escape(paragraph)
        ));
    }
    xaml.push_str("</FlowDocument>");
    parse(&xaml)?;
    let nested = zip_bytes(&[
        ("Xaml/Document.xaml".into(), xaml.into_bytes()),
        ("[Content_Types].xml".into(), format!("<Types xmlns=\"{CONTENT}\"><Default Extension=\"xaml\" ContentType=\"application/vnd.ms-wpf.xaml+xml\"/><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/></Types>").into_bytes()),
        ("_rels/.rels".into(), format!("<Relationships xmlns=\"{REL}\"><Relationship Id=\"MathcraftDocument\" Type=\"http://schemas.microsoft.com/wpf/2005/10/xaml/entry\" Target=\"/Xaml/Document.xaml\"/></Relationships>").into_bytes()),
    ])?;
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
            for old in result.children().filter(|n| n.is_element()) {
                if old.tag_name().namespace() == Some(ML)
                    || old.has_tag_name((RESULTS, "engineErrors"))
                    || old.has_tag_name((RESULTS, "Warnings"))
                {
                    patches.push((old.range(), String::new()));
                }
            }
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
        ("mathcad/settings/presentation.xml".into(), format!("<presentation xmlns=\"http://schemas.ptc.com/mathcad/settings/presentation10\"><pageModel paper-code=\"{paper}\" grid-size=\"{}\" orientation=\"Portrait\" margin-type=\"Standard\"/></presentation>", if fine { "Fine" } else { "Standard" }).into_bytes()),
        ("docProps/app.xml".into(), b"<properties xmlns=\"http://schemas.mathsoft.com/extended-properties\"><application>Mathcraft</application><appVersion>0.2.0</appVersion><serializationVersion>12.0.0.0</serializationVersion></properties>".to_vec()),
        ("docProps/core.xml".into(), format!("<cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><dc:title>{}</dc:title></cp:coreProperties>", escape(&doc.title)).into_bytes()),
    ];
    NativePackage::from_bytes(&zip_bytes(&parts)?)
}

#[cfg(test)]
mod tests {
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
                    format: Default::default(),
                },
                CanvasRegion {
                    id: 2,
                    left: 0.0,
                    top: 100.0,
                    content: CanvasContent::Text("new 한글 & text".into()),
                    source_region: Some("2".into()),
                    format: Default::default(),
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
                    format: Default::default(),
                },
                CanvasRegion {
                    id: 4,
                    left: 200.0,
                    top: 200.0,
                    content: real("11"),
                    source_region: None,
                    format: Default::default(),
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
                .all(|c| c.calculation_status.as_deref() == Some("Pending")
                    && c.result_kind.is_none())
        );
        assert_eq!(saved.caches().unwrap().len(), 4);
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
            .replace("zero-threshold=\"15\"", "zero-threshold=\"12\"")
            .replace("FontSize=\"18\"", "FontSize=\"18\" Bold=\"true\"")
            .replace("left=\"", "actualWidth=\"100\" actualHeight=\"30\" left=\"");
        source
            .set_part("mathcad/worksheet.xml", xml.into_bytes())
            .unwrap();
        source.write_new(&source_path).unwrap();
        doc.native_archive = Some(std::fs::read(&source_path).unwrap());
        doc.regions[0].source_region = Some("1".into());
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
    fn plot_export_failure_does_not_create_a_partial_native_document() {
        let dir = Dir::new();
        let path = dir.path("plot.mcdx");
        let mut doc = CanvasDocument::default();
        doc.add_math(0., 0.);
        doc.add_plot(0., 100.);
        assert!(export_mcdx(&doc, &path).unwrap_err().contains("XY plot"));
        assert!(!path.exists());
    }
}
