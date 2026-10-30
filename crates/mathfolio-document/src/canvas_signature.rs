//! Exact JSON signature framing. Values retain serde_json's implementation;
//! only fixed CanvasRegion/PortableTree object and array delimiters are emitted.
use super::{CanvasContent, CanvasDocument, CanvasRegion};
use mathfolio_editor::PortableTree;
use std::io::Write;
fn bytes(output: &mut impl Write, value: &[u8]) -> serde_json::Result<()> {
    output.write_all(value).map_err(serde_json::Error::io)
}
fn tree_json(tree: &PortableTree, output: &mut impl Write) -> serde_json::Result<()> {
    // Exhaustive destructuring makes new tree fields a compilation failure.
    let PortableTree { kind, children } = tree;
    bytes(output, b"{\"kind\":")?;
    serde_json::to_writer(&mut *output, kind)?;
    if children.is_empty() {
        return bytes(output, b",\"children\":[]}");
    }
    bytes(output, b",\"children\":[")?;
    for (index, child) in children.iter().enumerate() {
        if index != 0 {
            bytes(output, b",")?;
        }
        tree_json(child, output)?;
    }
    bytes(output, b"]}")
}
/// Byte-identical to the derived region JSON. This is signature framing, not
/// another file format. Update independent serde/golden oracles for serde
/// rename/skip-attribute changes; new fields fail the exhaustive pattern below.
pub fn write_region_signature_json(
    region: &CanvasRegion,
    output: &mut impl Write,
) -> serde_json::Result<()> {
    let CanvasRegion {
        id,
        left,
        top,
        content,
        source_region,
        rich_text,
        authored_text,
        format,
        calculation_disabled,
        calculation_enabled_override,
    } = region;
    let CanvasContent::Math(tree) = content else {
        return serde_json::to_writer(output, region);
    };
    bytes(output, b"{\"id\":")?;
    serde_json::to_writer(&mut *output, id)?;
    bytes(output, b",\"left\":")?;
    serde_json::to_writer(&mut *output, left)?;
    bytes(output, b",\"top\":")?;
    serde_json::to_writer(&mut *output, top)?;
    bytes(output, b",\"content\":{\"Math\":")?;
    tree_json(tree, output)?;
    bytes(output, b"},\"source_region\":")?;
    serde_json::to_writer(&mut *output, source_region)?;
    if let Some(wire) = rich_text {
        bytes(output, b",\"rich_text\":")?;
        serde_json::to_writer(&mut *output, wire)?;
    }
    if let Some(wire) = authored_text {
        bytes(output, b",\"authored_text\":")?;
        serde_json::to_writer(&mut *output, wire)?;
    }
    bytes(output, b",\"format\":")?;
    serde_json::to_writer(&mut *output, format)?;
    if *calculation_disabled {
        bytes(output, b",\"calculation_disabled\":true")?;
    }
    if *calculation_enabled_override {
        bytes(output, b",\"calculation_enabled_override\":true")?;
    }
    bytes(output, b"}")
}
/// Exact presentation-v1 tuple bytes without a collected region-reference Vec.
/// All fields and arrays keep source/serde order; filtering changes no region.
pub fn write_calculation_signature_json(
    document: &CanvasDocument,
    active_only: bool,
    output: &mut impl Write,
) -> serde_json::Result<()> {
    bytes(output, b"[[")?;
    let mut first = true;
    for region in document.regions.iter().filter(|r| {
        !matches!(r.content, CanvasContent::Text(_)) && (!active_only || !r.calculation_disabled)
    }) {
        if !first {
            bytes(output, b",")?;
        }
        first = false;
        write_region_signature_json(region, output)?;
    }
    bytes(output, b"],")?;
    serde_json::to_writer(&mut *output, &document.width)?;
    bytes(output, b",")?;
    serde_json::to_writer(&mut *output, &document.height)?;
    bytes(output, b",")?;
    serde_json::to_writer(&mut *output, &document.grid)?;
    bytes(output, b"]")
}

/// Stream through a bounded buffer: feeding every serde token directly to SHA
/// costs more than the JSON itself. This allocation is transient, not a cache.
pub(super) fn source_signature(document: &CanvasDocument, active_only: bool) -> String {
    use sha2::{Digest, Sha256};
    struct HashWriter<'a>(&'a mut Sha256);
    impl Write for HashWriter<'_> {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.update(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut hasher = Sha256::new();
    // Persisted cache domain: renaming it would discard valid pre-0.1.1 results.
    hasher.update(b"mathcraft-calculation-presentation-v1\0");
    {
        let mut output = std::io::BufWriter::with_capacity(64 * 1024, HashWriter(&mut hasher));
        write_calculation_signature_json(document, active_only, &mut output)
            .expect("canvas calculation source is serializable");
        output.flush().expect("infallible signature writer");
    }
    if let Some(archive) = &document.native_archive {
        hasher.update(b"\0native\0");
        hasher.update(archive);
    }
    format!("{:x}", hasher.finalize())
}
