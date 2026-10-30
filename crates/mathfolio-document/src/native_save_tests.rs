use super::*;
use crate::{
    canvas::{CanvasContent, CanvasDocument},
    export,
};
use mathfolio_editor::{NodeKind, PortableTree};
struct Dir(PathBuf);
impl Dir {
    fn new() -> Self {
        static ID: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "mathfolio-native-save-proposal-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
    fn count(&self) -> usize {
        fs::read_dir(&self.0).unwrap().count()
    }
}
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn doc(value: &str) -> CanvasDocument {
    let mut doc = CanvasDocument::default();
    doc.add_math(20., 20.);
    doc.regions[0].content = CanvasContent::Math(PortableTree {
        kind: NodeKind::Number(value.into()),
        children: vec![],
    });
    doc
}

#[test]
fn generated_prepared_roundtrip_and_new_write_never_replaces_existing() {
    let dir = Dir::new();
    let doc = doc("2");
    let prepared = export::prepare_mcdx(&doc).unwrap();
    assert!(prepared.bytes().len() <= MAX_NATIVE_SAVE_BYTES);
    assert!(
        NativePackage::from_bytes(prepared.bytes())
            .unwrap()
            .worksheet_xml()
            .unwrap()
            .contains(">2</ml:real>")
    );
    let target = dir.path("fresh.mcdx");
    let revision = prepared.write_new(&target).unwrap();
    assert_eq!(revision, DiskRevision::read(&target).unwrap());
    assert_eq!(revision, prepared.revision());
    let old = fs::read(&target).unwrap();
    assert!(prepared.write_new(&target).is_err());
    assert_eq!(fs::read(&target).unwrap(), old);
    assert!(export::export_mcdx(&doc, &target).is_err());
    assert!(prepared.write_new(&dir.path("bad.mathfolio")).is_err());
    assert_eq!(dir.count(), 1);
}

#[test]
fn explicit_repeated_native_replace_preserves_wrappers_opaque_parts_and_cache_policy() {
    let dir = Dir::new();
    let mut document = doc("2");
    let generated = export::prepare_mcdx(&document).unwrap();
    let mut package = NativePackage::from_bytes(generated.bytes()).unwrap();
    let source = package.worksheet_xml().unwrap().replace(
        "</math>",
        "<resultFormat><general/></resultFormat><formattingOverride FontSize=\"14\"/></math>",
    );
    let utf16 = std::iter::once(0xff)
        .chain(std::iter::once(0xfe))
        .chain(source.encode_utf16().flat_map(u16::to_le_bytes))
        .collect::<Vec<_>>();
    let worksheet = package.worksheet_part_name().to_owned();
    package.set_part(&worksheet, utf16).unwrap();
    package
        .set_part("custom/opaque.bin", vec![0, 255, 17])
        .unwrap();
    let cache = package.caches().unwrap().into_iter().next().unwrap();
    let id = cache.result_id.unwrap();
    package.set_part(&cache.part,format!("<resultsList xmlns=\"http://schemas.mathsoft.com/result10\" xmlns:ml=\"http://schemas.mathsoft.com/math50\"><resultData result-id=\"{id}\" calculation-status=\"Synchronized\"><ml:result><ml:real>2</ml:real></ml:result></resultData></resultsList>").into_bytes()).unwrap();
    let original = package.to_bytes().unwrap();
    document.native_archive = Some(original.clone());
    document.regions[0].source_region = Some("1".into());
    let target = dir.path("session.mcdx");
    fs::write(&target, &original).unwrap();
    let mut expected = DiskRevision::from_bytes(&original).unwrap();
    let unchanged = export::prepare_mcdx(&document).unwrap();
    assert_eq!(
        NativePackage::from_bytes(unchanged.bytes())
            .unwrap()
            .original_part(&cache.part),
        package.original_part(&cache.part)
    );
    for value in ["5", "7"] {
        document.regions[0].content = CanvasContent::Math(PortableTree {
            kind: NodeKind::Number(value.into()),
            children: vec![],
        });
        let prepared = export::prepare_mcdx(&document).unwrap();
        expected = prepared.replace_expected(&target, &expected).unwrap();
        assert_eq!(expected, DiskRevision::read(&target).unwrap());
        let reopened = NativePackage::open(&target).unwrap();
        assert_eq!(
            reopened.original_part("custom/opaque.bin"),
            Some(&[0, 255, 17][..])
        );
        assert!(
            reopened
                .original_part(&worksheet)
                .unwrap()
                .starts_with(&[0xff, 0xfe])
        );
        assert!(reopened.worksheet_xml().unwrap().contains(
            "<resultFormat><general/></resultFormat><formattingOverride FontSize=\"14\"/>"
        ));
        assert!(
            reopened
                .worksheet_xml()
                .unwrap()
                .contains(&format!(">{value}</ml:real>"))
        );
        let caches = reopened.caches().unwrap();
        assert_eq!(caches[0].calculation_status.as_deref(), Some("Pending"));
        assert!(
            caches[0]
                .result_mathml
                .as_ref()
                .unwrap()
                .contains(">2</ml:real>")
        );
        assert_eq!(
            document.native_archive.as_deref(),
            Some(original.as_slice())
        );
        assert_eq!(dir.count(), 1);
    }
}

#[test]
fn conflict_absent_readonly_and_wrong_target_preserve_existing_bytes_and_no_temp() {
    let dir = Dir::new();
    let target = dir.path("target.mcdx");
    let prepared = export::prepare_mcdx(&doc("2")).unwrap();
    let revision = prepared.write_new(&target).unwrap();
    fs::write(&target, b"external change").unwrap();
    assert!(prepared.replace_expected(&target, &revision).is_err());
    assert_eq!(fs::read(&target).unwrap(), b"external change");
    assert!(
        prepared
            .replace_expected(&dir.path("absent.mcdx"), &revision)
            .is_err()
    );
    let expected = DiskRevision::read(&target).unwrap();
    let old_permissions = fs::metadata(&target).unwrap().permissions();
    let mut readonly = old_permissions.clone();
    readonly.set_readonly(true);
    fs::set_permissions(&target, readonly).unwrap();
    let result = prepared.replace_expected(&target, &expected);
    fs::set_permissions(&target, old_permissions).unwrap();
    assert!(result.is_err());
    assert_eq!(fs::read(&target).unwrap(), b"external change");
    fs::create_dir(dir.path("folder.mcdx")).unwrap();
    assert!(
        prepared
            .replace_expected(&dir.path("folder.mcdx"), &expected)
            .is_err()
    );
    assert_eq!(dir.count(), 2);
}

#[cfg(unix)]
#[test]
fn symlink_file_and_parent_rejected_without_touching_destination() {
    use std::os::unix::fs::symlink;
    let dir = Dir::new();
    let prepared = export::prepare_mcdx(&doc("2")).unwrap();
    let target = dir.path("real.mcdx");
    let revision = prepared.write_new(&target).unwrap();
    let old = fs::read(&target).unwrap();
    symlink(&target, dir.path("alias.mcdx")).unwrap();
    assert!(DiskRevision::read(&dir.path("alias.mcdx")).is_err());
    assert!(
        prepared
            .replace_expected(&dir.path("alias.mcdx"), &revision)
            .is_err()
    );
    fs::create_dir(dir.path("actual")).unwrap();
    symlink(dir.path("actual"), dir.path("linked")).unwrap();
    assert!(prepared.write_new(&dir.path("linked/new.mcdx")).is_err());
    assert_eq!(fs::read(&target).unwrap(), old);
}

#[test]
fn late_external_change_after_temp_sync_preserves_external_bytes_and_cleans_owned_temp() {
    let dir = Dir::new();
    let target = dir.path("late.mcdx");
    let prepared = export::prepare_mcdx(&doc("2")).unwrap();
    let revision = prepared.write_new(&target).unwrap();
    let result = prepared.replace_with_hook(&target, &revision, || {
        fs::write(&target, b"late external edit").unwrap();
    });
    assert!(result.is_err());
    assert_eq!(fs::read(&target).unwrap(), b"late external edit");
    assert_eq!(dir.count(), 1);
}

#[cfg(unix)]
#[test]
fn replacement_preserves_unix_mode_and_only_replaces_named_hardlink() {
    use std::os::unix::fs::PermissionsExt;
    let dir = Dir::new();
    let target = dir.path("mode.mcdx");
    let before = export::prepare_mcdx(&doc("2")).unwrap();
    let revision = before.write_new(&target).unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o640)).unwrap();
    let sibling = dir.path("same-inode.mcdx");
    fs::hard_link(&target, &sibling).unwrap();
    let after = export::prepare_mcdx(&doc("3")).unwrap();
    after.replace_expected(&target, &revision).unwrap();
    assert_eq!(
        fs::metadata(&target).unwrap().permissions().mode() & 0o777,
        0o640
    );
    assert_eq!(fs::read(&target).unwrap(), after.bytes());
    assert_eq!(fs::read(&sibling).unwrap(), before.bytes());
    assert_eq!(dir.count(), 2);
}

#[test]
fn existing_zip_writer_obeys_exact_capacity_and_fails_before_output_overrun() {
    let prepared = export::prepare_mcdx(&doc("2")).unwrap();
    let package = NativePackage::from_bytes(prepared.bytes()).unwrap();
    let exact = package.to_bytes().unwrap();
    assert_eq!(package.to_bytes_bounded(exact.len()).unwrap(), exact);
    assert!(package.to_bytes_bounded(exact.len() - 1).is_err());
    let mut sink = BoundedCursor::new(4);
    sink.write_all(b"abcd").unwrap();
    assert!(sink.write_all(b"e").is_err());
    sink.seek(SeekFrom::Start(1)).unwrap();
    sink.write_all(b"XY").unwrap();
    assert!(sink.seek(SeekFrom::End(2)).is_err());
    assert_eq!(sink.into_inner(), b"aXYd");
}

#[test]
fn preparation_unsupported_source_and_oversized_revision_fail_without_filesystem_output() {
    let dir = Dir::new();
    let mut unsupported = doc("2");
    unsupported.grid = 123.45;
    assert!(export::prepare_mcdx(&unsupported).is_err());
    assert!(DiskRevision::from_bytes(&vec![0; MAX_NATIVE_SAVE_BYTES + 1]).is_err());
    assert_eq!(dir.count(), 0);
}
