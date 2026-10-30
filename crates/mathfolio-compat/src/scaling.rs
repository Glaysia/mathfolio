//! SCALE applies a named function on the right to the value on the left;
//! otherwise it multiplies. Original help/descriptor evidence: analysis/0110.
//! First-class function-valued expressions remain outside the current Value IR.
use super::*;

impl Scope {
    pub(super) fn scaling_function<'a>(&self, expression: &'a Expression) -> Option<&'a Symbol> {
        let Expression::Name(name) = expression else {
            return None;
        };
        match self.bindings.get(name) {
            Some(Binding::Function { .. }) => Some(name),
            Some(_) => None,
            None if builtin(name) => Some(name),
            None => None,
        }
    }

    pub(super) fn validate_call_name(&self, name: &Symbol, parameters: &[Symbol]) -> Result<()> {
        if !parameters.contains(name) && (self.bindings.contains_key(name) || !builtin(name)) {
            self.validate_name(name)?;
        }
        Ok(())
    }

    // Shared by ordinary APPLY and named-function SCALE. Arguments have already
    // been evaluated once in source order; captured scope and guard semantics
    // stay exactly those of the original call implementation.
    pub(super) fn call_named(
        &self,
        name: &Symbol,
        args: Vec<Value>,
        depth: usize,
    ) -> Result<Value> {
        // An explicit user definition takes priority over a built-in with the
        // same labelled identity. Failed/value bindings never become built-ins.
        if self.bindings.contains_key(name) {
            let binding = self.stored_binding(name)?.unwrap();
            if let Binding::Function {
                parameters,
                body,
                captured,
                local_guard,
            } = binding
            {
                if let Some(guard) = local_guard {
                    guard.check()?;
                }
                if args.len() != parameters.len() {
                    return Err(Fault::unsupported("Function argument count"));
                }
                let mut call = (**captured).clone();
                call.strings_use_origin = self.strings_use_origin;
                for (parameter, value) in parameters.iter().cloned().zip(args) {
                    call.bindings.insert(parameter, Binding::Value(value));
                }
                return call.evaluate(body, depth + 1);
            }
            return Err(Fault::unsupported("Calling a non-function"));
        }
        if !builtin(name) {
            return Err(Fault::unknown(name));
        }
        if unit_functions::is_builtin(name) {
            return unit_functions::forward(name, &args);
        }
        if strings::is_builtin(&name.name) {
            let sensitive = matches!(name.name.as_str(), "substr" | "search");
            if sensitive && self.strings_use_origin.is_none() {
                return Err(Fault::unsupported("String origin setting unavailable"));
            }
            let origin = if sensitive && self.strings_use_origin == Some(true) {
                let q = self.origin()?;
                if q.value.re < i32::MIN as f64 || q.value.re > i32::MAX as f64 {
                    return Err(Fault::unsupported("String ORIGIN exceeds 32-bit range"));
                }
                q.value.re as i32
            } else {
                0
            };
            return strings::call(&name.name, &args, origin);
        }
        if scalars::is_builtin(&name.name) {
            return scalars::call(&name.name, &args);
        }
        if statistics::is_builtin(&name.name) {
            return statistics::call(&name.name, &args);
        }
        if numerics::is_builtin(&name.name) {
            return numerics::call(&name.name, &args);
        }
        let origin = if matches!(name.name.as_str(), "last" | "submatrix") {
            match self.lookup(&Symbol {
                name: "ORIGIN".into(),
                label: "SYSTEM".into(),
            })? {
                Binding::Value(value) => value.as_quantity()?,
                _ => return Err(Fault::unsupported("Non-scalar SYSTEM ORIGIN")),
            }
        } else {
            Quantity::scalar(0.)
        };
        arrays::call(&name.name, &args, origin)
    }
}
