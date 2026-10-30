//! Bounded inert native text source, preserving original XML and resources.
//! Selection coordinates are paragraph Unicode scalars, NOT native TextPointer.
//! Original evidence: rich-text-next-20261019.md, initial load vs catalog update.
use roxmltree::{Document, Node, ParsingOptions};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    io::{Cursor, Read, Write},
    ops::Range,
    sync::Arc,
};
use zip::{ZipArchive, ZipWriter};

mod authored;
pub use authored::{
    AuthoredCreationSnapshot, AuthoredRichText, AuthoredStyleSnapshot, AuthoredTextBudget,
    AuthoredTextPatch, AuthoredTextSeed, AuthoredTextSeedSpec, AuthoredTextWire,
    checked_authored_wire_bytes,
};

mod cross_run;
pub use cross_run::{CrossRunPatch, CrossRunTypingContext, TypingFlowDirection};

mod native_full_selection;
pub use native_full_selection::NativeFullSelectionTypingContext;

mod paragraph_alignment;
pub use paragraph_alignment::{ParagraphAlignment, ParagraphAlignmentInfo};

mod paragraph_origin;
pub use paragraph_origin::ParagraphSplitPatch;

const XAML: &str = "http://schemas.microsoft.com/winfx/2006/xaml/presentation";
const XML: &str = "http://www.w3.org/XML/1998/namespace";
const PRESENTATION: &str = "http://schemas.ptc.com/mathcad/settings/presentation10";
const INNER: &str = "Xaml/Document.xaml";
pub const MAX_SOURCE_BYTES: usize = 1024 * 1024;
pub const MAX_RUNS: usize = 4096;
const MAX_PACKAGE_BYTES: usize = 16 * 1024 * 1024;
const MAX_EXPANDED_BYTES: u64 = 32 * 1024 * 1024;
const MAX_MEMBERS: usize = 256;
const MAX_XML_NODES: u32 = 32_768;
type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeTextIdentity {
    pub owner_region_id: String,
    pub item_idref: String,
    pub package_part: String,
    pub catalog_part: String,
    /// SHA256 of the exact namespace-complete original owning Text wrapper.
    /// The native adapter reconstructs this from the actual retained package.
    pub owner_xml_sha256: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeTextAnchor {
    identity: NativeTextIdentity,
    package_sha256: [u8; 32],
    original_xaml_sha256: [u8; 32],
    catalog_sha256: [u8; 32],
}
impl NativeTextAnchor {
    /// Untrusted routing metadata until from_wire rebinds original authority.
    pub fn identity(&self) -> &NativeTextIdentity {
        &self.identity
    }
}

/// Wire is inert until validated against an immutable authority + plain mirror.
/// Deserializing this DTO alone must NEVER authorize export or editing.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RichTextWire {
    version: u8,
    current_xaml: Arc<str>,
    origin: NativeTextAnchor,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "paragraph_origin::deserialize_origins"
    )]
    paragraph_origins: Option<Arc<[u16]>>,
    #[serde(skip)]
    visual_fingerprint: std::sync::OnceLock<[u8; 32]>,
}
impl PartialEq for RichTextWire {
    fn eq(&self, other: &Self) -> bool {
        self.version == other.version
            && self.current_xaml == other.current_xaml
            && self.origin == other.origin
            && self.paragraph_origins == other.paragraph_origins
    }
}
impl Eq for RichTextWire {}
impl RichTextWire {
    /// Derived cache routing only, never an authority/admission proof. Cached
    /// per immutable source. JSON tuple is unambiguous and includes origin.
    pub(crate) fn visual_source_fingerprint(&self) -> [u8; 32] {
        *self.visual_fingerprint.get_or_init(|| {
            let bytes = if let Some(origins) = &self.paragraph_origins {
                serde_json::to_vec(&(
                    self.version,
                    &self.origin,
                    self.current_xaml.as_ref(),
                    origins.as_ref(),
                ))
            } else {
                serde_json::to_vec(&(self.version, &self.origin, self.current_xaml.as_ref()))
            }
            .expect("serializing inert rich source cannot fail");
            Sha256::digest(bytes).into()
        })
    }

    /// Size of the inert current source; this does not confer authority.
    pub fn source_bytes(&self) -> usize {
        self.current_xaml
            .len()
            .saturating_add(paragraph_origin::additional_wire_bytes(
                self.paragraph_origins.as_deref(),
            ))
    }
    /// Not a proof. Caller resolves this against the actual package owner,
    /// unique relationship and active presentation before from_wire.
    pub fn origin(&self) -> &NativeTextAnchor {
        &self.origin
    }
}

#[derive(Clone, Debug)]
pub struct OriginalTextAuthority {
    package: Arc<[u8]>,
    catalog: Arc<[u8]>,
    original_xaml: Arc<str>,
    original_index: Arc<SourceIndex>,
    keys: HashSet<String>,
    anchor: NativeTextAnchor,
}

impl OriginalTextAuthority {
    /// Caller resolves a unique native relationship/owner before constructing.
    /// This module validates member names/budgets; it does not resolve URI/file IO.
    pub fn from_package(
        identity: NativeTextIdentity,
        package: Vec<u8>,
        catalog: Vec<u8>,
    ) -> Result<Arc<Self>> {
        validate_identity(&identity)?;
        let keys = parse_catalog(&catalog)?;
        let original_xaml = read_package(&package)?;
        let original_index = Arc::new(parse_source(&original_xaml, &keys)?);
        let anchor = NativeTextAnchor {
            identity,
            package_sha256: hash(&package),
            original_xaml_sha256: hash(original_xaml.as_bytes()),
            catalog_sha256: hash(&catalog),
        };
        Ok(Arc::new(Self {
            package: package.into(),
            catalog: catalog.into(),
            original_xaml: original_xaml.into(),
            original_index,
            keys,
            anchor,
        }))
    }
    pub fn original_package(&self) -> &[u8] {
        &self.package
    }
    pub fn original_catalog(&self) -> &[u8] {
        &self.catalog
    }
    pub fn original_xaml(&self) -> &str {
        &self.original_xaml
    }
    pub fn original_plain_text(&self) -> &str {
        &self.original_index.plain
    }
    pub fn anchor(&self) -> &NativeTextAnchor {
        &self.anchor
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextSelection {
    pub paragraph: usize,
    pub anchor: usize,
    pub focus: usize,
}
impl TextSelection {
    fn sorted(self) -> Range<usize> {
        self.anchor.min(self.focus)..self.anchor.max(self.focus)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weight {
    Normal,
    Bold,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontStyle {
    Normal,
    Italic,
}

/// Exact source decimal, independently bounded; no f32 serialization round trip.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DipSize(String);
impl DipSize {
    pub fn parse(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        let canonical = canonical_decimal(&value)?;
        let integer = canonical.split('.').next().unwrap_or("");
        if canonical == "0"
            || integer.len() > 4
            || integer.parse::<u32>().map_err(err)? > 4096
            || (integer == "4096" && canonical.contains('.'))
        {
            return Err("DIP font size must be positive and at most 4096".into());
        }
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Closed editable collection subset. Absence is NOT a native default claim.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextDecorations {
    None,
    Underline,
}
impl TextDecorations {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Underline => "Underline",
        }
    }
    fn parse(value: &str) -> Result<Self> {
        match value {
            "None" => Ok(Self::None),
            "Underline" => Ok(Self::Underline),
            _ => Err("Unsupported editable TextDecorations collection".into()),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TextFormat {
    pub weight: Option<Weight>,
    pub style: Option<FontStyle>,
    pub size_dip: Option<DipSize>,
    pub foreground_argb: Option<[u8; 4]>,
    /// None means unknown/mixed for a query, or no requested property edit.
    pub text_decorations: Option<TextDecorations>,
}
impl TextFormat {
    fn properties(&self) -> Vec<(&'static str, String)> {
        let mut out = Vec::with_capacity(5);
        if let Some(weight) = self.weight {
            out.push((
                "FontWeight",
                match weight {
                    Weight::Normal => "Normal",
                    Weight::Bold => "Bold",
                }
                .into(),
            ));
        }
        if let Some(style) = self.style {
            out.push((
                "FontStyle",
                match style {
                    FontStyle::Normal => "Normal",
                    FontStyle::Italic => "Italic",
                }
                .into(),
            ));
        }
        if let Some(size) = &self.size_dip {
            out.push(("FontSize", size.0.clone()));
        }
        if let Some([a, r, g, b]) = self.foreground_argb {
            out.push(("Foreground", format!("#{a:02X}{r:02X}{g:02X}{b:02X}")));
        }
        if let Some(value) = self.text_decorations {
            out.push(("TextDecorations", value.as_str().into()));
        }
        out
    }
}

#[derive(Clone, Debug)]
struct IndexedRun {
    range: Range<usize>,
    opening_end: usize,
    closing_start: usize,
    paragraph: usize,
    scalar_start: usize,
    boundaries: Vec<usize>,
    effective: [Option<String>; 5],
}
#[derive(Clone, Debug)]
struct IndexedParagraph {
    range: Range<usize>,
    opening_end: usize,
    closing_start: usize,
}
#[derive(Clone, Debug)]
struct SourceIndex {
    paragraph_xml: Vec<IndexedParagraph>,
    runs: Vec<IndexedRun>,
    paragraphs: Vec<String>,
    plain: String,
    paragraph_starts: Vec<usize>,
}

/// All positions are Unicode scalar indices into our exact plain projection.
/// Run literal CR/LF are rejected; separators belong between Paragraphs only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RunCharRange {
    pub run_ordinal: usize,
    pub paragraph: usize,
    pub start_in_paragraph: usize,
    pub end_in_paragraph: usize,
    pub global_start: usize,
    pub global_end: usize,
}

/// Private proof-bearing runtime source. Arc/index clone is shallow, parsing is
/// performed at construction/accepted changes only, never by the getters.
#[derive(Clone, Debug)]
pub struct ValidatedRichText {
    wire: Arc<RichTextWire>,
    authority: Arc<OriginalTextAuthority>,
    index: Arc<SourceIndex>,
}

#[derive(Clone, Debug)]
pub struct TextPatch {
    pub source: ValidatedRichText,
    pub selection: TextSelection,
}

/// Private-constructor proof of an ORIGINAL→CURRENT inert formatting delta.
/// Wire admission/plain equality alone does not construct this receipt.
#[derive(Clone, Debug)]
pub struct FormatOnlyReceipt {
    source: ValidatedRichText,
}
impl FormatOnlyReceipt {
    pub fn source(&self) -> &ValidatedRichText {
        &self.source
    }
    pub fn export_package(&self) -> Result<Arc<[u8]>> {
        self.source.export_package()
    }
}

impl ValidatedRichText {
    pub fn from_original(
        authority: Arc<OriginalTextAuthority>,
        plain_mirror: &str,
    ) -> Result<Self> {
        let wire = RichTextWire {
            version: 1,
            current_xaml: authority.original_xaml.clone(),
            origin: authority.anchor.clone(),
            paragraph_origins: None,
            visual_fingerprint: std::sync::OnceLock::new(),
        };
        Self::from_wire(wire, authority, plain_mirror)
    }
    pub fn from_wire(
        wire: RichTextWire,
        authority: Arc<OriginalTextAuthority>,
        plain_mirror: &str,
    ) -> Result<Self> {
        if wire.version != 1 || wire.origin != authority.anchor {
            return Err("Rich text origin/version mismatch".into());
        }
        let index = if wire.current_xaml.as_ref() == authority.original_xaml.as_ref() {
            authority.original_index.clone()
        } else {
            Arc::new(parse_source(&wire.current_xaml, &authority.keys)?)
        };
        if index.plain != plain_mirror {
            return Err("Rich text source/plain mirror mismatch".into());
        }
        Self::from_prepared(wire, authority, index)
    }
    fn from_prepared(
        wire: RichTextWire,
        authority: Arc<OriginalTextAuthority>,
        index: Arc<SourceIndex>,
    ) -> Result<Self> {
        if wire.version != 1 || wire.origin != authority.anchor {
            return Err("Rich text origin/version mismatch".into());
        }
        if wire.source_bytes() > MAX_SOURCE_BYTES {
            return Err("Rich source plus provenance budget exceeded".into());
        }
        let validated = Self {
            wire: Arc::new(wire),
            authority,
            index,
        };
        validated.prove_preserved_metadata()?;
        Ok(validated)
    }
    pub fn wire(&self) -> &RichTextWire {
        &self.wire
    }
    pub fn xaml(&self) -> &str {
        &self.wire.current_xaml
    }
    pub fn plain_text(&self) -> &str {
        &self.index.plain
    }
    pub fn run_char_ranges(&self) -> impl Iterator<Item = RunCharRange> + '_ {
        self.index.runs.iter().enumerate().map(|(ordinal, run)| {
            let start = run.scalar_start;
            let end = start + run.boundaries.len() - 1;
            let global = self.index.paragraph_starts[run.paragraph];
            RunCharRange {
                run_ordinal: ordinal,
                paragraph: run.paragraph,
                start_in_paragraph: start,
                end_in_paragraph: end,
                global_start: global + start,
                global_end: global + end,
            }
        })
    }
    pub fn validate_plain(&self, mirror: &str) -> Result<()> {
        if mirror == self.plain_text() {
            Ok(())
        } else {
            Err("Rich text source/plain mirror mismatch".into())
        }
    }

    /// Uniform selected SOURCE values only. None means mixed/unknown, never
    /// an any/all toggle decision. Caller retains original toolbar state.
    /// Uses the prepared index; no XML parse or full source clone.
    pub fn selection_format(&self, selection: TextSelection) -> Result<TextFormat> {
        selected_index_format(&self.index, selection)
    }

    /// Prepared-index capability only; no XML/ZIP parse, source clone or
    /// speculative setter. Mixed known collections are supported, whereas any
    /// selected imported Run with missing decoration remains unknown.
    pub fn selection_text_decorations_supported(&self, selection: TextSelection) -> Result<bool> {
        selected_text_decorations_supported(&self.index, selection, false)
    }

    /// Strict source admission also binds unrelated original metadata for
    /// ordinary text edits. A content edit is NOT a format-only cache receipt.
    /// Adjacent identical non-format metadata groups are compared, preserving
    /// key/order boundaries without pretending original WPF Run split identity.
    pub fn prove_preserved_metadata(&self) -> Result<()> {
        if self.wire.paragraph_origins.is_some() {
            return paragraph_origin::prove_mapped_metadata(self);
        }
        if self.xaml() == self.authority.original_xaml.as_ref() {
            return Ok(());
        }
        if !paragraph_alignment::same_block_skeleton(
            &self.authority.original_xaml,
            &self.authority.original_index,
            self.xaml(),
            &self.index,
        )? || metadata_groups(self.xaml(), &self.index)?
            != metadata_groups(
                &self.authority.original_xaml,
                &self.authority.original_index,
            )?
        {
            return Err("Original non-format metadata/block/key groups changed".into());
        }
        Ok(())
    }

    /// Bounded committed input only. Caller supplies the existing Run owner,
    /// so no boundary typing-affinity or cross-Run inheritance is invented.
    /// Empty replacement retains the existing Run and all its opening attrs.
    pub fn replace_in_run(
        &self,
        run_ordinal: usize,
        start: usize,
        end: usize,
        committed_text: &str,
    ) -> Result<Option<TextPatch>> {
        let run = self
            .index
            .runs
            .get(run_ordinal)
            .ok_or("Unknown Run owner")?;
        let count = run.boundaries.len() - 1;
        if start > end || end > count {
            return Err("Invalid Run scalar range".into());
        }
        if committed_text.len() > MAX_SOURCE_BYTES
            || committed_text
                .chars()
                .any(|ch| ch < ' ' || matches!(ch, '\u{FFFE}' | '\u{FFFF}'))
        {
            return Err("Committed text control/newline/size unsupported".into());
        }
        let body = &self.xaml()[run.opening_end..run.closing_start];
        let (decoded, _) = decode_body(body)?;
        let old: String = decoded.chars().skip(start).take(end - start).collect();
        if old == committed_text {
            return Ok(None);
        }
        let replacement = committed_text
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        let range =
            (run.opening_end + run.boundaries[start])..(run.opening_end + run.boundaries[end]);
        let mut wire = self.wire.as_ref().clone();
        let mut current = self.xaml().to_owned();
        current.replace_range(range, &replacement);
        wire.current_xaml = current.into();
        wire.visual_fingerprint = std::sync::OnceLock::new();
        let parsed = Arc::new(parse_source(&wire.current_xaml, &self.authority.keys)?);
        let source = Self::from_prepared(wire, self.authority.clone(), parsed)?;
        let caret = run.scalar_start + start + committed_text.chars().count();
        Ok(Some(TextPatch {
            source,
            selection: TextSelection {
                paragraph: run.paragraph,
                anchor: caret,
                focus: caret,
            },
        }))
    }

    /// Revalidate a forged/untrusted wire's delta, not an authoring UI flag.
    /// Exact non-Run skeleton, original Run boundaries/owner attrs, closing
    /// tags and raw text lexemes must remain. Only nonempty partitions of each
    /// original Run and five closed formatting property additions/changes
    /// are permitted. Existing explicit properties cannot be removed.
    pub fn prove_format_only(&self) -> Result<FormatOnlyReceipt> {
        if self.wire.paragraph_origins.is_some() {
            return Err("Paragraph provenance is never format-only".into());
        }
        let original = &self.authority.original_index;
        if self.index.plain != original.plain
            || !paragraph_alignment::same_block_skeleton(
                &self.authority.original_xaml,
                original,
                self.xaml(),
                &self.index,
            )?
        {
            return Err("Source text/block/unrelated XML changed; no format-only receipt".into());
        }
        let mut current = 0usize;
        for old in &original.runs {
            let old_source = &self.authority.original_xaml;
            let old_open = &old_source[old.range.start..old.opening_end];
            let old_base = non_format_opening(old_open)?;
            let old_attrs = raw_attributes(old_open)?;
            let old_body = &old_source[old.opening_end..old.closing_start];
            let old_close = &old_source[old.closing_start..old.range.end];
            let mut consumed = 0usize;
            loop {
                let next = self.index.runs.get(current).ok_or("Original Run removed")?;
                let new_open = &self.xaml()[next.range.start..next.opening_end];
                let new_body = &self.xaml()[next.opening_end..next.closing_start];
                let new_close = &self.xaml()[next.closing_start..next.range.end];
                let new_attrs = raw_attributes(new_open)?;
                if next.paragraph != old.paragraph
                    || non_format_opening(new_open)? != old_base
                    || new_close != old_close
                {
                    return Err("Run key/owner/unrelated attributes changed".into());
                }
                if old_attrs
                    .iter()
                    .filter(|attr| property_slot(attr.name).is_some())
                    .any(|attr| !new_attrs.iter().any(|new| new.name == attr.name))
                {
                    return Err("Original explicit property removed".into());
                }
                if !old_body[consumed..].starts_with(new_body)
                    || (!old_body.is_empty() && new_body.is_empty())
                {
                    return Err("Original Run raw text/partition boundary changed".into());
                }
                consumed += new_body.len();
                current += 1;
                if consumed == old_body.len() {
                    break;
                }
            }
        }
        if current != self.index.runs.len() {
            return Err("Extra/moved Run outside original partition".into());
        }
        Ok(FormatOnlyReceipt {
            source: self.clone(),
        })
    }

    /// Nonempty same-Paragraph selection only. No collapsed typing/defaults or
    /// mixed-toggle inference. Equal SOURCE inherited values are true no-op.
    pub fn set_selection_format(
        &self,
        selection: TextSelection,
        format: &TextFormat,
    ) -> Result<Option<TextPatch>> {
        let selected = selection.sorted();
        let paragraph = self
            .index
            .paragraphs
            .get(selection.paragraph)
            .ok_or("Unknown Paragraph")?;
        if selected.is_empty() || selected.end > paragraph.chars().count() {
            return Err("Expected nonempty Paragraph selection".into());
        }
        let properties = format.properties();
        if properties.is_empty() {
            return Ok(None);
        }
        let mut patches = Vec::new();
        for run in self
            .index
            .runs
            .iter()
            .filter(|run| run.paragraph == selection.paragraph)
        {
            let length = run.boundaries.len() - 1;
            let lo = selected.start.saturating_sub(run.scalar_start).min(length);
            let hi = selected.end.saturating_sub(run.scalar_start).min(length);
            if lo >= hi {
                continue;
            }
            let mut changes = Vec::new();
            for (name, desired) in &properties {
                let slot = property_slot(name).ok_or("Unknown formatting property")?;
                let Some(current) = run.effective[slot].as_deref() else {
                    // Explicit toolbar collection replacement needs no prior
                    // collection. Do not infer absent imported decoration as
                    // None: materialize the requested canonical property even
                    // for explicit None. Other unknown properties stay closed.
                    if slot == 4 {
                        changes.push((*name, desired.as_str()));
                        continue;
                    }
                    return Err("Effective source property unknown; no default inference".into());
                };
                validate_property(name, current)?;
                let equal = if *name == "FontSize" {
                    canonical_decimal(current)? == canonical_decimal(desired)?
                } else if *name == "Foreground" {
                    current.eq_ignore_ascii_case(desired)
                } else {
                    current == desired
                };
                if !equal {
                    changes.push((*name, desired.as_str()));
                }
            }
            if changes.is_empty() {
                continue;
            }
            let opening = &self.xaml()[run.range.start..run.opening_end];
            let chosen = patch_opening(opening, &changes)?;
            let body = &self.xaml()[run.opening_end..run.closing_start];
            let closing = &self.xaml()[run.closing_start..run.range.end];
            let mut replacement = String::new();
            for (first, last, head) in [
                (0, lo, opening),
                (lo, hi, chosen.as_str()),
                (hi, length, opening),
            ] {
                if first < last {
                    replacement.push_str(head);
                    replacement.push_str(&body[run.boundaries[first]..run.boundaries[last]]);
                    replacement.push_str(closing);
                }
            }
            patches.push((run.range.clone(), replacement));
        }
        if patches.is_empty() {
            return Ok(None);
        }
        let mut candidate = self.xaml().to_owned();
        for (range, replacement) in patches.into_iter().rev() {
            candidate.replace_range(range, &replacement);
        }
        let mut wire = self.wire.as_ref().clone();
        wire.current_xaml = candidate.into();
        wire.visual_fingerprint = std::sync::OnceLock::new();
        let source = Self::from_wire(wire, self.authority.clone(), self.plain_text())?;
        Ok(Some(TextPatch { source, selection }))
    }

    /// Artifact package output only; caller owns reference/COW reconciliation.
    /// No-op returns the original Arc. Untouched members are raw-copied.
    pub fn export_package(&self) -> Result<Arc<[u8]>> {
        if self.xaml() == self.authority.original_xaml.as_ref() {
            return Ok(self.authority.package.clone());
        }
        let mut before =
            ZipArchive::new(Cursor::new(self.authority.package.as_ref())).map_err(err)?;
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .set_raw_comment(before.comment().to_vec().into_boxed_slice())
            .map_err(err)?;
        for ordinal in 0..before.len() {
            let file = before.by_index(ordinal).map_err(err)?;
            if file.name().map_err(err)?.as_ref() == INNER {
                let name = file.name().map_err(err)?.into_owned();
                let mut options = file.options().into_full_options();
                if !file.comment().is_empty() {
                    options = options.with_file_comment(file.comment());
                }
                if file.extra_data_fields().next().is_some() {
                    return Err("Modified XAML entry has unsupported ZIP extra metadata".into());
                }
                writer.start_file(name, options).map_err(err)?;
                writer.write_all(self.xaml().as_bytes()).map_err(err)?;
            } else {
                writer.raw_copy_file(file).map_err(err)?;
            }
        }
        let output = writer.finish().map_err(err)?.into_inner();
        // An accepted output must fit the SAME reopening contract, including
        // expanded total size after a Run split increases the XAML member.
        let reopened = read_package(&output)?;
        if reopened != self.xaml() {
            return Err("Exported inner XAML mismatch".into());
        }
        Ok(output.into())
    }
}

fn hash(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
fn err(error: impl std::fmt::Display) -> String {
    error.to_string()
}
fn validate_identity(identity: &NativeTextIdentity) -> Result<()> {
    for value in [&identity.owner_region_id, &identity.item_idref] {
        if value.is_empty() || value.len() > 1024 || value.chars().any(char::is_control) {
            return Err("Invalid owner/relationship identity".into());
        }
    }
    for name in [&identity.package_part, &identity.catalog_part] {
        validate_part_name(name)?;
    }
    Ok(())
}
fn validate_part_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name.len() > 4096
        || name.starts_with('/')
        || name.contains(['\\', ':', '\0'])
        || name
            .split('/')
            .any(|part| part.is_empty() || matches!(part, "." | ".."))
    {
        return Err("Unsafe/ambiguous package member name".into());
    }
    Ok(())
}
fn read_package(bytes: &[u8]) -> Result<String> {
    if bytes.len() > MAX_PACKAGE_BYTES {
        return Err("Compressed package budget exceeded".into());
    }
    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(err)?;
    if archive.len() > MAX_MEMBERS {
        return Err("Package member budget exceeded".into());
    }
    let mut names = HashSet::new();
    let mut total = 0u64;
    let mut xaml = None;
    for ordinal in 0..archive.len() {
        let mut file = archive.by_index(ordinal).map_err(err)?;
        let name = file.name().map_err(err)?.into_owned();
        validate_part_name(&name)?;
        if !names.insert(name.clone()) || file.is_dir() || file.encrypted() {
            return Err("Unsupported duplicate/directory/encrypted member".into());
        }
        total = total
            .checked_add(file.size())
            .ok_or("Expanded package size overflow")?;
        if total > MAX_EXPANDED_BYTES {
            return Err("Expanded package budget exceeded".into());
        }
        if name == INNER {
            if file.extra_data_fields().next().is_some() {
                return Err(
                    "Inner XAML ZIP extra metadata cannot be preserved by this bounded writer"
                        .into(),
                );
            }
            if file.size() > MAX_SOURCE_BYTES as u64 {
                return Err("Inner XAML source budget exceeded".into());
            }
            let mut data = Vec::new();
            file.by_ref()
                .take(MAX_SOURCE_BYTES as u64 + 1)
                .read_to_end(&mut data)
                .map_err(err)?;
            if data.len() > MAX_SOURCE_BYTES {
                return Err("Inner XAML source budget exceeded".into());
            }
            xaml = Some(String::from_utf8(data).map_err(err)?);
        }
    }
    xaml.ok_or_else(|| "Missing inner Xaml/Document.xaml".into())
}
fn parse_xml(source: &str) -> Result<Document<'_>> {
    if source.len() > MAX_SOURCE_BYTES || source.contains("<!") {
        return Err("XML budget/DTD/CDATA/comment unsupported".into());
    }
    let doc = Document::parse_with_options(
        source,
        ParsingOptions {
            allow_dtd: false,
            nodes_limit: MAX_XML_NODES,
            ..Default::default()
        },
    )
    .map_err(err)?;
    if doc
        .descendants()
        .any(|node| node.is_pi() || node.is_comment())
    {
        return Err("PI/comments unsupported".into());
    }
    Ok(doc)
}
fn parse_catalog(bytes: &[u8]) -> Result<HashSet<String>> {
    let source = std::str::from_utf8(bytes).map_err(err)?;
    let doc = parse_xml(source)?;
    let root = doc.root_element();
    if !root.has_tag_name((PRESENTATION, "presentation")) {
        return Err("Unexpected presentation catalog namespace/root".into());
    }
    let rendering = unique_child(root, PRESENTATION, "textRendering")?;
    let styles = unique_child(rendering, PRESENTATION, "textStyles")?;
    let mut keys = HashSet::new();
    for style in styles.children().filter(Node::is_element) {
        if !style.has_tag_name((PRESENTATION, "textStyle")) {
            return Err("Unknown catalog style element".into());
        }
        let key = style.attribute("style-key").ok_or("Missing style-key")?;
        if key.is_empty() || key.len() > 1024 || !keys.insert(key.to_owned()) {
            return Err("Empty/duplicate style-key".into());
        }
        if style.children().any(|child| child.is_element()) {
            return Err("Nested catalog style unsupported".into());
        }
    }
    if keys.is_empty() {
        return Err("Empty textStyles catalog".into());
    }
    for style in styles.children().filter(Node::is_element) {
        if let Some(parent) = style
            .attribute("based-on-key")
            .filter(|value| !value.is_empty())
            && !keys.contains(parent)
        {
            return Err("Unresolved catalog based-on-key".into());
        }
    }
    Ok(keys)
}
fn unique_child<'a, 'input>(
    parent: Node<'a, 'input>,
    namespace: &str,
    name: &str,
) -> Result<Node<'a, 'input>> {
    let mut nodes = parent
        .children()
        .filter(|node| node.has_tag_name((namespace, name)));
    let node = nodes.next().ok_or("Missing catalog container")?;
    if nodes.next().is_some() {
        return Err("Ambiguous catalog container".into());
    }
    Ok(node)
}

fn parse_source(source: &str, keys: &HashSet<String>) -> Result<SourceIndex> {
    let doc = parse_xml(source)?;
    let root = doc.root_element();
    if root.tag_name().namespace() != Some(XAML)
        || !matches!(root.tag_name().name(), "Section" | "FlowDocument")
    {
        return Err("Unsupported XAML root".into());
    }
    validate_attributes(root, false)?;
    let mut runs = Vec::new();
    let mut paragraphs = Vec::new();
    let mut paragraph_xml = Vec::new();
    let mut paragraph_starts = Vec::new();
    let mut global_scalar_position = 0usize;
    for paragraph in root.children() {
        if paragraph.is_text() && paragraph.text().unwrap_or("").is_empty() {
            continue;
        }
        if !paragraph.has_tag_name((XAML, "Paragraph")) {
            return Err("Only direct Paragraph is editable".into());
        }
        if paragraphs.len() >= 1024 {
            return Err("Paragraph budget exceeded".into());
        }
        validate_attributes(paragraph, false)?;
        let paragraph_index = paragraphs.len();
        let mut text = String::new();
        let mut paragraph_scalars = 0usize;
        let mut count = 0;
        for node in paragraph.children() {
            if node.is_text() && node.text().unwrap_or("").is_empty() {
                continue;
            }
            if !node.has_tag_name((XAML, "Run")) {
                return Err("Only direct Run is editable".into());
            }
            validate_attributes(node, true)?;
            if let Some(style) = node.attribute("style")
                && !keys.contains(style)
            {
                return Err("Unknown Run style key".into());
            }
            if node.children().any(|child| !child.is_text()) {
                return Err("Nested Run content unsupported".into());
            }
            if runs.len() >= MAX_RUNS {
                return Err("Run budget exceeded".into());
            }
            let range = node.range();
            let opening_end = tag_end(source.as_bytes(), range.start)?;
            if source[..opening_end].ends_with("/>") {
                return Err("Self-closing Run unsupported".into());
            }
            let closing_start = range.start
                + source[range.clone()]
                    .rfind("</")
                    .ok_or("Missing Run closing tag")?;
            let body = &source[opening_end..closing_start];
            let (decoded, boundaries) = decode_body(body)?;
            if decoded.contains(['\r', '\n']) {
                return Err(
                    "Run literal line breaks are outside the paragraph mapping subset".into(),
                );
            }
            let parsed: String = node.children().filter_map(|child| child.text()).collect();
            if decoded != parsed {
                return Err("XML text normalization unsupported".into());
            }
            let scalar_start = paragraph_scalars;
            // decode_body emits one boundary per Unicode scalar, plus zero.
            // Empty Runs add zero; entities and astral scalars each add one.
            paragraph_scalars += boundaries.len() - 1;
            text.push_str(&decoded);
            let mut effective: [Option<String>; 5] = Default::default();
            for ancestor in [root, paragraph, node] {
                for (slot, name) in ["FontWeight", "FontStyle", "FontSize", "Foreground"]
                    .iter()
                    .enumerate()
                {
                    if let Some(value) = ancestor.attribute(*name) {
                        effective[slot] = Some(value.to_owned());
                    }
                }
            }
            // Inline.TextDecorations is not an inheriting DP. Native catalog
            // application is separate and unproven here; only explicit Run
            // source contributes. Root/Paragraph decoration remains rejected.
            effective[4] = node.attribute("TextDecorations").map(str::to_owned);
            runs.push(IndexedRun {
                range,
                opening_end,
                closing_start,
                paragraph: paragraph_index,
                scalar_start,
                boundaries,
                effective,
            });
            count += 1;
        }
        if count == 0 {
            return Err("Empty Paragraph source shape unsupported".into());
        }
        let range = paragraph.range();
        let opening_end = tag_end(source.as_bytes(), range.start)?;
        let closing_start = range.start
            + source[range.clone()]
                .rfind("</")
                .ok_or("Missing Paragraph closing tag")?;
        paragraph_xml.push(IndexedParagraph {
            range,
            opening_end,
            closing_start,
        });
        paragraph_starts.push(global_scalar_position);
        // XML/source caps bound both counters; the separator is one scalar.
        global_scalar_position += paragraph_scalars + 1;
        paragraphs.push(text);
    }
    if paragraphs.is_empty() {
        return Err("Empty rich source unsupported".into());
    }
    let plain = paragraphs.join("\n");
    Ok(SourceIndex {
        paragraph_xml,
        runs,
        paragraphs,
        plain,
        paragraph_starts,
    })
}

fn validate_attributes(node: Node<'_, '_>, is_run: bool) -> Result<()> {
    for attr in node.attributes() {
        if attr.namespace() == Some(XML) && matches!(attr.name(), "lang" | "space") {
            continue;
        }
        if attr.namespace().is_some() {
            return Err("Foreign/identity-bearing source attribute unsupported".into());
        }
        let name = attr.name();
        let allowed = if is_run {
            matches!(
                name,
                "style"
                    | "Background"
                    | "FontWeight"
                    | "FontStyle"
                    | "FontSize"
                    | "Foreground"
                    | "TextDecorations"
            )
        } else {
            matches!(
                name,
                "FlowDirection"
                    | "FontFamily"
                    | "FontStyle"
                    | "FontWeight"
                    | "FontStretch"
                    | "FontSize"
                    | "Foreground"
                    | "Background"
                    | "TextAlignment"
                    | "LineHeight"
                    | "IsHyphenationEnabled"
                    | "NumberSubstitution.CultureSource"
                    | "NumberSubstitution.Substitution"
            ) || typography_property(name)
        };
        if !allowed || attr.value().contains(['{', '}']) {
            return Err(format!("Unsupported source attribute {name}"));
        }
        if property_slot(name).is_some() {
            validate_property(name, attr.value())?;
        }
        if name == "Background" {
            validate_argb(attr.value())?;
        }
    }
    Ok(())
}
fn typography_property(name: &str) -> bool {
    let Some(value) = name.strip_prefix("Typography.") else {
        return false;
    };
    matches!(
        value,
        "AnnotationAlternates"
            | "CapitalSpacing"
            | "Capitals"
            | "CaseSensitiveForms"
            | "ContextualAlternates"
            | "ContextualLigatures"
            | "ContextualSwashes"
            | "DiscretionaryLigatures"
            | "EastAsianExpertForms"
            | "EastAsianLanguage"
            | "EastAsianWidths"
            | "Fraction"
            | "HistoricalForms"
            | "HistoricalLigatures"
            | "Kerning"
            | "MathematicalGreek"
            | "NumeralAlignment"
            | "NumeralStyle"
            | "SlashedZero"
            | "StandardLigatures"
            | "StandardSwashes"
            | "StylisticAlternates"
            | "Variants"
    ) || value
        .strip_prefix("StylisticSet")
        .and_then(|number| number.parse::<u8>().ok())
        .is_some_and(|number| (1..=20).contains(&number))
}
fn property_slot(name: &str) -> Option<usize> {
    match name {
        "FontWeight" => Some(0),
        "FontStyle" => Some(1),
        "FontSize" => Some(2),
        "Foreground" => Some(3),
        "TextDecorations" => Some(4),
        _ => None,
    }
}
fn validate_property(name: &str, value: &str) -> Result<()> {
    match name {
        "FontWeight" if matches!(value, "Normal" | "Bold") => Ok(()),
        "FontStyle" if matches!(value, "Normal" | "Italic") => Ok(()),
        "FontSize" => DipSize::parse(value).map(|_| ()),
        "Foreground" => validate_argb(value),
        "TextDecorations" => TextDecorations::parse(value).map(|_| ()),
        _ => Err(format!("Unsupported effective {name} value")),
    }
}
fn validate_argb(value: &str) -> Result<()> {
    if value.len() == 9
        && value.starts_with('#')
        && value.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
    {
        Ok(())
    } else {
        Err("Only literal #AARRGGBB is supported".into())
    }
}
fn canonical_decimal(value: &str) -> Result<String> {
    if value.is_empty() || value.len() > 64 {
        return Err("Decimal size lexical budget exceeded".into());
    }
    let mut parts = value.split('.');
    let integer = parts.next().unwrap_or("");
    let fraction = parts.next();
    if integer.is_empty()
        || !integer.bytes().all(|b| b.is_ascii_digit())
        || parts.next().is_some()
        || fraction.is_some_and(|f| f.is_empty() || !f.bytes().all(|b| b.is_ascii_digit()))
    {
        return Err("Expected finite unsigned DIP decimal".into());
    }
    let integer = integer.trim_start_matches('0');
    let integer = if integer.is_empty() { "0" } else { integer };
    let fraction = fraction.unwrap_or("").trim_end_matches('0');
    Ok(if fraction.is_empty() {
        integer.to_owned()
    } else {
        format!("{integer}.{fraction}")
    })
}

#[derive(Debug)]
struct Attribute<'a> {
    name: &'a str,
    value: &'a str,
    value_range: Range<usize>,
    full_range: Range<usize>,
}
fn raw_attributes(opening: &str) -> Result<Vec<Attribute<'_>>> {
    let bytes = opening.as_bytes();
    let mut i = 1;
    while i < bytes.len() && !bytes[i].is_ascii_whitespace() && !matches!(bytes[i], b'/' | b'>') {
        i += 1;
    }
    let mut attrs = Vec::new();
    loop {
        let full_start = i;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() || matches!(bytes[i], b'>' | b'/') {
            break;
        }
        let start = i;
        while i < bytes.len() && !bytes[i].is_ascii_whitespace() && bytes[i] != b'=' {
            i += 1;
        }
        let end = i;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if bytes.get(i) != Some(&b'=') {
            return Err("Unsupported raw attribute syntax".into());
        }
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let quote = *bytes.get(i).ok_or("Missing attribute quote")?;
        if !matches!(quote, b'\'' | b'"') {
            return Err("Missing attribute quote".into());
        }
        i += 1;
        let value_start = i;
        while i < bytes.len() && bytes[i] != quote {
            i += 1;
        }
        if i == bytes.len() {
            return Err("Unclosed attribute".into());
        }
        attrs.push(Attribute {
            name: &opening[start..end],
            value: &opening[value_start..i],
            value_range: value_start..i,
            full_range: full_start..i + 1,
        });
        i += 1;
    }
    Ok(attrs)
}
fn non_format_opening(opening: &str) -> Result<String> {
    let mut result = opening.to_owned();
    let attrs = raw_attributes(opening)?;
    for attr in attrs
        .iter()
        .rev()
        .filter(|attr| property_slot(attr.name).is_some())
    {
        result.replace_range(attr.full_range.clone(), "");
    }
    Ok(result)
}
fn outside_runs(source: &str, index: &SourceIndex) -> String {
    outside_run_ranges(source, index.runs.iter().map(|run| run.range.clone()))
}
/// Shared exact skeleton operation; the mapped proof supplies the byte ranges
/// of unchanged current Runs after their full raw Paragraph bodies are folded.
fn outside_run_ranges(source: &str, ranges: impl IntoIterator<Item = Range<usize>>) -> String {
    let mut out = String::new();
    let mut last = 0;
    for range in ranges {
        debug_assert!(last <= range.start && range.start <= range.end && range.end <= source.len());
        out.push_str(&source[last..range.start]);
        last = range.end;
    }
    out.push_str(&source[last..]);
    out
}
fn metadata_groups(source: &str, index: &SourceIndex) -> Result<Vec<(usize, String, String)>> {
    let mut groups = Vec::new();
    for run in &index.runs {
        let group = (
            run.paragraph,
            non_format_opening(&source[run.range.start..run.opening_end])?,
            source[run.closing_start..run.range.end].to_owned(),
        );
        if groups.last() != Some(&group) {
            groups.push(group);
        }
    }
    Ok(groups)
}
fn tag_end(source: &[u8], start: usize) -> Result<usize> {
    let mut quote = None;
    for (offset, byte) in source.iter().enumerate().skip(start) {
        match quote {
            Some(q) if q == *byte => quote = None,
            None if matches!(byte, b'\'' | b'"') => quote = Some(*byte),
            None if *byte == b'>' => return Ok(offset + 1),
            _ => (),
        }
    }
    Err("Unclosed XML tag".into())
}
fn patch_opening(opening: &str, changes: &[(&str, &str)]) -> Result<String> {
    let attrs = raw_attributes(opening)?;
    let mut patches = Vec::new();
    let mut additions = String::new();
    for (name, value) in changes {
        validate_property(name, value)?;
        if let Some(attr) = attrs.iter().find(|attr| attr.name == *name) {
            if attr.value != *value {
                patches.push((attr.value_range.clone(), *value));
            }
        } else {
            additions.push_str(&format!(" {name}=\"{value}\""));
        }
    }
    let mut result = opening.to_owned();
    patches.sort_by_key(|(range, _)| range.start);
    for (range, value) in patches.into_iter().rev() {
        result.replace_range(range, value);
    }
    result.insert_str(result.len() - 1, &additions);
    Ok(result)
}
fn decode_body(body: &str) -> Result<(String, Vec<usize>)> {
    let mut text = String::new();
    let mut boundaries = vec![0];
    let mut offset = 0;
    while offset < body.len() {
        let tail = &body[offset..];
        let (ch, length) = if tail.starts_with('&') {
            let last = tail.find(';').ok_or("Unclosed entity")?;
            let token = &tail[..=last];
            let ch = match token {
                "&amp;" => '&',
                "&lt;" => '<',
                "&gt;" => '>',
                "&quot;" => '"',
                "&apos;" => '\'',
                _ => {
                    let digits = token
                        .strip_prefix("&#")
                        .and_then(|value| value.strip_suffix(';'))
                        .ok_or("Unsupported entity")?;
                    let number = if let Some(hex) = digits.strip_prefix('x') {
                        u32::from_str_radix(hex, 16)
                    } else {
                        digits.parse()
                    }
                    .map_err(err)?;
                    char::from_u32(number).ok_or("Non-scalar entity")?
                }
            };
            (ch, last + 1)
        } else {
            let ch = tail.chars().next().ok_or("Missing scalar")?;
            if matches!(ch, '<' | '\r') {
                return Err("Nested markup/literal CR normalization unsupported".into());
            }
            (ch, ch.len_utf8())
        };
        text.push(ch);
        offset += length;
        boundaries.push(offset);
    }
    Ok((text, boundaries))
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "rich_text_source/scalar_index_tests.rs"]
mod scalar_index_tests;

/// Shared read-only effective-property query; source admission stays separate.
fn selected_index_format(index: &SourceIndex, selection: TextSelection) -> Result<TextFormat> {
    selected_index_format_with_decoration_default(index, selection, None)
}

fn selected_text_decorations_supported(
    index: &SourceIndex,
    selection: TextSelection,
    generated_default: bool,
) -> Result<bool> {
    let range = selection.sorted();
    let paragraph = index
        .paragraphs
        .get(selection.paragraph)
        .ok_or("Unknown Paragraph")?;
    if range.is_empty() || range.end > paragraph.chars().count() {
        return Err("Expected nonempty Paragraph selection".into());
    }
    let mut seen = false;
    for run in index
        .runs
        .iter()
        .filter(|run| run.paragraph == selection.paragraph)
    {
        let end = run.scalar_start + run.boundaries.len() - 1;
        if end <= range.start || run.scalar_start >= range.end {
            continue;
        }
        seen = true;
        if !generated_default && run.effective[4].is_none() {
            return Ok(false);
        }
    }
    Ok(seen)
}

// The only caller supplying a default is an already proved generated authored
// scaffold. Imported source never obtains this policy from an absent attribute.
fn selected_index_format_with_decoration_default(
    index: &SourceIndex,
    selection: TextSelection,
    default: Option<TextDecorations>,
) -> Result<TextFormat> {
    let decoration_default = default.map(TextDecorations::as_str);
    let range = selection.sorted();
    let paragraph = index
        .paragraphs
        .get(selection.paragraph)
        .ok_or("Unknown Paragraph")?;
    if range.is_empty() || range.end > paragraph.chars().count() {
        return Err("Expected nonempty Paragraph selection".into());
    }
    let mut values: [Option<String>; 5] = Default::default();
    let mut mixed = [false; 5];
    let mut seen = false;
    for run in index
        .runs
        .iter()
        .filter(|run| run.paragraph == selection.paragraph)
    {
        let end = run.scalar_start + run.boundaries.len() - 1;
        if end <= range.start || run.scalar_start >= range.end {
            continue;
        }
        for slot in 0..5 {
            let next = run.effective[slot].as_deref().or(if slot == 4 {
                decoration_default
            } else {
                None
            });
            if !seen {
                values[slot] = next.map(str::to_owned);
            } else {
                let same = match (values[slot].as_deref(), next) {
                    (Some(a), Some(b)) if slot == 2 => {
                        canonical_decimal(a)? == canonical_decimal(b)?
                    }
                    (Some(a), Some(b)) if slot == 3 => a.eq_ignore_ascii_case(b),
                    (a, b) => a == b,
                };
                if !same {
                    mixed[slot] = true;
                }
            }
        }
        seen = true;
    }
    for slot in 0..5 {
        if mixed[slot] {
            values[slot] = None;
        }
    }
    let weight = match values[0].as_deref() {
        Some("Normal") => Some(Weight::Normal),
        Some("Bold") => Some(Weight::Bold),
        _ => None,
    };
    let style = match values[1].as_deref() {
        Some("Normal") => Some(FontStyle::Normal),
        Some("Italic") => Some(FontStyle::Italic),
        _ => None,
    };
    let size_dip = values[2].take().map(DipSize::parse).transpose()?;
    let foreground_argb = values[3]
        .as_deref()
        .map(|color| -> Result<[u8; 4]> {
            validate_argb(color)?;
            let mut channels = [0u8; 4];
            for (index, channel) in channels.iter_mut().enumerate() {
                *channel = u8::from_str_radix(&color[(index * 2 + 1)..(index * 2 + 3)], 16)
                    .map_err(err)?;
            }
            Ok(channels)
        })
        .transpose()?;
    Ok(TextFormat {
        weight,
        style,
        size_dip,
        foreground_argb,
        text_decorations: values[4]
            .as_deref()
            .map(TextDecorations::parse)
            .transpose()?,
    })
}

#[cfg(test)]
#[path = "rich_text_source/underline_tests.rs"]
mod underline_tests;
