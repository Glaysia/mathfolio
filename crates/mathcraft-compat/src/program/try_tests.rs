use super::*;

fn marker() -> Symbol {
    Symbol {
        name: "marker".into(),
        label: "VARIABLE".into(),
    }
}
fn number(value: f64) -> Expression {
    Expression::Scalar(Quantity::scalar(value))
}
fn program_with(fault: Fault) -> Program {
    Program {
        instructions: vec![Instruction::Try {
            attempt: Program {
                instructions: vec![Instruction::Expression(Expression::Unsupported(fault))],
            },
            handler: Program {
                instructions: vec![Instruction::Assign {
                    target: marker(),
                    value: number(99.),
                }],
            },
        }],
    }
}

#[test]
fn host_limits_capability_gaps_and_upstream_failures_do_not_execute_the_handler() {
    let upstream = Fault {
        previous_region: Some("upstream".into()),
        ..Fault::new("divide_by_zero", "prior region failed")
    };
    for fault in [
        Fault::new("CANCELLED", "stopped"),
        Fault::new("program_budget", "host limit"),
        Fault::new("program_control", "invalid control"),
        Fault::new("program_no_value", "empty result"),
        Fault::new("EA041_RANGE_RECURRENCE", "not implemented recurrence"),
        Fault::unsupported("not implemented"),
        upstream,
    ] {
        let expected = fault.code.clone();
        let mut scope = Scope::default();
        assign_local(&mut scope, &marker(), Value::scalar(Quantity::scalar(7.)));
        let result = block(
            &program_with(fault),
            &mut scope,
            &mut Runtime { remaining: 100 },
            0,
        );
        assert_eq!(result.err().unwrap().code, expected);
        let Binding::Value(value) = scope.lookup(&marker()).unwrap() else {
            panic!("scalar expected")
        };
        assert_eq!(value.numeric_json()["real"].as_f64(), Some(7.));
    }
    let mut scope = Scope::default();
    assign_local(&mut scope, &marker(), Value::scalar(Quantity::scalar(7.)));
    let result = block(
        &program_with(Fault::new("divide_by_zero", "calculation failed")),
        &mut scope,
        &mut Runtime { remaining: 100 },
        0,
    )
    .unwrap();
    assert_eq!(result.control, Control::Normal);
    let Binding::Value(value) = scope.lookup(&marker()).unwrap() else {
        panic!("scalar expected")
    };
    assert_eq!(value.numeric_json()["real"].as_f64(), Some(99.));
}

#[test]
fn actual_cancellation_and_instruction_limits_remain_visible_and_fresh_retry_recovers() {
    let xml = format!(
        "<program xmlns='{ML}'><tryCatch><program><while><real>1</real><program><real>1</real></program></while></program><program><real>99</real></program></tryCatch></program>"
    );
    let value = from_mathml(&xml).unwrap();
    assert_eq!(execute(&value, &[], 30).unwrap_err().code, "program_budget");
    let control = super::super::CalculationControl::stop_after_checks(20);
    let stopped = control
        .run(|| execute(&value, &[], DEFAULT_BUDGET))
        .unwrap_err();
    assert!(stopped.is_cancelled());
    let retry = format!(
        "<program xmlns='{ML}'><tryCatch><program><apply><div/><real>1</real><real>0</real></apply></program><program><real>9</real></program></tryCatch></program>"
    );
    assert_eq!(
        execute(&from_mathml(&retry).unwrap(), &[], 100)
            .unwrap()
            .numeric_json()["real"]
            .as_f64(),
        Some(9.)
    );
}
