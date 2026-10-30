//! Cached source-bound styled TextEdit layout; no native runtime claim.
//! Call `from_validated` only AFTER source authority/editability admission. The
//! readonly RichDocument projection alone does not prove editable XAML safety.
//! Rebuild after a committed source/style command, never from XML each frame.
use eframe::egui::{
    self,
    text::{LayoutJob, LayoutSection},
};
use mathfolio_document::native_visual::{Alignment, Block, RichDocument, TextStyle};
use std::{ops::Range, sync::Arc};

#[derive(Clone, Debug)]
pub(crate) struct StyledSpan {
    pub bytes: Range<usize>,
    pub style: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct ParagraphOffsets {
    pub index: usize,
    /// Global UTF8 byte range excluding the separating newline.
    pub bytes: Range<usize>,
    pub scalar_start: usize,
}

/// Controller-supplied affinity, not inferred from nearest glyph. The range is
/// relative to the immutable source passed to from_validated, not a prior draft.
#[derive(Clone, Debug)]
pub(crate) struct Replacement {
    pub old_bytes: Range<usize>,
    pub inserted_style: usize,
}

pub(crate) struct RichTextLayout {
    alignment: Alignment,
    source: Arc<str>,
    formats: Arc<[egui::TextFormat]>,
    spans: Arc<[StyledSpan]>,
    pub paragraphs: Arc<[ParagraphOffsets]>,
    /// Direct Run ordinal -> cached style index; matches source run_char_ranges.
    pub run_styles: Arc<[usize]>,
}

/// Cache this projection across idle frames. No XAML or Run processing occurs
/// in job(). TextEdit requires an owned job.text; that one buffer copy remains.
pub(crate) struct ProjectedLayout {
    alignment: Alignment,
    text: Arc<str>,
    formats: Arc<[egui::TextFormat]>,
    spans: Arc<[StyledSpan]>,
}

fn admitted(style: &TextStyle, single: bool) -> Result<(), String> {
    if (!single && style.alignment != Alignment::Left)
        || !style.font_size.is_finite()
        || !(style.font_size > 0. && style.font_size <= 4096.)
    {
        return Err("Unsupported editable text geometry/style".into());
    }
    Ok(())
}

impl RichTextLayout {
    pub fn horizontal_align(&self) -> egui::Align {
        match self.alignment {
            Alignment::Left | Alignment::Justify => egui::Align::LEFT,
            Alignment::Center => egui::Align::Center,
            Alignment::Right => egui::Align::RIGHT,
        }
    }
    pub fn from_validated(document: &RichDocument, plain: &str) -> Result<Self, String> {
        let single = matches!(document.blocks.as_slice(), [Block::Paragraph(_)]);
        let alignment = match document.blocks.as_slice() {
            [Block::Paragraph(p)] => p.style.alignment,
            _ => Alignment::Left,
        };
        admitted(&document.style, single)?;
        let mut source = String::new();
        let mut formats = Vec::new();
        let mut spans = Vec::new();
        let mut paragraphs = Vec::new();
        let mut run_styles = Vec::new();
        let mut scalar_start = 0;
        let mut preceding_empty_style = None;
        for (index, block) in document.blocks.iter().enumerate() {
            let Block::Paragraph(paragraph) = block else {
                return Err("Editable text supports direct paragraphs/runs only".into());
            };
            admitted(&paragraph.style, single)?;
            if paragraph.margin != [0.; 4]
                || paragraph.line_height.is_some()
                || paragraph.style.background.is_some()
            {
                return Err("Unsupported TextEdit paragraph geometry/background".into());
            }
            let default_style = formats.len();
            formats.push(crate::native_presentation::text_format(
                &paragraph.style,
                None,
            ));
            if index != 0 {
                let start = source.len();
                source.push('\n');
                spans.push(StyledSpan {
                    bytes: start..source.len(),
                    style: preceding_empty_style.unwrap_or(default_style),
                });
                scalar_start += 1;
            }
            let start = source.len();
            let first_run_style = formats.len();
            for run in &paragraph.runs {
                admitted(&run.style, single)?;
                let style = formats.len();
                run_styles.push(style);
                formats.push(crate::native_presentation::text_format(&run.style, None));
                let start = source.len();
                source.push_str(&run.text);
                if start != source.len() {
                    spans.push(StyledSpan {
                        bytes: start..source.len(),
                        style,
                    });
                }
            }
            preceding_empty_style = None;
            if start == source.len() && !paragraph.runs.is_empty() {
                // One empty Run keeps its existing glyph style. For a single
                // all-empty Paragraph with multiple retained Runs, draw only
                // its defined Paragraph glyph style, not an arbitrary Run's.
                // This is visual state and grants no future typing affinity.
                let caret_style = if paragraph.runs.len() == 1 {
                    first_run_style
                } else if single {
                    default_style
                } else {
                    return Err("Empty paragraph requires an unambiguous Run style owner".into());
                };
                preceding_empty_style = Some(caret_style);
                spans.push(StyledSpan {
                    bytes: start..start,
                    style: caret_style,
                });
            }
            paragraphs.push(ParagraphOffsets {
                index,
                bytes: start..source.len(),
                scalar_start,
            });
            scalar_start += source[start..].chars().count();
        }
        if source != plain {
            return Err("Rich layout/plain buffer mismatch".into());
        }
        if formats.is_empty() {
            formats.push(crate::native_presentation::text_format(
                &document.style,
                None,
            ));
        }
        Ok(Self {
            alignment,
            source: source.into(),
            formats: formats.into(),
            spans: spans.into(),
            paragraphs: paragraphs.into(),
            run_styles: run_styles.into(),
        })
    }

    /// Scalar coordinates used by the source selection API -> exact UTF8 bytes.
    #[cfg(test)]
    pub fn paragraph_bytes(
        &self,
        paragraph: usize,
        chars: Range<usize>,
    ) -> Result<Range<usize>, String> {
        let p = self.paragraphs.get(paragraph).ok_or("Unknown paragraph")?;
        let text = &self.source[p.bytes.clone()];
        let boundary = |offset| {
            text.char_indices()
                .map(|(i, _)| i)
                .chain(std::iter::once(text.len()))
                .nth(offset)
        };
        if chars.start > chars.end {
            return Err("Reversed selection".into());
        }
        Ok(
            (p.bytes.start + boundary(chars.start).ok_or("Selection out of bounds")?)
                ..(p.bytes.start + boundary(chars.end).ok_or("Selection out of bounds")?),
        )
    }

    pub fn project(
        &self,
        buffer: &str,
        replacement: Option<Replacement>,
    ) -> Result<ProjectedLayout, String> {
        if buffer == self.source.as_ref() {
            return Ok(ProjectedLayout {
                alignment: self.alignment,
                text: self.source.clone(),
                formats: self.formats.clone(),
                spans: self.spans.clone(),
            });
        }
        let replacement =
            replacement.ok_or("Changed buffer requires explicit replacement/style")?;
        let range = replacement.old_bytes;
        if range.start > range.end
            || range.end > self.source.len()
            || !self.source.is_char_boundary(range.start)
            || !self.source.is_char_boundary(range.end)
            || replacement.inserted_style >= self.formats.len()
        {
            return Err("Invalid source replacement boundary/style".into());
        }
        let suffix = &self.source[range.end..];
        if buffer.len() < range.start + suffix.len()
            || !buffer.starts_with(&self.source[..range.start])
            || !buffer.ends_with(suffix)
        {
            return Err("Buffer is not the declared single replacement".into());
        }
        let inserted_end = buffer.len() - suffix.len();
        if !buffer.is_char_boundary(inserted_end) {
            return Err("Invalid draft UTF8 boundary".into());
        }
        let mut spans = Vec::new();
        for span in self.spans.iter() {
            if span.bytes.is_empty() && span.bytes.start < range.start {
                spans.push(span.clone());
                continue;
            }
            let end = span.bytes.end.min(range.start);
            if span.bytes.start < end {
                spans.push(StyledSpan {
                    bytes: span.bytes.start..end,
                    style: span.style,
                });
            }
        }
        // Deleting the sole text in a Run must retain that explicit style for
        // the now-empty preview paragraph as well as the eventual source.
        let empty_replacement = range.start == inserted_end
            && (range.start == 0 || buffer.as_bytes().get(range.start - 1) == Some(&b'\n'))
            && (inserted_end == buffer.len()
                || buffer.as_bytes().get(inserted_end) == Some(&b'\n'));
        if range.start < inserted_end || empty_replacement {
            spans.push(StyledSpan {
                bytes: range.start..inserted_end,
                style: replacement.inserted_style,
            });
        }
        for span in self.spans.iter() {
            if span.bytes.is_empty() && span.bytes.start > range.end {
                let at = inserted_end + span.bytes.start - range.end;
                spans.push(StyledSpan {
                    bytes: at..at,
                    style: span.style,
                });
                continue;
            }
            let start = span.bytes.start.max(range.end);
            if start < span.bytes.end {
                spans.push(StyledSpan {
                    bytes: inserted_end + start - range.end
                        ..inserted_end + span.bytes.end - range.end,
                    style: if empty_replacement
                        && start == range.end
                        && &self.source[span.bytes.clone()] == "\n"
                    {
                        replacement.inserted_style
                    } else {
                        span.style
                    },
                });
            }
        }
        Ok(ProjectedLayout {
            alignment: self.alignment,
            text: Arc::from(buffer),
            formats: self.formats.clone(),
            spans: spans.into(),
        })
    }
}

impl ProjectedLayout {
    pub fn job(&self, buffer: &str, wrap_width: f32) -> Result<LayoutJob, String> {
        if buffer != self.text.as_ref() {
            return Err("Stale rich layout projection".into());
        }
        if !wrap_width.is_finite() || wrap_width <= 0. {
            return Err("Invalid wrap width".into());
        }
        let mut job = LayoutJob {
            text: buffer.to_owned(),
            ..Default::default()
        };
        job.wrap.max_width = wrap_width;
        job.halign = match self.alignment {
            Alignment::Left | Alignment::Justify => egui::Align::LEFT,
            Alignment::Center => egui::Align::Center,
            Alignment::Right => egui::Align::RIGHT,
        };
        job.justify = self.alignment == Alignment::Justify;
        if self.alignment != Alignment::Left {
            // Matches TextEdit's own alignment layouter for trailing caret text.
            job.keep_trailing_whitespace = true;
        }
        job.sections = self
            .spans
            .iter()
            .map(|span| LayoutSection {
                leading_space: 0.,
                byte_range: egui::text::ByteIndex(span.bytes.start)
                    ..egui::text::ByteIndex(span.bytes.end),
                format: self.formats[span.style].clone(),
            })
            .collect();
        // Preserve an empty buffer exactly: never insert the readonly renderer's
        // synthetic space. TextEdit supplies caret/empty-row layout behavior.
        if buffer.is_empty() && job.sections.is_empty() {
            job.sections.push(LayoutSection {
                leading_space: 0.,
                byte_range: egui::text::ByteIndex(0)..egui::text::ByteIndex(0),
                format: self.formats[0].clone(),
            });
        }
        Ok(job)
    }

    pub fn layout(
        &self,
        ui: &egui::Ui,
        buffer: &str,
        wrap_width: f32,
    ) -> Result<Arc<egui::Galley>, String> {
        Ok(crate::typography::layout_job(
            ui.painter(),
            self.job(buffer, wrap_width)?,
        ))
    }
}

#[cfg(test)]
#[path = "rich_text_layout_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "rich_text_empty_style_tests.rs"]
mod empty_style_tests;

#[cfg(test)]
#[path = "paragraph_alignment_geometry_tests.rs"]
mod paragraph_alignment_geometry_tests;
