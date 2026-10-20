# Page header/footer regions and document serialization

This pass adds editable regular-page headers and footers to the independent
document model and exports them into Mathcad Prime's package structure. It is a
static reconstruction. Mathcad was not launched, loaded through CLR/COM, or
inspected through screenshots.

## Evidence ledger

| Evidence ID | Type and boundary | Observation |
| --- | --- | --- |
| REA `ev_7fbbf8302d0b0e9be32d8cf53f0ecb0c125892fa866fe41338bb21c559b2edb7` | Complete central-directory inventory of shipped `original-blank.mcdx`, SHA-256 `974c7e5db5a533e9666993350a996430f497147641158b1e185eeb292c57a7b4` | The package contains separate `mathcad/header.xml`, `mathcad/footer.xml`, `mathcad/settings/presentation.xml`, and root relationships. This establishes package membership, not application behavior. |
| REA `ev_fd14d985861c120dbd4fcc95771a2f9ee93650b4ae93e0670d62fc74a79688a8` | Static ZIP member inventory retained by the preceding analysis | Header and footer are 473-byte XML members (SHA-256 `9d2e7b8e...` and `633c080d...`); presentation is a 4718-byte XML member (SHA-256 `45c3637e...`). Coverage is complete for central-directory entries only. |
| `XSD-WS50-21bc9c62` | Installed `worksheet50.xsd`, SHA-256 `21bc9c62da5d026732dc4c0735583b29cb3ca2e4d2b19518e1fb197ee3dee543` | `header` and `footer` roots contain positioned `region` elements. Header/footer regions admit ordinary text and `fieldText`; fields include page number, save date, file name, and file path. Page-number and date template strings are closed enumerations. |
| `XSD-PRESENTATION10-592bec02` | Installed `presentation10.xsd`, SHA-256 `592bec026fda30702104035cf2fa36dd64025992db8973693737284d37ae69cc` | `pageModel` owns `starting-page-number`, `total-pages-offset`, and the flag for a distinct first-page header/footer. |

Static decoding of the inventoried blank package observes `<header><regions
/></header>` and `<footer><regions /></footer>`. Its root relationship part
points to both XML members. The presentation part has starting page 1 and total
pages offset 0. These are observations of one shipped blank document, not proof
that every Prime document has exactly the same optional parts or defaults.

## Priority decision

The worksheet schema also exposes an unbounded trace list, trace styling,
legends, axes, and four plot families. Native multi-trace serialization should
follow a multi-trace document model; emitting a large guessed plot subtree from
the current single-trace `PlotSpec` would produce weak compatibility. Rich text
formatting similarly lives inside nested XAML packages and requires run and
paragraph state that the current plain-string text region does not retain.

Regular page headers and footers were the smallest high-value document gap with
an exact schema contract and an observed package layout. Their free-positioned
region model also matches the page-layout evidence in analysis 0020. This makes
them the first implementation priority, followed by rich text state, then plot
serialization after the plot model gains multiple traces and axis/style data.

## Independent implementation

`CanvasDocument::page_sections` contains positioned header and footer regions,
starting-page number, and total-page offset. A region holds either plain text or
one of the four observed field families. Page number and save-date templates
are Rust enums mapped only to strings admitted by the installed schema.

Header/footer region IDs share the document's stable ID space and the same
coordinate, region-count, and total-text budgets as body regions. Adding a page
region marks the page sections explicit. Older version-2 `.mathcraft` files
lack this additive field; serde defaults them to source-owned, empty sections,
page 1, and offset 0.

When page sections are explicit, MCDX export replaces the `regions` collection
inside `header.xml` and `footer.xml`. Plain text uses the same bounded nested
XAML package and relationship mechanism as worksheet text. Dynamic fields are
written as `fieldText` with an empty cached `text` element and one schema-backed
field child. Export also updates the two page-numbering attributes in
`presentation.xml` while preserving unrelated attributes and elements.

Imported packages remain source-owned until an author explicitly edits page
sections. Body edits therefore retain original header/footer bytes, including
unknown content, comments, and formatting. This is a deliberate preservation
rule rather than an inference about Prime's edit state.

The Document ribbon now opens a modal page-section editor instead of an
inactive placeholder. Authors can add, reorder by deletion/reinsertion, edit
and position plain text plus page-number, saved-date, file-name and file-path
fields for both bands. Applying the draft is one document Undo step; cancelling
does not mutate the worksheet. The paper view repeats both bands on every page
and resolves page/file fields for preview. The saved-date preview intentionally
uses a descriptive placeholder because the document model does not yet retain
an authoritative save timestamp; MCDX export still writes the selected native
date field and format.

## Validation and remaining unknowns

Document tests cover save/reopen, old-v2 defaults, shared ID and text limits,
position validation, native text relationship round trips, all implemented
field families, page numbering, and byte-exact preservation of source-owned
headers. The generated XML is parsed by the package reader and is validated
separately against the installed worksheet and presentation schemas. These
checks establish structural conformance; they do not establish that Prime can
reopen or render every generated package.

Distinct first-page headers/footers remain unimplemented. The schema describes
container roots for them and presentation exposes the selection flag, but the
inspected blank package uses only singular regular header/footer parts. The
relationships and package naming produced by an actual first-page edit remain
unverified. Rich header/footer formatting, embedded math, images, and arbitrary
opaque page-region editing also remain outside this pass.
