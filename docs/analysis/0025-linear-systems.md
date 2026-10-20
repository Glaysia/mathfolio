# Mathcad Prime 12 `lsolve`: static contract and independent bounded solver

Status: static shipped-artifact investigation and independent Rust
implementation, 2026-10-15. No Mathcad process, CLR, COM, GUI, screenshot, or
original numerical engine was executed.

## Observations

| Evidence | Static observation | Boundary |
|---|---|---|
| `efimatrix.dll`, SHA256 `5f037dd885d2760f4af9dc1e000c2220d08c19ef109b0c163dfe79167365c2de`, MVID `61993031-02d1-47d5-a686-313f4f466ed2`; REA parent `ev_21f7f73e57f41b16c939ad649823f910075949cb91e105748efad3e3720a4a1f`, summary projection `ev_27fb1a7a4bff8c6ce6eaf2831b9396868f58a245d9d9bac4303a4a3410233b87` | Complete managed metadata contains the module interface carrier used by the calculation dispatcher. | The facade does not contain the native solver body. |
| SHA-verified `ModuleInterfaceAttribute` token `0x0c000003`, blob SHA256 `e0995bd604385e4b4e88656b33fa507aebfd13af7bf9d0344c2b456aa5735927`; decoded interface SHA256 `2857ea7c7a792e6e828f915065df287f186126664f66b9f2356d4d091835138d` | `mc_lsolve :: V {a}->V {b}->V {a-1 b}` dispatches to `[efimatrix_]Efi.IntelMKL::lsolve`. Thus the result dimension is right-hand dimension divided by coefficient dimension. | An interface signature establishes dispatch and dimension algebra, not numerical thresholds. |
| `efimatrix_.dll`, SHA256 `0eda05cf6f4f67a46fba769143797c73f1a71b5a626bffddfebfda9ee9eee496`, MVID `f0915c22-a64f-4b58-ba5f-f200230b9210`; selected-type parent `ev_ab9a491b51a9779153ea472ef1a75a614b968b30c6e5f26c194988199c4aa2e5`, method page `ev_b657a817e01572e6acee5e61dfc48cf844852dae55a24feb5d8f97161e6a70c9` | `Efi.IntelMKL.lsolve` token `0x060001c9` requires both operands to be matrices and equal row counts. Uniform scalar matrices route to `ScalarLsolve` `0x060001c7`; other physical matrices route to `CrazyLsolve` `0x060001c8`. | Recovered CIL is not original source and was not executed. |
| Call-edge page `ev_a144afaf8e44386d42d7131cc4e0c1f3dd752be4e58a0a6cb6b732883adec0fe` | `ScalarLsolve` calls `dgesv`/`zgesv` for square real/complex inputs and `dgelss`/`zgelss` for the rectangular branches. `CrazyLsolve` strips dimensions, calls the scalar solver, checks matrix dimension algebra, and reapplies the linear result dimension. | The linked LAPACK calls do not establish the exact `rcond`, rank decision, workspace, rounding or error mapping. |
| User-string page `ev_4cfc25c987153c6701306ef8ddfdd62d983fd497d6044a7b66bd2eea0e35a2ac` | The selected type retains errors for matrix requirement, size mismatch, non-linear-algebra matrices and unit mismatch. | A retained string alone does not prove which branch emits it; the CIL anchors above establish the relevant uses. |
| Installed `solving_linear_systems.html`, SHA256 `9d369df6e46718beadc5836b3f60a2520f78bbf491691fdb2c66af3304fb4f6a` | `lsolve(M,v)` solves `M*x=v`; `M` and `v` may be real or complex, `v` may be a vector or matrix with matching rows, square `M` must be nonsingular, and inconsistent rectangular systems return a least-squares solution. The page identifies LU plus Intel BLAS/LAPACK and compares the result with `geninv(M)*v`. | Shipped help is behavioral documentation, not runtime evidence. |
| Installed `example_solving_a_linear_system_of_equations.html`, SHA256 `091ece15da5258d7614b4a55a067c9a349e8a0ab0245818a092a8f1bde31a4a5` | The shipped example uses the named `lsolve` function as an alternative to a solve block. | Embedded cached results were not treated as a newly executed oracle. |

## Independent implementation

`crates/mathcraft-compat/src/arrays.rs` registers the FUNCTION-labelled name
`lsolve` and accepts a coefficient matrix plus vector/matrix right-hand side.
It supports real or complex square systems and overdetermined systems whose
coefficient matrix has full column rank. Multiple right-hand columns are solved
together. Uniform physical dimensions are retained as `dimension(b)-dimension(A)`.
The parser and worksheet dispatcher use the normal native Math50 function path.

The algorithm is new Rust code: two-pass modified Gram-Schmidt QR, complex
Hermitian projections, and triangular back substitution. It produces the
least-squares minimizer for the supported overdetermined domain. A scaled rank
threshold and explicit work bound prevent a deficient or excessive input from
being returned as a plausible solution. This does not claim the original
LAPACK rounding, pivoting, rank threshold, selected minimizer or bit pattern.

Underdetermined systems, rank-deficient matrices, mixed-dimensional linear
algebra matrices and non-matrix/range operands fail explicitly. Prime has a
`CrazyLsolve` path for compatible mixed physical matrices, but its complete
dimension-factorization contract has not been reconstructed, so Mathcraft does
not flatten those units or guess a result.

## Verification and remaining unknowns

`crates/mathcraft-compat/tests/arrays.rs` covers a square solve, matrix right
side (`A*x=I`), complex solve, physical `s/m` result, inconsistent
overdetermined least squares, rank deficiency, underdetermined input, row
mismatch, mixed dimensions and native Math50 worksheet dispatch. These are
mathematical regression tests of Mathcraft's independent implementation; the
installed Prime engine was not run.

On 2026-10-15, `cargo test -p mathcraft-compat --locked` passed all 112
compatibility tests, including the 15 array tests, and
`cargo clippy -p mathcraft-compat --all-targets --locked -- -D warnings`
completed without warnings.

Original `rcond`, rank-deficient/underdetermined result policy, near-singular
warning behavior, large-matrix limits, mixed-unit factorization details, exact
localized errors and LAPACK version-dependent rounding remain unknown.
