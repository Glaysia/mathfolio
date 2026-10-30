#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    mathfolio_ui::run_native()
}

#[cfg(target_arch = "wasm32")]
fn main() {}
