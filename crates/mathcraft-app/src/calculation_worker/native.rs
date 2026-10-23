//! Native thread transport; shared execution semantics live in the parent.
use super::*;
use std::sync::{
    Condvar, Mutex,
    mpsc::{self, Receiver, Sender, TryRecvError},
};
use std::thread::{self, JoinHandle};
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
