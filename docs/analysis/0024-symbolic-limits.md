# Mathcad Prime 12 symbolic limit boundary and finite polynomial subset

Status: static shipped-artifact investigation and independent Rust implementation,
2026-10-15. No Mathcad process, CLR, COM, GUI, screenshot, or original symbolic
backend was executed.

## Observations

| Evidence | Static observation | Boundary |
|---|---|---|
| `artifacts/rea/document-deep/schemas/Math50.xsd`, SHA256 `a10afb4dd442a4f5dd77b6f1df903c1d2e5b6f4d6a701aae38b0cefee21f6ca3`, extraction evidence `ev_733639bc33098dab0c4d11199cf3e2e1224402d74e9ca171f32c3355e310c704`, lines 1116–1136 | The Math50 `limit` operator has an optional `direction` attribute. Its only schema values are `left` and `right`; omission denotes a bidirectional limit. | Schema establishes serialized shape, not CAS results. |
| `eficalculus.dll`, SHA256 `32e4f689ea0ab64cfc51630a998ec0691905c75bb3b885de2f0e2fb2b194b224`, MVID `affb980e-00c1-4952-9a68-c12ad48a4ba2`; managed summary `ev_f1e0046c9c1b2c2d84bab0ae6b667f5523008870a63ff79be1d9acece830c487`, summary projection `ev_9711b271371bcd58b951c0f93ff13f156c1107d71847e8fc44ed35cbe3608e55` | The artifact is complete CIL metadata for `EfiCalculus`, without loading or executing it. | Metadata tokens below are build-local to this hash and MVID. |
| REA managed members `ev_3b4adf57521cccaa8dc25dda11b6df831085beabfd7d329d6f6f1927ff2dbde0` | `op_limitleft` `0x06000010`, `op_limitright` `0x06000011`, and `op_limit` `0x06000012` each load the same `#US` literal `limit symbolic operator only` and call `Mpl.Run.AppException.Raise`. | This proves the shipped managed numeric dispatch rejects these three paths. It does not recover the separate symbolic engine algorithm. |
| Installed `Help/en/PTC_Mathcad_Help/limit_operator.html`, SHA256 `0522e9d18ee2779ed236293b7b006b7f35f5166938e529a544596676e76b2343` | Limit is described as a symbolic operator, inserted with Ctrl+L. It has variable, target, direction and body placeholders; direction is right, left or empty/two-sided. The page explicitly says the numerical equals sign cannot evaluate limits. | Help text is shipped documentation, not runtime observation. |
| Same help page and `example_using_the_limit_operator.html`, SHA256 `4d43435ea5a0cb4f9bbbcacf1256b8da891b08f601360586c6ac8d36887cbf8b` | Embedded worksheet XML serializes a limit as `apply(limit, lambda(boundVars, body), target)` inside `symEval`. Examples include `sin(x)/x` at zero and a target labelled CONSTANT infinity. | Cached `symResult` examples are documentation payloads and were not used as executable oracle output. |

## Inference and design decision

The common exception body and the help restriction imply that Mathcraft must not
route `limit` through its numerical calculus evaluator. The independent
implementation therefore accepts it only inside the existing `symEval` parser.
For a polynomial at a finite exact rational target, direct substitution is an
exact mathematical result; left, right and two-sided forms necessarily agree.
That equality is a mathematical inference for this bounded domain, not a
recovered claim about the original CAS algorithm.

## Implemented subset

`crates/mathcraft-compat/src/symbolics.rs` now parses the native three-operand
shape, validates the optional direction, requires one simple bound variable,
and substitutes a finite exact rational target into a bounded polynomial. The
lambda variable is lexical: an outer worksheet value with the same name is not
consulted, including a unit-bearing value. Free identifiers continue to follow
the existing conservative worksheet substitution policy. Output remains a
namespaced `symResult`, and cached input results remain ignored.

This implementation deliberately rejects infinity, functions such as
`sin(x)/x`, rational expressions with variable denominators, removable poles,
piecewise limits, complex directions and malformed/unknown direction values.
It does not register a numeric limit operator or pretend a sampled value is a
symbolic limit. Resource bounds remain those in analysis 0012.

## Verification and unknowns

`cargo test -p mathcraft-compat --test symbolics` passes 9 tests. New cases
cover exact finite polynomial limits, all three direction encodings, lexical
shadowing by a unit-bearing worksheet variable, invalid direction, infinity
and a removable pole. These tests verify Mathcraft's independent contract; no
Prime result was executed or compared.

The original symbolic algorithm, branch and assumption rules, infinity
classification, removable-discontinuity simplification, transcendental limits,
complex limits, error categories and presentation ordering remain unknown.
