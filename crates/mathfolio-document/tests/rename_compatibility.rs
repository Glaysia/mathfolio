use mathfolio_document::{canvas::CanvasDocument, export::prepare_mcdx, native::NativePackage};

const LEGACY: &[u8] = include_bytes!("../../../tests/reference/legacy/pre-rename.mathcraft");

#[test]
fn pre_rename_calculation_cache_survives_both_extensions_without_recalculation() {
    let document = CanvasDocument::from_bytes(LEGACY).unwrap();
    let cache = document
        .saved_calculation_cache()
        .expect("legacy cache remains valid");
    assert_eq!(cache.rows.len(), 7);
    let signature = cache.source_signature.clone();
    let folder = std::env::temp_dir().join(format!("mathfolio-rename-{}", std::process::id()));
    std::fs::create_dir(&folder).unwrap();
    for extension in ["mathfolio", "mathcraft", "MATHCRAFT", "MATHFOLIO"] {
        let path = folder.join(format!("legacy.{extension}"));
        document.save(&path).unwrap();
        let reopened = CanvasDocument::load(&path).unwrap();
        assert_eq!(reopened, document);
        assert_eq!(
            reopened.saved_calculation_cache().unwrap().source_signature,
            signature
        );
        std::fs::remove_file(path).unwrap();
    }
    assert!(document.save(&folder.join("original.mcdx")).is_err());
    std::fs::remove_dir(folder).unwrap();
}

#[test]
fn newly_exported_native_metadata_uses_current_product_and_version() {
    let bytes = prepare_mcdx(&CanvasDocument::default()).unwrap();
    let package = NativePackage::from_bytes(bytes.bytes()).unwrap();
    let properties = package
        .parts()
        .iter()
        .find(|p| p.name == "docProps/app.xml")
        .unwrap();
    let xml = std::str::from_utf8(properties.bytes()).unwrap();
    assert!(xml.contains("<application>Mathfolio</application>"));
    assert!(xml.contains(concat!(
        "<appVersion>",
        env!("CARGO_PKG_VERSION"),
        "</appVersion>"
    )));
    assert!(!xml.contains("Mathcraft"));
}
