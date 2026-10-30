//! Bounded requested result views, separate from numeric/source authority.
use crate::canvas::{CanvasContent, CanvasRegion};
use serde::{Deserialize, Deserializer, Serialize, de};
use std::collections::{HashMap, HashSet};

pub const MAX_MATRIX_RESULT_VIEWS: usize = 4096;
pub const MAX_MATRIX_VIEW_PATH: usize = 64;
pub const MAX_MATRIX_VIEW_CELLS: u32 = 65_536;
const MAX_PATH_INDEX: usize = 250_000;
const MAX_OFFSET: u32 = i32::MAX as u32;

fn present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

fn path<'de, D>(deserializer: D) -> Result<Option<Vec<usize>>, D::Error>
where
    D: Deserializer<'de>,
{
    struct PathVisitor;
    impl<'de> de::Visitor<'de> for PathVisitor {
        type Value = Vec<usize>;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("a bounded nonempty Area child path")
        }
        fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
            let mut result = Vec::new();
            while let Some(index) = seq.next_element::<usize>()? {
                if result.len() >= MAX_MATRIX_VIEW_PATH || index > MAX_PATH_INDEX {
                    return Err(de::Error::custom("Matrix result path exceeds limits"));
                }
                result.push(index);
            }
            if result.is_empty() {
                return Err(de::Error::custom(
                    "Matrix Area child path must not be empty",
                ));
            }
            Ok(result)
        }
    }
    deserializer.deserialize_seq(PathVisitor).map(Some)
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "TargetWire")]
pub struct MatrixResultTarget {
    pub owner_id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region_path: Option<Vec<usize>>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TargetWire {
    owner_id: u64,
    #[serde(default, deserialize_with = "path")]
    region_path: Option<Vec<usize>>,
}
impl TryFrom<TargetWire> for MatrixResultTarget {
    type Error = String;
    fn try_from(wire: TargetWire) -> Result<Self, Self::Error> {
        let target = Self {
            owner_id: wire.owner_id,
            region_path: wire.region_path,
        };
        target.validate()?;
        Ok(target)
    }
}
impl MatrixResultTarget {
    pub fn math(owner_id: u64) -> Self {
        Self {
            owner_id,
            region_path: None,
        }
    }
    pub fn area_child(owner_id: u64, region_path: Vec<usize>) -> Self {
        Self {
            owner_id,
            region_path: Some(region_path),
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.owner_id == 0
            || self.region_path.as_ref().is_some_and(|p| {
                p.is_empty()
                    || p.len() > MAX_MATRIX_VIEW_PATH
                    || p.iter().any(|n| *n > MAX_PATH_INDEX)
            })
        {
            return Err("Invalid matrix result target".into());
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "SizeWire")]
pub struct MatrixViewSize {
    pub rows: u32,
    pub cols: u32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SizeWire {
    rows: u32,
    cols: u32,
}
impl TryFrom<SizeWire> for MatrixViewSize {
    type Error = String;
    fn try_from(w: SizeWire) -> Result<Self, Self::Error> {
        let size = Self {
            rows: w.rows,
            cols: w.cols,
        };
        size.validate()?;
        Ok(size)
    }
}
impl MatrixViewSize {
    pub fn validate(&self) -> Result<(), String> {
        if self.rows == 0
            || self.cols == 0
            || self
                .rows
                .checked_mul(self.cols)
                .is_none_or(|n| n > MAX_MATRIX_VIEW_CELLS)
        {
            return Err("Matrix requested size exceeds supported limits".into());
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "OffsetWire")]
pub struct MatrixViewOffset {
    pub row: u32,
    pub col: u32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OffsetWire {
    row: u32,
    col: u32,
}
impl TryFrom<OffsetWire> for MatrixViewOffset {
    type Error = String;
    fn try_from(w: OffsetWire) -> Result<Self, Self::Error> {
        let offset = Self {
            row: w.row,
            col: w.col,
        };
        offset.validate()?;
        Ok(offset)
    }
}
impl MatrixViewOffset {
    pub fn validate(&self) -> Result<(), String> {
        if self.row > MAX_OFFSET || self.col > MAX_OFFSET {
            return Err("Matrix requested offset exceeds supported limits".into());
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "ViewportWire")]
pub struct MatrixResultViewport {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<MatrixViewSize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<MatrixViewOffset>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_indices: Option<bool>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ViewportWire {
    #[serde(default, deserialize_with = "present")]
    size: Option<MatrixViewSize>,
    #[serde(default, deserialize_with = "present")]
    offset: Option<MatrixViewOffset>,
    #[serde(default, deserialize_with = "present")]
    show_indices: Option<bool>,
}
impl TryFrom<ViewportWire> for MatrixResultViewport {
    type Error = String;
    fn try_from(w: ViewportWire) -> Result<Self, Self::Error> {
        let view = Self {
            size: w.size,
            offset: w.offset,
            show_indices: w.show_indices,
        };
        view.validate()?;
        Ok(view)
    }
}
impl MatrixResultViewport {
    pub fn is_empty(&self) -> bool {
        self.size.is_none() && self.offset.is_none() && self.show_indices.is_none()
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.is_empty() {
            return Err("Matrix viewport must contain an explicit request".into());
        }
        if let Some(size) = self.size {
            size.validate()?;
        }
        if let Some(offset) = self.offset {
            offset.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MatrixResultView {
    pub target: MatrixResultTarget,
    pub viewport: MatrixResultViewport,
}

/// Canonically target-sorted; private storage prevents bypassing admission.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct MatrixResultViews(Vec<MatrixResultView>);
impl<'de> Deserialize<'de> for MatrixResultViews {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ViewsVisitor;
        impl<'de> de::Visitor<'de> for ViewsVisitor {
            type Value = MatrixResultViews;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a bounded unique matrix result view array")
            }
            fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut views = MatrixResultViews::default();
                while let Some(view) = seq.next_element::<MatrixResultView>()? {
                    if views.len() >= MAX_MATRIX_RESULT_VIEWS || views.get(&view.target).is_some() {
                        return Err(de::Error::custom(
                            "Duplicate or excessive matrix result views",
                        ));
                    }
                    views
                        .set(view.target, view.viewport)
                        .map_err(de::Error::custom)?;
                }
                Ok(views)
            }
        }
        deserializer.deserialize_seq(ViewsVisitor)
    }
}
impl MatrixResultViews {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &MatrixResultView> {
        self.0.iter()
    }
    pub fn get(&self, target: &MatrixResultTarget) -> Option<&MatrixResultViewport> {
        self.0
            .binary_search_by(|v| v.target.cmp(target))
            .ok()
            .map(|i| &self.0[i].viewport)
    }
    /// Validate before mutation; returns the old request for view-only history.
    pub fn set(
        &mut self,
        target: MatrixResultTarget,
        viewport: MatrixResultViewport,
    ) -> Result<Option<MatrixResultViewport>, String> {
        target.validate()?;
        // Builder/default absence is a removal command, never a retained entry.
        if viewport.is_empty() {
            return Ok(self.remove(&target));
        }
        viewport.validate()?;
        match self.0.binary_search_by(|v| v.target.cmp(&target)) {
            Ok(i) => Ok(Some(std::mem::replace(&mut self.0[i].viewport, viewport))),
            Err(i) => {
                if self.len() >= MAX_MATRIX_RESULT_VIEWS {
                    return Err("Matrix result view count exceeds limits".into());
                }
                self.0.insert(i, MatrixResultView { target, viewport });
                Ok(None)
            }
        }
    }
    pub fn remove(&mut self, target: &MatrixResultTarget) -> Option<MatrixResultViewport> {
        self.0
            .binary_search_by(|v| v.target.cmp(target))
            .ok()
            .map(|i| self.0.remove(i).viewport)
    }
    pub fn remove_owner(&mut self, owner_id: u64) -> usize {
        let before = self.len();
        self.0.retain(|v| v.target.owner_id != owner_id);
        before - self.len()
    }
    /// Pure model admission; no evaluator, ZIP, native export or shape inference.
    /// Area XML is read at most once per referenced owner and never on get().
    pub fn validate_for_regions(&self, regions: &[CanvasRegion]) -> Result<(), String> {
        if self.is_empty() {
            return Ok(());
        }
        let mut owners = HashMap::new();
        for region in regions {
            if owners.insert(region.id, region).is_some() {
                return Err("Ambiguous matrix result owner".into());
            }
        }
        let mut area_paths = HashMap::<u64, HashSet<Vec<usize>>>::new();
        for view in &self.0 {
            view.target.validate()?;
            view.viewport.validate()?;
            let region = owners
                .get(&view.target.owner_id)
                .ok_or("Missing matrix result owner")?;
            match (&view.target.region_path, &region.content) {
                (None, CanvasContent::Math(_)) => {}
                (Some(path), CanvasContent::Opaque { kind, xml }) if kind == "Area" => {
                    if let std::collections::hash_map::Entry::Vacant(entry) =
                        area_paths.entry(region.id)
                    {
                        let paths = crate::area::area_math_children_xml(xml, &[])?
                            .into_iter()
                            .map(|child| child.region_path)
                            .collect();
                        entry.insert(paths);
                    }
                    if !area_paths[&region.id].contains(path) {
                        return Err("Matrix result path is not a direct Area Math child".into());
                    }
                }
                _ => {
                    return Err(
                        "Matrix result target must be Math or a direct Area Math child".into(),
                    );
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "matrix_result_views_tests.rs"]
mod tests;
