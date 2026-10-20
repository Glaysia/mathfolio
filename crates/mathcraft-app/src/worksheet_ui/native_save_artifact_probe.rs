//! Native Save functional observations using generated temporary sources only.
use super::*;
use sha2::{Digest, Sha256};

fn copy_snapshot(output: &Path, source: &Path, name: &str) -> serde_json::Value {
    let bytes = std::fs::read(source).unwrap();
    let destination = output.join(name);
    assert!(!destination.exists());
    std::fs::write(&destination, &bytes).unwrap();
    serde_json::json!({"name":name,"bytes":bytes.len(),"sha256":format!("{:x}",Sha256::digest(&bytes))})
}
fn observation(f: &Fixture, label: &str, generation: u64) -> serde_json::Value {
    assert!(f.app.last_executed_regions.is_empty());
    assert!(!f.app.background.running);
    assert_eq!(f.app.background.submitted_generation(), generation);
    let target = f.app.native_save_target.as_ref();
    serde_json::json!({
        "label":label,"plain":f.source(),"dirty":f.app.dirty,
        "path_name":f.app.path.as_ref().and_then(|p|p.file_name()).map(|p|p.to_string_lossy().into_owned()),
        "native_target_name":target.and_then(|t|t.path.file_name()).map(|p|p.to_string_lossy().into_owned()),
        "native_revision_sha256":target.map(|t|t.revision.sha256().iter().map(|b|format!("{b:02x}")).collect::<String>()),
        "native_revision_bytes":target.map(|t|t.revision.len()),
        "source_sha256":format!("{:x}",Sha256::digest(serde_json::to_vec(&f.app.document.regions).unwrap())),
        "loaded_archive_sha256":f.app.document.native_archive.as_deref().map(|b|format!("{:x}",Sha256::digest(b))),
        "undo":f.app.undo.len(),"redo":f.app.redo.len(),
        "submitted_generation":f.app.background.submitted_generation(),"executed_regions":0
    })
}

/// Expected red before the separately owned queue-order fix. Imported native
/// Text already uses rich-mode segmentation. Exercise BOTH own and newly saved
/// native Text whose live document has no native_archive; do not split deliveries.
#[test]
fn combined_text_then_ctrl_s_saves_new_source_in_same_full_app_delivery() {
    for native in [false, true] {
        let mut f = Fixture::new();
        f.app = WorksheetApp {
            automatic_calculation: false,
            ..Default::default()
        };
        f.text = f.app.document.add_text(38.4, 38.4, "앞😀뒤");
        f.app.selected = Some(f.text);
        f.app.focus_new = Some(f.text);
        f.app.insertion = None;
        f.ctx = egui::Context::default();
        crate::configure(&f.ctx);
        f.frame(vec![]);
        f.select(0, 1);
        let target = f.root.join(if native {
            "new-no-archive.mcdx"
        } else {
            "own-no-archive.mathcraft"
        });
        assert!(f.app.document.native_archive.is_none());
        assert!(
            f.app
                .save_to_destination(Some(target.clone()), SaveIntent::SaveAs),
            "{}",
            f.app.status
        );
        assert!(
            f.app.document.native_archive.is_none(),
            "First native Save must not fabricate a source rebase"
        );
        let generation = f.app.background.submitted_generation();
        let history = f.app.undo.len();
        f.frame(vec![
            egui::Event::Text("X".into()),
            key(egui::Key::S, egui::Modifiers::COMMAND),
        ]);
        assert_eq!(f.source(), "X😀뒤");
        f.assert_cold(&target, "X😀뒤");
        assert!(f.app.status.starts_with("Saved"), "{}", f.app.status);
        assert!(
            !f.app.dirty,
            "Text must be applied before Save, not afterward; native={native}"
        );
        assert_eq!(f.app.undo.len(), history + 1);
        if native {
            assert!(
                NativePackage::open(&target)
                    .unwrap()
                    .caches()
                    .unwrap()
                    .is_empty()
            );
        } else {
            assert!(
                CanvasDocument::load(&target)
                    .unwrap()
                    .native_archive
                    .is_none()
            );
        }
        assert_eq!(f.app.background.submitted_generation(), generation);
        assert!(f.app.last_executed_regions.is_empty());
    }
}

#[test]
#[ignore = "functional artifact output only; not a CPU profile"]
fn native_save_controller_artifact_probe() {
    let output = PathBuf::from(
        std::env::var_os("MATHCRAFT_NATIVE_SAVE_OUTPUT")
            .expect("Set MATHCRAFT_NATIVE_SAVE_OUTPUT to a fresh directory"),
    );
    assert!(!output.exists(), "Artifact output must be new");
    std::fs::create_dir_all(&output).unwrap();
    let mut f = Fixture::new();
    let baseline = f.app.document.native_archive.clone();
    let generation = f.app.background.submitted_generation();
    let initial = std::fs::read(&f.native).unwrap();
    let mut files = vec![copy_snapshot(
        &output,
        &f.native,
        "native-save-original.mcdx",
    )];
    let mut observations = vec![observation(&f, "native-open", generation)];

    f.frame(vec![egui::Event::Text("한".into())]);
    let history = f.app.undo.len();
    f.frame(vec![key(egui::Key::S, egui::Modifiers::COMMAND)]);
    assert!(f.app.status.starts_with("Saved"), "{}", f.app.status);
    assert_eq!(f.app.undo.len(), history);
    assert!(!f.app.dirty);
    f.assert_cold(&f.native, "한😀뒤");
    files.push(copy_snapshot(&output, &f.native, "native-save-first.mcdx"));
    observations.push(observation(&f, "same-path-first", generation));

    f.select(1, 1);
    f.frame(vec![egui::Event::Text("글".into())]);
    f.frame(vec![key(egui::Key::S, egui::Modifiers::COMMAND)]);
    assert!(f.app.status.starts_with("Saved"), "{}", f.app.status);
    f.assert_cold(&f.native, "한글😀뒤");
    files.push(copy_snapshot(&output, &f.native, "native-save-second.mcdx"));
    observations.push(observation(&f, "same-path-second", generation));
    let previous_target = std::fs::read(&f.native).unwrap();
    assert_eq!(
        previous_target,
        std::fs::read(output.join("native-save-second.mcdx")).unwrap()
    );

    let save_as = f.root.join("native-save-as-target.mcdx");
    assert!(
        f.app
            .save_to_destination(Some(save_as.clone()), SaveIntent::SaveAs),
        "{}",
        f.app.status
    );
    files.push(copy_snapshot(&output, &save_as, "native-save-as.mcdx"));
    observations.push(observation(&f, "save-as-native", generation));
    let previous_after_save_as = std::fs::read(&f.native).unwrap();
    assert_eq!(previous_after_save_as, previous_target);
    f.select(f.source().chars().count(), f.source().chars().count());
    f.frame(vec![egui::Event::Text("!".into())]);
    f.frame(vec![key(egui::Key::S, egui::Modifiers::COMMAND)]);
    assert!(f.app.status.starts_with("Saved"), "{}", f.app.status);
    f.assert_cold(&save_as, "한글😀뒤!");
    files.push(copy_snapshot(
        &output,
        &save_as,
        "native-save-as-edited.mcdx",
    ));
    observations.push(observation(&f, "save-as-followed-current", generation));
    let previous_after_edit = std::fs::read(&f.native).unwrap();
    assert_eq!(previous_after_edit, previous_target);

    let own = f.root.join("native-save-own.mathcraft");
    assert!(
        f.app
            .save_to_destination(Some(own.clone()), SaveIntent::SaveAs),
        "{}",
        f.app.status
    );
    assert!(f.app.native_save_target.is_none());
    assert!(CanvasDocument::load(&own).unwrap().native_archive.is_some());
    f.assert_cold(&own, "한글😀뒤!");
    files.push(copy_snapshot(&output, &own, "native-save-own.mathcraft"));
    observations.push(observation(&f, "own-roundtrip", generation));
    assert_eq!(f.app.document.native_archive, baseline);
    assert_eq!(
        std::fs::read(output.join("native-save-original.mcdx")).unwrap(),
        initial
    );

    let (rich_files, rich_observations) =
        crate::worksheet_ui::rich_text_integration_tests::native_save_rich_probe::capture(&output);
    files.extend(rich_files);
    observations.extend(rich_observations);
    assert_eq!(
        files
            .iter()
            .filter(|r| r["name"].as_str().unwrap().ends_with(".mcdx"))
            .count(),
        8
    );
    assert_eq!(
        files
            .iter()
            .filter(|r| r["name"].as_str().unwrap().ends_with(".mathcraft"))
            .count(),
        2
    );
    assert_eq!(files.len(), 10);
    let report = serde_json::json!({
        "original_runtime":false,"gui":false,"calculation_executions":0,
        "calculation_scope":"opened edit/save/cold workflow; fixture setup excluded",
        "fixture_setup_uses_explicit_rust_calculation":true,
        "native_files":8,"own_files":2,"source_archive_rebased":false,
        "original_snapshot_sha256":format!("{:x}",Sha256::digest(&initial)),
        "previous_target_snapshot":"native-save-second.mcdx",
        "previous_target_bytes":previous_target.len(),
        "previous_target_after_save_as_sha256":format!("{:x}",Sha256::digest(&previous_after_save_as)),
        "previous_target_after_edit_sha256":format!("{:x}",Sha256::digest(&previous_after_edit)),
        "original_snapshot_unchanged":true,"files":files,"observations":observations
    });
    std::fs::write(
        output.join("observation.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    println!(
        "NATIVE_SAVE_CONTROLLER native_files=8 own_files=2 original_runtime=false gui=false calculations=0"
    );
}
