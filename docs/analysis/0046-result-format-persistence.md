# Result-format persistence and precision-preserving saved inputs

Date: 2026-10-16. This document describes the independent Rust document model
and serializer. Original contracts are recorded separately in
[0045](0045-result-format-contract.md); the supplied PDF differences are in
[0043](0043-pdf-output-comparison.md). No original runtime or application UI was
used for this work.

## Model and inherited presentation

`MathFormat` now represents Percent alongside General, Decimal, Scientific and
Engineering. `ComplexFormat` represents `i`, `j`, polar radians, polar multiples
of pi and polar degrees. The model also retains trailing zeros, e notation,
engineering notation, exponential threshold and zero/complex thresholds.

New fields deserialize with defaults and are omitted from JSON when default.
A regression checks the exact previous serialized default object, so old
calculation signatures are not changed merely by adding the fields. Precision
remains 0..15; exponential threshold is 0..17 and zero/complex thresholds are
0..307, following the inspected schema. The larger integer types preserve the
schema's threshold range.

Public readers:

- `read_math_format_with_defaults(math, defaults)` inherits each absent local
  attribute from the supplied worksheet presentation.
- `read_presentation_math_format(presentation)` reads mathRendering formatting
  and its worksheet-namespace results. Point-valued `FontSize` is distinguished
  from DIP-valued mathStyle font-size.
- `read_worksheet_math_format_defaults(package)` reads the retained presentation
  part with bounded XML parsing and default values if the part is absent.
- `read_math_format(math)` remains a compatibility wrapper using default values.

An inherited default does not set `explicit`: an unchanged region should not
acquire local overrides just because its worksheet defines presentation defaults.

## Native XML output and preservation

Percent is a `resultFormat/percent` element, not a percent unit inserted into
the semantic expression. The five imaginary-value strings round-trip exactly.
Local numeric overrides are written separately from MathML.

Source-based export patches supported options individually and preserves
unrelated matrix settings, font options and native XML siblings. An unchanged
unsupported radix, fraction representation or `round-trip-decimal-places`
precision token is retained. This preservation is not an implementation of
fraction or nondecimal display semantics. If the user explicitly changes to
Percent, Scientific or Engineering, the schema fixes their radix to decimal;
the export therefore changes that attribute as required.

The observed Prime writer emits zero/complex/exponential thresholds for General,
and omits zero/complex thresholds for the other numeric styles. The common XSD
attributes nevertheless permit zero/complex thresholds there. Our serializer
retains explicitly nondefault supported values instead of losing them. General
`use-engineering-format` is also XSD-backed; the observed Prime reader/writer
does not use it. These are independent persistence decisions, not claims that
the observed original writer emits those attributes.

## Precision-preserving saved presentation inputs

`SavedCalculationRow.numeric_source` is optional. Its tagged values are
`native_mathml` and `evaluated_value`, containing respectively an inert native
literal result subtree or an independent evaluated-value JSON string. The
document layer stores and bounds these strings; it does not execute or evaluate
them. The application can use their original precision to apply result-format
changes without recalculating a worksheet or rounding an already rounded display.

Each string is limited to 4 MiB, and it counts toward the existing 8 MiB total
saved-presentation budget. Error rows cannot also carry a numeric source. Older
rows omit the field and deserialize as None. Existing presentation strings
remain available independently of this extra precision source.

## Local native evidence and verification

The ignored report
`artifacts/rea/pdf-comparison-20261016/native-result-format-examples.json`
contains SHA-256 identifiers and bounded snippets from the three inspected
local MCDX packages. All three have worksheet default imaginary-value `i`.
`0818.mcdx` region 54 and `detune.mcdx` regions 64, 70 and 83 contain actual
local `polarDegrees` overrides. None of those three packages contains a percent
result-format example. Their efficiency formulas that were found explicitly
multiply by 100, unlike assuming that they are the exact source of the supplied
1015 PDF. A 1015 source MCDX was not found in the scoped Desktop/Downloads file
name search, so a PDF-to-source region-ID match is not asserted.

Document regressions cover old JSON shape, inert numeric cache bounds,
per-attribute inheritance, all five complex styles with Percent, source-based
format conversion, unrelated native properties and unsupported round-trip and
fraction preservation. Retained focused log:
`artifacts/rea/pdf-comparison-20261016/document-format-tests.log`.

Verification completed: 49 document tests passed, and document Clippy with all
targets and warnings denied passed. The seven captured independent MCDX files,
including the new Percent/complex-format export, passed all 28 schema checks
across worksheet, header, footer and presentation parts. Reproducible ignored
validator and report:
`artifacts/rea/pdf-comparison-20261016/validate-format-exports.py` and
`format-native-schema-validation.json` in the same directory. Validation checks
the installed schema contract; it does not claim runtime reopening in Prime.

Final validation: all seven captures were regenerated from the final source
after the validation cutoff, including the narrow fraction/common-attribute
hardening and removal of `use-mixed-numbers` on explicit format conversion.
All 49 document tests passed again, followed by all 28 schema checks. Retained
final logs are `document-format-tests-final.log` and
`format-native-schema-validation-final.log` in the evidence directory. The JSON
report records each capture SHA-256 and the three document-source SHA-256 values
at validation. Final `export.rs` SHA-256:
`994bd4fafe818b5aebaecab12f5447d28649873eb6d3a0576237cf97d89438a1`.
Final integrated Windows/Linux checks are reported by the parent integration
work, separately from this focused checkpoint.
