//! Append to the worksheet_ui probe child or register as its submodule.
//! Async entry: use the previously reviewed std Wake deadline executor.
use eframe::{
    egui_wgpu::{RendererOptions, WgpuSetup},
    wgpu,
};

pub(super) async fn initialize_primary_only() -> Result<
    (
        wgpu::Instance,
        wgpu::Device,
        wgpu::Queue,
        RendererOptions,
        serde_json::Value,
    ),
    String,
> {
    let native = eframe::NativeOptions::default();
    let mut renderer = RendererOptions {
        msaa_samples: native.multisampling as _,
        depth_stencil_format: eframe::egui_wgpu::depth_format_from_bits(
            native.depth_buffer,
            native.stencil_buffer,
        ),
        dithering: native.dithering,
        ..Default::default()
    };
    // Opt-in unread offscreen comparison only. No production renderer setting
    // is changed here; record the exact experimental option in the receipt.
    let dithering_override = match std::env::var("MATHCRAFT_GRAPHICS_DITHERING") {
        Ok(value) if value == "on" => Some(true),
        Ok(value) if value == "off" => Some(false),
        Ok(_) => return Err("Dithering diagnostic accepts only on/off".into()),
        Err(std::env::VarError::NotPresent) => None,
        Err(error) => return Err(format!("Dithering diagnostic variable: {error}")),
    };
    if let Some(value) = dithering_override {
        renderer.dithering = value;
    }
    let surface = serde_json::json!({"configured_present_mode":format!("{:?}",native.wgpu_options.surface.present_mode),
        "configured_maximum_frame_latency":native.wgpu_options.surface.desired_maximum_frame_latency,
        "surface_created":false,"present_called":false});
    let WgpuSetup::CreateNew(mut setup) = native.wgpu_options.wgpu_setup else {
        return Err("Existing GPU setup unsupported".into());
    };
    if setup.native_adapter_selector.is_some() {
        return Err("Custom selector not admitted".into());
    }
    let configured = setup.instance_descriptor.backends;
    let probed = configured & (wgpu::Backends::DX12 | wgpu::Backends::VULKAN);
    if probed.is_empty() {
        return Err("No primary backend allowed; GL remains forbidden".into());
    }
    setup.instance_descriptor.backends = probed;
    setup.instance_descriptor.display = None;
    setup.display_handle = None;
    // Receipt only, captured before the descriptor is consumed. No override of
    // validation/debug/compiler options and no additional backend/device call.
    let flags = setup.instance_descriptor.flags;
    let build_flags = wgpu::InstanceFlags::from_build_config();
    let instance_configuration = serde_json::json!({
        "app_cfg_debug_assertions":cfg!(debug_assertions),
        "app_cfg_test":cfg!(test),
        "requested_instance_flags":format!("{flags:?}"),
        "requested_instance_flags_bits":flags.bits(),
        "requested_debug":flags.contains(wgpu::InstanceFlags::DEBUG),
        "requested_validation":flags.contains(wgpu::InstanceFlags::VALIDATION),
        "requested_gpu_based_validation":flags.contains(wgpu::InstanceFlags::GPU_BASED_VALIDATION),
        "requested_validation_indirect_call":flags.contains(wgpu::InstanceFlags::VALIDATION_INDIRECT_CALL),
        "wgpu_from_build_config_flags":format!("{build_flags:?}"),
        "wgpu_from_build_config_bits":build_flags.bits(),
        "configured_dx12_compiler_not_observed_actual":format!("{:?}",setup.instance_descriptor.backend_options.dx12.shader_compiler).chars().take(256).collect::<String>(),
        "probe_modifies_flags_or_compiler":false,
        "native_debug_layer_enabled_observed":null,
        "note":"Requested descriptor flags; environment was already folded by egui-wgpu defaults. Not proof a native debug layer/compiler was available."
    });
    let instance = wgpu::Instance::new(setup.instance_descriptor);
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: setup.power_preference,
            compatible_surface: None,
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        })
        .await
        .map_err(|e| format!("Headless adapter request: {e}"))?;
    let info = adapter.get_info();
    if !matches!(info.backend, wgpu::Backend::Dx12 | wgpu::Backend::Vulkan) {
        return Err("Forbidden selected backend".into());
    }
    let descriptor = (setup.device_descriptor)(&adapter);
    let (device, queue) = adapter
        .request_device(&descriptor)
        .await
        .map_err(|e| format!("Headless device request: {e}"))?;
    let report = serde_json::json!({"configured_backends":format!("{configured:?}"),"probed_backends":format!("{probed:?}"),
        "excluded_gl":configured.contains(wgpu::Backends::GL),"adapter":{"name":info.name.chars().take(128).collect::<String>(),"driver":info.driver.chars().take(128).collect::<String>(),"driver_info":info.driver_info.chars().take(128).collect::<String>(),
        "backend":format!("{:?}",info.backend),"device_type":format!("{:?}",info.device_type),"vendor":info.vendor,"device":info.device},
        "surface_defaults_only":surface,"instance_configuration":instance_configuration,"actual_gui_selected":null,"headless_format":"Bgra8Unorm",
        "diagnostic_dithering_override":dithering_override,
        "renderer":{"msaa_samples":renderer.msaa_samples,"depth_stencil":format!("{:?}",renderer.depth_stencil_format),
        "dithering":renderer.dithering,"predictable_texture_filtering":renderer.predictable_texture_filtering}});
    Ok((instance, device, queue, renderer, report))
}
