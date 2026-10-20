# Structured program, calculus, and symbolic editing

## Boundary

This milestone makes the already implemented Math50 program, numerical-calculus,
and bounded symbolic subsets editable as semantic trees. It does not claim
pixel-exact Prime layout or complete operator coverage. No Mathcad process was
started and no GUI, screenshot, or runtime probe was used.

## Static observations

| Evidence | Observation | Limit |
| --- | --- | --- |
| REA managed-member selection of `Ptc.EquationEditor.Tree.DefaultDescriptorTable`, complete within the selected type: parent evidence `ev_b32cc6da4661d8fd45ed0195ae5e7bbc03d79cab18a651a5aadb5917a763a0d5`, summary view `ev_0ecc5a2117d0115dc780a76ddd3c7a635e631b651864ce6772e5dc6eed6df91f`; `Ptc.EquationEditor.dll` SHA256 `d419032c0e99a62eab2f278e571a338d8c4b9662655af8ab462d482a028c0223`, MVID `d8627dca-3258-470c-b4d1-c00bbe2f2441` | The selected shipped type has 9 methods. Its file-backed CIL/user strings include descriptors whose XML names are `localDefine`, `symEval`, `summation`, `product`, `integral`, `derivative`, `partDerivative`, `program`, `while`, `for`, `return`, and `if`. The retained descriptor extraction binds these strings to build-local method token `0x06000bbb`. | Descriptor metadata establishes editor vocabulary and some static properties. It does not establish evaluation algorithms, exact rendering, caret transitions, or serialized child expansion by itself. Tokens are build-local. REA reported partial `#US` conformance because terminal flag bytes differ from its strict ECMA check; readable UTF-16 was retained. |
| `docs/analysis/0006-programming-semantics.md`, evidence P1-P8 | Math50 programs are ordered instruction blocks. Local assignment has two operands; `for` has variable/iterable/body; `while` has condition/body; `return` has one operand; `if` contains test/then and optional branches/else programs. | This milestone represents `if/then[/else]`; repeated `elseif` and `alsoif`, break, and continue remain outside the editor tree even though the evaluator understands them. |
| `docs/analysis/0007-numerics-semantics.md`, embedded Math50 schema SHA256 `a10afb4dd442a4f5dd77b6f1df903c1d2e5b6f4d6a701aae38b0cefee21f6ca3` | Calculus applications contain an operator, a lambda with one `boundVars` child and body, plus degree or lower/upper bound wrappers. | Schema shape does not establish numerical method or presentation geometry. |
| `docs/analysis/0012-symbolics-semantics.md`, schema/help and engine boundary evidence | `symEval` owns an input, zero or more command children, and an optional cached result. Symbolic evaluation is forbidden within a program. | Cached `symResult` is output state, not editable source. The independent engine remains a bounded polynomial subset. |

## Independent implementation policy

`mathcraft-editor::NodeKind` now has explicit nodes for `Program`,
`LocalDefine`, `If`, `For`, `While`, `Return`, `Integral`, `Derivative`,
`Summation`, `Product`, `SymbolicEvaluation`, and `SymbolicCommand`. Variable
child counts are carried in the node kind and validated against the child list.
Programs are limited to 1..=4096 statements and symbolic evaluations to 16
commands, matching existing independent evaluator budgets rather than asserting
Prime limits.

The editor exposes insertion templates. A selected/current expression becomes
the body or condition, and the remaining semantic slots are placeholders.
Generic leaf traversal supplies Tab cycling, whole-subtree selection, deletion,
and undo without a parallel caret model. Validation requires program bodies to
be `Program` nodes, targets/binders/keywords to be identifiers or incomplete
placeholders, and rejects symbolic evaluation below a program.

The compatibility bridge imports and exports native Math50 shapes. Calculus
uses child order `[body, variable, lower, upper]` or
`[body, variable, degree]` internally and reconstructs the lambda/bound wrapper
order at the boundary. Symbolic result caches are validated and omitted from
the editable source tree. Symbolic identifiers omit the presentation-only
`xml:space` attribute because the current bounded symbolic parser admits only
its own narrower identifier metadata contract.

The formula renderer provides source-node hit geometry for every operand. A
program uses an indented vertical line stack; control structures use keyword
rows and nested program boxes; calculus uses integral/sum/product glyphs with
bounds and an explicit binder; derivative and symbolic evaluation use compact
operator rows. These measurements and glyph choices are independent UI policy.

## Verification

Unit coverage checks:

- portable-tree validation rejects invalid program bodies, command shapes,
  counts, and forbidden program/symbolic nesting;
- insertion templates preserve tree validity and deterministic leaf order;
- program MathML round-trips and executes a local assignment plus return;
- integral, derivative, summation, and product trees round-trip through native
  lambda/bound MathML;
- symbolic evaluation round-trips and reaches the bounded symbolic engine;
- headless layout produces hit geometry for every source leaf, and placeholder
  cycling advances through nested program/calculus/symbolic structures.

No original-runtime comparison was performed.

## Unverified and deferred

- exact Prime visual metrics, multiline branch stacking, integral limit
  placement, derivative typography, keyword placement, focus painting, and
  pointer hit priorities;
- repeated `elseif`/`alsoif`, break/continue, local functions, indexed local
  assignment, indefinite integrals, limits, vector derivatives, and symbolic
  result-cache editing;
- exact Prime Tab order, insertion-key gestures, delete/unwrap policy for each
  new descriptor, and program line insertion/removal commands;
- preservation of presentation attributes such as symbolic arrow/equal style,
  hidden sides/keywords, local-assignment style, and integral algorithm hints.
