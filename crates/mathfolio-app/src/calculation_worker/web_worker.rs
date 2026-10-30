//! Browser transport. Killing a busy worker provides cancellation without SAB.
use super::*;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::mpsc::{self, Receiver, TryRecvError},
};
use wasm_bindgen::{JsCast, prelude::*};
use web_sys::{ErrorEvent, MessageEvent, Worker, WorkerOptions, WorkerType};

thread_local! { static URL: RefCell<String> = const { RefCell::new(String::new()) }; }
pub(crate) fn set_url(url: String) {
    URL.with(|value| *value.borrow_mut() = url);
}

struct Transport {
    worker: Worker,
    _message: Closure<dyn FnMut(MessageEvent)>,
    _error: Closure<dyn FnMut(ErrorEvent)>,
}
impl Drop for Transport {
    fn drop(&mut self) {
        self.worker.set_onmessage(None);
        self.worker.set_onerror(None);
        self.worker.terminate();
    }
}
pub(crate) struct CalculationWorker {
    url: String,
    transport: RefCell<Option<Transport>>,
    sender: mpsc::Sender<CalculationReply>,
    replies: Receiver<CalculationReply>,
    notify: Arc<dyn Fn() + Send + Sync>,
    generation: Rc<Cell<u64>>,
    busy: Rc<Cell<bool>>,
    failed: Rc<Cell<bool>>,
    previous: RefCell<Vec<mathfolio_compat::RegionEvaluation>>,
    memory: Rc<Cell<u32>>,
    started: Rc<Cell<u64>>,
}
impl CalculationWorker {
    pub fn new(notify: impl Fn() + Send + Sync + 'static) -> std::io::Result<Self> {
        let (sender, replies) = mpsc::channel();
        let value = Self {
            url: URL.with(|v| v.borrow().clone()),
            transport: RefCell::new(None),
            sender,
            replies,
            notify: Arc::new(notify),
            generation: Rc::default(),
            busy: Rc::default(),
            failed: Rc::default(),
            previous: RefCell::default(),
            memory: Rc::default(),
            started: Rc::default(),
        };
        // Saved worksheets can open without calculating. Start the browser
        // worker only when a real job is submitted, not for every mounted view.
        Ok(value)
    }
    fn start(&self) -> Result<(), String> {
        let options = WorkerOptions::new();
        options.set_type(WorkerType::Module);
        let worker =
            Worker::new_with_options(&self.url, &options).map_err(|e| format!("Worker: {e:?}"))?;
        let send = self.sender.clone();
        let notify = self.notify.clone();
        let generation = self.generation.clone();
        let busy = self.busy.clone();
        let memory = self.memory.clone();
        let started = self.started.clone();
        let message = Closure::new(move |event: MessageEvent| {
            if js_sys::Reflect::get(&event.data(), &"kind".into())
                .ok()
                .and_then(|v| v.as_string())
                .as_deref()
                == Some("started")
            {
                if let Some(job) = js_sys::Reflect::get(&event.data(), &"generation".into())
                    .ok()
                    .and_then(|v| v.as_f64())
                    && job as u64 == generation.get()
                {
                    started.set(job as u64);
                    notify();
                }
                return;
            }
            if let Ok(size) = js_sys::Reflect::get(&event.data(), &"worker_memory_bytes".into())
                && let Some(size) = size.as_f64()
            {
                memory.set(size as u32);
            }
            let reply: Result<CalculationReply, _> = serde_wasm_bindgen::from_value(event.data());
            let reply = reply.unwrap_or_else(|e| CalculationReply {
                generation: generation.get(),
                updates: vec![],
                plots: vec![],
                error: Some(format!("Worker protocol: {e}")),
                cancelled: false,
            });
            if reply.generation == generation.get() {
                busy.set(false);
                let _ = send.send(reply);
                notify();
            }
        });
        worker.set_onmessage(Some(message.as_ref().unchecked_ref()));
        let failed = self.failed.clone();
        let notify = self.notify.clone();
        let error = Closure::new(move |event: ErrorEvent| {
            event.prevent_default();
            failed.set(true);
            notify();
        });
        worker.set_onerror(Some(error.as_ref().unchecked_ref()));
        self.failed.set(false);
        *self.transport.borrow_mut() = Some(Transport {
            worker,
            _message: message,
            _error: error,
        });
        Ok(())
    }
    pub fn submit(&self, job: CalculationJob) -> Result<(), String> {
        if job.generation <= self.generation.get() {
            return Err("Calculation generations must increase".into());
        }
        if self.busy.get() {
            self.transport.borrow_mut().take();
        }
        let fresh = self.transport.borrow().is_none();
        if fresh {
            self.start()?;
        }
        self.generation.set(job.generation);
        let data =
            serde_wasm_bindgen::to_value(&(job, fresh.then(|| self.previous.borrow().clone())))
                .map_err(|e| e.to_string())?;
        self.transport
            .borrow()
            .as_ref()
            .ok_or("Worker stopped")?
            .worker
            .post_message(&data)
            .map_err(|e| format!("Worker submit: {e:?}"))?;
        self.busy.set(true);
        Ok(())
    }
    pub fn try_recv(&self) -> Result<CalculationReply, TryRecvError> {
        loop {
            let reply = self.replies.try_recv()?;
            if reply.generation == self.generation.get() {
                if let Some(update) = reply.updates.last() {
                    *self.previous.borrow_mut() = update.rows.clone();
                }
                return Ok(reply);
            }
        }
    }
    pub fn cancel(&self) {
        if !self.busy.replace(false) {
            return;
        }
        self.transport.borrow_mut().take();
        let _ = self.sender.send(CalculationReply {
            generation: self.generation.get(),
            updates: vec![],
            plots: vec![],
            error: None,
            cancelled: true,
        });
        (self.notify)();
    }
    // Browser work begins immediately; there is no not-yet-taken automatic queue.
    pub fn pause_automatic(&self) {}
    pub fn reset(&self) {
        self.transport.borrow_mut().take();
        self.busy.set(false);
        self.previous.borrow_mut().clear();
        while self.replies.try_recv().is_ok() {}
    }
    pub fn is_finished(&self) -> bool {
        self.failed.get()
    }
    pub fn memory_bytes(&self) -> Option<u32> {
        (self.transport.borrow().is_some() && self.memory.get() > 0).then(|| self.memory.get())
    }
    pub fn started_generation(&self) -> Option<u64> {
        (self.transport.borrow().is_some() && self.started.get() > 0).then(|| self.started.get())
    }
    pub fn count(&self) -> u32 {
        u32::from(self.transport.borrow().is_some())
    }
}

#[wasm_bindgen]
pub struct CalculatorWorker {
    calculator: WorksheetCalculator,
}
#[wasm_bindgen]
impl CalculatorWorker {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            calculator: WorksheetCalculator::default(),
        }
    }
    pub fn run(&mut self, input: JsValue) -> Result<JsValue, JsValue> {
        let (job, previous): (
            CalculationJob,
            Option<Vec<mathfolio_compat::RegionEvaluation>>,
        ) = serde_wasm_bindgen::from_value(input)?;
        if let Some(previous) = previous {
            self.calculator
                .update(&job.source, job.grid)
                .map_err(|e| JsValue::from_str(&e))?;
            self.calculator.restore_display_rows(&previous);
        }
        let control = CalculationControl::default();
        let reply = control.run(|| execute(&mut self.calculator, job, &control));
        let result = serde_wasm_bindgen::to_value(&reply)?;
        let memory: js_sys::WebAssembly::Memory = wasm_bindgen::memory().unchecked_into();
        js_sys::Reflect::set(
            &result,
            &"worker_memory_bytes".into(),
            &JsValue::from_f64(js_sys::Uint8Array::new(&memory.buffer()).length() as f64),
        )?;
        Ok(result)
    }
}
