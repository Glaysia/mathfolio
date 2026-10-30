//! Optional metadata-only region clipboard extension. This is not a result cache.
use super::*;
use mathfolio_document::canvas::MatrixResultViewport;
use serde::{Deserialize, Deserializer};

fn present_view<'de, D: Deserializer<'de>>(d: D) -> Result<Option<MatrixResultViewport>, D::Error> {
    let view = MatrixResultViewport::deserialize(d)?;
    if view.is_empty() {
        return Err(serde::de::Error::custom(
            "Empty clipboard matrix view must be omitted",
        ));
    }
    Ok(Some(view))
}
/// Parse this optional extension directly from original JSON so duplicate known
/// fields/nested fields cannot disappear through the legacy Value projection.
/// Other legacy envelope/bare-content fields deliberately remain ignored here.
pub(super) fn read_view(encoded: &str) -> Result<Option<MatrixResultViewport>, String> {
    #[derive(Deserialize)]
    struct Envelope {
        #[serde(default, deserialize_with = "present_view")]
        matrix_result_view: Option<MatrixResultViewport>,
    }
    serde_json::from_str::<Envelope>(encoded)
        .map(|v| v.matrix_result_view)
        .map_err(|e| e.to_string())
}
impl WorksheetApp {
    pub(super) fn clipboard_matrix_view(
        &self,
        region: &CanvasRegion,
    ) -> Result<Option<MatrixResultViewport>, String> {
        if !matches!(region.content, CanvasContent::Math(_)) {
            return Ok(None);
        }
        let request = self
            .document
            .matrix_result_views
            .get(&MatrixResultTarget::math(region.id))
            .copied();
        if region.source_region.is_some() {
            // Copy positively resolved metadata before the destination loses its
            // native owner. Do not substitute defaults for an unsupported read.
            match self.matrix_view_sources.effective(region.id, request) {
                Some(effective) => Ok((!effective.is_empty()).then_some(effective)),
                None if request.is_none() => Ok(None),
                None => Err("Native matrix result format cannot be copied safely".into()),
            }
        } else {
            Ok(request)
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn old_envelopes_and_bare_content_remain_absent() {
        for source in [
            r#"{"content":{"Text":"a"},"format":{}}"#,
            r#"{"Math":{"kind":"Placeholder","children":[]}}"#,
        ] {
            assert_eq!(read_view(source).unwrap(), None);
        }
    }
    #[test]
    fn extension_is_strict_but_does_not_require_result_or_engine_authority() {
        let view=read_view(r#"{"content":{},"matrix_result_view":{"size":{"rows":100,"cols":100},"offset":{"row":9,"col":8},"show_indices":false}}"#).unwrap().unwrap();
        assert_eq!(view.size.unwrap().rows, 100);
        assert_eq!(view.show_indices, Some(false));
        for source in [
            r#"{"matrix_result_view":null}"#,
            r#"{"matrix_result_view":{}}"#,
            r#"{"matrix_result_view":{"show_indices":true},"matrix_result_view":{"show_indices":false}}"#,
            r#"{"matrix_result_view":{"size":{"rows":1,"rows":2,"cols":3}}}"#,
            r#"{"matrix_result_view":{"size":{"rows":65536,"cols":2}}}"#,
            r#"{"matrix_result_view":{"future":true}}"#,
        ] {
            assert!(read_view(source).is_err(), "{source}");
        }
    }
}
