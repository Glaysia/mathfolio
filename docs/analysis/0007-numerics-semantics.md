# Numerical operators: recovered contracts and independent implementation

Status: static investigation and executable Rust implementation, 2026-10-15.
No Mathcad process, GUI, COM, screenshot, or original worksheet was used in this
work. Original native numerical results have not been collected for these new
operators. The tests below are self-authored mathematical examples, not a
Mathcad compatibility oracle.

## Evidence and what it establishes

All installation paths below are relative to
`C:\Program Files\PTC\Mathcad Prime 12.0.0.1`.
Raw local evidence remains in ignored `artifacts/rea/numerics/` and
`artifacts/rea/engine/`; installed implementation text is not copied into the
Rust source or this report.

| Evidence | Established fact | Limit of evidence |
| --- | --- | --- |
| Embedded `Math50.xsd`, extracted from `Ptc.Core.dll`; SHA256 `a10afb4dd442a4f5dd77b6f1df903c1d2e5b6f4d6a701aae38b0cefee21f6ca3` | `apply` contains calculus operator, `lambda`, and bounds or degree; lambda has `boundVars` then expression. `derivative` and `partDerivative` have optional degree; `integral`, `summation`, `product` use lower/upper bounds. Integral algorithm defaults to adaptive. | Grammar alone does not establish iteration, tolerance, rounding, or convergence algorithms. |
| `lisp/front12.lisp` SHA256 `5b15dd2d9066b253200133a5b5fd79263718a9f5f5d4e520cdadda5c104e01ee`, lines 3551–3578 and 3628–3655 | Front end recognizes derivative orders and partial derivative variants with a single lambda binder. Its rewriting renames the bound variable and substitutes the body to avoid collisions. | This is front-end rewriting, not a recovered numerical derivative implementation. |
| `lisp/solve.lisp`, `make-solve-call`, lines 574–629 | Multivariate solve lowering passes TOL, CTOL, initial guesses, objective/constraint lambdas, sense information, options, callbacks, and optimization flags into the solve operator. | This is solve-block lowering; it is not proof that the `root` builtin is implemented there. |
| REA managed members `eficalculus-registry.json`, evidence `ev_3b4adf57521cccaa8dc25dda11b6df831085beabfd7d329d6f6f1927ff2dbde0` | `EfiCalculus.values.op_deriv_1` token `0x0600000d`, `op_deriv_0` `0x0600000e`, and partial/fixed-order wrappers call `Efi.Calculus.NthDerivative`. `op_integral_0` `0x0600000f` calls `Efi.Calculus.Integrate`. | Wrappers expose the dispatch boundary, not the native algorithm bodies. Tokens are valid only for the recorded build. |
| REA `eficalculus-attributes.json` plus SHA-verified decoded ModuleInterface payload `eficalculus-interface.txt` | Integral ABI takes a dimensionless tolerance, function callback, compatible bounds, and method option. Its unit-generic result multiplies the input-variable and function-result dimensions. It exposes fixed derivative orders 0–5, partial derivatives, indefinite integration and limits. | Public declarations do not prove behavior of unsupported forms or exact numeric thresholds. |
| REA `efifitting-attributes.json`, evidence `ev_96e1df861e33808a49b4d65a0cbada2129599c5da9e7ed07d4231e808c5ee987`, ModuleInterface token `0x0c000003`, blob SHA256 `e0644d6941ff4fbd323eeffa4925ee45eb41b8446e04a3a8ae5141e2a3d26c94` | SHA-verified decoded interface places scalar root functions in `Efi.Fitting.root` and `root_bracket`, with a callback whose input and returned root share dimensions. Polynomial-root entry points are `polyroots`, `polyroots_0`, `polyroots_1`, `polyroots_2`; the general entry accepts an origin-related integer and returns dimensionless roots. | The native bodies and meaning of every method-selector entry point are not recovered. `op_rootn` in EfiCore is an nth-root operation and must not be confused with this solver. |
| REA `efisolver-registry.json`, evidence `ev_8dd7f40075a8d281d57480f5a84e31eb050fcae747424d170472ba03f9630036` | `op_find` `0x06000017` delegates to tuple solving/conversion; `op_solve` `0x0600001a` calls `Efi.Solver.Solve`. | Does not reconstruct find/minerr/optimization semantics. |

The calculus wrapper SHA256 is
`32e4f689ea0ab64cfc51630a998ec0691905c75bb3b885de2f0e2fb2b194b224`
(MVID `affb980e-00c1-4952-9a68-c12ad48a4ba2`). The fitting wrapper SHA256 is
`00828cda3b3c5cc02fb81c624a77c4044a074ebd3e9d3e44713c32e0aa506c38`
(MVID `58c25ca6-9747-4b45-b4e3-106b36ee14d9`).

`Help/en/PTC_Mathcad_Help` supplies additional shipped behavioral evidence.
`help-evidence.json` records source hashes and selected pages;
`help-mathml-evidence.json` retains the corresponding textarea XML and hashes.
The inspected root example uses legacy `label="Keyword"` XML. Modern
`labels="KEYWORD"` handling follows the current schema and the project's
identifier representation; legacy-help XML is not silently accepted as current
document XML.

## Recovered user-level contracts

- `finding_roots.html`: `root(expression, variable[, a, b])` uses an existing
  initial guess without brackets; the bracketed form requires real ordered
  bounds with opposite function signs and requires no preceding guess. Complex
  guesses permit complex roots. Ribbon insertion assigns the Keyword label.
  TOL values above `1e-5` do not loosen root convergence beyond that cap. The
  documented original methods are secant/Mueller and Ridder/Brent. A small
  residual can still give an inaccurate location for a flat function.
- The same page specifies polynomial coefficients in constant-first order,
  vector length 2–99, and a vector of roots. It documents Laguerre as the
  default method and companion-matrix solving as an alternative.
- `derivative_and_prime_operators.html`: numerical derivative and partial
  derivative orders are integers 0–5; a blank exponent means first derivative.
  The function is scalar-valued and may be complex. Original numerical
  differentiation uses a variation of Ridder's divided-difference method with
  several step sizes and an error table. Its stated accuracy degrades with
  derivative order. Prime/function-derivative representation is a separate
  documented form and is not implemented by this module.
- `integral_operator.html`: finite definite integration accepts scalar-valued,
  possibly complex functions; bounds are real and have compatible units.
  Multiple integrals are constructed by nesting operators. The help warns about
  sharp peaks and discontinuities. `numerical_integration_methods.html`
  describes the original adaptive method as Gauss quadrature; several other
  algorithms and endpoint/infinite-limit cases exist.
- `summation_operator.html`, `product_operator.html`, and
  `using_the_summation_and_product_operators.html`: explicit local bounds bind a
  variable only inside the operand; an external same-named variable, including
  one defined as 3 metres, is unaffected. Vector and existing-range forms also
  exist. **Original limits may be noninteger real numbers**, such as 0.4 and
  3.7. Their exact iteration rule was not recovered; integer-only handling in
  this implementation is a declared subset, not an original restriction.

## Implemented API and behavior

`crates/mathcraft-compat/src/numerics.rs` adds
`NumericalExpression::{Root,Derivative,Integral,Fold}` behind
`Expression::Numerical`. `parse_operator` runs before ordinary eager function
calls, so the root variable and expression are not evaluated before binding.
`evaluate`, `validate_names`, and `collect_range_names` integrate with the
existing worksheet scope, label-aware identity, function capture, and range
expansion. `is_builtin`/`call` provide the ordinary `FUNCTION`-labelled
`polyroots` dispatcher.

The implemented methods are independent Rust algorithms:

| Form | Current calculation | Explicit boundary |
| --- | --- | --- |
| `root(expr,x,a,b)` | Real bisection, endpoint checks, maximum 256 iterations, residual-based acceptance | No Ridder/Brent parity claim; compatible finite bounds and real samples required. A narrowed interval alone does not count as a root, preventing false success at a pole. |
| `root(expr,x)` | Complex-capable secant iteration from the bound initial value, maximum 128 iterations | No Mueller fallback or guarantee of the same selected root as Mathcad. |
| `derivative` / `partDerivative` | Independently generated centered nine-point finite-difference weights; up to 12 shrinking steps; successive estimates must stabilize; orders 0–5 | No copied coefficient tables, original Ridder error-table algorithm, prime operator, symbolic derivative, or accuracy guarantee matching Mathcad. |
| Finite `integral` with default/adaptive method | Iterative adaptive Simpson subdivision with error estimate/correction, real bounds, complex-valued samples, reversed bounds and nesting | This is not the documented original Gauss implementation. No singular/infinite endpoints, other method selections, or symbolic/indefinite integrals. Sampling cannot detect every missed peak or discontinuity. |
| Explicit-bound `summation` / `product` | Inclusive ascending integer iteration, cloned local binding, scalar complex quantities, propagated sum/product dimensions | Noninteger, existing-range and vector-operator forms return unsupported; reversed bounds fail. Equality of bounds is accepted as one term but was not original-runtime-verified. |
| `polyroots(coefficients)` | Independent complex Laguerre iteration, synthetic deflation and polishing against the original polynomial | No native bodies or PTC tables used; no companion-matrix selector, original root ordering, or ill-conditioned/repeated-root accuracy guarantee. |

Polynomial coefficients must be a column vector of 2–99 finite values with a
nonzero final coefficient and compatible common dimensions. Their first stored
element is always the constant term, so changing ORIGIN does not shift their
meaning. Common dimensions cancel; roots are dimensionless. This follows the
observed interface and mathematical representation but has not been compared
against new original-runtime results for dimensioned coefficients. Laguerre
tests cover real, complex, repeated and zero roots; acceptance checks the
scaled residual, not an assumed real-only answer.

Root and integral read SYSTEM-labelled TOL, defaulting to `0.001`; the root
solver applies the documented `1e-5` cap. The current independent tolerance
implementation accepts positive finite dimensionless TOL at most 1. Derivative
stability targets are independent implementation choices. Derivative dimensions
are function dimensions minus the order times the bound variable's dimensions;
integral dimensions add function and variable dimensions. Bounds and sample
dimensions are checked before returning a result.

Every numerical invocation uses a shared nested budget of 10,000 samples or
polynomial iterations and maximum nesting depth 8. Local folds additionally
check their term count before starting. Quadrature subdivision depth is capped
at 20; each Laguerre root/polish attempt is capped at 128 iterations. An RAII
guard releases nesting state on all failure paths, so an unsuccessful region
does not poison later regions. `numeric_domain` and `numeric_nonconvergence`
are project-defined errors, not recovered Mathcad error identifiers. Unknown
semantic attributes and unsupported forms fail explicitly.

## Validation and remaining work

`cargo test -p mathcraft-compat --test numerics --locked` passes 16 integration
tests through `evaluate_worksheet`, including unit propagation, scopes and
variable preservation, real/complex roots, derivative orders, blank degree,
partial differentiation, non-polynomial differentiation/integration, nested
integrals, ORIGIN independence, polynomial residuals, invalid inputs, work
limits, and recovery in the next region after budget exhaustion.

No result-cache value is used by these algorithms. These tests establish the
implemented subset's mathematical behavior; they do not establish complete
Mathcad numeric parity. Remaining work includes original-native numerical
algorithm recovery or an authorized headless reference corpus, tolerance and
error matching, root-selection/order cases, noninteger fold iteration,
existing-range/vector forms, alternate integration algorithms, prime operators,
solve blocks/find/minerr/optimization, and symbolic CAS.
