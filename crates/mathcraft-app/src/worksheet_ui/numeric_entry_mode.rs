//! Original descriptor mode changes future authoring, never existing math.
use super::*;
impl WorksheetApp {
    pub(super) fn numeric_input_mode(&self) -> Option<bool> {
        self.document
            .bin_oct_hex_mode
            .or(self.native_bin_oct_hex_mode)
    }
    pub(super) fn set_numeric_input_mode(&mut self, ctx: &egui::Context, enabled: bool) {
        if self.numeric_input_mode() == Some(enabled) {
            return;
        }
        let before = self.snapshot_with_document(DocumentHistory::NumericInputMode(
            self.document.bin_oct_hex_mode,
        ));
        self.document.bin_oct_hex_mode = Some(enabled);
        self.remember(before);
        formula::set_numeric_input_mode(ctx, enabled);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authoring_toggle_and_mode_history_never_change_existing_math_or_calculate() {
        let mut app = WorksheetApp {
            automatic_calculation: false,
            ..Default::default()
        };
        let id = app.document.add_math(20., 20.);
        let source = PortableTree {
            kind: NodeKind::Binary(BinaryOp::Evaluate),
            children: vec![
                PortableTree {
                    kind: NodeKind::Number("1101".into()),
                    children: vec![],
                },
                PortableTree {
                    kind: NodeKind::Placeholder,
                    children: vec![],
                },
            ],
        };
        app.document.regions[0].content = CanvasContent::Math(source.clone());
        app.recalculate();
        let result = app.results[&id].value.as_ref().unwrap().numeric_json();
        app.last_executed_regions.clear();
        let signature = app.document.calculation_signature();
        let pending = app.pending_regions.clone();
        let ctx = egui::Context::default();
        for action in [
            RibbonAction::SetBinOctHexMode(true),
            RibbonAction::Undo,
            RibbonAction::Redo,
            RibbonAction::SetBinOctHexMode(false),
        ] {
            app.ribbon_command(&ctx, action);
            assert_eq!(app.document.calculation_signature(), signature);
            assert_eq!(app.pending_regions, pending);
            assert_eq!(
                app.results[&id].value.as_ref().unwrap().numeric_json(),
                result
            );
            assert_eq!(
                app.document.regions[0].content,
                CanvasContent::Math(source.clone())
            );
            assert!(app.last_executed_regions.is_empty());
        }
        assert_eq!(app.numeric_input_mode(), Some(false));
        assert_eq!(app.undo.len(), 2);
        assert!(!input_intent::ribbon_needs_baseline(
            RibbonAction::SetBinOctHexMode(true)
        ));
    }
    #[test]
    fn unknown_native_mode_is_not_a_guessed_enabled_mode_and_own_override_is_explicit() {
        let mut app = WorksheetApp {
            native_bin_oct_hex_mode: None,
            ..Default::default()
        };
        assert_eq!(app.numeric_input_mode(), None);
        app.set_numeric_input_mode(&egui::Context::default(), false);
        assert_eq!(app.numeric_input_mode(), Some(false));
        assert_eq!(app.document.bin_oct_hex_mode, Some(false));
        app.undo();
        assert_eq!(app.numeric_input_mode(), None);
        app.apply(Pending::New);
        assert_eq!(app.numeric_input_mode(), Some(false));
        assert_eq!(app.document.bin_oct_hex_mode, None);
    }
}
