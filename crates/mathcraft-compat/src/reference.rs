//! Replay previously captured synthetic reference results, without Mathcad/GUI.
use crate::{PlacementGrid, RegionEvaluation, evaluate_worksheet_with_grid};
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Serialize)]
pub struct CaseReport {
    pub id: String,
    pub saved_geometry_match: bool,
    pub input_geometry_match: bool,
    pub saved_differences: Vec<String>,
    pub input_differences: Vec<String>,
}
#[derive(Debug, Serialize)]
pub struct ReferenceReport {
    pub product: String,
    pub version: String,
    pub cases: Vec<CaseReport>,
    pub saved_geometry_matched: usize,
    pub input_geometry_matched: usize,
    pub scope: &'static str,
}
impl ReferenceReport {
    pub fn all_match(&self) -> bool {
        self.cases
            .iter()
            .all(|c| c.saved_geometry_match && c.input_geometry_match)
    }
}
fn numeric_match(actual: &Value, expected: &Value, absolute: f64, relative: f64) -> bool {
    match (actual, expected) {
        (Value::Number(a), Value::Number(b)) => match (a.as_f64(), b.as_f64()) {
            (Some(a), Some(b)) => (a - b).abs() <= absolute.max(b.abs() * relative),
            _ => false,
        },
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len()
                && b.iter().all(|(k, v)| {
                    a.get(k)
                        .is_some_and(|a| numeric_match(a, v, absolute, relative))
                })
        }
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len()
                && a.iter()
                    .zip(b)
                    .all(|(a, b)| numeric_match(a, b, absolute, relative))
        }
        _ => actual == expected,
    }
}
fn compare(
    actual: &[RegionEvaluation],
    expected: &[Value],
    absolute: f64,
    relative: f64,
) -> Vec<String> {
    let mut differences = Vec::new();
    if actual.len() != expected.len() {
        differences.push(format!(
            "Region count {} != {}",
            actual.len(),
            expected.len()
        ));
    }
    for (i, (a, e)) in actual.iter().zip(expected).enumerate() {
        let value = a
            .display_value
            .as_ref()
            .or(a.value.as_ref())
            .map(|v| v.numeric_json())
            .unwrap_or(Value::Null);
        if !numeric_match(&value, &e["value"], absolute, relative) {
            differences.push(format!("Region {i}: value {value} != {}", e["value"]));
        }
        let expected_errors = e["errors"].as_array().expect("validated errors array");
        let expected_error = expected_errors.first();
        let code = expected_error
            .and_then(|v| v["code"].as_str())
            .map(|s| s.split('\t').next().unwrap_or(s));
        if a.error.as_ref().map(|f| f.code.as_str()) != code {
            differences.push(format!(
                "Region {i}: error {:?} != {:?}",
                a.error.as_ref().map(|f| &f.code),
                code
            ));
        }
        let previous = expected_error.is_some_and(|e| e["previous_error_id"].is_string());
        if previous
            != a.error
                .as_ref()
                .is_some_and(|f| f.previous_region.is_some())
        {
            differences.push(format!("Region {i}: error dependency differs"));
        }
        if e["redefinition_warning"].as_bool() != Some(a.redefinition_warning) {
            differences.push(format!("Region {i}: redefinition warning differs"));
        }
    }
    differences
}

/// Compares displayed scalar/vector values, error families, dependency presence
/// and redefinition flags. Exact error arguments/IDs, physical output dimensions,
/// typography and runtime lifecycle are outside this measurement.
pub fn check_reference(corpus: &Value) -> Result<ReferenceReport, String> {
    if corpus["schema_version"].as_u64() != Some(1) {
        return Err("Unsupported reference schema".into());
    }
    let cases = corpus["cases"]
        .as_array()
        .ok_or("Missing reference cases")?;
    if cases.is_empty() || cases.len() > 10000 {
        return Err("Invalid reference case count".into());
    }
    let absolute = corpus["numeric_tolerance"]["absolute"]
        .as_f64()
        .ok_or("Missing tolerance")?;
    let relative = corpus["numeric_tolerance"]["relative"]
        .as_f64()
        .ok_or("Missing tolerance")?;
    if !absolute.is_finite()
        || !relative.is_finite()
        || !(0.0..=1e-8).contains(&absolute)
        || !(0.0..=1e-6).contains(&relative)
    {
        return Err("Reference tolerance is outside supported bounds".into());
    }
    let mut results = Vec::new();
    for case in cases {
        let id = case["id"].as_str().ok_or("Missing case ID")?;
        let expected = case["expected"]
            .as_array()
            .ok_or("Missing expected regions")?;
        if expected.is_empty() {
            return Err(format!("Empty result set in {id}"));
        }
        for row in expected {
            if row["status"].as_str() != Some("Synchronized") {
                return Err(format!("Unsettled reference in {id}"));
            }
            let errors = row["errors"].as_array().ok_or("Missing errors array")?;
            if errors.len() > 1
                || errors.iter().any(|e| !e["code"].is_string())
                || !row["redefinition_warning"].is_boolean()
                || row.get("value").is_none()
            {
                return Err(format!("Malformed reference region in {id}"));
            }
        }
        let grid = PlacementGrid::from_presentation(
            case["presentation_xml"]
                .as_str()
                .ok_or("Missing presentation settings")?,
        )?;
        let run = |field: &str| -> Result<Vec<String>, String> {
            let xml = case[field]
                .as_str()
                .ok_or_else(|| format!("Missing {field}"))?;
            match evaluate_worksheet_with_grid(
                xml,
                if field == "input_worksheet_xml" {
                    Some(grid)
                } else {
                    None
                },
            ) {
                Ok(actual) => Ok(compare(&actual, expected, absolute, relative)),
                Err(error) => Ok(vec![error]),
            }
        };
        let saved = run("saved_worksheet_xml")?;
        let input = run("input_worksheet_xml")?;
        results.push(CaseReport {
            id: id.into(),
            saved_geometry_match: saved.is_empty(),
            input_geometry_match: input.is_empty(),
            saved_differences: saved,
            input_differences: input,
        });
    }
    Ok(ReferenceReport {
        product: corpus["product"].as_str().ok_or("Missing product")?.into(),
        version: corpus["version"].as_str().ok_or("Missing version")?.into(),
        saved_geometry_matched: results.iter().filter(|c| c.saved_geometry_match).count(),
        input_geometry_matched: results.iter().filter(|c| c.input_geometry_match).count(),
        cases: results,
        scope: "Synthetic top-level MathML subset, including page-grid load placement. Numeric display, error families/dependency presence, redefinition flags. Full Mathcad conformance remains incomplete.",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn corpus() -> Value {
        serde_json::from_str(include_str!("../../../tests/reference/mathcad12.json")).unwrap()
    }
    #[test]
    fn settled_original_cases_in_saved_and_loaded_geometry() {
        let report = check_reference(&corpus()).unwrap();
        assert_eq!(report.cases.len(), 38);
        let saved_failures: Vec<_> = report
            .cases
            .iter()
            .filter(|c| !c.saved_geometry_match)
            .collect();
        assert!(saved_failures.is_empty(), "{saved_failures:#?}");
        // Prime snaps 20 and 20+1e-8 to the same page grid before X ordering.
        let raw_failures: Vec<_> = report
            .cases
            .iter()
            .filter(|c| !c.input_geometry_match)
            .map(|c| c.id.as_str())
            .collect();
        assert!(raw_failures.is_empty(), "{raw_failures:#?}");
        assert!(report.all_match());
    }
    #[test]
    fn pending_and_incomplete_results_are_rejected() {
        let mut c = corpus();
        c["cases"][0]["expected"][0]["status"] = "Pending".into();
        assert!(check_reference(&c).is_err());
        c["cases"][0]["expected"] = serde_json::json!([]);
        assert!(check_reference(&c).is_err());
    }
    #[test]
    fn a_wrong_reference_answer_is_detected() {
        let mut c = corpus();
        c["cases"][0]["expected"][0]["value"]["real"] = 6.into();
        let report = check_reference(&c).unwrap();
        assert!(!report.cases[0].saved_geometry_match);
        assert!(!report.cases[0].input_geometry_match);
    }
}
