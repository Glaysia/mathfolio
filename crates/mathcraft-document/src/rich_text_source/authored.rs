//! Generated scaffold is Mathcraft policy, not captured
//! Prime fresh-empty serialization. No fabricated NativeTextAnchor is used.
//! Separate authored provenance; imported wire and authority are unchanged.
use super::*;
use std::sync::OnceLock;

mod creation;
pub use creation::AuthoredCreationSnapshot;

const MAX_CATALOG_STYLES: usize = 256;
const MAX_SEED_BYTES: usize = 128 * 1024;
const ROOT_END: &str = "</FlowDocument>";
const P_OPEN: &str = "<Paragraph>";
const P_END: &str = "</Paragraph>";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoredStyleSnapshot {
    pub key: String,
    pub font_family: String,
    /// Resolved XAML DIP, not presentation10 font-size points.
    pub size_dip: String,
    pub weight: String,
    pub font_style: String,
    /// First slice accepts only explicit Left paragraph geometry.
    pub alignment: String,
    pub foreground_rgba: [u8; 4],
    pub background_rgba: [u8; 4],
}

/// Closed internal resolved catalog DTO, not native presentation10 XML.
/// Resolving native name/based-on/implicit Normal is a separate adapter task.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoredTextSeedSpec {
    pub format: AuthoredStyleSnapshot,
    pub proofing_language: String,
    pub catalog: Vec<AuthoredStyleSnapshot>,
    /// Local current-format decoration, not a native catalog default. Missing
    /// keeps the exact v1 seed JSON/hash/generated scaffold. Present null is not
    /// an additional spelling for absence.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_decorations"
    )]
    pub text_decorations: Option<TextDecorations>,
}

fn deserialize_decorations<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<TextDecorations>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    TextDecorations::deserialize(deserializer).map(Some)
}

#[derive(Clone, Debug)]
pub struct AuthoredTextSeed {
    spec: Arc<AuthoredTextSeedSpec>,
    keys: HashSet<String>,
    root_open: Arc<str>,
    run_open: Arc<str>,
    original_xaml: Arc<str>,
    original_index: Arc<SourceIndex>,
    fingerprint: [u8; 32],
    serialized_spec_bytes: usize,
    encoded_hash_bytes: usize,
    wire_fixed_bytes: usize,
}

fn plain_attribute(value: &str, maximum: usize) -> Result<()> {
    if value.is_empty()
        || value.len() > maximum
        || value
            .chars()
            .any(|c| c < ' ' || matches!(c, '\u{FFFE}' | '\u{FFFF}' | '{' | '}'))
    {
        return Err("Unsupported authored literal attribute".into());
    }
    Ok(())
}
fn escaped(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
fn argb([r, g, b, a]: [u8; 4]) -> String {
    format!("#{a:02X}{r:02X}{g:02X}{b:02X}")
}
fn validate_style(style: &AuthoredStyleSnapshot) -> Result<()> {
    plain_attribute(&style.key, 1024)?;
    plain_attribute(&style.font_family, 256)?;
    if style.size_dip.len() > 64 {
        return Err("Authored DIP lexical budget exceeded".into());
    }
    DipSize::parse(style.size_dip.clone())?;
    validate_property("FontWeight", &style.weight)?;
    validate_property("FontStyle", &style.font_style)?;
    if style.alignment != "Left" {
        return Err("Unsupported authored paragraph alignment".into());
    }
    Ok(())
}
impl AuthoredTextSeed {
    pub fn new(spec: AuthoredTextSeedSpec) -> Result<Arc<Self>> {
        Self::from_shared(Arc::new(spec))
    }
    /// Own-file admission regenerates once from an immutable shared DTO. Edits
    /// reuse this validated seed; no catalog deepcopy or serialization per key.
    fn from_shared(spec: Arc<AuthoredTextSeedSpec>) -> Result<Arc<Self>> {
        validate_style(&spec.format)?;
        if spec.catalog.is_empty() || spec.catalog.len() > MAX_CATALOG_STYLES {
            return Err("Authored catalog count budget exceeded".into());
        }
        let language = &spec.proofing_language;
        if language.is_empty()
            || language.len() > 64
            || !language
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-')
            || language.starts_with('-')
            || language.ends_with('-')
            || language.contains("--")
        {
            return Err("Unsupported explicit proofing language".into());
        }
        let mut keys = HashSet::new();
        for entry in &spec.catalog {
            validate_style(entry)?;
            if !keys.insert(entry.key.clone()) {
                return Err("Duplicate authored style key".into());
            }
        }
        if !keys.contains(&spec.format.key) {
            return Err("Unknown authored format style key".into());
        }
        // Current format may be a local override of the selected catalog style;
        // catalog equality is deliberately not inferred from the style key.
        let encoded = serde_json::to_vec(spec.as_ref()).map_err(err)?;
        if encoded.len() > MAX_SEED_BYTES {
            return Err("Authored seed byte budget exceeded".into());
        }
        let f = &spec.format;
        let root_open:Arc<str>=format!("<FlowDocument xmlns=\"{XAML}\" xml:space=\"preserve\" xml:lang=\"{}\" TextAlignment=\"Left\" FontFamily=\"{}\" Background=\"{}\">",escaped(language),escaped(&f.font_family),argb(f.background_rgba)).into();
        let decorations = spec
            .text_decorations
            .map(|value| format!(" TextDecorations=\"{}\"", value.as_str()))
            .unwrap_or_default();
        let run_open:Arc<str>=format!("<Run style=\"{}\" FontWeight=\"{}\" FontStyle=\"{}\" FontSize=\"{}\" Foreground=\"{}\"{decorations}>",escaped(&f.key),f.weight,f.font_style,f.size_dip,argb(f.foreground_rgba)).into();
        let original_xaml: Arc<str> =
            format!("{root_open}{P_OPEN}{run_open}</Run>{P_END}{ROOT_END}").into();
        let original_index = Arc::new(parse_source(&original_xaml, &keys)?);
        let fingerprint: [u8; 32] = Sha256::digest(&encoded).into();
        let encoded_hash_bytes = serde_json::to_vec(&fingerprint).map_err(err)?.len();
        // Compact derive field order is covered by independent serde oracle.
        let wire_fixed_bytes = b"{\"version\":1,\"seed\":".len()
            + encoded.len()
            + b",\"seed_sha256\":".len()
            + encoded_hash_bytes
            + b",\"current_xaml\":".len()
            + 1;
        Ok(Arc::new(Self {
            serialized_spec_bytes: encoded.len(),
            encoded_hash_bytes,
            wire_fixed_bytes,
            spec,
            keys,
            root_open,
            run_open,
            original_xaml,
            original_index,
            fingerprint,
        }))
    }
    pub fn spec(&self) -> &AuthoredTextSeedSpec {
        &self.spec
    }
    pub fn fingerprint(&self) -> [u8; 32] {
        self.fingerprint
    }
    pub fn serialized_spec_bytes(&self) -> usize {
        self.serialized_spec_bytes
    }
    pub fn encoded_hash_bytes(&self) -> usize {
        self.encoded_hash_bytes
    }
}

/// Untrusted own-file DTO, admitted only by from_wire regeneration and proof.
/// Separate type means old native RichTextWire JSON is completely unchanged.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoredTextWire {
    version: u8,
    seed: Arc<AuthoredTextSeedSpec>,
    seed_sha256: [u8; 32],
    current_xaml: Arc<str>,
    // Derived non-authority facts only: immutable clones share them, wire omits
    // them, and from_wire still regenerates/proves every untrusted source.
    #[serde(skip)]
    derived: Arc<OnceLock<AuthoredWireFacts>>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AuthoredWireFacts {
    serialized_wire_bytes: usize,
    visual_fingerprint: [u8; 32],
}
impl PartialEq for AuthoredTextWire {
    fn eq(&self, other: &Self) -> bool {
        self.version == other.version
            && self.seed == other.seed
            && self.seed_sha256 == other.seed_sha256
            && self.current_xaml == other.current_xaml
    }
}
impl Eq for AuthoredTextWire {}
impl AuthoredTextWire {
    /// Exact immutable-version identity for retained runtime caches. False for
    /// independently deserialized equal bytes; caller then performs admission.
    pub fn same_source_version(&self, other: &Self) -> bool {
        self.version == other.version
            && self.seed_sha256 == other.seed_sha256
            && Arc::ptr_eq(&self.seed, &other.seed)
            && Arc::ptr_eq(&self.current_xaml, &other.current_xaml)
    }
    /// None means raw cold data has not completed full source admission yet.
    pub fn cached_serialized_wire_bytes(&self) -> Option<usize> {
        self.derived.get().map(|facts| facts.serialized_wire_bytes)
    }
    /// Visual invalidation token, never native or calculation authority.
    pub fn cached_visual_fingerprint(&self) -> Option<[u8; 32]> {
        self.derived.get().map(|facts| facts.visual_fingerprint)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuthoredTextBudget {
    /// Exact serde_json bytes of the complete own wire (seed appears once).
    serialized_wire_bytes: usize,
    current_xml_bytes: usize,
    serialized_seed_bytes: usize,
}
impl AuthoredTextBudget {
    pub fn serialized_wire_bytes(self) -> usize {
        self.serialized_wire_bytes
    }
    pub fn current_xml_bytes(self) -> usize {
        self.current_xml_bytes
    }
    pub fn serialized_seed_bytes(self) -> usize {
        self.serialized_seed_bytes
    }
}
/// Exact wire payload cost per owner, even when immutable seeds are shared.
/// Caller adds plain mirrors/other document sources under its existing cap.
pub fn checked_authored_wire_bytes<'a>(
    values: impl IntoIterator<Item = &'a AuthoredRichText>,
    limit: usize,
) -> Result<usize> {
    values.into_iter().try_fold(0usize, |sum, value| {
        sum.checked_add(value.budget.serialized_wire_bytes)
            .filter(|n| *n <= limit)
            .ok_or_else(|| "Aggregate authored wire budget exceeded".into())
    })
}
#[derive(Clone, Debug)]
pub struct AuthoredRichText {
    wire: Arc<AuthoredTextWire>,
    seed: Arc<AuthoredTextSeed>,
    index: Arc<SourceIndex>,
    budget: AuthoredTextBudget,
}
#[derive(Clone, Debug)]
pub struct AuthoredTextPatch {
    pub source: AuthoredRichText,
    pub selection: TextSelection,
    /// Present after split, valid only for this returned immutable source.
    pub following_run: Option<usize>,
}

/// Compact serde_json string encoding byte cost, including outer quotes.
/// UTF8 is retained verbatim; JSON controls/quotes/backslash are escaped.
/// Independent serde_json tests guard this serialization-specific arithmetic.
fn json_string_bytes(value: &str) -> Result<usize> {
    value.bytes().try_fold(2usize, |sum, b| {
        let count = match b {
            b'"' | b'\\' | b'\x08' | b'\t' | b'\n' | b'\x0c' | b'\r' => 2,
            0..=0x1f => 6,
            _ => 1,
        };
        sum.checked_add(count)
            .ok_or_else(|| "Authored JSON string cost overflow".into())
    })
}

impl AuthoredRichText {
    pub fn empty(seed: Arc<AuthoredTextSeed>) -> Result<Self> {
        Self::prepared(
            AuthoredTextWire {
                version: 1,
                seed: seed.spec.clone(),
                seed_sha256: seed.fingerprint,
                current_xaml: seed.original_xaml.clone(),
                derived: Default::default(),
            },
            seed.clone(),
            seed.original_index.clone(),
        )
    }
    pub fn from_wire(wire: AuthoredTextWire, plain: &str) -> Result<Self> {
        // A caller may deserialize the inert DTO independently of from_json.
        // Reject oversized fields/count BEFORE cloning that untrusted seed.
        if wire.version != 1
            || wire.current_xaml.len() > MAX_SOURCE_BYTES
            || wire.seed.catalog.is_empty()
            || wire.seed.catalog.len() > MAX_CATALOG_STYLES
            || wire.seed.proofing_language.len() > 64
        {
            return Err("Authored untrusted wire preflight exceeded".into());
        }
        validate_style(&wire.seed.format)?;
        for style in &wire.seed.catalog {
            validate_style(style)?;
        }
        let seed = AuthoredTextSeed::from_shared(wire.seed.clone())?;
        if wire.version != 1 || wire.seed_sha256 != seed.fingerprint {
            return Err("Authored seed/version mismatch".into());
        }
        let index = if wire.current_xaml == seed.original_xaml {
            seed.original_index.clone()
        } else {
            Arc::new(parse_source(&wire.current_xaml, &seed.keys)?)
        };
        if index.plain != plain {
            return Err("Authored source/plain mismatch".into());
        }
        Self::prepared(wire, seed, index)
    }
    /// Preferred untrusted entry point: bound raw JSON BEFORE serde allocates.
    /// Deserializing the DTO alone never grants source authority.
    pub fn from_json(bytes: &[u8], plain: &str) -> Result<Self> {
        if bytes.len() > MAX_SOURCE_BYTES {
            return Err("Authored own JSON budget exceeded".into());
        }
        Self::from_wire(serde_json::from_slice(bytes).map_err(err)?, plain)
    }
    fn prepared(
        wire: AuthoredTextWire,
        seed: Arc<AuthoredTextSeed>,
        index: Arc<SourceIndex>,
    ) -> Result<Self> {
        if wire.version != 1
            || !Arc::ptr_eq(&wire.seed, &seed.spec)
            || wire.seed_sha256 != seed.fingerprint
            || wire.current_xaml.len() > MAX_SOURCE_BYTES
        {
            return Err("Authored identity/source budget mismatch".into());
        }
        // Exact own wire cost, including JSON escapes and seed/catalog copies.
        // Controller sums this once PER carrier, not once per Arc pointer.
        let serialized_wire_bytes = seed
            .wire_fixed_bytes
            .checked_add(json_string_bytes(&wire.current_xaml)?)
            .ok_or("Authored JSON cost overflow")?;
        if serialized_wire_bytes > MAX_SOURCE_BYTES {
            return Err("Authored serialized wire budget exceeded".into());
        }
        let budget = AuthoredTextBudget {
            serialized_wire_bytes,
            current_xml_bytes: wire.current_xaml.len(),
            serialized_seed_bytes: seed.serialized_spec_bytes,
        };
        let value = Self {
            wire: Arc::new(wire),
            seed,
            index,
            budget,
        };
        value.prove_generated_source()?;
        let mut hash = Sha256::new();
        hash.update(b"mathcraft-authored-text-visual-v1\0");
        hash.update([value.wire.version]);
        hash.update(value.wire.seed_sha256);
        hash.update((value.xaml().len() as u64).to_le_bytes());
        hash.update(value.xaml().as_bytes());
        let facts = AuthoredWireFacts {
            serialized_wire_bytes,
            visual_fingerprint: hash.finalize().into(),
        };
        if let Some(old) = value.wire.derived.get() {
            if *old != facts {
                return Err("Stale authored derived source facts".into());
            }
        } else {
            // Racing admission of the same immutable bytes computes equal facts.
            let _ = value.wire.derived.set(facts);
        }
        Ok(value)
    }
    fn prove_generated_source(&self) -> Result<()> {
        // Exact root/Paragraph skeleton forbids injected spans/resources/attrs,
        // QName changes and non-Run gaps; source parser already rejects active XML.
        let count = self.index.paragraphs.len();
        let mut skeleton = String::from(self.seed.root_open.as_ref());
        for _ in 0..count {
            skeleton.push_str(P_OPEN);
            skeleton.push_str(P_END);
        }
        skeleton.push_str(ROOT_END);
        if count == 0
            || (outside_runs(self.xaml(), &self.index) != skeleton
                && super::paragraph_alignment::authored_block_skeleton(self.xaml(), &self.index)?
                    != skeleton)
        {
            return Err("Authored source scaffold changed".into());
        }
        let base = non_format_opening(&self.seed.run_open)?;
        for run in &self.index.runs {
            let open = &self.xaml()[run.range.start..run.opening_end];
            if non_format_opening(open)? != base
                || &self.xaml()[run.closing_start..run.range.end] != "</Run>"
            {
                return Err("Authored Run metadata changed".into());
            }
            let attrs = raw_attributes(open)?;
            for name in ["FontWeight", "FontStyle", "FontSize", "Foreground"] {
                let value = attrs
                    .iter()
                    .find(|a| a.name == name)
                    .ok_or("Missing authored explicit format")?;
                validate_property(name, value.value)?;
            }
        }
        Ok(())
    }
    pub fn wire(&self) -> &AuthoredTextWire {
        &self.wire
    }
    pub fn seed(&self) -> &Arc<AuthoredTextSeed> {
        &self.seed
    }
    pub fn xaml(&self) -> &str {
        &self.wire.current_xaml
    }
    pub fn plain_text(&self) -> &str {
        &self.index.plain
    }
    pub fn budget(&self) -> AuthoredTextBudget {
        self.budget
    }
    /// Uniform effective values from this admitted authored index.
    pub fn selection_format(&self, selection: TextSelection) -> Result<TextFormat> {
        // Construction/from_wire completed prove_generated_source before this
        // query is reachable. This is our closed scaffold's no-decoration
        // policy, not a claim about missing native catalog/inherited values.
        selected_index_format_with_decoration_default(
            &self.index,
            selection,
            Some(TextDecorations::None),
        )
    }
    /// This source already passed the closed scaffold proof. Missing Run
    /// decoration follows generated-source policy, never native inference.
    pub fn selection_text_decorations_supported(&self, selection: TextSelection) -> Result<bool> {
        selected_text_decorations_supported(&self.index, selection, true)
    }
    pub fn run_char_ranges(&self) -> impl Iterator<Item = RunCharRange> + '_ {
        self.index.runs.iter().enumerate().map(|(ordinal, r)| {
            let global = self.index.paragraph_starts[r.paragraph];
            RunCharRange {
                run_ordinal: ordinal,
                paragraph: r.paragraph,
                start_in_paragraph: r.scalar_start,
                end_in_paragraph: r.scalar_start + r.boundaries.len() - 1,
                global_start: global + r.scalar_start,
                global_end: global + r.scalar_start + r.boundaries.len() - 1,
            }
        })
    }
    fn changed(&self, source: String) -> Result<Self> {
        if source.len() > MAX_SOURCE_BYTES {
            return Err("Authored XML budget exceeded".into());
        }
        let index = Arc::new(parse_source(&source, &self.seed.keys)?);
        let mut wire = self.wire.as_ref().clone();
        wire.current_xaml = source.into();
        wire.derived = Default::default();
        Self::prepared(wire, self.seed.clone(), index)
    }
    /// Metadata-preserving selection deletion, without future typing affinity.
    pub fn delete_cross_run_selection(
        &self,
        selection: TextSelection,
    ) -> Result<CrossRunPatch<Self>> {
        self.apply_cross_run(selection, "", None)
    }

    /// Ordinary committed typing. No retained springload override, paste,
    /// IME grouping or cross-Paragraph replacement is inferred here.
    pub fn replace_cross_run_selection(
        &self,
        selection: TextSelection,
        text: &str,
        context: CrossRunTypingContext<'_>,
    ) -> Result<CrossRunPatch<Self>> {
        if text.is_empty() {
            return Err("Use explicit selection deletion".into());
        }
        self.apply_cross_run(selection, text, Some(context))
    }

    fn apply_cross_run(
        &self,
        selection: TextSelection,
        text: &str,
        context: Option<CrossRunTypingContext<'_>>,
    ) -> Result<CrossRunPatch<Self>> {
        let prepared = super::cross_run::prepare_cross_run(
            self.xaml(),
            &self.index,
            selection,
            text,
            context,
            Some(TextDecorations::None),
            Some("LeftToRight"),
            0,
        )?;
        Ok(CrossRunPatch {
            source: self.changed(prepared.xml)?,
            selection: prepared.selection,
            insertion_owner: prepared.insertion_owner,
        })
    }

    pub fn replace_in_run(
        &self,
        ordinal: usize,
        start: usize,
        end: usize,
        text: &str,
    ) -> Result<Option<AuthoredTextPatch>> {
        let run = self.index.runs.get(ordinal).ok_or("Unknown authored Run")?;
        let len = run.boundaries.len() - 1;
        if start > end
            || end > len
            || text.len() > MAX_SOURCE_BYTES
            || text
                .chars()
                .any(|c| c < ' ' || matches!(c, '\u{FFFE}' | '\u{FFFF}'))
        {
            return Err("Unsupported authored committed edit".into());
        }
        let (old, _) = decode_body(&self.xaml()[run.opening_end..run.closing_start])?;
        if old.chars().skip(start).take(end - start).eq(text.chars()) {
            return Ok(None);
        }
        let replacement = escaped(text);
        let range = run.opening_end + run.boundaries[start]..run.opening_end + run.boundaries[end];
        let size = self.xaml().len() - range.len() + replacement.len();
        if size > MAX_SOURCE_BYTES {
            return Err("Authored edit budget exceeded".into());
        }
        let mut xml = self.xaml().to_owned();
        xml.replace_range(range, &replacement);
        let caret = run.scalar_start + start + text.chars().count();
        Ok(Some(AuthoredTextPatch {
            source: self.changed(xml)?,
            following_run: None,
            selection: TextSelection {
                paragraph: run.paragraph,
                anchor: caret,
                focus: caret,
            },
        }))
    }
    /// Explicit nonempty range only. Empty caret/springload remains controller
    /// state; this API does not silently claim to format a collapsed selection.
    pub fn set_selection_format(
        &self,
        selection: TextSelection,
        format: &TextFormat,
    ) -> Result<Option<AuthoredTextPatch>> {
        let selected = selection.sorted();
        let paragraph = self
            .index
            .paragraphs
            .get(selection.paragraph)
            .ok_or("Unknown authored Paragraph")?;
        if selected.is_empty() || selected.end > paragraph.chars().count() {
            return Err("Expected nonempty authored selection".into());
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
            .filter(|r| r.paragraph == selection.paragraph)
        {
            let n = run.boundaries.len() - 1;
            let lo = selected.start.saturating_sub(run.scalar_start).min(n);
            let hi = selected.end.saturating_sub(run.scalar_start).min(n);
            if lo >= hi {
                continue;
            }
            let mut changes = Vec::new();
            for (name, value) in &properties {
                let slot = property_slot(name).ok_or("Unknown format")?;
                let current = run.effective[slot]
                    .as_deref()
                    .or((slot == 4).then_some("None"))
                    .ok_or("Missing authored effective property")?;
                let equal = if *name == "FontSize" {
                    canonical_decimal(current)? == canonical_decimal(value)?
                } else if *name == "Foreground" {
                    current.eq_ignore_ascii_case(value)
                } else {
                    current == value
                };
                if !equal {
                    changes.push((*name, value.as_str()));
                }
            }
            if changes.is_empty() {
                continue;
            }
            let open = &self.xaml()[run.range.start..run.opening_end];
            let chosen = patch_opening(open, &changes)?;
            let body = &self.xaml()[run.opening_end..run.closing_start];
            let close = &self.xaml()[run.closing_start..run.range.end];
            let mut value = String::new();
            for (a, b, head) in [(0, lo, open), (lo, hi, chosen.as_str()), (hi, n, open)] {
                if a < b {
                    value.push_str(head);
                    value.push_str(&body[run.boundaries[a]..run.boundaries[b]]);
                    value.push_str(close);
                }
            }
            patches.push((run.range.clone(), value));
        }
        if patches.is_empty() {
            return Ok(None);
        }
        let mut size = self.xaml().len();
        for (range, value) in &patches {
            size = size
                .checked_sub(range.len())
                .and_then(|n| n.checked_add(value.len()))
                .filter(|n| *n <= MAX_SOURCE_BYTES)
                .ok_or("Authored format budget exceeded")?;
        }
        let mut xml = self.xaml().to_owned();
        for (range, value) in patches.into_iter().rev() {
            xml.replace_range(range, &value);
        }
        Ok(Some(AuthoredTextPatch {
            source: self.changed(xml)?,
            selection,
            following_run: None,
        }))
    }
    pub fn split_paragraph_at_caret(&self, selection: TextSelection) -> Result<AuthoredTextPatch> {
        if selection.anchor != selection.focus || self.index.paragraphs.len() >= 1024 {
            return Err("Unsupported authored split caret/count".into());
        }
        let length = self
            .index
            .paragraphs
            .get(selection.paragraph)
            .ok_or("Unknown authored Paragraph")?
            .chars()
            .count();
        if selection.focus > length {
            return Err("Authored caret outside paragraph".into());
        }
        let (ordinal, run) = if selection.focus == 0 {
            self.index
                .runs
                .iter()
                .enumerate()
                .find(|(_, r)| r.paragraph == selection.paragraph)
        } else if selection.focus == length {
            self.index
                .runs
                .iter()
                .enumerate()
                .rfind(|(_, r)| r.paragraph == selection.paragraph)
        } else {
            self.index.runs.iter().enumerate().find(|(_, r)| {
                r.paragraph == selection.paragraph
                    && r.scalar_start < selection.focus
                    && selection.focus < r.scalar_start + r.boundaries.len() - 1
            })
        }
        .ok_or("Ambiguous internal Run-boundary split; owner required")?;
        let cut = run.opening_end + run.boundaries[selection.focus - run.scalar_start];
        let open = &self.xaml()[run.range.start..run.opening_end];
        let add = "</Run>".len() + P_END.len() + P_OPEN.len() + open.len();
        if self
            .xaml()
            .len()
            .checked_add(add)
            .is_none_or(|n| n > MAX_SOURCE_BYTES)
        {
            return Err("Authored split budget exceeded".into());
        }
        let mut xml = String::with_capacity(self.xaml().len() + add);
        xml.push_str(&self.xaml()[..cut]);
        xml.push_str("</Run>");
        xml.push_str(P_END);
        xml.push_str(P_OPEN);
        xml.push_str(open);
        xml.push_str(&self.xaml()[cut..]);
        Ok(AuthoredTextPatch {
            source: self.changed(xml)?,
            following_run: Some(ordinal + 1),
            selection: TextSelection {
                paragraph: selection.paragraph + 1,
                anchor: 0,
                focus: 0,
            },
        })
    }
}

#[cfg(test)]
#[path = "authored_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "authored_underline_tests.rs"]
mod underline_tests;

#[path = "authored/paragraph_alignment.rs"]
mod paragraph_alignment;

#[path = "authored/paragraph_alignment_info.rs"]
mod paragraph_alignment_info;

#[path = "authored/select_all.rs"]
mod select_all;
