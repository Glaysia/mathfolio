# Native cache persistence: exact JSON floating-point roundtrip

## Reproduction and cause

After the specification-table integration passed its earlier 452-test Windows
and Linux checks, the real 1015/1016 save/reopen probe exposed another defect.
Editing one table cell and calculating produced 80 synchronized cached rows.
The `.mathcraft` file contained those rows, but reopening accepted none of them
and left all 80 pending. The earlier source archive
`mathcraft-spec-table-validation-20261016.tar`, SHA
`11c398ae4027cc8fdc849effeaedcd867151a5da0f553da100d75fee2feba486`,
is the pre-fix validation snapshot; its passing tests did not cover
this failure. Final post-fix validation must use a new source snapshot.

The calculation presentation signature includes the serialized region source
and its geometric coordinates. Native coordinates contain nontrivial fractions.
The workspace used `serde_json 1.0.154` with its `default,std` features, without
`float_roundtrip`. Its fast floating-point decoder can round such a decimal to
an adjacent f64. Consequently, the program's own JSON save/load changed source
geometry bits, and recomputing the signature correctly rejected the cache.

Examples from the saved 1015 worksheet, compared against Rust's f64 decimal
parser:

| Serialized decimal | Original/standard parser f64 bits | Previous JSON decoder bits |
| --- | --- | --- |
| `907.0866141732283` | `408c58b162c58b16` | `408c58b162c58b17` |
| `1285.0393700787401` | `4094142850a14285` | `4094142850a14284` |
| `1530.7086614173227` | `4097ead5ab56ad5a` | `4097ead5ab56ad5b` |
| `1662.9921259842517` | `4099fbf7efdfbf7e` | `4099fbf7efdfbf7d` |
| `1965.3543307086613` | `409eb56ad5ab56ad` | `409eb56ad5ab56ac` |

These values occur in actual saved region coordinates, including regions
49, 64, 75, 80 and 93. The defect also affects some stored numerical values;
it is not a table-specific metadata or disabled-region migration issue.

Retained evidence is under `artifacts/rea/spec-table-editing-20261016/`:
the tiny independent `cache_float_probe.rs`, `cache-float-baseline.txt`, the
red regression result `cache-float-regression-before.txt`, and copies of the
pre-fix generated `.mathcraft` files. Their pre-fix SHA-256 values are:

- 1015: `77c7e2ed7f25e901ad78c096a7009b8832ce68dff70841abed43590e4f82645b`
- 1016: `c0e3525f25950fc9dc895d19d75a6f9f175fcd7891a5f05c48239a34457ff061`

## Minimal correction and verification boundary

The workspace dependency now enables `serde_json`'s `float_roundtrip` feature.
The installed crate declares that feature without additional dependencies;
the lockfile and document schema need no change. No coordinates are rounded,
no saved signatures are rewritten to bypass verification, and mismatched caches
remain invalid. The disabled-region migration and signature algorithms are
unchanged.

The new document regression saves a document with all five actual fractional
coordinates and synchronized presentation rows, then loads it from disk. It
asserts exact left/top f64 bit equality, unchanged active and saved signatures,
the complete preserved cache, and equality of the full document. Before the
fix, the first assertion fails by one ULP. The focused Windows release test
passes after the feature correction (one passed, 61 filtered out), with its
log retained as `cache-float-fixed-test.log`. The application's real 1015/1016
save/reopen probes are recorded by the final integration verification. This
document regression and the separate input-table status regression extend the earlier 452-test scope to 454 passing workspace tests on both Windows and peets; final integration details are recorded in CONFORMANCE.md.

The separate input-table cache classification correction is owned by the
application integration: synchronized native column references with no displayed
payload are legitimate for input tables. It must require unchanged source before
restoring those statuses. That correction is distinct from exact JSON decoding
and must not be inferred from this document regression.
