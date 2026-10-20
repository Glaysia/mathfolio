//! Authored-only current-format capture. No XML parse or source mutation.
use super::*;

/// Runtime-only small snapshot: a shallow immutable seed reference plus current
/// Run appearance. No current XML/index, history, catalog copy or native anchor.
#[derive(Clone, Debug)]
pub struct AuthoredCreationSnapshot {
    seed: Arc<AuthoredTextSeed>,
    format: AuthoredStyleSnapshot,
    text_decorations: Option<TextDecorations>,
}

impl AuthoredCreationSnapshot {
    pub fn format(&self) -> &AuthoredStyleSnapshot {
        &self.format
    }

    /// Resolved policy for this already admitted authored Run.
    pub fn text_decorations(&self) -> TextDecorations {
        self.text_decorations.unwrap_or(TextDecorations::None)
    }

    /// Materialize at a new-owner/explicit command boundary, not every frame or
    /// typing event. Different format requires bounded catalog clone, seed JSON
    /// serialization/hash and generated source validation. Equal format reuses
    /// the existing validated seed, including its original size lexeme.
    pub fn materialize(&self) -> Result<Arc<AuthoredTextSeed>> {
        if self.format == self.seed.spec.format
            && self.text_decorations == self.seed.spec.text_decorations
        {
            return Ok(self.seed.clone());
        }
        AuthoredTextSeed::new(AuthoredTextSeedSpec {
            format: self.format.clone(),
            proofing_language: self.seed.spec.proofing_language.clone(),
            catalog: self.seed.spec.catalog.clone(),
            text_decorations: self.text_decorations,
        })
    }
}

impl AuthoredRichText {
    /// Capture the explicitly owned Run, not a guessed side of a collapsed
    /// caret or a mixed selection. Run ordinal belongs to this source version.
    /// Non-format source metadata is immutable under authored source proof.
    pub fn creation_format_for_run(&self, ordinal: usize) -> Result<AuthoredStyleSnapshot> {
        let run = self
            .index
            .runs
            .get(ordinal)
            .ok_or("Unknown authored creation Run")?;
        let property = |slot: usize, name| {
            let value = run.effective[slot]
                .as_deref()
                .ok_or("Missing authored creation format property")?;
            validate_property(name, value)?;
            Ok::<_, String>(value)
        };
        let mut format = self.seed.spec.format.clone();
        format.weight = property(0, "FontWeight")?.into();
        format.font_style = property(1, "FontStyle")?.into();
        format.size_dip = property(2, "FontSize")?.into();
        let color = property(3, "Foreground")?;
        let byte = |range: Range<usize>| u8::from_str_radix(&color[range], 16).map_err(err);
        format.foreground_rgba = [byte(3..5)?, byte(5..7)?, byte(7..9)?, byte(1..3)?];
        Ok(format)
    }

    /// None in source is the closed authored scaffold's empty decoration
    /// policy. Never call this to resolve an imported native Run.
    pub fn creation_text_decorations_for_run(&self, ordinal: usize) -> Result<TextDecorations> {
        self.index
            .runs
            .get(ordinal)
            .ok_or("Unknown authored creation Run")?
            .effective[4]
            .as_deref()
            .map(TextDecorations::parse)
            .transpose()
            .map(|value| value.unwrap_or(TextDecorations::None))
    }

    /// Suitable retained app state after successful committed editing/formatting
    /// or an explicit command. Never apply this to transient IME projection.
    pub fn creation_snapshot_for_run(&self, ordinal: usize) -> Result<AuthoredCreationSnapshot> {
        Ok(AuthoredCreationSnapshot {
            seed: self.seed.clone(),
            format: self.creation_format_for_run(ordinal)?,
            text_decorations: self
                .index
                .runs
                .get(ordinal)
                .ok_or("Unknown authored creation Run")?
                .effective[4]
                .as_deref()
                .map(TextDecorations::parse)
                .transpose()?,
        })
    }

    /// Convenience for an immediate new Text command. For frequent capture,
    /// retain creation_snapshot_for_run and defer materialize until creation.
    pub fn creation_seed_for_run(&self, ordinal: usize) -> Result<Arc<AuthoredTextSeed>> {
        self.creation_snapshot_for_run(ordinal)?.materialize()
    }
}

#[cfg(test)]
#[path = "creation_tests.rs"]
mod tests;
