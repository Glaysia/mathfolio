//! Multi-region clipboard packets contain independent sources and relative offsets.
//! All candidates and aggregate budgets are validated before any insertion/cut.
use super::*;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Packet {
    version: u32,
    items: Vec<Item>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Item {
    left: f64,
    top: f64,
    region: Payload,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Payload {
    content: CanvasContent,
    #[serde(default)]
    format: MathFormat,
    #[serde(default)]
    calculation_disabled: bool,
    #[serde(
        default,
        deserialize_with = "present_view",
        skip_serializing_if = "Option::is_none"
    )]
    matrix_result_view: Option<MatrixResultViewport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    rich_text: Option<Box<mathfolio_document::rich_text_source::RichTextWire>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    authored_text: Option<Box<mathfolio_document::rich_text_source::AuthoredTextWire>>,
}
fn present_view<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<MatrixResultViewport>, D::Error> {
    let view = MatrixResultViewport::deserialize(d)?;
    if view.is_empty() {
        return Err(serde::de::Error::custom(
            "Empty matrix result view must be omitted",
        ));
    }
    Ok(Some(view))
}

fn decode(encoded: &str) -> Result<(CanvasRegion, Option<MatrixResultViewport>), String> {
    let view = matrix_region_clipboard::read_view(encoded)
        .map_err(|e| format!("Clipboard matrix result view is invalid: {e}"))?;
    let value: serde_json::Value = serde_json::from_str(encoded).map_err(|e| e.to_string())?;
    let rich =
        value
            .get("rich_text")
            .cloned()
            .map(
                serde_json::from_value::<
                    Option<Box<mathfolio_document::rich_text_source::RichTextWire>>,
                >,
            )
            .transpose()
            .map_err(|e| e.to_string())?
            .flatten();
    let authored = value
        .get("authored_text")
        .cloned()
        .map(
            serde_json::from_value::<
                Option<Box<mathfolio_document::rich_text_source::AuthoredTextWire>>,
            >,
        )
        .transpose()
        .map_err(|e| e.to_string())?
        .flatten();
    let disabled = value
        .get("calculation_disabled")
        .cloned()
        .map(serde_json::from_value::<bool>)
        .transpose()
        .map_err(|e| e.to_string())?
        .unwrap_or(false);
    let format = value
        .get("format")
        .cloned()
        .map(serde_json::from_value::<MathFormat>)
        .transpose()
        .map_err(|e| e.to_string())?
        .unwrap_or_default();
    let mut content: CanvasContent =
        serde_json::from_value(value.get("content").cloned().unwrap_or(value))
            .map_err(|e| e.to_string())?;
    if let CanvasContent::Opaque { kind, .. } = &mut content
        && kind == "specTable"
    {
        *kind = "spec-table".into();
    }
    let valid = match &content {
        CanvasContent::Math(tree) => tree.validate().is_ok(),
        CanvasContent::Text(text) => text.len() <= 1024 * 1024,
        CanvasContent::Plot(plot) => plot.validate().is_ok(),
        CanvasContent::Opaque { xml, kind }
            if matches!(kind.as_str(), "spec-table" | "specTable") =>
        {
            mathfolio_document::spec_table::SpecTable::parse(xml).is_ok()
        }
        _ => false,
    };
    if !valid
        || (view.is_some() && !matches!(content, CanvasContent::Math(_)))
        || ((rich.is_some() || authored.is_some()) && !matches!(content, CanvasContent::Text(_)))
        || (rich.is_some() && authored.is_some())
        || (disabled && matches!(content, CanvasContent::Text(_)))
        || !format.font_points.is_finite()
        || !(1. ..=200.).contains(&format.font_points)
        || format.precision > 15
    {
        return Err("Clipboard region is invalid or unsupported".into());
    }
    Ok((
        CanvasRegion {
            id: 0,
            left: 0.,
            top: 0.,
            content,
            source_region: None,
            rich_text: rich,
            authored_text: authored,
            format,
            calculation_disabled: disabled,
            calculation_enabled_override: false,
        },
        view,
    ))
}

impl WorksheetApp {
    fn region_clipboard_value(&mut self, id: u64) -> Result<serde_json::Value, String> {
        let rich = self.clipboard_rich_text(id)?;
        let region = self
            .document
            .regions
            .iter()
            .find(|r| r.id == id)
            .ok_or("Selected region no longer exists")?;
        if matches!(region.content, CanvasContent::Opaque { .. }) && !is_spec_table(region) {
            return Err(
                "선택한 읽기 전용 영역은 복사할 수 없습니다. 선택은 변경되지 않았습니다.".into(),
            );
        }
        let view = self.clipboard_matrix_view(region)?;
        let mut value = serde_json::json!({"content":region.content,"format":region.format,"calculation_disabled":region.calculation_disabled});
        if let Some(view) = view {
            value["matrix_result_view"] = serde_json::to_value(view).map_err(|e| e.to_string())?;
        }
        if let Some(wire) = rich {
            value["rich_text"] = serde_json::to_value(wire).map_err(|e| e.to_string())?;
        }
        if let Some(wire) = &region.authored_text {
            value["authored_text"] = serde_json::to_value(wire).map_err(|e| e.to_string())?;
        }
        Ok(value)
    }
    fn encode_selected_regions(&mut self) -> Result<String, String> {
        let selected: HashSet<_> = self.selected_region_ids().into_iter().collect();
        let origins: Vec<_> = self
            .document
            .regions
            .iter()
            .filter(|r| selected.contains(&r.id))
            .map(|r| (r.id, r.left, r.top))
            .collect();
        let left = origins.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
        let top = origins.iter().map(|p| p.2).fold(f64::INFINITY, f64::min);
        let mut items = Vec::with_capacity(origins.len());
        for (id, x, y) in origins {
            items.push(serde_json::json!({"left":x-left,"top":y-top,"region":self.region_clipboard_value(id)?}));
        }
        let payload = serde_json::to_string(&serde_json::json!({"version":1,"items":items}))
            .map_err(|e| e.to_string())?;
        if payload.len() > 8 * 1024 * 1024 {
            return Err("Clipboard regions are too large".into());
        }
        Ok(format!("{}{payload}", region_selection::CLIPBOARD_PREFIX))
    }
    pub(super) fn copy_region_group(&mut self, ctx: &egui::Context, cut: bool) {
        match self.encode_selected_regions() {
            Ok(text) => {
                ctx.copy_text(text);
                if cut {
                    self.remove_selected();
                }
            }
            Err(error) => self.status = error,
        }
    }
    pub(super) fn duplicate_region_group(&mut self) {
        match self.encode_selected_regions() {
            Ok(text) => {
                self.paste_region(&text);
            }
            Err(error) => self.status = error,
        }
    }
    pub(super) fn paste_clipboard_regions(
        &mut self,
        encoded: &str,
        group: bool,
    ) -> Result<(), String> {
        if encoded.len() > 8 * 1024 * 1024 {
            return Err("Clipboard regions are too large".into());
        }
        let selected: HashSet<_> = self.selected_region_ids().into_iter().collect();
        let base = self.insertion.unwrap_or_else(|| {
            let mut regions = self
                .document
                .regions
                .iter()
                .filter(|r| selected.contains(&r.id))
                .peekable();
            if regions.peek().is_none() {
                return (0., self.document.grid);
            }
            let (mut x, mut y) = (f64::INFINITY, f64::INFINITY);
            for r in regions {
                x = x.min(r.left);
                y = y.min(r.top);
            }
            (x + self.document.grid * 2., y + self.document.grid * 2.)
        });
        let mut candidates = Vec::new();
        if group {
            let value: Packet = serde_json::from_str(encoded).map_err(|e| e.to_string())?;
            if value.version != 1 {
                return Err("Unsupported region clipboard version".into());
            }
            let items = value.items;
            if items.is_empty() || items.len() > 10_000 {
                return Err("Invalid region clipboard count".into());
            }
            for item in items {
                let (x, y) = (item.left, item.top);
                let mut decoded =
                    decode(&serde_json::to_string(&item.region).map_err(|e| e.to_string())?)?;
                decoded.0.left = base.0 + x;
                decoded.0.top = base.1 + y;
                candidates.push(decoded);
            }
        } else {
            let mut decoded = decode(encoded)?;
            decoded.0.left = base.0;
            decoded.0.top = base.1;
            candidates.push(decoded);
        }
        // Validate source/ID/viewport/aggregate text budgets as one transaction.
        let mut probe = self.document.clone();
        // Structural edits invalidate the old cache, which may still name a
        // region just removed by Cut. Presentations remain in saved_results.
        probe.calculation_cache = None;
        for (region, view) in &mut candidates {
            region.id = probe.next_id;
            if matches!(region.content, CanvasContent::Text(_)) {
                self.rich_text.validate_append_to(&probe, region)?;
            }
            probe.next_id = probe
                .next_id
                .checked_add(1)
                .ok_or("Region ID space exhausted")?;
            if let Some(view) = view {
                probe
                    .matrix_result_views
                    .set(MatrixResultTarget::math(region.id), *view)?;
            }
            probe.regions.push(region.clone());
        }
        probe.validate()?;
        self.commit_table_edits(None)?;
        let before = self.snapshot();
        let ids: Vec<_> = candidates.iter().map(|c| c.0.id).collect();
        self.document.next_id = probe.next_id;
        self.document.calculation_cache = None;
        self.document.matrix_result_views = probe.matrix_result_views;
        for (region, _) in candidates {
            self.document.regions.push(region);
        }
        for id in &ids {
            self.refresh_rich_text_visual(*id);
            self.refresh_table_visual(*id);
        }
        self.selected = ids.first().copied();
        self.region_selection.ids = ids.iter().copied().collect();
        self.region_selection.marquee_cancel();
        self.insertion = None;
        self.editing_region = None;
        self.focus_new = None;
        self.remember(before);
        self.calculation_changed(false);
        Ok(())
    }
}
