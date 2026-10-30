//! Source-bound native rich Text export and pure source admission.
//! Requires A's read-only origin/identity and positive FormatOnlyReceipt API.
//! Never admits a caller boolean or same-plain-only proof as inert formatting.
use super::*;
use crate::rich_text_source::{
    FormatOnlyReceipt, NativeTextIdentity, OriginalTextAuthority, ValidatedRichText,
};
use sha2::{Digest, Sha256};
use std::sync::Arc;

const XAML: &str = "http://schemas.microsoft.com/winfx/2006/xaml/presentation";
// First bounded subset: the catalog actually used by current native defaults.
// Do not infer a different active catalog from an untrusted wire's routing key.
const CATALOG: &str = "mathcad/settings/presentation.xml";
const MAX_DEFERRED: usize = 4096;
const MAX_DEFERRED_SOURCE: usize = 8 * 1024 * 1024;
const MAX_NESTED_PACKAGE: usize = 16 * 1024 * 1024;
// Additional transient authority admission budget for this first native subset.
// A shared 16-MiB target referenced by many owners must not be cloned 4096 times.
const MAX_AUTHORITY_BYTES: usize = 64 * 1024 * 1024;

struct NativeBinding {
    owner: String,
    item_idref: String,
    package_part: String,
    text_source: String,
}

/// Private receipt constructor is the ONLY path to deferred output. Fields are
/// deliberately private even to the parent export module's ordinary callers.
pub(super) struct ValidatedTextPatch {
    binding: NativeBinding,
    catalog_sha256: [u8; 32],
    package_sha256: [u8; 32],
    source: AdmittedSource,
}

enum AdmittedSource {
    Format(FormatOnlyReceipt),
    Content(ValidatedRichText),
}
#[derive(Default)]
pub(super) struct DeferredTextEdits(Vec<ValidatedTextPatch>);
#[derive(Default)]
pub(super) struct OrdinaryTextEdits(Vec<ValidatedTextPatch>);
pub(super) struct DerivedTextEdits(HashMap<u64, DerivedTextPatch>);
struct DerivedTextPatch {
    source: ValidatedRichText,
    original_region: String,
}

/// Read-only validation accounting; not another source authority or wire DTO.
/// source_bytes is current XAML ONLY; CanvasDocument must add it once to its
/// existing total source budget (which already counted Text plain mirrors).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RichTextAdmissionStats {
    pub carrier_count: usize,
    pub source_bytes: usize,
    pub plain_text_bytes: usize,
    pub authority_bytes: usize,
}
struct Admission {
    edits: PreparedTextEdits,
    stats: RichTextAdmissionStats,
}

pub(super) struct PreparedTextEdits {
    pub(super) deferred: DeferredTextEdits,
    pub(super) ordinary: OrdinaryTextEdits,
    pub(super) derived: DerivedTextEdits,
}
fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn region_key(value: &str) -> Result<String, String> {
    if value.is_empty() || value.len() > 1024 || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err("Unsupported rich text owner identity".into());
    }
    let value = value.trim_start_matches('0');
    Ok(if value.is_empty() { "0" } else { value }.to_owned())
}

fn binding(package: &NativePackage, owner: &str) -> Result<NativeBinding, String> {
    let source = package.worksheet_xml()?;
    let doc = parse(&source)?;
    let root = doc.root_element();
    if !root.has_tag_name((WS, "worksheet")) {
        return Err("Rich text authority requires Worksheet50 source".into());
    }
    let key = region_key(owner)?;
    let matches = root
        .descendants()
        .filter(|node| node.has_tag_name((WS, "region")))
        .filter(|node| {
            node.attribute("region-id")
                .and_then(|s| region_key(s).ok())
                .as_deref()
                == Some(key.as_str())
        })
        .collect::<Vec<_>>();
    if matches.len() != 1 || matches[0].attribute("region-id") != Some(owner) {
        return Err("Missing or ambiguous rich text owner identity".into());
    }
    let region = matches[0];
    let collections = root
        .children()
        .filter(|node| node.has_tag_name((WS, "regions")))
        .collect::<Vec<_>>();
    if collections.len() != 1 || region.parent() != Some(collections[0]) {
        return Err("Only a direct worksheet Text owner is supported".into());
    }
    let payloads = region
        .children()
        .filter(Node::is_element)
        .collect::<Vec<_>>();
    if payloads.len() != 1 || !payloads[0].has_tag_name((WS, "text")) {
        return Err("Rich text owner must contain one Text payload".into());
    }
    let text = payloads[0];
    if text
        .attributes()
        .any(|a| a.namespace().is_some() || a.name() != "item-idref")
    {
        return Err("Unknown rich Text wrapper attribute".into());
    }
    // Preserve an inert worksheet-wide style wrapper verbatim. Embedded math,
    // resource dictionaries, binding or any unknown child stay unsupported.
    let inline = text.children().filter(Node::is_element).collect::<Vec<_>>();
    if inline.len() > 1 {
        return Err("Ambiguous rich Text inline wrapper".into());
    }
    for node in inline {
        if !node.has_tag_name((XAML, "FlowDocument"))
            || node.children().any(|child| {
                child.is_element()
                    || child.is_text() && !child.text().unwrap_or("").trim().is_empty()
            })
            || node.attributes().any(|a| {
                a.namespace().is_some()
                    || !matches!(
                        a.name(),
                        "Background"
                            | "Foreground"
                            | "FontFamily"
                            | "FontSize"
                            | "FontWeight"
                            | "FontStyle"
                    )
                    || a.value().contains(['{', '}'])
            })
        {
            return Err("Unsupported embedded/active rich Text wrapper".into());
        }
    }
    let item_idref = text
        .attribute("item-idref")
        .ok_or("Rich Text relationship missing")?;
    let package_part = crate::native_visual::resolve_relationship(
        package,
        package.worksheet_part_name(),
        item_idref,
        FLOW_REL,
    )?;
    // The resolver checks typed internal relation + duplicate selected Id.
    // Actual bytes, rather than a guessed canonical xaml pathname, bind origin.
    if package.original_part(&package_part).is_none() {
        return Err("Rich Text relationship target missing".into());
    }
    Ok(NativeBinding {
        owner: owner.to_owned(),
        item_idref: item_idref.to_owned(),
        package_part,
        text_source: source[text.range()].to_owned(),
    })
}

/// Read-only extraction for native import/admission. No ZIP/source execution,
/// no URI/file/network resolving and no promotion of a render projection.
pub(crate) fn read_native_authority(
    package: &NativePackage,
    owner: &str,
    catalog_part: &str,
) -> Result<Arc<OriginalTextAuthority>, String> {
    if catalog_part != CATALOG {
        return Err("Rich text catalog location is outside the supported subset".into());
    }
    let found = binding(package, owner)?;
    let nested = package
        .original_part(&found.package_part)
        .ok_or("Missing rich text package")?;
    let catalog = package
        .original_part(CATALOG)
        .ok_or("Missing rich text style catalog")?;
    OriginalTextAuthority::from_package(
        NativeTextIdentity {
            owner_xml_sha256: digest(owner_source(package, owner)?.as_bytes()),
            owner_region_id: found.owner,
            item_idref: found.item_idref,
            package_part: found.package_part,
            catalog_part: CATALOG.to_owned(),
        },
        nested.to_vec(),
        catalog.to_vec(),
    )
}

/// Admission is eager. Serialization/install is deferred so original context
/// and program proofs observe the unchanged inert text source. A's positive
/// original→current delta proof, not mere grammar+sameplain, authorizes this.
fn admit(canvas: &CanvasDocument, package: &NativePackage) -> Result<Admission, String> {
    admit_regions(
        canvas.regions.iter(),
        canvas.native_archive.is_some(),
        package,
    )
}

fn admit_regions<'a, I>(
    regions: I,
    has_native_source: bool,
    package: &NativePackage,
) -> Result<Admission, String>
where
    I: Iterator<Item = &'a CanvasRegion> + Clone,
{
    let region_count = regions.clone().count();
    let carrier_count = regions
        .clone()
        .filter(|region| region.rich_text.is_some())
        .count();
    // Exact old no-carrier path: do not decode/read package or duplicate old
    // validation/error ordering when there is no new optional source at all.
    if carrier_count == 0 {
        return Ok(Admission {
            edits: PreparedTextEdits {
                deferred: DeferredTextEdits::default(),
                ordinary: OrdinaryTextEdits::default(),
                derived: DerivedTextEdits(HashMap::new()),
            },
            stats: RichTextAdmissionStats::default(),
        });
    }
    if region_count > MAX_DEFERRED {
        return Err("Canvas region count exceeds rich Text admission budget".into());
    }
    if !has_native_source {
        return Err("Rich Text source has no retained native authority".into());
    }
    let mut plain_text_bytes = 0usize;
    for region in regions.clone() {
        if let CanvasContent::Text(plain) = &region.content {
            if plain.len() > crate::rich_text_source::MAX_SOURCE_BYTES {
                return Err("Canvas text region exceeds 1 MiB".into());
            }
            plain_text_bytes = plain_text_bytes
                .checked_add(plain.len())
                .ok_or("Canvas Text budget overflow")?;
        }
    }
    if plain_text_bytes > MAX_DEFERRED_SOURCE {
        return Err("Canvas Text exceeds aggregate source budget".into());
    }
    let mut deferred = Vec::new();
    let mut ordinary = Vec::new();
    let mut derived = HashMap::new();
    let mut authorities: HashMap<String, Arc<OriginalTextAuthority>> = HashMap::new();
    let mut authority_bytes = 0usize;
    let mut source_bytes = 0usize;
    let mut owners = HashSet::new();
    for region in regions.clone() {
        // Authored provenance is separately admitted by Canvas validation and
        // actual authored_export::fresh_region/cold proof, never rebound here
        // to a fabricated imported NativeTextIdentity.
        let Some(wire) = region.rich_text.as_deref() else {
            continue;
        };
        let CanvasContent::Text(plain) = &region.content else {
            return Err("Rich Text source is only valid on a Text region".into());
        };
        let identity = wire.origin().identity();
        let origin_owner = identity.owner_region_id.as_str();
        let derivative = region.source_region.is_none();
        if region
            .source_region
            .as_deref()
            .is_some_and(|owner| owner != origin_owner)
            || identity.catalog_part != CATALOG
        {
            return Err("Rich Text origin routing does not match source owner/catalog".into());
        }
        if !derivative && !owners.insert(region_key(origin_owner)?) {
            return Err("Duplicate original rich Text source owners".into());
        }
        let found = binding(package, origin_owner)?;
        let original_package = package
            .original_part(&found.package_part)
            .ok_or("Missing rich Text target")?;
        let catalog = package
            .original_part(CATALOG)
            .ok_or("Missing rich Text catalog")?;
        // Scoped exact-origin sharing: repeated Text-authority-equivalent copies must not
        // duplicate one immutable original package/catalog for each new owner.
        let authority = if let Some(authority) = authorities.get(origin_owner) {
            authority.clone()
        } else {
            read_native_authority(package, origin_owner, &identity.catalog_part)?
        };
        let current = ValidatedRichText::from_wire(wire.clone(), authority.clone(), plain)?;
        if !authorities.contains_key(origin_owner) {
            authority_bytes = authority_bytes
                .checked_add(original_package.len())
                .and_then(|b| b.checked_add(catalog.len()))
                .ok_or("Rich Text authority budget overflow")?;
            if authority_bytes > MAX_AUTHORITY_BYTES {
                return Err("Rich Text transient authorities exceed bounded subset".into());
            }
            authorities.insert(origin_owner.to_owned(), authority.clone());
        }
        source_bytes = source_bytes
            .checked_add(current.xaml().len())
            .ok_or("Rich text source budget overflow")?;
        if source_bytes > MAX_DEFERRED_SOURCE
            || source_bytes
                .checked_add(plain_text_bytes)
                .is_none_or(|sum| sum > MAX_DEFERRED_SOURCE)
        {
            return Err("Rich Text source exceeds aggregate export budget".into());
        }
        if !derivative && current.xaml() == authority.original_xaml() {
            continue;
        }
        if derivative {
            // Native identity is assigned ONLY by parent ResultIds later.
            let original_region = derivative_template(package, origin_owner)?;
            if derived
                .insert(
                    region.id,
                    DerivedTextPatch {
                        source: current,
                        original_region,
                    },
                )
                .is_some()
            {
                return Err("Duplicate derivative canvas Text identity".into());
            }
            continue;
        }
        let is_content_edit = current.plain_text() != authority.original_plain_text()
            || current.has_original_raw_body_only_delta();
        let source = if is_content_edit {
            AdmittedSource::Content(current)
        } else {
            AdmittedSource::Format(current.prove_format_only()?)
        };
        let patch = ValidatedTextPatch {
            binding: found,
            catalog_sha256: digest(catalog),
            package_sha256: digest(original_package),
            source,
        };
        if is_content_edit {
            ordinary.push(patch);
        } else {
            deferred.push(patch);
        }
    }
    Ok(Admission {
        edits: PreparedTextEdits {
            deferred: DeferredTextEdits(deferred),
            ordinary: OrdinaryTextEdits(ordinary),
            derived: DerivedTextEdits(derived),
        },
        stats: RichTextAdmissionStats {
            carrier_count,
            source_bytes,
            plain_text_bytes,
            authority_bytes,
        },
    })
}

/// Shared admission only. Does NOT call CanvasDocument::validate (which calls
/// this facade), project results, clone/stage NativePackage, serialize JSON or
/// export nested packages. Readonly extraction/parsing and positive source
/// proofs remain mandatory. Existing helper completes original namespace scope.
pub(crate) fn validate_carriers(
    canvas: &CanvasDocument,
    package: &NativePackage,
) -> Result<RichTextAdmissionStats, String> {
    Ok(admit(canvas, package)?.stats)
}

/// Command-only borrowed prospective admission. Full source accounting is Rust
/// only; native authority is rebound once for the command, never for each key.
pub(crate) fn validate_append(
    canvas: &CanvasDocument,
    package: &NativePackage,
    candidate: &CanvasRegion,
) -> Result<RichTextAdmissionStats, String> {
    canvas.validate_text_append_budget(candidate)?;
    Ok(admit_regions(
        canvas.regions.iter().chain(std::iter::once(candidate)),
        canvas.native_archive.is_some(),
        package,
    )?
    .stats)
}
pub(crate) fn validate_replacement(
    canvas: &CanvasDocument,
    package: &NativePackage,
    candidate: &CanvasRegion,
) -> Result<RichTextAdmissionStats, String> {
    canvas.validate_text_replace_budget(candidate)?;
    Ok(admit_regions(
        canvas.regions.iter().map(|old| {
            if old.id == candidate.id {
                candidate
            } else {
                old
            }
        }),
        canvas.native_archive.is_some(),
        package,
    )?
    .stats)
}

/// Export consumes the SAME admitted source objects/proofs, without a second
/// source admission pass or weaker parallel validation implementation.
pub(super) fn prepare(
    canvas: &CanvasDocument,
    package: &NativePackage,
) -> Result<PreparedTextEdits, String> {
    Ok(admit(canvas, package)?.edits)
}

fn owner_source(package: &NativePackage, owner: &str) -> Result<String, String> {
    binding(package, owner)?;
    let source = package.worksheet_xml()?;
    let doc = parse(&source)?;
    let node = doc
        .descendants()
        .find(|n| n.has_tag_name((WS, "region")) && n.attribute("region-id") == Some(owner))
        .ok_or("Missing Text origin owner")?;
    // This EXACT helper/source is also used for identity.owner_xml_sha256.
    crate::spec_table::standalone(node, &source)
}

fn derivative_template(package: &NativePackage, owner: &str) -> Result<String, String> {
    let source = owner_source(package, owner)?;
    let doc = parse(&source)?;
    let node = doc.root_element();
    // No guessing about protection/private semantics. Preserve known original
    // wrapper source, and reject unsupported owner attributes before export.
    if node.attributes().any(|a| {
        a.namespace().is_some()
            || !matches!(
                a.name(),
                "region-id"
                    | "left"
                    | "top"
                    | "width"
                    | "height"
                    | "actualWidth"
                    | "actualHeight"
                    | "align-x"
                    | "align-y"
            )
    }) {
        return Err("Protected/private/unknown rich Text owner cannot be duplicated".into());
    }
    Ok(source)
}

/// Matching actual Text authority only (not whole-workbook provenance). Called after the parent's existing
/// ResultIds.allocate_region() supplied a fresh id. COW is BEFORE source proof,
/// so existing raw result payload stays visible Pending conservatively.
pub(super) fn fresh_region(
    region: &CanvasRegion,
    id: &str,
    package: &mut NativePackage,
    worksheet: &str,
    derived: &mut DerivedTextEdits,
) -> Result<String, String> {
    let entry = derived
        .0
        .remove(&region.id)
        .ok_or("Unproven derivative Text owner")?;
    let CanvasContent::Text(plain) = &region.content else {
        return Err("Expected derivative Text".into());
    };
    entry.source.validate_plain(plain)?;
    let nested = entry.source.export_package()?;
    if nested.len() > MAX_NESTED_PACKAGE {
        return Err("Derived rich Text package exceeds reopening budget".into());
    }
    let item_idref =
        super::add_text_package_part(package, worksheet, id, nested.as_ref().to_vec(), true)?;
    let source = entry.original_region;
    let doc = parse(&source)?;
    let owner = doc.root_element();
    if !owner.has_tag_name((WS, "region")) {
        return Err("Expected original region wrapper".into());
    }
    let text = owner
        .children()
        .find(|n| n.has_tag_name((WS, "text")))
        .ok_or("Missing original Text wrapper")?;
    let mut changes = attrs(
        owner,
        &source,
        &[
            ("region-id", Some(id.into())),
            ("left", Some(region.left.to_string())),
            ("top", Some(region.top.to_string())),
            ("actualWidth", None),
            ("actualHeight", None),
        ],
    )?;
    changes.extend(attrs(text, &source, &[("item-idref", Some(item_idref))])?);
    apply(source, changes)
}

pub(super) fn assert_all_derived_bound(derived: &DerivedTextEdits) -> Result<(), String> {
    if derived.0.is_empty() {
        Ok(())
    } else {
        Err("Unbound derivative Text source".into())
    }
}

/// No source/result proof is relaxed. Run this only after export_into's normal
/// proof + result writes, on the ephemeral export package. Atomic publication
/// also protects any future internal caller: any failure leaves package intact.
pub(super) fn apply_deferred(
    package: &mut NativePackage,
    patches: DeferredTextEdits,
) -> Result<(), String> {
    apply_validated(package, patches.0)
}

/// Ordinary changed-content carrier is installed BEFORE the unchanged proof;
/// COW changes context, so native origin stays Pending with old payload retained.
pub(super) fn apply_ordinary(
    package: &mut NativePackage,
    patches: OrdinaryTextEdits,
) -> Result<(), String> {
    apply_validated(package, patches.0)
}

fn apply_validated(
    package: &mut NativePackage,
    patches: Vec<ValidatedTextPatch>,
) -> Result<(), String> {
    if patches.is_empty() {
        return Ok(());
    }
    let mut staged = package.clone();
    for patch in patches {
        let current = binding(&staged, &patch.binding.owner)?;
        if current.item_idref != patch.binding.item_idref
            || current.package_part != patch.binding.package_part
            || current.text_source != patch.binding.text_source
            || staged.original_part(CATALOG).map(digest) != Some(patch.catalog_sha256)
            || staged
                .original_part(&patch.binding.package_part)
                .map(digest)
                != Some(patch.package_sha256)
        {
            return Err("Rich Text source/context changed after proof".into());
        }
        let nested = match &patch.source {
            AdmittedSource::Format(proof) => proof.export_package()?,
            AdmittedSource::Content(source) => source.export_package()?,
        };
        if nested.len() > MAX_NESTED_PACKAGE {
            return Err("Exported rich Text package exceeds reopening budget".into());
        }
        let worksheet = staged.worksheet_part_name().to_owned();
        // Factored existing add_text_part allocator/rel/content-type installer.
        // strict_reserved=true additionally respects absent ZIP part declarations.
        let item_idref = super::add_text_package_part(
            &mut staged,
            &worksheet,
            &patch.binding.owner,
            nested.as_ref().to_vec(),
            true,
        )?;
        let source = staged.worksheet_xml()?;
        let document = parse(&source)?;
        let key = region_key(&patch.binding.owner)?;
        let nodes = document
            .descendants()
            .filter(|n| n.has_tag_name((WS, "region")))
            .filter(|n| {
                n.attribute("region-id")
                    .and_then(|s| region_key(s).ok())
                    .as_deref()
                    == Some(key.as_str())
            })
            .collect::<Vec<_>>();
        if nodes.len() != 1 {
            return Err("Rich Text owner became ambiguous".into());
        }
        let owner = nodes[0];
        let text = owner
            .children()
            .find(|n| n.has_tag_name((WS, "text")))
            .ok_or("Missing rich Text owner")?;
        let mut edits = attrs(text, &source, &[("item-idref", Some(item_idref))])?;
        edits.extend(attrs(
            owner,
            &source,
            &[("actualWidth", None), ("actualHeight", None)],
        )?);
        set_xml(&mut staged, &worksheet, apply(source, edits)?)?;
    }
    *package = staged;
    Ok(())
}
