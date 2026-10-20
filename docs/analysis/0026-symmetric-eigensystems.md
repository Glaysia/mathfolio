# Mathcad Prime 12 symmetric eigensystems: static contract and bounded reconstruction

Status: static shipped-artifact investigation and independent Rust
implementation, 2026-10-15. No Mathcad process, CLR, COM, GUI, screenshot, or
original numerical engine was executed.

## Observations

| Evidence | Static observation | Boundary |
|---|---|---|
| `efimatrix.dll`, SHA256 `5f037dd885d2760f4af9dc1e000c2220d08c19ef109b0c163dfe79167365c2de`, MVID `61993031-02d1-47d5-a686-313f4f466ed2`; REA parent `ev_21f7f73e57f41b16c939ad649823f910075949cb91e105748efad3e3720a4a1f`, summary `ev_27fb1a7a4bff8c6ce6eaf2831b9396868f58a245d9d9bac4303a4a3410233b87` | The authenticated module interface declares `mc_eigenvals :: V {a}->V {a}` and `mc_eigenvecs :: V {a}->D {}->V {}`. Eigenvalues retain the matrix dimension; eigenvectors are dimensionless. | The interface fixes dispatch and dimension algebra, not numerical behavior. |
| `efimatrix_.dll`, SHA256 `0eda05cf6f4f67a46fba769143797c73f1a71b5a626bffddfebfda9ee9eee496`, MVID `f0915c22-a64f-4b58-ba5f-f200230b9210`; selected-type REA `ev_4b48fa18406545de354078b2a69f69191fbff0856a2aeee8063ad4805adedcdb` | `Efi.IntelMKL.eigenvals` token `0x060001c3`, IL SHA256 `5eda1289da8cf1ab9cefa2e33caf3182b7968ef84e6a866bbecec8a80fe63130`, requires a homogeneous scalar square matrix. It selects `zheev` for complex Hermitian, `zgeev` for other complex, `dsyev` for real symmetric, and `dgeev` for other real matrices. | Recovered CIL is not original source and was not executed. |
| Same selected-type REA | `Efi.IntelMKL.eigenvecs` token `0x060001c4`, IL SHA256 `33ed2b098408d43ac5ab62f60ef05e56156a386f39134fde5f1bdcdd050ea052`, defaults to right eigenvectors, accepts only `L` or `R`, requires a square matrix, and uses the same four-way matrix-class dispatch. | Mathcraft's current numeric value model has no string value, so this reconstruction exposes only the default one-argument call. |
| Exact `<Module>` selection `ev_e29aa520bbec56228be3ee4d25e07e8074f508a7db875b6287fffc827e0fe037` | `isSymRealMatrix` token `0x0600009d`, IL SHA256 `38bd6de1dbb66f7aab1d263ebd14038d44017494a44671338fb8e46488b5f8d0`, rejects imaginary data and compares each mirrored real pair exactly. `dsyev` token `0x060000a4`, IL SHA256 `d0104fa67b120fcda1c2f14f2778692c1a3279011d7e6bc6a9bb957c040769d3`, calls LAPACK, rejects nonconvergence, sorts its eigenvalue vector and rearranges eigenvector columns by the same permutation. | These anchors establish the symmetric branch and correspondence. They do not reveal a portable MKL rounding contract. |
| Installed `eigenvectors_and_eigenvalues.html`, SHA256 `d29149239f8cb2f8ffd851980402653c955fd80b742a09f3eda92d7ef3f16e85` | `eigenvals(M)` returns eigenvalues; column `n` of `eigenvecs(M)` corresponds to element `n` of `eigenvals(M)` and is normalized. Matrices may be real or complex. Real returned values are sorted from largest to smallest. Symmetric and general matrices use different algorithms, all backed by Intel BLAS/LAPACK. | Shipped help is behavioral documentation, not runtime evidence. |
| Installed `example_eigenvectors_and_eigenvalues.html`, SHA256 `25a5d341f1c089e830b3d232f5a0f0e5dbfe4d5d786ee2e857c625b81201af1a` | The shipped example treats eigenvalues and the corresponding eigenvector columns as an engineering workflow. | Cached example results were not treated as a newly executed oracle. |

## Supported inference and independent implementation

The shared facts support a narrow first slice: a finite, homogeneous, real,
exactly symmetric square matrix may use the symmetric eigensystem path.
`crates/mathcraft-compat/src/arrays.rs` therefore registers `eigenvals` and
`eigenvecs` for that input class. Values are sorted in descending order and the
same permutation is applied to the eigenvector columns. `eigenvals` preserves
the common physical dimension. `eigenvecs` returns normalized dimensionless
columns.

The implementation is independent Rust code. It scales the input, runs bounded
cyclic Jacobi rotations, and verifies every resulting eigenpair against the
normalized original matrix before returning it. The limit is 32 by 32 and 64
sweeps. Failure to converge or to pass the residual check is an explicit error;
an unfinished iterate is never returned as a successful result. This does not
claim MKL bit patterns, tolerances, or eigenvector signs.

General real matrices, complex Hermitian/general matrices, mixed-dimensional
matrices, matrices above the bound and the optional left-eigenvector string fail
explicitly. In particular, the general problem is not silently symmetrized and
a complex Hermitian matrix is not flattened into a real approximation.

## Verification and unknowns

`crates/mathcraft-compat/tests/arrays.rs` checks descending order, the
`A*v=lambda*v` relation for two- and three-dimensional non-diagonal matrices,
orthonormal columns, trace equality, physical eigenvalue dimensions,
dimensionless eigenvectors, native Math50 dispatch, and explicit rejection of
nonsymmetric, complex, mixed-unit and oversized inputs.

On 2026-10-15, `cargo test -p mathcraft-compat --locked` passed all 116 tests,
including the 18 array tests, and
`cargo clippy -p mathcraft-compat --all-targets --locked -- -D warnings`
completed without warnings.

Prime's general `dgeev`/`zgeev` behavior, complex Hermitian phase choices,
repeated-eigenspace basis selection, left eigenvectors, LAPACK tolerance and
rounding, large-matrix work limits and localized error mapping remain
unreconstructed.
