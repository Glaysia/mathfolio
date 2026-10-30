# Mathfolio patch

Based on the crates.io egui-wgpu0.36.2 source. Original files retain the upstream
MIT OR Apache-2.0 licenses. The Mathfolio-specific `src/software.rs` and
`src/software_tests.rs` additions are AGPL-3.0-only.

`WgpuConfiguration.windows_cpu_fallback` defaults to false. Mathfolio opts in
for its opaque, mesh-only worksheet. If the selected Windows adapter is Cpu,
and the window does not require transparency/depth/MSAA, the existing eframe
Painter sends the same egui primitives and managed texture deltas to the CPU
rasterizer and a softbuffer native pixel surface. It does not configure a DXGI
swapchain on that window. Hardware adapters use the original WGPU path.

The eframe/winit input, IME, repaint, viewport-command and close handling remain
in place. The opt-in path supports managed 2D textures, rather than GPU callbacks
or externally registered GPU user textures. Mathfolio currently uses neither of
those GPU-only facilities. Captures come from the presented CPU pixel buffer.
Per-window surfaces are removed along with their viewport.

The rasterizer uses direct rendering rather than upstream's primitive hash cache,
to avoid hash collisions or stale source/image pixels. Diagnostic timing data is
enabled only when the opt-in native profile requests it.
