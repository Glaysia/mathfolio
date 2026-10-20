//! Artifact child of export. Ordinary catalog font-size is proven direct DIP;
//! exact three layout keys use a different original points property and reject.
//! No default/inherited style resolution or original empty serializer claim.
use super::*;
use crate::rich_text_source::{AuthoredRichText, AuthoredStyleSnapshot};

const PRESENTATION: &str = "http://schemas.ptc.com/mathcad/settings/presentation10";
const CATALOG: &str = "mathcad/settings/presentation.xml";
const MAX_CATALOG_BYTES: usize = 1024 * 1024;

/// Command-only admission: actual allocation/cold-source proof on a private
/// package. Imported carriers still use their unchanged native authority path.
/// Prospective append/replacement enumerate exactly the future owners.
pub(super) fn validate_regions<'a>(
    regions: impl Iterator<Item = &'a CanvasRegion>,
    package: &NativePackage,
    imported_source_bytes: usize,
) -> Result<(), String> {
    let mut authored = Vec::new();
    let mut total = imported_source_bytes;
    for region in regions {
        if let CanvasContent::Text(plain) = &region.content {
            total = total
                .checked_add(plain.len())
                .ok_or("Text source budget overflow")?;
        }
        if let Some(source) = region.validated_authored_text()? {
            total = total
                .checked_add(source.budget().serialized_wire_bytes())
                .ok_or("Text source budget overflow")?;
            authored.push((region, source));
        }
    }
    if authored.is_empty() {
        return Ok(());
    }
    if total > 8 * 1024 * 1024 {
        return Err("Canvas text exceeds total 8 MiB budget".into());
    }
    let mut candidate = package.clone();
    let mut next = candidate
        .regions()?
        .iter()
        .filter_map(|r| r.region_id.as_deref()?.parse::<u64>().ok())
        .max()
        .unwrap_or(0);
    for (region, source) in authored {
        next = next
            .checked_add(1)
            .ok_or("Native owner ID space exhausted")?;
        let (mut staged, fragment) =
            prepare_region(&candidate, next, region.left, region.top, &source)?;
        append_owner(&mut staged, &fragment)?;
        candidate = staged;
    }
    Ok(())
}

/// New owner append only. Caller owns actual document ID allocation. A whole
/// candidate is returned; source package is never modified on any failure.
/// Existing plain/None/imported export paths do not call this helper.
fn prepare_region(
    original: &NativePackage,
    owner: u64,
    left: f64,
    top: f64,
    source: &AuthoredRichText,
) -> Result<(NativePackage, String), String> {
    if owner == 0
        || !left.is_finite()
        || !top.is_finite()
        || left < 0.
        || top < 0.
        || left > 1_000_000.
        || top > 1_000_000.
    {
        return Err("Unsupported authored owner/position".into());
    }
    let worksheet = original.worksheet_part_name().to_owned();
    let xml = original.worksheet_xml()?;
    let doc = parse(&xml)?;
    let root = doc.root_element();
    if !root.has_tag_name((WS, "worksheet")) {
        return Err("Expected native worksheet root".into());
    }
    let mut collections = root.children().filter(|n| n.has_tag_name((WS, "regions")));
    let _regions = collections.next().ok_or("Missing root regions")?;
    if collections.next().is_some() {
        return Err("Ambiguous root regions".into());
    }
    let mut seen_ids = HashSet::new();
    let new_id = owner.to_string();
    for region in root
        .descendants()
        .filter(|n| n.has_tag_name((WS, "region")))
    {
        let raw = region
            .attribute("region-id")
            .ok_or("Missing existing owner ID")?;
        if raw.is_empty() || !raw.bytes().all(|b| b.is_ascii_digit()) {
            return Err("Unsupported existing owner ID".into());
        }
        let normalized = raw.trim_start_matches('0');
        let normalized = if normalized.is_empty() {
            "0"
        } else {
            normalized
        };
        if !seen_ids.insert(normalized) || normalized == new_id {
            return Err("Authored owner ID already exists".into());
        }
    }
    let current_catalog = original
        .original_xml_part(CATALOG)?
        .ok_or("Missing active authored presentation part")?;
    let types = original
        .original_xml_part("[Content_Types].xml")?
        .ok_or("Missing native content types")?;
    let types_doc = parse(&types)?;
    if !types_doc.root_element().has_tag_name((CONTENT, "Types")) {
        return Err("Invalid native content types".into());
    }
    let active=types_doc.root_element().children().filter(|n|n.has_tag_name((CONTENT,"Override"))&&n.attribute("ContentType")==Some("application/vnd.openxmlformats-officedocument.mathprocessingml.mathcad.settings.presentation+xml")).collect::<Vec<_>>();
    if active.len() != 1
        || active[0].attribute("PartName") != Some("/mathcad/settings/presentation.xml")
    {
        return Err("Relocated/ambiguous active authored catalog unsupported".into());
    }
    let normalized_catalog = normalize_layout_keys(&current_catalog)?;
    let catalog = merge_catalog(&normalized_catalog, source)?;
    // Reuse exact existing ZIP member order/settings and reservation allocator.
    let nested = text_package_from_xaml(source.xaml().as_bytes().to_vec())?;
    if nested.len() > 16 * 1024 * 1024 {
        return Err("Authored nested package exceeds reopening bound".into());
    }
    let id = owner.to_string();
    let reservation = reserve_text_part(original, &worksheet, &id, true)?;
    let mut candidate = original.clone();
    let item = install_text_package_part(&mut candidate, reservation, nested)?;
    if catalog != current_catalog {
        set_xml(&mut candidate, CATALOG, catalog)?;
    }
    let region = format!(
        "<region xmlns=\"{WS}\" region-id=\"{id}\" left=\"{left}\" top=\"{top}\"><text item-idref=\"{}\"/></region>",
        escape(&item)
    );
    // Verify the exact prospective owner through unchanged package binding,
    // then restore old worksheet BYTES. Returned helper installs parts/catalog
    // only; outer export remains the single actual owner append authority.
    append_owner(&mut candidate, &region)?;
    validate_installed(&candidate, owner, source)?;
    candidate.set_part(
        &worksheet,
        original
            .original_part(&worksheet)
            .ok_or("Missing worksheet bytes")?
            .to_vec(),
    )?;
    Ok((candidate, region))
}

/// Actual export::new_region hook: installs catalog/parts/relationships ONLY.
/// Caller appends this returned owning fragment exactly once. Outer export
/// already stages package publication; local staging also avoids partial error.
pub(super) fn fresh_region(
    region: &CanvasRegion,
    id: &str,
    package: &mut NativePackage,
    worksheet: &str,
) -> Result<String, String> {
    if worksheet != package.worksheet_part_name() {
        return Err("Only main worksheet authored Text supported".into());
    }
    let owner = id
        .parse::<u64>()
        .map_err(|_| "Unsupported authored owner ID")?;
    if owner.to_string() != id {
        return Err("Expected canonical authored owner ID".into());
    }
    let source = region
        .validated_authored_text()?
        .ok_or("Missing authored Text source")?;
    let (candidate, fragment) = prepare_region(package, owner, region.left, region.top, &source)?;
    *package = candidate;
    Ok(fragment)
}

/// Standalone staged-append exercise retained for headless integration tests.
/// Production fresh_region does not append, preventing duplicate outer owners.
#[cfg(test)]
pub(super) fn append_region(
    original: &NativePackage,
    owner: u64,
    left: f64,
    top: f64,
    source: &AuthoredRichText,
) -> Result<NativePackage, String> {
    let (mut candidate, region) = prepare_region(original, owner, left, top, source)?;
    append_owner(&mut candidate, &region)?;
    validate_installed(&candidate, owner, source)?;
    Ok(candidate)
}
fn append_owner(candidate: &mut NativePackage, region: &str) -> Result<(), String> {
    let xml = candidate.worksheet_xml()?;
    let doc = parse(&xml)?;
    let regions = doc
        .root_element()
        .children()
        .find(|n| n.has_tag_name((WS, "regions")))
        .ok_or("Missing regions")?;
    let updated = insert_child(&xml[regions.range()], region)?;
    let mut changed = xml.clone();
    changed.replace_range(regions.range(), &updated);
    let part = candidate.worksheet_part_name().to_owned();
    set_xml(candidate, &part, changed)
}
fn validate_installed(
    candidate: &NativePackage,
    owner: u64,
    source: &AuthoredRichText,
) -> Result<(), String> {
    // Construct authority only from the actual emitted owner/relationship/parts.
    // This neither fabricates a NativeTextAnchor nor changes imported admission.
    let authority = read_native_text_authority(candidate, &owner.to_string())?;
    let reopened =
        crate::rich_text_source::ValidatedRichText::from_original(authority, source.plain_text())?;
    if reopened.xaml() != source.xaml() {
        return Err("Authored native cold source mismatch".into());
    }
    Ok(())
}

fn rgba([r, g, b, a]: [u8; 4]) -> String {
    format!("#{a:02X}{r:02X}{g:02X}{b:02X}")
}
fn declaration_values(style: &AuthoredStyleSnapshot) -> [(&'static str, String); 10] {
    [
        ("style-key", style.key.clone()),
        ("font-family", style.font_family.clone()),
        ("font-size", style.size_dip.clone()),
        ("font-weight", style.weight.clone()),
        ("font-style", style.font_style.clone()),
        ("foreground", rgba(style.foreground_rgba)),
        ("background", rgba(style.background_rgba)),
        ("text-align", style.alignment.clone()),
        ("text-indent", "0".into()),
        ("variants", "Normal".into()),
    ]
}
fn declaration(style: &AuthoredStyleSnapshot) -> String {
    let mut xml = format!(
        "<textStyle xmlns=\"{PRESENTATION}\" name=\"{}\"",
        escape(&style.key)
    );
    for (name, value) in declaration_values(style) {
        xml.push_str(&format!(" {name}=\"{}\"", escape(&value)));
    }
    xml.push_str("/>");
    xml
}
fn unique_child<'a, 'i>(parent: Node<'a, 'i>, name: &str) -> Result<Option<Node<'a, 'i>>, String> {
    let mut matches = parent
        .children()
        .filter(|n| n.has_tag_name((PRESENTATION, name)));
    let first = matches.next();
    if matches.next().is_some() {
        return Err(format!("Ambiguous authored {name}"));
    }
    Ok(first)
}
/// Original 0a5d adds style-key=name before read; intentionally only the three
/// proven layout identities are normalized here. No numeric/default conversion.
fn normalize_layout_keys(xml: &str) -> Result<String, String> {
    if xml.len() > MAX_CATALOG_BYTES {
        return Err("Authored catalog byte bound exceeded".into());
    }
    let doc = parse(xml)?;
    let root = doc.root_element();
    if !root.has_tag_name((PRESENTATION, "presentation")) {
        return Err("Unexpected authored catalog namespace".into());
    }
    let Some(rendering) = unique_child(root, "textRendering")? else {
        return Ok(xml.to_owned());
    };
    let Some(styles) = unique_child(rendering, "textStyles")? else {
        return Ok(xml.to_owned());
    };
    let mut names = HashSet::new();
    let mut keys = HashSet::new();
    let mut patches = Vec::new();
    for entry in styles.children().filter(Node::is_element) {
        if !entry.has_tag_name((PRESENTATION, "textStyle"))
            || entry.children().any(|node| node.is_element())
        {
            return Err("Unsupported native textStyle element".into());
        }
        let name = entry
            .attribute("name")
            .filter(|s| !s.is_empty())
            .ok_or("Native style name missing")?;
        if !names.insert(name) {
            return Err("Duplicate native style name".into());
        }
        let key = if let Some(key) = entry.attribute("style-key") {
            key
        } else {
            if !matches!(
                name,
                "areaLayoutStyle" | "headerLayoutStyle" | "footerLayoutStyle"
            ) {
                return Err("Unknown name-only style unsupported".into());
            }
            patches.extend(attrs(entry, xml, &[("style-key", Some(name.to_owned()))])?);
            name
        };
        if key.is_empty() || !keys.insert(key) {
            return Err("Duplicate/empty normalized style key".into());
        }
    }
    apply(xml.to_owned(), patches)
}
fn merge_catalog(xml: &str, source: &AuthoredRichText) -> Result<String, String> {
    if xml.len() > MAX_CATALOG_BYTES {
        return Err("Authored catalog byte bound exceeded".into());
    }
    let doc = parse(xml)?;
    let root = doc.root_element();
    if !root.has_tag_name((PRESENTATION, "presentation")) {
        return Err("Unexpected authored catalog namespace".into());
    }
    let key = &source.seed().spec().format.key;
    if matches!(
        key.as_str(),
        "areaLayoutStyle" | "headerLayoutStyle" | "footerLayoutStyle"
    ) {
        return Err(
            "Layout style uses original point-size carrier; ordinary authored bridge rejects it"
                .into(),
        );
    }
    let style = source
        .seed()
        .spec()
        .catalog
        .iter()
        .find(|s| &s.key == key)
        .ok_or("Missing resolved authored style")?;
    let new_style = declaration(style);
    let Some(rendering) = unique_child(root, "textRendering")? else {
        // Correct XSD order is known only for this closed minimal prefix.
        // Existing settings/resources are not moved or recreated.
        if root.children().filter(Node::is_element).any(|n| {
            n.tag_name().namespace() != Some(PRESENTATION)
                || !matches!(n.tag_name().name(), "pageModel" | "mathRendering")
        }) {
            return Err("Missing textRendering in nonminimal catalog unsupported".into());
        }
        return checked_catalog(insert_child(
            xml,
            &format!(
                "<textRendering xmlns=\"{PRESENTATION}\" ProofingLanguage=\"{}\"><textStyles>{new_style}</textStyles></textRendering>",
                escape(&source.seed().spec().proofing_language)
            ),
        )?);
    };
    let Some(styles) = unique_child(rendering, "textStyles")? else {
        if rendering.children().any(|node| node.is_element()) {
            return Err("Unknown textRendering children without style collection".into());
        }
        let updated = insert_child(
            &xml[rendering.range()],
            &format!("<textStyles xmlns=\"{PRESENTATION}\">{new_style}</textStyles>"),
        )?;
        let mut result = xml.to_owned();
        result.replace_range(rendering.range(), &updated);
        return checked_catalog(result);
    };
    let mut matching = None;
    let mut display_collision = false;
    let mut keys = HashSet::new();
    let mut names = HashSet::new();
    for entry in styles.children().filter(Node::is_element) {
        if !entry.has_tag_name((PRESENTATION, "textStyle"))
            || entry.children().any(|node| node.is_element())
        {
            return Err("Unsupported explicit authored catalog entry".into());
        }
        let entry_key = entry
            .attribute("style-key")
            .ok_or("Name-only/default style resolver not implemented")?;
        let name = entry
            .attribute("name")
            .ok_or("Native textStyle name required")?;
        if entry_key.is_empty() || name.is_empty() || !keys.insert(entry_key) || !names.insert(name)
        {
            return Err("Duplicate/empty native style identity".into());
        }
        if entry_key == key {
            matching = Some(entry);
        } else if name == key {
            display_collision = true;
        }
    }
    if let Some(found) = matching {
        let expected = declaration_values(style);
        // Closed original defaults: never erase or rewrite an existing entry.
        // Ordinary font-size remains exact DIP, distinct from layout points.
        let required = expected
            .iter()
            .filter(|(name, _)| !matches!(*name, "text-indent" | "variants"));
        if required
            .clone()
            .any(|(name, value)| found.attribute(*name) != Some(value.as_str()))
            || found.attributes().any(|a| {
                a.namespace().is_some()
                    || match a.name() {
                        "name" => false,
                        "based-on-key" => !a.value().is_empty(),
                        "marker-style" => a.value() != "None",
                        "text-indent" => a.value() != "0",
                        "variants" => a.value() != "Normal",
                        name => !expected
                            .iter()
                            .any(|(n, v)| name == *n && a.value() == v.as_str()),
                    }
            })
        {
            return Err("Existing authored style key definition conflicts".into());
        }
        return Ok(xml.to_owned());
    }
    if display_collision {
        return Err("Authored style display name collision".into());
    }
    let updated = insert_child(&xml[styles.range()], &new_style)?;
    let mut result = xml.to_owned();
    result.replace_range(styles.range(), &updated);
    checked_catalog(result)
}
fn checked_catalog(xml: String) -> Result<String, String> {
    if xml.len() > MAX_CATALOG_BYTES {
        Err("Authored merged catalog exceeds bound".into())
    } else {
        parse(&xml)?;
        Ok(xml)
    }
}

#[cfg(test)]
#[path = "authored_export_tests.rs"]
mod tests;
