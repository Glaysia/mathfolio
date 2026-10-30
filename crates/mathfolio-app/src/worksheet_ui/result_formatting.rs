#[cfg(test)]
mod tests {
    use super::super::*;
    use mathfolio_document::canvas::ComplexFormat;
    use std::io::Write;

    fn path(extension: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "mathfolio-format-{}-{}.{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            extension
        ))
    }
    fn text(app: &mut WorksheetApp, id: u64) -> String {
        let style = app
            .document
            .regions
            .iter()
            .find(|r| r.id == id)
            .unwrap()
            .format;
        app.display_result(id, style)
            .unwrap()
            .cells
            .iter()
            .map(|c| c.plain())
            .collect::<Vec<_>>()
            .join(";")
    }
    fn calculated(source: &str) -> (WorksheetApp, u64) {
        let mut app = WorksheetApp::default();
        let id = app.document.add_math(0., 20.);
        let mut editor = Editor::new();
        formula::input_text(&mut editor, source);
        app.document.regions[0].content = CanvasContent::Math(editor.to_portable().unwrap());
        app.recalculate();
        app.selected = Some(id);
        app.last_executed_regions.clear();
        (app, id)
    }
    fn native(result: &str, local: &str, default: &str) -> PathBuf {
        let path = path("mcdx");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        let parts = [
            (
                "mathcad/worksheet.xml",
                format!(
                    r#"<worksheet xmlns="http://schemas.mathsoft.com/worksheet50" xmlns:ml="http://schemas.mathsoft.com/math50"><regions><region region-id="42" left="0" top="20"><math resultRef="900"><ml:eval><ml:id labels="VARIABLE">unavailable</ml:id><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>{local}</math></region></regions></worksheet>"#
                ),
            ),
            (
                "mathcad/results.xml",
                format!(
                    r#"<resultsList xmlns="http://schemas.mathsoft.com/result10" xmlns:ml="http://schemas.mathsoft.com/math50"><resultData result-id="900" calculation-status="Synchronized">{result}</resultData></resultsList>"#
                ),
            ),
            (
                "mathcad/settings/presentation.xml",
                format!(
                    r#"<presentation xmlns="http://schemas.ptc.com/mathcad/settings/presentation10" xmlns:ws="http://schemas.mathsoft.com/worksheet50"><mathRendering><ws:results>{default}</ws:results></mathRendering></presentation>"#
                ),
            ),
        ];
        for (name, source) in parts {
            zip.start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(source.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
        path
    }

    #[test]
    fn result_buttons_format_live_values_and_saved_full_precision_without_recalculation() {
        let (mut app, id) = calculated("0.94359=");
        let context = egui::Context::default();
        assert_eq!(text(&mut app, id), "0.944");
        app.ribbon_command(&context, RibbonAction::SetResultFormat(4));
        assert_eq!(text(&mut app, id), "94.359%");
        app.ribbon_command(&context, RibbonAction::SetTrailingZeros(true));
        app.ribbon_command(&context, RibbonAction::SetPrecision(5));
        assert_eq!(text(&mut app, id), "94.35900%");
        assert!(app.last_executed_regions.is_empty());
        assert_eq!(
            app.results[&id].value.as_ref().unwrap().numeric_json()["real"],
            0.94359
        );
        app.persist_calculation_cache();
        let saved = path("mathfolio");
        app.document.save(&saved).unwrap();
        app.open(&saved);
        app.selected = Some(id);
        app.ribbon_command(&context, RibbonAction::SetResultFormat(1));
        assert_eq!(text(&mut app, id), "0.94359");
        assert!(app.last_executed_regions.is_empty());
        assert!(app.results.is_empty());
        std::fs::remove_file(saved).unwrap();
    }

    #[test]
    fn native_defaults_local_polar_override_and_percent_keep_cold_open_inert() {
        let percent = native(
            "<ml:real>0.94359</ml:real>",
            "",
            "<ws:percent precision=\"3\" show-trailing-zeros=\"false\"/>",
        );
        let mut app = WorksheetApp::default();
        app.open(&percent);
        let id = app.document.regions[0].id;
        assert_eq!(
            app.document.regions[0].format.result_format,
            ResultFormat::Percent
        );
        assert_eq!(text(&mut app, id), "94.359%");
        app.selected = Some(id);
        app.ribbon_command(&egui::Context::default(), RibbonAction::SetResultFormat(1));
        assert_eq!(text(&mut app, id), "0.944");
        assert!(app.last_executed_regions.is_empty());
        assert!(app.results.is_empty());
        assert!(app.calculation_error.is_none());
        std::fs::remove_file(percent).unwrap();

        let polar = native(
            "<ml:complex><ml:real>3</ml:real><ml:imag>4</ml:imag></ml:complex>",
            "<resultFormat><general imaginary-value=\"polarDegrees\"/></resultFormat>",
            "<ws:decimal precision=\"4\" imaginary-value=\"j\" show-trailing-zeros=\"true\"/>",
        );
        app.open(&polar);
        let id = app.document.regions[0].id;
        assert_eq!(
            app.document.regions[0].format.complex_format,
            ComplexFormat::PolarDegrees
        );
        assert_eq!(app.document.regions[0].format.precision, 4);
        let value = text(&mut app, id);
        assert!(value.contains("5.0000"), "{value}");
        assert!(value.contains("53.1301"), "{value}");
        assert!(value.contains('∠') && value.contains('°'), "{value}");
        assert!(app.last_executed_regions.is_empty());
        std::fs::remove_file(polar).unwrap();
    }

    #[test]
    fn complex_format_button_is_undoable_persistent_and_preserves_numeric_value() {
        let (mut app, id) = calculated("3+4i=");
        let before = app.results[&id].value.as_ref().unwrap().numeric_json();
        let context = egui::Context::default();
        app.ribbon_command(
            &context,
            RibbonAction::SetComplexFormat(ComplexFormat::PolarDegrees),
        );
        assert!(text(&mut app, id).contains("53.13"));
        assert_eq!(
            app.results[&id].value.as_ref().unwrap().numeric_json(),
            before
        );
        let export = path("mcdx");
        mathfolio_document::export::export_mcdx(&app.document, &export).unwrap();
        let mut imported = WorksheetApp::default();
        imported.open(&export);
        assert_eq!(
            imported.document.regions[0].format.complex_format,
            ComplexFormat::PolarDegrees
        );
        imported.recalculate();
        assert_eq!(text(&mut imported, id), text(&mut app, id));
        app.undo();
        assert_eq!(
            app.document.regions[0].format.complex_format,
            ComplexFormat::I
        );
        std::fs::remove_file(export).unwrap();
    }

    #[test]
    fn malformed_cached_numeric_data_cannot_become_an_executable_expression() {
        let mut row = SavedCalculationRow {
            region_id: 1,
            status: SavedCalculationStatus::Synchronized,
            display: None,
            error: None,
            warnings: vec![],
            symbolic_mathml: None,
            numeric_source: Some(SavedNumericSource::EvaluatedValue(
                "{\"kind\":\"matrix\",\"rows\":2,\"cols\":2,\"elements\":[]}".into(),
            )),
        };
        assert!(numeric_cache::display(&row, &MathFormat::default()).is_none());
        row.numeric_source = Some(SavedNumericSource::NativeMathml(
            "<apply><id>unavailable</id></apply>".into(),
        ));
        assert!(numeric_cache::display(&row, &MathFormat::default()).is_none());
    }
}
