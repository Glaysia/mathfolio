//! Version 2 native Mathcraft canvas documents.
//!
//! This is currently a flat editable canvas. Nested Mathcad regions, sections,
//! relationships and unsupported content remain available in `native_archive`;
//! saving this format does not export edits back to Mathcad's package format.
//! Portable trees contain semantic content, never editor caret or undo history.
use mathcraft_editor::{NodeKind, PortableTree};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

pub const CANVAS_VERSION: u32 = 2;
pub const MAX_NATIVE_ARCHIVE_BYTES: usize = 16 * 1024 * 1024;
const MAX_DOCUMENT_BYTES: u64 = 128 * 1024 * 1024;
const MAX_REGIONS: usize = 4096;
const MAX_TEXT_BYTES: usize = 8 * 1024 * 1024;
const MAX_TREE_NODES_TOTAL: usize = 65_536;
// Keeps the serialized tree within serde_json's bounded nesting parser.
const MAX_CANVAS_TREE_DEPTH: usize = 48;
const MAX_DIMENSION: f64 = 1_000_000_000.0;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanvasDocument {
    pub version: u32,
    pub title: String,
    pub width: f64,
    pub height: f64,
    pub grid: f64,
    pub next_id: u64,
    pub regions: Vec<CanvasRegion>,
    pub native_archive: Option<Vec<u8>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanvasRegion {
    pub id: u64,
    pub left: f64,
    pub top: f64,
    pub content: CanvasContent,
    pub source_region: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum CanvasContent {
    Math(PortableTree),
    Text(String),
    Opaque { xml: String, kind: String },
}

impl Default for CanvasDocument {
    fn default() -> Self {
        Self {
            version: CANVAS_VERSION,
            title: "Untitled worksheet".into(),
            width: 21. * 96. / 2.54,
            height: 29.7 * 96. / 2.54,
            grid: 0.5 * 96. / 2.54,
            next_id: 1,
            regions: Vec::new(),
            native_archive: None,
        }
    }
}

impl CanvasDocument {
    /// Adds a blank structural expression. Invalid caller-supplied coordinates
    /// remain invalid until corrected; validate/save will reject them.
    pub fn add_math(&mut self, left: f64, top: f64) -> u64 {
        self.add_region(
            left,
            top,
            CanvasContent::Math(PortableTree {
                kind: NodeKind::Placeholder,
                children: Vec::new(),
            }),
        )
    }

    pub fn add_text(&mut self, left: f64, top: f64, text: impl Into<String>) -> u64 {
        self.add_region(left, top, CanvasContent::Text(text.into()))
    }

    fn add_region(&mut self, left: f64, top: f64, content: CanvasContent) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        self.regions.push(CanvasRegion {
            id,
            left: snap_coordinate(left, self.grid).unwrap_or(left),
            top: snap_coordinate(top, self.grid).unwrap_or(top),
            content,
            source_region: None,
        });
        id
    }

    /// Snaps both coordinates before mutating the region, so an invalid move
    /// never leaves half an update behind. Vector order is not evaluation order.
    pub fn move_region(&mut self, id: u64, left: f64, top: f64) -> Result<(), String> {
        let left = snap_coordinate(left, self.grid)?;
        let top = snap_coordinate(top, self.grid)?;
        let region = self
            .regions
            .iter_mut()
            .find(|r| r.id == id)
            .ok_or_else(|| format!("Unknown canvas region {id}"))?;
        region.left = left;
        region.top = top;
        Ok(())
    }

    /// Loads only Mathcraft v2. Mathcad import is a separate source-preserving
    /// conversion path; this method never opens external package relationships.
    pub fn load(path: &Path) -> Result<Self, String> {
        check_extension(path)?;
        let mut file = File::open(path).map_err(|e| e.to_string())?;
        if file.metadata().map_err(|e| e.to_string())?.len() > MAX_DOCUMENT_BYTES {
            return Err("Canvas document exceeds byte limit".into());
        }
        let mut bytes = Vec::new();
        Read::by_ref(&mut file)
            .take(MAX_DOCUMENT_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_DOCUMENT_BYTES {
            return Err("Canvas document exceeds byte limit".into());
        }
        let doc: Self = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        doc.validate()?;
        Ok(doc)
    }

    /// Validates and writes a new sibling temporary file, then atomically
    /// replaces the destination. Failed validation/write/rename preserves the
    /// old destination; an incomplete temporary file is removed on failure.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        check_extension(path)?;
        self.validate()?;
        // Compact JSON bounds the overhead of the preserved byte array.
        let bytes = serde_json::to_vec(self).map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_DOCUMENT_BYTES {
            return Err("Serialized canvas document exceeds byte limit".into());
        }
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let name = path
            .file_name()
            .ok_or("Canvas output file name is missing")?;
        static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos();
        let mut temp_name = name.to_os_string();
        temp_name.push(format!(
            ".{}-{stamp}-{}.tmp",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        let temp = parent.join(temp_name);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        let written = file.write_all(&bytes).and_then(|_| file.sync_all());
        drop(file);
        if let Err(error) = written {
            let _ = std::fs::remove_file(&temp);
            return Err(error.to_string());
        }
        if let Err(error) = std::fs::rename(&temp, path) {
            let _ = std::fs::remove_file(&temp);
            return Err(error.to_string());
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.version != CANVAS_VERSION {
            return Err(format!(
                "Unsupported canvas document version {}",
                self.version
            ));
        }
        if !self.width.is_finite()
            || !self.height.is_finite()
            || !(1.0..=MAX_DIMENSION).contains(&self.width)
            || !(1.0..=MAX_DIMENSION).contains(&self.height)
        {
            return Err("Canvas dimensions must be finite positive sizes within limits".into());
        }
        if !self.grid.is_finite() || self.grid <= 0.0 || self.grid > MAX_DIMENSION {
            return Err("Canvas grid must be finite and positive within limits".into());
        }
        if self.title.len() > 1024 || self.regions.len() > MAX_REGIONS {
            return Err("Canvas title or region count exceeds limits".into());
        }
        if self
            .native_archive
            .as_ref()
            .is_some_and(|b| b.len() > MAX_NATIVE_ARCHIVE_BYTES)
        {
            return Err("Preserved native package exceeds 16 MiB".into());
        }
        let mut ids = HashSet::new();
        let mut text_bytes = self.title.len();
        let mut tree_nodes = 0usize;
        for region in &self.regions {
            if region.id == 0 || region.id >= self.next_id || !ids.insert(region.id) {
                return Err("Canvas region IDs must be unique, nonzero and below next_id".into());
            }
            check_coordinate(region.left)?;
            check_coordinate(region.top)?;
            if let Some(source) = &region.source_region {
                if source.len() > 1024 {
                    return Err("Source region reference exceeds byte limit".into());
                }
                text_bytes = text_bytes.saturating_add(source.len());
            }
            match &region.content {
                CanvasContent::Math(tree) => {
                    tree.validate()
                        .map_err(|e| format!("Region {}: {e}", region.id))?;
                    let mut stack = vec![(tree, 1)];
                    while let Some((node, depth)) = stack.pop() {
                        tree_nodes += 1;
                        if tree_nodes > MAX_TREE_NODES_TOTAL || depth > MAX_CANVAS_TREE_DEPTH {
                            return Err("Canvas expression complexity exceeds limits".into());
                        }
                        text_bytes = text_bytes.saturating_add(match &node.kind {
                            NodeKind::Identifier(s)
                            | NodeKind::Number(s)
                            | NodeKind::Imaginary(s) => s.len(),
                            NodeKind::AnnotatedIdentifier { name, labels } => {
                                name.len().saturating_add(labels.len())
                            }
                            NodeKind::NativeIdentifier {
                                name,
                                labels,
                                subscript,
                                ..
                            } => name
                                .len()
                                .saturating_add(labels.len())
                                .saturating_add(subscript.as_ref().map_or(0, String::len)),
                            NodeKind::ImaginarySymbol { value, symbol } => {
                                value.len().saturating_add(symbol.len())
                            }
                            _ => 0,
                        });
                        stack.extend(node.children.iter().map(|n| (n, depth + 1)));
                    }
                }
                CanvasContent::Text(text) => {
                    if text.len() > 1024 * 1024 {
                        return Err("Canvas text region exceeds 1 MiB".into());
                    }
                    text_bytes = text_bytes.saturating_add(text.len());
                }
                CanvasContent::Opaque { xml, kind } => {
                    if kind.is_empty() || kind.len() > 128 || xml.len() > MAX_TEXT_BYTES {
                        return Err("Opaque region type or XML exceeds limits".into());
                    }
                    text_bytes = text_bytes
                        .saturating_add(xml.len())
                        .saturating_add(kind.len());
                }
            }
            if text_bytes > MAX_TEXT_BYTES {
                return Err("Canvas text exceeds total 8 MiB budget".into());
            }
        }
        if self.next_id == 0 {
            return Err("Canvas next_id must be nonzero".into());
        }
        Ok(())
    }
}

fn check_extension(path: &Path) -> Result<(), String> {
    if path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("mathcraft"))
    {
        Ok(())
    } else {
        Err("Canvas documents require the .mathcraft extension".into())
    }
}

fn check_coordinate(value: f64) -> Result<(), String> {
    if value.is_finite() && value.abs() <= MAX_DIMENSION {
        Ok(())
    } else {
        Err("Canvas coordinates must be finite and within limits".into())
    }
}

/// Project canvas policy: nearest grid point, midpoint ties towards the smaller
/// coordinate. Negative coordinates use that same policy; this does not claim
/// to reproduce Mathcad's negative-coordinate clipping or restricted areas.
pub fn snap_coordinate(value: f64, grid: f64) -> Result<f64, String> {
    check_coordinate(value)?;
    if !grid.is_finite() || grid <= 0.0 || grid > MAX_DIMENSION {
        return Err("Canvas grid must be finite and positive within limits".into());
    }
    let remainder = value.rem_euclid(grid);
    let offset = if remainder > grid / 2.0 {
        grid - remainder
    } else {
        -remainder
    };
    let snapped = value + offset;
    check_coordinate(snapped)?;
    Ok(snapped)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestDir(std::path::PathBuf);
    impl TestDir {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "mathcraft-canvas-{}-{stamp}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn path(&self, name: &str) -> std::path::PathBuf {
            self.0.join(name)
        }
    }
    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn atom(kind: NodeKind) -> PortableTree {
        PortableTree {
            kind,
            children: Vec::new(),
        }
    }

    #[test]
    fn geometry_tree_opaque_content_and_native_bytes_survive_save_load() {
        let dir = TestDir::new();
        let path = dir.path("canvas.mathcraft");
        let mut doc = CanvasDocument::default();
        assert!(doc.regions.is_empty());
        let math = doc.add_math(21.0, 58.0);
        let text = doc.add_text(300.0, 97.0, "한글 메모");
        doc.regions[0].content = CanvasContent::Math(PortableTree {
            kind: NodeKind::Fraction,
            children: vec![
                atom(NodeKind::AnnotatedIdentifier {
                    name: "ωᵣ".into(),
                    labels: "VARIABLE".into(),
                }),
                atom(NodeKind::Number("2".into())),
            ],
        });
        doc.regions[0].source_region = Some("/mathcad/worksheet.xml#region=42".into());
        doc.regions.push(CanvasRegion {
            id: doc.next_id,
            left: 59.25,
            top: 100.125,
            content: CanvasContent::Opaque {
                xml: "<future xmlns='urn:unknown' flag=' untouched '/>".into(),
                kind: "plot3d".into(),
            },
            source_region: Some("native:88".into()),
        });
        doc.next_id += 1;
        doc.native_archive = Some(vec![b'P', b'K', 3, 4, 0, 255, 17, 0]);
        doc.move_region(math, 61.0, 139.0).unwrap();
        doc.move_region(text, 253.0, 178.0).unwrap();
        doc.save(&path).unwrap();
        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(!saved.contains("undo"));
        assert!(!saved.contains("caret"));
        assert!(!saved.contains("arena"));
        assert_eq!(CanvasDocument::load(&path).unwrap(), doc);
        let next = doc.add_math(0.0, 0.0);
        assert!(next > math && next > text);
        doc.title = "Updated".into();
        doc.save(&path).unwrap();
        assert_eq!(CanvasDocument::load(&path).unwrap(), doc);
    }

    #[test]
    fn midpoint_snaps_down_and_failed_move_is_atomic() {
        assert_eq!(snap_coordinate(10.0, 4.0).unwrap(), 8.0);
        assert_eq!(snap_coordinate(10.1, 4.0).unwrap(), 12.0);
        assert_eq!(snap_coordinate(-10.0, 4.0).unwrap(), -12.0);
        let mut doc = CanvasDocument::default();
        let id = doc.add_math(19.2, 38.4);
        let before = doc.clone();
        assert!(doc.move_region(id, 200.0, f64::NAN).is_err());
        assert_eq!(doc, before);
        assert!(doc.move_region(999, 20.0, 20.0).is_err());
        assert_eq!(doc, before);
    }

    #[test]
    fn invalid_save_does_not_replace_previous_document() {
        let dir = TestDir::new();
        let path = dir.path("safe.mathcraft");
        let mut doc = CanvasDocument::default();
        doc.add_text(0.0, 0.0, "original");
        doc.save(&path).unwrap();
        let before = std::fs::read(&path).unwrap();
        doc.regions[0].top = f64::INFINITY;
        assert!(doc.save(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert_eq!(std::fs::read_dir(&dir.0).unwrap().count(), 1);
        assert!(doc.save(&dir.path("original.mcdx")).is_err());
    }

    #[test]
    fn rename_failure_removes_only_temporary_output() {
        let dir = TestDir::new();
        let destination = dir.path("occupied.mathcraft");
        std::fs::create_dir(&destination).unwrap();
        std::fs::write(destination.join("keep"), b"unchanged").unwrap();
        assert!(CanvasDocument::default().save(&destination).is_err());
        assert_eq!(
            std::fs::read(destination.join("keep")).unwrap(),
            b"unchanged"
        );
        assert_eq!(std::fs::read_dir(&dir.0).unwrap().count(), 1);
    }

    #[test]
    fn rejects_wrong_version_duplicate_ids_invalid_trees_and_oversized_payloads() {
        let mut doc = CanvasDocument::default();
        doc.add_math(0.0, 0.0);
        doc.regions.push(doc.regions[0].clone());
        assert!(doc.validate().is_err());
        doc.regions.pop();
        doc.regions[0].content = CanvasContent::Math(PortableTree {
            kind: NodeKind::Fraction,
            children: Vec::new(),
        });
        assert!(doc.validate().is_err());
        doc.regions[0].content = CanvasContent::Text("x".repeat(1024 * 1024 + 1));
        assert!(doc.validate().is_err());
        doc.regions.clear();
        doc.version = 1;
        assert!(doc.validate().is_err());
        doc.version = 2;
        doc.native_archive = Some(vec![0; MAX_NATIVE_ARCHIVE_BYTES + 1]);
        assert!(doc.validate().is_err());
        doc.native_archive = None;
        doc.grid = 0.0;
        assert!(doc.validate().is_err());
    }

    #[test]
    fn supported_deep_tree_roundtrips_and_unknown_serialized_state_is_rejected() {
        let dir = TestDir::new();
        let path = dir.path("tree.mathcraft");
        let mut tree = atom(NodeKind::Placeholder);
        for _ in 1..MAX_CANVAS_TREE_DEPTH {
            tree = PortableTree {
                kind: NodeKind::Group,
                children: vec![tree],
            };
        }
        let mut doc = CanvasDocument::default();
        doc.add_math(0.0, 0.0);
        doc.regions[0].content = CanvasContent::Math(tree);
        doc.save(&path).unwrap();
        assert_eq!(CanvasDocument::load(&path).unwrap(), doc);
        let mut json = serde_json::to_value(&doc).unwrap();
        json["undo"] = serde_json::json!([]);
        std::fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
        assert!(CanvasDocument::load(&path).is_err());
    }
}
