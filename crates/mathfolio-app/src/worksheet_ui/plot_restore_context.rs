//! One cold-restore proof for a canvas and its actual retained archive.
//! This does not decode points, execute a query, or turn a cache into source.
use super::*;
#[cfg(test)]
thread_local! { static PROOF_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }

/// Caller supplies the package decoded from document.native_archive. The whole
/// source projection is performed once, never from a paint/key/trace loop.
pub(super) fn may_restore_synchronized(
    document: &CanvasDocument,
    package: &NativePackage,
) -> Result<bool, String> {
    #[cfg(test)]
    PROOF_CALLS.with(|n| n.set(n.get() + 1));
    if document.native_archive.is_none() {
        return Ok(false);
    }
    let defaults = CanvasDocument::default();
    let mut grid = defaults.grid;
    let mut sizes = vec![[defaults.width, defaults.height]];
    if let Some(source) = package.original_xml_part("mathcad/settings/presentation.xml")?
        && let Ok(parsed) = mathfolio_compat::PlacementGrid::from_presentation(
            source.trim_start_matches('\u{feff}'),
        )
    {
        grid = parsed.spacing();
        if let Some((paper, _)) = crate::page::source_settings(&source) {
            sizes = vec![[f64::from(paper.x), f64::from(paper.y)]];
            // Own documents may still have the exact f64 authoring paper size,
            // whereas import_native projects that same source through Vec2.
            // These are known representations, not an epsilon comparison.
            let xml = roxmltree::Document::parse(source.trim_start_matches('\u{feff}'))
                .map_err(|error| error.to_string())?;
            if let Some(model) = xml
                .descendants()
                .find(|n| n.tag_name().name() == "pageModel")
                && let Some(paper) = model
                    .attribute("paper-code")
                    .and_then(mathfolio_document::canvas::PaperSize::from_code)
            {
                let landscape = model.attribute("orientation") == Some("Landscape");
                sizes.push(paper.size(landscape));
                if paper == mathfolio_document::canvas::PaperSize::A4 && !landscape {
                    // CanvasDocument's pre-native default uses centimetres in
                    // its arithmetic; preserve those exact historic JSON bits.
                    sizes.push([defaults.width, defaults.height]);
                }
            }
        }
    }
    // Internal canvas input coordinates/grid are also signature-bound. They
    // cannot be silently replaced by the retained package's older settings.
    if document.grid.to_bits() != grid.to_bits()
        || !sizes.iter().any(|size| {
            document.width.to_bits() == size[0].to_bits()
                && document.height.to_bits() == size[1].to_bits()
        })
    {
        return Ok(false);
    }
    mathfolio_document::export::source_context_matches(document, package)
}

/// Demote retained presentations after all ordinary/Area restore branches.
/// This never submits work: values remain historical Pending presentations.
pub(super) fn invalidate_unproven(app: &mut WorksheetApp, package: Option<&NativePackage>) {
    if app.document.calculation_cache.is_some() || app.document.native_archive.is_none() {
        return;
    }
    let owned = if package.is_none() {
        app.document
            .native_archive
            .as_deref()
            .and_then(|b| NativePackage::from_bytes(b).ok())
    } else {
        None
    };
    if package
        .or(owned.as_ref())
        .is_some_and(|p| may_restore_synchronized(&app.document, p) == Ok(true))
    {
        return;
    }
    for region in &app.document.regions {
        if calculatable(region) && !region.calculation_disabled {
            app.pending_regions.insert(region.id);
        }
    }
    for row in app.saved_results.values_mut() {
        row.status = SavedCalculationStatus::Pending;
        if matches!(
            row.numeric_source,
            Some(SavedNumericSource::NativeMathml(_))
        ) {
            row.numeric_source = None;
        }
    }
    for (owner, rows) in &mut app.area_saved {
        for (key, row) in rows {
            row.status = SavedCalculationStatus::Pending;
            if matches!(
                row.numeric_source,
                Some(SavedNumericSource::NativeMathml(_))
            ) {
                row.numeric_source = None;
            }
            app.area_pending.insert((*owner, key.clone()));
        }
    }
    for (id, result) in &mut app.plot_results {
        let Ok(data) = result else { continue };
        for trace in &mut data.traces {
            if matches!(trace, crate::plots::TraceOutcome::Ready(_)) {
                let crate::plots::TraceOutcome::Ready(curve) = std::mem::replace(
                    trace,
                    crate::plots::TraceOutcome::Pending { previous: None },
                ) else {
                    unreachable!()
                };
                *trace = crate::plots::TraceOutcome::Pending {
                    previous: Some(curve),
                };
            }
        }
        if let Some(row) = app.saved_results.get_mut(id) {
            let display = data.to_saved();
            if !display.is_empty() {
                row.display = Some(display);
            }
        }
    }
    // The parent bundle is another inert snapshot. Rebuild its child statuses
    // too so an own-format save/reopen cannot re-promote nested old results.
    let empty = HashMap::new();
    for region in &app.document.regions {
        if let Some(saved) = app.area_saved.get(&region.id) {
            match area_cache::save_parent(region, &empty, saved, &app.area_pending, false) {
                Ok(row) => {
                    app.saved_results.insert(region.id, row);
                }
                Err(_) => {
                    if let Some(row) = app.saved_results.get_mut(&region.id) {
                        row.numeric_source = None;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn scratch() -> PathBuf {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let output = std::env::temp_dir().join(format!(
            "mathfolio-restore-context-{}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&output).unwrap();
        output
    }
    fn clean(output: &Path) {
        for p in std::fs::read_dir(output).unwrap() {
            std::fs::remove_file(p.unwrap().path()).unwrap();
        }
        std::fs::remove_dir(output).unwrap();
    }
    fn app(area: bool) -> WorksheetApp {
        let mut app = WorksheetApp {
            automatic_calculation: false,
            ..Default::default()
        };
        app.document.add_math(0., 0.);
        app.document.regions[0].content=CanvasContent::Math(worksheet_math::tree_from_mathml(r#"<ml:define xmlns:ml="http://schemas.mathsoft.com/math50"><ml:id>gain</ml:id><ml:real>2</ml:real></ml:define>"#).unwrap());
        app.document.add_plot(0., 100.);
        let CanvasContent::Plot(spec) = &mut app.document.regions[1].content else {
            panic!()
        };
        spec.set_axis_expression_text(mathfolio_document::canvas::PlotAxis::Y, 0, "gain*x")
            .unwrap();
        let query = app.document.add_math(0., 80.);
        app.document.regions.iter_mut().find(|r|r.id==query).unwrap().content=CanvasContent::Math(worksheet_math::tree_from_mathml(r#"<ml:eval xmlns:ml="http://schemas.mathsoft.com/math50"><ml:id>gain</ml:id><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>"#).unwrap());
        if area {
            app.document.add_opaque(0.,400.,r#"<region xmlns="http://schemas.mathsoft.com/worksheet50" xmlns:ml="http://schemas.mathsoft.com/math50" region-id="0" left="0" top="400"><Area AreaHeight="200" IsCollapsed="false"><regions><region region-id="0" left="0" top="0"><math resultRef="1"><ml:eval><ml:id>gain</ml:id><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval></math></region></regions></Area></region>"#,"Area");
        }
        app.recalculate();
        assert!(
            app.calculation_error.is_none(),
            "{:?}",
            app.calculation_error
        );
        app.persist_calculation_cache();
        app
    }
    fn native_open(output: &Path, area: bool) -> WorksheetApp {
        let source = app(area);
        let path = output.join("source.mcdx");
        source.document.export_mcdx(&path).unwrap();
        let mut opened = WorksheetApp::default();
        opened.open(&path);
        assert!(opened.last_executed_regions.is_empty());
        assert!(
            opened.pending_regions.is_empty(),
            "fresh native: {}",
            opened.status
        );
        assert!(
            opened.plot_results.values().all(Result::is_ok),
            "{}",
            opened.status
        );
        opened
    }
    fn change_gain(app: &mut WorksheetApp) {
        app.document.regions[0].content=CanvasContent::Math(worksheet_math::tree_from_mathml(r#"<ml:define xmlns:ml="http://schemas.mathsoft.com/math50"><ml:id>gain</ml:id><ml:real>7</ml:real></ml:define>"#).unwrap());
    }
    #[test]
    fn source_proof_is_in_memory_result_inert_and_rejects_upstream_changes() {
        let output = scratch();
        let mut app = native_open(&output, false);
        let package =
            NativePackage::from_bytes(app.document.native_archive.as_ref().unwrap()).unwrap();
        let original: Vec<_> = package
            .parts()
            .iter()
            .map(|p| (p.name.clone(), p.bytes().to_vec()))
            .collect();
        assert!(may_restore_synchronized(&app.document, &package).unwrap());
        change_gain(&mut app);
        assert!(!may_restore_synchronized(&app.document, &package).unwrap());
        assert_eq!(
            package
                .parts()
                .iter()
                .map(|p| (p.name.clone(), p.bytes().to_vec()))
                .collect::<Vec<_>>(),
            original
        );
        assert!(app.last_executed_regions.is_empty());
        clean(&output);
    }
    #[test]
    fn exact_own_archive_restores_zero_queries_but_changed_upstream_and_area_stay_pending() {
        for area in [false, true] {
            let output = scratch();
            let mut imported = native_open(&output, area);
            imported.document.calculation_cache = None;
            let own = output.join("unchanged.mathfolio");
            imported.document.save(&own).unwrap();
            let mut valid = WorksheetApp::default();
            valid.open(&own);
            assert!(valid.last_executed_regions.is_empty());
            assert!(valid.pending_regions.is_empty(), "{}", valid.status);
            assert!(valid.plot_results.values().all(Result::is_ok));
            let old_plot = valid.plot_results.clone();
            let old_rows = valid.saved_results.clone();
            change_gain(&mut imported);
            let changed = output.join("changed.mathfolio");
            imported.document.save(&changed).unwrap();
            let mut stale = WorksheetApp::default();
            stale.open(&changed);
            assert!(stale.last_executed_regions.is_empty());
            assert!(
                !stale.pending_regions.is_empty(),
                "stale native rows became current: {}",
                stale.status
            );
            for region in &stale.document.regions {
                if calculatable(region)
                    && let Some(row) = stale.saved_results.get(&region.id)
                {
                    assert_eq!(
                        row.status,
                        SavedCalculationStatus::Pending,
                        "region {}",
                        region.id
                    );
                }
            }
            for (id, old) in &old_rows {
                if matches!(
                    valid
                        .document
                        .regions
                        .iter()
                        .find(|r| r.id == *id)
                        .unwrap()
                        .content,
                    CanvasContent::Math(_)
                ) {
                    assert_eq!(stale.saved_results[id].display, old.display);
                    assert!(stale.saved_results[id].numeric_source.is_none());
                }
            }
            let ctx = egui::Context::default();
            crate::configure(&ctx);
            for _ in 0..2 {
                super::super::tests::frame(&mut stale, &ctx, vec![]);
            }
            assert!(
                stale.last_executed_regions.is_empty(),
                "restore must not schedule Automatic calculation"
            );
            assert_eq!(old_plot.len(), stale.plot_results.len());
            for (id, old) in &old_plot {
                let old = old.as_ref().unwrap();
                let pending = stale.plot_results[id].as_ref().unwrap();
                assert_eq!(
                    old.traces
                        .iter()
                        .map(crate::plots::TraceOutcome::curve)
                        .collect::<Vec<_>>(),
                    pending
                        .traces
                        .iter()
                        .map(crate::plots::TraceOutcome::curve)
                        .collect::<Vec<_>>()
                );
            }
            for data in stale.plot_results.values().filter_map(|r| r.as_ref().ok()) {
                assert!(
                    data.traces
                        .iter()
                        .all(|r| !matches!(r, crate::plots::TraceOutcome::Ready(_)))
                );
            }
            if area {
                assert!(
                    stale
                        .area_saved
                        .values()
                        .flat_map(|rows| rows.values())
                        .all(|row| row.status == SavedCalculationStatus::Pending)
                );
            }
            stale.persist_calculation_cache();
            let second = output.join("pending.mathfolio");
            stale.document.save(&second).unwrap();
            let mut reopened = WorksheetApp::default();
            reopened.open(&second);
            assert!(reopened.last_executed_regions.is_empty());
            assert!(
                reopened
                    .saved_results
                    .values()
                    .all(|row| row.status == SavedCalculationStatus::Pending)
            );
            assert!(
                reopened
                    .area_saved
                    .values()
                    .flat_map(|rows| rows.values())
                    .all(|row| row.status == SavedCalculationStatus::Pending)
            );
            clean(&output);
        }
    }
    #[test]
    fn exact_legacy_authoring_paper_representation_still_requires_whole_context_proof() {
        let output = scratch();
        let mut source = app(false);
        let native = output.join("source.mcdx");
        source.document.export_mcdx(&native).unwrap();
        let package = NativePackage::open(&native).unwrap();
        source.document.native_archive = Some(std::fs::read(&native).unwrap());
        for (i, region) in source.document.regions.iter_mut().enumerate() {
            region.source_region = Some((i + 1).to_string());
        }
        assert!(may_restore_synchronized(&source.document, &package).unwrap());
        let defaults = CanvasDocument::default();
        assert_eq!(source.document.width.to_bits(), defaults.width.to_bits());
        assert_eq!(source.document.height.to_bits(), defaults.height.to_bits());
        // An arbitrary nearby f64 is not a supported legacy/native form.
        source.document.width = f64::from_bits(defaults.width.to_bits() + 16);
        assert!(!may_restore_synchronized(&source.document, &package).unwrap());
        source.document.width = defaults.width;
        change_gain(&mut source);
        assert!(!may_restore_synchronized(&source.document, &package).unwrap());
        clean(&output);
    }
    #[test]
    fn trusted_actual_native_import_skips_projection_but_own_archive_requires_one_proof() {
        PROOF_CALLS.with(|n| n.set(0));
        let output = scratch();
        let mut imported = native_open(&output, false);
        assert_eq!(
            PROOF_CALLS.with(std::cell::Cell::get),
            0,
            "trusted native origin only"
        );
        imported.document.calculation_cache = None;
        let own = output.join("own.mathfolio");
        imported.document.save(&own).unwrap();
        let mut opened = WorksheetApp::default();
        opened.open(&own);
        assert_eq!(
            PROOF_CALLS.with(std::cell::Cell::get),
            1,
            "retained archive presence is not a trusted import origin"
        );
        assert!(opened.pending_regions.is_empty() && opened.last_executed_regions.is_empty());
        clean(&output);
    }
    #[test]
    fn proof_binds_grid_order_deletion_disabled_and_external_page_context() {
        let output = scratch();
        let original = native_open(&output, false);
        let package =
            NativePackage::from_bytes(original.document.native_archive.as_ref().unwrap()).unwrap();
        for case in 0..5 {
            let mut doc = original.document.clone();
            match case {
                0 => doc.grid += 1.,
                1 => {
                    doc.regions[0].top = 500.;
                }
                2 => {
                    doc.regions.remove(0);
                }
                3 => {
                    doc.regions[0].calculation_disabled = true;
                }
                _ => doc.width += 1.,
            }
            assert!(
                !may_restore_synchronized(&doc, &package).unwrap_or(false),
                "case{case}"
            );
        }
        clean(&output);
    }
}

#[cfg(test)]
#[test]
#[ignore = "Opt-in cold restore proof CPU/temporary package accounting; no original runtime"]
fn cold_context_proof_profile() {
    let output = std::path::PathBuf::from(
        std::env::var_os("MATHFOLIO_RESTORE_CONTEXT_OUTPUT").expect("output"),
    );
    std::fs::create_dir_all(&output).unwrap();
    let mut rows = vec![];
    for count in [1, 4000] {
        let mut doc = CanvasDocument::default();
        for i in 0..count {
            let id = doc.add_math(0., i as f64 * doc.grid);
            doc.regions.iter_mut().find(|r| r.id == id).unwrap().content =
                CanvasContent::Math(PortableTree {
                    kind: NodeKind::Number(i.to_string()),
                    children: vec![],
                });
        }
        doc.add_plot(0., (count + 1) as f64 * doc.grid);
        let native = output.join(format!("context-{count}.mcdx"));
        doc.export_mcdx(&native).unwrap();
        let (doc, package) = import_native(&native).unwrap();
        let decoded_bytes: usize = package.parts().iter().map(|p| p.bytes().len()).sum();
        let mut ns = vec![];
        for _ in 0..5 {
            let start = std::time::Instant::now();
            assert!(may_restore_synchronized(&doc, &package).unwrap());
            ns.push(start.elapsed().as_nanos());
        }
        rows.push(serde_json::json!({"scalar_regions":count,"plots":1,"part_bytes_cloned":decoded_bytes,"proof_elapsed_ns":ns,"queries_executed":0,"notes":"headless isolated proof elapsed CPU-bound timing; includes ephemeral decoded-part clone/source rewrite/context hashes; no zip/IO/engine in timed block"}));
    }
    std::fs::write(
        output.join("context-proof.json"),
        serde_json::to_vec_pretty(&rows).unwrap(),
    )
    .unwrap();
}
