//! Explicit headless Windows DX12/Vulkan adapter discovery.
//! Hardware/driver discovery only: no window, surface, Device, Queue or submit.
use eframe::{egui_wgpu::WgpuSetup, wgpu};
use serde_json::{Value, json};
use std::{
    future::Future,
    io::Write,
    path::PathBuf,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
    time::{Duration, Instant},
};

struct ThreadWake(std::thread::Thread);
impl Wake for ThreadWake {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}
fn wait<F: Future>(future: F) -> Result<F::Output, &'static str> {
    let waker = Waker::from(Arc::new(ThreadWake(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if Instant::now() >= deadline {
            return Err("future deadline exceeded");
        }
        if let Poll::Ready(output) = future.as_mut().poll(&mut cx) {
            return Ok(output);
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        std::thread::park_timeout(remaining.min(Duration::from_millis(50)));
    }
}
fn bounded(text: &str) -> String {
    text.chars().take(128).collect()
}
fn adapter(info: wgpu::AdapterInfo) -> Value {
    json!({"name":bounded(&info.name),"backend":format!("{:?}",info.backend),
        "device_type":format!("{:?}",info.device_type),"driver":bounded(&info.driver),
        "driver_info":bounded(&info.driver_info),"vendor":info.vendor,"device":info.device,
        "text_truncated":{"name":info.name.chars().count()>128,"driver":info.driver.chars().count()>128,"driver_info":info.driver_info.chars().count()>128},
        "text_limit_chars":128})
}
fn panic_text(payload: Box<dyn std::any::Any + Send>) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| bounded(s))
        .or_else(|| payload.downcast_ref::<String>().map(|s| bounded(s)))
        .unwrap_or_else(|| "non-string native/backend panic".into())
}
fn probe() -> Value {
    // Match main's NativeOptions default. In locked eframe this overrides
    // WgpuConfiguration's own HIGH_THROUGHPUT with LOW_LATENCY.
    let config = eframe::NativeOptions::default().wgpu_options;
    let surface_defaults = json!({"present_mode":format!("{:?}",config.surface.present_mode),
        "desired_maximum_frame_latency":config.surface.desired_maximum_frame_latency,
        "surface_created":false,"actual_gui_configuration_observed":false});
    let WgpuSetup::CreateNew(mut setup) = config.wgpu_setup else {
        return json!({"status":"unexpected-existing-setup","surface_defaults":surface_defaults});
    };
    let configured_backends = setup.instance_descriptor.backends;
    // WGL discovery can create a hidden helper HWND. Exclude GL from the
    // instance itself, not merely from enumeration, to keep this probe windowless.
    let backends = configured_backends & (wgpu::Backends::DX12 | wgpu::Backends::VULKAN);
    setup.instance_descriptor.backends = backends;
    let power = setup.power_preference;
    let settings = json!({"configured_backends":format!("{configured_backends:?}"),"probed_backends":format!("{backends:?}"),"gl_excluded_to_avoid_helper_window":true,"power_preference":format!("{power:?}"),
        "instance_flags":format!("{:?}",setup.instance_descriptor.flags),
        "display_handle":false,"custom_selector":setup.native_adapter_selector.is_some(),
        "force_fallback_adapter":false,"apply_limit_buckets":false});
    let mut report = json!({"status":"initialized","settings":settings,"surface_defaults":surface_defaults,
        "adapters":[],"headless_preferred":null,"actual_gui_selected":null,
        "device_requested":false,"gpu_submitted":false,"original_runtime_executed":false});
    // Instance construction and adapter discovery may initialize user-mode
    // graphics drivers. The Rust future deadline cannot interrupt a blocking
    // native driver call; caller must enforce a process timeout if needed.
    let instance = wgpu::Instance::new(setup.instance_descriptor);
    match wait(instance.enumerate_adapters(backends)) {
        Ok(adapters) => {
            report["adapter_count"] = json!(adapters.len());
            report["adapters_truncated"] = json!(adapters.len() > 64);
            report["adapters"] = Value::Array(
                adapters
                    .iter()
                    .take(64)
                    .map(|a| adapter(a.get_info()))
                    .collect(),
            );
        }
        Err(error) => report["enumeration_error"] = json!(error),
    }
    if std::env::var("MATHCRAFT_GRAPHICS_REQUEST_DEFAULT").as_deref() == Ok("0") {
        report["headless_request_status"] = json!("skipped-by-caller");
    } else {
        match wait(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: power,
            compatible_surface: None,
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        })) {
            Ok(Ok(chosen)) => {
                report["headless_preferred"] = adapter(chosen.get_info());
                report["headless_request_status"] = json!("adapter-only-success");
            }
            Ok(Err(error)) => report["headless_request_error"] = json!(bounded(&error.to_string())),
            Err(error) => report["headless_request_error"] = json!(error),
        }
    }
    report["status"] = json!("completed-adapter-discovery");
    report
}

#[test]
#[ignore = "Explicit Windows hardware/driver discovery only; no surface/window/device/submit"]
fn windows_headless_wgpu_adapter_probe() {
    let output = PathBuf::from(
        std::env::var_os("MATHCRAFT_GRAPHICS_OUTPUT")
            .expect("fresh MATHCRAFT_GRAPHICS_OUTPUT directory required"),
    );
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::create_dir(&output)
        .expect("output must be a fresh directory; existing output is never replaced");
    let mut result = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(probe)) {
        Ok(report) => report,
        Err(panic) => {
            json!({"status":"backend-panic","panic":panic_text(panic),"actual_gui_selected":null,"device_requested":false,"gpu_submitted":false})
        }
    };
    let mut env = serde_json::Map::new();
    for name in [
        "WGPU_BACKEND",
        "WGPU_POWER_PREF",
        "WGPU_VALIDATION",
        "WGPU_DEBUG",
        "WGPU_ALLOW_UNDERLYING_NONCOMPLIANT_ADAPTER",
        "WGPU_GPU_BASED_VALIDATION",
    ] {
        env.insert(
            name.into(),
            std::env::var(name)
                .ok()
                .map(|v| json!(bounded(&v)))
                .unwrap_or(Value::Null),
        );
    }
    result["selection_environment"] = Value::Object(env);
    result["trace_env_present"] = json!(std::env::var_os("WGPU_TRACE").is_some());
    result["schema"] = json!("mathcraft-headless-adapter-discovery-v1");
    result["limits"] = json!({"adapters":64,"string_chars":128,"json_bytes":262144,"future_deadline_seconds":15,"blocking_driver_timeout_guaranteed":false});
    result["interpretation"] = json!(
        "Headless no-surface preference is not proof of GUI adapter selection, hardware acceleration, presentation support or latency."
    );
    let bytes = serde_json::to_vec_pretty(&result).unwrap();
    assert!(bytes.len() <= 262144, "bounded adapter JSON exceeded cap");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output.join("wgpu-adapters.json"))
        .unwrap();
    file.write_all(&bytes).unwrap();
    println!(
        "GRAPHICS_ADAPTER_PROBE status={} adapters={} no_surface=true no_device=true no_submit=true",
        result["status"], result["adapter_count"]
    );
}
