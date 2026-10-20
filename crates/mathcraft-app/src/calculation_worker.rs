//! Persistent calculator thread. Submission and polling never wait for execution.
use crate::plots::{self, PlotData};
use mathcraft_compat::{
    CalculationControl, CalculationRequest, CalculationUpdate, PlacementGrid, WorksheetCalculator,
};
use mathcraft_document::canvas::PlotSpec;
use std::sync::{
    Arc, Condvar, Mutex,
    mpsc::{self, Receiver, Sender, TryRecvError},
};
use std::thread::{self, JoinHandle};

pub(crate) struct CalculationJob {
    pub strings_use_origin: Option<bool>,
    pub generation: u64,
    pub source: Arc<str>,
    pub grid: Option<PlacementGrid>,
    /// Ordered engine commands: visible active first, then the visible prefix.
    /// Empty requests/plots prepare dirty state without calculating anything.
    pub requests: Vec<CalculationRequest>,
    pub plots: Vec<PlotJob>,
    /// Origin of this submission. Switching to Manual may pause queued automatic
    /// work, while explicit F5/Ctrl+F5 commands remain executable.
    pub automatic: bool,
}

pub(crate) struct PlotJob {
    pub id: u64,
    pub left: f64,
    pub top: f64,
    pub spec: PlotSpec,
}

pub(crate) struct PlotReply {
    pub id: u64,
    pub result: Result<PlotData, String>,
}

pub(crate) struct CalculationReply {
    pub generation: u64,
    /// Only the final snapshot is kept; its executed IDs aggregate every command.
    pub updates: Vec<CalculationUpdate>,
    pub plots: Vec<PlotReply>,
    pub error: Option<String>,
    pub cancelled: bool,
}

struct WorkItem {
    job: CalculationJob,
    control: CalculationControl,
    epoch: u64,
}

#[derive(Default)]
struct QueueState {
    pending: Option<WorkItem>,
    running: Option<CalculationControl>,
    latest_generation: Option<u64>,
    epoch: u64,
    shutdown: bool,
}

#[derive(Default)]
struct Shared {
    state: Mutex<QueueState>,
    changed: Condvar,
}

pub(crate) struct CalculationWorker {
    shared: Arc<Shared>,
    replies: Receiver<CalculationReply>,
    thread: JoinHandle<()>,
}

impl CalculationWorker {
    /// The notification must not block; the desktop caller requests a repaint.
    pub fn new(on_reply: impl Fn() + Send + Sync + 'static) -> std::io::Result<Self> {
        let shared = Arc::new(Shared::default());
        let (send, replies) = mpsc::channel();
        let worker_shared = shared.clone();
        let thread = thread::Builder::new()
            .name("mathcraft-calculation".into())
            .spawn(move || run(worker_shared, send, on_reply))?;
        Ok(Self {
            shared,
            replies,
            thread,
        })
    }

    /// At most one queued source survives; a new source supersedes running work.
    pub fn submit(&self, job: CalculationJob) -> Result<(), String> {
        let mut state = self.shared.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.shutdown {
            return Err("Calculation worker is stopped".into());
        }
        if state
            .latest_generation
            .is_some_and(|generation| job.generation <= generation)
        {
            return Err("Calculation generations must increase".into());
        }
        if let Some(running) = &state.running {
            running.cancel();
        }
        if let Some(pending) = &state.pending {
            pending.control.cancel();
        }
        state.latest_generation = Some(job.generation);
        state.pending = Some(WorkItem {
            job,
            control: CalculationControl::default(),
            epoch: state.epoch,
        });
        self.shared.changed.notify_one();
        Ok(())
    }

    pub fn try_recv(&self) -> Result<CalculationReply, TryRecvError> {
        loop {
            let reply = self.replies.try_recv()?;
            let state = self.shared.state.lock().unwrap_or_else(|e| e.into_inner());
            if state.latest_generation == Some(reply.generation) {
                return Ok(reply);
            }
        }
    }

    /// Cancel execution, retaining preparation of a newer queued source so that
    /// the current generation can report accurate pending rows and old values.
    pub fn cancel(&self) {
        let mut state = self.shared.state.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(running) = &state.running {
            running.cancel();
        }
        if let Some(pending) = &mut state.pending {
            pending.control.cancel();
            pending.job.requests.clear();
            pending.job.plots.clear();
        }
        self.shared.changed.notify_one();
    }

    /// Manual mode pauses only automatic work that has not been taken by the
    /// worker. Running calculations keep their token and explicit commands stay
    /// queued. Source preparation still publishes accurate pending state.
    pub fn pause_automatic(&self) {
        let mut state = self.shared.state.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(pending) = &mut state.pending
            && pending.job.automatic
        {
            pending.job.requests.clear();
            pending.job.plots.clear();
        }
    }

    /// New/opened documents must not inherit an earlier document's environment.
    /// Generation numbers continue to increase across resets.
    pub fn reset(&self) {
        let mut state = self.shared.state.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(running) = &state.running {
            running.cancel();
        }
        if let Some(pending) = state.pending.take() {
            pending.control.cancel();
        }
        state.epoch = state.epoch.wrapping_add(1);
        // Reply sends hold this same lock, so no pre-reset reply can race this drain.
        while self.replies.try_recv().is_ok() {}
        self.shared.changed.notify_one();
    }

    pub fn shutdown(&self) {
        let mut state = self.shared.state.lock().unwrap_or_else(|e| e.into_inner());
        state.shutdown = true;
        if let Some(running) = &state.running {
            running.cancel();
        }
        if let Some(pending) = state.pending.take() {
            pending.control.cancel();
        }
        self.shared.changed.notify_one();
    }

    pub fn is_finished(&self) -> bool {
        self.thread.is_finished()
    }
}

impl Drop for CalculationWorker {
    fn drop(&mut self) {
        self.shutdown();
        // Dropping JoinHandle detaches: closing a window never joins an expensive
        // calculation on its UI thread. Cooperative cancellation releases it.
    }
}

fn run(shared: Arc<Shared>, replies: Sender<CalculationReply>, on_reply: impl Fn()) {
    let mut calculator = WorksheetCalculator::default();
    let mut epoch = 0;
    loop {
        let item = {
            let mut state = shared.state.lock().unwrap_or_else(|e| e.into_inner());
            while state.pending.is_none() && !state.shutdown {
                state = shared
                    .changed
                    .wait(state)
                    .unwrap_or_else(|e| e.into_inner());
            }
            if state.shutdown {
                break;
            }
            let item = state.pending.take().expect("notified pending calculation");
            state.running = Some(item.control.clone());
            item
        };
        if epoch != item.epoch {
            calculator = WorksheetCalculator::default();
            epoch = item.epoch;
        }
        let generation = item.job.generation;
        let reply = item
            .control
            .run(|| execute(&mut calculator, item.job, &item.control));
        let delivered = {
            let mut state = shared.state.lock().unwrap_or_else(|e| e.into_inner());
            state.running = None;
            if state.shutdown {
                break;
            }
            state.epoch == item.epoch
                && state.latest_generation == Some(generation)
                && replies.send(reply).is_ok()
        };
        if delivered {
            on_reply();
        }
    }
}

fn execute(
    calculator: &mut WorksheetCalculator,
    job: CalculationJob,
    control: &CalculationControl,
) -> CalculationReply {
    match job.strings_use_origin {
        Some(value) => {
            calculator.set_strings_use_origin(value);
        }
        None => {
            calculator.set_strings_use_origin_unavailable();
        }
    }
    let mut reply = CalculationReply {
        generation: job.generation,
        updates: vec![],
        plots: vec![],
        error: None,
        cancelled: false,
    };
    let prepared = if control.is_cancelled() {
        // A queued Stop cancels execution, but the newest source still needs its
        // dirty-state preparation. Keep this bounded parse off the UI thread.
        CalculationControl::default().run(|| calculator.update(&job.source, job.grid))
    } else {
        calculator.update(&job.source, job.grid)
    };
    let mut update = match prepared {
        Ok(update) => update,
        Err(error) => {
            reply.cancelled = control.is_cancelled();
            if !reply.cancelled {
                reply.error = Some(error);
            }
            return reply;
        }
    };
    // update owns the parsed source now; the submitted XML is not needed during
    // potentially lengthy evaluation or plot sampling.
    drop(job.source);
    let mut executed = Vec::new();
    for request in job.requests {
        if control.is_cancelled() {
            break;
        }
        match calculator.calculate(request) {
            Ok(mut next) => {
                executed.append(&mut next.executed_region_ids);
                update = next;
                if update.cancelled {
                    break;
                }
            }
            Err(error) => {
                if !control.is_cancelled() {
                    reply.error = Some(error);
                }
                break;
            }
        }
    }
    update.executed_region_ids = executed;
    reply.cancelled = control.is_cancelled() || update.cancelled;
    if !reply.cancelled && reply.error.is_none() {
        for query in job.plots {
            if control.is_cancelled() {
                reply.cancelled = true;
                break;
            }
            let result = calculator
                .context_before(query.left, query.top)
                .and_then(|context| plots::evaluate_in_context(&query.spec, &context));
            if control.is_cancelled() {
                reply.cancelled = true;
                break;
            }
            reply.plots.push(PlotReply {
                id: query.id,
                result,
            });
        }
    }
    update.cancelled |= reply.cancelled;
    reply.updates.push(update);
    reply
}

#[cfg(test)]
mod tests {
    use super::*;
    use mathcraft_compat::{Value, editor_bridge::worksheet_xml};
    use std::time::Duration;

    fn source(value: u32) -> String {
        worksheet_xml(&[
            (1, 0., 0., format!("<ml:define><ml:id labels=\"VARIABLE\">x</ml:id><ml:real>{value}</ml:real></ml:define>")),
            (2, 0., 30., "<ml:eval><ml:id labels=\"VARIABLE\">x</ml:id><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>".into()),
            (3, 0., 60., "<ml:eval><ml:id labels=\"VARIABLE\">x</ml:id><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>".into()),
        ]).unwrap()
    }
    fn job(generation: u64, value: u32, requests: Vec<CalculationRequest>) -> CalculationJob {
        CalculationJob {
            strings_use_origin: Some(false),
            generation,
            source: source(value).into(),
            grid: None,
            requests,
            plots: vec![],
            automatic: false,
        }
    }
    fn receive(worker: &CalculationWorker, notifications: &Receiver<()>) -> CalculationReply {
        notifications.recv_timeout(Duration::from_secs(5)).unwrap();
        worker.try_recv().unwrap()
    }
    fn number(reply: &CalculationReply, index: usize) -> f64 {
        let Value::Scalar { quantity } = reply.updates.last().unwrap().rows[index]
            .value
            .as_ref()
            .unwrap()
        else {
            panic!("expected scalar");
        };
        quantity.value.re
    }
    #[test]
    fn worker_preserves_session_pending_values_and_active_then_prefix_order() {
        let (notify, notifications) = mpsc::channel();
        let worker = CalculationWorker::new(move || {
            let _ = notify.send(());
        })
        .unwrap();
        worker
            .submit(job(1, 2, vec![CalculationRequest::Worksheet]))
            .unwrap();
        let first = receive(&worker, &notifications);
        assert_eq!(number(&first, 2), 2.);
        worker
            .submit(job(
                2,
                2,
                vec![CalculationRequest::ThroughRegion("3".into())],
            ))
            .unwrap();
        let clean = receive(&worker, &notifications);
        assert!(clean.updates[0].executed_region_ids.is_empty());
        worker.submit(job(3, 7, vec![])).unwrap();
        let prepared = receive(&worker, &notifications);
        assert_eq!(prepared.updates[0].pending_region_ids, ["1", "2", "3"]);
        assert!(prepared.updates[0].executed_region_ids.is_empty());
        assert_eq!(number(&prepared, 2), 2.);
        worker
            .submit(job(
                4,
                7,
                vec![
                    CalculationRequest::ActiveRegion("2".into()),
                    CalculationRequest::ThroughRegion("3".into()),
                ],
            ))
            .unwrap();
        let calculated = receive(&worker, &notifications);
        assert_eq!(calculated.updates.len(), 1);
        assert_eq!(calculated.updates[0].executed_region_ids, ["1", "2", "3"]);
        assert_eq!(number(&calculated, 2), 7.);
        worker.reset();
        worker.submit(job(5, 7, vec![])).unwrap();
        let reopened = receive(&worker, &notifications);
        assert!(
            reopened.updates[0]
                .rows
                .iter()
                .all(|row| row.value.is_none())
        );
        worker.shutdown();
        assert!(worker.submit(job(6, 7, vec![])).is_err());
    }

    #[test]
    fn rapid_sources_coalesce_and_stale_replies_cannot_be_consumed() {
        let (notify, notifications) = mpsc::channel();
        let (release, gate) = mpsc::channel();
        let gate = Mutex::new(Some(gate));
        let worker = CalculationWorker::new(move || {
            let _ = notify.send(());
            if let Some(gate) = gate.lock().unwrap().take() {
                gate.recv_timeout(Duration::from_secs(5)).unwrap();
            }
        })
        .unwrap();
        worker
            .submit(job(1, 2, vec![CalculationRequest::Worksheet]))
            .unwrap();
        notifications.recv_timeout(Duration::from_secs(5)).unwrap();
        for generation in 2..=4 {
            worker
                .submit(job(
                    generation,
                    generation as u32,
                    vec![CalculationRequest::Worksheet],
                ))
                .unwrap();
        }
        assert!(
            matches!(worker.try_recv(), Err(TryRecvError::Empty)),
            "The queued first reply is already stale"
        );
        release.send(()).unwrap();
        let reply = receive(&worker, &notifications);
        assert_eq!(reply.generation, 4);
        assert_eq!(number(&reply, 2), 4.);
        assert!(matches!(worker.try_recv(), Err(TryRecvError::Empty)));
        assert!(worker.submit(job(4, 8, vec![])).is_err());
    }

    #[test]
    fn stopping_a_queued_source_prepares_its_pending_state_without_executing_it() {
        let (notify, notifications) = mpsc::channel();
        let (release, gate) = mpsc::channel();
        let gate = Mutex::new(Some(gate));
        let worker = CalculationWorker::new(move || {
            let _ = notify.send(());
            if let Some(gate) = gate.lock().unwrap().take() {
                gate.recv_timeout(Duration::from_secs(5)).unwrap();
            }
        })
        .unwrap();
        worker
            .submit(job(1, 2, vec![CalculationRequest::Worksheet]))
            .unwrap();
        let _ = receive(&worker, &notifications);
        worker
            .submit(job(2, 9, vec![CalculationRequest::Worksheet]))
            .unwrap();
        worker.cancel();
        release.send(()).unwrap();
        let cancelled = receive(&worker, &notifications);
        assert!(cancelled.cancelled);
        assert!(cancelled.updates[0].executed_region_ids.is_empty());
        assert_eq!(cancelled.updates[0].pending_region_ids, ["1", "2", "3"]);
        assert_eq!(number(&cancelled, 2), 2.);
        worker
            .submit(job(
                3,
                9,
                vec![CalculationRequest::ThroughRegion("3".into())],
            ))
            .unwrap();
        let resumed = receive(&worker, &notifications);
        assert!(!resumed.cancelled);
        assert_eq!(number(&resumed, 2), 9.);
    }

    #[test]
    fn plots_run_in_the_computed_context_on_the_same_worker() {
        let (notify, notifications) = mpsc::channel();
        let worker = CalculationWorker::new(move || {
            let _ = notify.send(());
        })
        .unwrap();
        let mut task = job(1, 2, vec![CalculationRequest::Worksheet]);
        let spec = PlotSpec {
            source: mathcraft_document::canvas::PlotSource::single("t", "x*t"),
            ..Default::default()
        };
        task.plots.push(PlotJob {
            id: 10,
            left: 0.,
            top: 100.,
            spec,
        });
        worker.submit(task).unwrap();
        let reply = receive(&worker, &notifications);
        assert_eq!(reply.plots[0].id, 10);
        let data = reply.plots[0].result.as_ref().unwrap();
        assert!(data.first_curve().unwrap().points.len() >= 50);
        assert!(
            data.first_curve()
                .unwrap()
                .points
                .iter()
                .all(|[x, y]| (y - 2. * x).abs() < 1e-8)
        );
    }

    #[test]
    fn shutdown_and_drop_do_not_join_a_worker_waiting_to_notify() {
        let (notify, notifications) = mpsc::channel();
        let (release, gate) = mpsc::channel();
        let gate = Mutex::new(gate);
        let worker = CalculationWorker::new(move || {
            let _ = notify.send(());
            gate.lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(5))
                .unwrap();
        })
        .unwrap();
        worker
            .submit(job(1, 2, vec![CalculationRequest::Worksheet]))
            .unwrap();
        let _ = receive(&worker, &notifications);
        worker.shutdown();
        assert!(
            !worker.is_finished(),
            "The test notification deliberately holds the worker"
        );
        drop(worker);
        // Reaching this send proves closing did not synchronously join the worker.
        release.send(()).unwrap();
    }

    #[test]
    fn invalid_source_reports_an_error_and_the_next_source_can_still_calculate() {
        let (notify, notifications) = mpsc::channel();
        let worker = CalculationWorker::new(move || {
            let _ = notify.send(());
        })
        .unwrap();
        let mut invalid = job(1, 2, vec![CalculationRequest::Worksheet]);
        invalid.source = "<broken>".into();
        worker.submit(invalid).unwrap();
        let failed = receive(&worker, &notifications);
        assert!(failed.error.is_some());
        assert!(failed.updates.is_empty());
        assert!(!failed.cancelled);
        worker
            .submit(job(2, 7, vec![CalculationRequest::Worksheet]))
            .unwrap();
        let recovered = receive(&worker, &notifications);
        assert!(recovered.error.is_none());
        assert_eq!(number(&recovered, 2), 7.);
    }

    #[test]
    fn manual_mode_pauses_queued_automatic_work_but_preserves_explicit_calculation_commands() {
        for (automatic, requests) in [
            (true, vec![CalculationRequest::ThroughRegion("3".into())]),
            (
                false,
                vec![
                    CalculationRequest::ActiveRegion("2".into()),
                    CalculationRequest::ThroughRegion("3".into()),
                ],
            ),
            (false, vec![CalculationRequest::Worksheet]),
        ] {
            let (notify, notifications) = mpsc::channel();
            let (release, gate) = mpsc::channel();
            let gate = Mutex::new(Some(gate));
            let worker = CalculationWorker::new(move || {
                let _ = notify.send(());
                if let Some(gate) = gate.lock().unwrap().take() {
                    gate.recv_timeout(Duration::from_secs(5)).unwrap();
                }
            })
            .unwrap();
            worker
                .submit(job(1, 2, vec![CalculationRequest::Worksheet]))
                .unwrap();
            let _ = receive(&worker, &notifications);
            // Hold the worker after its first result so the second job is
            // certainly queued, without depending on scheduler timing.
            let mut queued = job(2, 9, requests);
            queued.automatic = automatic;
            queued.plots.push(PlotJob {
                id: 10,
                left: 0.,
                top: 100.,
                spec: PlotSpec {
                    source: mathcraft_document::canvas::PlotSource::single("t", "x*t"),
                    ..Default::default()
                },
            });
            worker.submit(queued).unwrap();
            worker.pause_automatic();
            release.send(()).unwrap();
            let reply = receive(&worker, &notifications);
            assert!(!reply.cancelled);
            assert!(reply.error.is_none());
            if automatic {
                assert!(reply.updates[0].executed_region_ids.is_empty());
                assert_eq!(reply.updates[0].pending_region_ids, ["1", "2", "3"]);
                assert_eq!(number(&reply, 2), 2.);
                assert!(reply.plots.is_empty());
            } else {
                assert_eq!(reply.updates[0].executed_region_ids, ["1", "2", "3"]);
                assert!(reply.updates[0].pending_region_ids.is_empty());
                assert_eq!(number(&reply, 2), 9.);
                let data = reply.plots[0].result.as_ref().unwrap();
                assert!(
                    data.first_curve()
                        .unwrap()
                        .points
                        .iter()
                        .all(|[x, y]| (y - 9. * x).abs() < 1e-8)
                );
            }
        }
    }
}
