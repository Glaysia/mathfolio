//! Native source dimensions are independent of the app's f32 allocation.
use super::PlotSpec;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "GraphWire")]
pub struct PlotGraphSize {
    pub width: f64,
    pub height: f64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GraphWire {
    width: f64,
    height: f64,
}
impl TryFrom<GraphWire> for PlotGraphSize {
    type Error = String;
    fn try_from(raw: GraphWire) -> Result<Self, String> {
        let value = Self {
            width: raw.width,
            height: raw.height,
        };
        value.validate()?;
        Ok(value)
    }
}
impl PlotGraphSize {
    pub fn validate(&self) -> Result<(), String> {
        if !self.width.is_finite()
            || self.width <= 0.
            || !self.height.is_finite()
            || self.height <= 0.
        {
            return Err("Native plot graph size must be finite and positive".into());
        }
        Ok(())
    }
    pub fn same_source(&self, other: &Self) -> bool {
        self.width.to_bits() == other.width.to_bits()
            && self.height.to_bits() == other.height.to_bits()
    }
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "GeometryWire")]
pub struct PlotNativeGeometry {
    pub graph_size: Option<PlotGraphSize>,
    pub requested_width: Option<f64>,
    pub requested_height: Option<f64>,
    pub actual_width: Option<f64>,
    pub actual_height: Option<f64>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GeometryWire {
    graph_size: Option<PlotGraphSize>,
    requested_width: Option<f64>,
    requested_height: Option<f64>,
    actual_width: Option<f64>,
    actual_height: Option<f64>,
}
impl TryFrom<GeometryWire> for PlotNativeGeometry {
    type Error = String;
    fn try_from(raw: GeometryWire) -> Result<Self, String> {
        let value = Self {
            graph_size: raw.graph_size,
            requested_width: raw.requested_width,
            requested_height: raw.requested_height,
            actual_width: raw.actual_width,
            actual_height: raw.actual_height,
        };
        value.validate()?;
        Ok(value)
    }
}
pub(crate) fn same_number(a: Option<f64>, b: Option<f64>) -> bool {
    a.map(f64::to_bits) == b.map(f64::to_bits)
}
impl PlotNativeGeometry {
    pub fn validate(&self) -> Result<(), String> {
        if let Some(size) = &self.graph_size {
            size.validate()?;
        }
        if [self.requested_width, self.requested_height]
            .into_iter()
            .flatten()
            .any(|v| !v.is_finite() || v <= 0.)
        {
            return Err("Native plot requested size must be finite and positive".into());
        }
        if [self.actual_width, self.actual_height]
            .into_iter()
            .flatten()
            .any(|v| !v.is_finite() || v < 0.)
        {
            return Err("Native plot actual size must be finite and nonnegative".into());
        }
        Ok(())
    }
    pub fn same_source(&self, other: &Self) -> bool {
        (match (&self.graph_size, &other.graph_size) {
            (None, None) => true,
            (Some(a), Some(b)) => a.same_source(b),
            _ => false,
        }) && same_number(self.requested_width, other.requested_width)
            && same_number(self.requested_height, other.requested_height)
            && same_number(self.actual_width, other.actual_width)
            && same_number(self.actual_height, other.actual_height)
    }
    /// Missing requested dimensions use app defaults, never saved graph/actual.
    pub fn allocation(&self) -> Result<[f32; 2], String> {
        self.validate()?;
        let size = [
            self.requested_width.unwrap_or(420.) as f32,
            self.requested_height.unwrap_or(260.) as f32,
        ];
        if !size[0].is_finite()
            || !size[1].is_finite()
            || !(120. ..=2000.).contains(&size[0])
            || !(100. ..=2000.).contains(&size[1])
        {
            return Err("Native plot requested size exceeds the supported allocation range".into());
        }
        Ok(size)
    }
}
impl PlotSpec {
    /// One atomic outer resize. Unchanged axes retain exact native f64 source.
    pub fn set_outer_size(&mut self, width: f32, height: f32) -> Result<bool, String> {
        let changed_width = self.width.to_bits() != width.to_bits();
        let changed_height = self.height.to_bits() != height.to_bits();
        if !changed_width && !changed_height {
            self.validate()?;
            return Ok(false);
        }
        let mut next = self.clone();
        next.width = width;
        next.height = height;
        if let Some(geometry) = &mut next.native_geometry {
            if changed_width {
                geometry.requested_width = Some(f64::from(width));
            }
            if changed_height {
                geometry.requested_height = Some(f64::from(height));
            }
            geometry.actual_width = None;
            geometry.actual_height = None;
        }
        next.validate()?;
        *self = next;
        Ok(true)
    }
}
