//! Restore positively decoded XY curve data without evaluating axis expressions.
use super::*;
use mathfolio_document::native::{NativeCache, NativeRegion, parse_result_id};

const RESULT_NS: &str = "http://schemas.mathsoft.com/result10";
const MATH_NS: &str = "http://schemas.mathsoft.com/math50";
const MAX_PART_BYTES: usize = 8 * 1024 * 1024;
const MAX_PART_NODES: u32 = 250_000;
#[cfg(test)]
thread_local! {
    static PART_VALIDATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

// Region identities are normally numeric, but lossless import also retains
// unknown lexical identities. Canonicalize decimal aliases without truncating
// large original IDs to the result-reference i32 domain.
fn owner_key(source: &str) -> String {
    let source = source.trim_matches([' ', '\t', '\r', '\n']);
    let (negative, digits) = if let Some(digits) = source.strip_prefix('-') {
        (true, digits)
    } else {
        (false, source.strip_prefix('+').unwrap_or(source))
    };
    if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) {
        let digits = digits.trim_start_matches('0');
        if digits.is_empty() {
            "0".into()
        } else if negative {
            format!("-{digits}")
        } else {
            digits.into()
        }
    } else {
        source.into()
    }
}

fn structural_text(node: roxmltree::Node<'_, '_>) -> bool {
    node.children().any(|n| {
        n.is_comment()
            || n.is_pi()
            || (n.is_text()
                && !n
                    .text()
                    .unwrap_or("")
                    .trim_matches([' ', '\t', '\r', '\n'])
                    .is_empty())
    })
}

fn decode_curve(
    node: roxmltree::Node<'_, '_>,
) -> Result<Option<mathfolio_document::native_plot_units::NativePlotUnitResult>, String> {
    if node.children().any(|axis| {
        (axis.has_tag_name((MATH_NS, "RangeInfo")) || axis.has_tag_name((MATH_NS, "ResultInfo")))
            && axis.children().any(|n| n.is_element())
    }) {
        mathfolio_document::native_plot_units::read_result_with_units(node)
    } else {
        // Preserve the established no-unit cold path without copying/reparsing
        // an otherwise unchanged large vector payload.
        Ok(
            mathfolio_document::native_plot_result::read_result(node)?.map(|curve| {
                mathfolio_document::native_plot_units::NativePlotUnitResult {
                    curve,
                    range_units: None,
                    result_units: None,
                }
            }),
        )
    }
}

fn validate_result_data(node: roxmltree::Node<'_, '_>) -> Result<bool, String> {
    if node.attributes().any(|a| {
        a.namespace().is_some() || !["result-id", "calculation-status", "tag"].contains(&a.name())
    }) {
        return Err("Unsupported native XY resultData attributes".into());
    }
    parse_result_id(node.attribute("result-id").unwrap_or(""))?;
    if node
        .attribute("calculation-status")
        .is_some_and(|s| !matches!(s, "Synchronized" | "Pending"))
    {
        return Err("Unsupported native XY calculation status".into());
    }
    if structural_text(node) {
        return Err("Unsupported native XY resultData content".into());
    }
    let mut children = node.children().filter(|n| n.is_element());
    let Some(payload) = children.next() else {
        return if node.attribute("calculation-status") == Some("Pending") {
            Ok(false)
        } else {
            Err("Native XY cache has no curve payload".into())
        };
    };
    if !payload.has_tag_name((MATH_NS, "Trace2dResult")) || children.next().is_some() {
        return Err("Unsupported or duplicate native XY resultData payload".into());
    }
    // The finite codec rejects unsupported structural data. Comments and PIs
    // need separate rejection because they are not numerical presentation.
    if payload.descendants().any(|n| n.is_comment() || n.is_pi()) {
        return Err("Unsupported native XY curve annotations".into());
    }
    Ok(true)
}

// This runs once per referenced part in its actual inherited namespace context.
// No DOM or duplicate point vectors survive construction of the index.
fn validate_part(
    source: &str,
    wanted: &HashSet<i32>,
) -> Result<HashMap<i32, Result<bool, String>>, String> {
    #[cfg(test)]
    PART_VALIDATIONS.with(|count| count.set(count.get() + 1));
    if source.len() > MAX_PART_BYTES {
        return Err("Native XY result part exceeds XML byte limit".into());
    }
    let dom = roxmltree::Document::parse_with_options(
        source,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: MAX_PART_NODES,
            ..Default::default()
        },
    )
    .map_err(|e| format!("Invalid native XY result part: {e}"))?;
    let root = dom.root_element();
    if !root.has_tag_name((RESULT_NS, "resultsList"))
        || root.attributes().len() != 0
        || structural_text(root)
    {
        return Err("Unsupported native XY resultsList structure".into());
    }
    let mut result = HashMap::new();
    for node in root.children().filter(|n| n.is_element()) {
        if !node.has_tag_name((RESULT_NS, "resultData")) {
            return Err("Unsupported native XY resultsList sibling".into());
        }
        let id = parse_result_id(node.attribute("result-id").unwrap_or(""))?;
        if wanted.contains(&id) {
            let validation = validate_result_data(node);
            if result.insert(id, validation).is_some() {
                result.insert(id, Err("Ambiguous native XY result identity".into()));
            }
        }
    }
    Ok(result)
}

pub(super) struct CacheIndex<'a> {
    rows: HashMap<i32, Option<&'a NativeCache>>,
    references: HashMap<i32, usize>,
    owners: HashMap<String, usize>,
    validation: HashMap<i32, Result<bool, String>>,
}
impl<'a> CacheIndex<'a> {
    pub(super) fn new(
        package: &NativePackage,
        caches: &'a [NativeCache],
        regions: &[NativeRegion],
    ) -> Result<Self, String> {
        let references = package.result_reference_counts()?;
        let mut rows = HashMap::new();
        let mut parts: HashMap<&str, HashSet<i32>> = HashMap::new();
        for row in caches {
            if let Some(id) = row
                .result_id
                .as_deref()
                .and_then(|id| parse_result_id(id).ok())
            {
                if let std::collections::hash_map::Entry::Vacant(entry) = rows.entry(id) {
                    entry.insert(Some(row));
                } else {
                    rows.insert(id, None);
                }
                // Other concrete result kinds cannot be finite curve payloads.
                // Keep their binding so selecting one reports unsupported, but
                // avoid retaining a structural verdict for every scalar result.
                if references.contains_key(&id)
                    && matches!(row.result_kind.as_deref(), None | Some("Trace2dResult"))
                {
                    parts.entry(&row.part).or_default().insert(id);
                }
            }
        }
        let mut owners = HashMap::new();
        for region in regions
            .iter()
            .filter(|r| r.part == package.worksheet_part_name())
        {
            if let Some(id) = &region.region_id {
                *owners.entry(owner_key(id)).or_insert(0) += 1;
            }
        }
        let mut validation = HashMap::new();
        for (part, wanted) in parts {
            let checked = package
                .original_xml_part(part)
                .and_then(|xml| xml.ok_or_else(|| "Missing native XY result part".into()))
                .and_then(|xml| validate_part(&xml, &wanted));
            match checked {
                Ok(mut checked) => {
                    for id in wanted {
                        validation.insert(
                            id,
                            checked.remove(&id).unwrap_or_else(|| {
                                Err("Native XY cache is absent from its original part".into())
                            }),
                        );
                    }
                }
                Err(error) => {
                    for id in wanted {
                        validation.insert(id, Err(error.clone()));
                    }
                }
            }
        }
        Ok(Self {
            rows,
            references,
            owners,
            validation,
        })
    }
    pub(super) fn saved_row(
        &self,
        region: &CanvasRegion,
        source: &str,
    ) -> Result<Option<SavedCalculationRow>, String> {
        self.saved_row_with_policy(region, source, false)
    }
    /// A legitimate own-format edit can be newer than its retained archive.
    /// That archive has no presentation for the edited source; it is not a
    /// malformed native cache. Strict inspection retains its diagnostic API.
    pub(super) fn restore_row(
        &self,
        region: &CanvasRegion,
        source: &str,
    ) -> Result<Option<SavedCalculationRow>, String> {
        self.saved_row_with_policy(region, source, true)
    }
    fn saved_row_with_policy(
        &self,
        region: &CanvasRegion,
        source: &str,
        source_changed_is_absence: bool,
    ) -> Result<Option<SavedCalculationRow>, String> {
        let CanvasContent::Plot(spec) = &region.content else {
            return Ok(None);
        };
        let Some(owner) = &region.source_region else {
            return Ok(None);
        };
        if self.owners.get(&owner_key(owner)) != Some(&1) {
            return Err("Ambiguous or missing native XY owning region identity".into());
        }
        let original = mathfolio_document::native_plot::parse_with_references(source)?;
        if !mathfolio_document::native_plot::matches_retained_source(spec, &original.spec)? {
            if source_changed_is_absence {
                return Ok(None);
            }
            return Err("Native XY cached source differs from the current plot".into());
        }
        let refs = original.trace_result_refs;
        if refs.len() != spec.trace_count() {
            return Err("Native XY trace/cache count differs".into());
        }
        let mut traces = Vec::with_capacity(refs.len());
        let mut synchronized = true;
        let mut original_mathml = None;
        let mut missing = false;
        for (reference, trace) in refs.into_iter().zip(spec.paired_sources()) {
            let Some(id) = reference else {
                missing = true;
                synchronized = false;
                traces.push(crate::plots::TraceOutcome::Pending { previous: None });
                continue;
            };
            if self.references.get(&id) != Some(&1) {
                return Err("Ambiguous native XY result reference".into());
            }
            let Some(cache) = self.rows.get(&id) else {
                missing = true;
                synchronized = false;
                traces.push(crate::plots::TraceOutcome::Pending { previous: None });
                continue;
            };
            let cache = cache.ok_or("Ambiguous native XY result identity")?;
            if cache
                .result_kind
                .as_deref()
                .is_some_and(|kind| kind != "Trace2dResult")
            {
                return Err("Unsupported native XY cached result kind".into());
            }
            let present = self
                .validation
                .get(&id)
                .ok_or("Unvalidated native XY result part")?
                .as_ref()
                .map_err(Clone::clone)?;
            if !present {
                missing = true;
                synchronized = false;
                traces.push(crate::plots::TraceOutcome::Pending { previous: None });
                continue;
            }
            if !cache.errors.is_empty() || !cache.warnings.is_empty() {
                return Err("Native XY cache contains unsupported diagnostics".into());
            }
            let raw = cache
                .result_mathml
                .as_deref()
                .ok_or("Native XY cache has no curve payload")?;
            let dom = roxmltree::Document::parse_with_options(
                raw,
                roxmltree::ParsingOptions {
                    allow_dtd: false,
                    nodes_limit: 384,
                    ..Default::default()
                },
            )
            .map_err(|e| format!("Invalid native XY curve: {e}"))?;
            let data = decode_curve(dom.root_element())?
                .ok_or("Unsupported native XY cached curve data")?;
            if data.curve.points.is_empty() || data.curve.points.len() > 2000 {
                return Err("Native XY cached curve exceeds supported point limits".into());
            }
            synchronized &= cache.calculation_status.as_deref() == Some("Synchronized");
            original_mathml.get_or_insert_with(|| raw.to_owned());
            let axis = |source_scaling: Option<mathfolio_document::canvas::PlotScaling>,
                        residual_units: Option<
                mathfolio_document::native_plot_units::NativeUnitMonomial,
            >| {
                if source_scaling.is_none() && residual_units.is_none() {
                    None
                } else {
                    Some(Box::new(
                        mathfolio_document::plot_axis_presentation::PlotAxisPresentation::Native {
                            source_scaling,
                            residual_units,
                        },
                    ))
                }
            };
            let curve = crate::plots::PlotTraceData {
                points: data.curve.points.into(),
                implicit_domain: false,
                label: trace.label.to_owned(),
                style: *trace.style,
                presentation: crate::plots::TracePresentation::pair(
                    axis(trace.x.scaling.cloned(), data.range_units),
                    axis(trace.y.scaling.cloned(), data.result_units),
                ),
            };
            traces.push(
                if cache.calculation_status.as_deref() == Some("Synchronized") {
                    crate::plots::TraceOutcome::Ready(curve)
                } else {
                    crate::plots::TraceOutcome::Pending {
                        previous: Some(curve),
                    }
                },
            );
        }
        if missing && traces.iter().all(|trace| trace.curve().is_none()) {
            return Ok(None);
        }
        let data = crate::plots::PlotData { traces };
        let display = data.to_saved();
        if display.is_empty() {
            return Err("Native XY cached presentation exceeds saved-data limits".into());
        }
        Ok(Some(SavedCalculationRow {
            region_id: region.id,
            status: if synchronized {
                SavedCalculationStatus::Synchronized
            } else {
                SavedCalculationStatus::Pending
            },
            display: Some(display),
            error: None,
            warnings: vec![],
            symbolic_mathml: None,
            // An inert native presentation; real successful calculation clears it.
            numeric_source: original_mathml.map(SavedNumericSource::NativeMathml),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};

    const WS: &str = "http://schemas.mathsoft.com/worksheet50";
    fn source(reference: Option<&str>) -> String {
        let mut payload = mathfolio_document::native_plot::payload(&PlotSpec::default()).unwrap();
        if let Some(reference) = reference {
            payload = payload.replace("<trace>", &format!("<trace resultRef=\"{reference}\">"));
        }
        format!("<region xmlns=\"{WS}\" region-id=\"7\" left=\"10\" top=\"20\">{payload}</region>")
    }
    fn curve() -> String {
        mathfolio_document::native_plot_result::finite_unitless_xml(&[[0., 1.], [2., 3.]])
            .unwrap()
            .unwrap()
    }
    fn row(attributes: &str, payload: &str) -> String {
        format!("<resultData result-id=\"9\" {attributes}>{payload}</resultData>")
    }
    fn package(source: &str, extra: &str, rows: &str) -> NativePackage {
        let worksheet =
            format!("<worksheet xmlns=\"{WS}\"><regions>{source}{extra}</regions></worksheet>");
        let results = format!(
            "<resultsList xmlns=\"{RESULT_NS}\" xmlns:m=\"{MATH_NS}\">{rows}</resultsList>"
        );
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, xml) in [
            ("mathcad/worksheet.xml", worksheet),
            ("mathcad/result.xml", results),
        ] {
            zip.start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(xml.as_bytes()).unwrap();
        }
        NativePackage::from_bytes(&zip.finish().unwrap().into_inner()).unwrap()
    }
    fn region(source: &str) -> CanvasRegion {
        CanvasRegion {
            id: 42,
            left: 10.,
            top: 20.,
            source_region: Some("7".into()),
            rich_text: None,
            authored_text: None,
            content: CanvasContent::Plot(mathfolio_document::native_plot::parse(source).unwrap()),
            format: Default::default(),
            calculation_disabled: false,
            calculation_enabled_override: false,
        }
    }
    fn restore(
        source: &str,
        extra: &str,
        rows: &str,
    ) -> Result<Option<SavedCalculationRow>, String> {
        let package = package(source, extra, rows);
        let caches = package.caches()?;
        let regions = package.regions()?;
        CacheIndex::new(&package, &caches, &regions)?.saved_row(&region(source), source)
    }

    #[test]
    fn unit_cache_restore_is_inert_preserves_residuals_and_scaling_independently() {
        use mathfolio_document::{
            canvas::{PlotAxis, PlotScaling},
            native_plot_units::{NativeUnitMonomial, NativeUnitReference},
            plot_axis_presentation::PlotAxisPresentation,
            plot_snapshot::{SavedPlotSnapshot, SavedPlotTrace},
        };
        let mut spec = PlotSpec::default();
        let scaling = PlotScaling::from_tree(mathfolio_editor::PortableTree {
            kind: mathfolio_editor::NodeKind::AnnotatedIdentifier {
                name: "in".into(),
                labels: "UNIT".into(),
            },
            children: vec![],
        })
        .unwrap()
        .unwrap();
        spec.set_axis_scaling(PlotAxis::Y, 0, Some(scaling.clone()))
            .unwrap();
        let payload = mathfolio_document::native_plot::payload(&spec)
            .unwrap()
            .replace("<trace>", "<trace resultRef=\"9\">");
        let source = format!(
            "<region xmlns=\"{WS}\" region-id=\"7\" left=\"10\" top=\"20\">{payload}</region>"
        );
        let units = NativeUnitMonomial {
            system: Some("original system".into()),
            references: vec![NativeUnitReference {
                unit: "unknown-residual𝛼".into(),
                power_numerator: Some(-2),
                power_denominator: None,
            }],
        };
        let curve = mathfolio_document::native_plot_units::finite_with_units_xml(
            &[[0., 1.], [2., 3.]],
            &Some(units.clone()),
            &None,
        )
        .unwrap()
        .unwrap();
        for (status, expected) in [
            ("Synchronized", SavedCalculationStatus::Synchronized),
            ("Pending", SavedCalculationStatus::Pending),
        ] {
            let restored = restore(
                &source,
                "",
                &row(&format!("calculation-status=\"{status}\""), &curve),
            )
            .unwrap()
            .unwrap();
            assert_eq!(restored.status, expected);
            let snapshot =
                SavedPlotSnapshot::parse_for_spec(restored.display.as_deref().unwrap(), &spec)
                    .unwrap();
            assert_eq!(snapshot.version, 3);
            let data = match &snapshot.traces[0] {
                SavedPlotTrace::Ready { data }
                | SavedPlotTrace::Pending {
                    previous: Some(data),
                } => data,
                _ => panic!(),
            };
            assert_eq!(data.points, [[0., 1.], [2., 3.]]);
            assert_eq!(
                data.x_presentation.as_deref(),
                Some(&PlotAxisPresentation::Native {
                    source_scaling: None,
                    residual_units: Some(units.clone())
                })
            );
            assert_eq!(
                data.y_presentation.as_deref(),
                Some(&PlotAxisPresentation::Native {
                    source_scaling: Some(scaling.clone()),
                    residual_units: None
                })
            );
            let Some(SavedNumericSource::NativeMathml(raw)) = restored.numeric_source else {
                panic!()
            };
            assert!(raw.contains("unknown-residual𝛼"));
        }
    }

    #[test]
    fn inherited_namespaces_xml_int_aliases_pending_and_native_marker_are_preserved() {
        let source = source(Some(" +0009 "));
        // Payload prefixes are bound on the actual part, not on resultData.
        let curve = curve()
            .replace(&format!(" xmlns:ml=\"{MATH_NS}\""), "")
            .replace("ml:", "m:");
        for (status, expected) in [
            ("Pending", SavedCalculationStatus::Pending),
            ("Synchronized", SavedCalculationStatus::Synchronized),
        ] {
            let rows = row(
                &format!("calculation-status=\"{status}\" tag=\"kept-native\""),
                &curve,
            )
            .replace("result-id=\"9\"", "result-id=\" +0009 \t\"");
            let restored = restore(&source, "", &rows).unwrap().unwrap();
            assert_eq!(restored.region_id, 42);
            assert_eq!(restored.status, expected);
            let Some(SavedNumericSource::NativeMathml(raw)) = restored.numeric_source else {
                panic!()
            };
            assert!(raw.contains("Trace2dResult"));
            let data =
                crate::plots::PlotData::from_saved(restored.display.as_deref().unwrap()).unwrap();
            assert_eq!(data.first_curve().unwrap().points, vec![[0., 1.], [2., 3.]]);
        }
    }

    #[test]
    fn missing_refs_and_missing_rows_are_absence_but_existing_bad_data_is_an_error() {
        assert!(restore(&source(None), "", "").unwrap().is_none());
        assert!(restore(&source(Some("9")), "", "").unwrap().is_none());
        let pending = row("calculation-status=\"Pending\"", "");
        assert!(restore(&source(Some("9")), "", &pending).unwrap().is_none());
        let empty = row("", "");
        assert!(restore(&source(Some("9")), "", &empty).is_err());
        let unsupported = row(
            "",
            &curve().replace("TraceType=\"Vector\"", "TraceType=\"Parametric\""),
        );
        assert!(restore(&source(Some("9")), "", &unsupported).is_err());
    }

    #[test]
    fn duplicate_owner_ids_including_decimal_aliases_and_nested_children_are_rejected() {
        let source = source(Some("9"));
        let rows = row("", &curve());
        for alias in ["7", "+0007", " 007 "] {
            let extra = format!("<region region-id=\"{alias}\"><math/></region>");
            assert!(
                restore(&source, &extra, &rows)
                    .unwrap_err()
                    .contains("owning region")
            );
            let nested = format!(
                "<region region-id=\"99\"><Area><regions>{extra}</regions></Area></region>"
            );
            assert!(
                restore(&source, &nested, &rows)
                    .unwrap_err()
                    .contains("owning region")
            );
        }
        assert_eq!(owner_key("-000"), owner_key("+0"));
        assert_eq!(
            owner_key("184467440737095516160"),
            owner_key("+0184467440737095516160")
        );
        assert_ne!(owner_key("id7"), owner_key("7"));
    }

    #[test]
    fn duplicate_result_id_and_reference_bindings_are_rejected() {
        let source = source(Some("9"));
        let first = row("", &curve());
        let alias = first.replace("result-id=\"9\"", "result-id=\"+009\"");
        assert!(
            restore(&source, "", &format!("{first}{alias}"))
                .unwrap_err()
                .contains("identity")
        );
        let second_source = source.replace("region-id=\"7\"", "region-id=\"8\"");
        assert!(
            restore(&source, &second_source, &first)
                .unwrap_err()
                .contains("reference")
        );
    }

    #[test]
    fn unknown_attributes_siblings_duplicate_payloads_and_diagnostics_are_visible_errors() {
        let source = source(Some("9"));
        let curve = curve();
        for rows in [
            row("future=\"1\"", &curve),
            row("xmlns:q=\"urn:future\" q:tag=\"1\"", &curve),
            row("calculation-status=\"Future\"", &curve),
            row("", &format!("{curve}{curve}")),
            row("", &format!("{curve}<future/>")),
            row("", &format!("{curve}<engineErrors/>")),
            row("", &format!("{curve}<Warnings/>")),
            row("", &format!("<!-- hidden -->{curve}")),
            row("", &format!("{curve}discarded")),
            row("", &format!("{curve}\u{a0}")),
            format!("{}<future/>", row("", &curve)),
            row("", &curve.replace("<ml:Data>", "<ml:Data><!-- hidden -->")),
        ] {
            assert!(restore(&source, "", &rows).is_err(), "accepted {rows}");
        }
    }

    #[test]
    fn actual_part_namespaces_and_curve_limits_cannot_be_hidden_by_first_payload() {
        let source = source(Some("9"));
        let wrong_ns = curve().replace(MATH_NS, "urn:not-math");
        assert!(restore(&source, "", &row("", &wrong_ns)).is_err());
        let many = vec![[0., 1.]; 2001];
        let many = mathfolio_document::native_plot_result::finite_unitless_xml(&many)
            .unwrap()
            .unwrap();
        assert!(
            restore(&source, "", &row("", &many))
                .unwrap_err()
                .contains("point limit")
        );
        let mut region = region(&source);
        let CanvasContent::Plot(spec) = &mut region.content else {
            panic!()
        };
        spec.set_axis_expression_text(mathfolio_document::canvas::PlotAxis::Y, 0, "x+1")
            .unwrap();
        let package = package(&source, "", &row("", &curve()));
        let caches = package.caches().unwrap();
        assert!(
            CacheIndex::new(&package, &caches, &package.regions().unwrap())
                .unwrap()
                .saved_row(&region, &source)
                .unwrap_err()
                .contains("source differs")
        );
    }

    #[test]
    fn strict_validation_is_once_per_referenced_part_and_bounded_without_point_retention() {
        let source = source(Some("9"));
        let package = package(&source, "", &row("", &curve()));
        let caches = package.caches().unwrap();
        let before = PART_VALIDATIONS.get();
        let index = CacheIndex::new(&package, &caches, &package.regions().unwrap()).unwrap();
        assert_eq!(PART_VALIDATIONS.get(), before + 1);
        assert_eq!(index.validation.len(), 1);
        // Repeat restore uses the retained structural verdict, not another part
        // DOM; only the bounded curve codec runs for each requested snapshot.
        let expected = index.saved_row(&region(&source), &source).unwrap();
        assert_eq!(
            index.saved_row(&region(&source), &source).unwrap(),
            expected
        );
        assert_eq!(PART_VALIDATIONS.get(), before + 1);
        let wanted = HashSet::from([9]);
        assert!(
            validate_part(&" ".repeat(MAX_PART_BYTES + 1), &wanted)
                .unwrap_err()
                .contains("byte limit")
        );
    }
    #[test]
    fn sibling_results_and_foreign_part_owner_ids_do_not_poison_a_unique_good_curve() {
        let source = source(Some("9"));
        let rows = format!(
            "{}{}",
            row("", &curve()),
            row("future=\"unsupported\"", "").replace("result-id=\"9\"", "result-id=\"10\"")
        );
        let scalar = "<region region-id=\"8\"><math resultRef=\"11\"/></region>";
        let rows = format!(
            "{rows}<resultData result-id=\"11\"><m:result><m:real>3</m:real></m:result></resultData>"
        );
        let package = package(&source, scalar, &rows);
        let caches = package.caches().unwrap();
        let mut regions = package.regions().unwrap();
        let mut foreign = regions[0].clone();
        foreign.part = "mathcad/header.xml".into();
        regions.push(foreign);
        let index = CacheIndex::new(&package, &caches, &regions).unwrap();
        assert_eq!(index.validation.len(), 1);
        assert!(
            index
                .saved_row(&region(&source), &source)
                .unwrap()
                .is_some()
        );
    }
}
