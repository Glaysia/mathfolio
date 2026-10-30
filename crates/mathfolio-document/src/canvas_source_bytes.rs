//! ARTIFACT ONLY: proposed child of canvas.rs, Rust-only source accounting.
//! No native package access, XML, serde, evaluator, source or archive clones.
use super::*;

pub(super) fn node_text_bytes(kind: &NodeKind) -> usize {
    match kind {
        NodeKind::Identifier(s)
        | NodeKind::Number(s)
        | NodeKind::StringLiteral(s)
        | NodeKind::BasedNumber { literal: s, .. }
        | NodeKind::Imaginary(s) => s.len(),
        NodeKind::AnnotatedIdentifier { name, labels } => name.len().saturating_add(labels.len()),
        NodeKind::NativeIdentifier {
            name,
            labels,
            subscript,
            ..
        } => name
            .len()
            .saturating_add(labels.len())
            .saturating_add(subscript.as_ref().map_or(0, String::len)),
        NodeKind::ImaginarySymbol { value, symbol } => value.len().saturating_add(symbol.len()),
        _ => 0,
    }
}

fn add(total: &mut usize, bytes: usize) -> Result<(), String> {
    *total = total
        .checked_add(bytes)
        .ok_or("Canvas text exceeds total 8 MiB budget")?;
    if *total > MAX_TEXT_BYTES {
        return Err("Canvas text exceeds total 8 MiB budget".into());
    }
    Ok(())
}

fn text_bytes(
    plain: &str,
    rich: Option<&crate::rich_text_source::RichTextWire>,
    authored: Option<&crate::rich_text_source::AuthoredTextWire>,
) -> Result<usize, String> {
    if plain.len() > 1024 * 1024 {
        return Err("Canvas text region exceeds 1 MiB".into());
    }
    if rich.is_some() && authored.is_some() {
        return Err("Text cannot have both imported and authored source".into());
    }
    let authored_bytes = authored
        .map(|wire| {
            wire.cached_serialized_wire_bytes()
                .ok_or("Authored Text source requires prior admission")
        })
        .transpose()?
        .unwrap_or(0);
    let rich_bytes = rich.map_or(0, |source| source.source_bytes());
    if rich_bytes > crate::rich_text_source::MAX_SOURCE_BYTES {
        return Err("Rich Text source exceeds 1 MiB".into());
    }
    plain
        .len()
        .checked_add(rich_bytes)
        .and_then(|bytes| bytes.checked_add(authored_bytes))
        .ok_or_else(|| "Canvas text exceeds total 8 MiB budget".into())
}

fn static_text_region(region: &CanvasRegion) -> Result<(), String> {
    if !matches!(region.content, CanvasContent::Text(_)) {
        return Err("Expected a Text region".into());
    }
    region.validated_authored_text()?;
    check_coordinate(region.left)?;
    check_coordinate(region.top)?;
    let f = &region.format;
    if !f.font_points.is_finite()
        || !(1. ..=200.).contains(&f.font_points)
        || f.precision > 15
        || f.exponential_threshold > 17
        || f.zero_threshold > 307
        || f.complex_threshold > 307
    {
        return Err("Text region format is outside document limits".into());
    }
    if region.calculation_disabled || region.calculation_enabled_override {
        return Err("Text cannot carry calculation-disable overrides".into());
    }
    if region
        .source_region
        .as_ref()
        .is_some_and(|source| source.len() > 1024)
    {
        return Err("Source region reference exceeds byte limit".into());
    }
    Ok(())
}

impl CanvasDocument {
    /// Same raw UTF-8 source accounting as validate, not complete validation.
    /// Existing fields/tree/property semantics remain checked by validate.
    /// Every traversal borrows source; current XAML Arc and native_archive are
    /// never cloned or decoded. This method is for committed-candidate checks.
    pub fn source_text_bytes(&self) -> Result<usize, String> {
        let mut total = 0usize;
        add(&mut total, self.title.len())?;
        let mut nodes = 0usize;
        for region in &self.regions {
            if let Some(source) = &region.source_region {
                add(&mut total, source.len())?;
            }
            if region.authored_text.is_some() && region.rich_text.is_some() {
                return Err("Text cannot have both imported and authored source".into());
            }
            if let Some(source) = &region.authored_text {
                if !matches!(region.content, CanvasContent::Text(_))
                    || region.source_region.is_some()
                {
                    return Err("Invalid authored Text source owner".into());
                }
                add(
                    &mut total,
                    source
                        .cached_serialized_wire_bytes()
                        .ok_or("Authored Text source requires prior admission")?,
                )?;
            }
            if let Some(source) = &region.rich_text {
                add(&mut total, source.source_bytes())?;
            }
            match &region.content {
                CanvasContent::Text(plain) => {
                    if plain.len() > 1024 * 1024 {
                        return Err("Canvas text region exceeds 1 MiB".into());
                    }
                    add(&mut total, plain.len())?;
                }
                CanvasContent::Math(tree) => {
                    let mut stack = vec![(tree, 1usize)];
                    while let Some((node, depth)) = stack.pop() {
                        nodes = nodes
                            .checked_add(1)
                            .ok_or("Canvas expression complexity exceeds limits")?;
                        if nodes > MAX_TREE_NODES_TOTAL || depth > MAX_CANVAS_TREE_DEPTH {
                            return Err("Canvas expression complexity exceeds limits".into());
                        }
                        add(&mut total, node_text_bytes(&node.kind))?;
                        if node.children.len() > MAX_TREE_NODES_TOTAL.saturating_sub(nodes) {
                            return Err("Canvas expression complexity exceeds limits".into());
                        }
                        stack.extend(node.children.iter().map(|child| (child, depth + 1)));
                    }
                }
                CanvasContent::Plot(spec) => {
                    add(&mut total, spec.source_text_bytes())?;
                    add(&mut total, spec.x_axis.label.len())?;
                    add(&mut total, spec.y_axis.label.len())?;
                }
                CanvasContent::Opaque { xml, kind } => {
                    add(&mut total, xml.len())?;
                    add(&mut total, kind.len())?;
                }
            }
        }
        for region in self
            .page_sections
            .header
            .regions
            .iter()
            .chain(self.page_sections.footer.regions.iter())
        {
            if let PageBandContent::Text(plain) = &region.content {
                if plain.len() > 1024 * 1024 {
                    return Err("Header or footer text region exceeds 1 MiB".into());
                }
                add(&mut total, plain.len())?;
            }
        }
        Ok(total)
    }

    /// Bytes available for the NEW plain mirror + current rich XAML together.
    /// Source reference/title/other regions/page bands stay charged once.
    pub fn text_source_headroom(&self, id: u64) -> Result<usize, String> {
        let mut found = self.regions.iter().filter(|region| region.id == id);
        let region = found.next().ok_or("Text source owner is missing")?;
        if found.next().is_some() {
            return Err("Text source owner is ambiguous".into());
        }
        let CanvasContent::Text(plain) = &region.content else {
            return Err("Expected Text source owner".into());
        };
        let old = text_bytes(
            plain,
            region.rich_text.as_deref(),
            region.authored_text.as_deref(),
        )?;
        let total = self.source_text_bytes()?;
        let unchanged = total
            .checked_sub(old)
            .ok_or("Inconsistent Text source accounting")?;
        MAX_TEXT_BYTES
            .checked_sub(unchanged)
            .ok_or_else(|| "Canvas text exceeds total 8 MiB budget".into())
    }

    /// Per committed key: ONLY source byte guards. Caller already owns a cached
    /// validated authority/next ValidatedRichText. This does not parse XML/ZIP or
    /// rebind the other carriers. Call before publishing document/runtime state.
    pub fn validate_text_edit_budget(
        &self,
        id: u64,
        plain: &str,
        rich: Option<&crate::rich_text_source::RichTextWire>,
    ) -> Result<(), String> {
        if self
            .regions
            .iter()
            .any(|region| region.id == id && region.authored_text.is_some())
        {
            return Err("Authored Text requires its source-aware editor budget".into());
        }
        if text_bytes(plain, rich, None)? > self.text_source_headroom(id)? {
            return Err("Canvas text exceeds total 8 MiB budget".into());
        }
        Ok(())
    }

    /// Command-only replacement, including selected-range formatting. Source
    /// routing cannot be changed as a side effect. Authoritative origin checks
    /// are separate and may parse on this command; never call them per key.
    pub fn validate_text_replace_budget(&self, candidate: &CanvasRegion) -> Result<(), String> {
        static_text_region(candidate)?;
        let mut owners = self
            .regions
            .iter()
            .filter(|region| region.id == candidate.id);
        let old = owners.next().ok_or("Text source owner is missing")?;
        if owners.next().is_some() || old.source_region != candidate.source_region {
            return Err("Prospective Text source owner changed or is ambiguous".into());
        }
        let CanvasContent::Text(plain) = &candidate.content else {
            unreachable!()
        };
        if text_bytes(
            plain,
            candidate.rich_text.as_deref(),
            candidate.authored_text.as_deref(),
        )? > self.text_source_headroom(candidate.id)?
        {
            return Err("Canvas text exceeds total 8 MiB budget".into());
        }
        Ok(())
    }

    /// Command-only prospective append. Candidate is one owned region already
    /// built from clipboard/runtime source, not a clone of the whole document.
    /// No ID/selection/history mutation occurs until this + origin admission pass.
    pub fn validate_text_append_budget(&self, candidate: &CanvasRegion) -> Result<(), String> {
        static_text_region(candidate)?;
        if candidate.id != self.next_id
            || candidate.id == 0
            || self.next_id.checked_add(1).is_none()
        {
            return Err("Invalid prospective Text region identity".into());
        }
        if candidate.source_region.is_some() {
            return Err("Appended Text must be a fresh native derivative owner".into());
        }
        let page_count = self
            .page_sections
            .header
            .regions
            .len()
            .saturating_add(self.page_sections.footer.regions.len());
        if self
            .regions
            .len()
            .saturating_add(page_count)
            .saturating_add(1)
            > MAX_REGIONS
        {
            return Err("Canvas title or region count exceeds limits".into());
        }
        let CanvasContent::Text(plain) = &candidate.content else {
            unreachable!()
        };
        let mut total = self.source_text_bytes()?;
        add(
            &mut total,
            text_bytes(
                plain,
                candidate.rich_text.as_deref(),
                candidate.authored_text.as_deref(),
            )?,
        )
    }
}

#[cfg(test)]
#[path = "canvas_source_bytes_tests.rs"]
mod tests;
