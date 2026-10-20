# Mathcad 12 symbolic evaluation: shipped contract and independent polynomial subset

No original process, CLR, COM, GUI, screenshots, or symbolic backend was executed. This is independent Rust algebra over a bounded domain. It is not a replacement for the complete original CAS, and tests below do not establish original-runtime parity.

## Recovered input and output contract

`artifacts/rea/document-deep/schemas/Math50.xsd`, SHA256 `a10afb4dd442a4f5dd77b6f1df903c1d2e5b6f4d6a701aae38b0cefee21f6ca3`, resource-extraction manifest evidence `ev_733639bc33098dab0c4d11199cf3e2e1224402d74e9ca171f32c3355e310c704`:

| Anchor | Fact |
|---|---|
| Lines2092–2112, symEval | One expression, zero or more command elements, optional final symResult. Optional hide-lhs/hide-keywords booleans and style attribute. |
| Lines478–484 | Symbolic style values default/right-arrow/equals. |
| Lines615–628, command | One or two expression children represent keyword and parameters. This does not establish that every keyword accepts any two expressions. |
| Lines2113 onward, symResult | Result contains another mathematical expression. Symbolic output is not constrained to numeric scalar values. |
| Lines719,1074,1087 | Derivative operator; degree wrapper; lambda contains boundVars and body expression. |

Installed help under `Help/en/PTC_Mathcad_Help` supplies behavioral and serialized examples:

- `symbolic_evaluation_operator.html`: arrow applies to the entire expression; keyword/modifier placeholder is above it. The operator cannot be used as a single line in a program.
- `using_keywords_for_symbolic_operations.html`: existing variable assignments are substituted before applying a keyword. Stacked modifiers may interact; an arbitrary command list must not be treated as unrelated sequential calls.
- `simplifying_expressions.html`: simplify includes algebraic/trigonometric/exponential identities; modifiers and branch-domain assumptions matter. The tiny polynomial normalizer implements only part of this contract.
- `about_factoring_and_expanding_expressions.html`: expand multiplies out products/powers and may apply function identities.
- `to_evaluate_derivatives_symbolically.html`: native example is symEval containing apply/derivative, lambda/boundVars/body and degree/placeholder. A command/placeholder can represent an empty keyword slot. No differentiate keyword was established.
- `to_solve_equations_symbolically.html`: solve accepts Boolean equality or an expression interpreted as equal to0. The keyword can omit the variable when unambiguous; several solutions return a vector. Decimal inputs, domain assumptions, systems, periodic answers and fully modifiers require additional semantics beyond polynomial algebra.
- `prolog/keywords.prolog` lines44,61–62 enables expand/simplify/solve as symbolic keywords. Other keyword names are inventory facts, not implementation claims.

## REA engine boundary evidence

Complete retained record: `artifacts/rea/symbolics/efisymbolics-native-members.json`, REA evidence `ev_db39f51f2aba862968dd9716f106969a855798d8ab56af1139b8a78873820444`. Target `efisymbolics_.dll`, SHA256 `9c32b41d4362c6443e30b885476a05687133f9b93ad8f89dd3e0e37acdef887b`, MVID `97cbf0e9-3a8e-4d6d-b4a1-923c1e7faa76`. Supplemental `efisymbolics-native-il.json` decodes25 selected methods and verifies their raw CIL hashes against REA. This reads bytes only.

| Method token | Established boundary |
|---|---|
| Efi.Symbolics.mc_axiom_eval0x06000091 | Object.ToString, Marshal.StringToBSTR, current managed thread ID, Efi._AxiomEval0x06000076, Marshal.PtrToStringBSTR and cleanup appear in call evidence. It is a string/native-engine bridge; this does not reveal complete algebra semantics. Raw IL SHA256 `8d0a873325239a6c2afdad8c8095c8d17196a529e3c4e4856b1dd0bbd8344b98`. |
| Efi.Symbolics.mc_mupad_eval0x06000090 | Selected shipped body calls Efi.Throw. MuPAD naming alone must not be described as an operational backend. Raw IL SHA256 `9aaea22e555a59fe864b7280d9a07ae8756005b9cbe682bfee6ad112f26fced6`. |
| Efi.Symbolics.to_symbolic_string0x06000094 | Explicit integer/real/complex, string and matrix conversions, invariant number formatting, recursive matrix element conversion. Thus original symbolic input handling is wider than this implementation. |

`artifacts/rea/engine/efisymbolics-interface.txt` establishes distinct Expr constructors, parser/emitter boundary, numeric↔symbolic conversion and evaluator entry points. Neither interface names nor preserved command names justify claiming that the native CAS was reconstructed.

Exact hashes of all eight inspected help/keyword files are retained in `artifacts/rea/symbolics/source-hashes.json` with the read-only hashing script beside it.

## Implemented independent subset and integration

`crates/mathcraft-compat/src/symbolics.rs` parses native symEval/command and returns a standalone namespaced symResult expression via `Value::Symbolic { mathml }`. Cached input symResult is never trusted as the answer. Metadata flags/styles are validated but do not affect algebra. Parent hooks use Expression::Symbolic and normal worksheet evaluation dispatch; symbolic results remain distinct from numeric Value payloads.

Arithmetic uses checked i128 normalized rational coefficients, including exact decimal/exponent conversion; there is no binary floating-point arithmetic in polynomial transforms. Bounded multivariate polynomials support addition/subtraction, unary negation, multiplication/scale, division by rational constants, and nonnegative integer powers. Like terms combine and zero coefficients cancel. Simplify/expand produce an independently chosen canonical expanded polynomial form; original formatting, ordering, factoring decisions and rounding are not reproduced.

Derivative/partDerivative parses native lambda structure, one variable and placeholder/default or bounded positive integer degree, then differentiates polynomials exactly. No invented differentiate keyword is registered. Trigonometric and general function derivatives remain unsupported.

Finite exact-rational limits of bounded polynomials are also supported through the native `apply(limit, lambda, target)` shape. The optional left/right direction is validated and the lambda variable is lexical. The shipped numeric-dispatch rejection and the deliberately narrow symbolic implementation are documented separately in [0024](0024-symbolic-limits.md). Infinity, transcendental limits, variable-denominator rational functions and removable poles remain unsupported.

Solve handles one variable and degree at most2. Linear roots are exact rational values. Quadratic roots use exact rational arithmetic and exact square-root expressions; a repeated root returns a scalar, other two roots a native2×1 MathML matrix. Exact-square discriminants reduce to rational roots. Parametric coefficients, systems, inequalities, higher-degree polynomials, identity/no-solution sets, root ordering parity and assumptions/fully modifiers remain unsupported. Decimal inputs currently produce exact rational forms, differing from original decimal-solve presentation.

Worksheet binding substitution is conservative: explicit VARIABLE bindings must contain a real dimensionless exact integer within2^53. Such values substitute into the polynomial; failed bindings propagate their original fault. Existing noninteger, unitful, array, function or symbolic values fail explicitly rather than silently remaining free symbols. Undefined names remain polynomial variables. No general original symbolic definition snapshots, unit/constant semantics or contextual label inference are claimed. Polynomial identifier labels are restricted to missing/VARIABLE/automatic `*`, and output uses VARIABLE. Native styled spans/subscripts and other labels are rejected.

## Limits and verification

Policies:1MiB standalone input,64 expression nesting,512 polynomial terms,32 maximum exponent/derivative degree,16 keyword slots and checked rational overflow. A term-product bound caps each multiplication. These are project resource limits, not recovered original limits. Zero-to-zero power is rejected explicitly. Only simplify/expand/solve are accepted; unknown keywords/modifiers and commands after solve fail explicitly. Algebraic cancellation does not authorize arbitrary rational-function cancellation or branch-domain identities.

Meaningful synthetic tests are in `crates/mathcraft-compat/tests/symbolics.rs`: exact fractions/decimals, multivariate expansion, polynomial derivative degrees, solve structure and roots, ignored cached results, real worksheet dispatch/binding substitution/faults, malformed XML, unsupported operations and resource/overflow rejection. `cargo test -p mathcraft-compat --test programming --test statistics --test symbolics` passed22 tests (8/7/7). Program parsing now rejects descendant symEval, including assignment RHS, in accordance with the shipped operator help. Package Clippy at this checkpoint reported only four collapsible-if diagnostics in the concurrently developed solve module; these owned modules had no diagnostics. No original-runtime comparison was run.
