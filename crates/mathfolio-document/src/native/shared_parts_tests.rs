//! Private payload sharing tests; no source/archive is evaluated or executed.
use super::*;
use sha2::{Digest, Sha256};

const SHEET: &str = r#"<?xml version="1.0" encoding="utf-8"?><ws:worksheet xmlns:ws="http://schemas.mathsoft.com/worksheet50" xmlns:u="urn:unknown"><ws:regions><ws:region region-id="7" left="1" top="2"><u:widget payload="uninterpreted"/></ws:region></ws:regions></ws:worksheet>"#;
const RESULTS: &[u8] = br#"<resultsList xmlns="http://schemas.mathsoft.com/result10"><resultData result-id="9" calculation-status="Pending" tag="opaque"><future xmlns="urn:future">retained</future></resultData></resultsList>"#;
const OPAQUE: &[u8] = &[0, 255, 0, 31, 128, 70, 0];

fn archive(sheet: &[u8]) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .set_raw_comment(b"retained-comment".to_vec().into_boxed_slice())
        .unwrap();
    for (name, bytes, compression) in [
        ("mathcad/worksheet.xml", sheet, CompressionMethod::Deflated),
        ("mathcad/result.xml", RESULTS, CompressionMethod::Deflated),
        ("custom/private.bin", OPAQUE, CompressionMethod::Stored),
        ("custom/", &[][..], CompressionMethod::Stored),
    ] {
        writer
            .start_file(
                name,
                SimpleFileOptions::default()
                    .compression_method(compression)
                    .last_modified_time(
                        zip::DateTime::from_date_and_time(2020, 2, 3, 4, 5, 6).unwrap(),
                    )
                    .unix_permissions(0o640),
            )
            .unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}
fn fixture() -> NativePackage {
    NativePackage::from_bytes(&archive(SHEET.as_bytes())).unwrap()
}

#[test]
fn unchanged_payloads_share_but_modified_and_added_parts_are_branch_local() {
    let original = fixture();
    let first = original.clone();
    let mut branch = first.clone();
    for ((a, b), c) in original.parts.iter().zip(&first.parts).zip(&branch.parts) {
        assert!(Arc::ptr_eq(&a.bytes, &b.bytes));
        assert!(Arc::ptr_eq(&a.bytes, &c.bytes));
        assert_eq!(a.name, b.name);
        assert_eq!(a.compression, c.compression);
        assert_eq!(a.modified, b.modified);
        assert_eq!(a.unix_mode, c.unix_mode);
    }
    let replacement = vec![1, 2, 3, 4, 5];
    let incoming_ptr = replacement.as_ptr();
    branch.set_part("custom/private.bin", replacement).unwrap();
    assert_eq!(
        branch.original_part("custom/private.bin").unwrap().as_ptr(),
        incoming_ptr
    );
    assert_eq!(original.original_part("custom/private.bin"), Some(OPAQUE));
    assert_eq!(first.original_part("custom/private.bin"), Some(OPAQUE));
    assert_eq!(
        branch.original_part("custom/private.bin"),
        Some(&[1, 2, 3, 4, 5][..])
    );
    for (a, b) in original.parts.iter().zip(&branch.parts) {
        assert_eq!(
            Arc::ptr_eq(&a.bytes, &b.bytes),
            a.name != "custom/private.bin"
        );
    }
    branch.set_part("custom/new.bin", vec![9, 8, 7]).unwrap();
    assert!(original.original_part("custom/new.bin").is_none());
    assert!(first.original_part("custom/new.bin").is_none());
    let mut later = branch.clone();
    later.set_part("custom/new.bin", vec![6]).unwrap();
    assert_eq!(branch.original_part("custom/new.bin"), Some(&[9, 8, 7][..]));
}

#[test]
fn zip_output_matches_vec_backed_reference_and_preserves_exact_part_metadata() {
    let input = archive(SHEET.as_bytes());
    // Windows ZIP permission encoding need not equal the builder's requested
    // Unix mode. Preserve the mode actually stored in the independent input.
    let observed_mode = ZipArchive::new(Cursor::new(&input))
        .unwrap()
        .by_name("custom/private.bin")
        .unwrap()
        .unix_mode();
    let original = NativePackage::from_bytes(&input).unwrap();
    let copy = original.clone();
    let legacy = LegacyPackage::from_package(&original);
    legacy.assert_same(&copy);
    let encoded = copy.to_bytes().unwrap();
    assert_eq!(encoded, legacy.to_bytes());
    let reopened = NativePackage::from_bytes(&encoded).unwrap();
    legacy.assert_same(&reopened);
    assert_eq!(
        original.caches().unwrap()[0].calculation_status.as_deref(),
        Some("Pending")
    );
    let mut zip = ZipArchive::new(Cursor::new(encoded)).unwrap();
    assert_eq!(zip.comment(), b"retained-comment");
    let opaque = zip.by_name("custom/private.bin").unwrap();
    assert_eq!(opaque.compression(), CompressionMethod::Stored);
    assert_eq!(opaque.unix_mode(), observed_mode);
}

#[test]
fn utf16_xml_change_detaches_only_selected_part_and_keeps_opaque_cache_bytes() {
    for little in [true, false] {
        let xml = SHEET.replace("utf-8", "utf-16");
        let mut raw = if little {
            vec![0xff, 0xfe]
        } else {
            vec![0xfe, 0xff]
        };
        for unit in xml.encode_utf16() {
            raw.extend(if little {
                unit.to_le_bytes()
            } else {
                unit.to_be_bytes()
            });
        }
        let original = NativePackage::from_bytes(&archive(&raw)).unwrap();
        let mut changed = original.clone();
        let replacement = xml.replace("uninterpreted", "새 값 😀");
        crate::export::set_xml(&mut changed, "mathcad/worksheet.xml", replacement.clone()).unwrap();
        assert_eq!(
            original.original_part("mathcad/worksheet.xml"),
            Some(raw.as_slice())
        );
        assert_eq!(changed.worksheet_xml().unwrap(), replacement);
        let changed_bytes = changed.original_part("mathcad/worksheet.xml").unwrap();
        assert!(changed_bytes.starts_with(&raw[..2]));
        for (a, b) in original.parts.iter().zip(&changed.parts) {
            assert_eq!(
                Arc::ptr_eq(&a.bytes, &b.bytes),
                a.name != "mathcad/worksheet.xml"
            );
        }
        let cold = NativePackage::from_bytes(&changed.to_bytes().unwrap()).unwrap();
        assert_eq!(
            cold.original_part("mathcad/worksheet.xml"),
            Some(changed_bytes)
        );
        assert_eq!(cold.original_part("mathcad/result.xml"), Some(RESULTS));
        assert_eq!(cold.original_part("custom/private.bin"), Some(OPAQUE));
    }
}

#[test]
fn rejected_mutations_preserve_bytes_sharing_and_both_package_branches() {
    let original = fixture();
    let mut changed = original.clone();
    let before = changed.to_bytes().unwrap();
    for bytes in [
        b"<broken>".to_vec(),
        b"<!DOCTYPE a [<!ENTITY x 'bad'>]><a>&x;</a>".to_vec(),
        vec![0xff, 0xfe, 0],
    ] {
        assert!(changed.set_part("mathcad/worksheet.xml", bytes).is_err());
        assert_eq!(changed.to_bytes().unwrap(), before);
        for (a, b) in original.parts.iter().zip(&changed.parts) {
            assert!(Arc::ptr_eq(&a.bytes, &b.bytes));
        }
    }
}

#[test]
fn independently_forged_archives_remain_rejected_before_payload_publication() {
    let mut duplicate = archive(SHEET.as_bytes());
    // Same-width filename substitution in both local/central records.
    let from = b"custom/private.bin";
    let to = b"mathcad/result.xml";
    assert_eq!(from.len(), to.len());
    let positions: Vec<_> = duplicate
        .windows(from.len())
        .enumerate()
        .filter_map(|(i, w)| (w == from).then_some(i))
        .collect();
    assert_eq!(positions.len(), 2);
    for at in positions {
        duplicate[at..at + to.len()].copy_from_slice(to);
    }
    assert!(
        NativePackage::from_bytes(&duplicate)
            .unwrap_err()
            .to_lowercase()
            .contains("duplicate")
    );
    let mut oversized = archive(SHEET.as_bytes());
    let central = oversized
        .windows(4)
        .position(|w| w == b"PK\x01\x02")
        .unwrap();
    oversized[central + 24..central + 28]
        .copy_from_slice(&((MAX_PART_BYTES + 1) as u32).to_le_bytes());
    assert!(
        NativePackage::from_bytes(&oversized)
            .unwrap_err()
            .contains("size exceeds")
    );
    let dtd = archive(b"<!DOCTYPE ws [<!ENTITY secret 'inert'>]><worksheet xmlns='http://schemas.mathsoft.com/worksheet50'>&secret;</worksheet>");
    assert!(NativePackage::from_bytes(&dtd).unwrap_err().contains("DTD"));
}

// Test-only representation of the pinned old Clone and ZIP write data paths.
#[derive(Clone)]
struct LegacyPart {
    name: String,
    compression: CompressionMethod,
    bytes: Vec<u8>,
    modified: Option<zip::DateTime>,
    unix_mode: Option<u32>,
}
#[derive(Clone)]
struct LegacyPackage {
    source: PathBuf,
    parts: Vec<LegacyPart>,
    worksheet_part: usize,
    archive_comment: Vec<u8>,
}
impl LegacyPackage {
    fn from_package(package: &NativePackage) -> Self {
        Self {
            source: package.source.clone(),
            worksheet_part: package.worksheet_part,
            archive_comment: package.archive_comment.clone(),
            parts: package
                .parts
                .iter()
                .map(|p| LegacyPart {
                    name: p.name.clone(),
                    compression: p.compression,
                    bytes: p.bytes().to_vec(),
                    modified: p.modified,
                    unix_mode: p.unix_mode,
                })
                .collect(),
        }
    }
    fn assert_same(&self, package: &NativePackage) {
        assert_eq!(self.source, package.source);
        assert_eq!(self.worksheet_part, package.worksheet_part);
        assert_eq!(self.archive_comment, package.archive_comment);
        assert_eq!(self.parts.len(), package.parts.len());
        for (a, b) in self.parts.iter().zip(&package.parts) {
            assert_eq!(a.name, b.name);
            assert_eq!(a.compression, b.compression);
            assert_eq!(a.bytes, b.bytes());
            assert_eq!(a.modified, b.modified);
            assert_eq!(a.unix_mode, b.unix_mode);
        }
    }
    fn to_bytes(&self) -> Vec<u8> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .set_raw_comment(self.archive_comment.clone().into_boxed_slice())
            .unwrap();
        for part in &self.parts {
            let mut options = SimpleFileOptions::default().compression_method(part.compression);
            if let Some(modified) = part.modified {
                options = options.last_modified_time(modified);
            }
            if let Some(mode) = part.unix_mode {
                options = options.unix_permissions(mode);
            }
            writer.start_file(&part.name, options).unwrap();
            writer.write_all(&part.bytes).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }
}

#[test]
#[ignore = "manual readonly bounded profile; no timing threshold or GUI"]
fn profile_native_clone_readonly_input_cold_and_warm() {
    let path = std::env::var_os("MATHFOLIO_NATIVE_CLONE_INPUT")
        .expect("set a readonly frozen fixture path, e.g. LCC copy");
    let source = std::fs::read(path).unwrap();
    assert!(
        source.len() <= 64 * 1024 * 1024,
        "diagnostic input archive cap"
    );
    let started = std::time::Instant::now();
    let package = NativePackage::from_bytes(&source).unwrap();
    eprintln!(
        "NATIVE_CLONE_OBSERVE cold_candidate_ns={} archive_bytes={}",
        started.elapsed().as_nanos(),
        source.len()
    );
    eprintln!("NATIVE_CLONE_INPUT sha256={:x}", Sha256::digest(&source));
    let payload: usize = package.parts.iter().map(|part| part.bytes.len()).sum();
    assert!(
        payload <= 16 * 1024 * 1024,
        "diagnostic decoded payload cap"
    );
    let legacy = LegacyPackage::from_package(&package);
    legacy.assert_same(&package);
    // Static logical work, NOT allocation-profiler or measured byte counters.
    eprintln!(
        "NATIVE_CLONE_STATIC parts={} payload_bytes={payload} old_clone_payload_bytes_per_clone={payload} candidate_clone_payload_bytes_per_clone=0",
        package.parts.len()
    );
    let started = std::time::Instant::now();
    for _ in 0..32 {
        std::hint::black_box(legacy.clone());
    }
    eprintln!(
        "NATIVE_CLONE_OBSERVE legacy_clone_iterations=32 elapsed_ns={}",
        started.elapsed().as_nanos()
    );
    let started = std::time::Instant::now();
    for _ in 0..32 {
        std::hint::black_box(package.clone());
    }
    eprintln!(
        "NATIVE_CLONE_OBSERVE candidate_clone_iterations=32 elapsed_ns={}",
        started.elapsed().as_nanos()
    );
}
