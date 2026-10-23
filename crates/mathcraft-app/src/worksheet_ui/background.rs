//! Desktop calculation dispatch. Direct calculator tests keep a synchronous path.
use super::*;
use crate::calculation_worker::{CalculationJob, CalculationReply, CalculationWorker, PlotJob};
use std::sync::mpsc::TryRecvError;

#[derive(Default)]
pub(super) struct BackgroundState {
    worker: Option<CalculationWorker>,
    generation: u64,
    pub(super) running: bool,
    source: Option<Arc<str>>,
    signature: Option<String>,
    faults: HashMap<u64, String>,
    whole: bool,
    pub(super) preparation: super::edit_commit::PreparationCache,
}
impl BackgroundState {
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn web_worker_memory(&self) -> Option<u32> {
        self.worker
            .as_ref()
            .and_then(CalculationWorker::memory_bytes)
    }
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn web_worker_count(&self) -> u32 {
        self.worker.as_ref().map_or(0, CalculationWorker::count)
    }
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn web_worker_started(&self) -> Option<u64> {
        self.worker
            .as_ref()
            .and_then(CalculationWorker::started_generation)
    }
    #[cfg(any(test, feature = "native-profile", target_arch = "wasm32"))]
    pub(super) fn submitted_generation(&self) -> u64 {
        self.generation
    }

    pub(super) fn enabled(&self) -> bool {
        self.worker.is_some()
    }
    pub(super) fn reset(&mut self) {
        if let Some(worker) = &self.worker {
            worker.reset();
        }
        self.generation = self.generation.saturating_add(1);
        self.running = false;
        self.source = None;
        self.signature = None;
        self.faults.clear();
        self.preparation = Default::default();
    }
    /// Source can commit before a command discovers an invalid range/editor.
    /// Invalidate old replies immediately without discarding preparation caches.
    pub(super) fn invalidate_reply(&mut self) {
        if let Some(worker) = &self.worker {
            worker.cancel();
        }
        self.generation = self.generation.saturating_add(1);
        self.running = false;
        self.signature = None;
        self.faults.clear();
    }
}

impl WorksheetApp {
    pub(crate) fn enable_background_calculation(&mut self, ctx: egui::Context) {
        if self.background.enabled() {
            return;
        }
        match CalculationWorker::new(move || ctx.request_repaint()) {
            Ok(worker) => {
                // A worker starts without bindings. Keep all displayed values
                // available while its first source is prepared, including when
                // this is enabled after a direct headless calculation.
                self.persist_calculation_cache();
                if let Some(cache) = &self.document.calculation_cache {
                    for row in &cache.rows {
                        self.saved_results
                            .entry(row.region_id)
                            .or_insert_with(|| row.clone());
                    }
                }
                self.background.worker = Some(worker);
            }
            Err(error) => {
                self.status = format!("계산 스레드를 시작할 수 없습니다: {error}");
            }
        }
    }

    pub(super) fn prepare_background_state(&mut self, source: &str) {
        if self.background.source.as_deref() != Some(source) {
            // Dirty status is provisional until the worker's parsed snapshot
            // arrives. Keep payloads, and never present old values as fresh.
            self.mark_regions_pending(
                (self
                    .document
                    .regions
                    .iter()
                    .filter(|r| calculation_enabled(r))
                    .map(|r| r.id))
                .collect::<Vec<_>>(),
            );
        }
    }

    pub(super) fn background_plot_jobs(
        &self,
        command: CalculationCommand,
        through: Option<u64>,
    ) -> Vec<PlotJob> {
        let end = through
            .and_then(|id| self.document.regions.iter().find(|r| r.id == id))
            .map(|r| (r.top, r.left));
        self.document
            .regions
            .iter()
            .filter_map(|region| {
                let CanvasContent::Plot(spec) = &region.content else {
                    return None;
                };
                if region.calculation_disabled {
                    return None;
                }
                let in_range = matches!(
                    command,
                    CalculationCommand::Worksheet | CalculationCommand::Automatic
                ) || end.is_some_and(|(top, left)| {
                    region.top < top || (region.top == top && region.left <= left)
                });
                let force = matches!(command, CalculationCommand::Worksheet)
                    || (matches!(command, CalculationCommand::Now)
                        && self.editing_region == Some(region.id)
                        && self.visible_regions.contains(&region.id));
                (in_range && (force || self.pending_regions.contains(&region.id))).then(|| {
                    PlotJob {
                        id: region.id,
                        left: region.left,
                        top: region.top,
                        spec: spec.clone(),
                    }
                })
            })
            .collect()
    }

    pub(super) fn dispatch_background(
        &mut self,
        input: CalculationInput,
        requests: Vec<CalculationRequest>,
        plots: Vec<PlotJob>,
        command: CalculationCommand,
    ) {
        let signature = input.signature.clone();
        // Text-only edits do not replace an explicit calculation already in
        // flight. Plot, disabled-region and math changes still invalidate it.
        if requests.is_empty()
            && plots.is_empty()
            && self.background.source.as_deref() == Some(input.source.as_ref())
            && self.background.signature.as_deref() == Some(&signature)
        {
            return;
        }
        for request in &requests {
            let position = match request {
                CalculationRequest::ActiveRegion(id) => self
                    .document
                    .regions
                    .iter()
                    .find(|r| r.id.to_string() == *id || input.areas.owner(id) == Some(r.id))
                    .map(|r| (r.top, r.left)),
                _ => None,
            };
            if matches!(request, CalculationRequest::Worksheet) || position.is_some() {
                self.mark_regions_pending(
                    (self
                        .document
                        .regions
                        .iter()
                        .filter(|r| calculation_enabled(r))
                        .filter(|r| position.is_none_or(|p| (r.top, r.left) >= p))
                        .map(|r| r.id))
                    .collect::<Vec<_>>(),
                );
            }
        }
        self.background.generation = self.background.generation.saturating_add(1);
        self.background.source = Some(input.source.clone());
        self.background.signature = Some(signature);
        self.background.faults = input.faults;
        self.background.whole = matches!(command, CalculationCommand::Worksheet);
        self.background.running = true;
        self.calculation_pending = !self.pending_regions.is_empty();
        let job = CalculationJob {
            strings_use_origin: input.strings_use_origin,
            generation: self.background.generation,
            automatic: matches!(command, CalculationCommand::Automatic)
                && !requests
                    .iter()
                    .any(|r| matches!(r, CalculationRequest::ActiveRegion(_))),
            source: input.source,
            grid: None,
            requests,
            plots,
        };
        if let Some(worker) = &self.background.worker
            && let Err(error) = worker.submit(job)
        {
            self.background.running = false;
            self.calculation_error = Some(error);
        }
    }

    pub(super) fn poll_background_calculation(&mut self) {
        while let Some(worker) = &self.background.worker {
            match worker.try_recv() {
                Ok(reply) => self.accept_background_reply(reply),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.background.running = false;
                    self.calculation_error = Some("계산 스레드가 종료되었습니다.".into());
                    break;
                }
            }
        }
        if self.background.running
            && self
                .background
                .worker
                .as_ref()
                .is_some_and(CalculationWorker::is_finished)
        {
            self.background.running = false;
            self.calculation_error = Some("계산 스레드가 종료되었습니다.".into());
        }
    }

    fn accept_background_reply(&mut self, reply: CalculationReply) {
        if reply.generation != self.background.generation {
            return;
        }
        #[cfg(target_arch = "wasm32")]
        if !reply.updates.is_empty() || !reply.plots.is_empty() {
            self.host.changed();
        }
        self.background.running = false;
        self.last_executed_regions.clear();
        for update in reply.updates {
            self.accept_calculation_update(update);
        }
        for plot in reply.plots {
            if self.document.regions.iter().any(|r| {
                r.id == plot.id
                    && !r.calculation_disabled
                    && matches!(r.content, CanvasContent::Plot(_))
            }) {
                self.plot_results.insert(plot.id, plot.result);
                if let Some(source) = self
                    .document
                    .regions
                    .iter()
                    .find(|region| region.id == plot.id)
                    .and_then(|region| region.source_region.as_ref())
                {
                    Arc::make_mut(&mut self.native_visuals)
                        .regions
                        .remove(source);
                }
                self.pending_regions.remove(&plot.id);
                self.saved_results.remove(&plot.id);
                self.last_executed_regions.push(plot.id.to_string());
            }
        }
        let faults = std::mem::take(&mut self.background.faults);
        self.apply_calculation_faults(faults, self.background.whole);
        self.calculation_error = reply.error;
        if reply.cancelled {
            self.status = "계산 중지 · 수동 계산".into();
        }
        self.calculation_pending = !self.pending_regions.is_empty();
    }

    pub(super) fn stop_all_calculations(&mut self) {
        if !self.background.running {
            return;
        }
        self.automatic_calculation = false;
        if let Some(worker) = &self.background.worker {
            worker.cancel();
        }
        self.status = "계산 중지 요청 · 수동 계산".into();
    }

    pub(super) fn pause_automatic_calculations(&self) {
        if let Some(worker) = &self.background.worker {
            worker.pause_automatic();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inline_plot_commit_rejects_old_reply_even_when_now_returns_before_dispatch() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut app = WorksheetApp {
            automatic_calculation: false,
            ..Default::default()
        };
        let id = app.document.add_plot(20., 20.);
        app.recalculate();
        app.persist_calculation_cache();
        app.restore_saved_calculations();
        let points = app.plot_results[&id].clone();
        app.enable_background_calculation(context.clone());
        let generation = app.background.generation;
        app.selected = Some(id);
        app.focus_new = Some(id);
        app.insertion = None;
        super::super::tests::frame(&mut app, &context, vec![]);
        super::super::tests::frame(&mut app, &context, vec![]);
        super::super::tests::frame(
            &mut app,
            &context,
            vec![
                egui::Event::Key {
                    key: egui::Key::A,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::COMMAND,
                },
                egui::Event::Text("2*x".into()),
            ],
        );
        assert_eq!(app.background.generation, generation);
        app.visible_calculation_end = None;
        app.calculate_command(CalculationCommand::Now);
        assert!(app.background.generation > generation);
        assert!(app.pending_regions.contains(&id));
        app.accept_background_reply(CalculationReply {
            generation,
            updates: vec![],
            plots: vec![crate::calculation_worker::PlotReply {
                id,
                result: points.clone(),
            }],
            error: Some("stale plot reply".into()),
            cancelled: false,
        });
        assert!(app.pending_regions.contains(&id) && app.last_executed_regions.is_empty());
        super::super::tests::assert_pending_curves_unchanged(&app.plot_results[&id], &points);
        assert!(app.calculation_error.is_none());
        app.persist_calculation_cache();
        assert_eq!(
            app.document.saved_calculation_cache().unwrap().rows[0].status,
            SavedCalculationStatus::Pending
        );
    }
    #[test]
    fn draft_keystrokes_submit_no_worker_jobs_and_enter_submits_one() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut app = WorksheetApp::default();
        app.enable_background_calculation(context.clone());
        let generation = app.background.generation;
        for text in ["a", ":", "2", "3", "4"] {
            super::super::tests::frame(&mut app, &context, vec![egui::Event::Text(text.into())]);
            assert_eq!(app.background.generation, generation);
            assert!(!app.background.running);
        }
        super::super::tests::frame(&mut app, &context, vec![super::super::tests::enter()]);
        assert_eq!(app.background.generation, generation + 1);
        settle(&mut app);
        assert!(app.uncommitted_math.is_empty());
        assert!(!app.calculation_pending);
    }
    fn gated_app() -> (
        WorksheetApp,
        std::sync::mpsc::Receiver<()>,
        std::sync::mpsc::Sender<()>,
    ) {
        use std::sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
            mpsc,
        };
        let (entered, waiting) = mpsc::channel();
        let (release, released) = mpsc::channel();
        let first = AtomicBool::new(true);
        let released = Arc::new(Mutex::new(released));
        let worker = CalculationWorker::new(move || {
            if first.swap(false, Ordering::SeqCst) {
                entered.send(()).unwrap();
                released
                    .lock()
                    .unwrap()
                    .recv_timeout(std::time::Duration::from_secs(5))
                    .unwrap();
            }
        })
        .unwrap();
        let mut app = WorksheetApp::default();
        app.background.worker = Some(worker);
        (app, waiting, release)
    }

    #[test]
    fn auto_off_holds_queued_automatic_work_but_preserves_explicit_calculation() {
        for explicit in [false, true] {
            let (mut app, waiting, release) = gated_app();
            let id = app.document.add_math(0., 0.);
            source(&mut app, id, "2+3=");
            app.recalculate();
            waiting
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap();
            app.poll_background_calculation();
            source(&mut app, id, "9+2=");
            app.calculation_changed(false);
            if explicit {
                app.recalculate();
            }
            app.ribbon_command(
                &egui::Context::default(),
                RibbonAction::SetAutomaticCalculation(false),
            );
            release.send(()).unwrap();
            settle(&mut app);
            assert_eq!(app.pending_regions.contains(&id), !explicit);
            assert_eq!(
                app.results[&id].value.as_ref().unwrap().numeric_json()["real"],
                if explicit { 11. } else { 5. }
            );
            assert!(app.calculation_error.is_none());
        }
    }

    #[test]
    fn explicit_equals_is_not_discarded_when_automatic_calculation_is_turned_off() {
        let (mut app, waiting, release) = gated_app();
        let id = app.document.add_math(0., 0.);
        source(&mut app, id, "2+3=");
        app.recalculate();
        waiting
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        app.poll_background_calculation();
        source(&mut app, id, "9+2=");
        app.editing_region = Some(id);
        app.stage_math_edit(id, true);
        app.ribbon_command(
            &egui::Context::default(),
            RibbonAction::SetAutomaticCalculation(false),
        );
        release.send(()).unwrap();
        settle(&mut app);
        assert!(!app.pending_regions.contains(&id));
        assert_eq!(
            app.results[&id].value.as_ref().unwrap().numeric_json()["real"],
            11.
        );
    }

    #[test]
    fn text_delete_duplicate_and_move_do_not_replace_a_manual_full_calculation() {
        let (mut app, waiting, release) = gated_app();
        let id = app.document.add_math(0., 0.);
        source(&mut app, id, "2+3=");
        app.recalculate();
        waiting
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        app.poll_background_calculation();
        app.automatic_calculation = false;
        source(&mut app, id, "9+2=");
        app.recalculate();
        let generation = app.background.generation;
        let text = app.document.add_text(0., 200., "note");
        app.selected = Some(text);
        app.nudge(1., 1.);
        assert_eq!(app.background.generation, generation);
        app.duplicate_selected();
        assert_eq!(app.background.generation, generation);
        app.remove_selected();
        assert_eq!(app.background.generation, generation);
        release.send(()).unwrap();
        settle(&mut app);
        assert!(!app.pending_regions.contains(&id));
        assert!(app.last_executed_regions.contains(&id.to_string()));
        assert_eq!(
            app.results[&id].value.as_ref().unwrap().numeric_json()["real"],
            11.
        );
    }

    fn settle(app: &mut WorksheetApp) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while app.background.running {
            app.poll_background_calculation();
            assert!(
                std::time::Instant::now() < deadline,
                "worker did not settle"
            );
            std::thread::yield_now();
        }
    }
    fn source(app: &mut WorksheetApp, id: u64, text: &str) {
        let mut editor = Editor::new();
        formula::input_text(&mut editor, text);
        app.editors.remove(&id);
        app.document
            .regions
            .iter_mut()
            .find(|r| r.id == id)
            .unwrap()
            .content = CanvasContent::Math(editor.to_portable().unwrap());
    }
    #[test]
    fn background_calculation_keeps_latest_edit_and_manual_prepare_does_not_execute() {
        let mut app = WorksheetApp::default();
        let id = app.document.add_math(0., 0.);
        source(&mut app, id, "2+3=");
        app.enable_background_calculation(egui::Context::default());
        app.recalculate();
        source(&mut app, id, "9+2=");
        app.calculation_changed(false);
        settle(&mut app);
        assert_eq!(
            app.results[&id].value.as_ref().unwrap().numeric_json()["real"],
            11.
        );
        app.automatic_calculation = false;
        source(&mut app, id, "1+3=");
        app.calculation_changed(false);
        settle(&mut app);
        assert!(app.pending_regions.contains(&id));
        assert!(app.last_executed_regions.is_empty());
        assert_eq!(
            app.results[&id].value.as_ref().unwrap().numeric_json()["real"],
            11.
        );
        app.recalculate();
        settle(&mut app);
        assert_eq!(
            app.results[&id].value.as_ref().unwrap().numeric_json()["real"],
            4.
        );
    }
    #[test]
    fn visible_active_definition_refreshes_clean_plot_in_the_same_requested_range() {
        let mut app = WorksheetApp::default();
        let id = app.document.add_math(0., 0.);
        source(&mut app, id, "a:=2");
        let plot = app.document.add_plot(0., 100.);
        let CanvasContent::Plot(spec) = &mut app.document.regions[1].content else {
            unreachable!()
        };
        *spec.primary_y_expression_mut().unwrap() = "a*x".into();
        app.enable_background_calculation(egui::Context::default());
        app.recalculate();
        settle(&mut app);
        app.editing_region = Some(id);
        app.visible_regions.extend([id, plot]);
        app.visible_calculation_end = Some(plot);
        app.calculate_command(CalculationCommand::Now);
        settle(&mut app);
        assert!(app.last_executed_regions.contains(&plot.to_string()));
        assert!(!app.pending_regions.contains(&plot));
        assert!(app.plot_results[&plot].is_ok());
        app.editing_region = None;
        app.calculate_command(CalculationCommand::Now);
        assert!(!app.background.running);
        assert!(app.last_executed_regions.is_empty());
    }

    #[test]
    fn stop_moves_to_manual_keeps_old_value_and_auto_on_resumes_queued_source() {
        use std::sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
            mpsc,
        };
        let (entered, waiting) = mpsc::channel();
        let (release, released) = mpsc::channel();
        let released = Arc::new(Mutex::new(released));
        let first = Arc::new(AtomicBool::new(true));
        let worker = CalculationWorker::new(move || {
            if first.swap(false, Ordering::SeqCst) {
                entered.send(()).unwrap();
                released
                    .lock()
                    .unwrap()
                    .recv_timeout(std::time::Duration::from_secs(5))
                    .unwrap();
            }
        })
        .unwrap();
        let mut app = WorksheetApp::default();
        app.background.worker = Some(worker);
        let id = app.document.add_math(0., 0.);
        source(&mut app, id, "2+3=");
        app.recalculate();
        waiting
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        app.poll_background_calculation();
        assert_eq!(
            app.results[&id].value.as_ref().unwrap().numeric_json()["real"],
            5.
        );
        source(&mut app, id, "9+2=");
        app.calculation_changed(false);
        assert!(app.background.running);
        app.stop_all_calculations();
        assert!(!app.automatic_calculation);
        release.send(()).unwrap();
        settle(&mut app);
        assert!(app.pending_regions.contains(&id));
        assert!(app.calculation_error.is_none());
        assert_eq!(
            app.results[&id].value.as_ref().unwrap().numeric_json()["real"],
            5.
        );
        app.ribbon_command(
            &egui::Context::default(),
            RibbonAction::SetAutomaticCalculation(true),
        );
        settle(&mut app);
        assert!(app.automatic_calculation);
        assert!(!app.pending_regions.contains(&id));
        assert_eq!(
            app.results[&id].value.as_ref().unwrap().numeric_json()["real"],
            11.
        );
    }

    #[test]
    fn new_document_resets_worker_and_stale_reply_cannot_replace_new_results() {
        let mut app = WorksheetApp::default();
        let id = app.document.add_math(0., 0.);
        source(&mut app, id, "2=");
        app.enable_background_calculation(egui::Context::default());
        app.recalculate();
        let old = app.background.generation;
        app.apply(Pending::New);
        let id = app.document.add_math(0., 0.);
        source(&mut app, id, "7=");
        app.recalculate();
        settle(&mut app);
        assert!(app.background.generation > old);
        app.accept_background_reply(CalculationReply {
            generation: old,
            updates: vec![],
            plots: vec![],
            error: Some("stale".into()),
            cancelled: false,
        });
        assert!(app.calculation_error.is_none());
        assert_eq!(
            app.results[&id].value.as_ref().unwrap().numeric_json()["real"],
            7.
        );
    }
}
