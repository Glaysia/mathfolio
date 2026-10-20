//! Authored own Text admission, separate from retained native Text authority.
use super::*;
use crate::rich_text_source::AuthoredRichText;

impl CanvasRegion {
    /// Full source admission. Cached byte/fingerprint facts alone never prove
    /// this owner's type, plain mirror or provenance routing.
    pub fn validated_authored_text(&self) -> Result<Option<AuthoredRichText>, String> {
        if self.authored_text.is_some() && self.rich_text.is_some() {
            return Err("Text cannot have both imported and authored source".into());
        }
        let Some(wire) = self.authored_text.as_deref() else {
            return Ok(None);
        };
        let CanvasContent::Text(plain) = &self.content else {
            return Err("Authored Text source requires a Text region".into());
        };
        if self.source_region.is_some() {
            return Err("Authored Text must be a new native owner".into());
        }
        if plain.len() > crate::rich_text_source::MAX_SOURCE_BYTES {
            return Err("Authored plain mirror exceeds 1 MiB".into());
        }
        AuthoredRichText::from_wire(wire.clone(), plain).map(Some)
    }

    /// Prospective same/foreign-own clipboard duplicate. This clones only the
    /// region and immutable wire (no whole document/native archive), preserves
    /// authored provenance and never converts an imported native authority.
    pub fn authored_text_copy(&self, id: u64, left: f64, top: f64) -> Result<Self, String> {
        self.validated_authored_text()?
            .ok_or("Expected authored Text source")?;
        if id == 0 {
            return Err("Authored copy requires a nonzero owner".into());
        }
        check_coordinate(left)?;
        check_coordinate(top)?;
        let mut result = self.clone();
        result.id = id;
        result.left = left;
        result.top = top;
        result.source_region = None;
        Ok(result)
    }
}

impl CanvasDocument {
    /// Committed-key budget for a source already admitted by the authored
    /// editor. No seed serde, XML/ZIP parse, document/archive clone or export.
    pub fn validate_authored_text_edit_budget(
        &self,
        id: u64,
        source: &AuthoredRichText,
    ) -> Result<(), String> {
        let mut owners = self.regions.iter().filter(|region| region.id == id);
        let old = owners.next().ok_or("Text source owner is missing")?;
        if owners.next().is_some()
            || old.source_region.is_some()
            || old.rich_text.is_some()
            || !matches!(old.content, CanvasContent::Text(_))
        {
            return Err("Authored source owner is invalid or ambiguous".into());
        }
        let count = source
            .plain_text()
            .len()
            .checked_add(source.budget().serialized_wire_bytes())
            .ok_or("Canvas text exceeds total 8 MiB budget")?;
        if source.plain_text().len() > 1024 * 1024 || count > self.text_source_headroom(id)? {
            return Err("Canvas text exceeds total 8 MiB budget".into());
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "canvas_authored_tests.rs"]
mod tests;
