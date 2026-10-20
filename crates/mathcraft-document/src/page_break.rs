//! Pure root-worksheet page-break transactions. See docs/analysis/0141.
//! Canvas coordinates are authoritative; preserved native fragments are never
//! rewritten here. The caller owns blank-crosshair/selection and live bounds.
use crate::canvas::{CanvasContent, CanvasDocument, CanvasRegion, MathFormat};
use std::{cmp::Ordering, collections::HashMap};

const WS: &str = "http://schemas.mathsoft.com/worksheet50";
const LIMIT: f64 = 1_000_000_000.;
const EPSILON: f64 = 1e-10;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PageBreakLayout {
    pub body_height: f64,
    pub grid_height: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageBreakBoundsKind {
    Ordinary,
    PageWide,
    /// Unknown/protected content whose movement/geometry is not supported.
    Unsupported,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PageBreakBounds {
    pub region_id: u64,
    /// Absolute Canvas Y including render height and vertical alignment offset.
    pub bottom: f64,
    pub kind: PageBreakBoundsKind,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PageBreakChange {
    pub document: CanvasDocument,
    pub insertion: [f64; 2],
    /// Newly inserted, or removed, Canvas identity (not a package region ID).
    pub break_id: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RemovePageBreakDirection {
    Above,
    Below,
}

/// Strictly recognize the original empty payload, without trusting `kind` alone.
/// Outer coordinates/IDs may be stale: Canvas owns them until native export.
pub fn is_page_break(region: &CanvasRegion) -> Result<bool, String> {
    let CanvasContent::Opaque { xml, kind } = &region.content else {
        return Ok(false);
    };
    if kind != "pageBreak" {
        return Ok(false);
    }
    let source =
        roxmltree::Document::parse(xml).map_err(|_| "Invalid native page-break XML".to_string())?;
    let root = source.root_element();
    let payloads: Vec<_> = root
        .children()
        .filter(|node| node.is_element() && node.tag_name().name() != "metadata")
        .collect();
    if root.tag_name().name() != "region"
        || root.tag_name().namespace() != Some(WS)
        || payloads.len() != 1
        || payloads[0].tag_name().name() != "pageBreak"
        || payloads[0].tag_name().namespace() != Some(WS)
        || payloads[0].attributes().len() != 0
        || payloads[0]
            .children()
            .any(|node| node.is_element() || node.text().is_some_and(|s| !s.trim().is_empty()))
    {
        return Err("Page break must preserve a root region and empty native payload".into());
    }
    Ok(true)
}

fn compare(left: f64, right: f64) -> Ordering {
    if (left - right).abs() < EPSILON {
        Ordering::Equal
    } else {
        left.total_cmp(&right)
    }
}

fn coordinate(value: f64) -> Result<(), String> {
    if value.is_finite() && (0. ..=LIMIT).contains(&value) {
        Ok(())
    } else {
        Err("Page-break coordinates must be finite, nonnegative and within limits".into())
    }
}

struct PreparedBounds<'a> {
    breaks: Vec<usize>,
    by_id: HashMap<u64, &'a PageBreakBounds>,
}

fn preflight<'a>(
    document: &'a CanvasDocument,
    layout: PageBreakLayout,
    point: [f64; 2],
    bounds: &'a [PageBreakBounds],
) -> Result<PreparedBounds<'a>, String> {
    document.validate()?;
    coordinate(point[0])?;
    coordinate(point[1])?;
    if !layout.body_height.is_finite()
        || !layout.grid_height.is_finite()
        || layout.grid_height <= 0.
        || layout.grid_height > layout.body_height
        || layout.body_height > LIMIT
    {
        return Err("Page-break body/grid heights must be finite positive sizes".into());
    }
    let mut by_id = HashMap::new();
    for bound in bounds {
        coordinate(bound.bottom)?;
        if by_id.insert(bound.region_id, bound).is_some() {
            return Err("Duplicate page-break bounds identity".into());
        }
    }
    let mut breaks = Vec::new();
    let mut ordinary_count = 0;
    for (index, region) in document.regions.iter().enumerate() {
        coordinate(region.left)?;
        coordinate(region.top)?;
        if is_page_break(region)? {
            if compare(region.left, 0.) != Ordering::Equal {
                return Err("Page-break source must occupy the worksheet left edge".into());
            }
            breaks.push(index);
            continue;
        }
        ordinary_count += 1;
        let bound = by_id
            .get(&region.id)
            .ok_or("Missing live page-break bounds")?;
        if bound.bottom < region.top || bound.kind == PageBreakBoundsKind::Unsupported {
            return Err("Unsupported or invalid page-break bounds".into());
        }
        if bound.kind == PageBreakBoundsKind::PageWide
            && region.top < point[1]
            && point[1] < bound.bottom
        {
            return Err(
                "Insertion inside a page-wide region requires native overlap handling".into(),
            );
        }
    }
    if ordinary_count != by_id.len() {
        return Err("Page-break bounds must exactly cover non-break regions".into());
    }
    Ok(PreparedBounds { breaks, by_id })
}

/// Insert only at an already validated blank root crosshair. Selection state is
/// deliberately not inferred from a document. Returns one atomic undo candidate.
pub fn insert_page_break(
    document: &CanvasDocument,
    layout: PageBreakLayout,
    point: [f64; 2],
    bounds: &[PageBreakBounds],
) -> Result<PageBreakChange, String> {
    let PreparedBounds { breaks, by_id } = preflight(document, layout, point, bounds)?;
    let page = ((point[1] + layout.grid_height / 2.) / layout.body_height).trunc();
    let target_y = (page + 1.) * layout.body_height;
    coordinate(target_y)?;
    let next_break_y = breaks
        .iter()
        .map(|&index| document.regions[index].top)
        .filter(|&top| compare(top, point[1]) == Ordering::Greater)
        .min_by(f64::total_cmp);
    if next_break_y.is_some_and(|top| compare(top, target_y) == Ordering::Less) {
        return Err("Page-boundary rounding crosses an existing break".into());
    }
    let mut tops = Vec::with_capacity(document.regions.len());
    for (index, region) in document.regions.iter().enumerate() {
        let delta = if breaks.contains(&index) {
            if compare(region.top, target_y) != Ordering::Less {
                layout.body_height
            } else {
                0.
            }
        } else if compare(region.top, point[1]) == Ordering::Greater
            || (compare(region.top, point[1]) == Ordering::Equal
                && compare(region.left, point[0]) != Ordering::Less)
        {
            if next_break_y.is_some_and(|top| region.top >= top) {
                layout.body_height
            } else {
                target_y - point[1]
            }
        } else {
            0.
        };
        let top = region.top + delta;
        coordinate(top)?;
        if let Some(bound) = by_id.get(&region.id) {
            coordinate(bound.bottom + delta)?;
        }
        tops.push(top);
    }
    let id = document.next_id;
    let next_id = id.checked_add(1).ok_or("Page-break identity overflow")?;
    if id == 0
        || document.regions.len()
            + document.page_sections.header.regions.len()
            + document.page_sections.footer.regions.len()
            >= 4096
    {
        return Err("Page-break identity or region budget exhausted".into());
    }
    // Every source/bounds/arithmetic guard above precedes the document clone.
    let mut candidate = document.clone();
    for (region, top) in candidate.regions.iter_mut().zip(tops) {
        region.top = top;
    }
    candidate.next_id = next_id;
    candidate.regions.push(CanvasRegion {
        id,
        left: 0.,
        top: target_y,
        content: CanvasContent::Opaque {
            kind: "pageBreak".into(),
            xml: format!(
                "<region xmlns=\"{WS}\" region-id=\"0\" left=\"0\" top=\"0\" height=\"2\"><pageBreak/></region>"
            ),
        },
        source_region: None,
        rich_text: None,
        authored_text: None,
        format: MathFormat::default(),
        calculation_disabled: false,
        calculation_enabled_override: false,
    });
    candidate.validate()?;
    Ok(PageBreakChange {
        document: candidate,
        insertion: [0., target_y],
        break_id: id,
    })
}

/// Bounded single-break removal at a blank crosshair within one grid interval.
/// This does not implement arbitrary Remove Space, multi-break pulling, or the
/// original full CanDeleteVerticalSpace collision/restricted-area pipeline.
pub fn remove_single_page_break(
    document: &CanvasDocument,
    layout: PageBreakLayout,
    point: [f64; 2],
    break_id: u64,
    direction: RemovePageBreakDirection,
    bounds: &[PageBreakBounds],
) -> Result<PageBreakChange, String> {
    let PreparedBounds { breaks, by_id } = preflight(document, layout, point, bounds)?;
    if breaks.len() != 1 || document.regions[breaks[0]].id != break_id {
        return Err("Only explicit single-page-break removal is supported".into());
    }
    let break_y = document.regions[breaks[0]].top;
    let (upper, lower) = match direction {
        RemovePageBreakDirection::Above => (point[1] - layout.grid_height, point[1]),
        RemovePageBreakDirection::Below => (point[1], point[1] + layout.grid_height),
    };
    if !(break_y > upper && break_y <= lower) {
        return Err("Page break is outside the one-grid removal interval".into());
    }
    let mut prior_bottom: Option<f64> = None;
    for region in &document.regions {
        if region.id == break_id {
            continue;
        }
        let bound = by_id[&region.id];
        if region.top <= point[1] && point[1] < bound.bottom {
            return Err("Space removal requires a blank, non-overlapping insertion line".into());
        }
        if region.top < point[1] {
            prior_bottom = Some(prior_bottom.map_or(bound.bottom, |old| old.max(bound.bottom)));
        }
    }
    let offset = prior_bottom.map_or(-layout.body_height, |bottom| {
        (-layout.body_height).max(bottom - point[1])
    });
    let offset = if direction == RemovePageBreakDirection::Above {
        offset.max(-point[1])
    } else {
        offset
    };
    if offset >= 0. {
        return Err("Space removal has no safe upward movement".into());
    }
    let insertion = [point[0], point[1] + offset];
    coordinate(insertion[1])?;
    let mut tops = Vec::new();
    for region in &document.regions {
        if region.id == break_id {
            continue;
        }
        let moves = match direction {
            RemovePageBreakDirection::Above => region.top >= point[1],
            RemovePageBreakDirection::Below => region.top > point[1],
        };
        let top = region.top + if moves { offset } else { 0. };
        coordinate(top)?;
        coordinate(by_id[&region.id].bottom + if moves { offset } else { 0. })?;
        if moves && prior_bottom.is_some_and(|bottom| top < bottom) {
            return Err("Page-break removal would overlap preceding content".into());
        }
        tops.push((region.id, top));
    }
    let mut candidate = document.clone();
    candidate.regions.retain(|region| region.id != break_id);
    for (region, (_, top)) in candidate.regions.iter_mut().zip(tops) {
        region.top = top;
    }
    candidate.validate()?;
    Ok(PageBreakChange {
        document: candidate,
        insertion,
        break_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout() -> PageBreakLayout {
        PageBreakLayout {
            body_height: 700.,
            grid_height: 10.,
        }
    }
    fn text(document: &mut CanvasDocument, left: f64, top: f64) -> u64 {
        let id = document.next_id;
        document.next_id += 1;
        document.regions.push(CanvasRegion {
            id,
            left,
            top,
            content: CanvasContent::Text(format!("source {id}")),
            source_region: Some(format!("native-{id}")),
            rich_text: None,
            authored_text: None,
            format: MathFormat::default(),
            calculation_disabled: false,
            calculation_enabled_override: false,
        });
        id
    }
    fn bounds(document: &CanvasDocument) -> Vec<PageBreakBounds> {
        document
            .regions
            .iter()
            .filter(|r| !is_page_break(r).unwrap())
            .map(|r| PageBreakBounds {
                region_id: r.id,
                bottom: r.top + 10.,
                kind: PageBreakBoundsKind::Ordinary,
            })
            .collect()
    }

    #[test]
    fn first_insert_moves_y_x_suffix_and_preserves_sources() {
        let mut doc = CanvasDocument::default();
        text(&mut doc, 0., 100.);
        text(&mut doc, 20., 100.);
        text(&mut doc, 0., 120.);
        let before = doc.clone();
        let change = insert_page_break(&doc, layout(), [10., 100.], &bounds(&doc)).unwrap();
        assert_eq!(change.insertion, [0., 700.]);
        assert_eq!(
            change
                .document
                .regions
                .iter()
                .map(|r| r.top)
                .collect::<Vec<_>>(),
            [100., 700., 720., 700.]
        );
        for (old, new) in doc.regions.iter().zip(&change.document.regions) {
            assert_eq!(old.content, new.content);
            assert_eq!(old.source_region, new.source_region);
            assert_eq!(old.left, new.left);
        }
        assert_eq!(doc, before);
        let added = change.document.regions.last().unwrap();
        assert!(is_page_break(added).unwrap());
        assert!(added.source_region.is_none());
        assert!(
            matches!(&added.content, CanvasContent::Opaque {xml,..} if xml.contains("height=\"2\""))
        );
    }

    #[test]
    fn later_break_splits_delta_from_whole_page_shift() {
        let mut doc = CanvasDocument::default();
        text(&mut doc, 0., 300.);
        text(&mut doc, 0., 800.);
        let first = insert_page_break(&doc, layout(), [0., 500.], &bounds(&doc)).unwrap();
        let next = insert_page_break(
            &first.document,
            layout(),
            [0., 200.],
            &bounds(&first.document),
        )
        .unwrap();
        let tops: Vec<_> = next.document.regions.iter().map(|r| r.top).collect();
        assert_eq!(tops, [800., 1700., 1400., 700.]);
    }

    #[test]
    fn invalid_bounds_and_page_wide_crossing_are_atomic() {
        let mut doc = CanvasDocument::default();
        text(&mut doc, 0., 100.);
        let before = doc.clone();
        let mut b = bounds(&doc);
        b[0].bottom = 300.;
        b[0].kind = PageBreakBoundsKind::PageWide;
        assert!(insert_page_break(&doc, layout(), [0., 150.], &b).is_err());
        assert!(insert_page_break(&doc, layout(), [0., 150.], &[]).is_err());
        b[0].bottom = f64::NAN;
        assert!(insert_page_break(&doc, layout(), [0., 150.], &b).is_err());
        assert_eq!(doc, before);
    }

    #[test]
    fn overflow_and_malformed_native_break_are_atomic() {
        let mut doc = CanvasDocument {
            next_id: u64::MAX,
            ..CanvasDocument::default()
        };
        assert!(insert_page_break(&doc, layout(), [0., 100.], &[]).is_err());
        doc.next_id = 1;
        let mut change = insert_page_break(&doc, layout(), [0., 100.], &[]).unwrap();
        if let CanvasContent::Opaque { xml, .. } = &mut change.document.regions[0].content {
            *xml = format!("<region xmlns=\"{WS}\"><pageBreak nextPage=\"2\"/></region>");
        }
        assert!(insert_page_break(&change.document, layout(), [0., 100.], &[]).is_err());
        assert!(insert_page_break(&doc, layout(), [0., LIMIT], &[]).is_err());
    }

    #[test]
    fn simple_remove_pulls_to_previous_bottom_without_losing_source() {
        let mut doc = CanvasDocument::default();
        text(&mut doc, 0., 100.);
        text(&mut doc, 0., 300.);
        let inserted = insert_page_break(&doc, layout(), [0., 200.], &bounds(&doc)).unwrap();
        let removed = remove_single_page_break(
            &inserted.document,
            layout(),
            [0., 700.],
            inserted.break_id,
            RemovePageBreakDirection::Above,
            &bounds(&inserted.document),
        )
        .unwrap();
        assert_eq!(removed.insertion, [0., 110.]);
        assert_eq!(
            removed
                .document
                .regions
                .iter()
                .map(|r| r.top)
                .collect::<Vec<_>>(),
            [100., 210.]
        );
        assert_eq!(removed.document.regions[1].content, doc.regions[1].content);
        assert_eq!(
            removed.document.regions[1].source_region,
            doc.regions[1].source_region
        );
    }

    #[test]
    fn delete_from_above_break_preserves_the_reference_gap() {
        let mut doc = CanvasDocument::default();
        text(&mut doc, 0., 100.);
        text(&mut doc, 0., 300.);
        let inserted = insert_page_break(&doc, layout(), [0., 200.], &bounds(&doc)).unwrap();
        let removed = remove_single_page_break(
            &inserted.document,
            layout(),
            [30., 690.],
            inserted.break_id,
            RemovePageBreakDirection::Below,
            &bounds(&inserted.document),
        )
        .unwrap();
        assert_eq!(removed.insertion, [30., 110.]);
        assert_eq!(removed.document.regions[1].top, 220.);
    }

    #[test]
    fn remove_rejects_multiple_breaks_nonadjacent_and_collision() {
        let mut doc = CanvasDocument::default();
        text(&mut doc, 0., 100.);
        let first = insert_page_break(&doc, layout(), [0., 200.], &bounds(&doc)).unwrap();
        assert!(
            remove_single_page_break(
                &first.document,
                layout(),
                [0., 500.],
                first.break_id,
                RemovePageBreakDirection::Below,
                &bounds(&first.document)
            )
            .is_err()
        );
        let second = insert_page_break(
            &first.document,
            layout(),
            [0., 900.],
            &bounds(&first.document),
        )
        .unwrap();
        assert!(
            remove_single_page_break(
                &second.document,
                layout(),
                [0., 700.],
                first.break_id,
                RemovePageBreakDirection::Above,
                &bounds(&second.document)
            )
            .is_err()
        );
        let mut collision = first.document.clone();
        text(&mut collision, 0., 690.);
        let b = bounds(&collision);
        assert!(
            remove_single_page_break(
                &collision,
                layout(),
                [0., 695.],
                first.break_id,
                RemovePageBreakDirection::Below,
                &b
            )
            .is_err()
        );
    }
}
