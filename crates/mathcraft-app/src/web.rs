//! Versioned WASM surface; all worksheet authoring remains the native Rust App.
use crate::{worksheet_ui::WorksheetApp, *};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::prelude::*;

fn error(value: impl AsRef<str>) -> JsValue {
    js_sys::Error::new(value.as_ref()).into()
}

struct Surface(Rc<RefCell<WorksheetApp>>);
impl eframe::App for Surface {
    fn raw_input_hook(&mut self, ctx: &egui::Context, input: &mut egui::RawInput) {
        if self.0.borrow().host_is_active() {
            self.0.borrow_mut().raw_input_hook(ctx, input);
        } else {
            input.events.clear();
        }
    }
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        if self.0.borrow().host_is_active() {
            self.0.borrow_mut().ui(ui, frame);
        }
    }
}

#[wasm_bindgen]
pub struct MathcraftSession {
    app: Rc<RefCell<WorksheetApp>>,
    runner: eframe::WebRunner,
    fonts: egui::FontDefinitions,
    mounted: bool,
    disposed: bool,
}
#[wasm_bindgen]
impl MathcraftSession {
    #[wasm_bindgen(constructor)]
    pub fn new(protocol: u32) -> Result<MathcraftSession, JsValue> {
        if protocol != 1 {
            return Err(error("Unsupported SDK protocol; expected 1"));
        }
        Ok(Self {
            app: Rc::new(RefCell::new(WorksheetApp::default())),
            runner: eframe::WebRunner::new(),
            fonts: egui::FontDefinitions::default(),
            mounted: false,
            disposed: false,
        })
    }
    pub async fn mount(
        &mut self,
        canvas: web_sys::HtmlCanvasElement,
        worker_url: String,
    ) -> Result<(), JsValue> {
        self.check()?;
        if self.mounted {
            return Err(error("Already mounted"));
        }
        if worker_url.is_empty() {
            return Err(error("Worker URL is required"));
        }
        let app = self.app.clone();
        let fonts = self.fonts.clone();
        self.runner
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(move |cc| {
                    configure_surface(&cc.egui_ctx, fonts.clone(), Theme::Light);
                    app.borrow_mut().attach_host(cc.egui_ctx.clone(), fonts);
                    crate::calculation_worker::web_worker::set_url(worker_url);
                    app.borrow_mut()
                        .enable_background_calculation(cc.egui_ctx.clone());
                    Ok(Box::new(Surface(app)))
                }),
            )
            .await?;
        self.mounted = true;
        Ok(())
    }
    #[wasm_bindgen(js_name = provideFont)]
    pub fn provide_font(&mut self, name: String, bytes: &[u8]) -> Result<(), JsValue> {
        self.check()?;
        if self.mounted {
            return Err(error("Supply fonts before mount"));
        }
        if bytes.len() > 24 * 1024 * 1024 || name.is_empty() || skrifa::FontRef::new(bytes).is_err()
        {
            return Err(error("Invalid font"));
        }
        self.fonts.font_data.insert(
            name.clone(),
            egui::FontData::from_owned(bytes.to_vec()).into(),
        );
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            self.fonts
                .families
                .entry(family)
                .or_default()
                .insert(0, name.clone());
        }
        Ok(())
    }
    #[wasm_bindgen(js_name = loadDocument)]
    pub fn load_document(&self, name: &str, bytes: &[u8], discard: bool) -> Result<(), JsValue> {
        self.check()?;
        self.app
            .borrow_mut()
            .load_bytes(name, bytes, discard)
            .map_err(error)
    }
    pub fn state(&self) -> Result<JsValue, JsValue> {
        self.check()?;
        serde_wasm_bindgen::to_value(&self.app.borrow().host_state())
            .map_err(|e| error(e.to_string()))
    }
    #[wasm_bindgen(js_name = takeRequests)]
    pub fn take_requests(&self) -> Result<JsValue, JsValue> {
        self.check()?;
        serde_wasm_bindgen::to_value(&self.app.borrow_mut().take_host_requests())
            .map_err(|e| error(e.to_string()))
    }
    pub fn command(&self, command: &str) -> Result<(), JsValue> {
        self.check()?;
        self.app.borrow_mut().host_command(command).map_err(error)
    }
    #[wasm_bindgen(js_name = exportDocument)]
    pub fn export_document(&self, format: &str) -> Result<Vec<u8>, JsValue> {
        self.check()?;
        self.app.borrow_mut().export_bytes(format).map_err(error)
    }
    #[wasm_bindgen(js_name = prepareSave)]
    pub fn prepare_save(&self, name: &str) -> Result<JsValue, JsValue> {
        self.check()?;
        let (id, revision, bytes) = self
            .app
            .borrow_mut()
            .prepare_host_save(name)
            .map_err(error)?;
        let packet = js_sys::Object::new();
        js_sys::Reflect::set(&packet, &"id".into(), &JsValue::from_f64(id as f64))?;
        js_sys::Reflect::set(
            &packet,
            &"revision".into(),
            &JsValue::from_f64(revision as f64),
        )?;
        js_sys::Reflect::set(
            &packet,
            &"bytes".into(),
            &js_sys::Uint8Array::from(bytes.as_slice()),
        )?;
        Ok(packet.into())
    }
    #[wasm_bindgen(js_name = acknowledgeSave)]
    pub fn acknowledge_save(&self, id: u32, success: bool) -> Result<bool, JsValue> {
        self.check()?;
        self.app
            .borrow_mut()
            .acknowledge_host_save(u64::from(id), success)
            .map_err(error)
    }
    #[wasm_bindgen(js_name = setTheme)]
    pub fn set_theme(&self, theme: JsValue) -> Result<bool, JsValue> {
        self.check()?;
        let tokens: Option<HostTheme> = serde_wasm_bindgen::from_value(theme)?;
        let ctx = self.app.borrow().host_context().map_err(error)?;
        crate::set_host_theme(&ctx, tokens).map_err(error)
    }
    #[wasm_bindgen(js_name = setThemeMode)]
    pub fn set_theme_mode(&self, mode: JsValue) -> Result<(), JsValue> {
        self.check()?;
        let mode: ThemeMode = serde_wasm_bindgen::from_value(mode)?;
        let ctx = self.app.borrow().host_context().map_err(error)?;
        crate::set_theme_mode(&ctx, mode);
        Ok(())
    }
    #[wasm_bindgen(js_name = setActive)]
    pub fn set_active(&self, active: bool) -> Result<(), JsValue> {
        self.check()?;
        self.app.borrow_mut().set_host_active(active);
        Ok(())
    }
    #[wasm_bindgen(js_name = inspectDocument)]
    pub fn inspect_document(&self) -> Result<String, JsValue> {
        self.check()?;
        self.app.borrow().inspect_document().map_err(error)
    }
    #[wasm_bindgen(js_name = inspectResults)]
    pub fn inspect_results(&self) -> Result<JsValue, JsValue> {
        self.check()?;
        self.app.borrow().inspect_results().map_err(error)
    }
    #[wasm_bindgen(js_name = exportPdf)]
    pub fn export_pdf(&self) -> Result<Vec<u8>, JsValue> {
        self.check()?;
        self.app.borrow_mut().host_pdf().map_err(error)
    }
    pub fn dispose(&mut self) {
        if !self.disposed {
            self.runner.destroy();
            self.app.borrow_mut().host_dispose();
            self.disposed = true;
            self.mounted = false;
        }
    }
    #[wasm_bindgen(js_name = memoryBytes)]
    pub fn memory_bytes(&self) -> Result<f64, JsValue> {
        self.check()?;
        let memory: js_sys::WebAssembly::Memory = wasm_bindgen::memory().unchecked_into();
        Ok(js_sys::Uint8Array::new(&memory.buffer()).length() as f64)
    }
}
impl MathcraftSession {
    fn check(&self) -> Result<(), JsValue> {
        if self.disposed {
            Err(error("Session disposed"))
        } else {
            Ok(())
        }
    }
}
impl Drop for MathcraftSession {
    fn drop(&mut self) {
        self.dispose();
    }
}
