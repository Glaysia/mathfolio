//! Inner handle geometry and original margin-to-cell conversion. No UI state,
//! source, calculator or authority. Miniature dimensions remain our UI policy.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Handle {
    Left,
    Right,
    Up,
    Down,
    UpLeft,
    UpRight,
    DownLeft,
    DownRight,
}
impl Handle {
    pub(super) const ALL: [Self; 8] = [
        Self::Left,
        Self::Right,
        Self::Up,
        Self::Down,
        Self::UpLeft,
        Self::UpRight,
        Self::DownLeft,
        Self::DownRight,
    ];
    fn left(self) -> bool {
        matches!(self, Self::Left | Self::UpLeft | Self::DownLeft)
    }
    fn right(self) -> bool {
        matches!(self, Self::Right | Self::UpRight | Self::DownRight)
    }
    fn up(self) -> bool {
        matches!(self, Self::Up | Self::UpLeft | Self::UpRight)
    }
    fn down(self) -> bool {
        matches!(self, Self::Down | Self::DownLeft | Self::DownRight)
    }
    pub(super) fn cursor(self) -> egui::CursorIcon {
        match self {
            Self::Left | Self::Right => egui::CursorIcon::ResizeHorizontal,
            Self::Up | Self::Down => egui::CursorIcon::ResizeVertical,
            Self::UpLeft | Self::DownRight => egui::CursorIcon::ResizeNwSe,
            Self::UpRight | Self::DownLeft => egui::CursorIcon::ResizeNeSw,
        }
    }
    pub(super) fn rect(self, dark: Rect) -> Rect {
        // Original strips/corners are 4 logical DIP. Tiny-window partitioning
        // and the retained 240x160 miniature are independent app policies.
        let x = 4_f32.min(dark.width() / 2.);
        let y = 4_f32.min(dark.height() / 2.);
        let (l, r, t, b) = (dark.left(), dark.right(), dark.top(), dark.bottom());
        let (lo, hi) = match self {
            Self::Left => (pos2(l, t + y), pos2(l + x, b - y)),
            Self::Right => (pos2(r - x, t + y), pos2(r, b - y)),
            Self::Up => (pos2(l + x, t), pos2(r - x, t + y)),
            Self::Down => (pos2(l + x, b - y), pos2(r - x, b)),
            Self::UpLeft => (pos2(l, t), pos2(l + x, t + y)),
            Self::UpRight => (pos2(r - x, t), pos2(r, t + y)),
            Self::DownLeft => (pos2(l, b - y), pos2(l + x, b)),
            Self::DownRight => (pos2(r - x, b - y), pos2(r, b)),
        };
        Rect::from_min_max(lo, hi)
    }
    pub(super) fn at(dark: Rect, pointer: Pos2) -> Option<Self> {
        // Corner precedence at the exact shared strip boundary is our policy.
        [
            Self::UpLeft,
            Self::UpRight,
            Self::DownLeft,
            Self::DownRight,
            Self::Left,
            Self::Right,
            Self::Up,
            Self::Down,
        ]
        .into_iter()
        .find(|h| h.rect(dark).contains(pointer))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct View {
    pub(super) size: [usize; 2],
    pub(super) offset: [usize; 2],
}
impl View {
    pub(super) fn from_geometry(g: Geometry) -> Self {
        Self {
            size: g.size,
            offset: g.offset,
        }
    }
}

#[derive(Clone)]
pub(super) struct Resize {
    pub(super) handle: Handle,
    start: Pos2,
    light: Rect,
    shape: [usize; 2],
    original: View,
    pub(super) last: View,
}
impl Resize {
    pub(super) fn new(handle: Handle, start: Pos2, light: Rect, g: Geometry) -> Self {
        let original = View::from_geometry(g);
        Self {
            handle,
            start,
            light,
            shape: g.shape,
            original,
            last: original,
        }
    }
    pub(super) fn requested(&self, pointer: Pos2) -> View {
        if !pointer.is_finite() || self.light.width() <= 0. || self.light.height() <= 0. {
            return self.last;
        }
        let delta = pointer - self.start;
        let dx = f64::from(delta.x) / f64::from(self.light.width()) * self.shape[1] as f64;
        let dy = f64::from(delta.y) / f64::from(self.light.height()) * self.shape[0] as f64;
        let h = self.handle;
        // Generic07cc rejects the entire corner if either continuous width or
        // height becomes nonpositive. Use snapshot extent, not prior preview.
        if (h.left() && self.original.size[1] as f64 - dx <= 0.)
            || (h.right() && self.original.size[1] as f64 + dx <= 0.)
            || (h.up() && self.original.size[0] as f64 - dy <= 0.)
            || (h.down() && self.original.size[0] as f64 + dy <= 0.)
        {
            return self.last;
        }
        let l = (self.original.offset[1] as f64 + if h.left() { dx } else { 0. }).max(0.);
        let t = (self.original.offset[0] as f64 + if h.up() { dy } else { 0. }).max(0.);
        let r = ((self.shape[1] - self.original.offset[1] - self.original.size[1]) as f64
            - if h.right() { dx } else { 0. })
        .max(0.);
        let b = ((self.shape[0] - self.original.offset[0] - self.original.size[0]) as f64
            - if h.down() { dy } else { 0. })
        .max(0.);
        let row = (t as usize).min(self.shape[0] - 1);
        let col = (l as usize).min(self.shape[1] - 1);
        // Original1424 truncates the SUM of opposite margins, rather than
        // independently rounding the edge positions or accumulating steps.
        let rows = self.shape[0]
            .saturating_sub((t + b) as usize)
            .max(1)
            .min(self.shape[0] - row);
        let cols = self.shape[1]
            .saturating_sub((l + r) as usize)
            .max(1)
            .min(self.shape[1] - col);
        View {
            size: [rows, cols],
            offset: [row, col],
        }
    }
}
