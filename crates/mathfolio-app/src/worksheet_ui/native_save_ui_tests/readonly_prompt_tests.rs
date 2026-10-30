//! Child of native_save_ui_tests: generated files + scripted prompts only.
use super::*;
use crate::worksheet_ui::native_save_ui::SavePrompts;

#[derive(Default)]
struct Scripted {
    answer: Option<bool>,
    destination: Option<Result<Option<PathBuf>, String>>,
    calls: Vec<&'static str>,
    paths: Vec<Option<PathBuf>>,
}
impl SavePrompts for Scripted {
    fn readonly_save_as(&mut self, path: &Path) -> bool {
        self.calls.push("question");
        self.paths.push(Some(path.to_owned()));
        self.answer.take().expect("Unexpected readonly question")
    }
    fn destination(&mut self, current: Option<&Path>) -> Result<Option<PathBuf>, String> {
        self.calls.push("destination");
        self.paths.push(current.map(Path::to_owned));
        self.destination
            .take()
            .expect("Unexpected destination prompt")
    }
}

struct Readonly {
    path: PathBuf,
    original: std::fs::Permissions,
}
impl Readonly {
    fn new(path: &Path) -> Self {
        let original = std::fs::metadata(path).unwrap().permissions();
        let mut permissions = original.clone();
        permissions.set_readonly(true);
        std::fs::set_permissions(path, permissions).unwrap();
        Self {
            path: path.to_owned(),
            original,
        }
    }
}
impl Drop for Readonly {
    fn drop(&mut self) {
        let _ = std::fs::set_permissions(&self.path, self.original.clone());
    }
}

#[derive(Debug, PartialEq)]
struct Before {
    document: Vec<u8>,
    path: Option<PathBuf>,
    target: Option<NativeTarget>,
    date: String,
    history: (usize, usize),
    dirty: bool,
    generation: u64,
    selected: Option<u64>,
    focus_new: Option<u64>,
    composition: Option<usize>,
    cursor: String,
    source_file: Vec<u8>,
}
fn snapshot(f: &Fixture) -> Before {
    Before {
        document: serde_json::to_vec(&f.app.document).unwrap(),
        path: f.app.path.clone(),
        target: f.app.native_save_target.clone(),
        date: f.app.saved_date.format(DateSavedFormat::YearMonthDay),
        history: (f.app.undo.len(), f.app.redo.len()),
        dirty: f.app.dirty,
        generation: f.app.background.submitted_generation(),
        selected: f.app.selected,
        focus_new: f.app.focus_new,
        composition: f
            .app
            .text_composition
            .as_ref()
            .map(|c| c as *const _ as usize),
        cursor: format!("{:?}", f.app.text_cursors),
        source_file: std::fs::read(&f.native).unwrap(),
    }
}
fn start_preedit(f: &mut Fixture) {
    f.frame(vec![egui::Event::Ime(egui::ImeEvent::Enabled)]);
    f.frame(vec![egui::Event::Ime(egui::ImeEvent::Preedit {
        text: "한글".into(),
        active_range_chars: Some(0..2),
    })]);
    assert!(f.app.text_composition.is_some());
}

#[test]
fn readonly_decline_and_save_as_cancel_precede_ime_editor_cache_and_history_mutation() {
    for allow in [false, true] {
        let mut f = Fixture::new();
        f.frame(vec![egui::Event::Text("R".into())]);
        f.app.undo();
        f.frame(vec![]);
        f.select(0, 1);
        assert!(!f.app.redo.is_empty());
        start_preedit(&mut f);
        let _readonly = Readonly::new(&f.native);
        let before = snapshot(&f);
        let mut prompts = Scripted {
            answer: Some(allow),
            destination: if allow { Some(Ok(None)) } else { None },
            ..Default::default()
        };
        assert!(!f.app.save_with_prompts(false, &mut prompts));
        assert_eq!(snapshot(&f), before);
        assert_eq!(
            prompts.calls,
            if allow {
                vec!["question", "destination"]
            } else {
                vec!["question"]
            }
        );
        assert!(
            prompts
                .paths
                .iter()
                .all(|p| p.as_deref() == Some(f.native.as_path()))
        );
        assert!(f.app.text_composition.is_some());
        // The same original selected source still owns the later IME commit.
        f.frame(vec![egui::Event::Ime(egui::ImeEvent::Commit("완".into()))]);
        assert!(f.source().contains('완'));
    }
}

#[test]
fn readonly_affirmative_native_save_as_then_next_current_preserves_original_authority() {
    let mut f = Fixture::new();
    f.frame(vec![egui::Event::Text("R".into())]);
    let source = f.source().to_owned();
    let baseline = f.app.document.native_archive.clone();
    let original = std::fs::read(&f.native).unwrap();
    let history = (f.app.undo.len(), f.app.redo.len());
    let _readonly = Readonly::new(&f.native);
    let destination = f.root.join("chosen-native.mcdx");
    let mut prompts = Scripted {
        answer: Some(true),
        destination: Some(Ok(Some(destination.clone()))),
        ..Default::default()
    };
    assert!(
        f.app.save_with_prompts(false, &mut prompts),
        "{}",
        f.app.status
    );
    assert_eq!(prompts.calls, vec!["question", "destination"]);
    assert_eq!(f.app.path.as_deref(), Some(destination.as_path()));
    assert_eq!(f.app.native_save_target.as_ref().unwrap().path, destination);
    assert_eq!(f.app.document.native_archive, baseline);
    assert_eq!((f.app.undo.len(), f.app.redo.len()), history);
    assert!(!f.app.dirty);
    assert_eq!(std::fs::read(&f.native).unwrap(), original);
    f.assert_cold(&destination, &source);
    f.select(0, 1);
    f.frame(vec![egui::Event::Text("S".into())]);
    let changed = f.source().to_owned();
    let mut none = Scripted::default();
    assert!(
        f.app.save_with_prompts(false, &mut none),
        "{}",
        f.app.status
    );
    assert!(none.calls.is_empty());
    f.assert_cold(&destination, &changed);
    assert_eq!(std::fs::read(&f.native).unwrap(), original);
    assert_eq!(f.app.document.native_archive, baseline);
}

#[test]
fn readonly_affirmative_own_save_as_then_current_is_an_explicit_format_transition() {
    let mut f = Fixture::new();
    f.frame(vec![egui::Event::Text("R".into())]);
    let original = std::fs::read(&f.native).unwrap();
    let baseline = f.app.document.native_archive.clone();
    let _readonly = Readonly::new(&f.native);
    let destination = f.root.join("chosen-own.mathfolio");
    let mut prompts = Scripted {
        answer: Some(true),
        destination: Some(Ok(Some(destination.clone()))),
        ..Default::default()
    };
    assert!(
        f.app.save_with_prompts(false, &mut prompts),
        "{}",
        f.app.status
    );
    assert_eq!(prompts.calls, vec!["question", "destination"]);
    assert_eq!(f.app.path.as_deref(), Some(destination.as_path()));
    assert!(f.app.native_save_target.is_none());
    f.assert_cold(&destination, f.source());
    f.select(0, 1);
    f.frame(vec![egui::Event::Text("T".into())]);
    let changed = f.source().to_owned();
    let mut none = Scripted::default();
    assert!(
        f.app.save_with_prompts(false, &mut none),
        "{}",
        f.app.status
    );
    assert!(none.calls.is_empty());
    f.assert_cold(&destination, &changed);
    assert_eq!(std::fs::read(&f.native).unwrap(), original);
    assert_eq!(f.app.document.native_archive, baseline);
}

#[test]
fn readonly_chooser_error_and_invalid_extension_return_before_draft_preparation() {
    for invalid in [false, true] {
        let mut f = Fixture::new();
        start_preedit(&mut f);
        let _readonly = Readonly::new(&f.native);
        let before = snapshot(&f);
        let result = if invalid {
            Ok(Some(f.root.join("wrong.txt")))
        } else {
            Err("scripted choice failure".into())
        };
        let mut prompts = Scripted {
            answer: Some(true),
            destination: Some(result),
            ..Default::default()
        };
        assert!(!f.app.save_with_prompts(false, &mut prompts));
        assert_eq!(snapshot(&f), before);
        assert_eq!(prompts.calls, vec!["question", "destination"]);
        assert!(f.app.status.starts_with("Could not save:"));
    }
}

#[test]
fn choosing_same_readonly_path_keeps_lowlevel_refusal_and_explicit_save_as_skips_question() {
    let mut f = Fixture::new();
    f.frame(vec![egui::Event::Text("R".into())]);
    let _readonly = Readonly::new(&f.native);
    let original = std::fs::read(&f.native).unwrap();
    let identity = (f.app.path.clone(), f.app.native_save_target.clone());
    let history = (f.app.undo.len(), f.app.redo.len());
    let mut prompts = Scripted {
        answer: Some(true),
        destination: Some(Ok(Some(f.native.clone()))),
        ..Default::default()
    };
    assert!(!f.app.save_with_prompts(false, &mut prompts));
    assert_eq!(prompts.calls, vec!["question", "destination"]);
    assert_eq!(std::fs::read(&f.native).unwrap(), original);
    assert_eq!(
        (f.app.path.clone(), f.app.native_save_target.clone()),
        identity
    );
    assert_eq!((f.app.undo.len(), f.app.redo.len()), history);
    assert!(f.app.dirty);
    let destination = f.root.join("explicit-save-as.mcdx");
    let mut prompts = Scripted {
        destination: Some(Ok(Some(destination.clone()))),
        ..Default::default()
    };
    assert!(
        f.app.save_with_prompts(true, &mut prompts),
        "{}",
        f.app.status
    );
    assert_eq!(prompts.calls, vec!["destination"]);
    f.assert_cold(&destination, f.source());
    assert_eq!(std::fs::read(&f.native).unwrap(), original);
}

#[test]
fn missing_current_native_path_does_not_guess_readonly_or_retry_writer_failure() {
    let mut f = Fixture::new();
    std::fs::remove_file(&f.native).unwrap();
    let identity = (f.app.path.clone(), f.app.native_save_target.clone());
    let history = (f.app.undo.len(), f.app.redo.len());
    let mut prompts = Scripted::default();
    assert!(!f.app.save_with_prompts(false, &mut prompts));
    assert!(prompts.calls.is_empty());
    assert!(!f.native.exists());
    assert_eq!(
        (f.app.path.clone(), f.app.native_save_target.clone()),
        identity
    );
    assert_eq!((f.app.undo.len(), f.app.redo.len()), history);
}
