//! Calculation-only state saved before the first draft mutation. Capturing it
//! does not serialize MathML, label a worksheet, hash a source, or run an engine.
use super::*;

type EditorValidation = HashMap<u64, (u64, Option<String>)>;

pub(super) enum EditorInputs<'a> {
    Live(&'a HashMap<u64, Editor>),
    Frozen(&'a EditorValidation),
}
impl EditorInputs<'_> {
    pub(super) fn revisions(&self) -> HashMap<u64, u64> {
        match self {
            Self::Live(editors) => editors
                .iter()
                .map(|(id, editor)| (*id, editor.arena().revision()))
                .collect(),
            Self::Frozen(editors) => editors
                .iter()
                .map(|(id, (revision, _))| (*id, *revision))
                .collect(),
        }
    }
    pub(super) fn faults(
        &self,
        cache: &mut EditorValidation,
        disabled: &HashSet<u64>,
    ) -> HashMap<u64, String> {
        match self {
            Self::Live(editors) => {
                cache.retain(|id, _| editors.contains_key(id));
                editors
                    .iter()
                    .filter_map(|(id, editor)| {
                        if disabled.contains(id) {
                            return None;
                        }
                        let revision = editor.arena().revision();
                        let entry = cache.entry(*id).or_insert((u64::MAX, None));
                        if entry.0 != revision {
                            *entry = (revision, validation_error(editor));
                        }
                        entry.1.as_ref().map(|error| (*id, error.clone()))
                    })
                    .collect()
            }
            Self::Frozen(editors) => {
                cache.clone_from(editors);
                editors
                    .iter()
                    .filter(|(id, _)| !disabled.contains(id))
                    .filter_map(|(id, (_, error))| error.as_ref().map(|error| (*id, error.clone())))
                    .collect()
            }
        }
    }
}

fn validation_error(editor: &Editor) -> Option<String> {
    editor
        .to_portable()
        .err()
        .map(|error| format!("Incomplete expression: {error}"))
}

pub(super) struct CommittedBaseline {
    document: CanvasDocument,
    editors: EditorValidation,
}
impl CommittedBaseline {
    pub(super) fn capture(
        document: &CanvasDocument,
        editors: &HashMap<u64, Editor>,
        validation: &EditorValidation,
    ) -> Self {
        let frozen_editors = editors
            .iter()
            .map(|(id, editor)| {
                let revision = editor.arena().revision();
                let error = validation
                    .get(id)
                    .filter(|(old, _)| *old == revision)
                    .map(|(_, error)| error.clone())
                    .unwrap_or_else(|| validation_error(editor));
                (*id, (revision, error))
            })
            .collect();
        Self {
            document: CanvasDocument {
                width: document.width,
                height: document.height,
                grid: document.grid,
                // Signature and source preparation ignore plain text. Preserve
                // all other region fields, including disabled/override flags,
                // presentation, opaque payloads, and geometric order.
                regions: document
                    .regions
                    .iter()
                    .filter(|region| !matches!(region.content, CanvasContent::Text(_)))
                    .cloned()
                    .collect(),
                native_archive: document.native_archive.clone(),
                ..Default::default()
            },
            editors: frozen_editors,
        }
    }
    pub(super) fn materialize(&self) -> Result<CalculationInput, String> {
        prepare_input(
            &mut PreparationCache::default(),
            &self.document,
            EditorInputs::Frozen(&self.editors),
            &mut HashMap::new(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add(app: &mut WorksheetApp, text: &str, top: f64) -> u64 {
        let mut editor = Editor::new();
        formula::input_text(&mut editor, text);
        let tree = editor.to_portable().unwrap();
        let id = app.document.add_math(0., top);
        app.document.regions.last_mut().unwrap().content = CanvasContent::Math(tree);
        app.editors.insert(id, editor);
        id
    }
    fn fixture() -> (WorksheetApp, u64, u64) {
        let mut app = WorksheetApp::default();
        let definition = add(&mut app, "a:=2", 0.);
        let query = add(&mut app, "a=", 80.);
        (app, definition, query)
    }
    fn oracle(app: &WorksheetApp) -> (CalculationInput, HashMap<u64, PortableTree>) {
        let mut labels = HashMap::new();
        let input = prepare_input(
            &mut PreparationCache::default(),
            &app.document,
            EditorInputs::Live(&app.editors),
            &mut labels,
        )
        .unwrap();
        assert_eq!(input.signature, app.document.active_calculation_signature());
        (input, labels)
    }
    fn assert_input(actual: &CalculationInput, expected: &CalculationInput) {
        assert_eq!(actual.source, expected.source);
        assert_eq!(actual.signature, expected.signature);
        assert_eq!(actual.faults, expected.faults);
        assert_eq!(actual.region_ids, expected.region_ids);
    }
    fn change_rhs(app: &mut WorksheetApp, id: u64, value: &str) {
        let editor = app.editors.get_mut(&id).unwrap();
        let rhs = editor.arena().get(editor.root()).unwrap().children()[1];
        editor
            .set_atom(rhs, NodeKind::Number(value.into()))
            .unwrap();
        if let Ok(tree) = editor.to_portable() {
            app.document
                .regions
                .iter_mut()
                .find(|r| r.id == id)
                .unwrap()
                .content = CanvasContent::Math(tree);
        }
        app.stage_math_edit(id, false);
    }
    #[test]
    fn cold_retain_keeps_only_raw_source_and_frozen_materialization_is_draft_independent() {
        let (mut app, definition, _) = fixture();
        let (expected, labels) = oracle(&app);
        app.document.calculation_cache = Some(SavedCalculationCache {
            source_signature: "unused".into(),
            rows: vec![],
        });
        app.retain_committed_input();
        assert!(app.committed_input.is_none());
        assert!(app.background.preparation.input.is_none());
        assert!(app.background.preparation.has_committed_baseline());
        assert_eq!(app.background.preparation.full_dom_parses, 0);
        assert_eq!(app.background.preparation.labelled_decodes, 0);
        assert!(app.labelled_trees.is_empty());
        assert!(app.last_executed_regions.is_empty());
        assert!(app.results.is_empty());
        let baseline = app
            .background
            .preparation
            .committed_baseline
            .as_ref()
            .unwrap();
        assert!(baseline.document.calculation_cache.is_none());
        assert!(baseline.document.page_sections == CanvasDocument::default().page_sections);
        let mut frozen_labels = HashMap::new();
        let frozen = prepare_input(
            &mut PreparationCache::default(),
            &baseline.document,
            EditorInputs::Frozen(&baseline.editors),
            &mut frozen_labels,
        )
        .unwrap();
        assert_input(&frozen, &expected);
        assert_eq!(frozen_labels, labels);
        change_rhs(&mut app, definition, "23");
        let live = app.prepare_calculation_input().unwrap();
        let draft_labels = app.labelled_trees.clone();
        assert_ne!(live.source, expected.source);
        let committed = app.materialize_committed_input().unwrap();
        assert_input(&committed, &expected);
        assert_eq!(app.labelled_trees, draft_labels);
        assert!(Arc::ptr_eq(
            &live.source,
            &app.background.preparation.input.as_ref().unwrap().source
        ));
        assert_eq!(app.background.preparation.baseline_materializations, 1);
        assert_input(&app.materialize_committed_input().unwrap(), &expected);
        assert_eq!(app.background.preparation.baseline_materializations, 1);
        assert!(app.results.is_empty());
        assert!(app.last_executed_regions.is_empty());
    }
    #[test]
    fn a_commit_discards_the_unused_baseline_without_preparing_the_old_worksheet() {
        let (mut app, definition, query) = fixture();
        app.retain_committed_input();
        change_rhs(&mut app, definition, "23");
        app.commit_math_edits();
        assert!(!app.background.preparation.has_committed_baseline());
        assert_eq!(app.background.preparation.baseline_materializations, 0);
        assert!(app.uncommitted_math.is_empty());
        assert_eq!(
            app.results[&query].value.as_ref().unwrap().numeric_json()["real"],
            23.
        );
    }
    #[test]
    fn offscreen_f5_and_auto_toggle_materialize_the_frozen_source_without_committing_the_draft() {
        for toggle in [false, true] {
            let (mut app, definition, query) = fixture();
            app.automatic_calculation = false;
            app.retain_committed_input();
            change_rhs(&mut app, definition, "23");
            if toggle {
                app.ribbon_command(
                    &egui::Context::default(),
                    RibbonAction::SetAutomaticCalculation(true),
                );
            }
            app.visible_regions.clear();
            app.visible_calculation_end = Some(query);
            app.calculate_command(CalculationCommand::Now);
            assert!(app.uncommitted_math.contains(&definition));
            assert_eq!(
                app.results[&query].value.as_ref().unwrap().numeric_json()["real"],
                2.
            );
            assert_eq!(app.background.preparation.baseline_materializations, 1);
            app.commit_math_edits();
            app.calculate_command(CalculationCommand::Worksheet);
            assert_eq!(
                app.results[&query].value.as_ref().unwrap().numeric_json()["real"],
                23.
            );
        }
    }
    #[test]
    fn editor_invalidity_disabled_regions_and_undo_match_the_live_preparation_oracle() {
        let (mut app, definition, query) = fixture();
        change_rhs(&mut app, definition, "NaN");
        app.uncommitted_math.clear();
        let disabled = add(&mut app, "b:=4", 160.);
        app.document
            .regions
            .last_mut()
            .unwrap()
            .calculation_disabled = true;
        change_rhs(&mut app, disabled, "NaN");
        app.uncommitted_math.clear();
        let (expected, labels) = oracle(&app);
        assert!(expected.faults.contains_key(&definition));
        assert!(!expected.faults.contains_key(&disabled));
        app.retain_committed_input();
        // The editor can be undone and the live document independently replaced;
        // the old validation failure still belongs to the frozen baseline.
        assert!(app.editors.get_mut(&definition).unwrap().undo());
        let tree = app.editors[&definition].to_portable().unwrap();
        app.document
            .regions
            .iter_mut()
            .find(|r| r.id == definition)
            .unwrap()
            .content = CanvasContent::Math(tree);
        app.stage_math_edit(query, false);
        assert_input(&app.materialize_committed_input().unwrap(), &expected);
        assert!(app.labelled_trees.is_empty());
        assert!(!labels.is_empty());
    }
    #[test]
    fn no_op_retention_is_replaced_before_a_later_real_edit_and_same_length_archive_changes_are_kept()
     {
        let (mut app, definition, _) = fixture();
        app.document.native_archive = Some(vec![1, 2, 3]);
        app.retain_committed_input();
        let initial = app
            .background
            .preparation
            .committed_baseline
            .as_ref()
            .unwrap()
            .materialize()
            .unwrap();
        // A potential IME/key event made no edit. Committed formatting, order,
        // dimensions, and same-sized archive replacement happen before typing.
        app.document.regions[0].format.precision = 8;
        app.document.regions[0].calculation_enabled_override = true;
        app.document.regions[0].top = 120.;
        app.document.width += 1.;
        app.document.native_archive.as_mut().unwrap()[0] = 9;
        let (expected, _) = oracle(&app);
        assert_ne!(expected.signature, initial.signature);
        app.retain_committed_input();
        change_rhs(&mut app, definition, "23");
        assert_input(&app.materialize_committed_input().unwrap(), &expected);
    }
    #[test]
    fn full_xml_preparation_errors_are_deferred_until_the_committed_source_is_requested() {
        let (mut app, definition, _) = fixture();
        app.document.regions[0].left = f64::NAN;
        let expected = prepare_input(
            &mut PreparationCache::default(),
            &app.document,
            EditorInputs::Live(&app.editors),
            &mut HashMap::new(),
        )
        .err()
        .unwrap();
        app.retain_committed_input();
        assert!(app.calculation_error.is_none());
        assert!(app.committed_input.is_none());
        change_rhs(&mut app, definition, "23");
        assert_eq!(app.materialize_committed_input().err().unwrap(), expected);
        assert!(app.background.preparation.has_committed_baseline());
        assert!(app.committed_input.is_none());
        assert!(app.last_executed_regions.is_empty());
        assert!(app.results.is_empty());
    }
    #[test]
    fn open_and_new_drop_a_retained_draft_baseline_without_calculating() {
        let (mut app, definition, _) = fixture();
        app.retain_committed_input();
        change_rhs(&mut app, definition, "23");
        app.restore_saved_calculations();
        assert!(!app.background.preparation.has_committed_baseline());
        assert!(app.committed_input.is_none());
        assert!(app.last_executed_regions.is_empty());
        app.retain_committed_input();
        app.document = CanvasDocument::default();
        app.restore_saved_calculations();
        assert!(!app.background.preparation.has_committed_baseline());
        assert!(app.results.is_empty());
    }

    #[test]
    fn a_spec_table_source_and_its_column_definitions_are_frozen_before_cell_commit() {
        use mathfolio_document::spec_table::{CellSlot, SpecTable};
        let (mut app, definition, _) = fixture();
        let mut table = SpecTable::new(2, 1).unwrap();
        let atom = |kind| PortableTree {
            kind,
            children: vec![],
        };
        table
            .set_cell(
                0,
                CellSlot::Header,
                &atom(NodeKind::Identifier("col_values".into())),
            )
            .unwrap();
        table
            .set_cell(0, CellSlot::Body(0), &atom(NodeKind::Number("3".into())))
            .unwrap();
        table
            .set_cell(0, CellSlot::Body(1), &atom(NodeKind::Number("4".into())))
            .unwrap();
        let id = app.document.add_opaque(0., 120., table.xml(), "spec-table");
        let (expected, _) = oracle(&app);
        app.retain_committed_input();
        table
            .set_cell(0, CellSlot::Body(0), &atom(NodeKind::Number("99".into())))
            .unwrap();
        app.document
            .regions
            .iter_mut()
            .find(|r| r.id == id)
            .unwrap()
            .content = CanvasContent::Opaque {
            xml: table.xml().into(),
            kind: "spec-table".into(),
        };
        app.stage_math_edit(id, false);
        change_rhs(&mut app, definition, "23");
        assert_input(&app.materialize_committed_input().unwrap(), &expected);
        assert!(
            !app.committed_input
                .as_ref()
                .unwrap()
                .source
                .contains(">99<")
        );
        assert!(app.results.is_empty());
        assert!(app.last_executed_regions.is_empty());
    }
}
