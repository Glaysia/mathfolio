//! App creation policy and narrow explicit native catalog reader.
//! Runtime snapshots use DIP; only the three verified layout carriers use points.
//! Native source/catalog allocation and the original empty serializer are separate.
use mathfolio_document::rich_text_source::{
    AuthoredStyleSnapshot, AuthoredTextSeed, AuthoredTextSeedSpec,
};
use roxmltree::{Document, Node, ParsingOptions};
use std::{collections::HashSet, sync::Arc};

const PRESENTATION: &str = "http://schemas.ptc.com/mathcad/settings/presentation10";

fn only<'a, 'input>(parent: Node<'a, 'input>, name: &str) -> Result<Node<'a, 'input>, String> {
    let mut found = parent
        .children()
        .filter(|n| n.has_tag_name((PRESENTATION, name)));
    let first = found.next().ok_or("Missing native text catalog carrier")?;
    if found.next().is_some() {
        return Err("Duplicated native text catalog carrier".into());
    }
    Ok(first)
}

/// Bounded catalog entry lookup for an EXPLICITLY chosen ordinary style key.
/// This does not recover the application's unrepresented current-format state.
/// Installed serializer0a6c creates Ptc.TextStyle for every non-layout key;
/// serializer0a5d writes its FontSize directly in DIP. The three layout keys
/// have a separate points carrier and must not use this function.
#[cfg(test)]
pub(crate) fn seed_for_declared_native_style(
    presentation: &str,
    selected_key: &str,
) -> Result<Arc<AuthoredTextSeed>, String> {
    if layout_key(selected_key) {
        return Err("Layout definitions are not ordinary authored catalog styles".into());
    }
    seed_for_native_current_style(presentation, selected_key)
}

/// Resolve a caller-identified CURRENT format. A layout entry becomes Normal
/// only on the current-format clone; the seed's Normal definition comes from
/// the actual distinct ordinary Normal entry. No embedded Normal is fabricated.
#[cfg(test)]
pub(crate) fn seed_for_native_current_style(
    presentation: &str,
    selected_key: &str,
) -> Result<Arc<AuthoredTextSeed>, String> {
    seed_from_native(presentation, selected_key, false)
}

/// Initial BODY policy, not an inference about a previously retained selection.
/// Original manager071f supplies embedded Normal and overlays six appearance
/// fields from areaLayoutStyle. Raw presentation/catalog is never rewritten.
pub(crate) fn seed_for_native_body(presentation: &str) -> Result<Arc<AuthoredTextSeed>, String> {
    seed_from_native(presentation, "areaLayoutStyle", true)
}

fn seed_from_native(
    presentation: &str,
    selected_key: &str,
    embedded_area_normal: bool,
) -> Result<Arc<AuthoredTextSeed>, String> {
    if presentation.len() > 1024 * 1024 {
        return Err("Unsupported native catalog size".into());
    }
    let doc = Document::parse_with_options(
        presentation,
        ParsingOptions {
            allow_dtd: false,
            nodes_limit: 32_768,
            ..Default::default()
        },
    )
    .map_err(|e| e.to_string())?;
    if doc.descendants().any(|node| node.is_pi()) {
        return Err("Catalog processing instruction unsupported".into());
    }
    let root = doc.root_element();
    if !root.has_tag_name((PRESENTATION, "presentation")) {
        return Err("Unsupported presentation root".into());
    }
    let rendering = only(root, "textRendering")?;
    let language = rendering
        .attribute("ProofingLanguage")
        .ok_or("Missing explicit proofing language")?;
    let styles = only(rendering, "textStyles")?;
    let mut keys = HashSet::new();
    let mut selected = None;
    let mut normal = None;
    for node in styles.children().filter(Node::is_element) {
        if !node.has_tag_name((PRESENTATION, "textStyle")) || keys.len() >= 256 {
            return Err("Unsupported native catalog child/count".into());
        }
        // Actual native class selection uses style-key, falling back to name.
        // Lookup uniqueness covers both forms; target admission below requires
        // both explicit attributes because the existing append bridge does.
        let key = node
            .attribute("style-key")
            .or_else(|| node.attribute("name"))
            .filter(|s| !s.is_empty())
            .ok_or("Missing native style identity")?;
        if key.len() > 1024 || !keys.insert(key.to_owned()) {
            return Err("Duplicate or oversized native style key".into());
        }
        if key == selected_key {
            selected = Some(node);
        }
        if key == "Normal" {
            normal = Some(node);
        }
    }
    // Our fresh native writer may have only a complete explicit Normal style.
    // With no area declaration, reuse that definition; a present unsupported
    // area declaration still fails and is never hidden by this branch.
    let (selected_key, target) =
        if embedded_area_normal && selected_key == "areaLayoutStyle" && selected.is_none() {
            ("Normal", normal.ok_or("Missing explicit body Text style")?)
        } else {
            (
                selected_key,
                selected.ok_or("Explicit selected native style is missing")?,
            )
        };
    let format = read_explicit_style(target, selected_key)?;
    let catalog_style = if layout_key(selected_key) {
        if let Some(normal) = normal {
            read_explicit_style(normal, "Normal")?
        } else if embedded_area_normal && selected_key == "areaLayoutStyle" {
            // All six appearance fields are explicit on the admitted layout.
            // Among fields represented by this DTO, only key and alignment are
            // not in that override set; embedded Normal has Normal/Left.
            AuthoredStyleSnapshot {
                key: "Normal".into(),
                font_family: format.font_family.clone(),
                size_dip: format.size_dip.clone(),
                weight: format.weight.clone(),
                font_style: format.font_style.clone(),
                alignment: "Left".into(),
                foreground_rgba: format.foreground_rgba,
                background_rgba: format.background_rgba,
            }
        } else {
            return Err("Missing explicit ordinary Normal declaration".into());
        }
    } else {
        format.clone()
    };
    prepare_creation_seed(CreationSnapshot {
        current: format,
        proofing_language: language.into(),
        catalog: vec![catalog_style],
        unsupported: UnsupportedCreationProperties::default(),
    })
}

fn read_explicit_style(
    target: Node<'_, '_>,
    selected_key: &str,
) -> Result<AuthoredStyleSnapshot, String> {
    let is_layout = layout_key(selected_key);
    if (!is_layout && target.attribute("style-key") != Some(selected_key))
        || target.attribute("name").is_none_or(str::is_empty)
        || target
            .children()
            .any(|n| !n.is_text() || !n.text().unwrap_or("").trim().is_empty())
    {
        return Err("Selected style requires a complete explicit declaration".into());
    }
    let allowed = [
        "name",
        "style-key",
        "based-on-key",
        "font-family",
        "font-size",
        "font-weight",
        "font-style",
        "foreground",
        "background",
        "text-align",
        "text-indent",
        "marker-style",
        "variants",
        "margin",
    ];
    if target
        .attributes()
        .any(|a| a.namespace().is_some() || !allowed.contains(&a.name()))
        || target
            .attribute("based-on-key")
            .is_some_and(|s| !s.is_empty())
        || target.attribute("text-indent").is_some_and(|s| s != "0")
        || target
            .attribute("marker-style")
            .is_some_and(|s| s != "None")
        || target.attribute("variants").is_some_and(|s| s != "Normal")
        || target
            .attribute("margin")
            .is_some_and(|s| !is_layout || !matches!(s, "0" | "0,0,0,0"))
    {
        return Err("Selected native style requires unsupported inheritance/properties".into());
    }
    let required = |name| {
        target
            .attribute(name)
            .ok_or_else(|| format!("Missing explicit native style {name}"))
    };
    let color = |name| {
        let raw = required(name)?;
        if raw.len() != 9 || !raw.starts_with('#') {
            return Err("Expected literal native #AARRGGBB".to_string());
        }
        mathfolio_document::native_visual::parse_color(raw)
    };
    let size = required("font-size")?;
    let size_dip = if is_layout {
        layout_points_to_dip(size)?
    } else {
        size.to_owned()
    };
    Ok(AuthoredStyleSnapshot {
        key: selected_key.into(),
        font_family: required("font-family")?.into(),
        size_dip,
        weight: required("font-weight")?.into(),
        font_style: required("font-style")?.into(),
        // TextFormat102e's known default alignment enum0 is Left. This default
        // is allowed only on the layout branch, never arbitrary ordinary style.
        alignment: if is_layout {
            target.attribute("text-align").unwrap_or("Left").into()
        } else {
            required("text-align")?.into()
        },
        foreground_rgba: color("foreground")?,
        background_rgba: color("background")?,
    })
}

fn layout_points_to_dip(points: &str) -> Result<String, String> {
    // Narrow unsigned finite decimal admission, then the exact installed
    // WpfUtils05f7 Double instruction order: divide72, multiply96.
    mathfolio_document::rich_text_source::DipSize::parse(points)?;
    let points = points.parse::<f64>().map_err(|e| e.to_string())?;
    let dip = (points / 72.0) * 96.0;
    if !dip.is_finite() {
        return Err("Layout font size is not finite".into());
    }
    let lexical = dip.to_string();
    mathfolio_document::rich_text_source::DipSize::parse(lexical.clone())?;
    // This is a new resolved source lexeme, not original .NET writer bytes.
    Ok(lexical)
}

fn layout_key(key: &str) -> bool {
    matches!(
        key,
        "areaLayoutStyle" | "headerLayoutStyle" | "footerLayoutStyle"
    )
}

/// Current worksheet format, distinct from toolbar mixed/retained button state.
/// No Default: an absent current format is not permission to invent a fallback.
pub(crate) struct CreationSnapshot {
    pub current: AuthoredStyleSnapshot,
    pub proofing_language: String,
    /// Already resolved, closed catalog. No based-on or font-size-unit inference.
    pub catalog: Vec<AuthoredStyleSnapshot>,
    /// Unsupported properties must not disappear when making the smaller seed.
    pub unsupported: UnsupportedCreationProperties,
}

#[derive(Default)]
pub(crate) struct UnsupportedCreationProperties {
    pub underline_or_other_decorations: bool,
    pub nonzero_indent: bool,
    pub custom_paragraph_or_line_geometry: bool,
    pub right_to_left: bool,
    pub unresolved_script_or_font_properties: bool,
}

/// Prime Worksheet0a18 normalizes these three layout keys on a format CLONE.
/// All other properties and arbitrary custom keys must survive unchanged.
pub(crate) fn prepare_creation_seed(
    snapshot: CreationSnapshot,
) -> Result<Arc<AuthoredTextSeed>, String> {
    let unsupported = &snapshot.unsupported;
    if unsupported.underline_or_other_decorations
        || unsupported.nonzero_indent
        || unsupported.custom_paragraph_or_line_geometry
        || unsupported.right_to_left
        || unsupported.unresolved_script_or_font_properties
    {
        return Err("Current text format is outside the authored seed subset".into());
    }
    let mut current = snapshot.current;
    if layout_key(&current.key) {
        current.key = "Normal".into();
    }
    // Core checks complete explicit styles, language, key membership, duplicate
    // keys and logical byte budgets. Do not insert/replace a missing Normal.
    AuthoredTextSeed::new(AuthoredTextSeedSpec {
        format: current,
        proofing_language: snapshot.proofing_language,
        catalog: snapshot.catalog,
        text_decorations: None,
    })
}

#[cfg(test)]
#[path = "creation_seed_tests.rs"]
mod tests;
