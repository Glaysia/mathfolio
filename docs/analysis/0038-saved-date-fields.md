# Saved-date fields in the worksheet and printed pages

2026-10-16. Static REA and headless tests only. No Mathcad process, CLR/COM,
GUI or screenshots were used.

## Original contract

`Ptc.Controls.dll` (`Ptc.Controls.EngineeringDocument` type), SHA-256
`4f6e1d7cbdeaf5752d3c02d6b70b74d34e9c4d7c4cc66497c90c49feebb905fd`,
REA evidence `ev_9b70b5ea25aee75f94f3de2d771e6f18c65568a5bdfcebf56dd709ee365a702b`:

- `OnFileForWorksheetOpened`, token `0x0600010e`, IL SHA-256
  `7b93a9740b0c4051ef4450805be4437ba7f9d930c19445a2a345edd20c1c0ba0`,
  reads `File.GetLastWriteTime(FilePath)` and sets `LastSavedDate`.
- `SavePrimeDocFormat`, token `0x06000110`, IL SHA-256
  `c5d8adddd4f2e1b4511017be072e175403ad40f1dce3aa240ea80aab141a3409`,
  updates the ordinary saved document's date with `DateTime.Today` after saving.
- `DisplaySavedDate`, token `0x060000b6`, normally returns `LastSavedDate`.
  A customized absolute existing file path can instead provide a customized
  date or its filesystem modification time. This separate custom-path feature
  remains outside the current document model.

`Ptc.Controls.Worksheet.dll`, SHA-256
`c1a70ec8007228ff119d4a31b36b9062132be3a10c771459c1dbe3fd0a7baddd`,
REA evidence `ev_ca6ee361e44f5b877e72b99111bddb37dcf24e4dd6163ce516a16f3ad2b2611b`:

- `SavedDateConverter.Convert`, token `0x060003e1`, IL SHA-256
  `ac2f574a988ba24133047cfe24319294fbbcb62f1ffc8015904052f92824c36a`,
  uses `DateTime.Now` when the nullable date is absent. Consequently a new,
  unsaved worksheet has a date; an empty placeholder would change the observed
  contract.
- `System_Short` and `System_Long` use the current culture's short/long date
  patterns. `yyyy_mm_dd` replaces underscores with hyphens. The other numeric
  templates replace underscores with date separators and `mm` with `MM`.

The decoder `artifacts/rea/date-saved-decode.py` verifies the installed DLL
hashes and retained REA IL hashes before producing `date-saved-cil.json` and
`date-saved-cil.txt`. It reads PE bytes without loading either assembly.
The inspected shipped blank document has no `dcterms:modified` in its core
properties. More importantly, the actual opening method reads filesystem time;
there is no evidence for prioritizing `docProps/core.xml` over that value.

## Independent implementation

`page_fields::SavedDate` precomputes the seven presentations. Opening a document
reads the actual input path's modification time, including MCDX imports whose
future save path is unset. A successful save updates the cache from the saved
file. A new document initializes a local current date, matching the null-date
fallback. Failed opens/saves leave the previous date unchanged.

The worksheet preview and immutable print snapshot share this cached date.
Painting, calculation, page iteration and background printing do not read file
metadata, invoke a shell, or consult the clock for this field. Print snapshots
also retain their date if the live document subsequently changes.

Windows uses the system timezone and `GetDateFormatEx` for the user's short and
long date formats, plus the system date separator for numeric templates. Unix
uses `localtime_r` and a per-call `newlocale`/`strftime_l` handle; the process-wide
locale is never changed. There is one existing-version `libc` dependency on
Unix and no additional Windows dependency.

## Validation and limits

Tests cover all seven format mappings, numeric date separators, local calendar
conversion, opening old-timestamp native/own files, continuing to display the
cached date after the source file disappears, updating after a successful save,
and finding the actual date text in headless print shapes. Workspace validation
is recorded in [CONFORMANCE](../CONFORMANCE.md).

This is not a full port of .NET globalization. Windows' user regional settings
may differ from Prime's separately selected `CurrentUICulture`; calendars other
than Gregorian are not reconstructed. POSIX exposes a short date pattern but
no portable long-date setting. Linux's long variant currently uses localized
weekday/month names in an independent weekday/day/month/year order, and its
numeric templates use slash separators. These cross-culture differences remain
partial implementation. The unsaved fallback is cached at document creation;
date changes across midnight without another open/save remain unverified.
