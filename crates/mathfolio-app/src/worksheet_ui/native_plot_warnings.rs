//! Preserve unsupported native cache diagnostics across own-format saves.
use super::*;
pub(super) fn restore(app: &mut WorksheetApp, package: &NativePackage) {
    let candidates: Vec<_> = app
        .document
        .regions
        .iter()
        .filter(|region| {
            matches!(region.content, CanvasContent::Plot(_))
                && region.source_region.is_some()
                && app
                    .saved_results
                    .get(&region.id)
                    .is_none_or(|row| row.display.is_none() && row.error.is_none())
        })
        .collect();
    if candidates.is_empty() {
        return;
    }
    let inspected = (|| {
        let regions = package.regions()?;
        let caches = package.caches()?;
        let source = package.worksheet_xml()?;
        let dom = roxmltree::Document::parse(&source).map_err(|error| error.to_string())?;
        let index = native_plot_source::CacheIndex::new(package, &caches, &regions)?;
        let sources: HashMap<_, _> = dom
            .descendants()
            .filter(|node| node.has_tag_name(("http://schemas.mathsoft.com/worksheet50", "region")))
            .filter_map(|node| Some((node.attribute("region-id")?, node)))
            .collect();
        let mut errors = Vec::new();
        for region in &candidates {
            let owner = region.source_region.as_deref().unwrap();
            let Some(node) = sources.get(owner) else {
                continue;
            };
            let raw = standalone(*node, &source);
            let CanvasContent::Plot(current) = &region.content else {
                continue;
            };
            // Own documents can contain a legitimate committed edit newer than
            // the retained native archive. Its old cache is not a decode fault.
            if mathfolio_document::native_plot::parse(&raw).and_then(|original| {
                mathfolio_document::native_plot::matches_retained_source(current, &original)
            }) != Ok(true)
            {
                continue;
            }
            if let Err(error) = index.saved_row(region, &raw) {
                errors.push((owner.to_string(), error));
            }
        }
        Ok::<_, String>(errors)
    })();
    let errors = inspected.unwrap_or_else(|error| {
        candidates
            .iter()
            .map(|region| (region.source_region.clone().unwrap(), error.clone()))
            .collect()
    });
    for (owner, error) in errors {
        Arc::make_mut(&mut app.native_visuals)
            .regions
            .insert(owner, Err(error));
        app.status.push_str(" · 원본 그래프 저장 결과 지원 필요");
    }
}
