use super::*;

fn legacy_open(app: &mut WorksheetApp, path: &Path) {
    #[cfg(test)]
    let load_timing = crate::frame_profile::scope(crate::frame_profile::Phase::OpenLoad);
    let result = if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("mcdx"))
    {
        import_native(path).map(|(document, _)| document)
    } else {
        CanvasDocument::load(path)
    };
    #[cfg(test)]
    drop(load_timing);
    match result {
        Ok(document) => {
            #[cfg(test)]
            let visual_timing =
                crate::frame_profile::scope(crate::frame_profile::Phase::OpenVisuals);
            app.background.reset();
            app.saved_date = crate::page_fields::SavedDate::from_path(path).unwrap_or_default();
            app.page_margins = crate::page::standard_margins(&document);
            app.native_visuals = Arc::default();
            let mut visual_warning = None;
            #[cfg(test)]
            let mut archive_timing = Some(crate::frame_profile::scope(
                crate::frame_profile::Phase::VisualArchive,
            ));
            if let Some(bytes) = &document.native_archive
                && let Ok(package) = NativePackage::from_bytes(bytes)
            {
                #[cfg(test)]
                drop(archive_timing.take());
                #[cfg(test)]
                let decode_timing =
                    crate::frame_profile::scope(crate::frame_profile::Phase::VisualDecode);
                match NativeVisuals::from_package(&package) {
                    Ok(visuals) => {
                        let failed = visuals.regions.values().filter(|v| v.is_err()).count();
                        if failed > 0 {
                            visual_warning =
                                Some(format!(" · 원본 서식 {failed}개 영역 지원 필요"));
                        }
                        app.native_visuals = Arc::new(visuals);
                    }
                    Err(error) => visual_warning = Some(format!(" · 원본 서식: {error}")),
                }
                #[cfg(test)]
                drop(decode_timing);
                #[cfg(test)]
                let _settings_timing =
                    crate::frame_profile::scope(crate::frame_profile::Phase::VisualSettings);
                if let Ok(Some(source)) =
                    package.original_xml_part("mathcad/settings/presentation.xml")
                {
                    if document.page_setup.is_none()
                        && let Some((_, margins)) = crate::page::source_settings(&source)
                    {
                        app.page_margins = margins;
                    }
                    (app.show_grid, app.use_major_grid) = crate::page::view_settings(&source);
                }
            }
            #[cfg(test)]
            drop(archive_timing);
            #[cfg(test)]
            drop(visual_timing);
            #[cfg(test)]
            let _restore_timing =
                crate::frame_profile::scope(crate::frame_profile::Phase::OpenRestore);
            app.document = document;
            app.editors.clear();
            app.table_editors.clear();
            #[cfg(test)]
            let table_timing = crate::frame_profile::scope(crate::frame_profile::Phase::OpenTables);
            let tables: Vec<_> = app.document.regions.iter().filter(|r| matches!(&r.content, CanvasContent::Opaque { kind, .. } if matches!(kind.as_str(), "spec-table" | "specTable"))).map(|r| r.id).collect();
            for id in tables {
                app.refresh_table_visual(id);
            }
            #[cfg(test)]
            drop(table_timing);
            app.text_cursors.clear();
            app.region_heights.clear();
            app.region_widths.clear();
            app.undo.clear();
            app.redo.clear();
            app.results.clear();
            app.dirty = false;
            app.selected = None;
            app.editing_region = None;
            app.focus_new = None;
            app.dragging = None;
            app.drag_preview = None;
            app.drag_pointer_origin = None;
            app.page_sections_draft = None;
            app.insertion = None;
            app.path = if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("mathfolio"))
            {
                Some(path.into())
            } else {
                None
            };
            app.status = format!("Opened {}", path.display());
            app.restore_saved_calculations();
            if let Some(warning) = visual_warning {
                app.status.push_str(&warning);
            }
        }
        Err(e) => app.status = format!("Could not open: {e}"),
    }
}

fn compare_open(path: &Path) {
    let before = std::fs::read(path).unwrap();
    let mut old = WorksheetApp::default();
    let mut new = WorksheetApp::default();
    legacy_open(&mut old, path);
    new.open(path);
    assert_eq!(old.status, new.status);
    assert_eq!(
        serde_json::to_value(&old.document).unwrap(),
        serde_json::to_value(&new.document).unwrap()
    );
    assert_eq!(
        old.native_visuals.regions, new.native_visuals.regions,
        "all colors/XAML/table visuals"
    );
    assert_eq!(
        serde_json::to_value(&old.saved_results).unwrap(),
        serde_json::to_value(&new.saved_results).unwrap()
    );
    assert_eq!(old.pending_regions, new.pending_regions);
    assert_eq!(old.calculation_pending, new.calculation_pending);
    assert_eq!(old.page_margins, new.page_margins);
    assert_eq!(old.show_grid, new.show_grid);
    assert_eq!(old.use_major_grid, new.use_major_grid);
    assert_eq!(old.plot_signature, new.plot_signature);
    assert_eq!(old.calculation_error, new.calculation_error);
    if path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("mcdx"))
        && new.status.starts_with("Opened ")
    {
        // The independent legacy opener keeps its original copy-only policy.
        // Native Save intentionally retains the real path/revision; every
        // document, visual, cache and calculation comparison above stays exact.
        assert!(old.path.is_none());
        assert_eq!(new.path.as_deref(), Some(path));
        assert_eq!(
            new.native_save_target,
            Some(native_save_ui::NativeTarget::from_open(path, &new.document).unwrap())
        );
    } else {
        assert_eq!(old.path, new.path);
        assert!(new.native_save_target.is_none());
    }
    assert!(old.last_executed_regions.is_empty() && new.last_executed_regions.is_empty());
    assert!(old.editors.is_empty() && new.editors.is_empty());
    assert!(old.labelled_trees.is_empty() && new.labelled_trees.is_empty());
    assert_eq!(std::fs::read(path).unwrap(), before);
}
#[test]
fn native_open_matches_independent_legacy_for_saved_results_disabled_and_own_caches() {
    for disabled in [false, true] {
        let native = native_saved_result_fixture_with_disabled(
            "<ml:result><ml:real>12.5</ml:real></ml:result>",
            "Synchronized",
            false,
            disabled,
        );
        compare_open(&native);
        let mut source = WorksheetApp::default();
        source.open(&native);
        for cache in ["absent", "valid", "stale"] {
            let saved = calculation_test_path("mathfolio");
            source.document.calculation_cache = if cache == "absent" {
                None
            } else {
                Some(SavedCalculationCache {
                    source_signature: if cache == "valid" {
                        source.document.calculation_signature()
                    } else {
                        "stale".into()
                    },
                    rows: source.saved_results.values().cloned().collect(),
                })
            };
            if cache == "stale" {
                std::fs::write(&saved, serde_json::to_vec(&source.document).unwrap()).unwrap();
            } else {
                source.document.save(&saved).unwrap();
            }
            compare_open(&saved);
            std::fs::remove_file(saved).unwrap();
        }
        std::fs::remove_file(native).unwrap();
    }
}
#[test]
fn native_open_invalid_preserved_archive_and_import_errors_match_legacy() {
    let doc = CanvasDocument {
        native_archive: Some(b"invalid archive bytes".to_vec()),
        ..Default::default()
    };
    let saved = calculation_test_path("mathfolio");
    std::fs::write(&saved, serde_json::to_vec(&doc).unwrap()).unwrap();
    compare_open(&saved);
    std::fs::remove_file(saved).unwrap();
    let invalid = calculation_test_path("mcdx");
    std::fs::write(&invalid, b"not ZIP").unwrap();
    compare_open(&invalid);
    std::fs::remove_file(invalid).unwrap();
}
#[test]
#[ignore = "read-only original complete visual/cache/source oracle and own save variants"]
fn native_original_open_matches_legacy_for_full_visuals_current_table_and_cache_states() {
    for name in ["1015_LCC_S", "1016_LCC_S"] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../artifacts/rea/unit-selected-sources")
            .join(format!("{name}.mcdx"));
        compare_open(&path);
        let mut source = WorksheetApp::default();
        source.open(&path);
        for edited in [false, true] {
            if edited {
                for region in &mut source.document.regions {
                    if let CanvasContent::Opaque { xml, kind } = &mut region.content
                        && kind == "spec-table"
                    {
                        *xml = xml.replacen("<ml:real>", "<ml:real>9", 1);
                    }
                }
            }
            for cache in ["absent", "valid", "stale"] {
                let saved = calculation_test_path("mathfolio");
                source.document.calculation_cache = if cache == "absent" {
                    None
                } else {
                    Some(SavedCalculationCache {
                        source_signature: if cache == "valid" {
                            source.document.calculation_signature()
                        } else {
                            "stale".into()
                        },
                        rows: source.saved_results.values().cloned().collect(),
                    })
                };
                if cache == "stale" {
                    std::fs::write(&saved, serde_json::to_vec(&source.document).unwrap()).unwrap();
                } else {
                    source.document.save(&saved).unwrap();
                }
                compare_open(&saved);
                std::fs::remove_file(saved).unwrap();
            }
        }
        println!(
            "NATIVE_OPEN_ORACLE {name} regions={} visuals={} saved={}",
            source.document.regions.len(),
            source.native_visuals.regions.len(),
            source.saved_results.len()
        );
    }
}

fn independent_table_fixture(alias: bool, invalid: bool, utf16: bool) -> PathBuf {
    use std::io::Write;
    let tag = if alias { "specTable" } else { "spec-table" };
    let value = if invalid {
        "<q:unsupported/>"
    } else {
        "<q:real>1.00</q:real>"
    };
    let worksheet = format!(
        r##"<worksheet xmlns="http://schemas.mathsoft.com/worksheet50" xmlns:q="http://schemas.mathsoft.com/math50"><regions><region region-id="7" left="-0" top="32" width="181.5" actualHeight="108"><{tag}><math resultRef="18"><q:define><q:id labels="VARIABLE">lengths</q:id><q:apply><q:scale/><q:matrix rows="2" cols="1">{value}<q:real>2.00</q:real></q:matrix><q:id labels="UNIT">mm</q:id></q:apply></q:define><formattingOverride FontSize="17"/></math><math><q:define><q:id labels="VARIABLE">rates</q:id><q:matrix rows="2" cols="1"><q:real>3</q:real><q:real>4</q:real></q:matrix></q:define></math></{tag}></region><region region-id="8" left="-0" top="160" width="70" actualHeight="22"><math><q:eval><q:id labels="*" label-is-contextual="true">m</q:id><q:unitOverride><q:placeholder/></q:unitOverride></q:eval><formattingOverride BackgroundColor="#AA2060E0" FontSize="13"/></math></region></regions></worksheet>"##
    );
    let presentation = r#"<presentation xmlns="http://schemas.ptc.com/mathcad/settings/presentation10" xmlns:ws="http://schemas.mathsoft.com/worksheet50"><mathRendering><formatting FontSize="19"/><ws:results><ws:general precision="6"/></ws:results></mathRendering></presentation>"#;
    let path = calculation_test_path("mcdx");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    let bytes = if utf16 {
        let mut b = vec![0xff, 0xfe];
        b.extend(worksheet.encode_utf16().flat_map(u16::to_le_bytes));
        b
    } else {
        worksheet.into_bytes()
    };
    for (name, bytes) in [
        ("mathcad/worksheet.xml", bytes),
        (
            "mathcad/settings/presentation.xml",
            presentation.as_bytes().to_vec(),
        ),
    ] {
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(&bytes).unwrap();
    }
    zip.finish().unwrap();
    path
}
#[test]
fn native_fresh_table_styles_defaults_utf16_and_alias_error_fallback_match_legacy() {
    for (alias, invalid, utf16) in [
        (false, false, false),
        (false, false, true),
        (false, true, false),
        (true, false, false),
    ] {
        let path = independent_table_fixture(alias, invalid, utf16);
        compare_open(&path);
        let mut app = WorksheetApp::default();
        app.open(&path);
        assert!(app.status.starts_with("Opened"), "{}", app.status);
        if !alias && !invalid {
            let visual = app.native_visuals.regions["7"].as_ref().unwrap();
            assert_eq!(visual.size, [181.5, 108.]);
            let math = app.native_visuals.regions["8"].as_ref().unwrap();
            assert_eq!(math.background, Some([0x20, 0x60, 0xe0, 0xaa]));
            assert_eq!(app.document.regions[0].format.font_points, 19.);
        }
        if invalid {
            assert!(app.native_visuals.regions["7"].is_err());
        }
        std::fs::remove_file(path).unwrap();
    }
}
