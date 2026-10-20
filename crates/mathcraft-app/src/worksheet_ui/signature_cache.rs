//! Memoize only the serialization of exact region snapshots. The resulting
//! source bytes and SHA-256 remain CanvasDocument's presentation-v1 contract.
use mathcraft_document::canvas::{CanvasContent, CanvasDocument, CanvasRegion};
use mathcraft_editor::{NodeKind, PortableTree};
use sha2::{Digest, Sha256};
use std::io::Write;

#[cfg(test)]
#[path = "signature_profile.rs"]
mod profile;

const MAX_BYTES: usize = 32 * 1024 * 1024;
const MAX_REGIONS: usize = 10_000;

pub(super) struct SignatureCache {
    entries: Vec<Option<Box<Entry>>>,
    bytes: usize,
    budget: usize,
    #[cfg(test)]
    serializations: usize,
}

struct Entry {
    region: CanvasRegion,
    json: Vec<u8>,
    bytes: usize,
}

impl Default for SignatureCache {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            bytes: 0,
            budget: MAX_BYTES,
            #[cfg(test)]
            serializations: 0,
        }
    }
}

struct HashWriter(Sha256);
impl Write for HashWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        #[cfg(test)]
        let _timing = profile::scope(profile::Phase::Hash);
        self.0.update(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl SignatureCache {
    #[cfg(test)]
    pub(super) fn stats(&self) -> (usize, usize) {
        (self.retained_bytes(), self.serializations)
    }

    pub(super) fn signature(&mut self, document: &CanvasDocument) -> String {
        let mut hash = HashWriter(Sha256::new());
        hash.0.update(b"mathcraft-calculation-presentation-v1\0");
        self.write_source(document, &mut hash);
        // Compare/hash every archive byte, including same-sized replacements.
        // No pointer, compressed member identity or stale archive hash is used.
        #[cfg(test)]
        let _archive_timing = profile::scope(profile::Phase::Archive);
        if let Some(archive) = &document.native_archive {
            hash.0.update(b"\0native\0");
            hash.0.update(archive);
        }
        format!("{:x}", hash.0.finalize())
    }

    fn retained_bytes(&self) -> usize {
        self.bytes + self.entries.capacity() * std::mem::size_of::<Option<Box<Entry>>>()
    }

    fn remove(&mut self, index: usize) {
        if let Some(old) = self.entries[index].take() {
            self.bytes -= old.bytes;
        }
    }

    fn write_source(&mut self, document: &CanvasDocument, output: &mut impl Write) {
        #[cfg(test)]
        let manage_timing = profile::scope(profile::Phase::Manage);
        if document.regions.len() > MAX_REGIONS
            || document.regions.len() * std::mem::size_of::<Option<Box<Entry>>>() > self.budget
        {
            self.entries = Vec::new();
            self.bytes = 0;
            let regions: Vec<_> = document.regions.iter().filter(|r| active(r)).collect();
            serde_json::to_writer(
                output,
                &(regions, document.width, document.height, document.grid),
            )
            .expect("canvas calculation source is serializable");
            return;
        }
        // Vector position is only a lookup slot. Full snapshot equality below
        // prevents reordered/new/reused IDs from reusing another region's JSON.
        for index in document.regions.len()..self.entries.len() {
            self.remove(index);
        }
        self.entries.resize_with(document.regions.len(), || None);
        // A previous larger document must not retain an oversized empty index.
        if self.retained_bytes() > self.budget {
            self.entries = Vec::new();
            self.entries.resize_with(document.regions.len(), || None);
            self.bytes = 0;
        }
        if self.retained_bytes() > self.budget {
            self.entries = Vec::new();
            let regions: Vec<_> = document.regions.iter().filter(|r| active(r)).collect();
            serde_json::to_writer(
                output,
                &(regions, document.width, document.height, document.grid),
            )
            .expect("canvas calculation source is serializable");
            return;
        }
        #[cfg(test)]
        drop(manage_timing);
        output
            .write_all(b"[[")
            .expect("infallible signature writer");
        let mut first = true;
        for (index, region) in document.regions.iter().enumerate() {
            if !active(region) {
                self.remove(index);
                continue;
            }
            if !first {
                output.write_all(b",").expect("infallible signature writer");
            }
            first = false;
            if self.entries[index]
                .as_ref()
                .is_some_and(|old| exact(&old.region, region))
            {
                output
                    .write_all(&self.entries[index].as_ref().unwrap().json)
                    .expect("infallible signature writer");
                continue;
            }
            self.remove(index);
            #[cfg(test)]
            {
                self.serializations += 1;
            }
            #[cfg(test)]
            let json_timing = profile::scope(profile::Phase::Json);
            // Keep serde_json::to_vec's initial allocation and the exact cached
            // bytes; only fixed object/tree framing bypasses generic serde.
            let mut json = Vec::with_capacity(128);
            mathcraft_document::canvas::write_region_signature_json(region, &mut json)
                .expect("canvas calculation source is serializable");
            #[cfg(test)]
            drop(json_timing);
            output
                .write_all(&json)
                .expect("infallible signature writer");
            #[cfg(test)]
            let payload_timing = profile::scope(profile::Phase::Payload);
            let bytes = std::mem::size_of::<Entry>() + clone_payload(region) + json.capacity();
            #[cfg(test)]
            drop(payload_timing);
            if !matches!(region.content, CanvasContent::Plot(_))
                && bytes <= self.budget.saturating_sub(self.retained_bytes())
            {
                #[cfg(test)]
                let _clone_timing = profile::scope(profile::Phase::Clone);
                self.bytes += bytes;
                self.entries[index] = Some(Box::new(Entry {
                    region: region.clone(),
                    json,
                    bytes,
                }));
            }
        }
        output
            .write_all(b"],")
            .expect("infallible signature writer");
        #[cfg(test)]
        let _settings_timing = profile::scope(profile::Phase::Settings);
        let settings = serde_json::to_vec(&(document.width, document.height, document.grid))
            .expect("canvas calculation source is serializable");
        output
            .write_all(&settings[1..])
            .expect("infallible signature writer");
    }
}

fn active(region: &CanvasRegion) -> bool {
    !matches!(region.content, CanvasContent::Text(_)) && !region.calculation_disabled
}

fn exact(old: &CanvasRegion, new: &CanvasRegion) -> bool {
    #[cfg(test)]
    let _timing = profile::scope(profile::Phase::Exact);
    // PartialEq alone aliases -0/+0 although JSON preserves their sign. NaN
    // naturally misses the cache and uses serde's existing null conversion.
    // Plot has nested floats; retain ordinary serde rather than duplicating
    // every plot-field comparison and risking an incomplete identity guard.
    !matches!(new.content, CanvasContent::Plot(_))
        && old == new
        && old.left.to_bits() == new.left.to_bits()
        && old.top.to_bits() == new.top.to_bits()
        && old.format.font_points.to_bits() == new.format.font_points.to_bits()
}

/// Clone-owned payload, excluding allocator overhead and the process's RSS.
/// String/Vec clone capacities are their lengths, not the source capacities.
fn clone_payload(region: &CanvasRegion) -> usize {
    region.source_region.as_ref().map_or(0, String::len)
        + match &region.content {
            CanvasContent::Math(tree) => tree_payload(tree),
            CanvasContent::Text(text) => text.len(),
            CanvasContent::Opaque { xml, kind } => xml.len() + kind.len(),
            CanvasContent::Plot(_) => 0, // This variant is never retained.
        }
}

fn tree_payload(tree: &PortableTree) -> usize {
    let atom = match &tree.kind {
        NodeKind::Identifier(text) | NodeKind::Number(text) | NodeKind::Imaginary(text) => {
            text.len()
        }
        NodeKind::AnnotatedIdentifier { name, labels } => name.len() + labels.len(),
        NodeKind::NativeIdentifier {
            name,
            labels,
            subscript,
            ..
        } => name.len() + labels.len() + subscript.as_ref().map_or(0, String::len),
        NodeKind::ImaginarySymbol { value, symbol } => value.len() + symbol.len(),
        _ => 0,
    };
    atom + tree.children.len() * std::mem::size_of::<PortableTree>()
        + tree.children.iter().map(tree_payload).sum::<usize>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mathcraft_document::canvas::{PlotRange, PlotSpec, PlotTraceSpec, ResultFormat};

    fn document(count: usize) -> CanvasDocument {
        let mut doc = CanvasDocument::default();
        for n in 0..count {
            doc.add_math(0., n as f64 * 40.);
            doc.regions.last_mut().unwrap().content = CanvasContent::Math(PortableTree {
                kind: NodeKind::NativeIdentifier {
                    name: format!("幅µ{n}"),
                    labels: "VARIABLE".into(),
                    contextual: Some(false),
                    subscript: Some("_r".into()),
                },
                children: vec![],
            });
        }
        doc
    }

    fn same(cache: &mut SignatureCache, doc: &CanvasDocument) {
        let mut bytes = Vec::new();
        cache.write_source(doc, &mut bytes);
        let regions: Vec<_> = doc
            .regions
            .iter()
            .filter(|r| !matches!(r.content, CanvasContent::Text(_)))
            .filter(|r| !r.calculation_disabled)
            .collect();
        let old_json = serde_json::to_vec(&(regions, doc.width, doc.height, doc.grid)).unwrap();
        assert_eq!(bytes, old_json);
        // Independent pre-optimization Vec/derived-Serialize hash oracle.
        let mut old_hash = Sha256::new();
        old_hash.update(b"mathcraft-calculation-presentation-v1\0");
        old_hash.update(&old_json);
        if let Some(archive) = &doc.native_archive {
            old_hash.update(b"\0native\0");
            old_hash.update(archive);
        }
        let expected = format!("{:x}", old_hash.finalize());
        assert_eq!(cache.signature(doc), expected);
        assert_eq!(doc.active_calculation_signature(), expected);
        assert!(cache.retained_bytes() <= cache.budget);
    }

    #[test]
    fn byte_identity_survives_reorder_ids_metadata_disabled_and_native_archive_changes() {
        let mut doc = document(3);
        let mut cache = SignatureCache::default();
        same(&mut cache, &doc);
        for change in 0..13 {
            match change {
                0 => doc.regions.swap(0, 2),
                1 => doc.regions[0].id = 900,
                2 => doc.regions[0].source_region = Some("µ한글\"\\".into()),
                3 => doc.regions[0].format.result_format = ResultFormat::Scientific,
                4 => doc.regions[0].format.precision = 14,
                5 => doc.regions[0].calculation_disabled = true,
                6 => doc.regions[0].calculation_disabled = false,
                7 => doc.regions[0].calculation_enabled_override = true,
                8 => doc.native_archive = Some(vec![0, 1, 2, 3]),
                9 => doc.native_archive.as_mut().unwrap()[1] = 9,
                10 => {
                    doc.regions[0].content = CanvasContent::Opaque {
                        xml: "<math>µ&amp;한글</math>".into(),
                        kind: "math".into(),
                    }
                }
                11 => {
                    doc.regions.remove(1);
                }
                12 => doc.regions.clear(),
                _ => unreachable!(),
            }
            same(&mut cache, &doc);
        }
    }

    #[test]
    fn signed_zero_and_nonfinite_values_keep_existing_serde_source_contract() {
        let mut doc = document(1);
        let mut cache = SignatureCache::default();
        for value in [-0., 0., f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 12.5] {
            doc.regions[0].left = value;
            same(&mut cache, &doc);
            doc.regions[0].top = value;
            same(&mut cache, &doc);
            doc.regions[0].format.font_points = value as f32;
            same(&mut cache, &doc);
            doc.width = value;
            same(&mut cache, &doc);
            doc.height = value;
            same(&mut cache, &doc);
            doc.grid = value;
            same(&mut cache, &doc);
        }
    }

    #[test]
    fn plot_fallback_tracks_nested_float_fields_without_retaining_a_snapshot() {
        let mut doc = document(1);
        let mut plot = PlotSpec::default();
        plot.legacy_traces_mut()
            .unwrap()
            .push(PlotTraceSpec::default());
        plot.x_axis.range = Some(PlotRange { min: 0., max: 2. });
        doc.regions[0].content = CanvasContent::Plot(plot);
        let mut cache = SignatureCache::default();
        for value in [-0., 0., f64::NAN, f64::INFINITY, 0.5] {
            if let CanvasContent::Plot(plot) = &mut doc.regions[0].content {
                plot.width = value as f32;
                plot.x_axis.range.as_mut().unwrap().min = value;
                plot.legacy_traces_mut().unwrap()[0].style.marker_size = value as f32;
                plot.legacy_traces_mut().unwrap()[0].style.line_width = value as f32;
            }
            same(&mut cache, &doc);
            assert!(cache.entries[0].is_none());
        }
    }

    #[test]
    fn unchanged_snapshots_skip_serialization_but_text_and_disabled_regions_are_pruned() {
        let mut doc = document(3);
        let mut cache = SignatureCache::default();
        same(&mut cache, &doc);
        assert_eq!(cache.serializations, 3);
        same(&mut cache, &doc);
        assert_eq!(cache.serializations, 3);
        doc.regions[0].content = CanvasContent::Math(PortableTree {
            kind: NodeKind::Identifier("changed".into()),
            children: vec![],
        });
        same(&mut cache, &doc);
        assert_eq!(cache.serializations, 4);
        doc.regions[1].calculation_disabled = true;
        doc.regions[2].content = CanvasContent::Text("ignored".into());
        same(&mut cache, &doc);
        assert_eq!(cache.serializations, 4);
        assert!(cache.entries[1].is_none());
        assert!(cache.entries[2].is_none());
        doc.regions.truncate(1);
        same(&mut cache, &doc);
        assert_eq!(cache.entries.len(), 1);
    }

    #[test]
    fn budget_fallback_eviction_and_recovery_never_change_bytes() {
        let mut doc = document(3);
        let mut cache = SignatureCache {
            budget: 1024,
            ..Default::default()
        };
        same(&mut cache, &doc);
        assert!(cache.entries.iter().any(Option::is_none));
        doc.regions[0].content = CanvasContent::Opaque {
            xml: "µ".repeat(4096),
            kind: "math".into(),
        };
        same(&mut cache, &doc);
        assert!(cache.entries[0].is_none());
        doc.regions = document(1).regions;
        same(&mut cache, &doc);
        assert!(cache.entries[0].is_some());
        cache.budget = 1;
        same(&mut cache, &doc);
        assert!(cache.entries.is_empty());
        cache.budget = MAX_BYTES;
        same(&mut cache, &doc);
        assert!(cache.entries[0].is_some());
        doc.regions = document(MAX_REGIONS + 1).regions;
        same(&mut cache, &doc);
        assert!(cache.entries.is_empty());
    }
}
