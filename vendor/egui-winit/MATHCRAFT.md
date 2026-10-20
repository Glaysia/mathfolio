# Local egui-winit 0.36.2 patch

Source: the pinned crates.io egui-winit 0.36.2 archive. `UPSTREAM.json` records
the archive hash and original manifest/source hashes. Upstream licenses are
included unchanged from tag `0.36.2` in <https://github.com/emilk/egui>.

Mathcraft changes (2026-10-16):

- `src/lib.rs`: the C branch of `is_copy_command` requires no Shift/Alt.
  Ctrl+Shift+C must reach the equation editor as its original Key event.
  Inferring that key from a generic Copy event would also misinterpret the
  Windows Ctrl+Shift+Insert command or a dedicated Copy key.
- Two CPU-only unit tests cover the predicate and preserved clipboard branches.
  No window or native input is created.
- `Cargo.toml`: license include paths point to the copied local licenses.

All other source files remain byte-identical to the registry archive. The
upstream version and public API are unchanged. This is a retained source patch,
not an additional runtime library or a Mathcad resource.
