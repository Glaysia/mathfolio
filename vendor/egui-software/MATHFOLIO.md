# Mathfolio port

Source: DGriffin91/egui_software_backend, commit
`c70e02d3d95c75a82733deb97e516b13a6c86dc6` (see UPSTREAM.json).
The original rasterizer and constify macro retain MIT OR Apache-2.0 licensing.

The rasterizer is ported from egui0.34 to the locked0.36.2 texture delta batches.
It processes every upload in order, updates the cached UV-zero texel and sampler
options after partial uploads, and renders large clipped paper meshes through the
fixed-size direct output buffer. The direct mode no longer drops meshes merely
because their un-clipped bounds exceed8192 pixels.

Only the rasterizer is used. Window/input handling remains in the existing
eframe host; the separate upstream winit host and test-render dependencies are
omitted. Mathfolio uses direct SIMD rendering without the primitive hash cache.
The original input geometry, UVs, clipping, vertex colors and physical pixel
scale are retained. Printing/PDF uses the separate vector exporter.
