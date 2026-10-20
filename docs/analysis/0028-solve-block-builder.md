# Safe native solve-block authoring model

Status: evidence-backed authoring layer over the existing bounded Find/Minerr
engine, 2026-10-15. No Mathcad process, CLR, COM, GUI, screenshot, or original
numerical engine was executed.

## Observed structure reused from Prime 12

This work does not infer a second solve-block format. It uses the static facts
already authenticated in [0011](0011-solve-semantics.md):

- The embedded `Worksheet50.xsd` declares
  `region/solveblock/regions/region`. Participating child regions carry
  `solve-block-category` values `guess-value`, `constraint`, or `solver`.
- `Ptc.Controls.SolveBlock.dll`, SHA256
  `b63acc2a682036e7c4eff95b99d05e122fb9c6d42decf4e73e2383d87de2a0f5`,
  MVID `e609c2b1-e5a8-4161-83bd-26a7fd35fd40`, REA Evidence
  `ev_ab39ed8fd40ae00b28be688a58422659237043c4aee38a6fc024e688e2a3b633`,
  shows that `get_CalculatableChildrenAboveSolver` token `0x060000a3`
  takes children only until the selected solver. `UpdateSolverRL` token
  `0x060000e7` lowers the selected solver and normalizes only the initial
  character of its name.
- Shipped help and ten native XML examples retained in
  `artifacts/rea/solve/help-evidence.json` and
  `help-mathml-evidence.json` establish local guesses, ordered Find/Minerr
  arguments, scalar versus column-vector results, and the optional exported
  solver definition. These are static installed artifacts; cached results were
  not rerun as an oracle.

The existing parser and evaluator in `compat/src/solve.rs` already execute this
nested structure. They remain the semantic authority for the implemented
subset. The builder described below only creates accepted input for that path;
it does not implement another solver.

## Public model and generated form

`compat/src/solve_builder.rs` exposes `SolveMethod`, `SolveBlockBuilder`,
`SolveBlockModel`, and a typed `SolveBuildError`. Callers add named local guess
variables and `PortableTree` guess expressions, add equality pairs as two
`PortableTree` expressions, select an ordered unknown list, optionally select
an exported result name, and seal the model with `build()`.

`to_solveblock_xml()` emits the native nested solve-block fragment. Guess,
constraint, and solver children receive monotonically increasing `top`
coordinates so geometric order equals the model order. The solver call uses a
`KEYWORD`-labelled `Find` or `Minerr`; unknown and output names use
`VARIABLE` labels. The solver math wrapper carries a stable `solve-result`
result reference. `to_worksheet_xml()` places one such block in a minimal
worksheet that can be passed directly to `evaluate_worksheet`.

All caller-provided expressions go through the existing
`editor_bridge::tree_to_mathml` serializer. Names are represented as
`AnnotatedIdentifier` trees and therefore pass the same validation and XML
escaping. The equality operator, worksheet/solveblock tags, categories,
coordinates and result reference are fixed library output. There is no method
that accepts a raw XML fragment. The completed fragment is parsed as XML and
passed through the existing crate-private native solve-block parser before it
is returned.

## Independent policy and unsupported boundary

The safe builder deliberately narrows the larger original product contract.
Every selected unknown must have exactly one local guess. Find must have one
equality per unknown; Minerr must have at least as many equalities as unknowns.
Names must be nonempty, bounded, and free of control characters. Duplicate
guesses/unknowns, placeholders, nested definitions/evaluations, nonfinite
worksheet coordinates and a result name that replaces a local guess are
rejected before execution.

External guesses, non-solver local definitions, inequalities, complex or
matrix unknowns, tuple and parameterized outputs, result-unit overrides,
no-display result definitions, Minimize/Maximize, Odesolve, solver algorithm
options, and editing an arbitrary preexisting solve block are not builder
features. Some are broader original capabilities and some are already partial
engine capabilities. The API does not erase or approximate them; callers get a
build error or must preserve their original XML outside this model.

## Verification

`tests/solve_builder.rs` constructs and executes a two-variable Find block,
checks native categories, keyword labels, ordered vector result, exported
definition and `resultRef`, and constructs an inconsistent one-variable Minerr
block whose supported least-squares result is 2. A boundary test rejects a
placeholder, missing guesses and a nonsquare Find model. It also uses an
identifier containing XML metacharacters, verifies escaped output and parses
the original identifier text back from the DOM, demonstrating that the model
does not provide markup injection.

These are self-authored regression cases for the independent compatibility
engine. They do not establish original numerical parity, original UI behavior,
or a complete Mathcad solve-block document editor.

On 2026-10-15, `cargo test -p mathcraft-compat --locked` passed all 119 tests,
including the three builder tests and the eleven native solve integration
tests. `cargo clippy -p mathcraft-compat --all-targets --locked -- -D warnings`
also completed without warnings.
