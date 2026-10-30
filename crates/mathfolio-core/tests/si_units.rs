use mathfolio_core::{Quantity, unit, units};
use std::collections::HashSet;

fn close(a: f64, b: f64) {
    assert!((a - b).abs() <= a.abs().max(b.abs()) * 1e-12, "{a} != {b}");
}
fn scale(name: &str) -> f64 {
    units::lookup(name).unwrap().value.re
}
fn same_dimension(name: &str, dimension: [f64; 7]) {
    assert!(units::lookup(name).unwrap().same_dimension(&Quantity {
        value: Default::default(),
        dimension
    }));
}

#[test]
fn exact_mathcad_symbols_distinguish_gravity_gram_and_gauss() {
    close(scale("g"), 9.80665);
    same_dimension("g", [1., 0., -2., 0., 0., 0., 0.]);
    close(scale("gm"), 0.001);
    same_dimension("gm", [0., 1., 0., 0., 0., 0., 0.]);
    close(scale("G"), 0.0001);
    same_dimension("G", [0., 1., -2., -1., 0., 0., 0.]);
    close(scale("mg"), 0.000001);
    same_dimension("mg", [0., 1., 0., 0., 0., 0., 0.]);
    close(unit("g").unwrap().value.re, 9.80665);
    assert_eq!(unit("mg").unwrap().dimension, [0., 1., 0., 0., 0., 0., 0.]);
}

#[test]
fn engineering_conversions_agree_through_independent_physical_relations() {
    close(scale("ft") / scale("in"), 12.);
    close(scale("mi") / scale("ft"), 5280.);
    close(scale("psi"), scale("lbf") / (scale("in") * scale("in")));
    close(scale("atm"), 101325.);
    close(scale("hr"), 3600.);
    close(scale("day"), 24. * scale("hr"));
    close(scale("knot"), scale("nmi") / scale("hr"));
    same_dimension("psi", [-1., 1., -2., 0., 0., 0., 0.]);
}

#[test]
fn calorie_conventions_and_temperature_differences_are_not_conflated() {
    close(scale("cal"), 4.1868);
    close(scale("cal15"), 4.1858);
    assert_ne!(scale("cal"), scale("cal15"));
    close(scale("Btu"), 1055.05585262);
    close(scale("BTU"), scale("Btu"));
    close(scale("Δ°F") / scale("Δ°C"), 5. / 9.);
    same_dimension("Δ°F", [0., 0., 0., 0., 1., 0., 0.]);
    assert!(units::lookup("°C").is_none());
    assert!(units::lookup("°F").is_none());
}

#[test]
fn physical_constants_are_separate_from_the_unit_label_namespace() {
    for name in ["h", "ℏ", "c", "k", "α", "e.c", "N.A", "R.gas", "μ.0", "ε.0"] {
        assert!(units::lookup(name).is_none(), "{name}");
        assert!(units::constant_lookup(name).is_some(), "{name}");
    }
    close(
        units::constant_lookup("h").unwrap().value.re,
        6.62607015e-34,
    );
    close(units::constant_lookup("c").unwrap().value.re, 299792458.);
    close(
        units::constant_lookup("e.c").unwrap().value.re,
        1.602176634e-19,
    );
    let h = units::constant_lookup("h").unwrap();
    assert_eq!(h.dimension, [2., 1., -1., 0., 0., 0., 0.]);
    let charge = units::constant_lookup("e.c").unwrap();
    assert_eq!(charge.dimension, [0., 0., 1., 1., 0., 0., 0.]);
    assert!(units::constant_lookup("e_c").is_none());
}

#[test]
fn catalog_has_unique_finite_positive_facts_and_excludes_money() {
    let names: Vec<_> = units::names().collect();
    let constants: Vec<_> = units::constant_names().collect();
    assert_eq!(names.len(), 212);
    assert_eq!(constants.len(), 15);
    let mut seen = HashSet::new();
    for name in names {
        assert!(seen.insert(name));
        let q = units::lookup(name).unwrap();
        assert!(q.value.re.is_finite() && q.value.re > 0. && q.value.im == 0.);
        assert!(q.dimension.iter().all(|d| d.is_finite()));
        assert!(units::constant_lookup(name).is_none());
    }
    assert!(units::lookup("¤").is_none());
    assert!(units::constant_lookup("¤").is_none());
    assert!(units::lookup("made-up-unit").is_none());
    assert!(units::constant_lookup("made-up-constant").is_none());
}
