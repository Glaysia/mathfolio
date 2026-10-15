# Native solve blocks: structure, scope and bounded execution

Status: partial independently implemented numeric subset, 2026-10-15.
No original process, GUI, COM, screenshot or original worksheet execution was
used. These new solver answers have not been compared with an original-runtime
reference corpus.

## Recovered evidence

The embedded `Worksheet50.xsd` defines `region/solveblock/regions/region`.
Its child `solve-block-category` values are `guess-value`, `constraint`, and
`solver`; an absent category denotes a nonparticipant. A solver ordinarily
contains a definition or evaluation calling find/minerr/minimize/maximize or
odesolve. `solveblock` inherits the calculation-disable attribute. This is a
nested document structure, not a run of ordinary top-level equations delimited
by an invented Given keyword.

REA inspection of installed `Ptc.Controls.SolveBlock.dll` is retained in
`artifacts/rea/solve/solveblock-selected.json`, evidence
`ev_ab39ed8fd40ae00b28be688a58422659237043c4aee38a6fc024e688e2a3b633`.
Its SHA256 is
`b63acc2a682036e7c4eff95b99d05e122fb9c6d42decf4e73e2383d87de2a0f5`,
MVID `e609c2b1-e5a8-4161-83bd-26a7fd35fd40`. Static dncil decoding in
`solveblock-cil-selected.json` verifies this exact artifact hash before reading
the method bodies; it never loads the target assembly.

- `SolveBlockModel.get_CalculatableChildrenAboveSolver`, token `0x060000a3`,
  obtains the inherited calculatable sequence and calls Enumerable.TakeWhile.
  Its predicate `0x060000d4` compares the child with the selected Solver and
  negates equality.
- `IsChildAboveSolver`, `0x060000c0`, uses the same TakeWhile predicate followed
  by Contains. Its predicate `0x060000d7` has the same child-not-Solver test.
- `UpdateWhichItemIsSolver`, `0x060000b4`, selects via FirstOrDefault over
  calculatable children, then rewires/commits the selected aggregate. Exact
  movement/rewrap rules outside the subset have not been reconstructed.
- `SolverAggregate.UpdateSolverRL`, `0x060000e7`, constructs the solveblock
  lowering and normalizes only the solver name's initial character using
  ToLowerInvariant, then retains the rest of the name. This supports Find/find
  interchangeability without treating arbitrary identifier casing alike.

`lisp/solve.lisp` lines 574–629 and the EfiSolver wrapper interface retain the
native solve boundary: options, TOL, CTOL, objective and constraint callbacks,
initial unknown tuple, senses, formatting, callback and optimization flags.
They do not expose the native numerical implementation. These artifacts and
their hashes are also recorded in analysis 0007.

Shipped local help pages and ten native XML examples are retained, with source
hashes, in `artifacts/rea/solve/help-evidence.json` and
`help-mathml-evidence.json`:

- `about_solve_block_variables.html`: ordinary definitions inside the block
  remain local; a definition assigning the solver result is visible both inside
  and outside. Global definitions inside the block are not permitted.
- `about_guess_values.html` and `working_with_guess_values.html`: every unknown
  needs a guess; guesses may be external or local. Initial constraint values
  determine real/complex solution domain. Complex solving is an original
  capability, not an implemented capability of this subset.
- `system_of_equations_functions.html`: Find and Minerr require a solve block,
  return a scalar for one unknown and an ordered result vector for several.
  Arguments determine result order. It documents Levenberg–Marquardt and other
  solver choices. Minerr returns an approximate last iterate when convergence
  criteria cannot be met. Matrix unknowns, tuple outputs and parameterized
  block functions are original features.
- `about_solve_block_functions.html`: initial uppercase/lowercase solver names
  are interchangeable and are assigned the Keyword label. General minimize and
  maximize also have an outside-block form; symbolic block evaluation is not
  supported by the original.

## Implemented subset

`compat/src/solve.rs` exposes crate-private `parse_block(node, grid)`,
`evaluate(scope, block, region_id)`, `SolveBlock::exports()` and
`solver_result_ref()`. The parent evaluator integrates these through the actual
top-level native solveblock region, rather than flattening its children into
the worksheet scope.

Child regions use finite top/left coordinates and the supplied placement grid,
then stable geometric order. Only children preceding the selected solver enter
its aggregate. Ordinary definitions populate a cloned local scope. Explicit
scalar equality constraints retain separate operands, checked for compatible
dimensions. Category metadata must agree with the supported expression form.
Nonparticipating display evaluations are not treated as constraints. The single
selected solver's result definition is the only exported binding.

Find/find supports a square system of real scalar equalities. Minerr/minerr
supports at least as many scalar equalities as unknowns and returns an
approximate least-squares solution. Both use existing guesses, including
dimensioned quantities, while preserving the outer guesses. Output variables
retain their individual dimensions; several unknowns produce a column vector.
Explicit output display units use the existing display conversion path.

The numerical code is independently written: central finite-difference
Jacobians, pivoted elimination, damped line search for Newton steps and damped
normal equations for Minerr. It is not a recovered Mathcad algorithm. Residuals
are normalized using a common initial magnitude scale within each dimension
group; this scaling, damping, line-search policy and convergence thresholds
have not been verified against Mathcad. TOL/CTOL default to 0.001 and explicit
values must be finite, positive and dimensionless. Find checks the normalized
constraint residual against CTOL. Minerr also checks its independent gradient
and stagnation criteria and returns its last iterate at the iteration limit.

Work is bounded to 16 unknowns, 64 constraints, 256 child regions, 100 iterations,
16 line-search attempts per step and 10,000 residual-vector evaluations. Domain
and unit faults propagate. `solve_domain` and `solve_nonconvergence` are project
errors, not recovered original error identifiers. Unsupported metadata,
disabled blocks, inequalities, matrix/complex unknowns, tuple/parameterized
results, Minimize/Maximize and ODE solvers are explicitly unsupported.

A failed block poisons an earlier binding of a declared solver output. Even
when unknown execution metadata prevents normal parsing, a conservative scan
retains a recognizable output declaration for invalidation; it never executes
that metadata. This prevents stale successful answers from being read after a
failed recalculation. An output conflicting with a worksheet global is rejected
by the parent evaluator while preserving the global.

## Validation and limits

Eleven integration tests in `tests/solve.rs` pass through the actual nested
`evaluate_worksheet` path. They cover XML order differing from geometry, local
dependent guesses, linear/nonlinear systems, mixed units, solver-before/after
boundaries, external guess preservation, local-variable isolation, output
export, result reference propagation, approximate Minerr behavior, missing
guesses, invalid tolerances, inequality/complex/metadata rejection, disabled
block invalidation and global conflicts. The compatibility crate's all-target
Clippy check passes with warnings denied.

This is a runnable basic Find/Minerr block implementation. It does not establish
native numeric parity or complete solve-block editing and feedback: child
evaluation results below the solver, block error placement, solver choices,
complex/matrix solving, inequality constraints, parameterization, optimization
and exact native diagnostics remain unfinished. Test inputs are self-authored;
no original cached answer is used as truth.
