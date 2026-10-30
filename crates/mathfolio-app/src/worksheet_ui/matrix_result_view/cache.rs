//! Retain full matrix admission across view changes; never execute source.
use super::super::*;
use crate::result_display::{
    MatrixProjection, MatrixWindow, PreparedNativeMatrix, PreparedValueMatrix,
};

enum Authority {
    Native(PreparedNativeMatrix),
    Value(PreparedValueMatrix),
}
enum Admission {
    NotMatrix,
    Rejected(String),
    Prepared(Authority),
}
fn native_matrix_kind(raw: &str) -> Result<bool, String> {
    if raw.len() > 4 * 1024 * 1024 {
        return Err("Saved matrix carrier exceeds source limit".into());
    }
    let doc = roxmltree::Document::parse_with_options(
        raw,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: 262_144,
            ..Default::default()
        },
    )
    .map_err(|_| "Saved numeric carrier cannot be classified".to_owned())?;
    const ML: &str = "http://schemas.mathsoft.com/math50";
    let mut root = doc.root_element();
    if root.has_tag_name((ML, "result")) {
        let mut values = root.children().filter(roxmltree::Node::is_element);
        root = values.next().ok_or("Empty saved numeric result")?;
        if values.next().is_some() {
            return Err("Ambiguous saved numeric result".into());
        }
    }
    Ok(root.has_tag_name((ML, "matrix")))
}
impl Authority {
    fn project(
        &self,
        window: MatrixWindow,
        format: &MathFormat,
    ) -> Result<MatrixProjection, String> {
        match self {
            Self::Native(value) => value.project(window, format),
            Self::Value(value) => value.project(window, format),
        }
    }
    fn shape(&self) -> (usize, usize) {
        match self {
            Self::Native(value) => value.shape(),
            Self::Value(value) => value.shape(),
        }
    }
}

#[derive(Default)]
pub(in crate::worksheet_ui) struct MatrixCache {
    initialized: bool,
    authority: Option<Authority>,
    admission_error: Option<String>,
    key: Option<(MatrixWindow, MathFormat)>,
    value: Option<MatrixProjection>,
    error: Option<String>,
    #[cfg(test)]
    admissions: usize,
    #[cfg(test)]
    projections: usize,
}
impl MatrixCache {
    pub(in crate::worksheet_ui) fn prepare(
        &mut self,
        result: Option<&RegionEvaluation>,
        saved: Option<&SavedCalculationRow>,
    ) {
        if self.initialized {
            return;
        }
        self.initialized = true;
        #[cfg(test)]
        {
            self.admissions += 1;
        }
        match Self::admit(result, saved) {
            Admission::NotMatrix => {}
            Admission::Rejected(error) => self.admission_error = Some(error),
            Admission::Prepared(authority) => self.authority = Some(authority),
        }
    }
    fn admit(result: Option<&RegionEvaluation>, saved: Option<&SavedCalculationRow>) -> Admission {
        if let Some(value) = result.and_then(|r| r.display_value.as_ref().or(r.value.as_ref())) {
            if !matches!(value, mathfolio_compat::Value::Matrix { .. }) {
                return Admission::NotMatrix;
            }
            return match PreparedValueMatrix::new(Arc::new(value.clone())) {
                Ok(value) => Admission::Prepared(Authority::Value(value)),
                Err(error) => Admission::Rejected(error),
            };
        }
        let Some(source) = saved
            .filter(|r| r.error.is_none())
            .and_then(|r| r.numeric_source.as_ref())
        else {
            return Admission::NotMatrix;
        };
        match source {
            SavedNumericSource::NativeMathml(raw) => match PreparedNativeMatrix::parse(raw) {
                Ok(value) => Admission::Prepared(Authority::Native(value)),
                Err(error) => match native_matrix_kind(raw) {
                    Ok(false) => Admission::NotMatrix,
                    Ok(true) | Err(_) => Admission::Rejected(error),
                },
            },
            SavedNumericSource::EvaluatedValue(raw) if raw.len() <= 4 * 1024 * 1024 => {
                match serde_json::from_str::<mathfolio_compat::Value>(raw) {
                    Ok(value) if !matches!(value, mathfolio_compat::Value::Matrix { .. }) => {
                        Admission::NotMatrix
                    }
                    Ok(value) => match PreparedValueMatrix::new(Arc::new(value)) {
                        Ok(value) => Admission::Prepared(Authority::Value(value)),
                        Err(error) => Admission::Rejected(error),
                    },
                    Err(_) => Admission::Rejected("Invalid saved numeric carrier".into()),
                }
            }
            SavedNumericSource::EvaluatedValue(_) => {
                Admission::Rejected("Saved matrix carrier exceeds source limit".into())
            }
            _ => Admission::NotMatrix,
        }
    }
    pub(in crate::worksheet_ui) fn project(
        &mut self,
        window: MatrixWindow,
        format: MathFormat,
        result: Option<&RegionEvaluation>,
        saved: Option<&SavedCalculationRow>,
    ) -> Option<&MatrixProjection> {
        self.prepare(result, saved);
        let key = (window, format);
        if self.key != Some(key) {
            self.key = Some(key);
            self.value = None;
            self.error = None;
            if let Some(authority) = &self.authority {
                #[cfg(test)]
                {
                    self.projections += 1;
                }
                match authority.project(window, &format) {
                    Ok(projection) => self.value = Some(projection),
                    Err(error) => self.error = Some(error),
                }
            }
        }
        self.value.as_ref()
    }
    /// A command may not commit a supported DTO whose presentation is rejected.
    /// This publishes a new projection only on success; the complete admission
    /// is retained and an invalid request cannot destroy the old visible view.
    pub(in crate::worksheet_ui) fn preflight(
        &mut self,
        window: MatrixWindow,
        format: MathFormat,
    ) -> Result<(), String> {
        let authority = self
            .authority
            .as_ref()
            .ok_or("Complete matrix result is unavailable")?;
        // Exact successful preflight for this retained authority can be reused.
        // A rejected project has no value/error-free key; failed preflights
        // retain the previous successful key and cannot authorize a new one.
        if self.key == Some((window, format)) && self.value.is_some() && self.error.is_none() {
            return Ok(());
        }
        let next = authority.project(window, &format)?;
        #[cfg(test)]
        {
            self.projections += 1;
        }
        self.value = Some(next);
        self.key = Some((window, format));
        self.error = None;
        Ok(())
    }
    pub(in crate::worksheet_ui) fn shape(&self) -> Option<(usize, usize)> {
        self.authority.as_ref().map(Authority::shape)
    }

    pub(in crate::worksheet_ui) fn value(&self) -> Option<&MatrixProjection> {
        self.value.as_ref()
    }
    pub(in crate::worksheet_ui) fn error(&self) -> Option<&str> {
        self.error.as_deref().or(self.admission_error.as_deref())
    }
    pub(in crate::worksheet_ui) fn is_matrix(&self) -> bool {
        self.authority.is_some() || self.admission_error.is_some()
    }
    #[cfg(test)]
    pub(in crate::worksheet_ui) fn work_counts(&self) -> (usize, usize) {
        (self.admissions, self.projections)
    }
}
