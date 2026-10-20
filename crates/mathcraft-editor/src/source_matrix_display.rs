//! Source MATRIX display metadata, not evaluated-result presentation.
//!
//! Prime Equation 06001243 reads offset(row,column) before size(rows,columns);
//! 0600117a emits the optional first display followed by ALL source cells.
//! This closed subset rejects setter overhang normalization. It does not infer
//! source Origin/Show, result indices, or authoring factories' default window.
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceMatrixDisplay {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "non_null"
    )]
    offset: Option<[u32; 2]>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "non_null"
    )]
    size: Option<[u32; 2]>,
}

/// An omitted optional member is different from an explicitly null member.
pub(crate) fn non_null<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

impl SourceMatrixDisplay {
    pub fn from_parts(offset: Option<[u32; 2]>, size: Option<[u32; 2]>) -> Result<Self, String> {
        if size.is_some_and(|pair| pair.contains(&0)) {
            return Err("Source matrix display size must be positive".into());
        }
        Ok(Self { offset, size })
    }

    pub fn offset(self) -> Option<[u32; 2]> {
        self.offset
    }
    pub fn size(self) -> Option<[u32; 2]> {
        self.size
    }

    /// Returns (row/column offset, row/column size) without modifying source.
    /// Bounds are checked against FULL source shape, never against a result view.
    /// Tree/raw admission impose their existing independent cell/node budgets.
    pub fn window(self, shape: [usize; 2]) -> Result<([usize; 2], [usize; 2]), String> {
        if shape.contains(&0) || shape[0].checked_mul(shape[1]).is_none() {
            return Err("Invalid matrix size".into());
        }
        let offset = self.offset.unwrap_or([0, 0]).map(|n| n as usize);
        if offset.into_iter().zip(shape).any(|(n, full)| n >= full) {
            return Err("Source matrix display offset outside full shape".into());
        }
        let size = self
            .size
            .map(|pair| pair.map(|n| n as usize))
            .unwrap_or([shape[0] - offset[0], shape[1] - offset[1]]);
        if size
            .into_iter()
            .zip(shape)
            .any(|(n, full)| n == 0 || n > full)
        {
            return Err("Source matrix display size outside full shape".into());
        }
        if (0..2).any(|axis| {
            offset[axis]
                .checked_add(size[axis])
                .is_none_or(|n| n > shape[axis])
        }) {
            return Err("Source matrix display overhang normalization is unsupported".into());
        }
        Ok((offset, size))
    }
}

#[cfg(test)]
mod tests;
