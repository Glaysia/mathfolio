//! Source-linked shipped binding inventory, distinct from execution support.
use serde_json::{Value, json};

pub fn inventory() -> Value {
    let mut catalog: Value = serde_json::from_str(include_str!(
        "../../../docs/spec/mathcad12-engine-catalog.json"
    ))
    .expect("Versioned engine catalog must be valid JSON");
    for binding in catalog["bindings"].as_array_mut().expect("Binding array") {
        let name = binding["name"].as_str().unwrap_or("");
        let implemented = match binding["kind"].as_str() {
            Some("function") => {
                super::scalars::is_builtin(name)
                    || super::arrays::is_builtin(name)
                    || super::statistics::is_builtin(name)
                    || super::numerics::is_builtin(name)
                    || name == "root"
            }
            Some("operator") => matches!(
                name,
                "add"
                    | "sub"
                    | "negate"
                    | "mult"
                    | "div"
                    | "power"
                    | "sqrt"
                    | "nthroot"
                    | "transpose"
                    | "conjugate"
                    | "factorial"
                    | "abs"
                    | "vbar"
                    | "norm"
                    | "det"
                    | "lt"
                    | "leq"
                    | "gt"
                    | "geq"
                    | "not"
                    | "xor"
                    | "or"
                    | "and"
            ),
            Some("value") => matches!(name, "π" | "e" | "γ" | "_25_"),
            _ => false,
        };
        binding["rust_support"] = json!(if implemented {
            "bounded-subset"
        } else {
            "unimplemented"
        });
    }
    catalog["support_note"] = json!(
        "bounded-subset means a dispatcher exists with documented type, size and algorithm restrictions; it does not mean original parity. Programming and calculus syntax support are recorded separately in analysis 0006/0007."
    );
    catalog
}

pub fn summary() -> Value {
    let catalog = inventory();
    let bindings = catalog["bindings"].as_array().unwrap();
    let modules: Vec<_> = catalog["modules"].as_array().unwrap().iter().map(|module| {
        let name = module["module"].as_str().unwrap();
        let selected: Vec<_> = bindings.iter().filter(|b| b["module"] == name).collect();
        json!({"module":name,"bindings":selected.len(),"rust_bounded_subset":selected.iter().filter(|b| b["rust_support"] == "bounded-subset").count(),"evidence_id":module["evidence_id"]})
    }).collect();
    json!({"target":catalog["target"],"modules":modules,"bindings":bindings.len(),
        "rust_bounded_subset":bindings.iter().filter(|b| b["rust_support"] == "bounded-subset").count(),
        "note":catalog["support_note"],"limitations":catalog["limitations"]})
}
