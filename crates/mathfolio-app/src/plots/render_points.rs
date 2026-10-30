//! Immutable curve coordinates with lazy renderer-only projections. Persistence
//! and equality use raw f64 pairs; egui-specific arrays never enter JSON.
use egui_plot::PlotPoint;
use std::{
    fmt,
    ops::Deref,
    sync::{Arc, Mutex, OnceLock},
};

#[derive(Clone)]
pub(crate) struct CurvePoints(Arc<Storage>);

struct Storage {
    raw: Vec<[f64; 2]>,
    has_nan: bool,
    rendered: OnceLock<Vec<PlotPoint>>,
    sparse: Mutex<Option<(usize, Arc<[PlotPoint]>)>>,
}

impl From<Vec<[f64; 2]>> for CurvePoints {
    fn from(raw: Vec<[f64; 2]>) -> Self {
        let has_nan = raw.iter().flatten().any(|v| v.is_nan());
        Self(Arc::new(Storage {
            raw,
            has_nan,
            rendered: OnceLock::new(),
            sparse: Mutex::new(None),
        }))
    }
}
impl FromIterator<[f64; 2]> for CurvePoints {
    fn from_iter<T: IntoIterator<Item = [f64; 2]>>(iter: T) -> Self {
        iter.into_iter().collect::<Vec<_>>().into()
    }
}
impl Deref for CurvePoints {
    type Target = [[f64; 2]];
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}
impl fmt::Debug for CurvePoints {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.raw.fmt(f)
    }
}
impl PartialEq for CurvePoints {
    fn eq(&self, other: &Self) -> bool {
        (Arc::ptr_eq(&self.0, &other.0) && !self.0.has_nan) || self.0.raw == other.0.raw
    }
}
impl PartialEq<Vec<[f64; 2]>> for CurvePoints {
    fn eq(&self, other: &Vec<[f64; 2]>) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl CurvePoints {
    pub(crate) fn as_slice(&self) -> &[[f64; 2]] {
        &self.0.raw
    }
    pub(super) fn rendered(&self) -> &[PlotPoint] {
        self.0
            .rendered
            .get_or_init(|| self.0.raw.iter().copied().map(PlotPoint::from).collect())
    }
    /// Only the current sparse frequency is retained. Readers keep a small Arc
    /// across plot.show, so replacing this cache cannot invalidate a borrowed item.
    pub(super) fn sparse(&self, frequency: usize) -> Arc<[PlotPoint]> {
        assert!(frequency > 1, "full markers borrow rendered() directly");
        let mut cached = self
            .0
            .sparse
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if let Some((old, points)) = &*cached
            && *old == frequency
        {
            return points.clone();
        }
        let points: Arc<[PlotPoint]> = self
            .0
            .raw
            .iter()
            .step_by(frequency)
            .copied()
            .map(PlotPoint::from)
            .collect();
        *cached = Some((frequency, points.clone()));
        points
    }
    /// Coordinate payload accounting only: excludes Vec/Arc/control/allocator
    /// overhead and is deliberately not an RSS or allocated-byte measurement.
    #[cfg_attr(not(test), allow(dead_code))] // Opt-in artifact probe accounting.
    pub(crate) fn prepared_payload_bytes(&self) -> usize {
        let full = self
            .0
            .rendered
            .get()
            .map_or(0, |p| p.len() * std::mem::size_of::<PlotPoint>());
        let sparse = self
            .0
            .sparse
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        full + sparse
            .as_ref()
            .map_or(0, |(_, p)| p.len() * std::mem::size_of::<PlotPoint>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cloned_curve_borrows_one_lazy_projection_without_affecting_raw_equality() {
        let points: CurvePoints = vec![[0., -0.], [1., 3.], [2., 5.]].into();
        let clone = points.clone();
        assert!(Arc::ptr_eq(&points.0, &clone.0));
        assert_eq!(points.prepared_payload_bytes(), 0);
        assert_eq!(points.rendered().as_ptr(), clone.rendered().as_ptr());
        assert_eq!(points.prepared_payload_bytes(), 3 * 16);
        assert_eq!(points, clone);
        assert_eq!(points[0][1].to_bits(), (-0f64).to_bits());
        assert_eq!(points.rendered()[0].y.to_bits(), (-0f64).to_bits());
        assert_eq!(points, vec![[0., 0.], [1., 3.], [2., 5.]]);
    }
    #[test]
    fn sparse_frequency_replacement_is_bounded_and_surviving_readers_remain_valid() {
        let points: CurvePoints = (0..21).map(|i| [i as f64, (i * i) as f64]).collect();
        let old = points.sparse(10);
        assert_eq!(old.iter().map(|p| p.x).collect::<Vec<_>>(), [0., 10., 20.]);
        assert!(Arc::ptr_eq(&old, &points.sparse(10)));
        let next = points.sparse(4);
        assert_eq!(
            next.iter().map(|p| p.x).collect::<Vec<_>>(),
            [0., 4., 8., 12., 16., 20.]
        );
        assert_eq!(old[1].x, 10.);
        assert_eq!(points.prepared_payload_bytes(), 6 * 16);
        assert!(
            points.0.rendered.get().is_none(),
            "markers alone do not build full points"
        );
    }
    #[test]
    fn nan_self_equality_and_nonfinite_raw_values_keep_vec_semantics() {
        for raw in [
            vec![[f64::NAN, 1.]],
            vec![[f64::INFINITY, -0.]],
            vec![[1., 2.]],
        ] {
            let points: CurvePoints = raw.clone().into();
            assert_eq!(points == points.clone(), raw == raw.clone());
            assert_eq!(points == raw.clone(), raw == raw.clone());
        }
    }
}

#[cfg(target_arch = "wasm32")]
impl serde::Serialize for CurvePoints {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serde::Serialize::serialize(self.as_slice(), serializer)
    }
}
#[cfg(target_arch = "wasm32")]
impl<'de> serde::Deserialize<'de> for CurvePoints {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(<Vec<[f64; 2]> as serde::Deserialize>::deserialize(deserializer)?.into())
    }
}
