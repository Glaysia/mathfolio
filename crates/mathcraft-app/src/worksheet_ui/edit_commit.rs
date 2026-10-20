//! A formula draft changes on key input; evaluation changes on commit.
use super::*;

#[cfg(test)]
#[path = "input_latency.rs"]
mod input_latency;

#[cfg(test)]
#[path = "text_density_tests.rs"]
mod text_density_tests;

#[path = "signature_cache.rs"]
mod signature_cache;

#[path = "lazy_baseline.rs"]
mod lazy_baseline;
use lazy_baseline::{CommittedBaseline, EditorInputs};

#[derive(Default)]
pub(super) struct PreparationCache {
    native_string_settings: Option<([u8; 32], Result<bool, String>)>,
    committed_baseline: Option<CommittedBaseline>,
    signature: signature_cache::SignatureCache,
    input: Option<CalculationInput>,
    editors: HashMap<u64, u64>,
    synchronized: HashMap<u64, u64>,
    validation: HashMap<u64, (u64, Option<String>)>,
    geometry: Option<GeometryPreparation>,
    serialized: HashMap<u64, SerializedMath>,
    labelled_sources: HashMap<u64, String>,
    worksheet: worksheet_math::WorksheetXmlCache,
    #[cfg(test)]
    labelled_decodes: usize,
    #[cfg(test)]
    full_dom_parses: usize,
    #[cfg(test)]
    baseline_materializations: usize,
}

enum SerializedMath {
    Ready(Arc<worksheet_math::TypedMathml>),
    Failed { tree: PortableTree, error: String },
}
impl SerializedMath {
    fn new(tree: &PortableTree, id: u64, traces: &mut worksheet_math::MathmlTraces) -> Self {
        match traces.new_math(id, tree) {
            Ok(proof) => Self::Ready(proof),
            Err(error) => Self::Failed {
                tree: tree.clone(),
                error,
            },
        }
    }
    fn matches(&self, tree: &PortableTree) -> bool {
        match self {
            Self::Ready(proof) => proof.tree() == tree,
            Self::Failed { tree: old, .. } => old == tree,
        }
    }
}
impl PreparationCache {
    #[cfg(test)]
    pub(super) fn full_dom_parse_count(&self) -> usize {
        self.full_dom_parses
    }
    fn strings_use_origin(&mut self, document: &CanvasDocument) -> Result<bool, String> {
        use sha2::{Digest, Sha256};
        let Some(bytes) = &document.native_archive else {
            return Ok(false);
        };
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        if let Some((previous, value)) = &self.native_string_settings
            && *previous == digest
        {
            return value.clone();
        }
        let value = NativePackage::from_bytes(bytes)
            .and_then(|p| mathcraft_document::native_calculation::strings_use_origin(&p));
        self.native_string_settings = Some((digest, value.clone()));
        value
    }
    #[cfg(test)]
    pub(super) fn has_committed_baseline(&self) -> bool {
        self.committed_baseline.is_some()
    }
    #[cfg(test)]
    fn serialize_math(
        &mut self,
        id: u64,
        tree: &PortableTree,
    ) -> Result<Arc<worksheet_math::TypedMathml>, String> {
        self.serialize_math_with_trace(id, tree, &mut worksheet_math::MathmlTraces::new(0))
    }
    fn serialize_math_with_trace(
        &mut self,
        id: u64,
        tree: &PortableTree,
        traces: &mut worksheet_math::MathmlTraces,
    ) -> Result<Arc<worksheet_math::TypedMathml>, String> {
        let entry = match self.serialized.entry(id) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(SerializedMath::new(tree, id, traces))
            }
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                if !entry.get().matches(tree) {
                    entry.insert(SerializedMath::new(tree, id, traces));
                }
                entry.into_mut()
            }
        };
        match entry {
            SerializedMath::Ready(proof) => Ok(proof.clone()),
            SerializedMath::Failed { error, .. } => Err(error.clone()),
        }
    }
}

type MathStatement = (u64, f64, f64, String);

/// Exact statement/context identity; only coordinates are replaced. Retain the
/// baseline XML and byte ranges so rewriting many positions is one linear copy.
struct GeometryPreparation {
    settings: [u64; 3],
    archive: Option<Vec<u8>>,
    statements: Vec<(u64, String)>,
    order: Vec<usize>,
    source: Arc<str>,
    positions: Vec<(u64, std::ops::Range<usize>, std::ops::Range<usize>)>,
}

impl GeometryPreparation {
    fn settings(document: &CanvasDocument) -> [u64; 3] {
        [
            document.width.to_bits(),
            document.height.to_bits(),
            document.grid.to_bits(),
        ]
    }

    fn capture(
        document: &CanvasDocument,
        math: Vec<MathStatement>,
        source: Arc<str>,
        generated: &[worksheet_math::GeneratedRegion],
    ) -> Option<Self> {
        let positions: Vec<_> = generated
            .iter()
            .map(|region| (region.id, region.left.clone(), region.top.clone()))
            .collect();
        if positions.len() != math.len() {
            return None;
        }
        let order = worksheet_math::worksheet_region_order(&math).ok()?;
        // A collected IntoIter may recycle the larger MathStatement allocation.
        // Preserve the old baseline's exact-sized tuple/XML capacities instead.
        let mut statements = Vec::with_capacity(math.len());
        for (id, _, _, mut xml) in math {
            if xml.capacity() != xml.len() {
                xml = xml.into_boxed_str().into_string();
            }
            statements.push((id, xml));
        }
        Some(Self {
            settings: Self::settings(document),
            archive: document.native_archive.clone(),
            statements,
            order,
            source,
            positions,
        })
    }

    fn reposition(
        &self,
        document: &CanvasDocument,
        math: &[MathStatement],
    ) -> Result<Option<Arc<str>>, String> {
        if self.settings != Self::settings(document)
            || self.archive.as_deref() != document.native_archive.as_deref()
            || self.statements.len() != math.len()
            || !self
                .statements
                .iter()
                .zip(math)
                .all(|((old_id, old_xml), (id, _, _, xml))| old_id == id && old_xml == xml)
            || self.order != worksheet_math::worksheet_region_order(math)?
        {
            return Ok(None);
        }
        let coordinates: HashMap<_, _> = math
            .iter()
            .map(|(id, left, top, _)| (*id, (*left, *top)))
            .collect();
        let mut replacements = Vec::new();
        for (id, left_range, top_range) in &self.positions {
            let Some((left, top)) = coordinates.get(id) else {
                return Ok(None);
            };
            for (range, number) in [(left_range, left), (top_range, top)] {
                let number = number.to_string();
                if self.source[range.clone()] != number {
                    replacements.push((range.clone(), number));
                }
            }
        }
        if replacements.is_empty() {
            return Ok(Some(self.source.clone()));
        }
        replacements.sort_by_key(|(range, _)| range.start);
        let mut rewritten = String::with_capacity(self.source.len());
        let mut start = 0;
        for (range, number) in replacements {
            rewritten.push_str(&self.source[start..range.start]);
            rewritten.push_str(&number);
            start = range.end;
        }
        rewritten.push_str(&self.source[start..]);
        if rewritten.len() > 8 * 1024 * 1024 {
            return Err("Worksheet exceeds XML budget".into());
        }
        Ok(Some(rewritten.into()))
    }
}

impl WorksheetApp {
    pub(super) fn retain_committed_input(&mut self) {
        // A potential edit may be a no-op (IME preedit, rejected operator).
        // Recapture while no draft exists instead of trusting such an event's
        // old snapshot after another committed-source change.
        if self.committed_input.is_none() && self.uncommitted_math.is_empty() {
            self.background.preparation.committed_baseline = Some(CommittedBaseline::capture(
                &self.document,
                &self.editors,
                &self.background.preparation.validation,
            ));
        }
    }

    fn materialize_committed_input(&mut self) -> Result<CalculationInput, String> {
        if let Some(input) = &self.committed_input {
            return Ok(input.clone());
        }
        let baseline = self
            .background
            .preparation
            .committed_baseline
            .as_ref()
            .ok_or_else(|| "확정된 계산 입력이 없습니다.".to_string())?;
        // Frozen preparation is isolated: its labels/cache must never replace
        // those of the currently edited document.
        let input = baseline.materialize()?;
        #[cfg(test)]
        {
            self.background.preparation.baseline_materializations += 1;
        }
        self.background.preparation.committed_baseline = None;
        self.committed_input = Some(input.clone());
        Ok(input)
    }

    pub(super) fn stage_math_edit(&mut self, id: u64, evaluate: bool) {
        self.uncommitted_math.insert(id);
        self.pending_regions.insert(id);
        self.calculation_pending = true;
        if evaluate {
            // '=' is an explicit evaluation command, including in Manual mode.
            self.background.preparation.committed_baseline = None;
            self.calculation_changed(true);
        }
    }

    pub(super) fn commit_math_edits(&mut self) {
        if !self.uncommitted_math.is_empty() {
            self.background.preparation.committed_baseline = None;
            self.calculation_changed(false);
        }
    }
}

impl WorksheetApp {
    pub(super) fn prepare_calculation_input(&mut self) -> Result<CalculationInput, String> {
        prepare_input(
            &mut self.background.preparation,
            &self.document,
            EditorInputs::Live(&self.editors),
            &mut self.labelled_trees,
        )
    }

    pub(super) fn refresh_calculation_input(&mut self) -> Result<CalculationInput, String> {
        // Commit valid editor trees; incomplete edits become failed statements in
        // prepare_calculation_input instead of reviving the previous binding.
        for region in &mut self.document.regions {
            if let Some(editor) = self.editors.get(&region.id)
                && self.background.preparation.synchronized.get(&region.id)
                    != Some(&editor.arena().revision())
                && let Ok(tree) = editor.to_portable()
            {
                region.content = CanvasContent::Math(tree);
                self.background
                    .preparation
                    .synchronized
                    .insert(region.id, editor.arena().revision());
            }
        }
        let input = if self.uncommitted_math.is_empty() {
            self.background.preparation.committed_baseline = None;
            let input = self.prepare_calculation_input()?;
            self.committed_input = Some(input.clone());
            input
        } else {
            self.materialize_committed_input()?
        };
        if self.background.enabled() {
            self.prepare_background_state(&input.source);
        } else {
            match input.strings_use_origin {
                Some(value) => {
                    self.calculator.set_strings_use_origin(value);
                }
                None => {
                    self.calculator.set_strings_use_origin_unavailable();
                }
            }
            let update = self.calculator.update(&input.source, None)?;
            self.accept_calculation_update(update);
        }
        let signature = input.signature.clone();
        if signature != self.plot_signature {
            self.mark_regions_pending(
                (self
                    .document
                    .regions
                    .iter()
                    .filter(|region| {
                        !region.calculation_disabled
                            && matches!(region.content, CanvasContent::Plot(_))
                    })
                    .map(|region| region.id))
                .collect::<Vec<_>>(),
            );
            self.plot_signature = signature;
        }
        self.calculation_pending = !self.pending_regions.is_empty();
        Ok(input)
    }
}

// Area permits the explicitly false child disable flag; validate that narrow
// wrapper contract before projecting its one expression through the existing
// structural bridge. This is display data, never a calculation fault/binding.
fn area_labelled_tree(math: roxmltree::Node<'_, '_>) -> Result<PortableTree, String> {
    const WS: &str = "http://schemas.mathsoft.com/worksheet50";
    const ML: &str = "http://schemas.mathsoft.com/math50";
    if math.attributes().any(|a| {
        a.namespace().is_some()
            || (a.name() != "resultRef"
                && !(a.name() == "disable-calc" && matches!(a.value(), "false" | "0")))
    }) {
        return Err("Unsupported Area math presentation metadata".into());
    }
    let children: Vec<_> = math
        .children()
        .filter(roxmltree::Node::is_element)
        .collect();
    if children.iter().any(|n| {
        n.tag_name().namespace() != Some(ML)
            && !n.has_tag_name((WS, "resultFormat"))
            && !n.has_tag_name((WS, "formattingOverride"))
    }) {
        return Err("Unknown Area math presentation sibling".into());
    }
    let mut roots = children
        .into_iter()
        .filter(|n| n.tag_name().namespace() == Some(ML));
    let root = roots.next().ok_or("Missing Area math expression")?;
    if roots.next().is_some() {
        return Err("Multiple Area math expressions".into());
    }
    worksheet_math::tree_from_mathml_node(root)
}

fn prepare_input(
    cache: &mut PreparationCache,
    document: &CanvasDocument,
    editors: EditorInputs<'_>,
    labelled_trees: &mut HashMap<u64, PortableTree>,
) -> Result<CalculationInput, String> {
    #[cfg(test)]
    let mut timing = input_latency::PreparationTiming::start();
    let signature = cache.signature.signature(document);
    let revisions = editors.revisions();
    #[cfg(test)]
    timing.signature();
    if let Some(input) = &cache.input
        && input.signature == signature
        && cache.editors == revisions
    {
        #[cfg(test)]
        timing.finish(true);
        return Ok(input.clone());
    }
    let disabled: HashSet<_> = document
        .regions
        .iter()
        .filter(|r| r.calculation_disabled)
        .map(|r| r.id)
        .collect();
    let active_ids: HashSet<_> = document.regions.iter().map(|r| r.id).collect();
    cache.serialized.retain(|id, _| active_ids.contains(id));
    let mut faults = editors.faults(&mut cache.validation, &disabled);
    #[cfg(test)]
    timing.validation();
    let mut math = Vec::new();
    let mut typed = HashMap::new();
    let mut traces = worksheet_math::MathmlTraces::new(document.regions.len());
    let mut solveblocks = Vec::new();
    let mut areas = Vec::new();
    let mut area_index = super::area_calculation_input::AreaIndex::default();
    for region in &document.regions {
        if region.calculation_disabled {
            continue;
        }
        match &region.content {
            CanvasContent::Math(tree) => {
                if !matches!(
                    tree.kind,
                    NodeKind::Binary(
                        BinaryOp::Define | BinaryOp::GlobalDefine | BinaryOp::Evaluate
                    ) | NodeKind::SymbolicEvaluation { .. }
                ) {
                    continue;
                }
                if faults.contains_key(&region.id) {
                    math.push((region.id, region.left, region.top, fault_statement(tree)));
                    continue;
                }
                match cache.serialize_math_with_trace(region.id, tree, &mut traces) {
                    Ok(proof) => {
                        math.push((
                            region.id,
                            region.left,
                            region.top,
                            proof.source().to_owned(),
                        ));
                        typed.insert(region.id, proof);
                    }
                    Err(e) => {
                        faults.insert(region.id, e);
                        math.push((region.id, region.left, region.top, fault_statement(tree)));
                    }
                }
            }
            CanvasContent::Opaque { xml, kind }
                if matches!(kind.as_str(), "solveblock" | "spec-table" | "specTable") =>
            {
                match native_region_payload(xml, kind) {
                    Ok(payload) => {
                        if kind == "solveblock" {
                            solveblocks.push((region.id, region.left, region.top, payload));
                        } else {
                            math.push((region.id, region.left, region.top, payload));
                        }
                    }
                    Err(error) => {
                        faults.insert(region.id, error);
                        math.push((region.id, region.left, region.top, raw_fault_statement(xml)));
                    }
                }
            }
            CanvasContent::Opaque { xml, kind } if kind == "math" => {
                match native_region_payload(xml, "math") {
                    Ok(mut payload) => {
                        let unknown_metadata = roxmltree::Document::parse(&payload)
                            .ok()
                            .is_some_and(|xml| {
                                let root = xml.root_element();
                                matches!(root.tag_name().name(), "define" | "globalDefine" | "eval")
                                    && root.attributes().len() > 0
                            });
                        if unknown_metadata {
                            faults.insert(region.id, "지원하지 않는 수식 실행 설정".into());
                            payload = raw_fault_statement(&payload);
                        } else if let Err(error) = worksheet_math::worksheet_xml(&[(
                            region.id,
                            region.left,
                            region.top,
                            payload.clone(),
                        )]) {
                            faults.insert(region.id, error);
                            payload = raw_fault_statement(&payload);
                        }
                        math.push((region.id, region.left, region.top, payload));
                    }
                    Err(error) => {
                        faults.insert(region.id, error);
                        math.push((region.id, region.left, region.top, raw_fault_statement(xml)));
                    }
                }
            }
            CanvasContent::Opaque { xml, kind } if kind == "Area" => {
                match super::area_calculation_input::calculation_region(
                    xml,
                    region.id,
                    region.left,
                    region.top,
                ) {
                    Ok((source, index)) => {
                        areas.push(source);
                        area_index.append(index);
                    }
                    Err(error) => {
                        faults.insert(region.id, error);
                        math.push((region.id, region.left, region.top, raw_fault_statement(xml)));
                    }
                }
            }
            CanvasContent::Opaque { .. }
                if mathcraft_document::page_break::is_page_break(region) == Ok(true) => {}
            CanvasContent::Opaque { kind, .. } if matches!(kind.as_str(), "image" | "text") => {}
            CanvasContent::Opaque { .. } => {
                faults.insert(region.id, "이 영역의 계산은 아직 지원하지 않습니다".into());
            }
            CanvasContent::Text(_) | CanvasContent::Plot(_) => (),
        }
    }
    #[cfg(test)]
    timing.serialization();
    let flat = solveblocks.is_empty() && areas.is_empty();
    let repositioned = if flat {
        cache
            .geometry
            .as_ref()
            .map(|cache| cache.reposition(document, &math))
            .transpose()?
            .flatten()
    } else {
        None
    };
    // These IDs outlive the temporary statements. Move their owned XML into
    // the geometry baseline after the builder, instead of cloning every XML.
    let region_ids = math
        .iter()
        .map(|r| r.0)
        .chain(solveblocks.iter().map(|r| r.0))
        .chain(area_index.tails.keys().copied())
        .collect();
    let mut area_labelled: HashMap<u64, HashMap<String, PortableTree>> = HashMap::new();
    let source = if let Some(source) = repositioned {
        #[cfg(test)]
        timing.worksheet();
        #[cfg(test)]
        timing.display();
        source
    } else {
        #[cfg(test)]
        input_latency::capture_worksheet_input(&math);
        let (mut source, generated, decoded_trees) = if !areas.is_empty() {
            // Area declarations participate at their local container slot.
            // Labelling the outer rows first would permanently misclassify a
            // unit-like name before discovering a variable inside the Area.
            let mut nested = areas.clone();
            nested.extend(solveblocks.iter().map(|(id, left, top, xml)| {
                format!("<region region-id=\"{id}\" left=\"{left}\" top=\"{top}\">{xml}</region>")
            }));
            (
                worksheet_math::worksheet_xml_with_areas(&math, &nested)?,
                Vec::new(),
                Vec::new(),
            )
        } else if flat {
            cache
                .worksheet
                .build_prepared_with_traced_trees(&math, &typed, traces)?
                .into_parts_with_trees()
        } else {
            cache
                .worksheet
                .build_prepared(&math)?
                .into_parts_with_trees()
        };
        #[cfg(test)]
        timing.worksheet();
        #[cfg(test)]
        let display_timing =
            input_latency::display_profile::Frame::start(generated.len(), source.len());
        #[cfg(test)]
        display_timing.projections(&decoded_trees);
        if !flat && areas.is_empty() {
            let at = source.rfind("</regions>").ok_or_else(|| {
                "Generated worksheet is missing its regions container".to_string()
            })?;
            let mut nested = String::new();
            for (id, left, top, xml) in &solveblocks {
                nested.push_str(&format!(
                    "<region region-id=\"{id}\" left=\"{left}\" top=\"{top}\">{xml}</region>"
                ));
            }
            for area in &areas {
                nested.push_str(area);
            }
            source.insert_str(at, &nested);
            if source.len() > 8 * 1024 * 1024 {
                return Err("Worksheet exceeds XML budget".into());
            }
        }
        #[cfg(test)]
        let arc_timing =
            input_latency::display_profile::scope(input_latency::display_profile::Phase::Arc);
        let source: Arc<str> = source.into();
        #[cfg(test)]
        drop(arc_timing);
        #[cfg(test)]
        let geometry_timing =
            input_latency::display_profile::scope(input_latency::display_profile::Phase::Geometry);
        cache.geometry = None;
        let mut labelled_ids = HashSet::new();
        if flat {
            cache.geometry =
                GeometryPreparation::capture(document, math, source.clone(), &generated);
        }
        #[cfg(test)]
        drop(geometry_timing);
        if flat && (!cache.labelled_sources.is_empty() || decoded_trees.iter().any(Option::is_some))
        {
            #[cfg(test)]
            let _timing = input_latency::display_profile::scope(
                input_latency::display_profile::Phase::Traversal,
            );
            for (region, decoded_tree) in generated.into_iter().zip(decoded_trees) {
                if let Some(range) = region.math {
                    labelled_ids.insert(region.id);
                    let statement = &source[range];
                    cache.update_labelled_statement(labelled_trees, region.id, statement, || {
                        match decoded_tree {
                            Some(tree) => tree,
                            None => worksheet_math::tree_from_mathml(statement),
                        }
                    });
                }
            }
        } else {
            // A cold display cache decodes all regions; one DOM is less
            // work than parsing thousands of individual fragments.
            // Solveblock insertion also adds nested regions/namespace
            // scopes, so its post-insertion DOM remains authoritative.
            #[cfg(test)]
            {
                cache.full_dom_parses += 1;
            }
            if let Ok(xml) = {
                #[cfg(test)]
                let _timing = input_latency::display_profile::scope(
                    input_latency::display_profile::Phase::Dom,
                );
                roxmltree::Document::parse(&source)
            } {
                #[cfg(test)]
                let _timing = input_latency::display_profile::scope(
                    input_latency::display_profile::Phase::Traversal,
                );
                for region in xml.descendants().filter(|n| {
                    n.has_tag_name(("http://schemas.mathsoft.com/worksheet50", "region"))
                }) {
                    let Some(id) = region.attribute("region-id") else {
                        continue;
                    };
                    let Some(math) = region.children().find(|n| {
                        n.has_tag_name(("http://schemas.mathsoft.com/worksheet50", "math"))
                    }) else {
                        continue;
                    };
                    if let Ok(id) = id.parse::<u64>() {
                        labelled_ids.insert(id);
                        let statement = &source[math.range()];
                        cache.update_labelled_statement(labelled_trees, id, statement, || {
                            worksheet_math::tree_from_mathml_node(math)
                        });
                    } else if let Some(address) = area_index.children.get(id)
                        && let Ok(tree) = area_labelled_tree(math)
                    {
                        // Display-only fallible projection: unsupported source
                        // is not promoted to a calculation fault or binding.
                        area_labelled
                            .entry(address.owner)
                            .or_default()
                            .insert(address.key.clone(), tree);
                    }
                }
            }
        }
        #[cfg(test)]
        let prune_timing =
            input_latency::display_profile::scope(input_latency::display_profile::Phase::Prune);
        labelled_trees.retain(|id, _| labelled_ids.contains(id));
        cache
            .labelled_sources
            .retain(|id, _| labelled_ids.contains(id));
        #[cfg(test)]
        drop(prune_timing);
        #[cfg(test)]
        display_timing.finish(labelled_trees.len(), cache.labelled_sources.len());
        #[cfg(test)]
        timing.display();
        source
    };
    let input = CalculationInput {
        strings_use_origin: cache.strings_use_origin(document).ok(),
        source,
        faults,
        signature,
        region_ids,
        areas: Arc::new(area_index),
        area_labelled: Arc::new(area_labelled),
    };
    cache.input = Some(input.clone());
    cache.editors = revisions;
    #[cfg(test)]
    timing.finish(false);
    Ok(input)
}

impl PreparationCache {
    fn update_labelled_statement(
        &mut self,
        labelled_trees: &mut HashMap<u64, PortableTree>,
        id: u64,
        statement: &str,
        decode: impl FnOnce() -> Result<PortableTree, String>,
    ) {
        #[cfg(test)]
        let check_timing =
            input_latency::display_profile::scope(input_latency::display_profile::Phase::Check);
        if self.labelled_sources.get(&id).map(String::as_str) == Some(statement)
            && labelled_trees.contains_key(&id)
        {
            return;
        }
        #[cfg(test)]
        drop(check_timing);
        #[cfg(test)]
        {
            self.labelled_decodes += 1;
        }
        let decoded = {
            #[cfg(test)]
            let _timing = input_latency::display_profile::scope(
                input_latency::display_profile::Phase::Decode,
            );
            decode()
        };
        match decoded {
            Ok(tree) => {
                #[cfg(test)]
                let _timing = input_latency::display_profile::scope(
                    input_latency::display_profile::Phase::Store,
                );
                labelled_trees.insert(id, tree);
                self.labelled_sources.insert(id, statement.to_owned());
            }
            Err(_) => {
                #[cfg(test)]
                let _timing = input_latency::display_profile::scope(
                    input_latency::display_profile::Phase::Store,
                );
                labelled_trees.remove(&id);
                self.labelled_sources.remove(&id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn consuming_geometry_preserves_exact_owned_payload_without_reusing_statement_slack() {
        let document = CanvasDocument::default();
        let mut math = Vec::with_capacity(64);
        let mut statement = String::with_capacity(1024);
        statement.push_str("<ml:eval><ml:real>1</ml:real><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>");
        math.push((7, -0., 1., statement));
        let (source, generated) = worksheet_math::WorksheetXmlCache::default()
            .build_prepared(&math)
            .unwrap()
            .into_parts();
        let old: Vec<_> = math
            .iter()
            .map(|(id, _, _, xml)| (*id, xml.clone()))
            .collect();
        let geometry =
            GeometryPreparation::capture(&document, math, source.into(), &generated).unwrap();
        assert_eq!(geometry.statements, old);
        assert_eq!(geometry.statements.capacity(), old.capacity());
        assert_eq!(geometry.statements[0].1.capacity(), old[0].1.capacity());
    }
    #[test]
    fn serialized_proof_reuses_exact_tree_and_replaces_changed_or_failed_source() {
        use super::*;
        let mut cache = PreparationCache::default();
        let mut tree = PortableTree {
            kind: NodeKind::Binary(BinaryOp::Define),
            children: vec![
                PortableTree {
                    kind: NodeKind::Identifier("x".into()),
                    children: vec![],
                },
                PortableTree {
                    kind: NodeKind::Number("-0".into()),
                    children: vec![],
                },
            ],
        };
        let original = cache.serialize_math(42, &tree).unwrap();
        let original_source = original.source().to_owned();
        assert!(Arc::ptr_eq(
            &original,
            &cache.serialize_math(42, &tree).unwrap()
        ));
        tree.children[1].kind = NodeKind::Number("0".into());
        let changed = cache.serialize_math(42, &tree).unwrap();
        assert!(!Arc::ptr_eq(&original, &changed));
        assert_ne!(changed.source(), original_source);
        assert_eq!(
            changed.source(),
            worksheet_math::tree_to_mathml(&tree).unwrap()
        );
        tree.children[1].kind = NodeKind::Number("NaN".into());
        let error = cache.serialize_math(42, &tree).err().unwrap();
        assert_eq!(
            Some(error.clone()),
            worksheet_math::tree_to_mathml(&tree).err()
        );
        assert_eq!(cache.serialize_math(42, &tree).err(), Some(error));
        tree.children[1].kind = NodeKind::Number("-0".into());
        assert_eq!(
            cache.serialize_math(42, &tree).unwrap().source(),
            original_source
        );
        assert_eq!(
            original.source(),
            original_source,
            "old proofs remain immutable"
        );
    }
    use super::*;
    #[test]
    fn prepared_input_shares_xml_but_invalid_editor_and_undo_invalidate_it() {
        let mut app = large(3, 0);
        let first = app.prepare_calculation_input().unwrap();
        let warm = app.prepare_calculation_input().unwrap();
        assert!(Arc::ptr_eq(&first.source, &warm.source));
        let id = app.document.regions[0].id;
        let editor = app.editors.get_mut(&id).unwrap();
        let operand = editor.arena().get(editor.root()).unwrap().children()[1];
        editor
            .set_atom(operand, NodeKind::Number("NaN".into()))
            .unwrap();
        let invalid = app.prepare_calculation_input().unwrap();
        assert!(invalid.faults.contains_key(&id));
        assert_ne!(first.source, invalid.source);
        assert!(app.editors.get_mut(&id).unwrap().undo());
        let restored = app.prepare_calculation_input().unwrap();
        assert!(restored.faults.is_empty());
        assert_eq!(restored.source, first.source);
    }
    #[test]
    fn prepared_input_invalidates_for_disabled_content_geometry_and_native_archive() {
        let mut app = large(2, 0);
        let first = app.prepare_calculation_input().unwrap();
        app.document.regions[0].calculation_disabled = true;
        let disabled = app.prepare_calculation_input().unwrap();
        assert_ne!(first.source, disabled.source);
        assert_eq!(disabled.region_ids.len(), 1);
        app.document.regions[0].calculation_disabled = false;
        app.document.regions[0].top = 777.;
        let moved = app.prepare_calculation_input().unwrap();
        assert_ne!(first.source, moved.source);
        app.document.native_archive = Some(vec![1, 2, 3]);
        let archive = app.prepare_calculation_input().unwrap();
        assert_ne!(moved.signature, archive.signature);
        app.document.native_archive.as_mut().unwrap()[0] = 9;
        let changed = app.prepare_calculation_input().unwrap();
        assert_ne!(archive.signature, changed.signature);
        assert!(!Arc::ptr_eq(&archive.source, &changed.source));
    }
    #[test]
    fn warm_preparation_keeps_geometric_identifier_labels_after_reorder() {
        let mut app = large(1, 0);
        let define = app.document.regions[0].id;
        let mut editor = Editor::new();
        formula::input_text(&mut editor, "m:=3");
        app.document.regions[0].content = CanvasContent::Math(editor.to_portable().unwrap());
        app.editors.insert(define, editor);
        let eval = app.document.add_math(0., 100.);
        let mut editor = Editor::new();
        formula::input_text(&mut editor, "m=");
        app.document.regions[1].content = CanvasContent::Math(editor.to_portable().unwrap());
        app.editors.insert(eval, editor);
        let first = app.prepare_calculation_input().unwrap();
        let first_labels = app.labelled_trees.clone();
        let warm = app.prepare_calculation_input().unwrap();
        assert!(Arc::ptr_eq(&first.source, &warm.source));
        assert_eq!(first_labels, app.labelled_trees);
        app.document.regions[0].top = 200.;
        let moved = app.prepare_calculation_input().unwrap();
        assert_ne!(moved.source, first.source);
        assert_ne!(first_labels[&eval], app.labelled_trees[&eval]);
    }
    fn settle(app: &mut WorksheetApp) {
        let end = std::time::Instant::now() + std::time::Duration::from_secs(60);
        while app.background.running {
            app.poll_background_calculation();
            assert!(std::time::Instant::now() < end, "worker deadline");
            std::thread::yield_now();
        }
    }
    fn input_frame(app: &mut WorksheetApp, ctx: &egui::Context, events: Vec<egui::Event>) {
        super::super::tests::frame(app, ctx, events);
    }
    #[test]
    #[ignore = "CPU-only draft, commit, and explicit worker latency profile"]
    fn benchmark_draft_commit_and_worker_latency() {
        fn run(mut app: WorksheetApp, name: &str) {
            let ctx = egui::Context::default();
            crate::configure(&ctx);
            app.enable_background_calculation(ctx.clone());
            // Cold/open state, before any formula editor receives key input.
            let started = std::time::Instant::now();
            app.retain_committed_input();
            let retain = started.elapsed().as_micros();
            let id=app.document.regions.iter().find(|r|matches!(&r.content,CanvasContent::Math(t) if matches!(t.kind,NodeKind::Binary(BinaryOp::Define)))).unwrap().id;
            let tree = match &app
                .document
                .regions
                .iter()
                .find(|r| r.id == id)
                .unwrap()
                .content
            {
                CanvasContent::Math(t) => t.clone(),
                _ => unreachable!(),
            };
            app.editors
                .entry(id)
                .or_insert_with(|| Editor::from_portable(&tree).unwrap());
            app.selected = Some(id);
            app.focus_new = Some(id);
            input_frame(&mut app, &ctx, vec![]);
            let editor = app.editors.get_mut(&id).unwrap();
            let rhs = editor.arena().get(editor.root()).unwrap().children()[1];
            // Matrix workloads still measure a real key mutation: select one cell.
            let atom = editor
                .arena()
                .nodes()
                .find_map(|(id, n)| matches!(n.kind(), NodeKind::Number(_)).then_some(id))
                .unwrap_or(rhs);
            let caret = if let NodeKind::Number(s) = &editor.arena().get(atom).unwrap().kind() {
                s.len()
            } else {
                0
            };
            editor.place_text_caret(atom, caret).unwrap();
            let started = std::time::Instant::now();
            input_frame(&mut app, &ctx, vec![egui::Event::Text("1".into())]);
            let key = started.elapsed().as_micros();
            assert!(app.uncommitted_math.contains(&id));
            assert!(app.last_executed_regions.is_empty());
            let started = std::time::Instant::now();
            app.commit_math_edits();
            let commit = started.elapsed().as_micros();
            assert!(app.background.running);
            let started = std::time::Instant::now();
            settle(&mut app);
            let completion = started.elapsed().as_micros();
            let started = std::time::Instant::now();
            app.calculate_command(CalculationCommand::Worksheet);
            let recalc_submit = started.elapsed().as_micros();
            let started = std::time::Instant::now();
            settle(&mut app);
            let recalc_completion = started.elapsed().as_micros();
            let point = pos2(650., 550.);
            for pressed in [true, false] {
                let started = std::time::Instant::now();
                input_frame(
                    &mut app,
                    &ctx,
                    vec![
                        egui::Event::PointerMoved(point),
                        egui::Event::PointerButton {
                            pos: point,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                );
                println!(
                    "INPUT_POINTER name={name} pressed={pressed} elapsed_us={}",
                    started.elapsed().as_micros()
                );
            }
            println!(
                "INPUT_EVENTS name={name} regions={} cold_retain_us={retain} key_frame_us={key} commit_submit_us={commit} worker_remaining_us={completion} full_recalc_submit_us={recalc_submit} full_recalc_remaining_us={recalc_completion} rows={}",
                app.document.regions.len(),
                app.results.len()
            );
            assert!(
                app.calculation_error.is_none(),
                "{:?}",
                app.calculation_error
            );
        }
        run(large(4000, 0), "4k-scalars");
        run(large(100, 16), "100-matrices16");
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../artifacts/rea/unit-selected-sources/1015_LCC_S.mcdx");
        if fixture.is_file() {
            let mut app = WorksheetApp::default();
            app.open(&fixture);
            run(app, "1015_LCC_S");
        }
    }
    fn tree(kind: NodeKind, children: Vec<PortableTree>) -> PortableTree {
        PortableTree { kind, children }
    }
    fn large(count: usize, matrix: usize) -> WorksheetApp {
        let mut app = WorksheetApp::default();
        for n in 0..count {
            let id = app.document.add_math(0., n as f64 * 35.);
            let value = if matrix == 0 {
                tree(NodeKind::Number(n.to_string()), vec![])
            } else {
                tree(
                    NodeKind::Matrix {
                        rows: matrix,
                        cols: matrix,
                        display: None,
                    },
                    (0..matrix * matrix)
                        .map(|i| tree(NodeKind::Number(i.to_string()), vec![]))
                        .collect(),
                )
            };
            let source = tree(
                NodeKind::Binary(BinaryOp::Define),
                vec![tree(NodeKind::Identifier(format!("v{n}")), vec![]), value],
            );
            app.editors
                .insert(id, Editor::from_portable(&source).unwrap());
            app.document.regions.last_mut().unwrap().content = CanvasContent::Math(source);
        }
        app
    }
    #[test]
    #[ignore = "CPU input preparation component benchmark"]
    fn benchmark_input_preparation_latency() {
        for (count, matrix) in [(1000, 0), (4000, 0), (100, 16)] {
            let mut app = large(count, matrix);
            let started = std::time::Instant::now();
            let input = app.prepare_calculation_input().unwrap();
            let prep = started.elapsed().as_micros();
            let started = std::time::Instant::now();
            for _ in 0..3 {
                std::hint::black_box(app.document.active_calculation_signature());
            }
            let signature = started.elapsed().as_micros();
            let started = std::time::Instant::now();
            app.retain_committed_input();
            let retain = started.elapsed().as_micros();
            let started = std::time::Instant::now();
            app.sync_editors().unwrap();
            let sync = started.elapsed().as_micros();
            println!(
                "INPUT_LATENCY regions={count} matrix={matrix} xml_bytes={} prepare_us={prep} signatures3_us={signature} cold_retain_us={retain} sync_all_us={sync}",
                input.source.len()
            );
            assert!(app.last_executed_regions.is_empty());
        }
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../artifacts/rea/unit-selected-sources/1015_LCC_S.mcdx");
        if root.exists() {
            let mut app = WorksheetApp::default();
            app.open(&root);
            let started = std::time::Instant::now();
            let input = app.prepare_calculation_input().unwrap();
            let prep = started.elapsed().as_micros();
            let started = std::time::Instant::now();
            for _ in 0..3 {
                std::hint::black_box(app.document.active_calculation_signature());
            }
            println!(
                "INPUT_NATIVE regions={} archive_bytes={} xml_bytes={} prepare_us={prep} signatures3_us={}",
                app.document.regions.len(),
                app.document.native_archive.as_ref().map_or(0, Vec::len),
                input.source.len(),
                started.elapsed().as_micros()
            );
        }
    }
}
